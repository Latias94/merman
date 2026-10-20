"""Synthetic contracts for the preregistered joint non-regression protocol."""

from __future__ import annotations

import copy
import hashlib
import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import compare_self
from corpus_utils import load_corpus, resolve_lane_group
from perf_contract_test_support import CORPUS_PATH, preflight_receipt


class JointProtocolContractsTest(unittest.TestCase):
    @staticmethod
    def _sides(relative: int = 8, absolute: int = 8) -> dict:
        side = {
            "status": "stable", "complete_pairs": 8, "expected_pairs": 8,
            "checks": {f"{kind}_{metric}_within_margin": True
                       for kind in ("identity", "order") for metric in ("log", "absolute")},
            "required_pairs": {
                "relative": {"required_pairs": relative, "scheduled_pairs": min(relative, 32)},
                "absolute": {"required_pairs": absolute, "scheduled_pairs": min(absolute, 32)},
            },
        }
        return {name: copy.deepcopy(side) for name in ("base", "head")}

    def test_policy_uses_uncapped_counts_both_runners_minimum_and_relative_tie(self) -> None:
        for relative, absolute, selected, required in (
            (8, 8, "log_ratio", 8), (10, 8, "absolute_ns", 8),
            (8, 44, "log_ratio", 8), (44, 46, None, None),
        ):
            with self.subTest(relative=relative, absolute=absolute):
                result = compare_self._joint_calibration_policy(
                    self._sides(relative, absolute), max_pairs=32,
                )
                self.assertEqual(result["selected_metric"], selected)
                self.assertEqual(result["required_pairs"], required)
                self.assertEqual(result["metrics"]["absolute_ns"]["required_pairs"], absolute)
        sides = self._sides(8, 12)
        sides["head"]["required_pairs"]["relative"]["required_pairs"] = 16
        result = compare_self._joint_calibration_policy(sides, max_pairs=32)
        self.assertEqual(result["selected_metric"], "absolute_ns")
        self.assertEqual(result["metrics"]["log_ratio"]["required_pairs"], 16)

    def test_policy_requires_each_runner_identity_and_order_and_complete_aa(self) -> None:
        for side in ("base", "head"):
            for kind in ("identity", "order"):
                with self.subTest(side=side, kind=kind):
                    sides = self._sides()
                    sides[side]["checks"][f"{kind}_log_within_margin"] = False
                    result = compare_self._joint_calibration_policy(sides, max_pairs=32)
                    self.assertEqual(result["selected_metric"], "absolute_ns")
                    self.assertFalse(result["metrics"]["log_ratio"]["qualified"])
        sides = self._sides()
        sides["head"] = {"status": "contract_failure", "complete_pairs": 7, "expected_pairs": 8}
        self.assertFalse(compare_self._joint_calibration_policy(sides, max_pairs=32)["eligible"])

    @staticmethod
    def _bounds(relative: tuple[float, float], absolute: tuple[float, float]) -> dict:
        return {"log_ratio": dict(zip(("lower", "upper"), relative)),
                "absolute_ns": dict(zip(("lower", "upper"), absolute))}

    def _classify(self, policy: dict, relative=(0.11, 0.13), absolute=(1_100, 1_300),
                  pairs=8, fixed=8) -> str:
        return compare_self._classify_joint_confirmation(
            self._bounds(relative, absolute), policy=policy, pair_count=pairs,
            fixed_pairs=fixed, relative_threshold=0.10, absolute_threshold_ns=1_000,
        )

    def test_only_preselected_upper_certifies_and_equal_boundary_is_nonregression(self) -> None:
        relative_policy = compare_self._joint_calibration_policy(self._sides(), max_pairs=32)
        self.assertEqual(self._classify(relative_policy, relative=(0.08, 0.10)), "confirmed_non_regression")
        self.assertEqual(self._classify(relative_policy, absolute=(800, 1_000)), "inconclusive")
        absolute_policy = compare_self._joint_calibration_policy(self._sides(10, 8), max_pairs=32)
        self.assertEqual(self._classify(absolute_policy, absolute=(800, 1_000)), "confirmed_non_regression")
        self.assertEqual(self._classify(absolute_policy, relative=(0.08, 0.10)), "inconclusive")

    def test_regression_needs_both_qualified_metrics_strict_bounds_and_both_counts(self) -> None:
        policy = compare_self._joint_calibration_policy(self._sides(), max_pairs=32)
        self.assertEqual(self._classify(policy), "confirmed_regression")
        self.assertEqual(self._classify(policy, relative=(0.10, 0.13)), "inconclusive")
        self.assertEqual(self._classify(policy, absolute=(1_000, 1_300)), "inconclusive")
        policy = compare_self._joint_calibration_policy(self._sides(8, 10), max_pairs=32)
        self.assertEqual(self._classify(policy), "inconclusive")
        self.assertEqual(self._classify(policy, pairs=10, fixed=10), "confirmed_regression")
        policy = compare_self._joint_calibration_policy(self._sides(8, 44), max_pairs=32)
        self.assertEqual(self._classify(policy), "inconclusive")

    def test_fixed_complete_even_count_is_required_even_for_favorable_bounds(self) -> None:
        policy = compare_self._joint_calibration_policy(self._sides(), max_pairs=32)
        for pairs, fixed in ((8, 10), (10, 8), (7, 7), (9, 9)):
            with self.subTest(pairs=pairs, fixed=fixed):
                self.assertEqual(self._classify(policy, relative=(0.0, 0.01), pairs=pairs, fixed=fixed), "inconclusive")

    @staticmethod
    def _contracts(count=2) -> list[dict]:
        contracts = []
        for index in range(count):
            name = f"fixture_{index}"
            receipt = preflight_receipt(f"end_to_end/{name}")
            contracts.append({
                "name": name, "family": "flowchart", "base_benchmark": f"end_to_end/{name}",
                "head_benchmark": f"end_to_end/{name}", "coverage_status": "comparable",
                "coverage_reasons": [], "output_identity": {"status": "matched", "base": receipt, "head": receipt},
            })
        return contracts

    def _run(self, root: Path, *, count=2, blocked=False, broken=None, extra=(),
             contracts=None, aa_error=False, tamper=False, fail_plan=False,
             second_count=None, projection_collision=False) -> tuple:
        contracts = contracts if contracts is not None else self._contracts(count)
        events = []
        reports = []
        lane = resolve_lane_group(load_corpus(CORPUS_PATH), "end_to_end")
        original_execute = compare_self._execute_comparison
        original_summarize = compare_self._summarize_calibration
        original_write_json = compare_self.write_json_report
        summary_index = 0

        def summarize(*args, **kwargs):
            nonlocal summary_index
            summary = original_summarize(*args, **kwargs)
            if second_count is not None and summary_index in (2, 3):
                summary["required_pairs"]["relative"]["required_pairs"] = second_count
            summary_index += 1
            return summary

        def execute(args, report):
            reports.append(report)
            return original_execute(args, report)

        def prepare(recipe, **kwargs):
            provenance = {"git": {"dirty": False}, "executable": {"sha256": recipe.label}}
            return compare_self.PreparedRunner(
                recipe, root / recipe.label, recipe.label, set(), {}, provenance, {},
            ), provenance, []

        def write_json(path, report):
            original_write_json(path, report)
            if projection_collision and not (root / "report.md").exists():
                os.link(root / "report.calibration-plan.json", root / "report.md")

        def aa(runner, *, pair_count, **kwargs):
            events.append("aa_" + runner.recipe.label)
            rows = {}
            for index, item in enumerate(contracts):
                base = 10_000.0 if blocked and index == len(contracts) - 1 else 1_000_000.0
                rows[item["name"]] = [{
                    "a": {"normalized_ns": base}, "b": {"normalized_ns": base + 2_000},
                    "first": {"normalized_ns": base}, "second": {"normalized_ns": base + 2_000},
                } for _ in range(pair_count)]
            errors = {contracts[0]["name"]: "runner failed"} if aa_error else {}
            return {"rows": rows, "rounds": [], "errors": errors}

        def ab(*, pair_count, **kwargs):
            events.append("ab")
            report = reports[0]
            contract = report["method"]["joint_non_regression_contract"]
            receipt = contract["calibration_plan"]
            raw = Path(receipt["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), receipt["sha256"])
            snapshot = json.loads(raw)
            self.assertEqual(len(snapshot["fixtures"]), len(contracts))
            self.assertIsNone(snapshot["method"]["joint_non_regression_contract"]["calibration_plan"])
            self.assertEqual(snapshot["method"]["joint_non_regression_contract"]["fixed_confirmation_pairs"], pair_count)
            self.assertEqual(len(snapshot["calibration"]["base"]["rows"]), len(contracts))
            rows = {item["name"]: [{
                "pair_index": index,
                "order": "AB" if (index % 2 == 0) == (kwargs["start_side"] == "base") else "BA",
                "base": {"normalized_ns": 1_000_000.0}, "head": {"normalized_ns": 1_050_000.0},
            } for index in range(pair_count)] for item in contracts}
            if broken == "prefix":
                rows[contracts[-1]["name"]] = rows[contracts[-1]["name"]][:-2]
            elif broken == "order":
                rows[contracts[-1]["name"]][1]["order"] = rows[contracts[-1]["name"]][0]["order"]
            elif broken == "index":
                rows[contracts[-1]["name"]][1]["pair_index"] = 0
            if tamper:
                Path(receipt["path"]).write_text("changed", encoding="utf-8")
            return {"pair_count": pair_count, "rounds": [], "rows": rows, "errors": {}}

        with (
            redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()),
            mock.patch.object(compare_self, "rustc_verbose", return_value="test rustc"),
            mock.patch.object(compare_self, "_execute_comparison", side_effect=execute),
            mock.patch.object(compare_self, "_load_fixture_contracts", return_value=(contracts, {}, (lane, lane))),
            mock.patch.object(compare_self, "_prepare_runner", side_effect=prepare),
            mock.patch.object(compare_self, "_binary_independence_error", return_value=None),
            mock.patch.object(compare_self, "_confirmation_harness_identity_errors", return_value=[]),
            mock.patch.object(compare_self, "_complete_fixture_contracts", return_value=contracts),
            mock.patch.object(compare_self, "_run_aa_schedule", side_effect=aa),
            mock.patch.object(compare_self, "_summarize_calibration", side_effect=summarize),
            mock.patch.object(compare_self, "_run_ab_schedule", side_effect=ab) as fresh,
            mock.patch.object(compare_self, "_verification_errors", return_value=[]),
            mock.patch.object(compare_self, "_fixture_verification_errors", return_value=[]),
            mock.patch.object(compare_self, "write_json_report", side_effect=write_json),
        ):
            args = ["--base-dir", str(root / "base"), "--head-dir", str(root / "head"),
                    "--evidence-mode", "confirmation", "--confirmation-protocol", compare_self.JOINT_PROTOCOL,
                    "--absolute-threshold-ns", "1000", "--out", str(root / "report.md"),
                    "--json-out", str(root / "report.json"), *extra]
            if fail_plan:
                with mock.patch.object(compare_self, "_write_joint_plan", side_effect=OSError("disk full")):
                    result = compare_self.main(args)
            else:
                result = compare_self.main(args)
        return result, reports[0] if reports else None, fresh.call_count, events

    def test_full_19_scope_plan_precedes_ab_and_metadata_does_not_claim_two_tail_coverage(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result, report, calls, events = self._run(root, count=19)
            self.assertEqual((result, calls), (0, 1))
            self.assertEqual(events, ["aa_base", "aa_head", "ab"])
            method = report["method"]
            self.assertEqual(method["decision_error_contract"]["family_size"], 38)
            self.assertAlmostEqual(method["decision_error_contract"]["component_tail_error_rate"], .05 / 38)
            self.assertNotIn("simultaneous_confidence_level", method["confidence_contract"])
            self.assertEqual(len(method["joint_non_regression_contract"]["rows"]), 19)
            self.assertEqual(report["summary"]["confirmed_improvements"], 0)
            for row in report["rows"]:
                self.assertEqual(row["outcome"], "confirmed_non_regression")
                self.assertIsNone(row["improvement_outcome"])
                self.assertNotIn("simultaneous_confidence_level", row["bounds"]["confidence_contract"])
            markdown = (root / "report.md").read_text()
            self.assertIn("Improvement is not assessed", markdown)
            self.assertNotIn("Simultaneous confidence:", markdown)
            self.assertIn("log_ratio", markdown)

    def test_one_ineligible_row_blocks_entire_cohort_without_shrinking_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            result, report, calls, _ = self._run(Path(temp), blocked=True, count=19)
            self.assertEqual((result, calls), (3, 0))
            self.assertEqual(len(report["rows"]), 19)
            self.assertTrue(all(row["outcome"] == "inconclusive" for row in report["rows"]))
            contract = report["method"]["joint_non_regression_contract"]
            self.assertIsNone(contract["fixed_confirmation_pairs"])
            self.assertIsNone(contract["rows"]["fixture_18"]["selected_metric"])
            self.assertIsNotNone(contract["calibration_plan"])
            self.assertEqual(report["method"]["decision_error_contract"]["family_size"], 38)

    def test_cohort_uses_largest_selected_count_and_preserves_head_first_balance(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            result, report, calls, _ = self._run(Path(temp), second_count=10, extra=("--start-side", "head"))
            self.assertEqual((result, calls), (0, 1))
            contract = report["method"]["joint_non_regression_contract"]
            self.assertEqual(contract["fixed_confirmation_pairs"], 10)
            self.assertEqual(contract["rows"]["fixture_0"]["required_pairs"], 8)
            self.assertEqual(contract["rows"]["fixture_1"]["required_pairs"], 10)
            self.assertTrue(all(row["pair_count"] == 10 for row in report["rows"]))

    def test_joint_default_is_one_microsecond_without_changing_generic_default(self) -> None:
        for protocol, expected in ((compare_self.JOINT_PROTOCOL, 1_000), ("generic_v2", 50_000)):
            args = SimpleNamespace(confirmation_protocol=protocol, absolute_threshold_ns=None,
                                   absolute_threshold_us=None)
            self.assertEqual(compare_self._absolute_threshold_ns(args), expected)

    def test_contract_failure_plan_failure_and_incomplete_schedule_revoke_all_rows(self) -> None:
        for options in ({"aa_error": True}, {"fail_plan": True}, {"tamper": True},
                        {"broken": "prefix"}, {"broken": "order"}, {"broken": "index"}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temp:
                result, report, calls, _ = self._run(Path(temp), **options)
                self.assertEqual(result, 2)
                if options.get("aa_error") or options.get("fail_plan"):
                    self.assertEqual(calls, 0)
                for row in report["rows"]:
                    self.assertEqual(row["outcome"], "contract_failure")
                    self.assertIsNone(row["bounds"])
                    self.assertIsNone(row["improvement_outcome"])

    def test_noncomparable_or_non_svg_scope_never_runs_aa_or_ab(self) -> None:
        for kind in ("coverage", "output"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temp:
                contracts = self._contracts()
                if kind == "coverage":
                    contracts[1]["coverage_status"] = "coverage_only"
                else:
                    contracts[1]["output_identity"]["base"]["output_kind"] = "plain_ascii"
                result, report, calls, events = self._run(Path(temp), contracts=contracts)
                self.assertEqual((result, calls, events), (2, 0, []))
                self.assertEqual(report["method"]["decision_error_contract"]["family_size"], 4)

    def test_invalid_modes_and_thresholds_fail_before_sampling(self) -> None:
        for extra in (("--allow-dirty",), ("--discovery-only",), ("--evidence-mode", "diagnostic"),
                      ("--low-latency-baseline-ns", "fixture_0=40000"),
                      ("--calibration-pairs", "10"),
                      ("--relative-threshold-percent", "11"), ("--absolute-threshold-ns", "1001")):
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as temp:
                result, _, calls, events = self._run(Path(temp), extra=extra)
                self.assertEqual((result, calls, events), (2, 0, []))

    def test_joint_confidence_is_exactly_point_95_before_any_sampling(self) -> None:
        for confidence in ("0.90", "0.99", "0.9500000000001"):
            with self.subTest(confidence=confidence), tempfile.TemporaryDirectory() as temp:
                result, _, calls, events = self._run(
                    Path(temp), extra=("--confidence-level", confidence),
                )
                self.assertEqual((result, calls, events), (2, 0, []))
                report = json.loads((Path(temp) / "report.json").read_text())
                self.assertIn("exactly 0.95", report["contract_errors"][0]["message"])
        with tempfile.TemporaryDirectory() as temp:
            result, report, calls, _ = self._run(
                Path(temp), extra=("--confidence-level", "0.95"),
            )
            self.assertEqual((result, calls), (0, 1))
            self.assertEqual(report["method"]["confidence_level"], 0.95)

    def test_sidecar_is_exclusive_and_cannot_collide_with_markdown_or_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / "report.calibration-plan.json"
            path.write_bytes(b"retain")
            result, _, calls, _ = self._run(root)
            self.assertEqual((result, calls), (2, 0))
            self.assertEqual(path.read_bytes(), b"retain")
        for via_link in (False, True):
            with self.subTest(via_link=via_link), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                plan = root / "report.calibration-plan.json"
                markdown = root / "alias.md"
                if via_link:
                    plan.write_bytes(b"retain")
                    os.link(plan, markdown)
                else:
                    markdown = plan
                result, _, calls, events = self._run(root, extra=("--out", str(markdown)))
                self.assertEqual((result, calls, events), (2, 0, []))
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            plan = root / "report.calibration-plan.json"
            with self.assertRaises(compare_self.ContractViolation):
                compare_self._guard_joint_plan_outputs(plan, root / "report.md", root / "report.json", str(plan))

    def test_dangling_sidecar_symlink_is_not_followed_and_final_projection_is_rechecked(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            plan = root / "report.calibration-plan.json"
            target = root / "missing.json"
            plan.symlink_to(target)
            result, _, calls, events = self._run(root)
            self.assertEqual((result, calls, events), (2, 0, []))
            self.assertTrue(plan.is_symlink())
            self.assertFalse(target.exists())
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result, report, _, _ = self._run(root, projection_collision=True)
            self.assertEqual(result, 2)
            plan = root / "report.calibration-plan.json"
            receipt = report["method"]["joint_non_regression_contract"]["calibration_plan"]
            self.assertEqual(hashlib.sha256(plan.read_bytes()).hexdigest(), receipt["sha256"])
            self.assertEqual(json.loads((root / "report.json").read_text())["summary"]["outcome"], "contract_failure")

    def test_generic_discovery_reuse_contract_does_not_require_protocol_equality(self) -> None:
        source = {"comparison": {}, "environment": {}, "method": {"confirmation_protocol": "generic_v2"}, "recipes": {}}
        report = {**source, "method": {"confirmation_protocol": compare_self.JOINT_PROTOCOL}}
        compare_self._validate_reuse_comparison_contract(source=source, report=report, recipes={})


if __name__ == "__main__":
    unittest.main()
