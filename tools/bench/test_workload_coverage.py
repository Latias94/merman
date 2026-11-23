import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

if __package__:
    from .test_workload_reference import report as reference_report
    from .workload_coverage import summarize_coverage
    from .workload_reference import compare_reference
else:
    from test_workload_reference import report as reference_report
    from workload_coverage import summarize_coverage
    from workload_reference import compare_reference


ROOT = Path(__file__).resolve().parents[2]


def encode(data):
    return json.dumps(data, ensure_ascii=False).encode("utf-8")


def example_plan():
    text = "flowchart TD\r\nA[Café]\r\n"

    def request(diagram_id, source, *labels, fixture="shared"):
        return dict(
            fixture=fixture,
            diagram_id=diagram_id,
            source=source,
            source_sha256=hashlib.sha256(source.encode("utf-8")).hexdigest(),
            expected_labels=list(labels),
        )

    document = [
        request("one", text, "Café"),
        request("two", text, "Café"),
        request("three", text.replace("\r\n", "\n"), "Café"),
        request("four", text.replace("é", "e\u0301")),
        request("five", text),
    ]
    return dict(
        schema_version=1,
        suite="test",
        scenarios=[
            dict(id="doc", origin="document", requests=document),
            dict(id="alias", requests=[request("six", text, "Café", fixture="other")]),
        ],
        topologies=[dict(id="alias", source="flowchart LR\nA-->B\n")],
    )


def example_comparison(raw_plan, surface="cli"):
    scenario = json.loads(raw_plan)["scenarios"][0]
    requests = [{k: v for k, v in r.items() if k != "source"} for r in scenario["requests"]]
    times = [(1,) * len(requests)] * 2
    left = reference_report(surface, "merman", times)
    right = reference_report(surface, "mmdc" if surface == "cli" else "pele", times)
    for report in (left, right):
        report["plan_sha256"] = hashlib.sha256(raw_plan).hexdigest()
        report["scenarios"][0].update(id=scenario["id"], requests=requests)
    if surface == "cli":
        batches = right["scenarios"][0]["batches"]
        for batch in batches:
            batch["results"][1]["status"] = "invalid_output"
        batches[0]["results"][4]["status"] = "timeout"
        left["scenarios"][0]["batches"][0]["results"][2]["sha256"] = "b" * 64
    else:
        left["package"]["name"] = "@mermanjs/node-wasm"
        left["backend"] = "node-wasm"
    return compare_reference(left, right)


