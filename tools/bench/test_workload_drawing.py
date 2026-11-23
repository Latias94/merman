from copy import deepcopy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import workload_drawing as drawing


ARROW = {"edge": "L_A_B_0", "from": "A", "to": "B"}
FACTS = {"arrows": [ARROW], "groups": {"CI": ["A", "B"]}}
VERDICTS = [dict(kind="arrow", **ARROW, status="passed"),
            dict(kind="membership", group="CI", node="A", status="passed"),
            dict(kind="membership", group="CI", node="B", status="passed")]
SOURCE = "flowchart LR\nsubgraph CI\nA-->B\nend\n"
BRANCH_LABEL = "feature/example-branch"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def request(diagram_id, facts=None, *, source=SOURCE):
    row = dict(fixture="../external.mmd", source=source, source_sha256=sha(source.encode()), diagram_id=diagram_id)
    if facts is not None:
        row["drawing_facts"] = facts
    return row


def svg(engine, diagram_id):
    return f'\ufeff<svg xmlns="http://www.w3.org/2000/svg" id="{diagram_id}">\r\n<text>{engine} 图🚀e\u0301</text></svg>\r\n'.encode()


def browser_reply(rows, **changes):
    data = dict(browser="Chrome/test", observer_sha256=sha(drawing.OBSERVER.read_bytes()), results=rows)
    return subprocess.CompletedProcess([], 0, json.dumps({**data, **changes}).encode(), b"")


class DrawingAuditTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.plan = self.root / "plan.json"
        self.bundle = self.root / "bundle"
        self.bundle.mkdir()
        self.freeze([dict(id="doc", requests=[request("unreviewed"), request("checked", FACTS)])])

    def freeze(self, scenarios):
        self.scenarios = scenarios
        self.plan.write_bytes(json.dumps(dict(schema_version=1, scenarios=scenarios)).encode())

    def collector(self, surface, samples=2):
        node = surface == "node"
        engine = "pele" if node else "mmdc"
        original = self.root / "original" / engine
        original.mkdir(parents=True, exist_ok=True)
        report = dict(schema_version=1, surface=surface, engine=engine, plan_sha256=sha(self.plan.read_bytes()),
                      samples=samples, scenarios=[])
        if node:
            report.update(output_format="svg", package=dict(name="pele", version="1.2.3"),
                          backend="javascript", render_api="render", drawing_dir=str(original))
        else:
            report.update(format="svg", version="1.2.3", version_probe=dict(status="ok"), run_dir=str(original))
        for i, scenario in enumerate(self.scenarios):
            requests = [{k: r[k] for k in ("fixture", "diagram_id", "source_sha256")} for r in scenario["requests"]]
            row = dict(id=scenario["id"], requests=requests)
            batches = []
            for b in range(samples + int(node)):
                results = []
                for j, req in enumerate(requests):
                    data = svg(engine, req["diagram_id"])
                    if node:
                        result = dict(status="ok", output_bytes=len(data), output_sha256=sha(data))
                        if b == samples and "drawing_facts" in scenario["requests"][j]:
                            req["drawing_artifact"] = f"{i}-{j}.svg"
                            (original / req["drawing_artifact"]).write_bytes(data)
                    else:
                        name = f"s{i + 1}-b{b + 1}-r{j + 1}.svg"
                        result = dict(status="ok", output=name, bytes=len(data), sha256=sha(data))
                        (original / name).write_bytes(data)
                    results.append(result)
                batches.append(dict(results=results))
            if node:
                row.update(index=i, cold_batch=batches[0], warm_samples=batches[1:])
            else:
                row["batches"] = batches
            report["scenarios"].append(row)
        path = self.bundle / f"{engine}.json"
        path.write_bytes(json.dumps(report).encode())
        return path, report

    def check_unavailable(self, pair, reason):
        with patch.object(drawing.subprocess, "run") as observe:
            report = drawing.audit_drawings(self.plan, [pair])
        observe.assert_not_called()
        self.assertIsNone(report["observer"])
        scenario = report["products"][0]["scenarios"]["doc"]
        case, = scenario["cases"]
        self.assertEqual((scenario["status"], case["status"], case["reason"]), ("unavailable", "unavailable", reason))
        self.assertEqual(case["facts"], [dict(f, status="unavailable") for f in VERDICTS])
        return case

    def test_moved_node_and_cli_observe_exact_bytes_and_each_declared_occurrence_in_one_process(self):
        self.scenarios[0]["requests"].append(request("repeat", {"arrows": [ARROW]}))
        self.freeze([*self.scenarios, dict(id="unreviewed", requests=[request("other")])])
        node, cli = self.collector("node", samples=1), self.collector("cli")
        moved = self.bundle / "drawings" / "pele" / "0-1.svg"
        moved.parent.mkdir(parents=True)
        moved.write_bytes(svg("pele", "checked"))
        (Path(node[1]["drawing_dir"]) / moved.name).write_bytes(b"old contents")
        # Either stable CLI sample is evidence if the other timed file was not retained.
        (Path(cli[1]["run_dir"]) / "s1-b1-r2.svg").unlink()
        failed = dict(VERDICTS[1], status="failed", reason="node extends outside its cluster")
        peer_facts = [VERDICTS[2], failed, VERDICTS[0]]
        reply = browser_reply([list(reversed(VERDICTS)), [VERDICTS[0]], peer_facts, [VERDICTS[0]]])
        with patch.object(drawing.subprocess, "run", return_value=reply) as observe:
            audit = drawing.audit_drawings(self.plan, [node, cli], chrome=self.root / "chrome with spaces")
        observe.assert_called_once()
        args, kwargs = observe.call_args
        self.assertEqual(args[0], ["node", str(drawing.OBSERVER), "--chrome", str(self.root / "chrome with spaces")])
        expected_jobs = [dict(svg=svg(engine, diagram_id).decode(), facts=dict(facts, label_widths={}))
                         for engine in ("pele", "mmdc") for diagram_id, facts in
                         (("checked", FACTS), ("repeat", {"arrows": [ARROW], "groups": {}}))]
        self.assertEqual(json.loads(kwargs["input"]), {"jobs": expected_jobs})
        self.assertEqual(json.loads(kwargs["input"])["jobs"][0]["svg"].encode("utf-8"), moved.read_bytes())
        self.assertEqual(audit["planned"], dict(occurrences=2, facts=4))
        self.assertEqual(audit["plan_sha256"], sha(self.plan.read_bytes()))
        self.assertEqual(audit["observer"]["observer_sha256"], sha(drawing.OBSERVER.read_bytes()))
        left, right = audit["products"]
        self.assertEqual((left["surface"], left["package"], right["engine"], right["version"]),
                         ("node", "pele", "mmdc", "1.2.3"))
        self.assertEqual([p["report_path"] for p in (left, right)], [str(node[0]), str(cli[0])])
        for product in (left, right):
            self.assertEqual(product["scenarios"]["unreviewed"], dict(cases=[], facts=0, occurrences=0, status="not-declared"))
            self.assertEqual([c["request_index"] for c in product["scenarios"]["doc"]["cases"]], [1, 2])
        good, bad = (p["scenarios"]["doc"] for p in (left, right))
        self.assertEqual((good["status"], bad["status"]), ("passed", "failed"))
        self.assertEqual(good["cases"][0]["artifact"], dict(path=str(moved), bytes=len(moved.read_bytes()),
                                                         sha256=sha(moved.read_bytes()), sample_index=0))
        self.assertEqual(good["cases"][0]["facts"], VERDICTS)
        self.assertEqual(bad["cases"][0]["facts"], [VERDICTS[0], failed, VERDICTS[2]])
        self.assertEqual(Path(bad["cases"][0]["artifact"]["path"]).name, "s1-b2-r2.svg")
        self.assertEqual(bad["cases"][0]["artifact"]["sample_index"], 1)

    def test_label_widths_count_each_node_and_preserve_the_observed_status(self):
        widths = {"C": BRANCH_LABEL, "D": "Release"}
        width_only = request("branch", {"label_widths": widths}, source=f'flowchart LR\nC["{BRANCH_LABEL}"]-->D[Release]')
        mixed = {**FACTS, "label_widths": {"A": "A"}}
        self.freeze([dict(id="doc", requests=[width_only, request("mixed", mixed)])])
        pair = self.collector("node")
        release = dict(kind="label-width", node="D", label="Release", status="passed")
        start = dict(kind="label-width", node="A", label="A", status="passed")
        for outcome in (dict(status="passed"), dict(status="failed", reason="label extends past the right edge"),
                        dict(status="unavailable", reason="label geometry is unavailable")):
            branch = dict(kind="label-width", node="C", label=BRANCH_LABEL, **outcome)
            reply = browser_reply([[release, branch], [start, *VERDICTS]])
            with self.subTest(outcome=outcome), patch.object(drawing.subprocess, "run", return_value=reply) as observe:
                audit = drawing.audit_drawings(self.plan, [pair])
                jobs = json.loads(observe.call_args.kwargs["input"])["jobs"]
                self.assertEqual([job["facts"] for job in jobs], [dict(arrows=[], groups={}, label_widths=widths), mixed])
                self.assertEqual(audit["planned"], dict(occurrences=2, facts=6))
                row = audit["products"][0]["scenarios"]["doc"]
                first, second = row["cases"]
                self.assertEqual((row["status"], first["status"], second["status"]), (outcome["status"], outcome["status"], "passed"))
                self.assertEqual(first["facts"], [branch, release])
                self.assertEqual(second["facts"], [*VERDICTS, start])

    def test_expected_sequence_count_stays_bound_to_its_occurrence_and_observed_count(self):
        facts = {"sequence_messages": {"status": 32}}
        self.freeze([dict(id="sequence", requests=[request("loop", facts, source="sequenceDiagram\nA->>B: status")])])
        pair = self.collector("node")
        fact = dict(kind="sequence-message-count", label="status", expected=32)
        for observed in (1, 32):
            status = "passed" if observed == 32 else "failed"
            verdict = dict(fact, observed=observed, status=status)
            with patch.object(drawing.subprocess, "run", return_value=browser_reply([[verdict]])):
                product, = drawing.audit_drawings(self.plan, [pair])["products"]
            self.assertEqual(product["scenarios"]["sequence"]["cases"][0]["facts"], [verdict])
        for wrong in (dict(fact, expected=1, observed=1, status="passed"), dict(fact, observed=1, status="passed")):
            with patch.object(drawing.subprocess, "run", return_value=browser_reply([[wrong]])), self.assertRaises(ValueError):
                drawing.audit_drawings(self.plan, [pair])

    def test_label_width_observer_must_echo_the_exact_node_and_label_once(self):
        facts = {"label_widths": {"C": BRANCH_LABEL}}
        self.freeze([dict(id="doc", requests=[request("branch", facts, source=f'flowchart LR\nC["{BRANCH_LABEL}"]')])])
        pair = self.collector("cli")
        fact = dict(kind="label-width", node="C", label=BRANCH_LABEL, status="passed")
        for observed, error in (([], "fact count"), ([fact, fact], "fact count"),
                                ([dict(fact, node="D")], "fact identities"),
                                ([dict(fact, label=BRANCH_LABEL.upper())], "fact identities"),
                                ([dict(fact, status="ok")], "fact status")):
            with self.subTest(observed=observed), patch.object(drawing.subprocess, "run", return_value=browser_reply([observed])):
                with self.assertRaisesRegex(ValueError, error):
                    drawing.audit_drawings(self.plan, [pair])

    def test_real_render_failures_and_unrepeated_or_unstable_samples_keep_the_raw_outcome(self):
        source = self.collector("node")
        for cold, warm, reason in ((dict(status="render-error"), dict(status="render-error"), "renderer-failed"),
                                   (dict(status="render-error"), {}, "intermittent"),
                                   (dict(output_sha256="f" * 64), {}, "unstable-output"),
                                   (dict(output_sha256=None), {}, "unverified-output")):
            with self.subTest(reason=reason):
                path, report = deepcopy(source)
                row, = report["scenarios"]
                row["cold_batch"]["results"][1].update(cold)
                for batch in row["warm_samples"]:
                    batch["results"][1].update(warm)
                case = self.check_unavailable((path, report), reason)
                if "status" in cold:
                    self.assertIn("render-error", case["output"]["statuses"])
        self.check_unavailable(self.collector("cli", samples=1), "unverified-output")

    def test_failed_cli_request_keeps_other_cases_and_collector_status(self):
        self.scenarios[0]["requests"][0]["drawing_facts"] = {"groups": {"CI": ["A"]}}
        self.freeze(self.scenarios)
        path, raw = self.collector("cli")
        raw["status"] = "failed"
        for batch in raw["scenarios"][0]["batches"]:
            batch["results"][0] = dict(status="exit_error", returncode=1)
        with patch.object(drawing.subprocess, "run", return_value=browser_reply([VERDICTS])):
            product, = drawing.audit_drawings(self.plan, [(path, raw)])["products"]
        row = product["scenarios"]["doc"]
        first, second = row["cases"]
        self.assertEqual((product["report_status"], row["status"], second["status"]), ("failed", "unavailable", "passed"))
        self.assertEqual((first["reason"], first["output"]["statuses"]), ("renderer-failed", ["exit_error"]))

    def test_node_requires_the_recorded_occurrence_and_matching_local_or_original_bytes(self):
        pair = self.collector("node")
        req = pair[1]["scenarios"][0]["requests"][1]
        moved = self.bundle / "drawings" / "pele" / "0-1.svg"
        moved.parent.mkdir(parents=True)
        data = svg("pele", "checked")
        for corrupt in (data[:-1], data.replace(b"pele", b"xxxx")):
            with self.subTest(length=len(corrupt)):
                moved.write_bytes(corrupt)
                self.check_unavailable(pair, "invalid-artifact")  # A valid old file cannot mask bundle damage.
        moved.write_bytes(data)
        for wrong in ("0-0.svg", "../0-1.svg", "0-1.svg/../0-1.svg"):
            with self.subTest(wrong=wrong):
                req["drawing_artifact"] = wrong
                self.check_unavailable(pair, "invalid-artifact")
        del req["drawing_artifact"]
        self.check_unavailable(pair, "missing-artifact")
        req["drawing_artifact"] = moved.name
        moved.unlink()
        (Path(pair[1]["drawing_dir"]) / moved.name).unlink()
        self.check_unavailable(pair, "missing-artifact")

    def test_stable_bytes_that_cannot_be_decoded_as_utf8_never_reach_the_browser(self):
        pair = self.collector("node")
        raw = pair[1]
        data = b'<svg><text>\xff</text></svg>'
        (Path(raw["drawing_dir"]) / "0-1.svg").write_bytes(data)
        row, = raw["scenarios"]
        for batch in [row["cold_batch"], *row["warm_samples"]]:
            batch["results"][1].update(output_bytes=len(data), output_sha256=sha(data))
        self.assertIn("utf-8", self.check_unavailable(pair, "invalid-artifact")["detail"])

    def test_cli_uses_only_recorded_svg_outputs_within_run_dir_and_rejects_png(self):
        pair = self.collector("cli")
        report = pair[1]
        first = report["scenarios"][0]["batches"][0]["results"][1]
        original = Path(report["run_dir"])
        outside = original.parent / "outside.svg"
        outside.write_bytes(svg("mmdc", "checked"))
        for name in ("../outside.svg", r"..\outside.svg", str(outside), r"C:outside.svg", "sample.png", "s1-b2-r2.svg"):
            with self.subTest(name=name):
                first["output"] = name
                self.check_unavailable(pair, "invalid-artifact")
        first["output"] = "s1-b1-r1.svg"  # Exists, belongs to the other request.
        self.check_unavailable(pair, "invalid-artifact")
        first["svg_check"] = "s1-b1-r2.svg"
        report.update(format="png", png_svg_checks=[dict(output=first["svg_check"], status="ok")])
        with patch.object(drawing.subprocess, "run") as observe, self.assertRaisesRegex(ValueError, "requires SVG"):
            drawing.audit_drawings(self.plan, [pair])
        observe.assert_not_called()

    def test_moved_product_directory_link_cannot_redirect_artifact_reads(self):
        pair = self.collector("node")
        dest = self.bundle / "drawings"
        dest.mkdir()
        try:
            (dest / "pele").symlink_to(pair[1]["drawing_dir"], target_is_directory=True)
        except OSError as error:
            self.skipTest(f"directory symlink unavailable: {error}")
        self.check_unavailable(pair, "invalid-artifact")

    def test_report_binding_checks_the_plan_every_request_and_every_sample_even_without_facts(self):
        path, source = self.collector("node")
        for change in (lambda r: r.update(plan_sha256="0" * 64), lambda r: r.update(samples=3),
                       lambda r: r.update(samples=True), lambda r: r["scenarios"].clear(),
                       lambda r: r["scenarios"][0].update(index=1), lambda r: r["scenarios"][0].update(id="other"),
                       lambda r: r["scenarios"][0]["requests"].reverse(),
                       lambda r: r["scenarios"][0]["cold_batch"]["results"].pop(),
                       lambda r: r["scenarios"][0]["warm_samples"][0]["results"][1].update(output_bytes=float(len(svg("pele", "checked")))),
                       lambda r: r["scenarios"][0]["warm_samples"][0]["results"].append({"status": "ok"}),
                       lambda r: r.update(package=dict(name="mermaid", version="13.0.0"))):
            report = deepcopy(source)
            change(report)
            with self.assertRaises(ValueError):
                drawing.audit_drawings(self.plan, [(path, report)])
        for field in ("fixture", "source_sha256", "diagram_id"):
            with self.subTest(field=field):
                report = deepcopy(source)
                report["scenarios"][0]["requests"][0][field] = "other"
                with self.assertRaisesRegex(ValueError, "request identity"):
                    drawing.audit_drawings(self.plan, [(path, report)])
        self.scenarios[0]["requests"][0]["source"] = "changed in frozen plan"
        self.freeze(self.scenarios)
        with self.assertRaisesRegex(ValueError, "source_sha256"):
            drawing.audit_drawings(self.plan, [])

    def test_duplicate_plan_fields_cannot_hide_declared_membership(self):
        raw = self.plan.read_bytes().replace(b'"groups":', b'"groups": {"old": ["C"]}, "groups":')
        self.plan.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, "duplicate JSON key 'groups'"):
            drawing.audit_drawings(self.plan, [])

    def test_optional_drawing_required_markers_agree_with_each_occurrence_in_the_frozen_plan(self):
        for surface in ("node", "cli"):
            pair = self.collector(surface)
            unchecked, declared = pair[1]["scenarios"][0]["requests"]
            for req, value in ((unchecked, True), (declared, False), (declared, 1)):
                with self.subTest(surface=surface, marker=value):
                    req["drawing_required"] = value
                    with self.assertRaisesRegex(ValueError, "drawing_required"):
                        drawing.audit_drawings(self.plan, [pair])
                    del req["drawing_required"]
            for with_marker in (False, True):
                if with_marker:
                    declared["drawing_required"] = True
                with self.subTest(surface=surface, with_marker=with_marker):
                    with patch.object(drawing.subprocess, "run", return_value=browser_reply([VERDICTS])):
                        product, = drawing.audit_drawings(self.plan, [pair])["products"]
                    self.assertEqual(product["scenarios"]["doc"]["status"], "passed")

    def test_a_failed_node_scenario_preserves_the_verified_prefix_and_uncollected_tail(self):
        self.freeze([dict(id=sid, requests=[request("checked", FACTS)]) for sid in ("first", "second", "third")])
        path, raw = self.collector("node")
        raw.update(scenarios=raw["scenarios"][:1], status="failed", error="process timed out",
                   failed_scenario=dict(id="second", index=1, status="timeout"))
        with patch.object(drawing.subprocess, "run", return_value=browser_reply([VERDICTS])):
            product, = drawing.audit_drawings(self.plan, [(path, raw)])["products"]
        rows = product["scenarios"]
        self.assertEqual((product["failed_scenario"], product["error"]), (raw["failed_scenario"], raw["error"]))
        self.assertEqual([r["status"] for r in rows.values()], ["passed", "unavailable", "unavailable"])
        self.assertEqual(rows["second"]["cases"][0]["reason"], "scenario-failed")
        self.assertEqual(rows["third"]["cases"][0]["reason"], "not-collected")
        raw["failed_scenario"]["index"] = 2
        with self.assertRaisesRegex(ValueError, "failed_scenario"):
            drawing.audit_drawings(self.plan, [(path, raw)])

    def test_unavailable_geometry_and_observer_protocol_errors_never_become_a_pass(self):
        pair = self.collector("node")
        unknown = dict(VERDICTS[0], status="unavailable", reason="ambiguous endpoint")
        facts = [unknown, *VERDICTS[1:]]
        with patch.object(drawing.subprocess, "run", return_value=browser_reply([facts])):
            row = drawing.audit_drawings(self.plan, [pair])["products"][0]["scenarios"]["doc"]
        self.assertEqual(row["status"], "unavailable")
        self.assertEqual(row["cases"][0]["facts"][0], unknown)
        replies = [browser_reply([]), browser_reply([VERDICTS, VERDICTS]), browser_reply([VERDICTS[1:]]),
                   browser_reply([VERDICTS + [VERDICTS[0]]]), browser_reply([[VERDICTS[0], VERDICTS[1], VERDICTS[1]]]),
                   browser_reply([[dict(VERDICTS[0], status="ok"), *VERDICTS[1:]]]),
                   browser_reply([VERDICTS], browser=""), browser_reply([VERDICTS], observer_sha256="0" * 64),
                   subprocess.CompletedProcess([], 1, b"{}", b"Chrome launch failed"),
                   subprocess.CompletedProcess([], 0, b"not JSON", b"")]
        for index, key in ((0, "edge"), (0, "from"), (0, "to"), (1, "group"), (1, "node")):
            wrong = deepcopy(VERDICTS)
            wrong[index][key] = "other"
            replies.append(browser_reply([wrong]))
        for result in replies:
            with self.subTest(reply=result.stdout), patch.object(drawing.subprocess, "run", return_value=result):
                with self.assertRaises(ValueError):
                    drawing.audit_drawings(self.plan, [pair])


if __name__ == "__main__":
    unittest.main()
