#!/usr/bin/env python3
"""Build a temporary Flutter Android app that depends on the local merman plugin."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path


PLUGIN_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = PLUGIN_ROOT.parents[1]


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
            "--targets",
            *args.targets,
            "--profile",
            "release",
        ],
        cwd=REPO_ROOT,
    )
    generated_jni_libs = REPO_ROOT / "platforms" / "android" / "src" / "main" / "jniLibs"
    plugin_jni_libs = PLUGIN_ROOT / "android" / "src" / "main" / "jniLibs"
    shutil.copytree(generated_jni_libs, plugin_jni_libs, dirs_exist_ok=True)

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
