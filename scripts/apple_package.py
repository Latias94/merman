#!/usr/bin/env python3
"""Package an already built, complete Apple XCFramework and its Swift facade."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import plistlib
import shutil
import tempfile
from urllib.parse import urlsplit
from xml.parsers.expat import ExpatError
import zipfile

if __package__:
    from .artifact_profile_recipe import ArtifactProfileError, load_artifact_profiles
    from .release_archive import (
        ArchiveVerificationError, _normalize_member_name, regular_files_equal,
        repository_tree_files, require_regular_input, require_repository_root, sha256_file,
    )
    from .sdk_archive import (
        REPO_ROOT, SHA256, _workspace_version, require_clean_source_tree, resolve_source_commit,
    )
else:
    from artifact_profile_recipe import ArtifactProfileError, load_artifact_profiles
    from release_archive import (
        ArchiveVerificationError, _normalize_member_name, regular_files_equal,
        repository_tree_files, require_regular_input, require_repository_root, sha256_file,
    )
    from sdk_archive import (
        REPO_ROOT, SHA256, _workspace_version, require_clean_source_tree, resolve_source_commit,
    )


def slice_targets(entry: dict) -> tuple[str, ...]:
    """Map one XCFramework AvailableLibraries entry to its Rust target triples."""
    if not isinstance(entry, dict):
        raise ArchiveVerificationError("XCFramework library entry must be a dictionary")
    platform = entry.get("SupportedPlatform")
    variant = entry.get("SupportedPlatformVariant")
    architectures = entry.get("SupportedArchitectures")
    if (not isinstance(architectures, list) or not architectures
            or any(not isinstance(arch, str) for arch in architectures)
            or len(set(architectures)) != len(architectures)):
        raise ArchiveVerificationError("XCFramework slice architectures must be a nonempty unique string array")
    if not ((platform == "macos" and variant is None)
            or (platform == "ios" and variant in (None, "simulator"))):
        raise ArchiveVerificationError("unsupported XCFramework platform or variant")
    targets = []
    for arch in architectures:
        if arch not in ("arm64", "x86_64") or (platform == "ios" and variant is None and arch != "arm64"):
            raise ArchiveVerificationError("unsupported XCFramework architecture for this platform variant")
        rust_arch = "aarch64" if arch == "arm64" else arch
        suffix = "darwin" if platform == "macos" else "ios"
        if platform == "ios" and variant == "simulator" and arch == "arm64":
            suffix = "ios-sim"
        targets.append(f"{rust_arch}-apple-{suffix}")
    return tuple(targets)


def _tree_files(root: Path) -> dict[str, Path]:
    return {path.relative_to(root).as_posix(): path
            for path in repository_tree_files(root.parent, root.name).values()}


def _relative_path(root: Path, relative: object, label: str) -> Path:
    if not isinstance(relative, str):
        raise ArchiveVerificationError(f"XCFramework {label} must be a relative path")
    return root / _normalize_member_name(relative, is_directory=False)


def _validate_xcframework(package: Path, recipe) -> None:
    xcf = package / "Merman.xcframework"
    info = xcf / "Info.plist"
    require_regular_input(info, "XCFramework Info.plist")
    try:
        with info.open("rb") as stream:
            document = plistlib.load(stream)
    except (ValueError, plistlib.InvalidFileException, ExpatError) as error:
        raise ArchiveVerificationError("cannot read XCFramework Info.plist") from error
    entries = document.get("AvailableLibraries") if isinstance(document, dict) else None
    if not isinstance(entries, list) or not entries:
        raise ArchiveVerificationError("XCFramework must declare AvailableLibraries")
    if recipe.build_target_kind != "target-set":
        raise ArchiveVerificationError("Apple recipe must declare its complete targets")
    observed = set()
    identifiers = set()
    generated = package / "Sources/Merman/Generated"
    for entry in entries:
        targets = slice_targets(entry)
        if observed.intersection(targets):
            raise ArchiveVerificationError("XCFramework has overlapping targets in duplicate slices")
        observed.update(targets)
        identifier = entry.get("LibraryIdentifier")
        base = _relative_path(xcf, identifier, "LibraryIdentifier")
        if "/" in identifier or identifier in identifiers:
            raise ArchiveVerificationError("XCFramework LibraryIdentifier must be a unique directory name")
        identifiers.add(identifier)
        library = _relative_path(base, entry.get("LibraryPath"), "LibraryPath")
        require_regular_input(library, "XCFramework library")
        headers = _relative_path(base, entry.get("HeadersPath"), "HeadersPath")
        for actual, expected in (("MermanFFI.h", "MermanFFI.h"),
                                 ("module.modulemap", "MermanFFI.modulemap")):
            if not regular_files_equal(headers / actual, generated / expected):
                raise ArchiveVerificationError(f"XCFramework {actual} does not match the generated input")
    required = set(recipe.build_targets)
    if observed != required:
        raise ArchiveVerificationError(
            f"XCFramework targets differ from the Apple recipe: missing={sorted(required - observed)}, "
            f"unexpected={sorted(observed - required)}; partial --ios/--macos builds cannot be packaged"
        )


def _validate_artifact_url(url: str, version: str, binary_name: str) -> None:
    try:
        parsed = urlsplit(url)
        valid = (parsed.scheme == "https" and parsed.hostname and parsed.username is None
                 and parsed.password is None and "?" not in url and "#" not in url
                 and not any(character.isspace() or ord(character) < 32 for character in url)
                 and parsed.path.rsplit("/", 1)[-1] == binary_name)
    except ValueError:
        valid = False
    if not valid:
        raise ArchiveVerificationError("artifact URL must be HTTPS without credentials, query or fragment, "
                                       "and name the version-specific XCFramework ZIP")
    if parsed.hostname == "github.com":
        parts = parsed.path.split("/")[1:]
        if (len(parts) != 6 or not all(parts[:2])
                or parts[2:5] != ["releases", "download", f"v{version}"]):
            raise ArchiveVerificationError("GitHub artifact URL must identify the exact versioned release asset")


def _manifest(*, artifact_url: str | None = None, checksum: str | None = None) -> str:
    if artifact_url is None:
        binary = 'path: "Merman.xcframework"'
    else:
        escaped_url = artifact_url.replace("\\", "\\\\").replace('"', '\\"')
        binary = f'url: "{escaped_url}",\n            checksum: "{checksum}"'
    return f'''// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Merman",
    platforms: [.iOS(.v14), .macOS(.v12)],
    products: [.library(name: "Merman", targets: ["Merman"])],
    targets: [
        .binaryTarget(
            name: "MermanFFI",
            {binary}
        ),
        .target(
            name: "Merman",
            dependencies: ["MermanFFI"],
            path: "Sources/Merman"
        ),
    ]
)
'''


def _readme(version: str, profile) -> str:
    return f'''# Merman {version} for Apple

Requires Swift 5.9 or newer, iOS 14 or newer, or macOS 12 or newer.

Extract this archive and add the Merman-swiftpm-v{version} directory as a local
package in Xcode, then link the Merman library product. With SwiftPM, add
`.package(path: "../Merman-swiftpm-v{version}")` to your package dependencies
and select its Merman product. A Rust compiler or source checkout is not required.

```swift
import Merman

let client = Merman()
let svg = try client.renderSvg(source: "flowchart TD\\nA[Hello] --> B[World]", optionsJson: nil)
```

Recipe: {profile.profile_id}.
Expected capabilities: {", ".join(profile.capabilities)}.
Expected outputs: {", ".join(profile.outputs)}.
Use runtimeCatalogJson() to inspect the installed renderer's actual capabilities.

Keep the supplied Swift sources, resource contract and XCFramework together.
LICENSE, THIRD_PARTY_NOTICES.md and THIRD_PARTY_LICENSES contain the legal material.
SOURCE_AVAILABILITY.txt identifies the exact matching source revision.
'''


def _source_availability(version: str, commit: str) -> str:
    return (f"Merman version: {version}\nSource commit: {commit}\n"
            "Source repository: https://github.com/Latias94/merman\n\n"
            "Obtain this exact source revision from the package producer if it is not yet\n"
            "available in the repository. In a source checkout containing that commit, run:\n"
            f"git checkout --detach {commit}\n\n"
            "The corresponding source includes Cargo.lock, the Rust crates and generated\n"
            "bindings. Third-party source references and license obligations are recorded\n"
            "in THIRD_PARTY_NOTICES.md and THIRD_PARTY_LICENSES, including ELK and math.\n")


def _write_zip(destination: Path, files: dict[str, Path]) -> None:
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for name, path in sorted(files.items()):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            # Python 3.11 exposes the per-member streaming compression level here.
            info._compresslevel = 6
            info.file_size = path.stat().st_size
            with path.open("rb") as source, archive.open(info, "w") as output:
                shutil.copyfileobj(source, output, length=1024 * 1024)


def package_apple(*, repo_root: Path, xcframework: Path, output_dir: Path, version: str,
                  source_commit: str | None = None, artifact_url: str | None = None,
                  expected_checksum: str | None = None) -> dict[str, Path]:
    repo_root = require_repository_root(Path(repo_root))
    require_clean_source_tree(repo_root)
    if version != _workspace_version(repo_root):
        raise ArchiveVerificationError("Apple package version does not match the workspace version")
    commit = resolve_source_commit(repo_root, source_commit)
    profiles = load_artifact_profiles(repo_root / "capabilities/artifact-profiles-v2.json")
    profile = next((profile for profile in profiles if profile.profile_id == "apple-uniffi-native"), None)
    if profile is None:
        raise ArchiveVerificationError("descriptor is missing the apple-uniffi-native profile")
    package_name = f"Merman-swiftpm-v{version}"
    binary_name = f"Merman.xcframework-v{version}.zip"
    if artifact_url is not None:
        _validate_artifact_url(artifact_url, version, binary_name)
    if expected_checksum is not None and not SHA256.fullmatch(expected_checksum.lower()):
        raise ArchiveVerificationError("expected checksum must be a SHA-256 digest")

    apple = repo_root / "platforms/apple"
    files = {f"Sources/Merman/{name}": path for name, path in _tree_files(apple / "Sources/Merman").items()}
    files.update({f"Merman.xcframework/{name}": path for name, path in _tree_files(Path(xcframework)).items()})
    licenses = _tree_files(apple / "THIRD_PARTY_LICENSES")
    if not licenses:
        raise ArchiveVerificationError("Apple THIRD_PARTY_LICENSES tree must not be empty")
    files.update({f"THIRD_PARTY_LICENSES/{name}": path for name, path in licenses.items()})
    for name in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
        path = apple / name
        require_regular_input(path, "Apple legal input")
        files[name] = path
    for name in ("Merman.swift", "MermanResourceContract.swift", "MermanFFI.h", "MermanFFI.modulemap"):
        require_regular_input(apple / "Sources/Merman/Generated" / name, "generated Apple input")

    output_dir = Path(output_dir)
    if output_dir.is_symlink():
        raise ArchiveVerificationError("Apple output directory must not be a symlink")
    output_dir.mkdir(parents=True, exist_ok=True)
    output_dir = output_dir.resolve()
    result = {"package_dir": output_dir / package_name,
              "package_archive": output_dir / f"{package_name}.zip",
              "binary_archive": output_dir / binary_name}
    if artifact_url is not None:
        result["remote_manifest"] = output_dir / f"{package_name}.Package.swift"
    destinations = [*result.values(), *(Path(str(result[key]) + ".checksum")
                                       for key in ("package_archive", "binary_archive"))]
    if any(path.exists() or path.is_symlink() for path in destinations):
        raise ArchiveVerificationError("Apple output already exists; refusing to replace it")

    with tempfile.TemporaryDirectory(prefix=".apple-package-", dir=output_dir) as temporary:
        stage = Path(temporary)
        package = stage / package_name
        for name, source in sorted(files.items()):
            destination = package / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        _validate_xcframework(package, profile.cargo)
        (package / "Package.swift").write_text(_manifest(), encoding="utf-8", newline="\n")
        (package / "README.md").write_text(_readme(version, profile), encoding="utf-8", newline="\n")
        (package / "SOURCE_AVAILABILITY.txt").write_text(
            _source_availability(version, commit), encoding="utf-8", newline="\n")
        staged_files = _tree_files(package)
        binary_files = {name: path for name, path in staged_files.items()
                        if name.startswith(("Merman.xcframework/", "THIRD_PARTY_LICENSES/"))
                        or name in {"LICENSE", "THIRD_PARTY_NOTICES.md", "SOURCE_AVAILABILITY.txt"}}
        binary_archive = stage / binary_name
        _write_zip(binary_archive, binary_files)
        binary_checksum = sha256_file(binary_archive)
        if expected_checksum is not None and expected_checksum.lower() != binary_checksum:
            raise ArchiveVerificationError("expected checksum does not match the computed XCFramework ZIP checksum")
        _write_zip(stage / result["package_archive"].name,
                   {f"{package_name}/{name}": path for name, path in staged_files.items()})
        if artifact_url is not None:
            (stage / result["remote_manifest"].name).write_text(
                _manifest(artifact_url=artifact_url, checksum=binary_checksum), encoding="utf-8", newline="\n")
        for key in ("binary_archive", "package_archive"):
            archive = stage / result[key].name
            digest = binary_checksum if key == "binary_archive" else sha256_file(archive)
            (stage / f"{archive.name}.checksum").write_text(
                f"{digest}  {archive.name}\n", encoding="ascii", newline="\n")
        for destination in destinations:
            (stage / destination.name).rename(destination)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--xcframework", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-commit")
    parser.add_argument("--artifact-url")
    parser.add_argument("--expected-checksum")
    args = parser.parse_args(argv)
    xcframework = args.xcframework
    if xcframework is None:
        xcframework = args.repo_root / "platforms/apple/Merman.xcframework"
    try:
        result = package_apple(
            repo_root=args.repo_root, xcframework=xcframework,
            output_dir=args.output_dir, version=args.version, source_commit=args.source_commit,
            artifact_url=args.artifact_url, expected_checksum=args.expected_checksum,
        )
    except (ArchiveVerificationError, ArtifactProfileError, OSError) as error:
        parser.exit(1, f"{error}\n")
    print(json.dumps({name: str(path) for name, path in result.items()}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
