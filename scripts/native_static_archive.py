#!/usr/bin/env python3
"""Stage native static libraries without delivery-only embedded LLVM bitcode."""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if __package__:
    from .artifact_profile_recipe import REPO_ROOT, rustc_host_target
else:
    from artifact_profile_recipe import REPO_ROOT, rustc_host_target


def llvm_objcopy() -> Path:
    sysroot = subprocess.run(
        ["rustc", "--print", "sysroot"], cwd=REPO_ROOT,
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    host = rustc_host_target()
    executable = "llvm-objcopy.exe" if "windows" in host else "llvm-objcopy"
    tool = Path(sysroot) / "lib" / "rustlib" / host / "bin" / executable
    if not tool.is_file():
        raise RuntimeError(
            f"missing pinned Rust toolchain tool: {tool}; "
            "install it with: rustup component add llvm-tools-preview"
        )
    return tool


def stage_static_library(source: Path, destination: Path, target: str) -> None:
    """Preserve the build output and publish a cleaned distribution copy."""
    source = Path(source)
    destination = Path(destination)
    if source.resolve() == destination.resolve():
        raise ValueError("static library staging requires a destination separate from the source")
    destination.parent.mkdir(parents=True, exist_ok=True)
    if "windows" in target:
        # Pinned objcopy rejects the short import members required by COFF archives.
        shutil.copyfile(source, destination)
        return
    sections = ("__LLVM,__bitcode", "__LLVM,__cmdline") if "apple" in target else (".llvmbc", ".llvmcmd")
    try:
        tool = llvm_objcopy()
        with tempfile.TemporaryDirectory(prefix=".native-static-", dir=destination.parent) as temporary:
            staged = Path(temporary) / destination.name
            subprocess.run(
                [str(tool), *(f"--remove-section={section}" for section in sections),
                 str(source), str(staged)],
                check=True, capture_output=True, text=True,
            )
            staged.replace(destination)
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or str(error)).strip()
        raise RuntimeError(f"static library staging failed for {source}: {detail}") from error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    try:
        stage_static_library(args.source, args.output, args.target)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"static library staging failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
