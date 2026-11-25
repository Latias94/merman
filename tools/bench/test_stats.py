#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import math
import unittest

from merman_bench import stats as stats


class CompareSelfStatisticsContractsTest(unittest.TestCase):
    @staticmethod
    def _bounds(
        *,
        relative: tuple[float, float, float],
        absolute: tuple[float, float, float],
    ) -> dict[str, dict[str, float]]:
        return {
            "log_ratio": dict(zip(("estimate", "lower", "upper"), relative)),
            "absolute_ns": dict(zip(("estimate", "lower", "upper"), absolute)),
        }

    def test_paired_bounds_use_canonical_head_over_base_direction_and_mirror_improvement(self) -> None:
        base_ns = [1_000_000.0, 2_000_000.0, 4_000_000.0, 8_000_000.0]
        head_ns = [1_200_000.0, 2_200_000.0, 4_200_000.0, 8_200_000.0]

        bounds = stats.paired_bounds(base_ns=base_ns, head_ns=head_ns)

        expected_log_ratio = sum(
            math.log(head / base) for base, head in zip(base_ns, head_ns)
        ) / len(base_ns)
        self.assertAlmostEqual(bounds["log_ratio"]["estimate"], expected_log_ratio)
        self.assertAlmostEqual(bounds["absolute_ns"]["estimate"], 200_000.0)
        self.assertGreater(bounds["log_ratio"]["estimate"], 0.0)
        self.assertGreater(bounds["absolute_ns"]["estimate"], 0.0)
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["estimate"],
            -bounds["log_ratio"]["estimate"],
        )
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["lower"],
            -bounds["log_ratio"]["upper"],
        )
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["upper"],
            -bounds["log_ratio"]["lower"],
        )
        self.assertAlmostEqual(
            bounds["improvement_absolute_ns"]["lower"],
            -bounds["absolute_ns"]["upper"],
        )
        self.assertAlmostEqual(
            bounds["improvement_absolute_ns"]["upper"],
            -bounds["absolute_ns"]["lower"],
        )

    def test_paired_bounds_adjust_both_metrics_for_the_confidence_family(self) -> None:
        samples = {
            "base_ns": [100.0, 101.0, 200.0, 205.0, 350.0, 600.0],
            "head_ns": [103.0, 124.0, 250.0, 260.0, 354.0, 400.0],
            "bootstrap_seed": 17,
            "bootstrap_resamples": 1_000,
        }
        two_metrics = stats.paired_bounds(**samples, family_size=2)
        eight_metrics = stats.paired_bounds(**samples, family_size=8)
        self.assertEqual(eight_metrics, stats.paired_bounds(**samples, family_size=8))
        self.assertEqual(eight_metrics["confidence_contract"], {
            "simultaneous_confidence_level": 0.95,
            "component_confidence_level": 0.99375,
            "family_size": 8,
            "multiplicity_adjustment": "bonferroni",
        })
        for metric in ("log_ratio", "absolute_ns"):
            with self.subTest(metric=metric):
                self.assertEqual(eight_metrics[metric]["estimate"], two_metrics[metric]["estimate"])
                self.assertLess(eight_metrics[metric]["lower"], two_metrics[metric]["lower"])
                self.assertGreater(eight_metrics[metric]["upper"], two_metrics[metric]["upper"])

    def test_aa_calibration_checks_equivalence_for_both_metrics_and_the_full_schedule(self) -> None:
        cases = (
            (1_000_000.0, 1_001_000.0, 8, "stable", None),
            (1_000_000.0, 1_002_000.0, 8, "inconclusive", "absolute"),
            (100.0, 112.0, 8, "inconclusive", "log"),
            (1_000_000.0, 1_001_000.0, 7, "contract_failure", None),
        )
        for ns_a, ns_b, count, status, rejected_metric in cases:
            with self.subTest(ns_a=ns_a, ns_b=ns_b, count=count):
                a = {"normalized_ns": ns_a}
                b = {"normalized_ns": ns_b}
                pairs = [{"a": a, "b": b, "first": a, "second": b} for _ in range(count)]
                calibration = stats.summarize_calibration(
                    pairs,
                    expected_pairs=8,
                    relative_mde=math.log1p(0.10),
                    absolute_mde_ns=1_000.0,
                    max_pairs=32,
                    confidence_level=0.95,
                    bootstrap_seed=7,
                    bootstrap_resamples=10_000,
                )
                self.assertEqual(calibration["status"], status)
                self.assertEqual(calibration["stable"], status == "stable")
                self.assertEqual(calibration["complete_pairs"], count)
                if rejected_metric is not None:
                    for kind in ("identity", "order"):
                        self.assertFalse(calibration["checks"][f"{kind}_{rejected_metric}_within_margin"])
                    self.assertTrue(calibration["checks"]["required_pairs_fit_cap"])

    def test_confirmation_distinguishes_regression_non_regression_and_inconclusive(self) -> None:
        relative_threshold = math.log1p(0.10)
        cases = [
            (
                "confirmed_regression",
                self._bounds(
                    relative=(0.12, 0.11, 0.13),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "confirmed_non_regression",
                self._bounds(
                    relative=(0.08, 0.07, 0.09),
                    absolute=(35_000.0, 25_000.0, 45_000.0),
                ),
            ),
            (
                "confirmed_non_regression",
                self._bounds(
                    relative=(0.08, 0.07, 0.09),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "confirmed_non_regression",
                self._bounds(
                    relative=(0.12, 0.11, 0.13),
                    absolute=(35_000.0, 25_000.0, 45_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(0.10, 0.08, 0.12),
                    absolute=(50_000.0, 40_000.0, 60_000.0),
                ),
            ),
        ]
        for expected, bounds in cases:
            with self.subTest(expected=expected):
                self.assertEqual(
                    stats.classify_confirmation(
                        bounds,
                        relative_threshold=relative_threshold,
                        absolute_threshold_ns=50_000.0,
                        direction="regression",
                        evidence_mode="confirmation",
                        pair_count=8,
                        required_pairs=8,
                    ),
                    expected,
                )

    def test_confirmation_supports_the_mirrored_improvement_test(self) -> None:
        bounds = self._bounds(
            relative=(-0.13, -0.15, -0.11),
            absolute=(-70_000.0, -80_000.0, -60_000.0),
        )

        self.assertEqual(
            stats.classify_confirmation(
                bounds,
                relative_threshold=math.log1p(0.10),
                absolute_threshold_ns=50_000.0,
                direction="improvement",
                evidence_mode="confirmation",
                pair_count=8,
                required_pairs=8,
            ),
            "confirmed_improvement",
        )

        threshold = math.log1p(0.10)
        cases = [
            (
                "confirmed_non_improvement",
                self._bounds(
                    relative=(-0.08, -0.09, -0.07),
                    absolute=(-35_000.0, -45_000.0, -25_000.0),
                ),
            ),
            (
                "confirmed_non_improvement",
                self._bounds(
                    relative=(-0.08, -0.09, -0.07),
                    absolute=(-65_000.0, -75_000.0, -55_000.0),
                ),
            ),
            (
                "confirmed_non_improvement",
                self._bounds(
                    relative=(-0.12, -0.13, -0.11),
                    absolute=(-35_000.0, -45_000.0, -25_000.0),
                ),
            ),
        ]
        for expected, case_bounds in cases:
            with self.subTest(expected=expected, bounds=case_bounds):
                self.assertEqual(
                    stats.classify_confirmation(
                        case_bounds,
                        relative_threshold=threshold,
                        absolute_threshold_ns=50_000.0,
                        direction="improvement",
                        evidence_mode="confirmation",
                        pair_count=8,
                        required_pairs=8,
                    ),
                    expected,
                )

    def test_joint_confirmation_boundaries_and_crossing_intervals(self) -> None:
        cases = [
            ("complement", (0.09, 0.08, 0.10), (1_300.0, 1_200.0, 1_400.0)),
            ("complement", (0.12, 0.11, 0.13), (950.0, 900.0, 1_000.0)),
            ("inconclusive", (0.11, 0.10, 0.12), (1_300.0, 1_200.0, 1_400.0)),
            ("inconclusive", (0.12, 0.11, 0.13), (1_100.0, 1_000.0, 1_200.0)),
            ("inconclusive", (0.12, 0.11, 0.13), (1_000.0, 900.0, 1_100.0)),
            ("inconclusive", (0.10, 0.09, 0.11), (1_200.0, 1_100.0, 1_300.0)),
        ]
        for direction in ("regression", "improvement"):
            for expected, relative, absolute in cases:
                with self.subTest(direction=direction, relative=relative, absolute=absolute):
                    if direction == "improvement":
                        relative = (-relative[0], -relative[2], -relative[1])
                        absolute = (-absolute[0], -absolute[2], -absolute[1])
                    outcome = stats.classify_confirmation(
                        self._bounds(relative=relative, absolute=absolute),
                        relative_threshold=0.10,
                        absolute_threshold_ns=1_000.0,
                        direction=direction,
                        evidence_mode="confirmation",
                        pair_count=8,
                        required_pairs=8,
                    )
                    if expected == "complement":
                        expected = f"confirmed_non_{direction}"
                    self.assertEqual(outcome, expected)

    def test_single_metric_complement_cannot_bypass_confirmation_pair_requirements(self) -> None:
        for direction in ("regression", "improvement"):
            relative = (0.08, 0.07, 0.09)
            absolute = (1_300.0, 1_200.0, 1_400.0)
            if direction == "improvement":
                relative = (-relative[0], -relative[2], -relative[1])
                absolute = (-absolute[0], -absolute[2], -absolute[1])
            for pair_count, required_pairs in ((7, 7), (8, 10)):
                with self.subTest(direction=direction, pairs=pair_count, required=required_pairs):
                    self.assertEqual(
                        stats.classify_confirmation(
                            self._bounds(relative=relative, absolute=absolute),
                            relative_threshold=0.10,
                            absolute_threshold_ns=1_000.0,
                            direction=direction,
                            evidence_mode="confirmation",
                            pair_count=pair_count,
                            required_pairs=required_pairs,
                        ),
                        "inconclusive",
                    )

    def test_power_derived_pair_count_has_floor_even_rounding_and_cap_signal(self) -> None:
        floor = stats.required_pair_count(
            sigma=0.0,
            minimum_detectable_effect=0.10,
            max_pairs=32,
        )
        rounded = stats.required_pair_count(
            sigma=0.02,
            minimum_detectable_effect=0.01,
            max_pairs=32,
        )
        over_cap = stats.required_pair_count(
            sigma=0.02,
            minimum_detectable_effect=0.01,
            max_pairs=24,
        )

        self.assertEqual(floor.required_pairs, 8)
        self.assertEqual(floor.scheduled_pairs, 8)
        self.assertFalse(floor.exceeds_cap)
        self.assertEqual(rounded.required_pairs, 26)
        self.assertEqual(rounded.required_pairs % 2, 0)
        self.assertFalse(rounded.exceeds_cap)
        self.assertEqual(over_cap.required_pairs, 26)
        self.assertEqual(over_cap.scheduled_pairs, 24)
        self.assertTrue(over_cap.exceeds_cap)

    @staticmethod
    def _low_latency_side(*, low: float = -100.0, high: float = 100.0) -> dict:
        return {
            "status": "stable", "complete_pairs": 8, "expected_pairs": 8,
            "identity_bounds": {
                "log_ratio": {"lower": -0.002, "upper": 0.002},
                "absolute_ns": {"lower": low, "upper": high},
            },
            "order_effect_bounds": {
                "log_ratio": {"lower": -0.001, "upper": 0.001},
                "absolute_ns": {"lower": low, "upper": high},
            },
            "sigma": {"log_ratio": 0.001, "absolute_ns": 50.0},
        }

    def test_low_latency_thresholds_use_frozen_baseline_and_both_aa_sides(self) -> None:
        sides = {side: self._low_latency_side() for side in ("base", "head")}
        sides["head"]["order_effect_bounds"]["log_ratio"] = {
            "lower": -0.04, "upper": 0.03,
        }
        result = stats.low_latency_calibration(
            baseline_ns=40_000.0, side_summaries=sides, max_pairs=32,
        )
        self.assertTrue(result["stable"])
        self.assertAlmostEqual(result["noise"]["log_ratio"], 0.04)
        self.assertAlmostEqual(result["thresholds"]["log_ratio"], 0.12)
        self.assertAlmostEqual(
            result["thresholds"]["absolute_ns"], 40_000 * (1 - math.exp(-0.12)),
        )
        self.assertEqual(result["required_pairs"], 8)

    def test_low_latency_rejects_biased_aa_even_inside_generic_equivalence_margin(self) -> None:
        result = stats.low_latency_calibration(
            baseline_ns=40_000.0,
            side_summaries={"base": self._low_latency_side(low=10.0),
                            "head": self._low_latency_side()},
            max_pairs=32,
        )
        self.assertFalse(result["stable"])
        self.assertIn("base_identity_absolute_ns_contains_zero", result["reasons"])

    def test_low_latency_rejects_noise_and_power_over_preregistered_cap(self) -> None:
        sides = {side: self._low_latency_side(low=-1_001, high=1_001)
                 for side in ("base", "head")}
        sides["head"]["sigma"]["absolute_ns"] = 100_000.0
        result = stats.low_latency_calibration(
            baseline_ns=40_000.0, side_summaries=sides, max_pairs=8,
        )
        self.assertFalse(result["stable"])
        self.assertIn("absolute_noise_within_limit", result["reasons"])
        self.assertIn("required_pairs_fit_cap", result["reasons"])
        self.assertGreater(result["required_pairs"], 8)

    def test_low_latency_does_not_admit_incomplete_aa(self) -> None:
        sides = {side: self._low_latency_side() for side in ("base", "head")}
        sides["head"] = {"status": "contract_failure", "stable": False,
                         "complete_pairs": 7, "expected_pairs": 8}
        result = stats.low_latency_calibration(
            baseline_ns=40_000.0, side_summaries=sides, max_pairs=32,
        )
        self.assertFalse(result["stable"])
        self.assertIn("head_complete_aa_schedule", result["reasons"])

    def test_diagnostic_timing_is_advisory_even_when_bounds_look_regressive(self) -> None:
        bounds = self._bounds(
            relative=(0.15, 0.14, 0.16),
            absolute=(80_000.0, 70_000.0, 90_000.0),
        )

        outcome = stats.classify_confirmation(
            bounds,
            relative_threshold=math.log1p(0.10),
            absolute_threshold_ns=50_000.0,
            direction="regression",
            evidence_mode="diagnostic",
            pair_count=4,
            required_pairs=8,
        )

        self.assertEqual(outcome, "diagnostic_advisory")
