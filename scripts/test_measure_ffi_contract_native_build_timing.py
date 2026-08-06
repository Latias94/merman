#!/usr/bin/env python3
"""Tests for matched native FFI clean-build timing evidence."""

from __future__ import annotations

from copy import deepcopy
from datetime import datetime, timedelta, timezone
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from artifact_profile_recipe import REPO_ROOT  # noqa: E402
from ffi_contract_dependency_probes import BASELINE_COMMIT, BASELINE_TREE  # noqa: E402
from measure_ffi_contract_native_build_timing import (  # noqa: E402
    NativeBuildTimingError,
    REPORT_ID,
    REVIEW_THRESHOLD_RATIO,
    SCHEMA_VERSION,
    _validate_candidate_ancestry,
    _extract_git_archive,
    _measure_revision,
    embedded_report_sha256,
    load_report,
    timing_statistics,
    validate_report,
)


class NativeBuildTimingStatisticsTests(unittest.TestCase):
    def test_regression_must_exceed_threshold_and_observed_noise(self) -> None:
        low_noise = self.samples((100, 101, 99), (111, 112, 110))
        self.assertTrue(timing_statistics(low_noise)["review_required"])

        high_noise = self.samples((80, 100, 120), (100, 111, 122))
        statistics = timing_statistics(high_noise)
        self.assertGreater(statistics["median_regression_ratio"], REVIEW_THRESHOLD_RATIO)
        self.assertFalse(statistics["review_required"])

    def test_candidate_improvement_never_requires_review(self) -> None:
        statistics = timing_statistics(
            self.samples((100, 101, 99), (80, 81, 79))
        )
        self.assertLess(statistics["median_regression_ratio"], 0)
        self.assertFalse(statistics["review_required"])

    @staticmethod
    def samples(
        baseline: tuple[int, int, int],
        candidate: tuple[int, int, int],
    ) -> list[dict[str, object]]:
        return [
            {
                "pair_index": index,
                "order": ["baseline", "candidate"]
                if index % 2 == 1
                else ["candidate", "baseline"],
                "baseline_duration_ns": baseline[index - 1],
                "candidate_duration_ns": candidate[index - 1],
            }
            for index in range(1, 4)
        ]


