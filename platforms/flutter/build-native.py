#!/usr/bin/env python3
"""Build and package precompiled native assets for the Flutter package."""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from artifact_profile_recipe import cargo_build_args, load_artifact_profile
from native_symbol_contract import (
    C_ABI_SYMBOL_CONTRACT,
    assert_symbol_contract,
    read_defined_dynamic_symbols,
)


DESKTOP_RECIPE = load_artifact_profile("flutter-desktop-native")
IOS_RECIPE = load_artifact_profile("flutter-ios-native")
LIBRARY_STEM = DESKTOP_RECIPE.target_name.replace("-", "_")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode",
        choices=("host", "all-apple", "all-desktop"),
        nargs="?",
        default="host",
    )
    return parser.parse_args()


def run(args: list[str]) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=REPO_ROOT, check=True)


def llvm_nm() -> Path:
    candidates = []
    if resolved := shutil.which("llvm-nm"):
        candidates.append(Path(resolved))
    if shutil.which("xcrun"):
        completed = subprocess.run(
            ["xcrun", "--find", "llvm-nm"],
            capture_output=True,
            text=True,
            check=False,
        )
        if completed.returncode == 0 and completed.stdout.strip():
            candidates.append(Path(completed.stdout.strip()))
    rustc = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, check=True
    ).stdout
    host = next(
        line.removeprefix("host: ")
        for line in rustc.splitlines()
        if line.startswith("host: ")
    )
    sysroot = Path(
        subprocess.run(
            ["rustc", "--print", "sysroot"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    )
    candidates.append(sysroot / "lib" / "rustlib" / host / "bin" / "llvm-nm")
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise RuntimeError("llvm-nm not found; install the Rust llvm-tools-preview component")


def build(recipe, target: str, *, zig: bool = False) -> Path:
    run(["rustup", "target", "add", target])
    tool = "cargo-zigbuild" if zig else "cargo"
    run(cargo_build_args(recipe, locked=True, target=target, build_tool=tool))
    filename = native_filename(target)
    artifact = REPO_ROOT / "target" / target / recipe.cargo_profile / filename
    if not artifact.is_file():
        raise RuntimeError(f"native artifact was not produced: {artifact}")
    symbol_artifact = artifact
    external_only = None
    if "windows" in target:
        symbol_artifact = artifact.with_name(f"lib{LIBRARY_STEM}.dll.a")
        if not symbol_artifact.is_file():
            raise RuntimeError(f"Windows import library was not produced: {symbol_artifact}")
        external_only = True
    assert_symbol_contract(
        read_defined_dynamic_symbols(
            symbol_artifact,
            llvm_nm(),
            external_only=external_only,
        ),
        C_ABI_SYMBOL_CONTRACT,
        label=f"{recipe.profile_id} {target}",
    )
    return artifact


def native_filename(target: str) -> str:
    if "windows" in target:
        return f"{LIBRARY_STEM}.dll"
    if "apple" in target:
        return f"lib{LIBRARY_STEM}.dylib"
    return f"lib{LIBRARY_STEM}.so"


def package(artifact: Path, relative: str) -> None:
    destination = PACKAGE_ROOT / "native" / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(artifact, destination)
    if destination.suffix == ".dylib":
        run(
            [
                "install_name_tool",
                "-id",
                f"@rpath/{destination.name}",
                str(destination),
            ]
        )
        run(["codesign", "--force", "--sign", "-", str(destination)])
    print(f"Packaged {destination.relative_to(PACKAGE_ROOT)}")


def build_macos(target: str, arch: str) -> None:
    package(build(DESKTOP_RECIPE, target), f"macos/{arch}/libmerman_ffi.dylib")


def build_ios(target: str, slice_name: str) -> None:
    package(build(IOS_RECIPE, target), f"ios/{slice_name}/libmerman_ffi.dylib")


def build_linux(target: str, arch: str) -> None:
    package(
        build(DESKTOP_RECIPE, target, zig=True),
        f"linux/{arch}/libmerman_ffi.so",
    )


def build_windows() -> None:
    package(
        build(DESKTOP_RECIPE, "x86_64-pc-windows-gnu", zig=True),
        "windows/x86_64/merman_ffi.dll",
    )


def build_host() -> None:
    system = platform.system()
    machine = platform.machine().lower()
    arch = "arm64" if machine in {"arm64", "aarch64"} else "x86_64"
    if system == "Darwin":
        target = "aarch64-apple-darwin" if arch == "arm64" else "x86_64-apple-darwin"
        build_macos(target, arch)
    elif system == "Linux":
        target = "aarch64-unknown-linux-gnu" if arch == "arm64" else "x86_64-unknown-linux-gnu"
        package(build(DESKTOP_RECIPE, target), f"linux/{arch}/libmerman_ffi.so")
    elif system == "Windows" and arch == "x86_64":
        artifact = build(DESKTOP_RECIPE, "x86_64-pc-windows-gnu")
        package(artifact, "windows/x86_64/merman_ffi.dll")
    else:
        raise RuntimeError(f"unsupported Flutter host: {system}/{machine}")


def main() -> int:
    args = parse_args()
    try:
        if args.mode == "host":
            build_host()
        if args.mode in {"all-apple", "all-desktop"}:
            if platform.system() != "Darwin":
                raise RuntimeError(f"{args.mode} requires macOS")
            build_macos("aarch64-apple-darwin", "arm64")
            build_macos("x86_64-apple-darwin", "x86_64")
            build_ios("aarch64-apple-ios", "arm64")
            build_ios("aarch64-apple-ios-sim", "arm64-simulator")
            build_ios("x86_64-apple-ios", "x86_64-simulator")
        if args.mode == "all-desktop":
            build_linux("aarch64-unknown-linux-gnu", "aarch64")
            build_linux("x86_64-unknown-linux-gnu", "x86_64")
            build_windows()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
