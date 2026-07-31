#!/usr/bin/env python3
"""Build a temporary Flutter Android app that depends on the local merman plugin."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path


PLUGIN_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = PLUGIN_ROOT.parents[1]
TARGET_TO_ABI = {
    "aarch64-linux-android": "arm64-v8a",
    "x86_64-linux-android": "x86_64",
}
ANDROID_PLUGIN_BUILD = PLUGIN_ROOT / "android" / "build.gradle"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--targets",
        nargs="+",
        default=["aarch64-linux-android", "x86_64-linux-android"],
        help="Android Rust targets to build before creating the smoke app.",
    )
    parser.add_argument(
        "--project-name",
        default="merman_smoke",
        help="Temporary Flutter project name.",
    )
    return parser.parse_args()


def run(args: list[str], *, cwd: Path | None = None) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, check=True)


def requested_abis(targets: list[str]) -> tuple[str, ...]:
    unsupported = sorted(set(targets) - TARGET_TO_ABI.keys())
    if unsupported:
        raise RuntimeError(
            "unsupported Flutter Android Rust targets: " + ", ".join(unsupported)
        )
    return tuple(TARGET_TO_ABI[target] for target in targets)


def verify_requested_native_libraries(
    targets: list[str],
    jni_libs: Path = PLUGIN_ROOT / "android" / "src" / "main" / "jniLibs",
) -> None:
    for abi in requested_abis(targets):
        library = jni_libs / abi / "libmerman_ffi.so"
        if not library.is_file():
            raise RuntimeError(f"Flutter Android native library was not built: {library}")


def android_plugin_min_sdk(build_file: Path = ANDROID_PLUGIN_BUILD) -> int:
    matches = re.findall(
        r"(?m)^\s*minSdk\s+(\d+)\s*$",
        build_file.read_text(encoding="utf-8"),
    )
    if len(matches) != 1:
        raise RuntimeError(
            f"expected one literal minSdk declaration in Flutter plugin build: {build_file}"
        )
    return int(matches[0])


def configure_android_consumer(project_root: Path) -> None:
    min_sdk = android_plugin_min_sdk()
    candidates = (
        (
            project_root / "android" / "app" / "build.gradle.kts",
            "minSdk = flutter.minSdkVersion",
            f"minSdk = {min_sdk}",
        ),
        (
            project_root / "android" / "app" / "build.gradle",
            "minSdkVersion flutter.minSdkVersion",
            f"minSdkVersion {min_sdk}",
        ),
    )
    existing = [candidate for candidate in candidates if candidate[0].is_file()]
    if len(existing) != 1:
        raise RuntimeError(
            "expected one generated Flutter Android app build file: "
            + ", ".join(str(path) for path, _, _ in candidates)
        )

    build_file, generated, configured = existing[0]
    contents = build_file.read_text(encoding="utf-8")
    if contents.count(generated) != 1:
        raise RuntimeError(
            f"generated Flutter Android minSdk declaration changed: {build_file}"
        )
    build_file.write_text(contents.replace(generated, configured), encoding="utf-8")


def write_smoke_main(path: Path) -> None:
    path.write_text(
        """import 'package:flutter/material.dart';
import 'package:merman/merman.dart';

void main() {
  runApp(const SmokeApp());
}

class SmokeApp extends StatelessWidget {
  const SmokeApp({super.key});

  @override
  Widget build(BuildContext context) {
    final version = Merman.open().packageVersion;
    return MaterialApp(
      home: Scaffold(
        body: Center(child: Text('merman $version')),
      ),
    );
  }
}
""",
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    temp_root = Path(tempfile.gettempdir()) / f"{args.project_name}-{uuid.uuid4().hex}"

    print("Building Android native slices for Flutter plugin smoke")
    run(
        [
            sys.executable,
            str(REPO_ROOT / "platforms" / "android" / "build-android.py"),
            "--artifact-profile",
            "flutter-android-native",
            "--targets",
            *args.targets,
        ],
        cwd=REPO_ROOT,
    )
    verify_requested_native_libraries(args.targets)

    print(f"Creating temporary Flutter app: {temp_root}")
    run(
        [
            "flutter",
            "create",
            "--platforms",
            "android",
            "--project-name",
            args.project_name,
            str(temp_root),
        ]
    )
    configure_android_consumer(temp_root)

    pubspec = temp_root / "pubspec.yaml"
    with pubspec.open("a", encoding="utf-8") as handle:
        handle.write(
            f"""

dependency_overrides:
  merman:
    path: {PLUGIN_ROOT.as_posix()}
"""
        )

    write_smoke_main(temp_root / "lib" / "main.dart")
    run(["flutter", "pub", "get"], cwd=temp_root)
    run(["flutter", "build", "apk", "--debug"], cwd=temp_root)
    print(f"Flutter Android smoke app built at {temp_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
