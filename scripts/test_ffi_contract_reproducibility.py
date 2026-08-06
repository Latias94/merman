#!/usr/bin/env python3
"""Unit tests for the FFI reproducibility boundary."""

from __future__ import annotations

from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from ffi_contract_baseline_contract import (  # noqa: E402
    rust_toolchain_dependency_compatibility_projection,
    rust_toolchain_native_compatibility_projection,
)
from ffi_contract_reproducibility import (  # noqa: E402
    CANONICAL_REPRODUCIBILITY_ENVIRONMENT,
    FfiContractReproducibilityError,
    reject_ffi_contract_environment,
    resolve_rust_toolchain,
)


class ReproducibilityEnvironmentTests(unittest.TestCase):
    def test_canonical_values_are_valid_parent_environment(self) -> None:
        reject_ffi_contract_environment(CANONICAL_REPRODUCIBILITY_ENVIRONMENT)

    def test_noncanonical_and_unrelated_overrides_are_rejected(self) -> None:
        for environment in (
            {"CARGO_INCREMENTAL": "1"},
            {"SOURCE_DATE_EPOCH": ""},
            {"RUSTFLAGS": "-C target-cpu=native"},
        ):
            with self.subTest(environment=environment):
                with self.assertRaises(FfiContractReproducibilityError):
                    reject_ffi_contract_environment(environment)


class RustupResolutionTests(unittest.TestCase):
    def test_multicall_symlink_is_invoked_without_resolving_argv_zero(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            rustup_init = self._executable(root / "rustup-init")
            rustup = root / "rustup"
            rustup.symlink_to(rustup_init.name)
            cargo = self._executable(root / "cargo")
            rustc = self._executable(root / "rustc")
            canonical_cargo = cargo.resolve()
            canonical_rustc = rustc.resolve()
            commands: list[tuple[str, ...]] = []

            def runner(
                command: list[str] | tuple[str, ...],
            ) -> subprocess.CompletedProcess[str]:
                normalized = tuple(command)
                commands.append(normalized)
                outputs = {
                    (str(rustup), "which", "cargo"): str(cargo),
                    (str(rustup), "which", "rustc"): str(rustc),
                    (str(canonical_cargo), "-V"): "cargo 1.95.0 (test)",
                    (str(canonical_rustc), "-Vv"): (
                        "rustc 1.95.0 (test)\n"
                        "commit-hash: test\n"
                        "host: aarch64-apple-darwin"
                    ),
                }
                return subprocess.CompletedProcess(normalized, 0, outputs[normalized], "")

            with mock.patch(
                "ffi_contract_reproducibility.shutil.which",
                return_value=str(rustup),
            ):
                identity = resolve_rust_toolchain(runner)

            self.assertEqual(commands[0], (str(rustup), "which", "cargo"))
            self.assertNotEqual(commands[0][0], str(rustup_init))
            self.assertEqual(identity.host_target, "aarch64-apple-darwin")

    def test_explicit_rustup_failure_does_not_fall_back_to_path_tools(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            rustup = root / "rustup"
            self._executable(rustup)

            def runner(
                command: list[str] | tuple[str, ...],
            ) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess(command, 1, "", "failed")

            with mock.patch(
                "ffi_contract_reproducibility.shutil.which",
                return_value=str(rustup),
            ):
                with self.assertRaises(FfiContractReproducibilityError):
                    resolve_rust_toolchain(runner)

    @staticmethod
    def _executable(path: Path) -> Path:
        path.write_text("tool", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path


class ToolchainCompatibilityTests(unittest.TestCase):
    def test_dependency_projection_is_cross_host_but_release_sensitive(self) -> None:
        apple = self._toolchain("aarch64-apple-darwin", "a", "b", "1.95.0")
        linux = self._toolchain("x86_64-unknown-linux-gnu", "c", "d", "1.95.0")

        self.assertEqual(
            rust_toolchain_dependency_compatibility_projection(apple),
            rust_toolchain_dependency_compatibility_projection(linux),
        )
        self.assertNotEqual(
            rust_toolchain_native_compatibility_projection(apple),
            rust_toolchain_native_compatibility_projection(linux),
        )

        newer = self._toolchain("x86_64-unknown-linux-gnu", "c", "d", "1.96.0")
        self.assertNotEqual(
            rust_toolchain_dependency_compatibility_projection(apple),
            rust_toolchain_dependency_compatibility_projection(newer),
        )

    @staticmethod
    def _toolchain(
        host: str,
        cargo_sha: str,
        rustc_sha: str,
        release: str,
    ) -> dict[str, object]:
        return {
            "cargo": {"path": "/toolchain/cargo", "sha256": f"sha256:{cargo_sha}"},
            "rustc": {"path": "/toolchain/rustc", "sha256": f"sha256:{rustc_sha}"},
            "cargo_version": f"cargo {release} (test)",
            "rustc_verbose": (
                f"rustc {release} (test)\n"
                "commit-hash: test\n"
                f"host: {host}"
            ),
            "host_target": host,
        }


if __name__ == "__main__":
    unittest.main()
