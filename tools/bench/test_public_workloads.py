import argparse
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_workloads as bench
from workload_svg import validate_svg


def plan(suite="quick", names=(), documents=()):
    return bench.materialize(
        argparse.Namespace(suite=suite, scenario=list(names), document=documents, pele_fixtures=None)
    )


class PublicWorkloadsTest(unittest.TestCase):
    def test_named_native_suite_replays_the_existing_fixture_selection(self):
        corpus = bench.load_corpus(bench.HERE / "corpus.json")
        for suite in ("cross_family", "full"):
            (case,) = plan(names=[f"corpus-{suite}"])["scenarios"]
            expected = bench.select_corpus_fixtures(corpus, suite)
            self.assertEqual([r["fixture"] for r in case["requests"]], [f.name for f in expected])
            for request, fixture in zip(case["requests"], expected, strict=True):
                self.assertEqual(request["source"], (bench.ROOT / fixture.source).read_text(encoding="utf-8"))
            labeled = {r["fixture"] for r in case["requests"] if r.get("expected_labels")}
            self.assertIn("er_medium", labeled)
            self.assertNotIn("error_basic", labeled)

    def test_full_appends_to_the_reviewed_quick_and_legacy_scenarios(self):
        quick = plan()["scenarios"]
        full = plan("full")["scenarios"]
        self.assertEqual(
            [s["id"] for s in quick],
            ["small-svg", "repeat-flow", "repeat-sequence", "mixed-families", "retained-configs"],
        )
        self.assertEqual(full[:5], quick)
        self.assertEqual(
            [s["id"] for s in full[5:10]],
            [
                "different-flowcharts",
                "mixed-document",
                "repeated-message-labels",
                "unique-message-labels",
                "merman-issue-guide",
            ],
        )
        unchecked = [
            r["diagram_id"] for s in full for r in s["requests"] if "expected_labels" not in r
        ]
        self.assertEqual(unchecked, [])

    def test_missing_builtin_label_facts_block_materialization(self):
        catalog = bench.load(bench.HERE / "workloads.json")
        empty = bench.scenario("shape", "local", [("shape", "pie\n\"\" : 1")], {"shape": []})
        self.assertEqual(empty["requests"][0]["expected_labels"], [])
        del catalog["expected_labels"]["er_medium"]
        with patch.object(bench, "load", return_value=catalog):
            with self.assertRaisesRegex(ValueError, "service-design-document/er_medium"):
                plan(names=["service-design-document"])

    def test_only_instantiated_svg_definitions_contribute_labels(self):
        source = '<svg xmlns:xlink="http://www.w3.org/1999/xlink"><defs><g id="label"><text>Start</text></g><symbol id="unused"><text>Hidden</text></symbol><g id="loop"><use href="#loop"/></g></defs><use href="#loop"/><use xlink:href="#label"/></svg>'
        self.assertEqual(validate_svg(source, ["Start"])["status"], "ok")
        self.assertEqual(validate_svg(source, ["Hidden"])["status"], "missing-labels")

    def test_mixed_workload_preserves_occurrence_order_source_and_labels(self):
        result = plan(names=["mixed-families", "retained-configs"])
        self.assertEqual(result, plan(names=["mixed-families", "retained-configs"]))
        mixed, retained = [s["requests"] for s in result["scenarios"]]
        self.assertEqual([r["fixture"] for r in mixed], ["flow-padding", "seq-message"] * 4)
        self.assertEqual([r["fixture"] for r in retained], ["flow-padding", "seq-margin"] * 4)
        self.assertEqual(len({r["diagram_id"] for r in mixed + retained}), 16)
        self.assertEqual(
            {r["source_sha256"] for r in mixed}, {bench.digest(r["source"].encode()) for r in mixed}
        )
        self.assertEqual(mixed[1]["expected_labels"], ["hi"])

    def test_custom_document_and_curated_document_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "a.mmd").write_bytes(b"flowchart LR\r\nA-->B\r\n")
            manifest = d / "doc.json"
            manifest.write_text(
                json.dumps({"id": "guide", "origin": "local/guide", "files": ["a.mmd", "a.mmd"]})
            )
            result = plan(names=["guide", "merman-issue-guide"], documents=[manifest])
            doc = json.loads(manifest.read_bytes())
            doc["expected_labels"] = {"a.mmd": ["A", "B"]}
            manifest.write_text(json.dumps(doc))
            (labeled,) = plan(names=["guide"], documents=[manifest])["scenarios"]
            self.assertEqual([r["expected_labels"] for r in labeled["requests"]], [["A", "B"], ["A", "B"]])
            doc["expected_labels"]["typo.mmd"] = ["unexpected"]
            manifest.write_text(json.dumps(doc))
            with self.assertRaisesRegex(ValueError, "refer to files"):
                plan(names=["guide"], documents=[manifest])
        local, issues = result["scenarios"]
        self.assertEqual([r["source"] for r in local["requests"]], ["flowchart LR\nA-->B\n"] * 2)
        self.assertNotIn("expected_labels", local["requests"][0])
        capture = (bench.ROOT / issues["origin"]).with_name("comparison.json")
        originals = bench.load(capture)["cases"]
        self.assertEqual([case["issue"] for case in originals], [183, 184, 185, 186, 187])
        self.assertEqual(
            [(r["fixture"], r["source"]) for r in issues["requests"]],
            [(f"block-{i}", case["source"]) for i, case in enumerate(originals, 1)],
        )
        facts = ["Tests pass?", "changes requested", "find user by email", "Deploy", "Merge"]
        for request, fact in zip(issues["requests"], facts, strict=True):
            self.assertIn(fact, request["expected_labels"], request["fixture"])

    def test_service_design_revisits_architecture_after_er_and_swimlane(self):
        requests = plan(names=["service-design-document"])["scenarios"][0]["requests"]
        self.assertEqual(
            [r["source"].split()[0] for r in requests],
            ["architecture-beta", "erDiagram", "swimlane-beta", "architecture-beta"],
        )
        self.assertEqual(requests[0]["source_sha256"], requests[-1]["source_sha256"])
        self.assertNotEqual(requests[0]["diagram_id"], requests[-1]["diagram_id"])

    def test_layout_profiles_receive_the_same_bodies_and_facts_in_order(self):
        result = plan(names=["flowchart-layout-default", "flowchart-layout-dagre-classic"])
        default, dagre = [s["requests"] for s in result["scenarios"]]
        self.assertEqual(
            [r["fixture"] for r in default],
            ["flowchart_nested_clusters", "flowchart_long_edge_labels"] * 2,
        )
        for plain, configured in zip(default, dagre, strict=True):
            config, _, diagram = configured["source"].partition("\n---\n")
            self.assertIn("  layout: dagre", config.splitlines())
            self.assertEqual(diagram, plain["source"])
            self.assertEqual(configured["expected_labels"], plain["expected_labels"])
            self.assertNotEqual(configured["source_sha256"], plain["source_sha256"])

    def test_configured_label_facts_are_visible_in_the_upstream_svgs(self):
        files = bench.load(bench.HERE / "workloads.json")["fixture_files"]
        requests = plan(names=["configured-labels"])["scenarios"][0]["requests"]
        self.assertEqual(
            [r["fixture"] for r in requests],
            [
                "flowchart-markdown-svg",
                "flowchart-cjk-approval",
                "flowchart-theme-variables",
                "sequence-dark-font",
                "flowchart-markdown-svg",
            ],
        )
        for request in requests[:-1]:
            with self.subTest(fixture=request["fixture"]):
                source = Path(files[request["fixture"]])
                directory = bench.ROOT / "fixtures/upstream-svgs" / source.parent.name
                svg = (directory / source.name).with_suffix(".svg")
                self.assertEqual(
                    validate_svg(svg.read_bytes(), request["expected_labels"]), {"status": "ok"}
                )

    def test_zed_examples_use_the_original_issue_sources(self):
        case = plan(names=["zed-issue-examples"])["scenarios"][0]
        self.assertEqual(case["origin"], "fixtures/zed_issues")
        self.assertEqual(
            [r["fixture"].split("_")[1] for r in case["requests"]],
            ["51142", "56767", "57323", "57363"],
        )
        for request in case["requests"]:
            source = bench.ROOT / case["origin"] / (request["fixture"] + ".mmd")
            self.assertEqual(request["source"], source.read_text(encoding="utf-8"))

    def test_topology_and_sequence_controls_keep_their_input_dimensions(self):
        result = plan("full", ["fan-in-500", "unique-message-labels", "repeated-message-labels"])
        topo, unique, repeated = result["scenarios"]
        self.assertEqual(topo["requests"][0]["source"], result["topologies"][0]["source"])
        a, b = (r["requests"][0]["source"] for r in (unique, repeated))
        self.assertEqual((a.count("A->>B:"), len(a)), (100, len(b)))
        self.assertNotEqual(a, b)
        actors = "<text>A</text><text>B</text>"
        missing = validate_svg(f"<svg>{actors}</svg>", repeated["requests"][0]["expected_labels"])
        self.assertEqual(missing["missing_labels"], ["event-000"])
        only_first = f"<svg>{actors}<text>event-000</text></svg>"
        missing = validate_svg(only_first, unique["requests"][0]["expected_labels"])
        self.assertEqual(missing["status"], "missing-labels")
        self.assertEqual(missing["missing_labels"], ["event-050", "event-099"])

    def test_compare_refuses_incompatible_inputs_and_withholds_changed_output(self):
        base = {
            "schema_version": 1,
            "surface": "rust",
            "options": {"metric": "memory"},
            "plan_sha256": "x",
            "host": ["test"],
            "runner_sha256": "a",
            "harness_sha256": "same measurement code",
            "rows": {"mixed": {"outputs": ["x"], "completed": 1, "samples": [{"allocated_bytes": 20}]}},
        }
        head = copy.deepcopy(base)
        head["rows"]["mixed"]["samples"] = [{"allocated_bytes": 30}]
        self.assertEqual(
            bench.compare(base, head)["rows"]["mixed"]["metrics"]["allocated_bytes"]["change_percent"],
            50,
        )
        head["rows"]["mixed"]["outputs"] = ["different"]
        self.assertEqual(bench.compare(base, head)["rows"]["mixed"], {"status": "output-changed"})
        head["harness_sha256"] = "new measurement code"
        self.assertEqual(bench.compare(base, head)["classification"], "harness-changed")
        head["plan_sha256"] = "changed"
        with self.assertRaisesRegex(ValueError, "plan_sha256"):
            bench.compare(base, head)


if __name__ == "__main__":
    unittest.main()
