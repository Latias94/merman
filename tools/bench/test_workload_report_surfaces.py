import json
from copy import deepcopy
from hashlib import sha256
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

if __package__:
    from .test_workload_reference import report as reference_report
    from .workload_report_surfaces import load_browser, load_cli, load_rust
else:
    from test_workload_reference import report as reference_report
    from workload_report_surfaces import load_browser, load_cli, load_rust


def fingerprint(data):
    return sha256(data).hexdigest()


class SurfaceTests(unittest.TestCase):
    def setUp(self):
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        scratch = TemporaryDirectory(dir=target, prefix="reader-surfaces-")
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)

    def write(self, name, data):
        path = self.root / name
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        return path

    def cli_pair(self, fmt="svg", **request_changes):
        left, right = (reference_report("cli", product, fmt=fmt) for product in ("merman", "mmdc"))
        source = "flowchart TD\r\nA[Alpha] --> B[Café]"
        for raw in (left, right):
            for request in raw["scenarios"][0]["requests"]:
                request.update(source_sha256=fingerprint(source.encode()))
            raw["host"]["hostname"] = "private-host"
            raw["version_probe"]["log_tail"] = "arbitrary private log"
        requests = [dict(r, source=source) for r in left["scenarios"][0]["requests"]]
        requests[0].update(request_changes)
        scenarios = [dict(id="doc", requests=requests), dict(id="other", requests=[requests[1]])]
        plan = self.write("plan.json", dict(schema_version=1, scenarios=scenarios))
        left["plan_sha256"] = right["plan_sha256"] = fingerprint(plan.read_bytes())
        return plan, left, right

    def cli(self, plan, left, right):
        return load_cli(plan, self.write("left.json", left), self.write("right.json", right))

    def test_cli_product_files_and_median_of_batch_sums(self):
        for fmt in ("svg", "png"):
            with self.subTest(fmt=fmt):
                plan, left, right = self.cli_pair(fmt)
                lane = self.cli(plan, left, right)
                row, = lane["rows"]
                self.assertEqual((lane["format"], lane["samples"]), (fmt, 3))
                self.assertEqual((lane["coverage"]["planned_requests"], lane["coverage"]["reported_requests"]), (3, 2))
                self.assertEqual(lane["coverage"]["common_stable"], 2)
                self.assertEqual(row["stable_coverage"]["label_verified"], dict(requests=1, distinct_sources=1))
                self.assertEqual(row["common_render_ms"], dict(left=101, right=202, right_over_left=2))
                self.assertEqual(row["complete_batch_ms"]["left"], 111)
                self.assertEqual(lane["provenance"]["reports_sha256"]["right"], fingerprint((self.root / "right.json").read_bytes()))
                artifact, = lane["products"]["left"]["artifact_identity"]["files"]
                self.assertEqual(artifact, dict(role="executable", bytes=7, sha256="f" * 64))
                self.assertEqual(lane["host"], dict(platform="win32", arch="x64"))
                if fmt == "png":
                    self.assertEqual(row["requests"][0]["right"]["dimensions_px"], [[600, 200]])
                for private in ("private-host", "/products/", "private log"):
                    self.assertNotIn(private, json.dumps(lane, allow_nan=False))

    def test_cli_preserves_historical_withholding_and_decoder_gate(self):
        plan, left, right = self.cli_pair("png")
        for change, reason in (({"png_decoder": None}, "PNG decoder"),
                               ({"artifact_identity": None}, "artifact identity"),
                               ({"collector_sha256": None}, "collector")):
            with self.subTest(reason=reason):
                lane = self.cli(plan, left, {**right, **change})
                row, = lane["rows"]
                self.assertEqual(row["common_stable"], 2)
                self.assertEqual(row["common_render_ms"], dict(left=101, right=202, right_over_left=None))
                self.assertIsNone(row["complete_batch_ms"]["right_over_left"])
                self.assertIn(reason, row["ratio_withheld_reason"])
                self.assertIn(reason, lane["ratio_withheld_reason"])

    def test_cli_zero_process_or_batch_time_only_withholds_its_own_ratio(self):
        for zero_clock in ("process", "batch"):
            plan, left, right = self.cli_pair()
            for batch in left["scenarios"][0]["batches"]:
                if zero_clock == "batch":
                    batch["wall_ms"] = 0
                else:
                    for request in batch["results"]:
                        request["elapsed_ms"] = 0
            row, = self.cli(plan, left, right)["rows"]
            group, = row["render_timing_groups"]
            self.assertIsNone(row["ratio_withheld_reason"])
            self.assertEqual(group["render_ms"]["right_over_left"] is None, zero_clock == "process")
            self.assertEqual(row["complete_batch_ms"]["right_over_left"] is None, zero_clock == "batch")
            self.assertEqual(bool(group["ratio_withheld_reason"]), zero_clock == "process")
            self.assertEqual(bool(row["complete_batch_withheld_reason"]), zero_clock == "batch")

    def test_cli_requires_frozen_drawing_facts_even_in_older_raw_reports(self):
        plan, left, right = self.cli_pair(drawing_facts={"arrows": [{"edge": "L_A_B_0", "from": "A", "to": "B"}]})
        for raw in (left, right):
            raw["drawing_audit"] = {"status": "passed"}
        row, = self.cli(plan, left, right)["rows"]
        self.assertEqual(row["drawing_checks"], dict(left="not-checked", right="not-checked"))
        self.assertEqual(row["common_render_ms"]["left"], 101)
        self.assertIsNone(row["render_timing_groups"][0]["render_ms"]["right_over_left"])
        self.assertIn("drawing", row["ratio_withheld_reason"])

    def test_cli_rejections_and_failed_requests_stay_in_the_denominator(self):
        plan, left, right = self.cli_pair(expected_outcome="reject")
        for raw in (left, right):
            raw["scenarios"][0]["requests"][0]["expected_outcome"] = "reject"
            for batch in raw["scenarios"][0]["batches"]:
                batch["results"][0].update(status="expected-rejection", diagnostic="private log")
        for batch in right["scenarios"][0]["batches"]:
            batch["results"][1]["status"] = "observer-limit"
        lane = self.cli(plan, left, right)
        row, = lane["rows"]
        self.assertEqual((row["planned_requests"], row["common_expected_rejections"], row["common_stable"]), (2, 1, 0))
        self.assertEqual(row["requests"][1]["right"], dict(state="missing", statuses=["observer-limit"]))
        self.assertEqual(row["render_timing_groups"], [])
        self.assertIsNone(row["complete_batch_ms"])
        self.assertTrue(row["ratio_withheld_reason"])
        self.assertNotIn("private log", json.dumps(lane))

    def test_cli_rejects_reversed_products_wrong_plan_scope_host_and_request(self):
        plan, left, right = self.cli_pair()
        for a, b in ((right, left), ({**left, "surface": "node"}, right),
                     (left, {**right, "host": {**right["host"], "hostname": "different"}}),
                     (left, {**right, "version": "1.2.3\nC:/private/version.log"})):
            with self.assertRaises(ValueError):
                self.cli(plan, a, b)
        wrong = deepcopy(right)
        wrong["scenarios"][0]["requests"][0]["diagram_id"] = "different"
        with self.assertRaisesRegex(ValueError, "request identity"):
            self.cli(plan, left, wrong)
        plan.write_bytes(plan.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "plan_sha256"):
            self.cli(plan, left, right)

    def native(self):
        source = "flowchart LR\nA-->B\n"
        graph = dict(id="graph", nodes=2, edges=1, source=source, source_sha256=fingerprint(source.encode()))
        reordered = dict(graph, id="alternate", canonical_id="graph", statement_order="edges-reverse")
        plan = self.write("plan.json", dict(schema_version=1, topologies=[graph, reordered, dict(graph, id="unreported")]))
        accepted = dict(status="accepted", sha256="a" * 64, bytes=8)
        limit = dict(status="layout_work_rejected", id="max_layout_work_units", phase="layout_model", actual=12, maximum=10)
        row = dict(nodes=2, edges=1, default_budget=10, high_budget=100, default=accepted, high=accepted,
                   svg=dict(default=limit, high=dict(accepted, layout_work_units=12)),
                   boundary=dict(backend="elk", minimum_passing_budget=9, svg_consumed_layout_work_units=12,
                                 minimum_minus_one=dict(limit, actual=9, maximum=8)))
        rejected = dict(status="source_max_edges_rejected", code="parse", message="private-log C:/folder/file")
        blocked = dict(row, default=rejected, high=rejected, svg=dict(default=rejected, high=rejected), boundary=None)
        report = dict(schema_version=1, surface="rust", plan_sha256=fingerprint(plan.read_bytes()),
                      options=dict(metric="work", operation="layout-work", warmups=0, samples=1, site_layout=None, site_css_bytes=0),
                      host=["Windows", "AMD64", "private-host"], runner_sha256="c" * 64, harness_sha256="d" * 64,
                      revision="e" * 40, rows=dict(graph=row, alternate=blocked))
        return plan, report

    def test_native_layout_svg_and_parser_outcomes_have_independent_denominators(self):
        plan, report = self.native()
        path = self.write("work.json", report)
        lane = load_rust(plan, path)
        first, second = lane["rows"]
        self.assertEqual((first["layout_default"], first["svg_default"]), ("accepted", "layout_work_rejected"))
        self.assertEqual((first["minimum_passing_budget"], first["svg_consumed_layout_work_units"]), (9, 12))
        self.assertEqual(first["minimum_minus_one"]["maximum"], 8)
        self.assertNotIn("canonical_id", first)
        self.assertEqual((second["canonical_id"], second["statement_order"]), ("graph", "edges-reverse"))
        self.assertIsNone(second["minimum_passing_budget"])
        self.assertIsNone(second["svg_consumed_layout_work_units"])
        self.assertEqual(second["boundary_withheld_reason"], "source_max_edges_rejected")
        self.assertEqual((lane["coverage"]["planned_topologies"], lane["coverage"]["reported_topologies"]), (3, 2))
        self.assertEqual(lane["coverage"]["svg_default"], dict(layout_work_rejected=1, source_max_edges_rejected=1))
        self.assertEqual(lane["provenance"]["report_sha256"], fingerprint(path.read_bytes()))
        for private in ("private-host", "private-log", "C:/folder", "right_over_left"):
            self.assertNotIn(private, json.dumps(lane, allow_nan=False))

    def test_native_rejects_wrong_scope_plan_or_unwitnessed_boundary(self):
        plan, native = self.native()
        for changes in ({"surface": "cli"}, {"options": {**native["options"], "metric": "clock"}},
                        {"plan_sha256": "1" * 64}, {"rows": {"unknown": native["rows"]["graph"]}}):
            with self.assertRaises(ValueError):
                load_rust(plan, self.write("work.json", {**native, **changes}))
        for mutate in (lambda row: row.update(edges=4), lambda row: row["boundary"]["minimum_minus_one"].update(maximum=7),
                       lambda row: row["svg"]["high"].update(layout_work_units=13)):
            changed = deepcopy(native)
            mutate(changed["rows"]["graph"])
            with self.assertRaises(ValueError):
                load_rust(plan, self.write("work.json", changed))
        native["rows"]["alternate"]["svg"]["high"] = native["rows"]["graph"]["svg"]["high"]
        with self.assertRaisesRegex(ValueError, "maxEdges"):
            load_rust(plan, self.write("work.json", native))

    def browser(self):
        source = "flowchart TD\nA-->B\n"
        versions = dict(merman="0.11.0", mermaid="12.3.0")
        fixture = dict(id="basic-flowchart", family="flowchart", status="success", attempted=True, failure=None,
                       source=dict(bytes=len(source.encode()), sha256=fingerprint(source.encode())))
        for key, mode, metric in (("cold", "realm-cold", "firstPublishableSvgMs"), ("warm", "warm", "warmPublishableSvgMs")):
            plan = dict(mode=mode, iterations=2, seed=19)
            if key == "warm":
                plan["warmups"] = 1
            stats = {engine: {metric: dict(count=2, median=value)} for engine, value in (("merman", 4), ("mermaid", 5))}
            report = dict(schemaVersion=7, terminalStatus="success", input=dict(source=source), plan=plan,
                          versions=dict(expected=versions, observed={engine: [ver] for engine, ver in versions.items()}),
                          environment=dict(platform="Linux x86_64", userAgent="HeadlessChrome/133.0.0.0 C:/private-host"),
                          aggregates=dict(engines=stats, ratios={metric: 1.25 if key == "cold" else None}))
            fixture[key] = dict(mode=mode, seed=19, status="success", report=report, failure=None, skipReason=None)
        return dict(kind="merman-browser-family-corpus", schemaVersion=2, benchmarkReportSchemaVersion=7,
                    execution=dict(fixtureIsolation="fresh-browser-process-per-fixture"), versions=versions, terminalStatus="success",
                    catalog=dict(id="playground-generated-examples/family-baseline", role="family-baseline", availableFamilies=37,
                                 source="playground/src/generated/examples.ts", mermaidBaseline="mermaid@12.3.0"),
                    coverage=dict(availableFamilies=37, selectedFamilies=1, attemptedFamilies=1, succeededFamilies=1, failedFamilies=0),
                    fixtures=[fixture], run=dict(masterSeed=7, iterations=2, warmups=1, order=[fixture["id"]]))

    def test_browser_keeps_cold_warm_and_withheld_owner_ratio_separate(self):
        original = self.browser()
        path = self.write("browser.json", original)
        lane = load_browser(path)
        row, = lane["rows"]
        self.assertEqual(row["cold"]["median_ms"], dict(left=4, right=5, right_over_left=1.25))
        self.assertEqual(row["warm"]["median_ms"], dict(left=4, right=5, right_over_left=None))
        self.assertTrue(row["warm"]["ratio_withheld_reason"])
        self.assertEqual(row["warm"]["samples"], dict(left=2, right=2))
        self.assertEqual(lane["provenance"]["report_sha256"], fingerprint(path.read_bytes()))
        self.assertNotIn("private-host", json.dumps(lane, allow_nan=False))
        original.update(terminalStatus="complete-with-errors")
        original["coverage"].update(succeededFamilies=0, failedFamilies=1)
        fixture = original["fixtures"][0]
        fixture.update(status="failure")
        fixture["warm"].update(status="failure", failure={"message": "private log"}, report=None)
        lane = load_browser(self.write("browser.json", original))
        self.assertEqual(lane["coverage"]["failed_families"], 1)
        self.assertEqual(lane["rows"][0]["cold"]["median_ms"]["right_over_left"], 1.25)
        self.assertIsNone(lane["rows"][0]["warm"]["median_ms"]["left"])
        self.assertNotIn("private log", json.dumps(lane))
        fixture["warm"].update(report=dict(schemaVersion=7, terminalStatus="complete-with-errors"))
        failed = load_browser(self.write("browser.json", original))["rows"][0]["warm"]
        self.assertEqual(failed["samples"], dict(left=None, right=None))

    def test_browser_rejects_other_report_kinds_and_drifted_source_version_or_mode(self):
        browser = self.browser()
        for changes in ({"kind": "merman-browser-family-corpus-fixture"}, {"benchmarkReportSchemaVersion": 6},
                        {"run": {**browser["run"], "order": ["other"]}}):
            with self.assertRaises(ValueError):
                load_browser(self.write("browser.json", {**browser, **changes}))
        for field, changes in (("input", {"source": "other"}), ("plan", {"mode": "realm-cold"}),
                               ("versions", {"observed": {"merman": ["0.10.0"], "mermaid": ["12.3.0"]}})):
            changed = deepcopy(browser)
            changed["fixtures"][0]["warm"]["report"][field].update(changes)
            with self.assertRaises(ValueError):
                load_browser(self.write("browser.json", changed))


if __name__ == "__main__":
    unittest.main()
