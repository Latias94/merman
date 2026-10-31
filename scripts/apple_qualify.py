#!/usr/bin/env python3
"""Qualify an extracted Apple package and compare matched static distribution copies."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import zipfile

if __package__:
    from .artifact_profile_recipe import load_artifact_profiles
else:
    from artifact_profile_recipe import load_artifact_profiles

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))


def run(command, *, cwd: Path, **kwargs):
    print("RUN " + " ".join(str(x) for x in command), file=sys.stderr, flush=True)
    return subprocess.run([str(x) for x in command], cwd=cwd, check=True, **kwargs)


def compare_sizes(original: int, staged: int) -> dict:
    if original <= 0 or staged <= 0 or staged > original / 2:
        raise ValueError("matched XCFramework ZIP must reduce compressed payload by at least 50%")
    return {"original_compressed_bytes": original, "staged_compressed_bytes": staged,
            "reduction_fraction": 1 - staged / original}


def validate_consumer(path: Path, *, capabilities, outputs, version: str) -> dict:
    # Reuse the native/WASI SVG occurrence and glyph-path assertion.
    from platforms.wasi.common import validate_svg

    result = json.loads(path.read_text())
    catalog = result["catalog"]
    if (catalog["package_version"] != version
            or catalog["capabilities"]["capability_ids"] != list(capabilities)
            or catalog["capabilities"]["output_ids"] != list(outputs)):
        raise ValueError("installed Apple capability catalog does not match the descriptor")
    for name in ("basic", "math"):
        output = result[name]
        validate_svg(output["svg"].encode(), output["metadata"],
                     mathematical=True if name == "math" else None)
    if result["rejected_capability_ids"] != ["jpeg", "math", "pdf", "png"]:
        raise ValueError("installed Apple policy/codec controls did not complete")
    return result


def compile_consumer(package: Path, source: Path, work: Path, *, triple=None, sdk=None) -> Path:
    project = work / "consumer"
    project.mkdir(parents=True)
    # This consumer resolves only the extracted package. No source-tree dependency.
    (project / "Package.swift").write_text(
        '// swift-tools-version: 5.9\nimport PackageDescription\n'
        'let package = Package(name: "InstalledConsumer", platforms: [.macOS(.v12), .iOS(.v14)], '
        f'dependencies: [.package(name: "Merman", path: {json.dumps(str(package))})], '
        'targets: [.executableTarget(name: "InstalledConsumer", '
        'dependencies: [.product(name: "Merman", package: "Merman")], path: "Sources")])\n'
    )
    (project / "Sources").mkdir()
    shutil.copyfile(source, project / "Sources/InstalledConsumer.swift")
    args = ["swift", "build", "--package-path", project, "--configuration", "release", "--jobs", "2"]
    if triple:
        args += ["--triple", triple, "--sdk", sdk]
    run(args, cwd=work)
    binary_dir = run(args + ["--show-bin-path"], cwd=work, capture_output=True, text=True).stdout.strip()
    return Path(binary_dir) / "InstalledConsumer"


def qualify_package(*, repo_root: Path, package_dir: Path, work_dir: Path,
                    version: str, simulator: bool) -> dict:
    profile = next(p for p in load_artifact_profiles(repo_root / "capabilities/artifact-profiles-v2.json")
                   if p.profile_id == "apple-uniffi-native")
    work_dir.mkdir(parents=True, exist_ok=False)
    source = repo_root / "platforms/apple/tests/InstalledConsumer.swift"
    package = package_dir.resolve()
    binary = compile_consumer(package, source, work_dir / "macos")
    output = work_dir / "macos.json"
    with output.open("w") as stream:
        run([binary], cwd=work_dir, stdout=stream)
    result = validate_consumer(output, capabilities=profile.capabilities,
                               outputs=profile.outputs, version=version)
    report = {"macos_binary_bytes": binary.stat().st_size,
              "macos_math_sha256": hashlib.sha256(result["math"]["svg"].encode()).hexdigest()}
    device_sdk = run(["xcrun", "--sdk", "iphoneos", "--show-sdk-path"], cwd=work_dir,
                     capture_output=True, text=True).stdout.strip()
    compile_consumer(package, source, work_dir / "device", triple="arm64-apple-ios14.0", sdk=device_sdk)
    sim_sdk = run(["xcrun", "--sdk", "iphonesimulator", "--show-sdk-path"], cwd=work_dir,
                  capture_output=True, text=True).stdout.strip()
    host = run(["uname", "-m"], cwd=work_dir, capture_output=True, text=True).stdout.strip()
    arch = "arm64" if host == "arm64" else "x86_64"
    sim_binary = compile_consumer(package, source, work_dir / "simulator",
                                  triple=f"{arch}-apple-ios14.0-simulator", sdk=sim_sdk)
    if simulator:
        devices = json.loads(run(["xcrun", "simctl", "list", "devices", "available", "--json"],
                                  cwd=work_dir, capture_output=True, text=True).stdout)
        available = [d for runtime, entries in devices["devices"].items() if ".iOS-" in runtime
                     for d in entries if d.get("isAvailable")]
        if not available:
            raise RuntimeError("Apple qualification requires an installed iOS simulator runtime")
        selected = next((d for d in available if d["state"] == "Booted"), available[0])
        udid = selected["udid"]
        started = selected["state"] != "Booted"
        try:
            if started:
                run(["xcrun", "simctl", "boot", udid], cwd=work_dir)
                run(["xcrun", "simctl", "bootstatus", udid, "-b"], cwd=work_dir)
            sim_output = work_dir / "simulator.json"
            with sim_output.open("w") as stream:
                run(["xcrun", "simctl", "spawn", udid, sim_binary], cwd=work_dir, stdout=stream)
            validate_consumer(sim_output, capabilities=profile.capabilities,
                              outputs=profile.outputs, version=version)
            report["simulator_runtime"] = "passed"
        finally:
            if started:
                run(["xcrun", "simctl", "shutdown", udid], cwd=work_dir)
    else:
        report["simulator_runtime"] = "not-requested"
    return report


def compare_original(*, repo_root: Path, package_dir: Path, work_dir: Path,
                     staged_result: dict, version: str) -> dict:
    if __package__:
        from .apple_package import slice_targets
    else:
        from apple_package import slice_targets
    profile = next(p for p in load_artifact_profiles(repo_root / "capabilities/artifact-profiles-v2.json")
                   if p.profile_id == "apple-uniffi-native")
    framework = package_dir / "Merman.xcframework"
    info = plistlib.loads((framework / "Info.plist").read_bytes())
    work_dir.mkdir(parents=True, exist_ok=False)
    original_package = work_dir / "original-package"
    shutil.copytree(package_dir, original_package)
    original = original_package / "Merman.xcframework"
    for entry in info["AvailableLibraries"]:
        libraries = [repo_root / "target" / target / profile.cargo.cargo_profile /
                     ("lib" + profile.cargo.target_name + ".a") for target in slice_targets(entry)]
        destination = original / entry["LibraryIdentifier"] / entry["LibraryPath"]
        if len(libraries) == 1:
            shutil.copyfile(libraries[0], destination)
        else:
            run(["lipo", "-create", *libraries, "-output", destination], cwd=work_dir)
    sizes = []
    # Match the compression algorithm and file inventory, measuring only the XCF payload.
    for name, root in (("original", original), ("staged", framework)):
        archive = work_dir / f"{name}.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zipped:
            for path in sorted(root.rglob("*")):
                if path.is_file():
                    zipped.write(path, "Merman.xcframework/" + path.relative_to(root).as_posix())
        sizes.append(archive.stat().st_size)
    report = compare_sizes(*sizes)
    source = repo_root / "platforms/apple/tests/InstalledConsumer.swift"
    original_binary = compile_consumer(original_package, source, work_dir / "original-consumer")
    output = work_dir / "original.json"
    with output.open("w") as stream:
        run([original_binary], cwd=work_dir, stdout=stream)
    original_result = validate_consumer(output, capabilities=profile.capabilities,
                                       outputs=profile.outputs, version=version)
    if original_result != staged_result:
        raise ValueError("staged and original static-library consumer outputs differ")
    report["original_linked_macos_bytes"] = original_binary.stat().st_size
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-archive", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--simulator", action="store_true")
    parser.add_argument("--compare-original", action="store_true")
    args = parser.parse_args()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=False)
    extracted = work / "extracted"
    # Producer-owned archive; the packager restricts its inventory and paths.
    with zipfile.ZipFile(args.package_archive) as archive:
        archive.extractall(extracted)
    package = next(extracted.glob("*/Package.swift")).parent
    report = qualify_package(repo_root=REPO_ROOT, package_dir=package,
                             work_dir=work / "installed", version=args.version, simulator=args.simulator)
    if args.compare_original:
        report["matched_size"] = compare_original(repo_root=REPO_ROOT, package_dir=package,
                                                  work_dir=work / "comparison",
                                                  version=args.version,
                                                  staged_result=json.loads((work / "installed/macos.json").read_text()))
    (work / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
