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


EXACT_ENVIRONMENT_OVERRIDES = frozenset(
    {
        "AR",
        "ARFLAGS",
        "CC",
        "CFLAGS",
        "CLANG_PATH",
        "COMPILER_PATH",
        "CPP",
        "CPPFLAGS",
        "CPATH",
        "CXX",
        "CXXFLAGS",
        "CARGO",
        "CARGO_BUILD_INCREMENTAL",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_BUILD_TARGET_DIR",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_INCREMENTAL",
        "CARGO_HOST_CONFIG",
        "CARGO_TARGET_DIR",
        "CPLUS_INCLUDE_PATH",
        "C_INCLUDE_PATH",
        "DEVELOPER_DIR",
        "DYLD_FALLBACK_FRAMEWORK_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "HOST_AR",
        "HOST_CC",
        "HOST_CFLAGS",
        "HOST_CXX",
        "HOST_CXXFLAGS",
        "HOST_LD",
        "HOST_LDFLAGS",
        "LD",
        "LDFLAGS",
        "LIBCLANG_PATH",
        "LIBRARY_PATH",
        "MACOSX_DEPLOYMENT_TARGET",
        "NM",
        "OBJC",
        "OBJCFLAGS",
        "OBJCXX",
        "OBJCXXFLAGS",
        "OBJC_INCLUDE_PATH",
        "RANLIB",
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "RUSTUP_TOOLCHAIN",
        "SDKROOT",
        "SOURCE_DATE_EPOCH",
        "STRIP",
        "TARGET_AR",
        "TARGET_CC",
        "TARGET_CFLAGS",
        "TARGET_CXX",
        "TARGET_CXXFLAGS",
        "TARGET_LD",
        "TARGET_LDFLAGS",
        "TOOLCHAINS",
        "ZERO_AR_DATE",
    }
)

ENVIRONMENT_OVERRIDE_PREFIXES = (
    "AR_",
    "CC_",
    "CFLAGS_",
    "CXX_",
    "CXXFLAGS_",
    "CARGO_PROFILE_",
    "CARGO_BUILD_",
    "CARGO_TARGET_",
    "BINDGEN_EXTRA_CLANG_ARGS",
    "CMAKE_",
    "MESON_",
    "PKG_CONFIG",
    "LD_",
    "LDFLAGS_",
)

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


def reject_ffi_contract_environment(
    environment: Mapping[str, str] | None = None,
) -> None:
    values = os.environ if environment is None else environment
    overrides = sorted(
        key
        for key, value in values.items()
        if (
            key in CANONICAL_REPRODUCIBILITY_ENVIRONMENT
            and value != CANONICAL_REPRODUCIBILITY_ENVIRONMENT[key]
        )
        or (
            key not in CANONICAL_REPRODUCIBILITY_ENVIRONMENT
            and (
                key in EXACT_ENVIRONMENT_OVERRIDES
                or key.startswith(ENVIRONMENT_OVERRIDE_PREFIXES)
            )
        )
    )
    if overrides:
        raise FfiContractReproducibilityError(
            "FFI contract verification rejects environment overrides: "
            + ", ".join(overrides)
        )


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
            "CARGO_NET_OFFLINE": "true",
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
