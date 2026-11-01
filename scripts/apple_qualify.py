#!/usr/bin/env python3
"""Qualify local or HTTPS SwiftPM delivery against the final staged Apple package."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
from urllib.parse import urlsplit
import zipfile

if __package__:
    from .apple_package import _tree_files, _validate_artifact_url
    from .artifact_profile_recipe import load_artifact_profiles
    from .release_archive import regular_files_equal
else:
    from apple_package import _tree_files, _validate_artifact_url
    from artifact_profile_recipe import load_artifact_profiles
    from release_archive import regular_files_equal

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


def validate_consumer(path: Path, *, capabilities, outputs, diagram_families, version: str) -> dict:
    from platforms.wasi.common import validate_svg

    result = json.loads(path.read_text())
    catalog = result["catalog"]
    if (catalog["package_version"] != version
            or catalog["capabilities"]["capability_ids"] != list(capabilities)
            or catalog["capabilities"]["output_ids"] != list(outputs)
            or catalog.get("registry", {}).get("diagram_families") != list(diagram_families)):
        raise ValueError("installed Apple capability catalog does not match the descriptor")
    for name in ("basic", "math"):
        output = result[name]
        validate_svg(output["svg"].encode(), output["metadata"],
                     mathematical=True if name == "math" else None)
        if name == "basic" and not all(label in output["svg"] for label in ("Hello", "World")):
            raise ValueError("installed ordinary SVG is missing its fixture labels")
    if result["rejected_capability_ids"] != ["jpeg", "math", "pdf", "png"]:
        raise ValueError("installed Apple policy/codec controls did not complete")
    return result


def validate_remote_dependency(url: str | None, revision: str | None) -> None:
    if url is None and revision is None:
        return
    try:
        parsed = urlsplit(url or "")
        valid_url = (parsed.scheme == "https" and parsed.hostname and parsed.path not in ("", "/")
                     and parsed.username is None and parsed.password is None
                     and "?" not in url and "#" not in url
                     and not any(character.isspace() or ord(character) < 32 for character in url))
        parsed.port
    except ValueError:
        valid_url = False
    if not valid_url or not re.fullmatch(r"[0-9a-f]{40}", revision or ""):
        raise ValueError("remote dependency requires an HTTPS repository URL without credentials, query or "
                         "fragment and an exact 40-character commit revision")


def verify_remote_sources(package: Path, command: list, *, work: Path,
                          url: str, revision: str) -> None:
    run(command + ["resolve"], cwd=work)
    dependencies = json.loads(run(command + ["show-dependencies", "--format", "json"],
                                  cwd=work, capture_output=True, text=True).stdout)
    matches = [dependency for dependency in dependencies["dependencies"] if dependency["url"] == url]
    if len(matches) != 1:
        raise ValueError("SwiftPM did not resolve the requested remote dependency")
    checkout = Path(matches[0]["path"])
    actual_revision = run(["git", "-C", checkout, "rev-parse", "HEAD"], cwd=work,
                          capture_output=True, text=True).stdout.strip()
    if actual_revision != revision:
        raise ValueError("resolved SwiftPM revision does not match the requested commit")
    tracked = run(["git", "-C", checkout, "ls-files", "-z"], cwd=work,
                  capture_output=True, text=True).stdout.split("\0")
    expected = _tree_files(package)
    if (set(filter(None, tracked)) != set(expected)
            or any(not regular_files_equal(checkout / name, path) for name, path in expected.items())):
        raise ValueError("resolved SwiftPM sources do not match the staged remote package")


def compile_consumer(package: Path, source: Path, work: Path, *, triple=None, sdk=None,
                     remote_package_url: str | None = None,
                     remote_package_revision: str | None = None) -> Path:
    validate_remote_dependency(remote_package_url, remote_package_revision)
    project = work / "consumer"
    project.mkdir(parents=True)
    if remote_package_url is None:
        dependency = f'.package(name: "Merman", path: {json.dumps(str(package), ensure_ascii=False)})'
        product = '.product(name: "Merman", package: "Merman")'
    else:
        dependency = (f'.package(url: {json.dumps(remote_package_url, ensure_ascii=False)}, '
                      f'revision: "{remote_package_revision}")')
        product = '"Merman"'
    (project / "Package.swift").write_text(
        '// swift-tools-version: 5.9\nimport PackageDescription\n'
        'let package = Package(name: "InstalledConsumer", platforms: [.macOS(.v12), .iOS(.v14)], '
        f'dependencies: [{dependency}], '
        'targets: [.executableTarget(name: "InstalledConsumer", '
        f'dependencies: [{product}], path: "Sources")])\n'
    )
    (project / "Sources").mkdir()
    shutil.copyfile(source, project / "Sources/InstalledConsumer.swift")
    args = ["swift", "build", "--package-path", project, "--configuration", "release", "--jobs", "2"]
    if remote_package_url is not None:
        cache = work / "swift-cache"
        args += ["--cache-path", cache]
        verify_remote_sources(package, ["swift", "package", "--package-path", project,
                              "--cache-path", cache], work=work,
                              url=remote_package_url, revision=remote_package_revision)
    if triple:
        args += ["--triple", triple, "--sdk", sdk]
    run(args, cwd=work)
    binary_dir = run(args + ["--show-bin-path"], cwd=work, capture_output=True, text=True).stdout.strip()
    return Path(binary_dir) / "InstalledConsumer"


def qualify_package(*, repo_root: Path, package_dir: Path, work_dir: Path,
                    version: str, simulator: bool, remote_package_url: str | None = None,
                    remote_package_revision: str | None = None) -> tuple[dict, dict]:
    profile = next(p for p in load_artifact_profiles(repo_root / "capabilities/artifact-profiles-v2.json")
                   if p.profile_id == "apple-uniffi-native")
    work_dir.mkdir(parents=True, exist_ok=False)
    source = repo_root / "platforms/apple/tests/InstalledConsumer.swift"
    package = package_dir.resolve()
    if remote_package_url is not None:
        description = json.loads(run(
            ["swift", "package", "--package-path", package, "--scratch-path", work_dir / "manifest-build",
             "--cache-path", work_dir / "manifest-cache", "dump-package"],
            cwd=work_dir, capture_output=True, text=True).stdout)
        binaries = [target for target in description["targets"]
                    if target["name"] == "MermanFFI" and target["type"] == "binary"]
        if len(binaries) != 1 or not isinstance(binaries[0].get("url"), str):
            raise ValueError("remote qualification requires an HTTPS binary target, not a local XCFramework")
        _validate_artifact_url(binaries[0]["url"], version, f"Merman.xcframework-v{version}.zip")
    remote_options = dict(remote_package_url=remote_package_url, remote_package_revision=remote_package_revision)
    binary = compile_consumer(package, source, work_dir / "macos", **remote_options)
    output = work_dir / "macos.json"
    with output.open("w") as stream:
        run([binary], cwd=work_dir, stdout=stream, timeout=120)
    result = validate_consumer(output, capabilities=profile.capabilities,
                               outputs=profile.outputs, diagram_families=profile.diagram_families,
                               version=version)
    report = {"macos_binary_bytes": binary.stat().st_size,
              "macos_math_sha256": hashlib.sha256(result["math"]["svg"].encode()).hexdigest()}
    device_sdk = run(["xcrun", "--sdk", "iphoneos", "--show-sdk-path"], cwd=work_dir,
                     capture_output=True, text=True).stdout.strip()
    compile_consumer(package, source, work_dir / "device", triple="arm64-apple-ios14.0", sdk=device_sdk,
                     **remote_options)
    sim_sdk = run(["xcrun", "--sdk", "iphonesimulator", "--show-sdk-path"], cwd=work_dir,
                  capture_output=True, text=True).stdout.strip()
    host = run(["uname", "-m"], cwd=work_dir, capture_output=True, text=True).stdout.strip()
    arch = "arm64" if host == "arm64" else "x86_64"
    sim_binary = compile_consumer(package, source, work_dir / "simulator",
                                  triple=f"{arch}-apple-ios14.0-simulator", sdk=sim_sdk, **remote_options)
    if simulator:
        devices = json.loads(run(["xcrun", "simctl", "list", "devices", "available", "--json"],
                                  cwd=work_dir, capture_output=True, text=True, timeout=60).stdout)
        available = [d for runtime, entries in devices["devices"].items() if ".iOS-" in runtime
                     for d in entries if d.get("isAvailable")]
        if not available:
            raise RuntimeError("Apple qualification requires an installed iOS simulator runtime")
        selected = next((d for d in available if d["state"] == "Booted"), available[0])
        udid = selected["udid"]
        started = selected["state"] != "Booted"
        try:
            if started:
                run(["xcrun", "simctl", "boot", udid], cwd=work_dir, timeout=60)
                run(["xcrun", "simctl", "bootstatus", udid, "-b"], cwd=work_dir, timeout=300)
            sim_output = work_dir / "simulator.json"
            with sim_output.open("w") as stream:
                run(["xcrun", "simctl", "spawn", udid, sim_binary], cwd=work_dir, stdout=stream, timeout=120)
            validate_consumer(sim_output, capabilities=profile.capabilities,
                              outputs=profile.outputs, diagram_families=profile.diagram_families,
                              version=version)
            report["simulator_runtime"] = "passed"
        finally:
            if started:
                run(["xcrun", "simctl", "shutdown", udid], cwd=work_dir, timeout=30)
    else:
        report["simulator_runtime"] = "not-requested"
    if remote_package_url is not None:
        report["remote_dependency"] = {"url": remote_package_url, "revision": remote_package_revision,
                                       "staged_sources": "matched"}
    return report, result


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
    library_paths = {str(Path("Merman.xcframework") / entry["LibraryIdentifier"] / entry["LibraryPath"])
                     for entry in info["AvailableLibraries"]}
    shutil.copytree(
        package_dir, original_package,
        ignore=lambda directory, names: [name for name in names
            if str((Path(directory) / name).relative_to(package_dir)) in library_paths],
    )
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
        run([original_binary], cwd=work_dir, stdout=stream, timeout=120)
    original_result = validate_consumer(output, capabilities=profile.capabilities,
                                       outputs=profile.outputs, diagram_families=profile.diagram_families,
                                       version=version)
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
    parser.add_argument("--remote-package-url", help="HTTPS Git URL of the unmodified thin delivery tree")
    parser.add_argument("--remote-package-revision", help="Exact commit of that delivery repository")
    args = parser.parse_args()
    try:
        validate_remote_dependency(args.remote_package_url, args.remote_package_revision)
    except ValueError as error:
        parser.error(str(error))
    if args.remote_package_url is not None and args.compare_original:
        parser.error("--compare-original requires the complete local package, not a remote source tree")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=False)
    extracted = work / "extracted"
    # Producer-owned archive; the packager restricts its inventory and paths.
    with zipfile.ZipFile(args.package_archive) as archive:
        archive.extractall(extracted)
    package = next(extracted.glob("*/Package.swift")).parent
    report, staged_result = qualify_package(repo_root=REPO_ROOT, package_dir=package,
                                           work_dir=work / "installed", version=args.version,
                                           simulator=args.simulator,
                                           remote_package_url=args.remote_package_url,
                                           remote_package_revision=args.remote_package_revision)
    if args.compare_original:
        report["matched_size"] = compare_original(repo_root=REPO_ROOT, package_dir=package,
                                                  work_dir=work / "comparison",
                                                  version=args.version,
                                                  staged_result=staged_result)
    (work / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