class WorkloadCoverageTests(unittest.TestCase):
    def test_family_baselines_count_failed_unstable_and_unreported_independently(self):
        source = example_plan()["scenarios"][1]["requests"][0]
        plan = {"schema_version": 1, "scenarios": [dict(id=f"family-{name}", family=name, requests=[source])
                                                    for name in ("er", "state", "pie", "packet")]}
        raw = encode(plan)
        comparison = example_comparison(raw, "node")
        first = comparison["scenarios"][0]
        for family in ("state", "pie"):
            row = deepcopy(first)
            row["id"] = f"family-{family}"
            row["common_stable"] = 0
            if family == "state":
                row["requests"][0]["left"].update(state="unstable-output")
            else:
                row["requests"][0]["right"].update(state="missing", statuses=["missing-labels"])
                row["common_completed"] = 0
            comparison["scenarios"].append(row)
        report = summarize_coverage(raw, [comparison])
        self.assertEqual(report["planned"]["family_baselines"], 4)
        counts = report["comparisons"][0]["family_baselines"]
        self.assertEqual((counts["planned"], counts["reported"], counts["both_stable"], counts["unreported"]), (4, 3, 1, ["packet"]))
        for side, completed, missing in (("left", 3, "state"), ("right", 2, "pie")):
            self.assertEqual((counts[side]["completed"], counts[side]["stable"]), (completed, 2))
            self.assertEqual([g["family"] for g in counts[side]["gaps"]], [missing])
        plan["scenarios"][-1]["family"] = "state"
        with self.assertRaisesRegex(ValueError, "uniquely identified family baseline"):
            summarize_coverage(encode(plan))

    def test_plan_counts_exact_bytes_occurrences_and_independent_topologies(self):
        plan = example_plan()
        del plan["scenarios"][0]["requests"][0]["source_sha256"]
        result = summarize_coverage(encode(plan))
        self.assertEqual(
            result["planned"],
            dict(
                scenarios=2, requests=6, distinct_sources=3, topology_rows=1,
                with_expected_labels=dict(requests=4, distinct_sources=2),
            ),
        )
        doc, alias = result["scenarios"]
        self.assertEqual((doc["id"], alias["id"]), ("doc", "alias"))
        self.assertEqual(doc["with_expected_labels"], dict(requests=3, distinct_sources=2))
        self.assertEqual(
            doc["requests_without_expected_labels"],
            [dict(fixture="shared", diagram_id="four"), dict(fixture="shared", diagram_id="five")],
        )
        self.assertEqual(result["comparisons"], [])

    def test_completed_and_stable_label_checks_follow_the_reported_occurrences(self):
        raw = encode(example_plan())
        comparison = example_comparison(raw)
        (observation,) = summarize_coverage(raw, [comparison])["comparisons"]
        self.assertEqual(observation["unreported_scenarios"], ["alias"])
        for side in ("left", "right"):
            self.assertEqual(observation[side], comparison[side])
        (row,) = observation["scenarios"]
        self.assertEqual(row["planned_requests"], 5)
        for key, total, labels in (("common_completed", 3, 2), ("common_stable", 2, 1)):
            counts = row[key]
            self.assertEqual(counts["requests"], total)
            self.assertEqual(
                counts["label_verified"], dict(requests=labels, distinct_sources=labels)
            )
            self.assertEqual(counts["mechanical_only"], 1)
        issues = row["non_stable_requests"]
        self.assertEqual([r["diagram_id"] for r in issues], ["two", "three", "five"])
        self.assertEqual([r["fixture"] for r in issues], ["shared"] * 3)
        self.assertEqual(issues[0]["right"]["statuses"], ["invalid_output"])
        self.assertEqual(issues[1]["left"]["state"], "unstable-output")
        self.assertEqual(issues[2]["right"]["state"], "intermittent")
        self.assertNotIn("common_render_ms", row)

    def test_rejects_wrong_plan_identity_order_and_claimed_counts(self):
        plan = example_plan()
        raw = encode(plan)
        comparison = example_comparison(raw)
        with self.assertRaisesRegex(ValueError, "external workload comparison"):
            summarize_coverage(raw, [reference_report("cli", "merman")])
        with self.assertRaisesRegex(ValueError, "plan_sha256"):
            summarize_coverage(raw + b"\n", [comparison])
        doc = comparison["scenarios"][0]
        for names in (["alias", "doc"], ["doc", "doc"], ["unknown"]):
            rows = [{**doc, "id": name} for name in names]
            with self.subTest(names=names), self.assertRaisesRegex(ValueError, "order|duplicate"):
                summarize_coverage(raw, [{**comparison, "scenarios": rows}])
        for key, value in (
            ("requests", list(reversed(doc["requests"]))),
            ("requests", doc["requests"][:-1]),
            ("planned_requests", 3),
            ("common_completed", 2),
            ("common_stable", 4),
        ):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "request|intersection"):
                summarize_coverage(raw, [{**comparison, "scenarios": [{**doc, key: value}]}])
        for key, value in (
            ("expected_labels", []), ("fixture", "renamed"), ("source_sha256", "0" * 64)
        ):
            changed = deepcopy(comparison)
            changed["scenarios"][0]["requests"][0][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "request identity"):
                summarize_coverage(raw, [changed])
    def test_rejects_invalid_plan_occurrences_and_topology_hashes(self):
        plan = example_plan()
        for key, value in (
            ("source_sha256", "a" * 64), ("diagram_id", "two"), ("expected_labels", "Café")
        ):
            changed = deepcopy(plan)
            changed["scenarios"][0]["requests"][0][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                summarize_coverage(encode(changed))
        plan["topologies"][0]["source_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "source_sha256"):
            summarize_coverage(encode(plan))

    def test_cli_stdout_and_file_accept_multiple_product_pairs(self):
        raw = encode(example_plan())
        cli = example_comparison(raw)
        node = example_comparison(raw, "node")
        scratch = ROOT / "target"
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as temp:
            folder = Path(temp)
            (folder / "plan.json").write_bytes(raw)
            (folder / "cli.json").write_bytes(encode(cli))
            (folder / "node.json").write_bytes(encode(node))
            cmd = [
                sys.executable, "-B", ROOT / "tools/bench/perf_runner.py", "workloads", "coverage",
                "--plan", "plan.json", "--comparison", "node.json", "--comparison", "cli.json",
            ]
            stdout = subprocess.check_output(cmd, cwd=folder)
            output = folder / "results" / "coverage.json"
            subprocess.run([*cmd, "--out", output], cwd=folder, check=True)
            self.assertEqual(stdout, output.read_bytes())
        comparisons = json.loads(stdout)["comparisons"]
        self.assertEqual([c["surface"] for c in comparisons], ["node", "cli"])
        for side in ("left", "right"):
            self.assertEqual(comparisons[0][side], node[side])
        stable = comparisons[0]["scenarios"][0]["common_stable"]
        self.assertEqual(stable["label_verified"], dict(requests=3, distinct_sources=2))


if __name__ == "__main__":
    unittest.main()
