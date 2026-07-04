#!/usr/bin/env python3
"""Run local platform binding verification gates."""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
FLUTTER_ROOT = REPO_ROOT / "platforms" / "flutter"
ANDROID_ROOT = REPO_ROOT / "platforms" / "android"
APPLE_ROOT = REPO_ROOT / "platforms" / "apple"
ANDROID_JAR_OUT = REPO_ROOT / "target" / "platforms" / "android" / "merman-android.jar"
FLUTTER_JAR_OUT = REPO_ROOT / "target" / "platforms" / "flutter" / "merman-flutter-android-plugin.jar"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-android-slices", action="store_true")
    parser.add_argument("--run-flutter-android-smoke", action="store_true")
    parser.add_argument("--run-android-gradle-build", action="store_true")
    parser.add_argument("--gradle-path", default=os.environ.get("MERMAN_GRADLE"))
    parser.add_argument(
        "--build-apple-xcframework",
        action="store_true",
        help="Build the Apple XCFramework after scaffold checks. Requires macOS/Xcode.",
    )
    parser.add_argument(
        "--apple-platform",
        choices=["all", "ios", "macos"],
        default="all",
        help="Apple platforms to pass to scripts/build-apple-xcframework.sh.",
    )
    return parser.parse_args()


def step(name: str) -> None:
    print()
    print(f"==> {name}")


def run(args: list[str], *, cwd: Path = REPO_ROOT) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, check=True)