class RevisionMeasurementCleanupTests(unittest.TestCase):
    def test_sample_output_root_is_removed_after_measurement_is_extracted(
        self,
    ) -> None:
        profile = mock.Mock(label="ffi-full-native")
        runner = mock.Mock()
        runner.single_cargo_build_duration_ns.return_value = 123
        profile_projection = {"label": "ffi-full-native"}
        build_projection = {"command": ["cargo", "build"]}

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source"
            source_root.mkdir()
            output_root = root / "sample-output"
            sibling = root / "keep.txt"
            sibling.write_text("keep\n", encoding="utf-8")

            def capture(*args: object, **kwargs: object) -> tuple[dict[str, object], ...]:
                self.assertEqual(args, ((profile,),))
                self.assertEqual(kwargs["repo_root"], source_root)
                self.assertEqual(kwargs["output_root"], output_root)
                self.assertEqual(kwargs["rust_toolchain"], {"fixture": True})
                self.assertIs(kwargs["runner"], runner)
                artifact = output_root / "build" / "ffi-full-native" / "artifact.a"
                artifact.parent.mkdir(parents=True)
                artifact.write_bytes(b"large build output")
                return (
                    {
                        "profile": profile_projection,
                        "build": build_projection,
                    },
                )

            with (
                mock.patch(
                    "measure_ffi_contract_native_build_timing.load_native_artifact_profiles",
                    return_value=(profile,),
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.capture_native_artifact_measurements",
                    side_effect=capture,
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.TimedProcessRunner",
                    return_value=runner,
                ),
            ):
                measurement = _measure_revision(
                    source_root,
                    output_root,
                    {"fixture": True},
                )

            self.assertEqual(measurement.duration_ns, 123)
            self.assertEqual(measurement.profile, profile_projection)
            self.assertEqual(measurement.build, build_projection)
            self.assertFalse(output_root.exists())
            self.assertEqual(sibling.read_text(encoding="utf-8"), "keep\n")

    def test_sample_output_root_is_removed_when_measurement_raises(self) -> None:
        profile = mock.Mock(label="ffi-full-native")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source"
            source_root.mkdir()
            output_root = root / "sample-output"

            def capture(*args: object, **kwargs: object) -> tuple[dict[str, object], ...]:
                del args
                self.assertEqual(kwargs["output_root"], output_root)
                output_root.mkdir(parents=True)
                (output_root / "partial-build").write_bytes(b"partial")
                raise RuntimeError("measurement failed")

            with (
                mock.patch(
                    "measure_ffi_contract_native_build_timing.load_native_artifact_profiles",
                    return_value=(profile,),
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.capture_native_artifact_measurements",
                    side_effect=capture,
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "measurement failed"):
                    _measure_revision(source_root, output_root, {"fixture": True})

            self.assertFalse(output_root.exists())

    def test_sample_fails_closed_when_output_root_cannot_be_removed(self) -> None:
        profile = mock.Mock(label="ffi-full-native")
        runner = mock.Mock()
        runner.single_cargo_build_duration_ns.return_value = 123
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source"
            source_root.mkdir()
            output_root = root / "sample-output"

            def capture(*args: object, **kwargs: object) -> tuple[dict[str, object], ...]:
                del args
                self.assertEqual(kwargs["output_root"], output_root)
                output_root.mkdir(parents=True)
                return ({"profile": {}, "build": {}},)

            with (
                mock.patch(
                    "measure_ffi_contract_native_build_timing.load_native_artifact_profiles",
                    return_value=(profile,),
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.capture_native_artifact_measurements",
                    side_effect=capture,
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.TimedProcessRunner",
                    return_value=runner,
                ),
                mock.patch(
                    "measure_ffi_contract_native_build_timing.shutil.rmtree",
                    side_effect=OSError("directory busy"),
                ) as remove,
            ):
                with self.assertRaisesRegex(
                    NativeBuildTimingError,
                    "cannot remove native timing sample output root",
                ):
                    _measure_revision(source_root, output_root, {"fixture": True})

            remove.assert_called_once_with(output_root)
            self.assertTrue(output_root.is_dir())


class NativeBuildTimingReportTests(unittest.TestCase):
    def test_valid_report_round_trips_from_a_regular_file(self) -> None:
        report = self.report()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "timing.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            self.assertEqual(load_report(path), report)

    def test_stale_statistics_are_rejected(self) -> None:
        report = self.report()
        report["statistics"]["candidate_median_ns"] += 1
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "statistic"):
            validate_report(report)

    def test_over_threshold_report_requires_a_review(self) -> None:
        report = self.report(candidate=(112, 111, 113))
        self.assertTrue(report["statistics"]["review_required"])
        report["review"] = None
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "explicit review"):
            validate_report(report)

    def test_non_regressing_report_rejects_an_unneeded_review(self) -> None:
        report = self.report()
        report["review"] = {"accepted": True, "reason": "No review was needed."}
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "must not carry"):
            validate_report(report)

    def test_even_sample_count_is_rejected(self) -> None:
        report = self.report()
        report["measurement_boundary"]["sample_count_per_revision"] = 4
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "odd value"):
            validate_report(report)

    def test_candidate_cannot_reuse_the_baseline_commit_with_a_fake_tree(self) -> None:
        report = self.report()
        report["source_revisions"]["candidate"] = {
            "commit": BASELINE_COMMIT,
            "tree": "f" * 40,
        }
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "differ from the baseline"):
            validate_report(report)

    def test_captured_at_utc_rejects_a_non_utc_offset(self) -> None:
        report = self.report()
        report["captured_at_utc"] = datetime.now(
            timezone(timedelta(hours=8))
        ).isoformat()
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "must use UTC"):
            validate_report(report)

    def test_repository_validation_binds_head_tree_toolchain_and_recipe(self) -> None:
        report = self.repository_report()
        self.assertEqual(
            validate_report(report, repo_root=REPO_ROOT),
            report,
        )

    def test_repository_validation_rejects_a_rehashed_recipe_drift(self) -> None:
        report = self.repository_report()
        report["measurement_boundary"]["profile"]["cargo"]["features"].append(
            "forged-feature"
        )
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(
            NativeBuildTimingError,
            "descriptor-owned artifact recipe",
        ):
            validate_report(report, repo_root=REPO_ROOT)

    def test_repository_validation_rejects_a_rehashed_candidate_tree(self) -> None:
        report = self.repository_report()
        report["source_revisions"]["candidate"]["tree"] = "f" * 40
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "does not match"):
            validate_report(report, repo_root=REPO_ROOT)

    def test_repository_validation_rejects_machine_target_drift(self) -> None:
        report = self.repository_report()
        report["machine"]["system"] = "Linux"
        report["machine"]["machine"] = "x86_64"
        report["report_sha256"] = embedded_report_sha256(report)
        with self.assertRaisesRegex(NativeBuildTimingError, "Apple Darwin"):
            validate_report(report, repo_root=REPO_ROOT)

    def report(
        self,
        *,
        candidate: tuple[int, int, int] = (99, 100, 98),
    ) -> dict[str, object]:
        samples = NativeBuildTimingStatisticsTests.samples(
            (100, 101, 99),
            candidate,
        )
        statistics = timing_statistics(samples)
        review = (
            {"accepted": True, "reason": "Reviewed matched clean-build regression."}
            if statistics["review_required"]
            else None
        )
        report: dict[str, object] = {
            "schema_version": SCHEMA_VERSION,
            "report_id": REPORT_ID,
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "source_revisions": {
                "baseline": {
                    "commit": BASELINE_COMMIT,
                    "tree": BASELINE_TREE,
                },
                "candidate": {
                    "commit": "2" * 40,
                    "tree": "3" * 40,
                },
            },
            "toolchain": {
                "cargo": {
                    "path": "/toolchain/bin/cargo",
                    "sha256": "sha256:" + "4" * 64,
                },
                "rustc": {
                    "path": "/toolchain/bin/rustc",
                    "sha256": "sha256:" + "5" * 64,
                },
                "cargo_version": "cargo 1.95.0",
                "rustc_verbose": "rustc 1.95.0\nhost: aarch64-apple-darwin",
                "host_target": "aarch64-apple-darwin",
            },
            "machine": {
                "system": "Darwin",
                "release": "fixture",
                "machine": "arm64",
                "processor": "Apple fixture",
                "logical_cpu_count": 12,
                "memory_bytes": 48 * 1024 * 1024 * 1024,
                "python_version": "3.12.0",
            },
            "measurement_boundary": {
                "metric": "clean-cargo-build-and-link-wall-nanoseconds",
                "clock": "time.perf_counter_ns",
                "profile": {
                    "label": "ffi-full-native",
                    "budget_class": "full",
                    "target": "aarch64-apple-darwin",
                    "cargo": {"fixture": True},
                },
                "build": {"fixture": True},
                "fresh_target_directory_per_sample": True,
                "paired_order": "alternating-baseline-candidate",
                "sample_count_per_revision": 3,
                "review_threshold_ratio": REVIEW_THRESHOLD_RATIO,
                "noise_floor_method": "max-relative-mad-to-median",
                "timing_is_gating": True,
            },
            "samples": samples,
            "statistics": statistics,
            "review": review,
        }
        report["report_sha256"] = embedded_report_sha256(report)
        return deepcopy(report)

    def repository_report(self) -> dict[str, object]:
        report = self.report()
        baseline = json.loads(
            (
                REPO_ROOT
                / "abi"
                / "ffi-contract-baseline"
                / "native-artifact-sizes.json"
            ).read_text(encoding="utf-8")
        )
        full_profile = next(
            profile
            for profile in baseline["profiles"]
            if profile["profile"]["label"] == "ffi-full-native"
        )
        commit = subprocess.check_output(
            ("git", "--no-replace-objects", "rev-parse", "HEAD^{commit}"),
            cwd=REPO_ROOT,
            text=True,
        ).strip()
        tree = subprocess.check_output(
            ("git", "--no-replace-objects", "rev-parse", "HEAD^{tree}"),
            cwd=REPO_ROOT,
            text=True,
        ).strip()
        report["source_revisions"]["candidate"] = {
            "commit": commit,
            "tree": tree,
        }
        report["toolchain"] = deepcopy(baseline["toolchain"])
        report["measurement_boundary"]["profile"] = deepcopy(
            full_profile["profile"]
        )
        report["measurement_boundary"]["build"] = deepcopy(full_profile["build"])
        report["report_sha256"] = embedded_report_sha256(report)
        return report


