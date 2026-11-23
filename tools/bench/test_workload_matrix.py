from collections import Counter
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from test_workload_drawing import browser_reply
from test_workload_reference import report as node_report
from workload_drawing import audit_drawings
from workload_matrix import collect_node_matrix
from workload_reference import NODE_PRODUCTS, compare_reference


HERE = Path(__file__).resolve().parent
SCRATCH = HERE.parents[1] / "target"


class NodeMatrixTests(unittest.TestCase):
    def test_only_matching_passed_drawings_release_a_declared_scenario_ratio(self):
        for left, right in [("passed", "passed"), ("passed", "failed"), ("unavailable", "passed"), (None, None)]:
            products = {name: {"doc": {"status": state}} for name, state in (("merman", left), ("pele", right)) if state}
            raw = [node_report("node", name) for name in ("merman", "pele")]
            for report in raw:
                report["scenarios"][0]["requests"][0]["drawing_required"] = True
            result = compare_reference(*raw, drawing_products=products)
            row = result["scenarios"][0]
            self.assertEqual(row["drawing_checks"], {"left": left or "not-checked", "right": right or "not-checked"})
            for key in ("complete_batch_ms", "common_render_ms"):
                self.assertGreater(row[key]["left"], 0)
                self.assertEqual(row[key]["right_over_left"] is not None, left == right == "passed")

    def test_audited_plan_still_gates_older_collectors_without_request_flags(self):
        reports = [node_report("node", name) for name in ("merman", "pele")]
        for status in ("passed", "failed", "unavailable"):
            products = {name: {"doc": {"status": state, "cases": [{"request_index": 0}]}}
                        for name, state in (("merman", "passed"), ("pele", status))}
            row = compare_reference(*reports, drawing_products=products)["scenarios"][0]
            self.assertTrue(row["requests"][0]["drawing_required"])
            self.assertEqual(row["drawing_checks"], {"left": "passed", "right": status})
            self.assertEqual(row["complete_batch_ms"]["right_over_left"] is not None, status == "passed")

    def test_failed_peer_keeps_the_other_comparison_and_raw_inputs(self):
        SCRATCH.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            root = Path(tmp)
            source = "flowchart LR\nA[Alpha] --> B"
            request = dict(fixture="example", diagram_id="id-0", source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                           expected_labels=["Alpha"])
            data = dict(schema_version=1, scenarios=[dict(id="doc", requests=[dict(request, source=source)])], topologies=[])
            plan_bytes = json.dumps(data).encode()
            args = SimpleNamespace(plan=root / "input.json", out_dir=root / "results", peer=None, samples=2,
                                   timeout_seconds=30, merman_module=None, check_drawings=False, chrome=None)
            args.plan.write_bytes(plan_bytes)
            order = []

            def collect(cmd, *, cwd, stdout):
                name = cmd[cmd.index("--engine") + 1]
                order.append(name)
                if name == "pele":
                    stdout.write(b'{"error":"could not load the package"}')
                    return SimpleNamespace(returncode=1)
                report = node_report("node", "merman" if name == "merman" else "pele", [(1,), (2,)])
                manifest = json.loads((cwd / "package.json").read_bytes())["dependencies"]
                package = "@mermanjs/node" if name == "merman" else "mermaid"
                report["package"].update(name=package, version=manifest[package])
                report.update(engine=name, plan_sha256=hashlib.sha256(plan_bytes).hexdigest(),
                              node_project={"cwd": str(cwd)})
                if name == "mermaid-v13":
                    report["render_api"] = "mermaid.render"
                report["scenarios"][0]["requests"] = [request]
                stdout.write(json.dumps(report).encode())
                return SimpleNamespace(returncode=0)

            with patch("workload_matrix.subprocess.run", side_effect=collect):
                with self.assertRaisesRegex(ValueError, "incomplete Node collection"):
                    collect_node_matrix(args)
                with self.assertRaises(FileExistsError):
                    collect_node_matrix(args)
            self.assertEqual(order, ["merman", "pele", "mermaid-v13"])
            self.assertEqual((args.out_dir / "plan.json").read_bytes(), plan_bytes)
            coverage = json.loads((args.out_dir / "coverage.json").read_bytes())
            self.assertEqual(coverage["collection"]["status"], "incomplete")
            self.assertEqual(coverage["collection"]["errors"], {"pele": "could not load the package"})
            self.assertEqual(coverage["comparisons"][0]["right"]["engine"], "mermaid-v13")
            self.assertFalse((args.out_dir / "merman-vs-pele.json").exists())
            self.assertIn("| doc | 1/1 | 1 |", (args.out_dir / "summary.md").read_text(encoding="utf-8"))

    def test_matrix_preserves_drawings_and_both_pairs_across_throw_exit_and_timeout(self):
        SCRATCH.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            root = Path(tmp)
            project = root / "node-reference"
            project.mkdir()
            for file in ("workload_node.mjs", "workload_node_products.json", "workload_svg.py"):
                shutil.copyfile(HERE / file, root / file)
            for file in ("package.json", "package-lock.json"):
                shutil.copyfile(HERE / "node-reference" / file, project / file)
            versions = json.loads((project / "package.json").read_bytes())["dependencies"]
            svg = "'<svg><text>Alpha</text></svg>'"
            for name, code in {
                "@mermanjs/node": f"""export function createNodeEngine() {{ return {{
                    runtimeCatalog: {{package_version: {json.dumps(versions['@mermanjs/node'])}}},
                    renderSvgSync(source) {{
                        if (source === 'throw') throw new Error('test render exception');
                        if (source === 'exit') process.exit(73);
                        if (source === 'timeout') Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
                        return {svg};
                    }}, dispose() {{}}
                }}; }}""",
                "pele": f"export const render = () => ({{ svg: {svg} }});",
                "mermaid": f"export default {{ async render() {{ return {{ svg: {svg} }}; }} }};",
            }.items():
                pkg = project / "node_modules" / name
                pkg.mkdir(parents=True)
                (pkg / "package.json").write_text(json.dumps(dict(name=name, version=versions[name], exports="./index.mjs")))
                (pkg / "index.mjs").write_text(code)

            names = ["before", "throw", "exit", "timeout", "after"]
            rows = [dict(id=name, requests=[dict(source=name, fixture="one", diagram_id=name, expected_labels=["Alpha"],
                                                  drawing_facts={"label_widths": {"A": "Alpha"}})]) for name in names]
            plan = root / "input.json"
            plan.write_text(json.dumps(dict(schema_version=1, scenarios=rows)))
            dest = root / "bundle"
            args = SimpleNamespace(plan=plan, out_dir=dest, peer=None, samples=1, timeout_seconds=4,
                                   merman_module=None, check_drawings=True, chrome=None)
            def audit(plan, reports, *, chrome):
                # Only substitute the browser verdicts. The audit reads the actual retained SVGs.
                fact = dict(kind="label-width", node="A", label="Alpha", status="unavailable")
                reply = browser_reply([[fact]] * (2 + 2 * len(names)))
                with patch("workload_drawing.subprocess.run", return_value=reply):
                    return audit_drawings(plan, reports, chrome=chrome)

            with patch("workload_matrix.HERE", root), patch("workload_matrix.audit_drawings", audit):
                with self.assertRaisesRegex(ValueError, "incomplete Node collection"):
                    collect_node_matrix(args)
            coverage = json.loads((dest / "coverage.json").read_bytes())
            self.assertEqual(set(coverage["collection"]["errors"]), {"merman"})
            failures = coverage["collection"]["runs"]["merman"]["worker_failures"]
            self.assertEqual([(f["id"], f["status"]) for f in failures], [("exit", "failed"), ("timeout", "timeout")])
            products = json.loads((dest / "drawing-audit.json").read_bytes())["products"]
            for product in products:
                observed = [n for n, row in product["scenarios"].items() if "artifact" in row["cases"][0]]
                self.assertEqual(observed, ["before", "after"] if product["engine"] == "merman" else names)
            cases = products[0]["scenarios"]
            self.assertEqual([cases[n]["cases"][0]["reason"] for n in names[1:-1]], ["renderer-failed", "worker-failed", "worker-timeout"])
            self.assertIn("artifact", cases["after"]["cases"][0])
            for peer, summary in zip(("pele", "mermaid-v13"), coverage["comparisons"], strict=True):
                compared = json.loads((dest / f"merman-vs-{peer}.json").read_bytes())["scenarios"]
                self.assertEqual([r["id"] for r in compared], names)
                self.assertEqual(summary["unreported_scenarios"], [])
                self.assertEqual([r["requests"][0]["left"]["state"] for r in compared],
                                 ["stable", "missing", "unmeasured", "unmeasured", "stable"])
                self.assertTrue(all(r["requests"][0]["right"]["state"] == "stable" for r in compared))
                self.assertTrue(all(r["common_render_ms"] is None and r["complete_batch_ms"] is None for r in compared[1:-1]))
                self.assertIsNotNone(compared[-1]["complete_batch_ms"]["left"])
            markdown = (dest / "summary.md").read_text(encoding="utf-8")
            self.assertIn("timeout · worker timeout · requests unmeasured", markdown)
            self.assertIn("| after | 1/1 | 1 |", markdown)


