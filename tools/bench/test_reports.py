import argparse
from contextlib import redirect_stderr, redirect_stdout
from copy import deepcopy
import io
import json
from pathlib import Path
from statistics import median
import tempfile
import unittest
from unittest.mock import patch

from merman_bench.workloads import STAGE_PHASES, main
from merman_bench.report import FORMAT, add_report_parser, generate_report, render
from merman_bench.report_surfaces import load_stages


REPORTS = Path(__file__).resolve().parents[2] / "docs/performance/reports"


def example_receipt():
    return json.loads((REPORTS / "2026-11-23-windows-x64.json").read_bytes())


def page_for(**lanes):
    return render(dict(format=FORMAT, date="2026-11-23", measurements=lanes), "reader.json").decode()


def timing_row(name, times, planned=8, **fields):
    pairs = [dict(round=i, merman_ms=merman, peer_ms=peer, ratio=peer / merman)
             for i, (merman, peer) in enumerate(times, 1)]
    ratios = [p["ratio"] for p in pairs]
    complete = len(pairs) == planned
    return dict(scenario=name, peer="pele", requests_per_batch=3, planned_pairs=planned, eligible_pairs=len(pairs),
                status="complete" if complete else "partial" if pairs else "unavailable", pairs=pairs,
                merman_ms=median(p["merman_ms"] for p in pairs) if complete else None,
                peer_ms=median(p["peer_ms"] for p in pairs) if complete else None,
                ratio=dict(min=min(ratios), median=median(ratios), max=max(ratios)) if complete else None, **fields)

import sys
from merman_bench import comment as render_perf_comment