class GitArchiveExtractionTests(unittest.TestCase):
    def test_parent_escape_is_rejected(self) -> None:
        archive_bytes = io.BytesIO()
        with tarfile.open(fileobj=archive_bytes, mode="w") as archive:
            member = tarfile.TarInfo("../escape")
            member.size = 1
            archive.addfile(member, io.BytesIO(b"x"))
        archive_bytes.seek(0)
        with tempfile.TemporaryDirectory() as temporary:
            with tarfile.open(fileobj=archive_bytes, mode="r:") as archive:
                with self.assertRaisesRegex(NativeBuildTimingError, "escapes"):
                    _extract_git_archive(archive, Path(temporary))


class CandidateAncestryTests(unittest.TestCase):
    def test_post_capture_source_changes_do_not_reclassify_the_measured_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.git(root, "init")
            self.git(root, "config", "user.email", "timing@example.invalid")
            self.git(root, "config", "user.name", "Timing Test")
            source = root / "src" / "implementation.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn implementation() {}\n", encoding="utf-8")
            self.git(root, "add", "src/implementation.rs")
            self.git(root, "commit", "-m", "candidate")
            candidate = self.git(root, "rev-parse", "HEAD").stdout.strip()

            source.write_text("fn implementation() { changed(); }\n", encoding="utf-8")
            self.git(root, "add", "src/implementation.rs")
            self.git(root, "commit", "-m", "later source change")

            _validate_candidate_ancestry(root, candidate)

    def test_candidate_outside_checked_out_history_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.git(root, "init")
            self.git(root, "config", "user.email", "timing@example.invalid")
            self.git(root, "config", "user.name", "Timing Test")
            source = root / "source.txt"
            source.write_text("candidate\n", encoding="utf-8")
            self.git(root, "add", "source.txt")
            self.git(root, "commit", "-m", "candidate")
            candidate = self.git(root, "rev-parse", "HEAD").stdout.strip()

            self.git(root, "checkout", "--orphan", "unrelated")
            source.unlink()
            unrelated = root / "unrelated.txt"
            unrelated.write_text("unrelated\n", encoding="utf-8")
            self.git(root, "add", "-A")
            self.git(root, "commit", "-m", "unrelated")

            with self.assertRaisesRegex(
                NativeBuildTimingError,
                "not an ancestor",
            ):
                _validate_candidate_ancestry(root, candidate)

    @staticmethod
    def git(root: Path, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ("git", *args),
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )


if __name__ == "__main__":
    unittest.main()