def require_command(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise RuntimeError(f"{name} not found on PATH")
    return path


def bash_path(path: Path) -> str:
    resolved = path.resolve()
    if os.name == "nt":
        drive = resolved.drive.rstrip(":").lower()
        parts = [part for part in resolved.parts[1:]]
        if drive:
            return "/mnt/" + drive + "/" + "/".join(parts)
    return str(resolved)


def resolve_gradle_command(path: str | None) -> str:
    if path:
        resolved = Path(path).expanduser().resolve()
        if resolved.is_dir():
            gradle_bat = resolved / "gradle.bat"
            if gradle_bat.exists():
                return str(gradle_bat)
            gradle = resolved / "gradle"
            if gradle.exists():
                return str(gradle)
            raise RuntimeError(f"Gradle executable not found under: {resolved}")
        if not resolved.exists():
            raise RuntimeError(f"Gradle executable not found: {resolved}")
        return str(resolved)

    gradle = shutil.which("gradle")
    if not gradle:
        raise RuntimeError("gradle not found. Pass --gradle-path or set MERMAN_GRADLE.")
    return gradle


def host_dynamic_library() -> Path:
    system = platform.system()
    if system == "Windows":
        return REPO_ROOT / "target" / "debug" / "merman_ffi.dll"
    if system == "Darwin":
        return REPO_ROOT / "target" / "debug" / "libmerman_ffi.dylib"
    return REPO_ROOT / "target" / "debug" / "libmerman_ffi.so"


def flutter_android_embedding_jar() -> Path:
    flutter_root_env = os.environ.get("FLUTTER_ROOT")
    candidates: list[Path] = []
    if flutter_root_env:
        candidates.append(Path(flutter_root_env) / "bin" / "cache" / "artifacts" / "engine" / "android-arm64" / "flutter.jar")

    flutter = shutil.which("flutter")
    if flutter:
        flutter_bin = Path(flutter).resolve().parent
        candidates.append(flutter_bin.parent / "bin" / "cache" / "artifacts" / "engine" / "android-arm64" / "flutter.jar")

    for candidate in candidates:
        if candidate.exists():
            return candidate

    raise RuntimeError("Flutter Android embedding jar not found. Set FLUTTER_ROOT or run flutter doctor.")


def apple_build_args(apple_platform: str) -> list[str]:
    args = ["bash", "scripts/build-apple-xcframework.sh"]
    if apple_platform == "ios":
        args.append("--ios")
    elif apple_platform == "macos":
        args.append("--macos")
    return args


def main() -> int:
    args = parse_args()

    try:
        step("Rust FFI host tests")
        run(["cargo", "nextest", "run", "-p", "merman-ffi"])

        step("Android Rust target check")
        run(["rustup", "target", "add", "aarch64-linux-android"])
        run(["cargo", "check", "-p", "merman-ffi", "--target", "aarch64-linux-android"])
        run(
            [
                "cargo",
                "clippy",
                "--no-deps",
                "-p",
                "merman-ffi",
                "--target",
                "aarch64-linux-android",
                "--",
                "-D",
                "warnings",
            ]
        )

        step("Android Kotlin wrapper compile")
        kotlinc = require_command("kotlinc")
        ANDROID_JAR_OUT.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                kotlinc,
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanException.kt"),
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanTextMeasureRequest.kt"),
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanTextMeasureResult.kt"),
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanTextMeasurer.kt"),
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanEngine.kt"),
                str(ANDROID_ROOT / "src" / "main" / "kotlin" / "io" / "merman" / "MermanReusableEngine.kt"),
                "-d",
                str(ANDROID_JAR_OUT),
            ]
        )

        if args.build_android_slices:
            step("Android native slices")
            run(
                [
                    sys.executable,
                    str(ANDROID_ROOT / "build-android.py"),
                    "--targets",
                    "aarch64-linux-android",
                    "x86_64-linux-android",
                    "--profile",
                    "release",
                ]
            )

        step("Flutter/Dart package checks")
        flutter = require_command("flutter")
        dart = require_command("dart")
        run([flutter, "pub", "get"], cwd=FLUTTER_ROOT)
        run([flutter, "analyze"], cwd=FLUTTER_ROOT)
        run([dart, "format", "--set-exit-if-changed", "lib", "example", "tool"], cwd=FLUTTER_ROOT)
        run([dart, "run", "tool/callback_transaction_test.dart"], cwd=FLUTTER_ROOT)

        step("Flutter Android plugin Kotlin compile")
        flutter_jar = flutter_android_embedding_jar()
        FLUTTER_JAR_OUT.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                kotlinc,
                str(FLUTTER_ROOT / "android" / "src" / "main" / "kotlin" / "io" / "merman" / "flutter" / "MermanFlutterPlugin.kt"),
                "-classpath",
                str(flutter_jar),
                "-d",
                str(FLUTTER_JAR_OUT),
            ]
        )

        step("Flutter native packaging scaffold checks")
        bash = require_command("bash")
        for path in [
            FLUTTER_ROOT / "build-ios.sh",
            FLUTTER_ROOT / "build-desktop.sh",
            FLUTTER_ROOT / "ios" / "merman.podspec",
            FLUTTER_ROOT
            / "ios"
            / "merman"
            / "Sources"
            / "merman"
            / "MermanFlutterPlugin.swift",
            FLUTTER_ROOT / "macos" / "merman.podspec",
            FLUTTER_ROOT
            / "macos"
            / "merman"
            / "Sources"
            / "merman"
            / "MermanFlutterPlugin.swift",
            FLUTTER_ROOT / "linux" / "CMakeLists.txt",
            FLUTTER_ROOT / "linux" / "include" / "merman" / "merman_flutter_plugin.h",
            FLUTTER_ROOT / "windows" / "CMakeLists.txt",
            FLUTTER_ROOT / "windows" / "include" / "merman" / "merman_flutter_plugin_c_api.h",
        ]:
            if not path.exists():
                raise RuntimeError(f"required Flutter packaging file not found: {path}")
        run([bash, "-n", bash_path(FLUTTER_ROOT / "build-ios.sh")])
        run([bash, "-n", bash_path(FLUTTER_ROOT / "build-desktop.sh")])

        step("Dart FFI native smoke")
        run(["cargo", "build", "-p", "merman-ffi"])
        run([dart, "run", "example/smoke.dart", str(host_dynamic_library())], cwd=FLUTTER_ROOT)

        if args.run_android_gradle_build:
            arm64_lib = ANDROID_ROOT / "src" / "main" / "jniLibs" / "arm64-v8a" / "libmerman_ffi.so"
            x64_lib = ANDROID_ROOT / "src" / "main" / "jniLibs" / "x86_64" / "libmerman_ffi.so"
            if not arm64_lib.exists() or not x64_lib.exists():
                step("Android native slices for Gradle")
                run(
                    [
                        sys.executable,
                        str(ANDROID_ROOT / "build-android.py"),
                        "--targets",
                        "aarch64-linux-android",
                        "x86_64-linux-android",
                        "--profile",
                        "release",
                    ]
                )

            step("Android Gradle library assemble")
            gradle = resolve_gradle_command(args.gradle_path)
            run([gradle, "-p", str(ANDROID_ROOT), "assembleRelease", "--stacktrace"])

        step("Apple Swift package scaffold checks")
        for path in [
            REPO_ROOT / "Package.swift",
            REPO_ROOT / "scripts" / "build-apple-xcframework.sh",
            REPO_ROOT / "platforms" / "ios" / "build-ios.sh",
            APPLE_ROOT / "Sources" / "Merman" / "MermanEngine.swift",
            REPO_ROOT / "crates" / "merman-ffi" / "include" / "merman.h",
        ]:
            if not path.exists():
                raise RuntimeError(f"required Apple binding file not found: {path}")
        run([bash, "-n", bash_path(REPO_ROOT / "scripts" / "build-apple-xcframework.sh")])
        run([bash, "-n", bash_path(REPO_ROOT / "platforms" / "ios" / "build-ios.sh")])

        if args.build_apple_xcframework:
            if platform.system() != "Darwin":
                raise RuntimeError("--build-apple-xcframework requires macOS")
            step("Apple XCFramework build")
            run(apple_build_args(args.apple_platform))

        if args.run_flutter_android_smoke:
            step("Flutter Android APK packaging smoke")
            run(
                [
                    sys.executable,
                    str(FLUTTER_ROOT / "tool" / "android-smoke.py"),
                    "--targets",
                    "aarch64-linux-android",
                ]
            )

        print()
        print("Platform binding verification completed.")
        return 0
    except subprocess.CalledProcessError as exc:
        print(f"command failed with exit code {exc.returncode}: {' '.join(exc.cmd)}", file=sys.stderr)
        return exc.returncode
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