class ProcessRoundTests(unittest.TestCase):
    def setUp(self):
        SCRATCH.mkdir(exist_ok=True)
        tmp = tempfile.TemporaryDirectory(dir=SCRATCH)
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        self.args = SimpleNamespace(plan=root / "input.json", out_dir=root / "bundle", peer=None,
                                    process_rounds=2, samples=2, timeout_seconds=30,
                                    merman_module=None, check_drawings=False, chrome=None)
        self.freeze({"doc": ["z", "a"], "control": ["c"]})
        self.calls = []
        self.edit = lambda round_no, report: None
        collector = patch("workload_matrix.subprocess.run", side_effect=self.collect)
        self.addCleanup(collector.stop)
        collector.start()

    def freeze(self, documents):
        scenarios = [dict(id=name, requests=[dict(fixture="shared", source=key, diagram_id=key, expected_labels=[key])
                                             for key in keys]) for name, keys in documents.items()]
        self.args.plan.write_bytes(json.dumps(dict(schema_version=1, scenarios=scenarios)).encode())

    def collect(self, cmd, *, cwd, stdout):
        engine = cmd[cmd.index("--engine") + 1]
        round_no = int(Path(stdout.name).parent.name)
        self.calls.append((round_no, engine))
        plan = Path(cmd[cmd.index("--plan") + 1]).read_bytes()
        self.assertEqual(plan, self.args.plan.read_bytes())
        samples = int(cmd[cmd.index("--samples") + 1])
        product = NODE_PRODUCTS[engine]
        package = product["package"]
        version = json.loads((cwd / "package.json").read_bytes())["dependencies"][package]
        raw = node_report("node", engine)
        entry = raw["artifact_identity"]["files"][0]
        raw.update(plan_sha256=hashlib.sha256(plan).hexdigest(), samples=samples, timeout_seconds=self.args.timeout_seconds,
                   package=dict(name=package, version=version, entry=entry["path"], entry_sha256=entry["sha256"]),
                   render_api=product["render_api"], node_project=dict(cwd=str(cwd), digests={
                       f: hashlib.sha256((cwd / f).read_bytes()).hexdigest() for f in ("package.json", "package-lock.json")}),
                   scenarios=[])
        for index, scenario in enumerate(json.loads(plan)["scenarios"]):
            requests = scenario["requests"]
            times = [tuple(range(1, len(requests) + 1))] * samples
            row, = node_report("node", engine, times)["scenarios"]
            row.update(index=index, id=scenario["id"])
            for i, request in enumerate(requests):
                row["requests"][i] = {k: v for k, v in request.items() if k not in ("source", "drawing_facts")}
                row["requests"][i]["source_sha256"] = hashlib.sha256(request["source"].encode()).hexdigest()
                if "drawing_facts" in request:
                    row["requests"][i]["drawing_required"] = True
            raw["scenarios"].append(row)
        raw["planned_scenarios"] = len(raw["scenarios"])
        self.edit(round_no, raw)
        stdout.write(json.dumps(raw).encode())
        return SimpleNamespace(returncode=int(bool(raw.get("error") or raw.get("status") == "failed")))

    def run_matrix(self, *, incomplete=False):
        if incomplete:
            with self.assertRaisesRegex(ValueError, "incomplete Node collection.*process-rounds.json"):
                collect_node_matrix(self.args)
        else:
            self.assertEqual(collect_node_matrix(self.args), self.args.out_dir / "summary.md")
        return json.loads((self.args.out_dir / "process-rounds.json").read_bytes())

    def test_order_and_paired_distribution_from_each_workers_warm_median(self):
        self.args.process_rounds = 6
        left = [2, 4, 10, 20, 30, 40]
        right = [8, 4, 2, 10, 60, 20]

        def set_times(round_no, raw):
            value = (left if raw["engine"] == "merman" else right)[round_no - 1]
            for row in raw["scenarios"]:
                row["cold_batch"]["complete_batch_ms"] = 50_000
                for batch, millis in zip(row["warm_samples"], (value - 1, value + 1), strict=True):
                    batch["complete_batch_ms"] = millis

        self.edit = set_times
        report = self.run_matrix()
        self.assertEqual((report["classification"], report["process_rounds"], report["warm_batches_per_worker"]),
                         ("diagnostic", 6, 2))
        observed = [r["order"] for r in report["rounds"]]
        self.assertEqual(self.calls, [(i, engine) for i, order in enumerate(observed, 1) for engine in order])
        self.assertEqual(len(set(map(tuple, observed))), 6)
        for forward, reverse in zip(observed[::2], observed[1::2], strict=True):
            self.assertEqual(forward, reverse[::-1])
        for positions in zip(*observed, strict=True):
            self.assertEqual(Counter(positions), {engine: 2 for engine in NODE_PRODUCTS})
        for peer in ("pele", "mermaid-v13"):
            row = report["comparisons"][peer]["doc"]
            self.assertEqual((row["status"], row["eligible_pairs"]), ("complete", 6))
            self.assertEqual(row["paired_right_over_left"], dict(min=0.2, median=0.75, max=4))
            self.assertEqual(row["direction_counts"], dict(below_one=3, equal_one=1, above_one=2))
            self.assertEqual(row["per_process_median_ms"]["left"], dict(min=2, median=15, max=40))
            self.assertEqual([x["right_over_left"] for x in row["rounds"]], [4, 1, 0.2, 0.5, 2, 0.5])
        for item in report["rounds"]:
            directory = (self.args.out_dir / item["coverage"]).parent
            compared = json.loads((directory / "merman-vs-pele.json").read_bytes())
            self.assertEqual([r["diagram_id"] for r in compared["scenarios"][0]["requests"]], ["z", "a"])
        summary = (self.args.out_dir / "summary.md").read_text(encoding="utf-8")
        self.assertIn("6 fresh workers per product and scenario", summary)
        self.assertIn("2 warm batches", summary)
        with self.assertRaises(FileExistsError):
            collect_node_matrix(self.args)

    def test_incomplete_collectors_keep_other_rounds_peers_and_scenarios(self):
        def fail(round_no, raw):
            if round_no == 1 and raw["engine"] == "mermaid-v13":
                raw["error"] = "package could not load"
            if round_no == 2 and raw["engine"] == "merman":
                raw["status"] = "failed"
                raw["scenarios"][0].update(status="timeout", error="worker timed out", unmeasured_requests=2,
                                           cold_batch=None, warm_samples=[], init_ms=None, first_render_ms=None)

        self.edit = fail
        report = self.run_matrix(incomplete=True)
        self.assertEqual(len(self.calls), 6)
        self.assertEqual(report["collection_status"], "incomplete")
        pele = report["comparisons"]["pele"]
        self.assertEqual((pele["control"]["status"], pele["control"]["eligible_pairs"]), ("complete", 2))
        self.assertEqual((pele["doc"]["status"], pele["doc"]["eligible_pairs"]), ("partial", 1))
        self.assertIn("merman worker timeout", pele["doc"]["rounds"][1]["withheld_reason"])
        v13 = report["comparisons"]["mermaid-v13"]["doc"]
        self.assertEqual((v13["status"], v13["eligible_pairs"]), ("unavailable", 0))
        self.assertIn("package could not load", v13["rounds"][0]["withheld_reason"])
        for row in (pele["doc"], v13):
            for key in ("per_process_median_ms", "paired_right_over_left", "direction_counts"):
                self.assertIsNone(row[key])

    def test_stable_within_each_worker_can_drift_at_one_document_occurrence(self):
        self.args.peer = ["pele"]

        def change_output(round_no, raw):
            if round_no == 2 and raw["engine"] == "pele":
                row = raw["scenarios"][0]
                for batch in [row["cold_batch"], *row["warm_samples"]]:
                    batch["results"][1]["output_sha256"] = "e" * 64

        self.edit = change_output
        report = self.run_matrix()
        self.assertEqual(self.calls, [(1, "merman"), (1, "pele"), (2, "pele"), (2, "merman")])
        doc = report["comparisons"]["pele"]["doc"]
        self.assertIn("pele output changed at request 2 (a)", doc["rounds"][1]["withheld_reason"])
        self.assertIsNone(doc["paired_right_over_left"])
        self.assertTrue(report["comparisons"]["pele"]["control"]["paired_right_over_left"])

    def test_missing_unknown_or_changed_artifact_identity_keeps_raw_timings(self):
        self.args.peer = ["pele"]
        root = self.args.out_dir.parent
        for case in ("missing", "unknown", "changed", "collector"):
            with self.subTest(case=case):
                self.args.out_dir = root / case

                def change_identity(round_no, raw):
                    if round_no == 2 and case == "collector":
                        raw["collector_sha256"] = "e" * 64
                    elif round_no == 2 and raw["engine"] == "merman":
                        if case == "missing":
                            del raw["artifact_identity"]
                        elif case == "unknown":
                            raw["artifact_identity"].update(status="unknown", reason="unobserved addon")
                        else:
                            raw["artifact_identity"]["files"][1]["sha256"] = "b" * 64

                self.edit = change_identity
                report = self.run_matrix()
                doc = report["comparisons"]["pele"]["doc"]
                self.assertIsNone(doc["direction_counts"])
                reason = doc["rounds"][1]["withheld_reason"]
                self.assertIn("identity changed" if case in ("changed", "collector") else "identity is unknown", reason)
                self.assertIn(reason, (self.args.out_dir / "summary.md").read_text(encoding="utf-8"))
                directory = (self.args.out_dir / report["rounds"][1]["coverage"]).parent
                raw = json.loads((directory / "merman.json").read_bytes())
                self.assertGreater(raw["scenarios"][0]["warm_samples"][0]["complete_batch_ms"], 0)

    def test_drawing_failure_observer_drift_and_render_failure_only_gate_their_scenarios(self):
        self.args.check_drawings = True
        self.args.peer = ["pele"]
        self.args.process_rounds = 4
        self.freeze({"doc": ["first", "second"], "render": ["r"], "control": ["c"]})
        plan = json.loads(self.args.plan.read_bytes())
        plan["scenarios"][0]["requests"][1]["drawing_facts"] = {"label_widths": {"A": "second"}}
        self.args.plan.write_bytes(json.dumps(plan).encode())

        def render_error(round_no, raw):
            if round_no == 2 and raw["engine"] == "pele":
                row = raw["scenarios"][1]
                row["cold_batch"]["results"][0]["status"] = "missing-labels"

        def audit(plan, reports, *, chrome):
            products = []
            round_no = int(reports[0][0].parent.name)
            for _, raw in reports:
                passed = round_no != 2 or raw["engine"] == "merman"
                case = dict(status="passed" if passed else "failed", cases=[{"request_index": 1}])
                products.append(dict(engine=raw["engine"], scenarios={"doc": case}))
            return dict(products=products, observer={"browser": "Chrome/B" if round_no == 3 else "Chrome/A"})

        self.edit = render_error
        with patch("workload_matrix.audit_drawings", side_effect=audit) as observer:
            report = self.run_matrix()
        self.assertEqual(observer.call_count, 4)
        rows = report["comparisons"]["pele"]
        self.assertIn("pele drawing failed", rows["doc"]["rounds"][1]["withheld_reason"])
        self.assertIn("drawing observer identity", rows["doc"]["rounds"][2]["withheld_reason"])
        self.assertIn("0/1 stable", rows["render"]["rounds"][1]["withheld_reason"])
        for name, pairs in (("doc", 2), ("render", 3)):
            self.assertEqual(rows[name]["eligible_pairs"], pairs)
            self.assertIsNone(rows[name]["paired_right_over_left"])
        self.assertEqual(rows["control"]["status"], "complete")
        for item in report["rounds"]:
            self.assertTrue((self.args.out_dir / item["coverage"]).with_name("drawing-audit.json").exists())

    def test_reordered_requests_and_wrong_sample_count_keep_the_unaffected_peer(self):
        def malformed(round_no, raw):
            if raw["engine"] == "pele":
                if round_no == 1:
                    raw["scenarios"][0]["requests"].reverse()
                else:
                    raw["samples"] += 1

        self.edit = malformed
        report = self.run_matrix(incomplete=True)
        row = report["comparisons"]["pele"]["doc"]
        self.assertEqual((row["status"], row["eligible_pairs"]), ("unavailable", 0))
        self.assertIn("different request identity", row["rounds"][0]["withheld_reason"])
        self.assertIn("different sample count", row["rounds"][1]["withheld_reason"])
        self.assertEqual(report["comparisons"]["mermaid-v13"]["doc"]["status"], "complete")

    def test_rounds_require_even_count_at_least_two_before_writing(self):
        for count in (-2, 0, 1, 3, 2.0, True):
            self.args.process_rounds = count
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "even integer of at least 2"):
                collect_node_matrix(self.args)
        self.assertFalse(self.args.out_dir.exists())
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
