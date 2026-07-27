#!/usr/bin/env python3
"""Inspect native dynamic exports and enforce transport symbol boundaries."""

from __future__ import annotations

from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path
import subprocess


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


def canonicalize_owned_symbol(symbol: str) -> str:
    """Canonicalize leading decoration only when the remainder is Merman-owned."""
    candidate = symbol.lstrip("_")
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
        if len(fields) < 2 or len(fields[1]) != 1:
            malformed.append(line_number)
            continue
        symbols.add(canonicalize_owned_symbol(fields[0]))
    if malformed:
        rendered = ", ".join(str(line) for line in malformed)
        raise RuntimeError(f"llvm-nm emitted malformed symbol rows: {rendered}")
    if not symbols:
        raise RuntimeError("llvm-nm emitted no dynamic symbols")
    return symbols


def read_defined_dynamic_symbols(
    library: Path,
    llvm_nm: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> set[str]:
    """Read one built library's defined dynamic symbols with an explicit LLVM tool."""
    if not library.is_file():
        raise RuntimeError(f"native library does not exist: {library}")
    if not llvm_nm.is_file():
        raise RuntimeError(f"llvm-nm does not exist: {llvm_nm}")
    command = [str(llvm_nm)]
    if library.suffix == ".dylib":
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
