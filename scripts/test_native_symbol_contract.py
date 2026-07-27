#!/usr/bin/env python3
"""Tests for fail-closed native-library symbol contracts."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from native_symbol_contract import (
    ANDROID_JNI_SYMBOL_CONTRACT,
    C_ABI_SYMBOL_CONTRACT,
    assert_symbol_contract,
    canonicalize_owned_symbol,
    parse_llvm_nm_posix,
    read_defined_dynamic_symbols,
)


class LlvmNmParserTests(unittest.TestCase):
    def test_parser_reads_defined_dynamic_symbol_names(self) -> None:
        output = """JNI_OnLoad T 0000000000012340 0000000000000080
__cxa_finalize U 0000000000000000 0000000000000000
merman_internal t 0000000000013000 0000000000000010
"""

        self.assertEqual(
            parse_llvm_nm_posix(output),
            {"JNI_OnLoad", "__cxa_finalize", "merman_internal"},
        )

    def test_parser_canonicalizes_macho_owned_symbols_only(self) -> None:
        output = """_merman_get_native_api T 0000000000012340 0000000000000080
_JNI_OnLoad T 0000000000012440 0000000000000080
__merman_hidden T 0000000000012540 0000000000000080
__cxa_finalize U 0000000000000000 0000000000000000
"""
        self.assertEqual(
            parse_llvm_nm_posix(output),
            {
                "merman_get_native_api",
                "JNI_OnLoad",
                "merman_hidden",
                "__cxa_finalize",
            },
        )
        self.assertEqual(canonicalize_owned_symbol("__cxa_finalize"), "__cxa_finalize")

    def test_parser_rejects_empty_or_malformed_output(self) -> None:
        for output in ("", "not-a-posix-symbol-line\n"):
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                parse_llvm_nm_posix(output)

    def test_reader_uses_dynamic_defined_only_posix_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            library = root / "libtransport.so"
            llvm_nm = root / "llvm-nm"
            library.touch()
            llvm_nm.touch()
            commands: list[list[str]] = []

            def runner(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout="JNI_OnLoad T 00000000 00000010\n",
                    stderr="",
                )

            self.assertEqual(
                read_defined_dynamic_symbols(library, llvm_nm, runner=runner),
                {"JNI_OnLoad"},
            )
            self.assertEqual(
                commands,
                [
                    [
                        str(llvm_nm),
                        "--dynamic",
                        "--defined-only",
                        "--format=posix",
                        str(library),
                    ]
                ],
            )

    def test_reader_uses_external_defined_symbols_for_macho(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            library = root / "libtransport.dylib"
            llvm_nm = root / "llvm-nm"
            library.touch()
            llvm_nm.touch()
            commands: list[list[str]] = []

            def runner(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout="_merman_get_native_api T 00000000 00000010\n",
                    stderr="",
                )

            self.assertEqual(
                read_defined_dynamic_symbols(library, llvm_nm, runner=runner),
                {"merman_get_native_api"},
            )
            self.assertEqual(
                commands,
                [
                    [
                        str(llvm_nm),
                        "--extern-only",
                        "--defined-only",
                        "--format=posix",
                        str(library),
                    ]
                ],
            )


class NativeSymbolContractTests(unittest.TestCase):
    def test_android_jni_requires_jni_on_load_and_forbids_c_abi(self) -> None:
        assert_symbol_contract(
            {"JNI_OnLoad", "internal_helper"},
            ANDROID_JNI_SYMBOL_CONTRACT,
            label="Android JNI",
        )

        with self.assertRaisesRegex(RuntimeError, "missing required.*JNI_OnLoad"):
            assert_symbol_contract(
                {"internal_helper"},
                ANDROID_JNI_SYMBOL_CONTRACT,
                label="Android JNI",
            )
        with self.assertRaisesRegex(RuntimeError, "forbidden.*merman_get_native_api"):
            assert_symbol_contract(
                {"JNI_OnLoad", "merman_get_native_api"},
                ANDROID_JNI_SYMBOL_CONTRACT,
                label="Android JNI",
            )

    def test_c_abi_requires_discovery_and_forbids_jni_on_load(self) -> None:
        assert_symbol_contract(
            {"merman_get_native_api", "internal_helper"},
            C_ABI_SYMBOL_CONTRACT,
            label="C ABI",
        )

        with self.assertRaisesRegex(RuntimeError, "missing required.*merman_get_native_api"):
            assert_symbol_contract(
                {"internal_helper"},
                C_ABI_SYMBOL_CONTRACT,
                label="C ABI",
            )
        with self.assertRaisesRegex(RuntimeError, "forbidden.*JNI_OnLoad"):
            assert_symbol_contract(
                {"merman_get_native_api", "JNI_OnLoad"},
                C_ABI_SYMBOL_CONTRACT,
                label="C ABI",
            )

    def test_c_abi_rejects_every_unlisted_merman_owned_export(self) -> None:
        for legacy in ("merman_render_svg", "merman_engine_free", "_merman_abi2_call"):
            with self.subTest(legacy=legacy), self.assertRaisesRegex(
                RuntimeError, "unexpected Merman-owned"
            ):
                assert_symbol_contract(
                    {"_merman_get_native_api", legacy, "__cxa_finalize"},
                    C_ABI_SYMBOL_CONTRACT,
                    label="C ABI",
                )

        assert_symbol_contract(
            {"_merman_get_native_api", "__cxa_finalize", "malloc"},
            C_ABI_SYMBOL_CONTRACT,
            label="C ABI",
        )

    def test_android_rejects_static_jni_or_c_abi_owned_exports(self) -> None:
        for leaked in ("Java_io_merman_MermanNative_execute", "merman_render_svg"):
            with self.subTest(leaked=leaked), self.assertRaisesRegex(
                RuntimeError, "unexpected Merman-owned"
            ):
                assert_symbol_contract(
                    {"JNI_OnLoad", leaked, "__cxa_finalize"},
                    ANDROID_JNI_SYMBOL_CONTRACT,
                    label="Android JNI",
                )


if __name__ == "__main__":
    unittest.main()
