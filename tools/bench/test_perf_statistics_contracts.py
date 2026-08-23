#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import math
import sys
import unittest
from pathlib import Path

import compare_self


ROOT = Path(__file__).resolve().parents[2]


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

        bounds = compare_self.paired_bounds(base_ns=base_ns, head_ns=head_ns)

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
                "inconclusive",
                self._bounds(
                    relative=(0.08, 0.07, 0.09),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "inconclusive",
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
                    compare_self.classify_confirmation(
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
            compare_self.classify_confirmation(
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
                "inconclusive",
                self._bounds(
                    relative=(-0.08, -0.09, -0.07),
                    absolute=(-65_000.0, -75_000.0, -55_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(-0.12, -0.13, -0.11),
                    absolute=(-35_000.0, -45_000.0, -25_000.0),
                ),
            ),
        ]
        for expected, case_bounds in cases:
            with self.subTest(expected=expected, bounds=case_bounds):
                self.assertEqual(
                    compare_self.classify_confirmation(
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

    def test_power_derived_pair_count_has_floor_even_rounding_and_cap_signal(self) -> None:
        floor = compare_self.required_pair_count(
            sigma=0.0,
            minimum_detectable_effect=0.10,
            max_pairs=32,
        )
        rounded = compare_self.required_pair_count(
            sigma=0.02,
            minimum_detectable_effect=0.01,
            max_pairs=32,
        )
        over_cap = compare_self.required_pair_count(
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

    def test_suite_exit_code_uses_contract_regression_inconclusive_priority(self) -> None:
        cases = [
            (["diagnostic_advisory"], 0),
            (["confirmed_non_regression"], 0),
            (["inconclusive", "diagnostic_advisory"], 3),
            (["confirmed_regression", "inconclusive"], 1),
            (["contract_failure", "confirmed_regression", "inconclusive"], 2),
        ]
        for outcomes, expected in cases:
            with self.subTest(outcomes=outcomes):
                self.assertEqual(compare_self.suite_exit_code(outcomes), expected)

    def test_diagnostic_timing_is_advisory_even_when_bounds_look_regressive(self) -> None:
        bounds = self._bounds(
            relative=(0.15, 0.14, 0.16),
            absolute=(80_000.0, 70_000.0, 90_000.0),
        )

        outcome = compare_self.classify_confirmation(
            bounds,
            relative_threshold=math.log1p(0.10),
            absolute_threshold_ns=50_000.0,
            direction="regression",
            evidence_mode="diagnostic",
            pair_count=4,
            required_pairs=8,
        )

        self.assertEqual(outcome, "diagnostic_advisory")
        self.assertEqual(compare_self.suite_exit_code([outcome]), 0)

if __name__ == "__main__":
    sys.exit(unittest.main())
