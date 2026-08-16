#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import copy
import json
import math
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import compare_self
from perf_contract_test_support import minimal_corpus, preflight_receipt


ROOT = Path(__file__).resolve().parents[2]
PIPELINE_EXECUTABLE = (
    "pipeline-deadbeef.exe" if os.name == "nt" else "pipeline-deadbeef"
)


class CompareSelfRecipeContractsTest(unittest.TestCase):
    def _recipe(
        self,
        *,
        label: str,
        checkout: Path,
        package: str,
        bench: str,
        features: tuple[str, ...],
        default_features: bool,
        toolchain: str | None,
        target_dir: Path,
        target: str | None = None,
        locked: bool = True,
        corpus: Path,
    ):
        return compare_self.RunnerRecipe(
            label=label,
            checkout=checkout,
            package=package,
            bench=bench,
            features=features,
            default_features=default_features,
            toolchain=toolchain,
            target_dir=target_dir,
            target=target,
            locked=locked,
            corpus=corpus,
        )

    @staticmethod
    def _minimal_reusable_discovery() -> dict[str, object]:
        benchmark = "end_to_end/flowchart_medium"
        receipt = {
            "schema_version": 1,
            "benchmark": benchmark,
            "output_kind": "svg",
            "output_bytes": 123,
            "output_sha256": "a" * 64,
            "svg_elements": 7,
        }
        runner = {
            "recipe": {},
            "git": {},
            "manifest": {},
            "workspace_manifest": {},
            "lockfile": {},
            "corpus": {
                "preflight_receipts_required": True,
                "preflight_contract": {
                    "path": "/tmp/native-criterion-preflight-v1.json",
                    "bytes": 100,
                    "sha256": "b" * 64,
                },
            },
            "bench_target": {},
            "bench_source": {},
            "toolchain": {},
            "build_environment": {},
            "shared_target_profile_reset": {},
            "prebuild_command": [],
            "prebuild_stderr_tail": "",
            "source_executable": {},
            "frozen_executable": {},
            "executable": {},
            "discovery_command": [],
            "discovery": {"preflight_receipts": {benchmark: receipt}},
            "post_sampling_verification": {"status": "verified"},
            "shared_target_freeze": {
                "enabled": True,
                "context": "reuse-test",
                "target_dir": "/tmp/target",
            },
        }
        return {
            "schema_version": 2,
            "harness": {
                "schema": "compare-self-v2",
                "path": "/tmp/compare_self.py",
                "bytes": 100,
                "sha256": "a" * 64,
            },
            "method": {
                "evidence_mode": "confirmation",
                "evidence_quality": "discovery_only",
                "discovery_only": True,
                "shared_target_freeze": {
                    "enabled": True,
                    "context": "reuse-test",
                    "target_dir": "/tmp/target",
                    "build_order": ["base", "head"],
                    "cargo_build_jobs": "1",
                    "profile_reset": "cargo-clean-bench-profile-before-each-side",
                },
            },
            "summary": {
                "exit_code": 0,
                "outcome": "diagnostic_advisory",
                "contract_failures": 0,
                "comparable": 1,
            },
            "contract_errors": [],
            "calibration": None,
            "raw_rounds": [],
            "fixtures": [
                {
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "coverage_status": "comparable",
                    "output_identity": {
                        "status": "matched",
                        "base": copy.deepcopy(receipt),
                        "head": copy.deepcopy(receipt),
                    },
                    "post_sampling_verification": {"status": "verified"},
                }
            ],
            "rows": [
                {
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "outcome": "diagnostic_advisory",
                    "output_identity": {
                        "status": "matched",
                        "base": copy.deepcopy(receipt),
                        "head": copy.deepcopy(receipt),
                    },
                }
            ],
            "runners": {
                "base": {
                    **copy.deepcopy(runner),
                    "shared_target_freeze": {
                        **runner["shared_target_freeze"],
                        "build_sequence": 1,
                    },
                },
                "head": {
                    **copy.deepcopy(runner),
                    "shared_target_freeze": {
                        **runner["shared_target_freeze"],
                        "build_sequence": 2,
                    },
                },
            },
        }

    def test_reusable_discovery_loader_rejects_digest_drift_and_invalid_json(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            duplicate = root / "duplicate.json"
            duplicate.write_text('{"schema_version": 2, "schema_version": 2}\n')
            nonfinite = root / "nonfinite.json"
            nonfinite.write_text('{"value": NaN}\n')

            with self.assertRaisesRegex(compare_self.ContractViolation, "duplicate"):
                compare_self._load_reusable_discovery_report(
                    duplicate, expected_sha256="0" * 64
                )
            with self.assertRaisesRegex(compare_self.ContractViolation, "non-finite"):
                compare_self._load_reusable_discovery_report(
                    nonfinite, expected_sha256="0" * 64
                )

            valid = root / "valid.json"
            valid.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(compare_self.ContractViolation, "digest differs"):
                compare_self._load_reusable_discovery_report(
                    valid, expected_sha256="0" * 64
                )

            crlf = root / "crlf.json"
            crlf_bytes = b"{}\r\n"
            crlf.write_bytes(crlf_bytes)
            _value, description = compare_self._load_reusable_discovery_report(
                crlf,
                expected_sha256=compare_self.hashlib.sha256(crlf_bytes).hexdigest(),
            )
            self.assertEqual(description["bytes"], len(crlf_bytes))
            self.assertEqual(
                description["sha256"],
                compare_self.hashlib.sha256(crlf_bytes).hexdigest(),
            )
            method = {
                "discovery_reuse": {
                    "enabled": True,
                    "source_report": description,
                }
            }
            self.assertEqual(
                compare_self._discovery_reuse_verification_errors(method), []
            )
            crlf.write_bytes(b"{}\n")
            self.assertIn(
                "digest changed",
                " ".join(compare_self._discovery_reuse_verification_errors(method)),
            )

    def test_reusable_discovery_ignores_unselected_output_drift(self) -> None:
        selected = "end_to_end/flowchart_medium"
        unselected = "end_to_end/state_medium"
        origin = {
            "bench_count": 2,
            "benches": [selected, unselected],
            "skipped": {},
            "preflight_receipts": {
                selected: preflight_receipt(selected),
                unselected: preflight_receipt(
                    unselected, output_sha256="b" * 64
                ),
            },
            "output_sha256": "c" * 64,
        }
        current = copy.deepcopy(origin)
        current["preflight_receipts"][unselected]["output_sha256"] = "d" * 64
        current["output_sha256"] = "e" * 64

        compare_self._require_reusable_discovery_match(
            label="base",
            current=current,
            origin=origin,
            required_benchmarks=frozenset({selected}),
        )

        current["preflight_receipts"][selected]["output_sha256"] = "f" * 64
        with self.assertRaisesRegex(
            compare_self.ContractViolation, "selected preflight receipt"
        ):
            compare_self._require_reusable_discovery_match(
                label="base",
                current=current,
                origin=origin,
                required_benchmarks=frozenset({selected}),
            )

    def test_reusable_discovery_requires_successful_post_verified_frozen_evidence(
        self,
    ) -> None:
        valid = self._minimal_reusable_discovery()
        compare_self._validate_reusable_discovery_report(valid)

        sampled = copy.deepcopy(valid)
        sampled["raw_rounds"] = [{"pair": 1}]
        with self.assertRaisesRegex(compare_self.ContractViolation, "sampling observations"):
            compare_self._validate_reusable_discovery_report(sampled)

        unverified = copy.deepcopy(valid)
        unverified["runners"]["head"]["post_sampling_verification"]["status"] = "failed"
        with self.assertRaisesRegex(compare_self.ContractViolation, "post-verified"):
            compare_self._validate_reusable_discovery_report(unverified)

        missing_receipts = copy.deepcopy(valid)
        del missing_receipts["fixtures"][0]["output_identity"]
        with self.assertRaisesRegex(compare_self.ContractViolation, "output identity"):
            compare_self._validate_reusable_discovery_report(missing_receipts)

        wrong_order = copy.deepcopy(valid)
        wrong_order["runners"]["base"]["shared_target_freeze"]["build_sequence"] = 2
        with self.assertRaisesRegex(compare_self.ContractViolation, "build sequence"):
            compare_self._validate_reusable_discovery_report(wrong_order)

        type_confusions = (
            ("schema_version", lambda report: report.__setitem__("schema_version", 2.0)),
            (
                "discovery_only",
                lambda report: report["method"].__setitem__("discovery_only", 1),
            ),
            (
                "complete successfully",
                lambda report: report["summary"].__setitem__("exit_code", False),
            ),
            (
                "contract failures",
                lambda report: report["summary"].__setitem__(
                    "contract_failures", False
                ),
            ),
            (
                "comparable count",
                lambda report: report["summary"].__setitem__("comparable", 1.0),
            ),
        )
        for message, mutate in type_confusions:
            confused = copy.deepcopy(valid)
            mutate(confused)
            with self.subTest(message=message), self.assertRaisesRegex(
                compare_self.ContractViolation, message
            ):
                compare_self._validate_reusable_discovery_report(confused)

    def test_prepare_reused_runner_revalidates_every_frozen_input_without_cargo_build(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            manifest = checkout / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "merman"\nversion = "0.0.0"\n'
                '\n[[bench]]\nname = "pipeline"\nharness = false\n',
                encoding="utf-8",
            )
            lockfile = checkout / "Cargo.lock"
            lockfile.write_text("# lock\n", encoding="utf-8")
            corpus = checkout / "tools" / "bench" / "corpus.json"
            corpus.parent.mkdir(parents=True)
            corpus.write_text(
                json.dumps(
                    minimal_corpus(
                        schema_version=2,
                        default_group="end_to_end",
                    )
                )
                + "\n",
                encoding="utf-8",
            )
            contract_source = (
                ROOT
                / "docs/performance/contracts/native-criterion-preflight-v1.json"
            )
            contract_target = checkout / contract_source.relative_to(ROOT)
            contract_target.parent.mkdir(parents=True)
            contract_target.write_bytes(contract_source.read_bytes())
            bench_source = checkout / "benches" / "pipeline.rs"
            bench_source.parent.mkdir()
            bench_source.write_text("fn main() {}\n", encoding="utf-8")
            target_dir = (root / "target").resolve()
            executable_bytes = b"frozen executable"
            executable_sha256 = compare_self.hashlib.sha256(executable_bytes).hexdigest()
            executable = (
                target_dir
                / "perf-frozen"
                / "reuse-test"
                / ("base-" + "a" * 40 + f"-{executable_sha256}")
                / PIPELINE_EXECUTABLE
            )
            executable.parent.mkdir(parents=True)
            executable.write_bytes(executable_bytes)
            executable.chmod(0o555)
            executable = executable.resolve()
            recipe = self._recipe(
                label="base",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            git = {
                "revision": "a" * 40,
                "tree": "b" * 40,
                "dirty": False,
                "dirty_disposition": "clean",
                "dirty_entries": [],
                "dirty_entries_truncated": False,
            }
            bench_target, described_bench_source = compare_self._describe_bench_target(
                manifest, "pipeline"
            )
            self.assertEqual(described_bench_source, bench_source)
            files = {
                "manifest": compare_self._describe_required_file(manifest),
                "workspace_manifest": compare_self._describe_required_file(manifest),
                "lockfile": compare_self._describe_required_file(lockfile),
                "corpus": compare_self._describe_corpus(corpus, recipe=recipe),
                "bench_target": bench_target,
                "bench_source": compare_self._describe_required_file(bench_source),
            }
            executable_description = compare_self._describe_required_file(executable)
            frozen_description = {
                **executable_description,
                "executable": True,
                "mode": "0555",
            }
            receipt = preflight_receipt()
            receipt_line = "[bench][preflight] " + json.dumps(
                receipt, separators=(",", ":")
            )
            discovery_stdout = "end_to_end/flowchart_medium: benchmark\n"
            combined = "\n".join((discovery_stdout, receipt_line))
            discovery = {
                "bench_count": 1,
                "benches": ["end_to_end/flowchart_medium"],
                "skipped": {},
                "preflight_receipts": {
                    "end_to_end/flowchart_medium": receipt,
                },
                "output_sha256": compare_self.hashlib.sha256(
                    combined.encode("utf-8")
                ).hexdigest(),
            }
            toolchain = {
                "requested": "1.95.0",
                "rustc_verbose": "rustc test",
                "cargo_verbose": "cargo test",
            }
            source_executable = {
                **executable_description,
                "path": str(target_dir / "debug" / "deps" / executable.name),
                "executable": True,
            }
            build_environment = {
                "RUSTFLAGS": None,
                "CARGO_ENCODED_RUSTFLAGS": None,
                "CARGO_BUILD_JOBS": "1",
                "CARGO_PROFILE_BENCH_LTO": None,
                "CARGO_PROFILE_BENCH_CODEGEN_UNITS": None,
                "CARGO_PROFILE_BENCH_OPT_LEVEL": None,
            }
            origin = {
                "recipe": compare_self._recipe_report(recipe),
                "git": git,
                **files,
                "toolchain": toolchain,
                "build_environment": build_environment,
                "shared_target_profile_reset": {
                    "strategy": "cargo-clean-bench-profile-before-each-side",
                    "command": compare_self.cargo_clean_bench_profile_command(recipe),
                    "stdout_tail": "",
                    "stderr_tail": "Removed bench profile",
                },
                "prebuild_command": compare_self.cargo_prebuild_command(recipe),
                "prebuild_stderr_tail": "",
                "source_executable": source_executable,
                "frozen_executable": frozen_description,
                "executable": {
                    **executable_description,
                    "executable": True,
                    "role": "frozen",
                },
                "shared_target_freeze": {
                    "enabled": True,
                    "context": "reuse-test",
                    "target_dir": str(target_dir),
                    "build_sequence": 1,
                    "commit": git["revision"],
                    "tree": git["tree"],
                    "source_executable": source_executable,
                    "frozen_executable": frozen_description,
                },
                "discovery_command": compare_self.criterion_list_command(
                    executable,
                    exact_benchmark="end_to_end/flowchart_medium",
                ),
                "discovery": discovery,
                "post_sampling_verification": {
                    "status": "verified",
                    "git": git,
                    "files": {key: value["sha256"] for key, value in files.items()},
                    "executable_sha256": executable_description["sha256"],
                },
            }
            listed = mock.Mock(
                returncode=0,
                stdout=discovery_stdout,
                stderr=receipt_line,
            )
            package_id = f"path+file://{checkout}#merman@0.0.0"
            metadata = mock.Mock(
                returncode=0,
                stdout=json.dumps(
                    {
                        "workspace_members": [package_id],
                        "packages": [
                            {
                                "id": package_id,
                                "name": "merman",
                                "manifest_path": str(manifest),
                            }
                        ],
                    }
                ),
                stderr="",
            )

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self, "_toolchain_version", return_value="rustc test"
                ),
                mock.patch.object(
                    compare_self, "_cargo_version", return_value="cargo test"
                ),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    side_effect=[metadata, listed],
                ) as run,
            ):
                runner, provenance, errors = compare_self._prepare_reused_runner(
                    recipe,
                    origin=origin,
                    source_report={"path": "/tmp/discovery.json", "sha256": "c" * 64},
                    required_benchmarks=frozenset(
                        {"end_to_end/flowchart_medium"}
                    ),
                    timeout_seconds=1,
                )

            self.assertFalse(errors)
            self.assertIsNotNone(runner)
            assert runner is not None
            self.assertTrue(runner.frozen)
            self.assertEqual(runner.executable, executable.resolve())
            self.assertEqual(provenance["discovery_reuse"]["status"], "verified")
            self.assertEqual(run.call_count, 2)
            self.assertEqual(
                run.call_args_list[0].args[0][0:3],
                ["cargo", "+1.95.0", "metadata"],
            )
            self.assertEqual(run.call_args_list[1].args[0][0], str(executable))
            self.assertNotIn("build", run.call_args_list[0].args[0])

            invalid_rediscoveries = {
                "missing": mock.Mock(
                    returncode=0,
                    stdout=discovery_stdout,
                    stderr="",
                ),
                "extra": mock.Mock(
                    returncode=0,
                    stdout=discovery_stdout,
                    stderr=receipt_line
                    + "\n[bench][preflight] "
                    + json.dumps(
                        preflight_receipt(
                            "end_to_end/class_medium", output_sha256="c" * 64
                        ),
                        separators=(",", ":"),
                    ),
                ),
            }
            for label, invalid_rediscovery in invalid_rediscoveries.items():
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc test"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo test"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[metadata, invalid_rediscovery],
                    ),
                ):
                    invalid_runner, _invalid_provenance, invalid_errors = (
                        compare_self._prepare_reused_runner(
                            recipe,
                            origin=origin,
                            source_report={
                                "path": "/tmp/discovery.json",
                                "sha256": "c" * 64,
                            },
                            required_benchmarks=frozenset(
                                {"end_to_end/flowchart_medium"}
                            ),
                            timeout_seconds=1,
                        )
                    )
                self.assertIsNone(invalid_runner)
                self.assertTrue(
                    any("preflight receipts differ" in error for error in invalid_errors)
                )

            runner_mutations = {
                "Git revision": lambda value: value["git"].__setitem__(
                    "revision", "c" * 40
                ),
                "lockfile digest": lambda value: value["lockfile"].__setitem__(
                    "sha256", "0" * 64
                ),
                "toolchain": lambda value: value["toolchain"].__setitem__(
                    "cargo_verbose", "cargo other"
                ),
                "prebuild command": lambda value: value["prebuild_command"].append(
                    "--forged"
                ),
                "frozen mode": lambda value: (
                    value["frozen_executable"].__setitem__("mode", "0755"),
                    value["shared_target_freeze"]["frozen_executable"].__setitem__(
                        "mode", "0755"
                    ),
                ),
                "origin verification": lambda value: value[
                    "post_sampling_verification"
                ]["files"].__setitem__("lockfile", "0" * 64),
                "selected preflight receipt": lambda value: value["discovery"][
                    "preflight_receipts"
                ]["end_to_end/flowchart_medium"].__setitem__(
                    "output_sha256", "0" * 64
                ),
            }
            for label, mutate in runner_mutations.items():
                changed = copy.deepcopy(origin)
                mutate(changed)
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc test"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo test"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[metadata, listed],
                    ),
                ):
                    changed_runner, _changed_provenance, changed_errors = (
                        compare_self._prepare_reused_runner(
                            recipe,
                            origin=changed,
                            source_report={
                                "path": "/tmp/discovery.json",
                                "sha256": "c" * 64,
                            },
                            required_benchmarks=frozenset(
                                {"end_to_end/flowchart_medium"}
                            ),
                            timeout_seconds=1,
                        )
                    )

                self.assertIsNone(changed_runner)
                self.assertTrue(changed_errors)

            swapped_executable = (
                target_dir
                / "perf-frozen"
                / "reuse-test"
                / ("head-" + "a" * 40 + f"-{executable_sha256}")
                / executable.name
            )
            swapped_executable.parent.mkdir(parents=True)
            swapped_executable.write_bytes(executable_bytes)
            swapped_executable.chmod(0o555)
            swapped = copy.deepcopy(origin)
            for description in (
                swapped["executable"],
                swapped["frozen_executable"],
                swapped["shared_target_freeze"]["frozen_executable"],
            ):
                description["path"] = str(swapped_executable)
            swapped["discovery_command"][0] = str(swapped_executable)

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self, "_toolchain_version", return_value="rustc test"
                ),
                mock.patch.object(
                    compare_self, "_cargo_version", return_value="cargo test"
                ),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    return_value=metadata,
                ) as swapped_run,
            ):
                swapped_runner, _provenance, swapped_errors = (
                    compare_self._prepare_reused_runner(
                        recipe,
                        origin=swapped,
                        source_report={
                            "path": "/tmp/discovery.json",
                            "sha256": "c" * 64,
                        },
                        required_benchmarks=frozenset(
                            {"end_to_end/flowchart_medium"}
                        ),
                        timeout_seconds=1,
                    )
                )

            self.assertIsNone(swapped_runner)
            self.assertIn("destination identity differs", swapped_errors[0])
            swapped_run.assert_called_once()
            self.assertEqual(
                swapped_run.call_args.args[0][0:3],
                ["cargo", "+1.95.0", "metadata"],
            )

    def test_reuse_comparison_contract_allows_new_selection_but_rejects_runner_drift(
        self,
    ) -> None:
        root = Path("/tmp/reuse-contract")
        recipes = {
            side: self._recipe(
                label=side,
                checkout=root / side,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=root / f"target-{side}",
                corpus=Path("tools/bench/corpus.json"),
            )
            for side in ("base", "head")
        }
        method = {
            "preset": "long",
            "sample_size": 30,
            "warm_up_seconds": 2,
            "measurement_seconds": 3,
            "diagnostic_pairs": 2,
            "calibration_pairs": 8,
            "max_pairs": 32,
            "start_side": "base",
            "relative_threshold_percent": 10.0,
            "relative_threshold_log": math.log1p(0.10),
            "absolute_threshold_ns": 50_000.0,
            "absolute_threshold_us": 50.0,
            "confidence_level": 0.95,
            "bootstrap_seed": 0,
            "bootstrap_resamples": 10_000,
            "interval_contract": {"confirmation": "paired"},
        }
        report = {
            "comparison": {"base_label": "base", "head_label": "head"},
            "environment": {"machine": "test"},
            "method": method,
        }
        selection = {
            "kind": "filter",
            "groups": {"base": "end_to_end", "head": "end_to_end"},
            "lane_contracts": {"effective": ("render-svg",)},
        }
        contracts = [
            {
                "name": "flowchart_medium",
                "family": "flowchart",
                "base_benchmark": "end_to_end/flowchart_medium",
                "head_benchmark": "end_to_end/flowchart_medium",
                "selected": {"base": True, "head": True},
                "metadata": {"base": None, "head": None},
                "bytes": {
                    "status": "identical",
                    "base": {"sha256": "a" * 64},
                    "head": {"sha256": "a" * 64},
                },
            }
        ]
        source = json.loads(
            json.dumps(
                {
                    "comparison": report["comparison"],
                    "environment": report["environment"],
                    "method": method,
                    "recipes": {
                        side: compare_self._recipe_report(recipe)
                        for side, recipe in recipes.items()
                    },
                    "selection": selection,
                    "fixtures": contracts,
                }
            )
        )

        compare_self._validate_reuse_comparison_contract(
            source=source,
            report=report,
            recipes=recipes,
        )

        changed_selection = copy.deepcopy(source)
        changed_selection["selection"]["groups"]["head"] = "render"
        changed_selection["fixtures"][0]["bytes"]["head"]["sha256"] = "b" * 64
        changed_selection["method"]["bootstrap_seed"] = 20260806
        changed_selection["method"]["bootstrap_resamples"] = 20_000
        compare_self._validate_reuse_comparison_contract(
            source=changed_selection,
            report=report,
            recipes=recipes,
        )

        mutations = {
            "comparison labels": lambda value: value["comparison"].__setitem__(
                "head_label", "other"
            ),
            "environment": lambda value: value["environment"].__setitem__(
                "machine", "other"
            ),
            "method": lambda value: value["method"].__setitem__("sample_size", 31),
            "recipes": lambda value: value["recipes"]["head"].__setitem__(
                "bench", "other"
            ),
        }
        for label, mutate in mutations.items():
            changed = copy.deepcopy(source)
            mutate(changed)
            with self.subTest(label=label), self.assertRaisesRegex(
                compare_self.ContractViolation, "differs"
            ):
                compare_self._validate_reuse_comparison_contract(
                    source=changed,
                    report=report,
                    recipes=recipes,
                )

    def test_runner_recipes_build_each_side_with_its_own_cargo_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base_checkout = root / "base"
            head_checkout = root / "head"
            base_checkout.mkdir()
            head_checkout.mkdir()
            (base_checkout / "Cargo.lock").write_text("# base lock\n", encoding="utf-8")
            (head_checkout / "Cargo.lock").write_text("# head lock\n", encoding="utf-8")

            base = self._recipe(
                label="base",
                checkout=base_checkout,
                package="merman-alpha",
                bench="render",
                features=("render",),
                default_features=False,
                toolchain="1.92.0",
                target_dir=root / "targets" / "base",
                corpus=Path("tools/bench/corpus-alpha.json"),
            )
            head = self._recipe(
                label="head",
                checkout=head_checkout,
                package="merman",
                bench="pipeline",
                features=("svg", "raster"),
                default_features=True,
                toolchain=None,
                target_dir=root / "targets" / "head",
                corpus=Path("tools/bench/corpus.json"),
            )

            base_command = compare_self.cargo_prebuild_command(base)
            head_command = compare_self.cargo_prebuild_command(head)

        self.assertEqual(base_command[:3], ["cargo", "+1.92.0", "bench"])
        self.assertEqual(head_command[:2], ["cargo", "bench"])
        self.assertIn("merman-alpha", base_command)
        self.assertIn("render", base_command)
        self.assertIn("merman", head_command)
        self.assertIn("pipeline", head_command)
        self.assertEqual(base_command[base_command.index("--features") + 1], "render")
        self.assertEqual(
            head_command[head_command.index("--features") + 1],
            "svg,raster",
        )
        self.assertIn("--no-default-features", base_command)
        self.assertNotIn("--no-default-features", head_command)
        self.assertIn("--locked", base_command)
        self.assertIn("--locked", head_command)
        self.assertIn("--no-run", base_command)
        self.assertIn("--no-run", head_command)
        self.assertIn("--message-format=json-render-diagnostics", base_command)
        self.assertEqual(base.corpus, Path("tools/bench/corpus-alpha.json"))
        self.assertEqual(head.corpus, Path("tools/bench/corpus.json"))
        self.assertEqual(
            base_command[base_command.index("--target-dir") + 1],
            str(root / "targets" / "base"),
        )
        self.assertEqual(
            head_command[head_command.index("--target-dir") + 1],
            str(root / "targets" / "head"),
        )

    def test_prebuild_refuses_unlocked_or_missing_lockfile_recipes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            unlocked = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                locked=False,
                corpus=Path("tools/bench/corpus.json"),
            )
            locked_without_file = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                locked=True,
                corpus=Path("tools/bench/corpus.json"),
            )

            with self.assertRaisesRegex(ValueError, "locked"):
                compare_self.cargo_prebuild_command(unlocked)
            with self.assertRaisesRegex(FileNotFoundError, "Cargo.lock"):
                compare_self.cargo_prebuild_command(locked_without_file)

    def test_distinct_target_prebuild_forces_and_records_one_cargo_job(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            recipe = self._recipe(
                label="base",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            git = {
                "revision": "a" * 40,
                "tree": "b" * 40,
                "dirty": False,
                "dirty_disposition": "clean",
                "dirty_entries": [],
                "dirty_entries_truncated": False,
            }
            described_file = {
                "path": "/tmp/input",
                "bytes": 1,
                "sha256": "c" * 64,
            }
            failed_prebuild = mock.Mock(
                returncode=1,
                stdout="",
                stderr="expected test failure",
            )

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self,
                    "_find_package_manifest",
                    return_value=checkout / "Cargo.toml",
                ),
                mock.patch.object(
                    compare_self,
                    "_describe_required_file",
                    return_value=described_file,
                ),
                mock.patch.object(
                    compare_self,
                    "_describe_corpus",
                    return_value={"path": "/tmp/corpus", "sha256": "d" * 64},
                ),
                mock.patch.object(
                    compare_self,
                    "_describe_bench_target",
                    return_value=(
                        {"name": "pipeline", "entry": {}, "sha256": "e" * 64},
                        checkout / "benches" / "pipeline.rs",
                    ),
                ),
                mock.patch.object(compare_self, "_toolchain_version", return_value="rustc"),
                mock.patch.object(compare_self, "_cargo_version", return_value="cargo"),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    return_value=failed_prebuild,
                ) as run_process,
            ):
                runner, provenance, errors = compare_self._prepare_runner(
                    recipe,
                    allow_dirty=False,
                    timeout_seconds=1,
                )

            self.assertIsNone(runner)
            self.assertTrue(errors)
            self.assertNotIn("shared_target_profile_reset", provenance)
            self.assertEqual(provenance["build_environment"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(run_process.call_count, 1)
            self.assertEqual(
                run_process.call_args_list[0].kwargs["env"]["CARGO_BUILD_JOBS"],
                "1",
            )

    def test_shared_target_clean_resets_only_the_selected_bench_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=root / "target",
                target="aarch64-apple-darwin",
                corpus=Path("tools/bench/corpus.json"),
            )

            command = compare_self.cargo_clean_bench_profile_command(recipe)

            self.assertEqual(
                command,
                [
                    "cargo",
                    "+1.95.0",
                    "clean",
                    "--locked",
                    "--profile",
                    "bench",
                    "--target-dir",
                    str(root / "target"),
                    "--target",
                    "aarch64-apple-darwin",
                ],
            )
            self.assertNotIn("--workspace", command)
            self.assertNotIn("--release", command)

    def test_shared_target_requires_explicit_freeze_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            shared_target = root / "target"
            base = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=shared_target,
                corpus=Path("tools/bench/corpus.json"),
            )
            head = self._recipe(
                label="head",
                checkout=root / "head",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=shared_target,
                corpus=Path("tools/bench/corpus.json"),
            )

            with self.assertRaisesRegex(compare_self.ContractViolation, "distinct Cargo target"):
                compare_self._shared_target_freeze_plan(base, head, enabled=False)

            plan = compare_self._shared_target_freeze_plan(
                base,
                head,
                enabled=True,
                context="u4-shared-target-test",
            )
            self.assertIsNotNone(plan)
            assert plan is not None
            self.assertEqual(plan.target_dir, shared_target.resolve())

            distinct_head = compare_self.RunnerRecipe(
                **{
                    **head.__dict__,
                    "target_dir": root / "other-target",
                }
            )
            with self.assertRaisesRegex(compare_self.ContractViolation, "same Cargo target"):
                compare_self._shared_target_freeze_plan(
                    base,
                    distinct_head,
                    enabled=True,
                    context="u4-shared-target-test",
                )

    def test_shared_target_freeze_survives_cargo_artifact_overwrite_and_rejects_collision(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"base executable")
            artifact.chmod(0o755)
            base = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            head = compare_self.RunnerRecipe(**{**base.__dict__, "label": "head"})
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="overwrite-test",
            )
            base_git = {"revision": "a" * 40, "tree": "b" * 40}
            head_git = {"revision": "c" * 40, "tree": "d" * 40}

            frozen_base, base_freeze = compare_self._freeze_bench_executable(
                artifact,
                recipe=base,
                git=base_git,
                plan=plan,
                build_sequence=1,
            )
            artifact.write_bytes(b"head executable")
            frozen_head, head_freeze = compare_self._freeze_bench_executable(
                artifact,
                recipe=head,
                git=head_git,
                plan=plan,
                build_sequence=2,
            )

            self.assertEqual(frozen_base.read_bytes(), b"base executable")
            self.assertEqual(frozen_head.read_bytes(), b"head executable")
            self.assertNotEqual(frozen_base, frozen_head)
            self.assertEqual(stat.S_IMODE(frozen_base.stat().st_mode), 0o555)
            self.assertEqual(base_freeze["build_sequence"], 1)
            self.assertEqual(head_freeze["build_sequence"], 2)
            self.assertEqual(base_freeze["commit"], "a" * 40)
            self.assertEqual(base_freeze["tree"], "b" * 40)

            with self.assertRaisesRegex(compare_self.ContractViolation, "already exists"):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=head,
                    git=head_git,
                    plan=plan,
                    build_sequence=2,
                )

    def test_prepare_shared_target_uses_only_the_frozen_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.toml").write_text(
                '[package]\nname = "merman"\nversion = "0.0.0"\n'
                '\n[[bench]]\nname = "pipeline"\nharness = false\n',
                encoding="utf-8",
            )
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            corpus = checkout / "tools" / "bench" / "corpus.json"
            corpus.parent.mkdir(parents=True)
            corpus.write_text(
                json.dumps(
                    minimal_corpus(
                        schema_version=2,
                        default_group="end_to_end",
                    )
                )
                + "\n",
                encoding="utf-8",
            )
            contract_source = (
                ROOT
                / "docs/performance/contracts/native-criterion-preflight-v1.json"
            )
            contract_target = checkout / contract_source.relative_to(ROOT)
            contract_target.parent.mkdir(parents=True)
            contract_target.write_bytes(contract_source.read_bytes())
            bench_source = checkout / "benches" / "pipeline.rs"
            bench_source.parent.mkdir()
            bench_source.write_text("fn main() {}\n", encoding="utf-8")
            target_dir = root / "target"
            cargo_artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
            cargo_artifact.parent.mkdir(parents=True)
            cargo_artifact.write_bytes(b"bench executable")
            cargo_artifact.chmod(0o755)
            recipe = self._recipe(
                label="base",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            cargo_stdout = json.dumps(
                {
                    "reason": "compiler-artifact",
                    "target": {"kind": ["bench"], "name": "pipeline"},
                    "executable": str(cargo_artifact),
                }
            )
            cargo_result = mock.Mock(returncode=0, stdout=cargo_stdout, stderr="")
            package_id = f"path+file://{checkout}#merman@0.0.0"
            metadata_result = mock.Mock(
                returncode=0,
                stdout=json.dumps(
                    {
                        "workspace_members": [package_id],
                        "packages": [
                            {
                                "id": package_id,
                                "name": "merman",
                                "manifest_path": str(checkout / "Cargo.toml"),
                            }
                        ],
                    }
                ),
                stderr="",
            )
            clean_result = mock.Mock(
                returncode=0,
                stdout="",
                stderr="Removed 42 files, 12.3MiB total",
            )
            receipt = preflight_receipt()
            receipt_line = "[bench][preflight] " + json.dumps(
                receipt, separators=(",", ":")
            )
            discovery_result = mock.Mock(
                returncode=0,
                stdout="end_to_end/flowchart_medium: benchmark\n",
                stderr=receipt_line,
            )
            git = {
                "revision": "a" * 40,
                "tree": "b" * 40,
                "dirty": False,
                "dirty_disposition": "clean",
                "dirty_entries": [],
                "dirty_entries_truncated": False,
            }
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="prepare-test",
            )

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(compare_self, "_toolchain_version", return_value="rustc"),
                mock.patch.object(compare_self, "_cargo_version", return_value="cargo"),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    side_effect=[
                        metadata_result,
                        clean_result,
                        cargo_result,
                        discovery_result,
                    ],
                ) as run_process,
            ):
                runner, provenance, errors = compare_self._prepare_runner(
                    recipe,
                    allow_dirty=False,
                    timeout_seconds=1,
                    freeze_plan=plan,
                    build_sequence=1,
                )

            self.assertFalse(errors)
            self.assertIsNotNone(runner)
            assert runner is not None
            self.assertTrue(runner.frozen)
            self.assertTrue(
                runner.executable.is_relative_to(
                    (target_dir / "perf-frozen" / "prepare-test").resolve()
                )
            )
            self.assertEqual(provenance["build_environment"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(provenance["shared_target_freeze"]["build_sequence"], 1)
            self.assertEqual(
                provenance["shared_target_profile_reset"]["strategy"],
                "cargo-clean-bench-profile-before-each-side",
            )
            self.assertIn(
                "Removed 42 files",
                provenance["shared_target_profile_reset"]["stderr_tail"],
            )
            self.assertEqual(
                Path(provenance["source_executable"]["path"]).resolve(),
                cargo_artifact.resolve(),
            )
            self.assertEqual(provenance["discovery_command"][0], str(runner.executable))
            self.assertEqual(
                compare_self.criterion_command(
                    executable=runner.executable,
                    exact_bench="end_to_end/flowchart_medium",
                    sample_size=10,
                    warm_up_seconds=1,
                    measurement_seconds=1,
                )[0],
                str(runner.executable),
            )
            self.assertEqual(
                run_process.call_args_list[1].args[0][1:4],
                ["clean", "--locked", "--profile"],
            )
            self.assertEqual(
                run_process.call_args_list[1].kwargs["env"]["CARGO_BUILD_JOBS"],
                "1",
            )
            self.assertEqual(
                run_process.call_args_list[2].kwargs["env"]["CARGO_BUILD_JOBS"],
                "1",
            )
            self.assertEqual(
                provenance["discovery"]["preflight_receipts"],
                {"end_to_end/flowchart_medium": receipt},
            )

            invalid_discoveries = {
                "missing": mock.Mock(
                    returncode=0,
                    stdout="end_to_end/flowchart_medium: benchmark\n",
                    stderr="",
                ),
                "extra": mock.Mock(
                    returncode=0,
                    stdout="end_to_end/flowchart_medium: benchmark\n",
                    stderr=receipt_line
                    + "\n[bench][preflight] "
                    + json.dumps(
                        preflight_receipt(
                            "end_to_end/class_medium", output_sha256="c" * 64
                        ),
                        separators=(",", ":"),
                    ),
                ),
            }
            for label, invalid_discovery in invalid_discoveries.items():
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[
                            metadata_result,
                            clean_result,
                            cargo_result,
                            invalid_discovery,
                        ],
                    ),
                ):
                    invalid_runner, _invalid_provenance, invalid_errors = (
                        compare_self._prepare_runner(
                            recipe,
                            allow_dirty=False,
                            timeout_seconds=1,
                            freeze_plan=compare_self.SharedTargetFreezePlan(
                                target_dir=target_dir,
                                context=f"prepare-{label}",
                            ),
                            build_sequence=1,
                        )
                    )
                self.assertIsNone(invalid_runner)
                self.assertTrue(
                    any("preflight receipts differ" in error for error in invalid_errors)
                )

    def test_shared_target_freeze_rejects_source_digest_drift_during_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"original executable")
            artifact.chmod(0o755)
            recipe = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="source-drift-test",
            )
            copyfileobj = compare_self.shutil.copyfileobj

            def copy_then_mutate(source, destination, *, length):
                copyfileobj(source, destination, length=length)
                artifact.write_bytes(b"mutated executable")

            with (
                mock.patch.object(
                    compare_self.shutil,
                    "copyfileobj",
                    side_effect=copy_then_mutate,
                ),
                self.assertRaisesRegex(compare_self.ContractViolation, "changed while freezing"),
            ):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=recipe,
                    git={"revision": "a" * 40, "tree": "b" * 40},
                    plan=plan,
                    build_sequence=1,
                )

            frozen_files = list((target_dir / "perf-frozen").rglob(PIPELINE_EXECUTABLE))
            self.assertEqual(frozen_files, [])

    def test_shared_target_freeze_cleans_failed_read_only_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"bench executable")
            artifact.chmod(0o755)
            recipe = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="failed-publication-test",
            )
            digest = compare_self.hashlib.sha256(artifact.read_bytes()).hexdigest()

            with (
                mock.patch.object(
                    compare_self,
                    "_path_sha256",
                    side_effect=[digest, digest, digest, "0" * 64],
                ),
                self.assertRaisesRegex(
                    compare_self.ContractViolation,
                    "published frozen executable digest differs",
                ),
            ):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=recipe,
                    git={"revision": "a" * 40, "tree": "b" * 40},
                    plan=plan,
                    build_sequence=1,
                )

            frozen_files = list((target_dir / "perf-frozen").rglob(PIPELINE_EXECUTABLE))
            self.assertEqual(frozen_files, [])
            freeze_context = target_dir / "perf-frozen" / "failed-publication-test"
            self.assertEqual(list(freeze_context.iterdir()), [])

    def test_frozen_digest_drift_fails_before_round_sampling(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            executable = root / "pipeline"
            executable.write_bytes(b"frozen")
            executable.chmod(0o555)
            recipe = self._recipe(
                label="base",
                checkout=root,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            runner = compare_self.PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256=compare_self._path_sha256(executable),
                benches={"end_to_end/flowchart_medium"},
                skipped={},
                provenance={},
                env={},
                frozen=True,
            )
            executable.chmod(0o755)
            executable.write_bytes(b"drifted")

            with mock.patch.object(compare_self, "_measure_once") as measure:
                schedule = compare_self._run_aa_schedule(
                    runner,
                    contracts=[
                        {
                            "name": "flowchart_medium",
                            "base_benchmark": "end_to_end/flowchart_medium",
                        }
                    ],
                    pair_count=1,
                    sample_size=10,
                    warm_up_seconds=1,
                    measurement_seconds=1,
                    timeout_seconds=1,
                )

            measure.assert_not_called()
            self.assertIn("digest changed", schedule["errors"]["flowchart_medium"])
            self.assertIn("error", schedule["rounds"][0]["executable_verification"])

    def test_different_trees_require_distinct_frozen_executables(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            executable = root / "pipeline"
            executable.write_bytes(b"same executable")
            executable.chmod(0o555)
            recipe = self._recipe(
                label="base",
                checkout=root,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            digest = compare_self._path_sha256(executable)
            base = compare_self.PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256=digest,
                benches=set(),
                skipped={},
                provenance={"git": {"tree": "a" * 40}},
                env={},
                frozen=True,
            )
            head = compare_self.PreparedRunner(
                recipe=compare_self.RunnerRecipe(
                    **{**recipe.__dict__, "label": "head"}
                ),
                executable=executable,
                executable_sha256=digest,
                benches=set(),
                skipped={},
                provenance={"git": {"tree": "b" * 40}},
                env={},
                frozen=True,
            )

            error = compare_self._binary_independence_error(base, head)

            self.assertIsNotNone(error)
            self.assertIn("byte-identical", error)
            head.provenance["git"]["tree"] = "a" * 40
            self.assertIsNone(compare_self._binary_independence_error(base, head))

    def test_confirmation_requires_byte_identical_harness_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            executable = root / "pipeline"
            executable.write_bytes(b"bench")
            executable.chmod(0o555)
            recipe = self._recipe(
                label="base",
                checkout=root,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            provenance = {
                "bench_target": {"sha256": "1" * 64},
                "bench_source": {"sha256": "2" * 64},
                "corpus": {
                    "sha256": "3" * 64,
                    "preflight_contract": {"sha256": "4" * 64},
                },
            }
            base = compare_self.PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256="5" * 64,
                benches=set(),
                skipped={},
                provenance=copy.deepcopy(provenance),
                env={},
            )
            head = compare_self.PreparedRunner(
                recipe=compare_self.RunnerRecipe(
                    **{**recipe.__dict__, "label": "head"}
                ),
                executable=executable,
                executable_sha256="6" * 64,
                benches=set(),
                skipped={},
                provenance=copy.deepcopy(provenance),
                env={},
            )

            self.assertEqual(
                compare_self._confirmation_harness_identity_errors(base, head), []
            )
            cases = (
                ("bench_target", None, "Cargo [[bench]] entry"),
                ("bench_source", None, "benchmark source"),
                ("corpus", None, "corpus manifest"),
                ("corpus", "preflight_contract", "preflight contract"),
            )
            for key, nested, expected in cases:
                with self.subTest(field=expected):
                    changed = copy.deepcopy(head.provenance)
                    if nested is None:
                        changed[key]["sha256"] = "f" * 64
                    else:
                        changed[key][nested]["sha256"] = "f" * 64
                    head.provenance = changed
                    self.assertTrue(
                        any(
                            expected in error
                            for error in compare_self._confirmation_harness_identity_errors(
                                base, head
                            )
                        )
                    )
                    head.provenance = copy.deepcopy(provenance)

    def test_bench_target_identity_uses_only_the_selected_cargo_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            manifest = root / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "merman"\nversion = "0.0.0"\n'
                '\n[[bench]]\nname = "pipeline"\nharness = false\n'
                '\n[[bench]]\nname = "ascii_pipeline"\nharness = false\n'
                'path = "benches/custom_ascii.rs"\n',
                encoding="utf-8",
            )

            target, source = compare_self._describe_bench_target(
                manifest, "ascii_pipeline"
            )

        self.assertEqual(target["name"], "ascii_pipeline")
        self.assertEqual(
            target["entry"],
            {
                "name": "ascii_pipeline",
                "harness": False,
                "path": "benches/custom_ascii.rs",
            },
        )
        self.assertEqual(source, root / "benches/custom_ascii.rs")

    def test_prepare_checks_git_before_creating_an_in_checkout_target_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            checkout = Path(temp_dir) / "checkout"
            checkout.mkdir()
            target_dir = checkout / "unignored-target"
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )

            def capture_clean_tree(*_args, **_kwargs):
                self.assertFalse(target_dir.exists())
                return {
                    "revision": "a" * 40,
                    "dirty": False,
                    "dirty_disposition": "clean",
                    "dirty_entries": [],
                    "dirty_entries_truncated": False,
                }

            with mock.patch.object(
                compare_self,
                "_git_provenance",
                side_effect=capture_clean_tree,
            ):
                prepared, _provenance, errors = compare_self._prepare_runner(
                    recipe,
                    allow_dirty=False,
                    timeout_seconds=1,
                )

            self.assertIsNone(prepared)
            self.assertTrue(errors)
            self.assertTrue(target_dir.exists())

    def test_describe_required_file_reuses_a_precomputed_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "artifact"
            path.write_bytes(b"artifact")
            digest = "a" * 64
            with mock.patch.object(compare_self, "_path_sha256") as path_sha256:
                description = compare_self._describe_required_file(
                    path,
                    sha256=digest,
                )

        path_sha256.assert_not_called()
        self.assertEqual(description["sha256"], digest)

    def test_parses_the_unique_matching_bench_executable_from_cargo_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            executable = root / "target" / "release" / "deps" / PIPELINE_EXECUTABLE
            unrelated = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo#merman@0.9.0",
                "target": {"kind": ["lib"], "name": "merman"},
                "executable": None,
            }
            matching = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo#merman@0.9.0",
                "target": {"kind": ["bench"], "name": "pipeline"},
                "profile": {"test": True},
                "executable": str(executable),
            }
            cargo_stdout = "\n".join(
                ["Compiling merman", json.dumps(unrelated), json.dumps(matching)]
            )

            parsed = compare_self.parse_bench_executable(cargo_stdout, recipe=recipe)

            self.assertEqual(parsed, executable)
            duplicate_stdout = "\n".join(
                [cargo_stdout, json.dumps({**matching, "executable": str(executable) + "-2"})]
            )
            with self.assertRaisesRegex(ValueError, "unique|multiple"):
                compare_self.parse_bench_executable(duplicate_stdout, recipe=recipe)
            with self.assertRaisesRegex(ValueError, "unique|missing"):
                compare_self.parse_bench_executable(json.dumps(unrelated), recipe=recipe)

    def test_direct_criterion_command_uses_hidden_benchmark_mode_and_exact_filter(self) -> None:
        executable = Path("target") / "release" / "deps" / PIPELINE_EXECUTABLE
        command = compare_self.criterion_command(
            executable=executable,
            exact_bench="end_to_end/flowchart_medium",
            sample_size=30,
            warm_up_seconds=2,
            measurement_seconds=3,
        )

        self.assertEqual(command[0], str(executable))
        self.assertIn("--bench", command)
        self.assertEqual(command[command.index("--color") + 1], "never")
        self.assertEqual(
            command[command.index("--exact") + 1],
            "end_to_end/flowchart_medium",
        )
        self.assertEqual(command[command.index("--sample-size") + 1], "30")
        self.assertEqual(command[command.index("--warm-up-time") + 1], "2")
        self.assertEqual(command[command.index("--measurement-time") + 1], "3")

if __name__ == "__main__":
    sys.exit(unittest.main())
