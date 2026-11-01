#!/usr/bin/env python3
"""Check Flutter host selection and packaged paths without native builds."""

from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


BUILDER_PATH = Path(__file__).resolve().parents[1] / "platforms/flutter/build-native.py"
SPEC = importlib.util.spec_from_file_location("build_flutter_native", BUILDER_PATH)
assert SPEC is not None and SPEC.loader is not None
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class FlutterNativeBuilderTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(temporary)
        self.artifact = self.root / "native.fixture"
        self.artifact.write_bytes(b"native fixture")
        self.build = self.enterContext(patch.object(builder, "build", return_value=self.artifact))
        self.enterContext(patch.object(builder, "PACKAGE_ROOT", self.root / "flutter"))
        self.enterContext(patch.object(builder, "run"))
        self.enterContext(redirect_stdout(io.StringIO()))

    def test_supported_hosts_build_into_canonical_native_asset_paths(self) -> None:
        for system, machines, target, relative in (
            ("Darwin", ("arm64", "aarch64"), "aarch64-apple-darwin", "macos/arm64/libmerman_ffi.dylib"),
            ("Darwin", ("x86_64", "AMD64"), "x86_64-apple-darwin", "macos/x86_64/libmerman_ffi.dylib"),
            ("Linux", ("arm64", "aarch64", "AARCH64"), "aarch64-unknown-linux-gnu", "linux/aarch64/libmerman_ffi.so"),
            ("Linux", ("x86_64", "amd64"), "x86_64-unknown-linux-gnu", "linux/x86_64/libmerman_ffi.so"),
            ("Windows", ("AMD64", "x86_64"), "x86_64-pc-windows-gnu", "windows/x86_64/merman_ffi.dll"),
        ):
            for machine in machines:
                with self.subTest(system=system, machine=machine):
                    self.build.reset_mock()
                    package_root = self.root / f"{system}-{machine}"
                    with patch.object(builder, "PACKAGE_ROOT", package_root), patch.object(
                        builder.platform, "system", return_value=system
                    ), patch.object(builder.platform, "machine", return_value=machine):
                        builder.build_host()
                    self.build.assert_called_once_with(builder.DESKTOP_RECIPE, target)
                    self.assertEqual(
                        (package_root / "native" / relative).read_bytes(), b"native fixture"
                    )
                    self.assertFalse((package_root / "native/linux/arm64").exists())

    def test_unsupported_hosts_fail_before_build(self) -> None:
        for system, machine in (
            ("Darwin", "unknown"), ("Linux", "unknown"), ("Windows", "unknown"),
            ("Windows", "arm64"), ("Windows", "aarch64"),
            ("FreeBSD", "x86_64"),
        ):
            with self.subTest(system=system, machine=machine), patch.object(
                builder.platform, "system", return_value=system
            ), patch.object(builder.platform, "machine", return_value=machine):
                self.build.reset_mock()
                with self.assertRaisesRegex(RuntimeError, "unsupported Flutter host"):
                    builder.build_host()
                self.build.assert_not_called()


if __name__ == "__main__":
    unittest.main()
