#!/usr/bin/env python3
"""Build merman Android native slices and copy them into jniLibs."""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
JNI_LIBS = Path(__file__).resolve().parent / "src" / "main" / "jniLibs"

TARGET_TO_ABI = {
    "aarch64-linux-android": "arm64-v8a",
    "x86_64-linux-android": "x86_64",
    "armv7-linux-androideabi": "armeabi-v7a",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--targets",
        nargs="+",
        default=["aarch64-linux-android", "x86_64-linux-android"],
        help="Rust Android targets to build. Defaults to arm64 and x86_64.",
    )
    parser.add_argument(
        "--profile",
        default="release",
        choices=["debug", "release"],
        help="Cargo profile to build. Defaults to release.",
    )
    parser.add_argument(
        "--ndk-home",
        default=os.environ.get("ANDROID_NDK_HOME"),
        help="Android NDK path. Defaults to ANDROID_NDK_HOME or latest under ANDROID_HOME/ANDROID_SDK_ROOT.",
    )
    return parser.parse_args()


def run(args: list[str], *, cwd: Path = REPO_ROOT, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, env=env, check=True)


def ndk_host_tag() -> str:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Windows":
        return "windows-x86_64"
    if system == "Linux":
        return "linux-x86_64"
    if system == "Darwin":
        if machine in {"arm64", "aarch64"}:
            # Modern NDKs ship darwin-x86_64 prebuilt tools that run on Apple Silicon.
            return "darwin-x86_64"
        return "darwin-x86_64"
    raise RuntimeError(f"unsupported host platform for Android NDK: {system}")


def clang_name(target: str) -> str:
    api = "23"
    if target == "aarch64-linux-android":
        base = f"aarch64-linux-android{api}-clang"
    elif target == "x86_64-linux-android":
        base = f"x86_64-linux-android{api}-clang"
    elif target == "armv7-linux-androideabi":
        base = f"armv7a-linux-androideabi{api}-clang"
    else:
        raise RuntimeError(f"unsupported Android Rust target: {target}")
    if platform.system() == "Windows":
        return f"{base}.cmd"
    return base


def default_ndk_home(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit).expanduser().resolve()

    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        raise RuntimeError("ANDROID_NDK_HOME or ANDROID_HOME/ANDROID_SDK_ROOT must be set")

    ndk_root = Path(sdk).expanduser().resolve() / "ndk"
    candidates = sorted((path for path in ndk_root.iterdir() if path.is_dir()), reverse=True)
    if not candidates:
        raise RuntimeError(f"no Android NDK installation found under {ndk_root}")
    return candidates[0]


def clang_for_target(target: str, ndk: Path) -> Path:
    clang = ndk / "toolchains" / "llvm" / "prebuilt" / ndk_host_tag() / "bin" / clang_name(target)
    if not clang.exists():
        raise RuntimeError(f"Android clang not found: {clang}")
    return clang


def cargo_env_with_linker(target: str, clang: Path) -> dict[str, str]:
    env = os.environ.copy()
    env_name = f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"
    env[env_name] = str(clang)
    return env


def build_target(target: str, profile_name: str, ndk: Path) -> None:
    abi = TARGET_TO_ABI.get(target)
    if abi is None:
        raise RuntimeError(f"unsupported Android Rust target: {target}")

    clang = clang_for_target(target, ndk)
    env = cargo_env_with_linker(target, clang)

    print(f"==> Building merman-ffi for {target} ({abi})")
    run(["rustup", "target", "add", target])
    cargo_args = [
        "cargo",
        "build",
        "-p",
        "merman-ffi",
        "--target",
        target,
        "--manifest-path",
        str(REPO_ROOT / "Cargo.toml"),
    ]
    if profile_name == "release":
        cargo_args.insert(2, "--release")
    run(cargo_args, env=env)

    profile_dir = "release" if profile_name == "release" else "debug"
    artifact = REPO_ROOT / "target" / target / profile_dir / "libmerman_ffi.so"
    if not artifact.exists():
        raise RuntimeError(f"expected Android library not found: {artifact}")

    dest = JNI_LIBS / abi
    dest.mkdir(parents=True, exist_ok=True)
    shutil.copy2(artifact, dest / "libmerman_ffi.so")
    print(f"Copied {abi} library to {dest}")


def main() -> int:
    args = parse_args()
    try:
        ndk = default_ndk_home(args.ndk_home)
        print(f"Using Android NDK: {ndk}")
        for target in args.targets:
            build_target(target, args.profile, ndk)
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
