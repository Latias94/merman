from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import tempfile
import unittest

if __package__:
    from .test_workload_reference import report as worker_report
    from .workload_coverage import summarize_coverage
    from .workload_reference import compare_reference, node_worker_failures
    from .workload_report_node import load_matrix, load_rounds
else:
    from test_workload_reference import report as worker_report
    from workload_coverage import summarize_coverage
    from workload_reference import compare_reference, node_worker_failures
    from workload_report_node import load_matrix, load_rounds


def write(path, value):
    path.write_bytes(json.dumps(value).encode())


class NodeReaderReportTests(unittest.TestCase):
    def setUp(self):
        scratch = Path(__file__).resolve().parents[2] / "target"
        scratch.mkdir(exist_ok=True)
        tmp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.secret = 'C:\\private\\worker.log https://example.invalid/logs <svg>private</svg>'
        names = ("pie", "zenuml", "drawing-failure", "drawing-unchecked", "reject", "timeout", "document")
        self.plan = dict(schema_version=1, suite="quick", scenarios=[])
        for name in names:
            request = dict(fixture="one", diagram_id="repeated-id", source="flowchart LR\nA[private source]", expected_labels=[])
            scenario = dict(id=name, origin="https://example.invalid/input", requests=[request])
            if name in ("pie", "zenuml"):
                scenario["family"] = name
            if name in ("pie", "drawing-failure", "drawing-unchecked"):
                request["drawing_facts"] = {"label_widths": {"A": "private source"}}
            if name == "reject":
                request["expected_outcome"] = "reject"
            if name == "document":
                scenario["requests"].append(deepcopy(request))
            self.plan["scenarios"].append(scenario)
        write(self.root / "plan.json", self.plan)
        self.raw_plan = (self.root / "plan.json").read_bytes()
        self.plan_hash = sha256(self.raw_plan).hexdigest()
        (self.root / "consumer").mkdir()
        (self.root / "consumer/package-lock.json").write_bytes(b'{}\n')
        (self.root / "consumer/package.json").write_bytes(b'{}\n')

    def matrix(self, folder):
        raw = {}
        for engine in ("merman", "pele"):
            product = worker_report("node", engine, [(2,), (4,)], wasm=engine == "merman")
            product["host"].update(hostname="private-workstation", node="v22.3.4", cpu="Test CPU @ 2.5 GHz")
            product.update(plan_sha256=self.plan_hash, planned_scenarios=len(self.plan["scenarios"]),
                           node_project=dict(cwd="/private/project", digests={"package-lock.json": sha256(b'{}\n').hexdigest()}))
            template = product["scenarios"].pop()
            for index, scenario in enumerate(self.plan["scenarios"]):
                name = scenario["id"]
                request = scenario["requests"][0]
                row = deepcopy(template)
                row.update(id=name, index=index, requests=[dict(fixture=request["fixture"], diagram_id=request["diagram_id"],
                           source_sha256=sha256(request["source"].encode()).hexdigest(), drawing_required="drawing_facts" in request,
                           expected_outcome=request.get("expected_outcome", "draw"))])
                for batch in [row["cold_batch"], *row["warm_samples"]]:
                    if name == "reject" or (engine == "pele" and name == "zenuml"):
                        batch["results"] = [dict(status="expected-rejection" if name == "reject" else "render-error", error=self.secret)]
                    batch["results"] *= len(scenario["requests"])
                row["requests"] *= len(scenario["requests"])
                if name == "timeout" and engine == "pele":
                    product["status"] = "failed"
                    row.update(status="timeout", unmeasured_requests=1, error=self.secret, cold_batch=None, warm_samples=[],
                               init_ms=None, first_render_ms=None)
                product["scenarios"].append(row)
            raw[engine] = product
        drawings = {e: {"pie": {"status": "passed"}, "drawing-failure": {"status": "failed" if e == "pele" else "passed"}}
                    for e in raw}
        comparison = compare_reference(raw["merman"], raw["pele"], drawing_products=drawings)
        coverage = summarize_coverage(self.raw_plan, [comparison])
        runs = {e: dict(package=p["package"]["name"], version=p["package"]["version"], worker_failures=node_worker_failures(p))
                for e, p in raw.items()}
        runs["mermaid-v13"] = dict(report=self.secret, error=self.secret)
        coverage["collection"] = dict(status="incomplete", runs=runs, errors={"mermaid-v13": self.secret})
        write(folder / "coverage.json", coverage)
        write(folder / "merman-vs-pele.json", comparison)
        return comparison

    def assert_public(self, data):
        serialized = json.dumps(data, allow_nan=False)
        for private in ("private", "https://", "<svg", "/products/", "hostname", "withheld_reason", "output_bytes"):
            self.assertNotIn(private, serialized)

    def test_one_pass_intersections_drawing_gaps_failed_and_absent_workers(self):
        self.matrix(self.root)
        data = load_matrix(self.root / "coverage.json")
        self.assertEqual((data["kind"], data["suite"], data["status"], data["latency"]), ("node-matrix", "quick", "incomplete", []))
        self.assertEqual(data["planned"], dict(scenarios=7, requests=8, distinct_sources=1, families=2))
        self.assertEqual(data["host"], dict(os="win32", arch="x64", cpu="Test CPU @ 2.5 GHz", runtime="v22.3.4"))
        self.assertEqual(data["hashes"], dict(plan_sha256=self.plan_hash, lock_sha256=sha256(b'{}\n').hexdigest()))
        merman, pele, missing = (data["products"][p] for p in ("merman", "pele", "mermaid-v13"))
        self.assertEqual((merman["backend"], merman["version"], merman["entry_sha256"], merman["collector_sha256"]),
                         ("node-wasm", "1.2.3", "f" * 64, "c" * 64))
        self.assertEqual(merman["transport_sha256"], {"wasm-glue": "f" * 64, "wasm-module": "f" * 64})
        self.assertEqual((pele["status"], pele["workers"]), ("incomplete", dict(planned=7, missing=1, failed=0, timeout=1)))
        self.assertEqual((missing["status"], missing["workers"]["missing"], missing["version"]), ("unavailable", 7, None))
        observed = data["coverage"]["pele"]
        self.assertEqual([observed[k] for k in ("common_completed", "common_stable", "common_expected_rejections")], [5, 5, 1])
        self.assertEqual([observed[k + "_scenarios"] for k in ("eligible", "failed", "unchecked", "withheld")], [2, 3, 1, 1])
        self.assertEqual(observed["drawing_checks"], dict(planned=3, passed=1, failed=1, unchecked=1))
        self.assertEqual(observed["family_gaps"], [dict(family="zenuml", planned=1, reported=1, merman_stable=1, peer_stable=0, both_stable=0)])
        absent = data["coverage"]["mermaid-v13"]
        self.assertEqual((absent["unchecked_scenarios"], absent["reported_requests"], absent["families"]["planned"]), (7, 0, 2))
        self.assert_public(data)

    def rounds(self):
        history = []
        for index in (1, 2):
            folder = self.root / "rounds" / f"{index:03d}"
            folder.mkdir(parents=True, exist_ok=True)
            comparison = self.matrix(folder)
            values = {1: dict(left=1, right=8, right_over_left=8), 2: dict(left=4, right=2, right_over_left=0.5)}
            for row in comparison["scenarios"]:
                if row["id"] in ("pie", "document"):
                    row["complete_batch_ms"] = values[index]
            write(folder / "merman-vs-pele.json", comparison)
            order = ["merman", "pele", "mermaid-v13"]
            history.append(dict(round=index, status="incomplete", order=order if index == 1 else order[::-1],
                                coverage=f"rounds/{index:03d}/coverage.json"))
        identities = {comparison[k]["engine"]: dict(product=comparison[k], host=comparison["host"], collector_sha256="c" * 64)
                      for k in ("left", "right")}
        first, second = dict(round=1, left=1, right=8, right_over_left=8), dict(round=2, left=4, right=2, right_over_left=0.5)
        complete = dict(status="complete", eligible_pairs=2, rounds=[first, second],
                        per_process_median_ms=dict(left={"median": 2.5}, right={"median": 5}),
                        paired_right_over_left=dict(min=0.5, median=4.25, max=8))
        partial = dict(complete, status="partial", eligible_pairs=1, rounds=[first, dict(round=2, withheld_reason=self.secret)])
        zero = dict(complete, status="unavailable", eligible_pairs=0, rounds=[dict(round=r, withheld_reason=self.secret) for r in (1, 2)])
        evidence = self.root / "process-rounds.json"
        results = {s["id"]: zero for s in self.plan["scenarios"]}
        results.update(pie=complete, document=partial)
        write(evidence, dict(schema_version=1, left="merman", classification="diagnostic", plan_sha256=self.plan_hash,
                             reference_identities=identities, collection_status="incomplete", process_rounds=2, warm_batches_per_worker=2,
                             rounds=history, comparisons={"pele": results}))
        return evidence

    def test_finalized_independent_pairs_keep_partial_observations_and_withhold_aggregates(self):
        evidence = self.rounds()
        data = load_rounds(evidence)
        rows = {(r["peer"], r["scenario"]): r for r in data["latency"]}
        self.assertEqual(len(rows), 14)
        full = rows["pele", "pie"]
        self.assertEqual((full["eligible_pairs"], full["planned_pairs"], full["merman_ms"], full["peer_ms"], full["ratio"]),
                         (2, 2, 2.5, 5, dict(min=0.5, median=4.25, max=8)))
        part = rows["pele", "document"]
        self.assertEqual((full["requests_per_batch"], part["requests_per_batch"]), (1, 2))
        self.assertEqual((part["reasons"], rows["pele", "reject"]["reasons"], rows["pele", "timeout"]["reasons"]),
                         (["collector-withheld"], ["expected-rejection"], ["worker-failure"]))
        self.assertEqual(part["pairs"], [dict(round=1, merman_ms=1, peer_ms=8, ratio=8)])
        for row in (part, rows["pele", "reject"], rows["mermaid-v13", "pie"]):
            self.assertEqual([row[k] for k in ("merman_ms", "peer_ms", "ratio")], [None] * 3)
            self.assertEqual(row["planned_pairs"], 2)
        self.assertEqual(rows["mermaid-v13", "pie"]["pairs"], [])
        c = data["coverage"]["pele"]
        self.assertEqual((c["planned_requests"], c["common_completed"], c["eligible_scenarios"]), (16, 10, 3))
        self.assertEqual(data["products"]["mermaid-v13"]["workers"]["missing"], 14)
        self.assert_public(data)

    def test_stale_rounds_and_missing_sidecars_withhold_only_the_affected_pairs(self):
        for defect, reason in (("wrong-link", "evidence-mismatch"), ("missing-round", "missing-evidence"),
                               ("wrong-plan", "evidence-mismatch"), ("missing-comparison", "missing-evidence"),
                               ("stale-median", "evidence-mismatch"), ("missing-audit", "missing-evidence"),
                               ("lane-error", "collection-incomplete")):
            with self.subTest(defect=defect):
                evidence = self.rounds()
                data = json.loads(evidence.read_bytes())
                folder = self.root / "rounds/002"
                coverage = json.loads((folder / "coverage.json").read_bytes())
                if defect == "wrong-link":
                    data["rounds"][1]["coverage"] = self.secret
                elif defect == "missing-round":
                    data["rounds"].pop()
                elif defect == "wrong-plan":
                    coverage["plan_sha256"] = "1" * 64
                elif defect == "missing-comparison":
                    (folder / "merman-vs-pele.json").unlink()
                elif defect == "stale-median":
                    data["comparisons"]["pele"]["pie"]["rounds"][1]["left"] = 99
                elif defect == "missing-audit":
                    coverage["drawing_audit"] = "drawing-audit.json"
                else:
                    coverage["collection"]["errors"]["merman-vs-pele.json"] = self.secret
                write(folder / "coverage.json", coverage)
                write(evidence, data)
                projected = load_rounds(evidence)
                row = next(r for r in projected["latency"] if (r["peer"], r["scenario"]) == ("pele", "pie"))
                self.assertEqual(projected["status"], "incomplete")
                self.assertEqual((row["status"], row["eligible_pairs"], row["planned_pairs"], row["ratio"], row["reasons"]),
                                 ("partial", 1, 2, None, [reason]))
                self.assertEqual([p["round"] for p in row["pairs"]], [1])
                self.assert_public(projected)

    def test_missing_or_replaced_frozen_lock_withholds_independent_pairs(self):
        evidence = self.rounds()
        lock = self.root / "consumer/package-lock.json"
        lock.write_bytes(b'{}')
        for reason in ("frozen-input-mismatch", "missing-frozen-input"):
            with self.subTest(reason=reason):
                if reason == "missing-frozen-input":
                    lock.unlink()
                result = load_rounds(evidence)
                row = result["latency"][0]
                self.assertEqual((result["status"], row["reasons"], row["eligible_pairs"], row["merman_ms"], row["pairs"]),
                                 ("incomplete", [reason], 0, None, []))

    def test_unscoped_incomplete_collection_revokes_stale_success(self):
        self.matrix(self.root)
        path = self.root / "coverage.json"
        coverage = json.loads(path.read_bytes())
        del coverage["collection"]["runs"]["mermaid-v13"]
        coverage["collection"]["runs"]["pele"]["worker_failures"] = []
        coverage["collection"]["errors"] = {}
        write(path, coverage)
        result = load_matrix(self.root)
        self.assertEqual(result["status"], "incomplete")
        c = result["coverage"]["pele"]
        self.assertEqual((c["eligible_scenarios"], c["unchecked_scenarios"], c["common_stable"], c["reasons"]),
                         (0, 7, 0, ["collection-incomplete"]))

    def test_mixed_plans_are_rejected_before_using_finalized_counts(self):
        self.matrix(self.root)
        (self.root / "plan.json").write_bytes(self.raw_plan + b"\n")
        with self.assertRaisesRegex(ValueError, "different frozen plan"):
            load_matrix(self.root)
        (self.root / "plan.json").unlink()
        with self.assertRaisesRegex(ValueError, "missing or invalid frozen Node plan"):
            load_matrix(self.root)


if __name__ == "__main__":
    unittest.main()
