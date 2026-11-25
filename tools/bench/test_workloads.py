import argparse
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from merman_bench import workloads as bench
from merman_bench.svg import EXPECTED_OUTCOMES, validate_rejection, validate_svg


def plan(suite="quick", names=(), documents=()):
    return bench.materialize(
        argparse.Namespace(suite=suite, scenario=list(names), document=documents, pele_fixtures=None)
    )


class PublicWorkloadsTest(unittest.TestCase):
    def test_stage_timings_reconcile_with_the_batch_boundary(self):
        stages = {phase: {"elapsed_ns": 0, "calls": 0} for phase in bench.STAGE_PHASES}
        stages.update(layout_solve={"elapsed_ns": 11, "calls": 1}, end_to_end={"elapsed_ns": 30, "calls": 1})
        sample = {"stages": stages, "elapsed_ns": 30, "unattributed_ns": 19}
        bench.validate_stage_timing(sample)
        with self.assertRaisesRegex(ValueError, "do not match end-to-end time"):
            bench.validate_stage_timing({**sample, "unattributed_ns": 18})
        report = {"surface": "rust", "options": {"metric": "stages"}}
        with self.assertRaisesRegex(ValueError, "stage attribution reports do not support compare"):
            bench.compare(report, report)

    def test_stage_layouts_measure_fixed_families_and_reject_selectable_fallbacks(self):
        request = {"diagram_id": "stage-test", "source_sha256": "a" * 64}
        svg = '<svg id="stage-test"/>'
        cases = [
            ("sequence", "native", "native", [], "family-fixed", None),
            ("flowchart", "shoal", None, ["layout-shoal"], "selectable", None),
            ("flowchart", "dagre", None, ["layout-dagre"], "selectable", "layout-backend-mismatch"),
        ]
        for family, backend, fixed, capabilities, applicability, status in cases:
            with self.subTest(family=family, backend=backend):
                response = {"outputs": [svg], "layout_evidence": [dict(
                    request, family=family, backend=backend, fixed_backend=fixed,
                    required_capabilities=capabilities,
                )]}
                signatures = [{}]
                evidence = bench.check_layout_evidence(
                    response, {"requests": [request], "operation": "svg-data"}, signatures,
                    "shoal", inventory=False, stages=True,
                )
                result = bench.summarize_stage_layout(evidence, "shoal")
                self.assertEqual(result["applicability"], applicability)
                self.assertEqual(result["cross_backend_eligible"], backend == "shoal")
                self.assertEqual(signatures[0].get("status"), status)

    def test_source_reuse_controls_hold_size_topology_or_labels_as_declared(self):
        names = ["four-node-repeat", "four-node-relabel", "four-node-edges"]
        cases = [case["requests"] for case in plan(names=names)["scenarios"]]
        self.assertEqual([len(case) for case in cases], [8, 8, 8])
        self.assertEqual([len({r["source_sha256"] for r in case}) for case in cases], [1, 8, 8])
        self.assertEqual(len({case[0]["source"] for case in cases}), 1)
        lengths = {len(r["source"].encode()) for case in cases for r in case}
        self.assertEqual(len(lengths), 1)
        for case in cases:
            for request in case:
                self.assertEqual(request["diagram_id"], "four-node-preview")
                self.assertEqual(request["source"].count(" --> "), 3)
                self.assertEqual(list(request["drawing_facts"]["label_widths"].values()), request["expected_labels"])
        for i, request in enumerate(cases[1]):
            self.assertEqual(request["source"].replace(str(i), "0"), cases[0][i]["source"])
        for request in cases[2]:
            self.assertEqual(request["source"].splitlines()[:5], cases[0][0]["source"].splitlines()[:5])

    def test_full_appends_to_the_reviewed_quick_and_legacy_scenarios(self):
        quick_plan = plan()
        quick = quick_plan["scenarios"]
        self.assertEqual([s["id"] for s in quick_plan["stage_scenarios"]], ["fan-in-500"])
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
        recipes = bench.load(bench.HERE / "workloads.json")["scenarios"]
        self.assertEqual([s["id"] for s in full[:len(recipes)]], [r["id"] for r in recipes])
        families = plan("families")["scenarios"]
        self.assertEqual(full[len(recipes):len(recipes) + len(families)], families)
        self.assertEqual([s["id"] for s in full[len(recipes) + len(families):]],
                         ["scc-25", "scc-75", "scc-100", "scc-dag-25", "scc-dag-75", "scc-dag-100",
                          "scc-75-declarations-2", "scc-dag-75-declarations-2", "scc-100-edges-reverse", "scc-dag-100-edges-reverse"])
        unchecked = [
            r["diagram_id"] for s in full for r in s["requests"] if "expected_labels" not in r
        ]
        self.assertEqual(unchecked, [])

    def test_families_freeze_one_source_per_playground_baseline_in_catalog_order(self):
        examples = bench.load(bench.FAMILY_MANIFEST)["examples"]
        rows = sorted(
            (r for r in examples if r["evidence"]["role"] == "family-baseline"), key=lambda r: r["order"]
        )
        result = plan("families")
        self.assertEqual(result["topologies"], [])
        self.assertEqual({s["family"] for s in result["scenarios"]}, {r["diagramType"] for r in examples})
        for case, row in zip(result["scenarios"], rows, strict=True):
            with self.subTest(example=row["id"]):
                self.assertEqual(case["id"], f"family-{row['diagramType']}")
                self.assertEqual(case["family"], row["diagramType"])
                self.assertEqual(case["order"], row["order"])
                self.assertEqual(case["origin"], row["fixture"])
                (request,) = case["requests"]
                source = (bench.ROOT / row["fixture"]).read_text(encoding="utf-8")
                self.assertEqual(request["fixture"], row["id"])
                self.assertEqual(request["diagram_id"], f"wb-{case['id']}-1")
                self.assertEqual(request["source"], source)
                self.assertEqual(request["source_sha256"], bench.digest(source.encode()))

    def test_malformed_label_alternatives_identify_the_request(self):
        for facts in ([[]], [["x", " "]], [["x", 1]], [[["x"]]]):
            with self.subTest(facts=facts), self.assertRaisesRegex(ValueError, "review expected labels for doc/x"):
                bench.scenario("doc", "local", [("x", "info")], {"x": facts})

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

    def test_editor_preview_reuses_the_output_id_through_a_rejected_revision(self):
        fixed, rotating = plan(names=["editor-fixed-id", "editor-rotating-id"])["scenarios"]
        requests = fixed["requests"]
        self.assertEqual([r["diagram_id"] for r in requests], ["preview-01"] * 3)
        self.assertEqual(len({r["diagram_id"] for r in rotating["requests"]}), 3)
        self.assertEqual([r["expected_outcome"] for r in requests], ["draw", "reject", "draw"])
        for a, b in zip(requests, rotating["requests"], strict=True):
            self.assertEqual({**a, "diagram_id": b["diagram_id"]}, b)

    def test_external_drawings_follow_only_the_declaring_document_and_filename(self):
        files = ["block-4", "block-1", "flowchart_tiny", "block-1"]
        source = (
            "flowchart LR\n  subgraph First\n    X_in\n  end\n  subgraph Second\n    Y_out\n  end\n"
            "  X_in first@--> Y_out\n  X_in reviewed@--> Y_out\n"
        )
        tiny = bench.ROOT / "crates/merman/benches/fixtures/flowchart_tiny.mmd"
        sources = {"block-1": source, "block-4": source, "flowchart_tiny": tiny.read_text(encoding="utf-8")}
        facts = {
            "arrows": [{"edge": "reviewed", "from": "X_in", "to": "Y_out"}],
            "groups": {"First": ["X_in"], "Second": ["Y_out"]},
        }
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            for name, text in sources.items():
                (directory / name).write_bytes(text.replace("\n", "\r\n").encode())
            plain = directory / "plain.json"
            annotated = directory / "annotated.json"
            doc = {"id": "plain", "origin": "local/guide", "files": files}
            bench.write(plain, doc)
            bench.write(annotated, {**doc, "id": "annotated", "drawing_facts": {"block-1": facts}})
            names = ["annotated", "merman-issue-guide", "plain"]
            cases = plan(names=names, documents=[plain, annotated])["scenarios"]

        self.assertEqual([case["id"] for case in cases], names)
        annotated_case, guide, plain_case = cases
        self.assertEqual(guide, plan(names=["merman-issue-guide"])["scenarios"][0])
        for case, reviewed in [(annotated_case, [(2, facts), (4, facts)]), (plain_case, [])]:
            with self.subTest(document=case["id"]):
                requests = case["requests"]
                self.assertEqual([r["fixture"] for r in requests], files)
                self.assertEqual([r["source"] for r in requests], [sources[f] for f in files])
                self.assertEqual(
                    [r["source_sha256"] for r in requests], [bench.digest(sources[f].encode()) for f in files]
                )
                self.assertEqual(
                    [r["diagram_id"] for r in requests], [f"wb-{case['id']}-{i}" for i in range(1, 5)]
                )
                actual = [(i, r["drawing_facts"]) for i, r in enumerate(requests, 1) if "drawing_facts" in r]
                self.assertEqual(actual, reviewed)

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

    def test_cyclic_pairs_reuse_the_exact_work_input_across_public_surfaces(self):
        full = plan("full")
        topologies = {t["id"]: t for t in full["topologies"]}
        self.assertEqual(len(topologies), 32)
        names = ["scc-100", "scc-dag-100"]
        for case in plan(names=names)["scenarios"]:
            (request,) = case["requests"]
            self.assertEqual(request["source_sha256"], topologies[case["id"]]["source_sha256"])
            self.assertEqual(request["expected_labels"], [f"C{i}" for i in range(100)])
        self.assertSetEqual({t["id"] for t in plan()["topologies"] if t["id"].startswith("scc")},
                            {"scc-25", "scc-dag-25"})

    def test_imported_outcomes_are_explicit_and_bind_to_the_original_occurrences(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "manifest.json"
            for name in ("regular", "unread", "settings", "broken"):
                (path.parent / name).write_text("info", encoding="utf-8")
            doc = dict(id="custom", origin="local", files=["regular", "unread", "settings", "broken", "unread"],
                       expected_labels={"regular": ["hello"]}, expected_outcomes={"settings": "allow-empty", "broken": "reject"})
            bench.write(path, doc)
            requests = bench.document_scenario(path)["requests"]
            self.assertEqual([r["expected_outcome"] for r in requests], ["draw", "unreviewed", "allow-empty", "reject", "unreviewed"])
            for mapping, reason in [({"absent": "draw"}, "refer to files"), ({"unread": "pass"}, "invalid expected_outcome"),
                                    ({"regular": "reject"}, "conflict")]:
                bench.write(path, {**doc, "expected_outcomes": mapping})
                with self.assertRaisesRegex(ValueError, reason):
                    bench.document_scenario(path)

    def test_compare_refuses_incompatible_inputs_and_withholds_changed_output(self):
        base = {
            "schema_version": 1,
            "surface": "rust",
            "options": {"metric": "memory"},
            "plan_sha256": "x",
            "host": ["test"],
            "runner_sha256": "a",
            "harness_sha256": "same measurement code",
            "rows": {"mixed": {"outputs": ["x"], "completed": 1, "expected_outcomes": ["draw"], "samples": [{"allocated_bytes": 20}]}},
        }
        head = copy.deepcopy(base)
        head["rows"]["mixed"]["samples"] = [{"allocated_bytes": 30}]
        row = bench.compare(base, head)["rows"]["mixed"]
        self.assertEqual((row["status"], row["measured_output_check"]), ("matching-verification-outputs", "size-only"))
        legacy = copy.deepcopy(base)
        del legacy["rows"]["mixed"]["expected_outcomes"]
        self.assertEqual(bench.compare(legacy, legacy)["rows"]["mixed"], {"status": "source-outcomes-unverified"})
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


class RustSvgVerificationTests(unittest.TestCase):
    def test_wrapped_text_uses_the_shared_checker_outside_the_native_observation(self):
        svg = '<svg><text><tspan x="0" y="0">Banking</tspan><tspan x="0" y="15">System</tspan></text></svg>'
        request = dict(diagram_id="c4", expected_labels=[["Banking System", "Banking system"]])
        identities, shared = bench.rust_signatures([svg], [request], "svg-data")
        self.assertTrue(shared)
        self.assertEqual(identities, [dict(sha256=bench.digest(svg.encode()), bytes=len(svg), observed_size=len(svg))])
        changed, _ = bench.rust_signatures([svg.replace("System", "service")], [request], "svg-data")
        self.assertEqual(changed[0]["status"], "missing-labels")
        self.assertNotEqual(changed[0]["sha256"], identities[0]["sha256"])
        with self.assertRaisesRegex(ValueError, "alternative label spellings"):
            bench.rust_signatures(identities, [request], "svg-data")
        self.assertEqual(bench.rust_signatures(identities, [dict(request, expected_labels=["Banking System"])], "svg-data"),
                         (identities, False))

    def test_changed_shared_svg_validator_withholds_the_rust_delta(self):
        base = dict(schema_version=1, surface="rust", plan_sha256="plan", options={}, host=[], rows={},
                    harness_sha256="one executable", svg_validator_sha256="first reader")
        self.assertEqual(bench.compare(base, dict(base, svg_validator_sha256="second reader"))["classification"], "harness-changed")

    def test_exact_mode_checks_every_measured_value_outside_the_native_clock(self):
        small = "<svg><text>A</text></svg>"
        changed = small.replace(">A<", ">B<")
        def run(actual, exact, require_arrow=False):
            with tempfile.TemporaryDirectory() as tmp:
                directory = Path(tmp)
                runner = directory / "runner"
                runner.write_bytes(b"stub")
                selected = {"id": "case", "requests": [{"fixture": "tiny", "source": "info", "source_sha256": bench.digest(b"info"), "diagram_id": "one"}]}
                if require_arrow:
                    selected["requests"][0]["drawing_facts"] = {"arrows": [{"edge": "L_A_B_0", "from": "A", "to": "B"}]}
                frozen = directory / "plan.json"
                bench.write(frozen, {"schema_version": 1, "scenarios": [selected]})
                def observe(_runner, job):
                    if job.get("verify_only"):
                        values = {"outputs": [small]}
                    elif job.get("return_measured_svg"):
                        values = {"outputs": [actual], "metrics": {"elapsed_ns": 100}}
                    else:
                        values = {"sizes": [len(actual)], "metrics": {"elapsed_ns": 100}}
                    return {**values, "harness_sha256": "fixed"}
                args = argparse.Namespace(plan=frozen, metric="clock", operation="svg-data", scenario=[], site_layout=None,
                                          site_css_bytes=0, samples=2, warmups=0, runner=runner, revision="frozen",
                                          verify_measured_svg=exact, check_drawings=False, chrome=None)
                with patch.object(bench, "child", side_effect=observe):
                    return bench.run_rust(args)
        rejected = run(changed, True)["rows"]["case"]
        self.assertEqual(rejected["status"], "measurement-mismatch")
        self.assertEqual(rejected["samples"], [])
        self.assertIn("measured SVG differs", rejected["error"])
        before, after = run(small, False), run(changed, False)
        self.assertEqual(bench.compare(before, after)["rows"]["case"]["measured_output_check"], "size-only")
        same = run(small, True)
        self.assertEqual(bench.compare(same, same)["rows"]["case"]["status"], "same-output")
        missing_arrow = run(small, True, require_arrow=True)
        row = bench.compare(missing_arrow, missing_arrow)["rows"]["case"]
        self.assertEqual(row["status"], "drawing-not-verified")
        self.assertNotIn("metrics", row)
        wrong = copy.deepcopy(same)
        wrong["rows"]["case"]["measured_svg_sha256"].pop()
        with self.assertRaisesRegex(ValueError, "missing exact measured SVG evidence"):
            bench.compare(same, wrong)
        wrong = copy.deepcopy(same)
        wrong["options"].update(operation="layout-json", metric="memory")
        with self.assertRaisesRegex(ValueError, "measurement boundary"):
            bench.compare(wrong, wrong)


class RustLayoutTests(unittest.TestCase):


    def test_public_layout_inventory_checks_node_and_group_endpoints(self):
        # Identity fields from the public writer for the frozen small-svg request.
        graph = {"nodes": [{"id": "A", "is_cluster": False}, {"id": "B", "is_cluster": False}],
                 "edges": [{"id": "L_A_B_0", "from": "A", "to": "B"}], "clusters": []}
        document = {"schema_version": 1, "family_id": "flowchart", "diagram_type": "flowchart-v2",
                    "layout": {"FlowchartV2": graph}}
        self.assertEqual(bench.flowchart_layout_inventory(document), dict(nodes=["A", "B"], edges=[["L_A_B_0", "A", "B"]]))
        for overrides, error in (({"diagram_type": None, "diagramType": "flowchart-v2"}, "diagram_type"),
                                 ({"family_id": "swimlane"}, "family_id"),
                                 ({"layout": {"FlowchartV2": dict(graph, nodes=[*graph["nodes"], {"id": "G", "is_cluster": True}])}}, "cluster proxy"),
                                 ({"layout": {"FlowchartV2": dict(graph, nodes=graph["nodes"][:1])}}, "endpoint")):
            with self.subTest(overrides=overrides), self.assertRaisesRegex(ValueError, error):
                bench.flowchart_layout_inventory({**document, **overrides})
        graph["clusters"] = [{"id": "G"}]
        graph["edges"] = [{"id": "L_A_G_0", "from": "A", "to": "G"}]
        expected = dict(nodes=["A", "B"], groups=["G"], edges=[["L_A_G_0", "A", "G"]])
        self.assertEqual(bench.flowchart_layout_inventory(document), expected)
        graph["nodes"].append({"id": "G", "is_cluster": True})
        self.assertEqual(bench.flowchart_layout_inventory(document), expected)

    def test_shoal_needs_per_occurrence_evidence_even_with_legacy_svg_identities(self):
        inputs = [dict(diagram_id="preview", source_sha256=bench.digest(text)) for text in (b"first", b"second")]
        job = dict(operation="svg-data", requests=inputs)
        outputs = ['<svg id="preview"><text>A</text></svg>'] * 2
        proof = [dict(**r, family="flowchart", required_capabilities=["layout-shoal"]) for r in inputs]

        def check(outputs, receipts):
            signatures, _ = bench.rust_signatures(outputs, inputs, "svg-data")
            bench.check_layout_evidence(dict(outputs=outputs, layout_evidence=receipts), job, signatures, "shoal", inventory=False)
            return signatures

        old, _ = bench.rust_signatures(outputs, inputs, "svg-data")
        for unsupported in (None, []):
            with self.assertRaisesRegex(ValueError, "per-request layout evidence"):
                check(old, unsupported)
        with self.assertRaisesRegex(ValueError, "SVG bytes"):
            check(old, proof)
        with self.assertRaisesRegex(ValueError, "misidentified"):
            check(outputs, proof[::-1])
        with self.assertRaisesRegex(ValueError, "frozen ID"):
            check([outputs[0], outputs[1].replace("preview", "different")], proof)
        for capabilities, actual in (([], "dagre"), (["layout-elk"], "elk")):
            receipts = [proof[0], dict(proof[1], required_capabilities=capabilities)]
            first, pinned = check(outputs, receipts)
            self.assertNotIn("status", first)
            self.assertEqual(pinned["status"], "layout-backend-mismatch")
            self.assertEqual(receipts[1]["backend"], actual)
        receipts = [proof[0], dict(proof[1], family="sequence", required_capabilities=[])]
        self.assertEqual(check(outputs, receipts)[1]["status"], "layout-backend-mismatch")
        self.assertIsNone(receipts[1]["backend"])
        for family in ("agentflow", "architecture", "block", "c4", "class", "state", "er", "requirement", "swimlane", "usecase"):
            with self.subTest(family=family):
                receipts = [proof[0], dict(proof[1], family=family)]
                self.assertNotIn("status", check(outputs, receipts)[1])
                self.assertEqual(receipts[1]["backend"], "shoal")

    def test_graph_families_verify_layout_json_without_flowchart_inventories(self):
        request = dict(diagram_id="preview", source_sha256=bench.digest(b"source"))
        job = dict(operation="layout-json", requests=[request])

        def verify(family, capabilities, requested, *, inventory=False, output_family=None):
            output = dict(bytes=20, observed_size=20, sha256="a" * 64,
                          layout_json=dict(schema_version=1, family_id=output_family or family))
            receipt = dict(**request, family=family, required_capabilities=capabilities)
            response = dict(outputs=[output], layout_evidence=[receipt])
            signatures, _ = bench.rust_signatures(response["outputs"], [request], "layout-json")
            bench.check_layout_evidence(response, job, signatures, requested, inventory=inventory)
            return receipt, signatures[0]

        for family in ("class", "state", "er", "requirement", "usecase"):
            with self.subTest(family=family):
                receipt, signature = verify(family, ["layout-elk"], "elk")
                self.assertEqual(receipt["backend"], "elk")
                self.assertNotIn("status", signature)

                receipt, signature = verify(family, ["layout-shoal"], "shoal", inventory=True)
                self.assertEqual(receipt["backend"], "shoal")
                self.assertNotIn("inventory", receipt)
                self.assertEqual(signature["status"], "unsupported-inventory")

                receipt, signature = verify(family, [], "shoal")
                expected = None if family == "er" else "dagre"
                self.assertEqual(receipt["backend"], expected)
                self.assertEqual(signature["status"], "layout-backend-mismatch")

        receipt, signature = verify("architecture", ["layout-cytoscape"], "shoal")
        self.assertEqual(receipt["backend"], "cytoscape")
        self.assertEqual(signature["status"], "layout-backend-mismatch")

        receipt, signature = verify("er", [], "dagre")
        self.assertIsNone(receipt["backend"])
        self.assertEqual(signature["status"], "layout-backend-mismatch")
        self.assertIn("Swimlane", signature["message"])
        with self.assertRaisesRegex(ValueError, "layout JSON family"):
            verify("class", ["layout-shoal"], "shoal", output_family="flowchart")

    def test_one_binary_compares_frozen_layout_and_svg_occurrences(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            frozen = path / "plan.json"
            source = "flowchart LR\nsubgraph G[Group]\nA[Start]\nend\nA shared@-->|yes| B[End]\nA shared@-->|yes| B\n"
            case = bench.scenario("parallel", "test", [("two", source)], {"two": ["Start", "End", "yes"]}, diagram_id="preview")
            bench.write(frozen, dict(schema_version=1, scenarios=[case], topologies=[]))
            request, = case["requests"]
            binary = path / "clock"
            binary.write_bytes(b"one Shoal-capable executable")
            out = path / "comparison.json"
            mutation = None
            profile = None
            runner_confirms_profile = True

            def child(runner, job):
                self.assertEqual(runner.resolve(), binary.resolve())
                self.assertEqual(job["requests"], [request])
                self.assertEqual(job.get("resource_profile"), profile)
                backend = job["site"]["layout"]
                different = mutation if backend == "shoal" else None
                x = bench.LAYOUTS.index(backend) + 1
                nodes = [dict(id=key, x=x, is_cluster=False) for key in "AB"]
                if backend != "shoal":
                    nodes.append(dict(id="G", x=x, is_cluster=True))
                edge = {"id": "shared", "from": "A", "to": "B"}
                edges = [edge, dict(edge, to="A") if different == "endpoint" else edge]
                geometry = dict(schema_version=1, family_id="flowchart", diagram_type="flowchart-v2",
                                layout={"FlowchartV2": dict(nodes=nodes, edges=edges, clusters=[{"id": "G"}])})
                label = "no" if different == "label" else "yes"
                title = "Renamed" if different == "group-title" else "Group"
                svg = ('<svg id="preview"><g data-et="node" data-id="A"><text>Start</text></g>'
                       '<g data-et="node" data-id="B"><text>End</text></g>'
                       + (f'<g data-et="cluster" data-id="G"><g class="cluster-label"><text>{title}</text></g></g>'
                          if different != "group" else '')
                       + ''.join(f'<path data-et="edge" id="edge-{i}" data-id="shared" d="M0,0 L{x},0"/>'
                                 for i in range(1 if different == "occurrence" else 2))
                       + f'<g data-id="shared" class="label"><text>{label}</text></g>'
                       + '<g data-id="shared" class="label"><text>yes</text></g></svg>')
                data = svg if job["operation"] == "svg" else json.dumps(geometry)
                size = len(data.encode())
                if job.get("verify_only"):
                    self.assertTrue(job["verify_layout"])
                    caps = [] if backend == "dagre" or different == "pinned" else [f"layout-{backend}"]
                    proof = dict(family="flowchart", required_capabilities=caps, diagram_id="preview", source_sha256=request["source_sha256"])
                    result = dict(outputs=[svg if job["operation"] == "svg" else dict(
                        bytes=size, observed_size=size, sha256=bench.digest(data.encode()), layout_json=geometry)], layout_evidence=[proof])
                else:
                    self.assertNotIn("verify_layout", job)
                    self.assertNotEqual(different, "pinned", "a mismatched backend must never be timed")
                    result = dict(metrics={"elapsed_ns": 10 * x}, sizes=[size])
                    if job.get("return_measured_svg"):
                        result["outputs"] = [svg.replace(f"L{x},0", f"L{x},1") if different == "unstable" else svg]
                if profile and runner_confirms_profile:
                    result["resource_profile"] = profile
                return dict(**result, harness_sha256="one-harness")

            with patch.object(bench, "child", side_effect=child), patch.object(bench, "build_runner", return_value=binary) as build:
                for mutation, status in ((None, "matching-inventories"), ("endpoint", "inventory-mismatch"),
                                         ("label", "inventory-mismatch"), ("occurrence", "inventory-mismatch"),
                                         ("group", "inventory-mismatch"), ("group-title", "inventory-mismatch"),
                                         ("pinned", "incomplete"), ("unstable", "incomplete")):
                    with self.subTest(mutation=mutation):
                        code = bench.main(["rust-layouts", "--plan", str(frozen), "--out", str(out), "--samples", "2", "--revision", "test"])
                        build.assert_called_once_with("clock", shoal=True)
                        build.reset_mock()
                        report = bench.load(out)
                        self.assertEqual(report["plan_sha256"], bench.digest(frozen.read_bytes()))
                        row = report["rows"]["parallel"]
                        self.assertEqual((code, row["status"], row["planned"]), (int(bool(mutation)), status, 6))
                        self.assertEqual("median_ns" in row, mutation is None)
                        if mutation is None:
                            self.assertEqual(row["completed"], 6)
                            svgs = [r["svg"] for r in report["runs"].values()]
                            self.assertEqual(len({r["rows"]["parallel"]["outputs"][0]["sha256"] for r in svgs}), 3)
                            with self.assertRaisesRegex(ValueError, "options"):
                                bench.compare(svgs[0], svgs[1])

                mutation = None
                profile = "constrained"
                args = ["rust-layouts", "--plan", str(frozen), "--runner", str(binary), "--out", str(out),
                        "--resource-profile", profile]
                self.assertEqual(bench.main(args), 0)
                for run in bench.load(out)["runs"].values():
                    self.assertEqual([r["options"]["resource_profile"] for r in run.values()], [profile, profile])
                runner_confirms_profile = False
                with self.assertRaises(SystemExit):
                    bench.main(args)


class WorkloadSvgTests(unittest.TestCase):


    def test_viewport_applies_to_the_root_even_if_expected_text_survives(self):
        for attributes in (
            'width="0" height="10"', 'height=" -1px "', 'width="-0e2%"',
            'viewBox="0 0 0 0"', 'viewBox="-3,0,2,-4"', 'viewBox="0 0 nan 9"',
            'viewBox="0 0 1e999 9"', 'viewBox="0,,0,20,8"',
            'viewBox="0 0 30 10" style="width:0px !important;width:100%"',
            'width="100%" style="max-height:0px"',
        ):
            svg = f'<svg {attributes}><text>surviving label</text></svg>'
            for outcome in EXPECTED_OUTCOMES:
                with self.subTest(attributes=attributes, outcome=outcome):
                    self.assertEqual(validate_svg(svg, ["surviving label"], outcome)["status"], "invalid-svg")
        for attributes in ('', 'width="100%" style="max-width: 477.859px;"',
                           'viewBox="-20, -5 .1e3 4"', 'width="0" style="width:20px"'):
            svg = f'<svg {attributes}><g><rect width="0"/><text>label</text></g></svg>'
            with self.subTest(attributes=attributes):
                self.assertEqual(validate_svg(svg, ["label"]), {"status": "ok"})

    def test_allow_empty_needs_an_actual_sized_document_and_honors_label_facts(self):
        for svg in ('<svg viewBox="0 0 17 17"/>', '<svg width="17px" height="17"/>',
                    '<svg style="width:20px;height:2em"><defs><rect/></defs></svg>'):
            with self.subTest(svg=svg):
                self.assertEqual(validate_svg(svg)["status"], "empty-svg")
                self.assertEqual(validate_svg(svg, [], "allow-empty"), {"status": "ok", "output_class": "empty"})
                self.assertEqual(validate_svg(svg, ["missing"], "allow-empty")["status"], "missing-labels")
                self.assertEqual(validate_svg(svg, [], "reject")["status"], "unexpected-success")
                self.assertEqual(validate_svg(svg, [], "unreviewed"), {"status": "ok", "output_class": "empty"})
        for svg in (None, "", "  ", "<svg", "<html/>", "<svg/>", '<svg width="100%"/>'):
            for outcome in EXPECTED_OUTCOMES:
                with self.subTest(svg=svg, outcome=outcome):
                    self.assertIn(validate_svg(svg, [], outcome)["status"], ("empty-svg", "invalid-svg"))

    def test_error_diagram_does_not_establish_a_parse_rejection(self):
        for svg in ('<svg aria-roledescription="error"><rect/></svg>',
                    '<svg><text class="error-text">Syntax error in text</text></svg>'):
            self.assertEqual(validate_svg(svg, [], "reject")["status"], "error-svg")
        authored = '<svg><text>Syntax error in text</text></svg>'
        self.assertEqual(validate_svg(authored), {"status": "ok"})
        for outcome in ("allow-empty", "unreviewed"):
            self.assertEqual(validate_svg(authored, [], outcome), {"status": "ok", "output_class": "draw"})
        self.assertEqual(validate_svg(authored, [], "reject")["status"], "unexpected-success")

    def test_only_positive_product_parser_diagnostics_establish_rejection(self):
        merman = ("Diagram parse error (flowchart-v2): unexpected end of input; "
                  "expected node identifier, edge label")
        pele = "Parse error on line 9:\n  g-->\n------^\nExpecting 'NODE_STRING', got 'EOF'"
        mermaid = "Error at line 10, column 1: Expected a node id, got the end of the text."
        mmdc = ("Generating single mermaid chart\r\n\r\nError: Parse error on line 10:\n"
                "...->g g-->\n------^\nExpecting 'NODE_STRING', got 'EOF'")
        for code, message in (("MERMAN_PARSE_ERROR", merman), ("syntax", pele), ("SYNTAX", mermaid),
                              (1, merman), (1, mmdc), (None, merman), (None, pele), (None, mermaid)):
            with self.subTest(code=code, message=message):
                self.assertEqual(validate_rejection(code, message),
                                 {"status": "expected-rejection", "code": code, "message": message})
        max_edges = ("Diagram parse error (flowchart-v2): Flowchart edge limit exceeded: this edge group "
                     "would create 1 edges, but only 0 of the maxEdges budget remain")
        for code, message in (
            (None, max_edges), ("MERMAN_PARSE_ERROR", max_edges), (1, max_edges),
            ("MERMAN_PARSE_ERROR", "Diagram parse error (flowchart-v2): unexpected host error; expected a worker"),
            ("MERMAN_PARSE_ERROR", "parse failure"), (None, "Diagram parse error (flowchart-v2): unrelated failure"),
            ("MERMAN_RESOURCE_LIMIT_EXCEEDED", merman), ("limit", pele), ("LIMIT", mermaid),
            ("ENOENT", merman), ("INTERNAL", mermaid), ("semantic", pele), (-9, merman),
            (None, "Resource limit exceeded"), (1, "Parser.parseError (/path/chunk.js:1:20)"),
            (None, "Error: Failed to launch the browser\n" + pele), (None, "SyntaxError: JSON.parse failed"),
            (1, "Generating single mermaid chart\nError: Cannot find module 'parser'"), (1, ""),
        ):
            with self.subTest(code=code, message=message):
                self.assertEqual(validate_rejection(code, message)["status"], "render-error")
