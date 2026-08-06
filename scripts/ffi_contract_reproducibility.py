#!/usr/bin/env python3
"""Fail-closed local-environment checks for reproducible FFI evidence."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
from typing import Any, Callable, Sequence


PASSTHROUGH_ENVIRONMENT = (
    "CARGO_HOME",
    "HOME",
    "RUSTUP_HOME",
    "TEMP",
    "TMP",
    "TMPDIR",
)

CANONICAL_REPRODUCIBILITY_ENVIRONMENT = {
    "CARGO_INCREMENTAL": "0",
    "SOURCE_DATE_EPOCH": "0",
    "ZERO_AR_DATE": "1",
}


class FfiContractReproducibilityError(RuntimeError):
    """The local process environment cannot produce attributable evidence."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]


@dataclass(frozen=True)
class ExecutableIdentity:
    path: str
    sha256: str

    def projection(self) -> dict[str, str]:
        return {"path": self.path, "sha256": self.sha256}


@dataclass(frozen=True)
class RustToolchainIdentity:
    cargo: ExecutableIdentity
    rustc: ExecutableIdentity
    cargo_version: str
    rustc_verbose: str
    host_target: str

    def projection(self) -> dict[str, Any]:
        return {
            "cargo": self.cargo.projection(),
            "rustc": self.rustc.projection(),
            "cargo_version": self.cargo_version,
            "rustc_verbose": self.rustc_verbose,
            "host_target": self.host_target,
        }


def reject_cargo_configuration(
    repo_root: Path,
    *,
    environment: Mapping[str, str] | None = None,
    user_home: Path | None = None,
) -> None:
    values = os.environ if environment is None else environment
    resolved_repo = repo_root.resolve()
    search_roots = (resolved_repo, *resolved_repo.parents)
    default_cargo_home = (
        Path.home() if user_home is None else user_home.resolve()
    ) / ".cargo"
    cargo_homes = {default_cargo_home}
    configured_cargo_home = values.get("CARGO_HOME")
    if configured_cargo_home:
        cargo_home = Path(configured_cargo_home).expanduser()
        if not cargo_home.is_absolute():
            cargo_home = resolved_repo / cargo_home
        cargo_homes.add(cargo_home.resolve())
    candidates = {
        root / ".cargo" / name
        for root in search_roots
        for name in ("config", "config.toml")
    }
    candidates.update(
        cargo_home / name
        for cargo_home in cargo_homes
        for name in ("config", "config.toml")
    )
    observed = tuple(path for path in sorted(candidates) if path.exists())
    if observed:
        raise FfiContractReproducibilityError(
            "FFI contract verification rejects Cargo configuration: "
            + ", ".join(str(path) for path in observed)
        )


def ffi_contract_subprocess_environment(
    environment: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Build a small deterministic child environment after parent validation."""

    values = os.environ if environment is None else environment
    result = {
        key: values[key]
        for key in PASSTHROUGH_ENVIRONMENT
        if key in values and values[key]
    }
    result.update(CANONICAL_REPRODUCIBILITY_ENVIRONMENT)
    result.update(
        {
            "CARGO_TERM_COLOR": "never",
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "TERM": "dumb",
            "TZ": "UTC",
        }
    )
    return result


def resolve_rust_toolchain(runner: CommandRunner) -> RustToolchainIdentity:
    rustup = shutil.which("rustup")
    if rustup is not None:
        rustup_path = _invocable_executable(Path(rustup), "rustup")
        cargo_path = _rustup_tool_path(str(rustup_path), "cargo", runner)
        rustc_path = _rustup_tool_path(str(rustup_path), "rustc", runner)
    else:
        cargo_path = _path_tool("cargo")
        rustc_path = _path_tool("rustc")
    cargo = _executable_identity(cargo_path, "cargo")
    rustc = _executable_identity(rustc_path, "rustc")
    cargo_version = _checked_output((cargo.path, "-V"), runner)
    rustc_verbose = _checked_output((rustc.path, "-Vv"), runner)
    host_target = ""
    for line in rustc_verbose.splitlines():
        if line.startswith("host: "):
            host_target = line.removeprefix("host: ")
            break
    if not host_target:
        raise FfiContractReproducibilityError(
            "resolved rustc -Vv did not report a host target"
        )
    return RustToolchainIdentity(
        cargo=cargo,
        rustc=rustc,
        cargo_version=cargo_version,
        rustc_verbose=rustc_verbose,
        host_target=host_target,
    )


def rust_toolchain_provenance(runner: CommandRunner) -> dict[str, Any]:
    """Return the stable report projection for the resolved Rust toolchain."""

    return resolve_rust_toolchain(runner).projection()


def _rustup_tool_path(
    rustup: str,
    tool: str,
    runner: CommandRunner,
) -> Path:
    value = _checked_output((rustup, "which", tool), runner)
    return _canonical_executable(Path(value), tool)


def _path_tool(tool: str) -> Path:
    value = shutil.which(tool)
    if value is None:
        raise FfiContractReproducibilityError(f"could not resolve {tool} from PATH")
    return _canonical_executable(Path(value), tool)


def _invocable_executable(path: Path, label: str) -> Path:
    expanded = path.expanduser()
    absolute = expanded if expanded.is_absolute() else Path.cwd() / expanded
    if not absolute.is_file() or not os.access(absolute, os.X_OK):
        raise FfiContractReproducibilityError(
            f"resolved {label} is not an executable: {path}"
        )
    return absolute


def _canonical_executable(path: Path, label: str) -> Path:
    resolved = path.expanduser().resolve()
    if (
        not resolved.is_absolute()
        or resolved.is_symlink()
        or not resolved.is_file()
        or not os.access(resolved, os.X_OK)
    ):
        raise FfiContractReproducibilityError(
            f"resolved {label} is not a canonical executable: {path}"
        )
    return resolved


def _executable_identity(path: Path, label: str) -> ExecutableIdentity:
    try:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise FfiContractReproducibilityError(
            f"cannot hash resolved {label} executable {path}: {error}"
        ) from error
    return ExecutableIdentity(str(path), f"sha256:{digest}")


def _checked_output(command: Sequence[str], runner: CommandRunner) -> str:
    completed = runner(command)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise FfiContractReproducibilityError(
            f"toolchain command failed with {completed.returncode}: "
            f"{' '.join(command)}: {detail}"
        )
    if not isinstance(completed.stdout, str) or not completed.stdout.strip():
        raise FfiContractReproducibilityError(
            f"toolchain command produced no text: {' '.join(command)}"
        )
    return completed.stdout.strip()
