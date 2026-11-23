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

    def test_family_labels_match_reference_svgs_and_zenuml_layout(self):
        for case in plan("families")["scenarios"]:
            with self.subTest(family=case["family"]):
                (request,) = case["requests"]
                labels = request["expected_labels"]
                if not labels:
                    self.assertTrue(request["empty_label_reason"].strip())
                    continue
                fixture = Path(case["origin"]).relative_to("fixtures")
                svg = bench.ROOT / "fixtures/upstream-svgs" / fixture.with_suffix(".svg")
                if svg.is_file():
                    self.assertEqual(validate_svg(svg.read_bytes(), labels), {"status": "ok"})
                elif case["family"] == "zenuml":
                    golden = (bench.ROOT / case["origin"]).with_suffix(".layout.golden.json")
                    layout = bench.load(golden)["layout"]["ZenumlDiagram"]
                    visible = {
                        row["label"] for key in ("participants", "messages", "returns") for row in layout[key]
                    }
                    for fact in labels:
                        self.assertTrue(visible.intersection([fact] if isinstance(fact, str) else fact), fact)

    def test_requirement_and_sankey_accept_only_the_reviewed_complete_label_spellings(self):
        requirement, sankey = plan(names=["family-requirement", "family-sankey"])["scenarios"]

        def check(case, texts):
            svg = "<svg>" + "".join(f"<text>{text}</text>" for text in texts) + "</svg>"
            return validate_svg(svg, case["requests"][0]["expected_labels"])["status"]

        for label in ("A requirement", "Text: A requirement"):
            self.assertEqual(check(requirement, ["req1", label]), "ok")
        self.assertEqual(check(requirement, ["req1", "A requirement extended"]), "missing-labels")
        self.assertEqual(check(requirement, ["req10", "A requirement"]), "missing-labels")

        names = ["Electricity grid", "Over generation / exports", "H2 conversion"]
        totals = ["Electricity grid 245.32", "Over generation / exports 104.45", "H2 conversion 27.14"]
        for texts in (names, totals, [names[0], *totals[1:]]):
            self.assertEqual(check(sankey, texts), "ok")
        for i in range(len(totals)):
            wrong = totals.copy()
            wrong[i] = names[i] + " 0.00"
            self.assertEqual(check(sankey, wrong), "missing-labels")

    def test_explicit_family_selection_preserves_caller_order_across_suites(self):
        names = ["family-treeView", "small-svg", "family-er", "family-railroadPeg"]
        reference = {s["id"]: s for s in plan("full")["scenarios"]}
        for suite in ("quick", "full", "families"):
            with self.subTest(suite=suite):
                result = plan(suite, names)
                self.assertEqual(result["scenarios"], [reference[name] for name in names])
                self.assertEqual(result["topologies"], [])
        result = plan("families", ["family-er", "chain-50"])
        self.assertEqual([t["id"] for t in result["topologies"]], ["chain-50"])
        self.assertEqual([s["id"] for s in result["scenarios"]], ["family-er", "chain-50"])

    def test_family_projection_rejects_duplicate_and_missing_baselines(self):
        manifest = bench.load(bench.FAMILY_MANIFEST)
        rows = [r for r in manifest["examples"] if r["evidence"]["role"] == "family-baseline"]
        first, second = rows[:2]
        expected = plan("families")
        with tempfile.TemporaryDirectory() as tmp, patch.object(bench, "FAMILY_MANIFEST", Path(tmp) / "catalog.json"):
            bench.write(bench.FAMILY_MANIFEST, {**manifest, "examples": list(reversed(manifest["examples"]))})
            self.assertEqual(plan("families"), expected)
            for field in ("diagramType", "id", "order", "fixture"):
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, f"duplicate family-baseline {field}"):
                    bench.write(bench.FAMILY_MANIFEST, {**manifest, "examples": [first, {**second, field: first[field]}]})
                    plan("families")
            variant = {**second, "evidence": {"role": "variant"}}
            bench.write(bench.FAMILY_MANIFEST, {**manifest, "examples": [first, variant]})
            with self.assertRaisesRegex(ValueError, f"missing family-baseline.*{second['diagramType']}"):
                plan("families")

    def test_family_label_catalog_rejects_stale_missing_and_unreviewed_facts(self):
        reviews = bench.load(bench.FAMILY_LABELS)
        labels = reviews["expected_labels"]
        er = "entity-relationships"
        stale = labels.copy()
        stale["retired-example"] = stale.pop(er)
        cases = [
            ({"expected_labels": stale}, "missing.*entity-relationships.*stale.*retired-example"),
            ({"empty_label_reasons": {}}, "empty_label_reasons"),
            ({"expected_labels": {**labels, er: ["places", "places"]}}, "duplicate expected label"),
            ({"expected_labels": {**labels, er: [["A", "B"], ["B", "A"]]}}, "duplicate expected label"),
        ]
        with tempfile.TemporaryDirectory() as tmp, patch.object(bench, "FAMILY_LABELS", Path(tmp) / "labels.json"):
            for overrides, error in cases:
                with self.subTest(error=error), self.assertRaisesRegex(ValueError, error):
                    bench.write(bench.FAMILY_LABELS, {**reviews, **overrides})
                    plan("families")
            four = ["CUSTOMER", "LINE_ITEM", "places", "contains"]
            bench.write(bench.FAMILY_LABELS, {**reviews, "expected_labels": {**labels, er: four}})
            (case,) = plan(names=["family-er"])["scenarios"]
            self.assertEqual(case["requests"][0]["expected_labels"], four)

    def test_missing_builtin_label_facts_block_materialization(self):
        catalog = bench.load(bench.HERE / "workloads.json")
        empty = bench.scenario("shape", "local", [("shape", "pie\n\"\" : 1")], {"shape": []})
        self.assertEqual(empty["requests"][0]["expected_labels"], [])
        del catalog["expected_labels"]["er_medium"]
        with patch.object(bench, "parse_workload_json", return_value=catalog):
            with self.assertRaisesRegex(ValueError, "service-design-document/er_medium"):
                plan(names=["service-design-document"])

    def test_malformed_label_alternatives_identify_the_request(self):
        for facts in ([[]], [["x", " "]], [["x", 1]], [[["x"]]]):
            with self.subTest(facts=facts), self.assertRaisesRegex(ValueError, "review expected labels for doc/x"):
                bench.scenario("doc", "local", [("x", "info")], {"x": facts})

    def test_only_reviewed_builtin_and_corpus_occurrences_have_drawing_facts(self):
        tiny = {"arrows": [{"edge": "L_A_B_0", "from": "A", "to": "B"}]}
        expected = {
            ("small-svg", 1, "flowchart_tiny"): tiny,
            ("zed-issue-examples", 2, "zed_56767_flowchart_foreign_object_labels"): {
                "label_widths": {"C": "feature/example-branch"},
            },
            ("different-flowcharts", 1, "flowchart_tiny"): tiny,
            ("merman-issue-guide", 1, "block-1"): {"arrows": [{"edge": "L_B_A_0", "from": "B", "to": "A"}]},
            ("merman-issue-guide", 4, "block-4"): {
                "arrows": [{"edge": "L_T_S_0", "from": "T", "to": "S"}],
                "groups": {"CI": ["L", "T"], "Deploy": ["S", "P"]},
            },
        }
        full = plan("full")
        corpus = bench.load_corpus(bench.HERE / "corpus.json")
        medium = {"label_widths": {"A1": "Service 1"}}
        corpus_facts = {"flowchart_tiny": tiny, "flowchart_medium": medium}
        reviewed = {"flowchart_medium": medium, "flow-padding": {**tiny, "label_widths": {"A": "A", "B": "B"}},
                    "seq-message": {"sequence_messages": {"hi": 1}}}
        for case in full["scenarios"]:
            for index, request in enumerate(case["requests"], 1):
                if request["fixture"] in reviewed:
                    expected[case["id"], index, request["fixture"]] = reviewed[request["fixture"]]
        sources = {f.name: bench.ROOT / f.source for f in corpus.fixtures}
        full_by_id = {s["id"]: s for s in full["scenarios"]}
        for tier, count in (("low", 1), ("medium", 32), ("high", 256)):
            fixture = f"sequence_block_repeat_{tier}"
            name = f"sequence-loop-messages-{count:03}"
            (request,) = full_by_id[name]["requests"]
            self.assertEqual(request["source"], sources[fixture].read_text(encoding="utf-8"))
            self.assertEqual(request["source"].count("message-000000"), count)
            facts = {"sequence_messages": {"message-000000": count}}
            expected[name, 1, request["fixture"]] = facts
            corpus_facts[fixture] = facts
        for suite in corpus.suites:
            for i, fixture in enumerate(bench.select_corpus_fixtures(corpus, suite), 1):
                if fixture.name in corpus_facts:
                    expected[f"corpus-{suite}", i, fixture.name] = corpus_facts[fixture.name]
        names = [f"corpus-{suite}" for suite in corpus.suites] + [t["id"] for t in full["topologies"]]
        cases = full["scenarios"] + plan("full", names)["scenarios"]
        actual = {
            (case["id"], i, r["fixture"]): r["drawing_facts"]
            for case in cases for i, r in enumerate(case["requests"], 1) if "drawing_facts" in r
        }
        self.assertEqual(actual, expected)

    def test_single_request_drawing_facts_keep_optional_fields_and_literal_ids(self):
        arrows = [{"edge": "second", "from": "A", "to": "B"}]
        groups = {"CI": ["A"], "Deploy": ["B"]}
        messages = {"message-000000": 256, "Another message?": 2**53 - 1}
        for facts in (
            {"arrows": arrows}, {"groups": groups}, {"arrows": [], "groups": groups},
            {"arrows": arrows, "groups": {}},
            {"label_widths": {"A": "feature/example-branch"}},
            {"sequence_messages": messages},
            {"arrows": arrows, "groups": groups, "label_widths": {"A": "From"}, "sequence_messages": messages},
        ):
            with self.subTest(facts=facts):
                self.assertIs(bench.validate_drawing_facts(facts), facts)

    def test_invalid_drawing_contracts_are_rejected_with_request_context(self):
        arrow = {"edge": "back", "from": "B", "to": "A"}
        invalid = {
            "null": None,
            "empty": {},
            "no facts": {"arrows": [], "groups": {}},
            "invalid message mapping": {"sequence_messages": []},
            "empty message mapping alone": {"sequence_messages": {}},
            "blank message text": {"sequence_messages": {"  ": 1}},
            "non-string message text": {"sequence_messages": {42: 1}},
            "missing label": {"label_widths": {"A": ""}},
            "multiline source label": {"label_widths": {"A": "ab\ncd"}},
            "cluster as text node": {"groups": {"CI": ["A"]}, "label_widths": {"CI": "CI"}},
            "unknown alongside valid": {"arrows": [arrow], "group": {}},
            "malformed arrows": {"arrows": arrow},
            "implicit edge": {"arrows": [{"from": "B", "to": "A"}]},
            "unsupported arrow field": {"arrows": [{**arrow, "orientation": "left"}]},
            "blank endpoint": {"arrows": [{**arrow, "to": " "}]},
            "non-string ID": {"arrows": [{**arrow, "edge": 1}]},
            "self-loop": {"arrows": [{**arrow, "to": "B"}]},
            "repeated edge ID": {"arrows": [arrow, {**arrow, "to": "C"}]},
            "cluster endpoint": {"arrows": [arrow], "groups": {"A": ["X"]}},
            "malformed groups": {"groups": []},
            "empty group ID": {"groups": {"": ["L"]}},
            "no members": {"groups": {"CI": []}},
            "single string": {"groups": {"CI": "L"}},
            "malformed member": {"groups": {"CI": [["L"]]}},
            "repeated member": {"groups": {"CI": ["L", "L"]}},
            "two owners": {"groups": {"CI": ["L"], "Deploy": ["L"]}},
            "nested groups": {"groups": {"CI": ["Deploy"], "Deploy": ["S"]}},
        }
        for reason, facts in invalid.items():
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, r"guide/block-1\.drawing_facts"):
                bench.validate_drawing_facts(facts, "guide/block-1")
        for count in (0, -1, True, False, 1.0, 1.5, "1", None, 2**53):
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, r"guide/block-1\.drawing_facts\.sequence_messages"):
                bench.validate_drawing_facts({"sequence_messages": {"message-000000": count}}, "guide/block-1")

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

    def test_editor_preview_reuses_the_output_id_through_a_rejected_revision(self):
        fixed, rotating = plan(names=["editor-fixed-id", "editor-rotating-id"])["scenarios"]
        requests = fixed["requests"]
        self.assertEqual([r["diagram_id"] for r in requests], ["preview-01"] * 3)
        self.assertEqual(len({r["diagram_id"] for r in rotating["requests"]}), 3)
        self.assertEqual([r["expected_outcome"] for r in requests], ["draw", "reject", "draw"])
        for a, b in zip(requests, rotating["requests"], strict=True):
            self.assertEqual({**a, "diagram_id": b["diagram_id"]}, b)

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

    def test_external_manifest_rejects_unknown_and_duplicate_drawing_fields(self):
        prefix = '{"id":"guide","origin":"local","files":["a.mmd"],'
        cases = [
            ('"drawing_fact":{}', "drawing_fact"),
            ('"drawing_facts":[]', "keyed by fixture"),
            ('"drawing_facts":{"b.mmd":{"groups":{"CI":["L"]}}}', "b.mmd"),
            ('"drawing_facts":{"a.mmd":{"arrow":[]}}', r"guide/a.mmd\.drawing_facts"),
            ('"drawing_facts":{},"drawing_facts":{}', "duplicate JSON key 'drawing_facts'"),
            ('"drawing_facts":{"a.mmd":{},"a.mmd":{"groups":{"CI":["L"]}}}', "duplicate JSON key 'a.mmd'"),
            ('"drawing_facts":{"a.mmd":{"groups":{"CI":["L"],"CI":["T"]}}}', "duplicate JSON key 'CI'"),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            (directory / "a.mmd").write_text("flowchart LR\n  L --> T\n", encoding="utf-8")
            manifest = directory / "doc.json"
            for fields, error in cases:
                with self.subTest(fields=fields), self.assertRaisesRegex(ValueError, error):
                    manifest.write_text(prefix + fields + "}", encoding="utf-8")
                    plan(names=["guide"], documents=[manifest])

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

    def test_order_suite_replays_the_reviewed_bytes_in_work_and_public_svg(self):
        result = plan("topology-orders")
        groups = {}
        for case, row in zip(result["scenarios"], result["topologies"], strict=True):
            canonical = row.get("canonical_id", row["id"])
            groups.setdefault(canonical, []).append(row["source_sha256"])
            self.assertEqual((case["id"], case["requests"][0]["source"]), (row["id"], row["source"]))
            self.assertEqual(case["requests"][0]["expected_labels"], [f"C{i}" for i in range(row["nodes"])])
        # An algorithm or interpreter update must preserve the nine frozen sources per graph.
        self.assertEqual({k: bench.digest("\n".join(v).encode()) for k, v in groups.items()}, {
            "scc-75": "b8f0f9f1afe84bad035367fb202fd0e466806aa7efefbce3e05198242e99ef72",
            "scc-dag-75": "73aa985a49bad8d2fba87be8d93295fa4282cf2d653f62491c836ffd5b9eda63",
            "scc-100": "63db828896e69bb547bba5bbba673908bca16e20f77ee72983a6208aa88664a4",
            "scc-dag-100": "4449550eb542897afeb4b8ae82715abf45f0e9329076d5ebb49378ef25d3d80b",
        })
        one = plan("topology-orders", ["scc-100-declarations-3"])
        self.assertEqual([case["id"] for case in one["scenarios"]], ["scc-100-declarations-3"])

    def test_work_comparison_distinguishes_geometry_svg_and_charged_work(self):
        ok = {"status": "accepted", "bytes": 20, "sha256": "svg"}
        old = {"default": {"status": "accepted"}, "high": {"sha256": "layout"}, "high_budget": 1000,
               "boundary": {"minimum_passing_budget": 99}, "svg": {"default": {"status": "layout_work_rejected"},
                                                                  "high": {**ok, "layout_work_units": 120}}}
        new = {**old, "boundary": {"minimum_passing_budget": 90}, "svg": {"default": ok, "high": {**ok, "layout_work_units": 95}}}
        compared = bench.compare_topology(old, new)
        self.assertEqual((compared["default_status"], compared["default_svg_status"], compared["delta"]),
                         (["accepted", "accepted"], ["layout_work_rejected", "accepted"], -9))
        new["svg"]["high"]["sha256"] = "different svg"
        self.assertIsNone(bench.compare_topology(old, new)["delta"])
        del new["svg"]
        with self.assertRaisesRegex(ValueError, "incompatible"):
            bench.compare_topology(old, new)
        self.assertNotIn("default_svg_status", bench.compare_topology(new, new))

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
    def test_public_layout_json_requires_the_flat_flowchart_envelope_and_real_endpoints(self):
        # Identity fields from the public writer for the frozen small-svg request.
        graph = {"nodes": [{"id": "A", "is_cluster": False}, {"id": "B", "is_cluster": False}],
                 "edges": [{"id": "L_A_B_0", "from": "A", "to": "B"}], "clusters": []}
        document = {"schema_version": 1, "family_id": "flowchart", "diagram_type": "flowchart-v2",
                    "layout": {"FlowchartV2": graph}}
        self.assertEqual(bench.flat_layout_inventory(document), dict(nodes=["A", "B"], edges=[["L_A_B_0", "A", "B"]]))
        for overrides, error in (({"diagram_type": None, "diagramType": "flowchart-v2"}, "diagram_type"),
                                 ({"family_id": "swimlane"}, "family_id"),
                                 ({"layout": {"FlowchartV2": dict(graph, clusters=[{"id": "G"}])}}, "flat"),
                                 ({"layout": {"FlowchartV2": dict(graph, nodes=graph["nodes"][:1])}}, "endpoint")):
            with self.subTest(overrides=overrides), self.assertRaisesRegex(ValueError, error):
                bench.flat_layout_inventory({**document, **overrides})

    def test_automatic_build_opts_into_shoal_for_either_bench(self):
        for metric, target in (("clock", "public_workloads"), ("memory", "public_workload_memory")):
            artifact = dict(reason="compiler-artifact", target={"name": target}, executable="runner")
            with patch.object(bench.subprocess, "run") as build:
                build.return_value.stdout = json.dumps(artifact).encode()
                bench.build_runner(metric)
                self.assertNotIn("--features", build.call_args.args[0])
                bench.build_runner(metric, shoal=True)
                command = build.call_args.args[0]
                self.assertEqual(command[command.index("--features") + 1], "layout-shoal")
                self.assertEqual(command[command.index("--bench") + 1], target)

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
        receipts = [proof[0], dict(proof[1], family="agentflow")]
        self.assertNotIn("status", check(outputs, receipts)[1])
        self.assertEqual(receipts[1]["backend"], "shoal")

    def test_standalone_shoal_rejects_the_source_pinned_control_and_preserves_default_work(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            frozen, runner, out = (path / name for name in ("plan.json", "runner", "report.json"))
            runner.write_bytes(b"bench")
            bench.write(frozen, plan(names=["flowchart-layout-dagre-classic", "chain-50"]))
            args = ["rust", "--plan", str(frozen), "--out", str(out)]

            def pinned(_, job):
                self.assertTrue(job["verify_only"] and job["verify_layout"])
                self.assertEqual(job["site"], {"layout": "shoal"})
                receipts, outputs = [], []
                for request in job["requests"]:
                    self.assertIn("  layout: dagre\n", request["source"])
                    receipts.append(dict(family="flowchart", required_capabilities=[], diagram_id=request["diagram_id"],
                                         source_sha256=request["source_sha256"]))
                    outputs.append(dict(bytes=20, observed_size=20, sha256="a" * 64))
                return dict(harness_sha256="bench", layout_evidence=receipts, outputs=outputs)

            with patch.object(bench, "child", side_effect=pinned), patch.object(bench, "build_runner", return_value=runner) as build:
                name = "flowchart-layout-dagre-classic"
                self.assertEqual(bench.main(args + ["--scenario", name, "--site-layout", "shoal", "--operation", "layout-json"]), 1)
                build.assert_called_once_with("memory", shoal=True)
                row = bench.load(out)["rows"][name]
                self.assertEqual((row["completed"], row["samples"]), (0, []))
                self.assertTrue(all(r["backend"] == "dagre" for r in row["layout_evidence"]))

            work = args + ["--metric", "work", "--runner", str(runner)]
            legacy = dict(harness_sha256="old", boundary={"backend": "elk"})
            with patch.object(bench, "child", return_value=legacy) as child:
                with self.assertRaises(SystemExit):
                    bench.main(work + ["--site-layout", "shoal"])
                child.assert_not_called()
                self.assertEqual(bench.main(work), 0)
                self.assertNotIn("site", child.call_args.args[1])
                self.assertEqual(bench.load(out)["rows"]["chain-50"]["boundary"]["backend"], "elk")

    def test_one_binary_compares_frozen_layout_and_svg_occurrences(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            frozen = path / "plan.json"
            source = "flowchart LR\nA[Start] shared@-->|yes| B[End]\nA shared@-->|yes| B\n"
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
                edge = {"id": "shared", "from": "A", "to": "B"}
                edges = [edge, dict(edge, to="A") if different == "endpoint" else edge]
                geometry = dict(schema_version=1, family_id="flowchart", diagram_type="flowchart-v2",
                                layout={"FlowchartV2": dict(nodes=nodes, edges=edges, clusters=[])})
                label = "no" if different == "label" else "yes"
                svg = ('<svg id="preview"><g data-et="node" data-id="A"><text>Start</text></g>'
                       '<g data-et="node" data-id="B"><text>End</text></g>'
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


if __name__ == "__main__":
    unittest.main()
