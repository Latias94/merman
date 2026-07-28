#!/usr/bin/env python3
"""Inspect native dynamic exports and enforce transport symbol boundaries."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path
import shutil
import subprocess
import sys


CommandRunner = Callable[..., subprocess.CompletedProcess[str]]


@dataclass(frozen=True)
class NativeSymbolContract:
    required: frozenset[str]
    forbidden: frozenset[str]
    owned_prefixes: tuple[str, ...]
    allowed_owned: frozenset[str]


ANDROID_JNI_SYMBOL_CONTRACT = NativeSymbolContract(
    required=frozenset({"JNI_OnLoad"}),
    forbidden=frozenset({"merman_get_native_api"}),
    owned_prefixes=("merman_", "Java_io_merman_"),
    allowed_owned=frozenset(),
)
C_ABI_SYMBOL_CONTRACT = NativeSymbolContract(
    required=frozenset({"merman_get_native_api"}),
    forbidden=frozenset({"JNI_OnLoad"}),
    owned_prefixes=("merman_",),
    allowed_owned=frozenset({"merman_get_native_api"}),
)
SYMBOL_CONTRACTS = {
    "android-jni": ANDROID_JNI_SYMBOL_CONTRACT,
    "c-abi": C_ABI_SYMBOL_CONTRACT,
}


def canonicalize_owned_symbol(symbol: str) -> str:
    """Canonicalize leading decoration only when the remainder is Merman-owned."""
    candidate = (
        symbol.removeprefix("__imp_").lstrip("_")
        if symbol.startswith("__imp_")
        else symbol.lstrip("_")
    )
    if (
        candidate == "JNI_OnLoad"
        or candidate.startswith("merman_")
        or candidate.startswith("Java_io_merman_")
    ):
        return candidate
    return symbol


def parse_llvm_nm_posix(output: str) -> set[str]:
    """Parse `llvm-nm --format=posix` output without guessing malformed rows."""
    symbols: set[str] = set()
    malformed: list[int] = []
    for line_number, line in enumerate(output.splitlines(), start=1):
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) == 1 and fields[0].endswith(":"):
            continue
        if len(fields) < 2 or len(fields[1]) != 1:
            malformed.append(line_number)
            continue
        symbols.add(canonicalize_owned_symbol(fields[0]))
    if malformed:
        rendered = ", ".join(str(line) for line in malformed)
        raise RuntimeError(f"llvm-nm emitted malformed symbol rows: {rendered}")
    if not symbols:
        raise RuntimeError("llvm-nm emitted no symbols")
    return symbols


def read_defined_dynamic_symbols(
    library: Path,
    llvm_nm: Path,
    *,
    external_only: bool | None = None,
    architecture: str | None = None,
    runner: CommandRunner = subprocess.run,
) -> set[str]:
    """Read one artifact's ABI-visible definitions with an explicit LLVM tool."""
    if not library.is_file():
        raise RuntimeError(f"native library does not exist: {library}")
    if not llvm_nm.is_file():
        raise RuntimeError(f"llvm-nm does not exist: {llvm_nm}")
    command = [str(llvm_nm)]
    if architecture is not None:
        command.append(f"--arch={architecture}")
    use_external_symbols = (
        library.suffix in {".a", ".dylib", ".lib"}
        if external_only is None
        else external_only
    )
    if use_external_symbols:
        command.append("--extern-only")
    else:
        command.append("--dynamic")
    command.extend(
        [
            "--defined-only",
            "--format=posix",
            str(library),
        ]
    )
    completed = runner(
        command,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no diagnostic"
        raise RuntimeError(
            f"llvm-nm failed for {library} with exit {completed.returncode}: {detail}"
        )
    return parse_llvm_nm_posix(completed.stdout)


def read_macho_architectures(
    library: Path,
    lipo: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> tuple[str, ...]:
    """Read every architecture carried by a Mach-O artifact."""
    if not library.is_file():
        raise RuntimeError(f"native library does not exist: {library}")
    if not lipo.is_file():
        raise RuntimeError(f"lipo does not exist: {lipo}")
    command = [str(lipo), "-archs", str(library)]
    completed = runner(
        command,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no diagnostic"
        raise RuntimeError(
            f"lipo failed for {library} with exit {completed.returncode}: {detail}"
        )
    architectures = tuple(completed.stdout.split())
    if (
        not architectures
        or len(set(architectures)) != len(architectures)
        or any(
            not all(character.isalnum() or character in {"_", "-"} for character in item)
            for item in architectures
        )
    ):
        raise RuntimeError(
            f"lipo emitted invalid architectures for {library}: {completed.stdout!r}"
        )
    return architectures


def assert_symbol_contract(
    symbols: Iterable[str],
    contract: NativeSymbolContract,
    *,
    label: str,
) -> None:
    """Fail closed when a transport lacks its entry point or exposes the other transport."""
    observed = {canonicalize_owned_symbol(symbol) for symbol in symbols}
    missing = sorted(contract.required - observed)
    forbidden = sorted(contract.forbidden & observed)
    unexpected_owned = sorted(
        symbol
        for symbol in observed
        if symbol.startswith(contract.owned_prefixes)
        and symbol not in contract.allowed_owned
    )
    failures: list[str] = []
    if missing:
        failures.append("missing required symbols: " + ", ".join(missing))
    if forbidden:
        failures.append("forbidden symbols present: " + ", ".join(forbidden))
    if unexpected_owned:
        failures.append(
            "unexpected Merman-owned symbols present: " + ", ".join(unexpected_owned)
        )
    if failures:
        raise RuntimeError(f"{label} symbol contract failed: {'; '.join(failures)}")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--contract",
        choices=sorted(SYMBOL_CONTRACTS),
        required=True,
        help="Named transport symbol contract to enforce.",
    )
    parser.add_argument(
        "--llvm-nm",
        type=Path,
        required=True,
        help="Explicit llvm-nm executable used to inspect the artifact.",
    )
    parser.add_argument(
        "--external-only",
        action="store_true",
        default=None,
        help="Inspect external definitions instead of an ELF dynamic symbol table.",
    )
    architecture = parser.add_mutually_exclusive_group()
    architecture.add_argument(
        "--architecture",
        help="Inspect one architecture in a Mach-O universal binary.",
    )
    architecture.add_argument(
        "--all-macho-architectures",
        action="store_true",
        help="Inspect every architecture reported by lipo.",
    )
    parser.add_argument(
        "--lipo",
        type=Path,
        help="Explicit lipo executable. Defaults to the command on PATH.",
    )
    parser.add_argument(
        "--label",
        help="Diagnostic label. Defaults to the artifact path.",
    )
    parser.add_argument("artifact", type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    label = args.label or str(args.artifact)
    try:
        if args.lipo is not None and not args.all_macho_architectures:
            raise RuntimeError("--lipo requires --all-macho-architectures")
        if args.all_macho_architectures:
            lipo = args.lipo
            if lipo is None:
                resolved = shutil.which("lipo")
                if resolved is None:
                    raise RuntimeError(
                        "lipo is required for --all-macho-architectures"
                    )
                lipo = Path(resolved)
            architectures: tuple[str | None, ...] = read_macho_architectures(
                args.artifact,
                lipo,
            )
        else:
            architectures = (args.architecture,)

        for architecture_name in architectures:
            architecture_label = (
                f"{label} ({architecture_name})"
                if architecture_name is not None
                else label
            )
            symbols = read_defined_dynamic_symbols(
                args.artifact,
                args.llvm_nm,
                external_only=(
                    True if args.all_macho_architectures else args.external_only
                ),
                architecture=architecture_name,
            )
            assert_symbol_contract(
                symbols,
                SYMBOL_CONTRACTS[args.contract],
                label=architecture_label,
            )
    except RuntimeError as error:
        print(error, file=sys.stderr)
        return 1
    print(f"Verified native symbol contract: {label}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