class ReaderReportsTest(unittest.TestCase):
    def test_published_markdown_replays_from_the_portable_receipt_alone(self):
        receipts = sorted(REPORTS.glob("*.json"))
        self.assertTrue(receipts)
        for receipt in receipts:
            with self.subTest(receipt=receipt.name), redirect_stdout(io.StringIO()):
                self.assertEqual(main(["report", "--from", str(receipt), "--check"]), 0)
                page = receipt.with_suffix(".md").read_bytes()
                self.assertLess(page.index(b"## Overview"), page.index(b"## Reproduce this report"))
                self.assertIn(receipt.with_suffix(".md").name, (REPORTS / "README.md").read_text(encoding="utf-8"))
                self.assertIn(receipt.name.encode(), page)
                raw = receipt.read_bytes()
                for local in (b"E:\\\\Rust", b"C:\\\\Users", b"/Users/", b"/home/runner/", b"DESKTOP-", b'"hostname"', b"<svg", b"/node_modules/"):
                    self.assertNotIn(local, raw)

    def test_check_detects_an_edit_and_regeneration_restores_only_markdown(self):
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            copy = Path(tmp) / "copied measurements.json"
            copy.write_bytes(next(REPORTS.glob("*.json")).read_bytes())
            original = copy.read_bytes()
            self.assertEqual(main(["report", "--from", str(copy)]), 0)
            generated = copy.with_suffix(".md").read_bytes()
            self.assertIn(b"(copied%20measurements.json)", generated)
            copy.with_suffix(".md").write_bytes(generated.replace(b"\n", b"\r\n"))
            self.assertEqual(main(["report", "--from", str(copy), "--check"]), 0)
            copy.with_suffix(".md").write_bytes(b"Edited")
            with self.assertRaises(SystemExit):
                main(["report", "--from", str(copy), "--check"])
            self.assertEqual(copy.with_suffix(".md").read_bytes(), b"Edited")
            self.assertEqual(main(["report", "--from", str(copy)]), 0)
            self.assertEqual(copy.with_suffix(".md").read_bytes(), generated)
            self.assertEqual(copy.read_bytes(), original)

    def test_regeneration_can_update_existing_report_when_atomic_replace_is_denied(self):
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()):
            receipt = Path(tmp) / "measurement.json"
            receipt.write_bytes(next(REPORTS.glob("*.json")).read_bytes())
            original = receipt.with_suffix(".md")
            original.write_bytes(b"old generated report")
            with patch("merman_bench.report.os.replace", side_effect=PermissionError("reader holds a file handle")):
                self.assertEqual(main(["report", "--from", str(receipt)]), 0)
            self.assertIn(b"(measurement.json)", original.read_bytes())
            self.assertEqual(main(["report", "--from", str(receipt), "--check"]), 0)

    def test_without_independent_samples_coverage_publishes_no_timing(self):
        source = example_receipt()
        only = {"node_coverage": source["measurements"]["node_coverage"]}
        draft = render({**source, "measurements": only, "note": "local <preview> [abc](ref)", "raw_url": None}, "reader.json")
        self.assertIn(b"No independent Node timing rounds", draft)
        self.assertIn(b"No browser corpus was included", draft)
        self.assertIn(b"&lt;preview&gt;", draft)
        self.assertNotIn(b"[abc](ref)", draft)
        self.assertNotIn(b"Peer / Merman (range)", draft)
        self.assertIn(b"| Both completed | Both repeatable |", draft)
        self.assertIn(b"no Node startup or peak RSS measurements", draft)
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            render({**source, "raw_url": "https://user:password@example.org/archive"}, "reader.json")

    def test_node_uses_paired_ratios_and_explains_split_and_tied_rounds(self):
        rounds = example_receipt()["measurements"]["node_rounds"]
        checked = dict(planned=2, passed_pairs=8)
        rounds["latency"] = [
            timing_row("split", [(1, 8), (4, 2)] * 4, distinct_sources=1, visual_checks=checked),
            timing_row("one-tie", [(1, 2)] * 7 + [(1, 1)], visual_checks=checked),
            timing_row("all-tied", [(1, 1)] * 8, visual_checks=checked),
        ]
        page = page_for(node_rounds=rounds)
        self.assertIn("| split | Pele | 3 / 1 | 2.500 | 5.000 | 4.250 (0.500–8.000) | "
                      "Uncertain. Pele faster in 4/8 rounds, Merman in 4/8.", page)
        self.assertIn("Uncertain. Pele faster in 0/8 rounds, Merman in 7/8, 1 tied.", page)
        self.assertIn("No direction. All 8 pairs tied.", page)
        self.assertIn("0 consistent with verified drawing properties", page)
        self.assertIn("3 with mixed or tied rounds", page)

    def test_node_requires_complete_pairs_and_passing_drawing_checks_for_a_qualified_direction(self):
        source = example_receipt()
        rounds = source["measurements"]["node_rounds"]
        passed = dict(planned=2, passed_pairs=8)
        rounds["latency"] = [
            timing_row("audited-peer", [(2, 1)] * 8, visual_checks=passed),
            timing_row("audited-merman", [(1, 2)] * 8, visual_checks=passed),
            timing_row("labels", [(2, 1)] * 8, visual_checks=dict(planned=0, passed_pairs=0)),
            timing_row("legacy", [(2, 1)] * 8),
            timing_row("incomplete-audit", [(2, 1)] * 8, visual_checks=dict(planned=2, passed_pairs=7)),
            timing_row("partial", [(2, 1)] * 7, visual_checks=passed, reasons=["missing-evidence"]),
            timing_row("missing", [], reasons=["missing-product"]),
        ]
        rounds.pop("startup", None)
        note = "A separate visual spot check covered six SVGs."
        page = render({**source, "note": note, "measurements": {"node_coverage": source["measurements"]["node_coverage"],
                                                               "node_rounds": rounds}}, "reader.json").decode()
        self.assertIn(note, page)
        for faster in ("Pele", "Merman"):
            self.assertIn(f"{faster} faster in 8/8 rounds with the declared drawing properties verified. "
                          "| Checks declared for 2/3 requests. Both passed all declared properties in 8/8 pairs.", page)
        for evidence in ("SVG and any declared labels only", "Drawing checks unreported for this row",
                         "Checks declared for 2/3 requests. Both passed all declared properties in 7/8 pairs."):
            self.assertIn(f"Timing only. Pele faster in 8/8 rounds. | {evidence}", page)
        self.assertIn("| legacy | Pele | 3 / — |", page)
        self.assertIn("| partial | Pele | 3 / — | — | — | — | Insufficient data. 7/8 eligible pairs (missing-evidence).", page)
        self.assertIn("Insufficient data. 0/8 eligible pairs (missing-product).", page)
        self.assertIn("2 consistent with verified drawing properties, 3 consistent on timing alone, 2 with insufficient data", page)
        self.assertIn("no Node startup or peak RSS measurements", page)

    def test_node_startup_keeps_the_first_svg_when_a_later_request_is_an_expected_rejection(self):
        rounds = example_receipt()["measurements"]["node_rounds"]
        rounds["planned"]["scenarios"] = 1
        rounds["latency"] = [timing_row("editor-fixed-id", [], distinct_sources=3, reasons=["expected-rejection"],
                                       visual_checks=dict(planned=0, passed_pairs=0))]
        worker = dict(scenario="editor-fixed-id", engine="merman", workers=8, planned_workers=8, initialize_ms=6,
                      first_svg_ms=1.5, init_through_first_svg_ms=9.75, peak_rss_mib=120.25)
        rounds["startup"] = [worker, dict(worker, engine="pele", init_through_first_svg_ms=None, peak_rss_mib=None),
                             dict(worker, engine="mermaid-v13", workers=0, initialize_ms=None, first_svg_ms=None,
                                  init_through_first_svg_ms=None, peak_rss_mib=None)]
        page = page_for(node_rounds=rounds)
        self.assertIn("| editor-fixed-id | Pele | 3 / 3 | — | — | — | Warm ratio withheld. "
                      "The batch includes an expected rejection (0/8 eligible pairs).", page)
        self.assertIn("1 scenario/peer comparison in the 1-scenario plan", page)
        self.assertIn("1 withheld for expected rejections", page)
        self.assertIn("| editor-fixed-id | Merman | 8/8 | 6.000 | 1.500 | 9.750 | 120.250 |", page)
        self.assertIn("| editor-fixed-id | Pele | 8/8 | 6.000 | 1.500 | — | — |", page)
        self.assertIn("| editor-fixed-id | Mermaid v13 | 0/8 | — | — | — | — |", page)
        self.assertIn("including warm batches and output validation", page)

    def test_browser_prints_only_recorded_intervals_and_owner_ratios(self):
        browser = example_receipt()["measurements"]["browser"]
        for row in browser["rows"]:
            row["warm"].pop("budgeted_svg_ms", None)
        self.assertIn("no warm SVG byte-budget timings", page_for(browser=browser))
        first, second = browser["rows"][:2]
        first["warm"]["budgeted_svg_ms"] = dict(left=2, right=6, right_over_left=3)
        first["warm"].update(median_ms=dict(left=16, right=24, right_over_left=None), ratio_withheld_reason="Withheld")
        page = page_for(browser=browser)
        self.assertIn(f"| {first['id']} | warm | 16.000 | 24.000 | — |", page)
        self.assertIn(f"| {first['id']} | 2.000 | 6.000 | 3.000 |", page)
        self.assertIn(f"| {second['id']} | — | — | — |", page)
        self.assertIn("requestAnimationFrame", page)
        self.assertIn("DOM layout", page)
        first["warm"]["budgeted_svg_ms"].update(right=None, right_over_left=None)
        self.assertIn(f"| {first['id']} | 2.000 | — | — |", page_for(browser=browser))

    def test_cli_coverage_counts_distinct_sources_and_names_the_settings(self):
        export = next(lane for lane in example_receipt()["measurements"]["cli"] if lane["format"] == "svg")
        row = export["rows"][0]
        request = row["requests"][0]
        missing = dict(request, source_sha256="b" * 64, right=dict(state="missing", statuses=["timeout"]))
        row.update(id="repeat", requests=[request, request, missing], planned_requests=3, common_stable=2,
                   stable_coverage=dict(requests=2, distinct_sources=1), complete_batch_ms=None,
                   complete_batch_withheld_reason="Incomplete batch")
        export["rows"] = [row]
        export["coverage"].update(planned_requests=26, reported_requests=3, common_stable=2, planned_distinct_sources=25,
                                  planned_scenarios=5, reported_scenarios=1)
        page = page_for(cli=[export])
        self.assertIn("| repeat | 2/3 | 1/2 | renderer defaults | — | — | — |", page)
        self.assertIn("Incomplete batch", page)
        self.assertIn("SVG (25 distinct sources planned,", page)
        self.assertIn("2/26 requests in a 5-scenario plan with 25 distinct sources", page)
        self.assertIn("CLI PNG exports", page)

    def test_cli_note_stays_with_the_results_on_collection_and_replay(self):
        export = example_receipt()["measurements"]["cli"][0]
        note = ("Published builds.\r\n\r\nCLI: The controlled PNG settings are in a separate receipt.\r\n\r\n"
                "Host details accompany the run.\r\n\r\nCLI: Both CLIs process a whole Markdown document in one process\r\n"
                "and write updated Markdown plus one <svg> per diagram.")
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()):
            page = Path(tmp) / "report.md"
            with patch("merman_bench.report_surfaces.load_cli", return_value=export):
                self.assertEqual(main(["report", "--cli", "plan", "left", "right", "--date", "2026-11-23",
                                       "--note", note, "--out", str(page)]), 0)
            receipt = page.with_suffix(".json")
            self.assertEqual(json.loads(receipt.read_bytes())["note"], note)
            self.assertEqual(main(["report", "--from", str(receipt), "--check"]), 0)
            markdown = page.read_text(encoding="utf-8")
        overview = markdown.partition("## Node rendering coverage")[0]
        cli = markdown.split("## Command-line exports", 1)[1].partition("## Browser rendering")[0]
        self.assertIn("Published builds.\n\nHost details accompany the run.", overview)
        self.assertNotIn("controlled PNG", overview)
        self.assertTrue(cli.rstrip().endswith("**CLI note.** The controlled PNG settings are in a separate receipt.\n\n"
                        "Both CLIs process a whole Markdown document in one process and write updated Markdown "
                        "plus one &lt;svg&gt; per diagram."))

    def test_rust_identifies_the_local_stress_probe_and_links_downloadable_evidence(self):
        source = example_receipt()
        rust = source["measurements"]["rust_work"]
        revision = rust["products"]["merman"]["revision"]
        raw_url = "https://raw.githubusercontent.com/Latias94/merman/" + "a" * 40 + "/audit.json"
        page = render({**source, "note": None, "measurements": {"rust_work": rust}, "raw_url": raw_url}, "local.json").decode()
        self.assertIn(f"local Rust runner (recorded revision `{revision}`)", page)
        self.assertIn(f"{rust['coverage']['planned_topologies']} planned Flowchart stress controls", page)
        self.assertIn(f"[Download the raw audit data](<{raw_url}>)", page)
        self.assertIn("(local.json)", page)
        rust["products"]["merman"]["revision"] = None
        self.assertIn("local Rust runner (revision unreported)", page_for(rust_work=rust))

    def test_native_stage_layout_displays_realized_backends_when_they_differ(self):
        row = dict(layout="shoal", status="ok", stages=[dict(stage="layout", elapsed_ns=1_500_000)])
        page = page_for(rust_stages=dict(rows=[
            dict(row, scenario="changed", backends=["native"]),
            dict(row, scenario="mixed", backends=["shoal", "native"]),
            dict(row, scenario="matched", backends=["shoal"]),
            dict(row, scenario="historical"),
            dict(row, scenario="unobserved", backends=[], stages=[], status="unavailable"),
        ]))
        for name, layout in (("changed", "shoal → native"), ("mixed", "shoal → shoal, native"),
                             ("matched", "shoal"), ("historical", "shoal")):
            self.assertIn(f"| {name} | {layout} | layout | 1.500 |", page)
        self.assertIn("| unobserved | shoal | unavailable | — |", page)

    def test_native_stage_reader_requires_one_collection_identity(self):
        clock = {phase: dict(elapsed_ns=0, calls=0) for phase in STAGE_PHASES}
        clock["end_to_end"] = dict(elapsed_ns=2, calls=1)
        counters = dict(allocations=0, allocated_bytes=0, peak_growth_bytes=0)
        memory = {phase: counters for phase in STAGE_PHASES | {"unattributed", "end_to_end"}}
        row = dict(samples=[dict(elapsed_ns=2, unattributed_ns=2, stages=clock)],
                   memory_samples=[dict(counters, stages=memory)])
        options = dict(metric="stages", operation="svg-data", site_layout="dagre",
                       resource_profile="interactive", samples=1, with_memory=True)
        dagre = dict(options=options, plan_sha256="a" * 64, runner_sha256="b" * 64,
                     memory_runner_sha256="c" * 64, harness_sha256="d" * 64,
                     revision="e" * 40, host=["Windows", "AMD64", "local-host"],
                     rows={"small-svg": row})
        elk = dict(dagre, options=dict(options, site_layout="elk"))
        report = dict(schema_version=1, surface="rust-stages", layouts=dict(dagre=dagre, elk=elk))
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "stages.json"

            def read(payload):
                path.write_text(json.dumps(payload), encoding="utf-8")
                return load_stages(path)

            accepted = read(report)
            self.assertEqual([row["layout"] for row in accepted["rows"]], ["dagre", "elk"])
            self.assertNotIn("local-host", json.dumps(accepted))
            clock_only = deepcopy(report)
            for observation in clock_only["layouts"].values():
                del observation["options"]["with_memory"]
                del observation["memory_runner_sha256"]
                observation["revision"] = None
                observation["rows"] = {"small-svg": dict(samples=row["samples"])}
            self.assertEqual(read(clock_only)["runners"], {
                layout: {"runner_sha256": "b" * 64} for layout in ("dagre", "elk")
            })
            for field, value in (
                ("runner_sha256", "f" * 64),
                ("memory_runner_sha256", "f" * 64),
                ("harness_sha256", "f" * 64),
                ("revision", None),
                ("host", ["Darwin", "arm64", "another-host"]),
                ("options", dict(elk["options"], resource_profile="constrained")),
                ("rows", {"fan-in-500": row}),
            ):
                with self.subTest(field=field):
                    different = deepcopy(report)
                    different["layouts"]["elk"][field] = value
                    with self.assertRaisesRegex(ValueError, "one collection identity"):
                        read(different)
            incomplete = deepcopy(report)
            del incomplete["layouts"]["elk"]["memory_runner_sha256"]
            with self.assertRaisesRegex(ValueError, "clock and allocation runner fingerprints"):
                read(incomplete)

    def test_refuses_to_publish_over_an_existing_dated_pair(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            source = example_receipt()
            data = {**source, "measurements": {"rust_work": source["measurements"]["rust_work"]}}
            parser = argparse.ArgumentParser()
            add_report_parser(parser.add_subparsers())
            args = parser.parse_args(["report", "--rust-work", "plan.json", "work.json", "--date", source["date"],
                                      "--out", str(directory / "report.md")])
            with patch("merman_bench.report.collect", return_value=data):
                generate_report(args)
                with self.assertRaises(FileExistsError):
                    generate_report(args)
            self.assertEqual(json.loads((directory / "report.json").read_bytes())["format"], FORMAT)


class PerfCommentContractsTest(unittest.TestCase):


    @staticmethod
    def _v2_report(outcome: str) -> dict[str, object]:
        exit_codes = {
            "diagnostic_advisory": 0,
            "confirmed_regression": 1,
            "inconclusive": 3,
            "contract_failure": 2,
        }
        return {
            "schema_version": 2,
            "summary": {
                "outcome": outcome,
                "exit_code": exit_codes[outcome],
                "comparable": 1,
            },
            "selection": {
                "suite": "canary",
                "group": "end_to_end",
                "groups": {"base": "end_to_end", "head": "end_to_end"},
            },
            "comparison": {
                "base_label": "base@abc123",
                "head_label": "head@def456",
            },
            "method": {
                "preset": "long",
                "evidence_mode": "diagnostic"
                if outcome == "diagnostic_advisory"
                else "confirmation",
                "relative_threshold_percent": 10.0,
                "absolute_threshold_ns": 50_000.0,
                "confidence_level": 0.95,
                "confidence_contract": {
                    "simultaneous_confidence_level": 0.95,
                    "component_confidence_level": 0.99375,
                    "family_size": 8,
                    "multiplicity_adjustment": "bonferroni",
                },
                "pair_count": 8,
                "required_pairs": 8,
            },
            "recipes": {
                "base": {"logical_operations": 1},
                "head": {"logical_operations": 1},
            },
            "rows": [
                {
                    "benchmark": "end_to_end/flowchart_medium",
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "outcome": outcome,
                    "base_ns": 1_000_000.0,
                    "head_ns": 1_120_000.0,
                    "bounds": {
                        "relative_percent": {
                            "estimate": 12.0,
                            "lower": 10.5,
                            "upper": 13.5,
                        },
                        "absolute_ns": {
                            "estimate": 120_000.0,
                            "lower": 105_000.0,
                            "upper": 135_000.0,
                        },
                    },
                }
            ],
        }

    def test_schema_v2_fails_closed_when_report_and_process_exit_disagree(self) -> None:
        report = self._v2_report("confirmed_regression")
        report["summary"]["exit_code"] = 0

        body = render_perf_comment.render_comment(
            report,
            run_url="https://example.test/run",
            artifact_name="perf-regression",
            process_exit_code=1,
        )

        self.assertIn("Status: `contract failure`", body)
        self.assertIn("exit", body.lower())
        self.assertNotIn("Status: `confirmed regression`", body)

    def test_joint_comment_rejects_missing_certificates_and_unknown_protocol(self) -> None:
        for invalid in ("missing-contract", "unknown-protocol", "unknown-admission"):
            with self.subTest(invalid=invalid):
                report = self._v2_report("confirmed_regression")
                report["method"]["confirmation_protocol"] = "joint_non_regression_v1"
                if invalid == "unknown-protocol":
                    report["method"]["confirmation_protocol"] = "joint_non_regression_v2"
                if invalid == "unknown-admission":
                    report["summary"]["candidate_decision"] = "unknown"
                body = render_perf_comment.render_comment(
                    report,
                    run_url="https://example.test/run",
                    artifact_name="perf-regression",
                    process_exit_code=1,
                )
                self.assertIn("Status: `contract failure`", body)
                self.assertNotIn("Status: `confirmed regression`", body)

    def test_comment_cli_accepts_signal_exit_and_still_writes_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            output_path = root / "comment.md"
            result = render_perf_comment.main(
                [
                    "--json",
                    str(root / "missing.json"),
                    "--out",
                    str(output_path),
                    "--run-url",
                    "https://example.test/run",
                    "--process-exit-code",
                    "137",
                ]
            )
            body = output_path.read_text(encoding="utf-8")

        self.assertEqual(result, 2)
        self.assertIn("report unavailable", body)

    def test_unknown_report_schema_is_explicitly_unsupported_and_never_passed(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "schema_version": 999,
                "summary": {
                    "gate_status": "pass",
                    "outcome": "confirmed_non_regression",
                },
            },
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn("unsupported report schema", body.lower())
        self.assertNotIn("Status: `passed", body)
        self.assertNotIn("Status: `confirmed non-regression`", body)
