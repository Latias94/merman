#!/usr/bin/env python3
"""Tests for fail-closed native-library symbol contracts."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from native_symbol_contract import (
    ANDROID_JNI_SYMBOL_CONTRACT,
    C_ABI_SYMBOL_CONTRACT,
    assert_symbol_contract,
    canonicalize_owned_symbol,
    main,
    parse_llvm_nm_posix,
    read_defined_dynamic_symbols,
    read_macho_architectures,
)

class LlvmNmParserTests(unittest.TestCase):
    def test_parser_reads_defined_dynamic_symbol_names(self) -> None:
        output = """JNI_OnLoad T 0000000000012340 0000000000000080
__cxa_finalize U 0000000000000000 0000000000000000
merman_internal t 0000000000013000 0000000000000010
"""

        self.assertEqual(
            parse_llvm_nm_posix(output),
            {"JNI_OnLoad", "merman_internal"},
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
            },
        )
        self.assertEqual(canonicalize_owned_symbol("__cxa_finalize"), "__cxa_finalize")

    def test_undefined_required_symbol_does_not_satisfy_contract(self) -> None:
        for symbol_type in ("U", "u"):
            output = f"""merman_get_native_api {symbol_type} 0000000000000000 0000000000000000
internal_helper T 0000000000012340 0000000000000010
"""

            with self.subTest(symbol_type=symbol_type), self.assertRaisesRegex(
                RuntimeError,
                "missing required symbols: merman_get_native_api",
            ):
                assert_symbol_contract(
                    parse_llvm_nm_posix(output),
                    C_ABI_SYMBOL_CONTRACT,
                    label="C ABI",
                )

    def test_parser_rejects_empty_or_malformed_output(self) -> None:
        for output in ("", "not-a-posix-symbol-line\n"):
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                parse_llvm_nm_posix(output)

    def test_parser_accepts_static_archive_member_headers(self) -> None:
        output = """
merman_ffi.symbols.o:
merman_get_native_api T 00000000 00000010
__imp_merman_get_native_api I 00000000 00000000
"""

        self.assertEqual(
            parse_llvm_nm_posix(output),
            {"merman_get_native_api"},
        )
        self.assertEqual(
            canonicalize_owned_symbol("__imp_merman_get_native_api"),
            "merman_get_native_api",
        )

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

    def test_reader_can_select_one_macho_architecture(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            library = root / "MermanFFI"
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
                read_defined_dynamic_symbols(
                    library,
                    llvm_nm,
                    external_only=True,
                    architecture="arm64",
                    runner=runner,
                ),
                {"merman_get_native_api"},
            )
            self.assertEqual(
                commands,
                [
                    [
                        str(llvm_nm),
                        "--arch=arm64",
                        "--extern-only",
                        "--defined-only",
                        "--format=posix",
                        str(library),
                    ]
                ],
            )

    def test_macho_architecture_reader_uses_lipo_without_shell_parsing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            library = root / "MermanFFI"
            lipo = root / "lipo"
            library.touch()
            lipo.touch()
            commands: list[list[str]] = []

            def runner(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout="arm64 x86_64\n",
                    stderr="",
                )

            self.assertEqual(
                read_macho_architectures(library, lipo, runner=runner),
                ("arm64", "x86_64"),
            )
            self.assertEqual(commands, [[str(lipo), "-archs", str(library)]])


class NativeSymbolContractCliTests(unittest.TestCase):
    def test_cli_applies_the_c_abi_contract(self) -> None:
        args = [
            "--contract",
            "c-abi",
            "--llvm-nm",
            "/toolchain/llvm-nm",
            "--external-only",
            "--architecture",
            "arm64",
            "--label",
            "Flutter iOS arm64",
            "/package/MermanFFI",
        ]

        with patch(
            "native_symbol_contract.read_defined_dynamic_symbols",
            return_value={"merman_get_native_api", "__cxa_finalize"},
        ) as reader, patch("sys.stdout"):
            self.assertEqual(main(args), 0)
        reader.assert_called_once_with(
            Path("/package/MermanFFI"),
            Path("/toolchain/llvm-nm"),
            external_only=True,
            architecture="arm64",
        )

        with (
            patch(
                "native_symbol_contract.read_defined_dynamic_symbols",
                return_value={"merman_get_native_api", "merman_render_svg"},
            ),
            patch("sys.stderr"),
        ):
            self.assertEqual(main(args), 1)


class NativeSymbolContractTests(unittest.TestCase):
    def test_c_abi_contract_canonicalizes_windows_import_aliases(self) -> None:
        assert_symbol_contract(
            {"merman_get_native_api", "__imp_merman_get_native_api"},
            C_ABI_SYMBOL_CONTRACT,
            label="Windows import library",
        )

        with self.assertRaisesRegex(
            RuntimeError,
            "unexpected Merman-owned symbols present: merman_render_svg",
        ):
            assert_symbol_contract(
                {"merman_get_native_api", "__imp_merman_render_svg"},
                C_ABI_SYMBOL_CONTRACT,
                label="Windows import library",
            )

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
