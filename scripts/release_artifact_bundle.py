#!/usr/bin/env python3
"""Prepare and verify Merman's immutable cargo-dist release bundle."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import sys

if __package__:
    from .release_archive import (
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        read_checksum,
        release_archive_name_for,
        require_regular_input,
        sha256_file,
    )
else:
    from release_archive import (
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        read_checksum,
        release_archive_name_for,
        require_regular_input,
        sha256_file,
    )


__all__ = (
    "ReleaseArtifactError",
    "assemble_bundle",
    "prepare_global_inputs",
    "verify_bundle",
    "verify_native_receipts",
    "write_native_receipt",
)


MANIFEST_NAME = "release-verification.json"
MANIFEST_SCHEMA_VERSION = 1
RELEASE_WORKFLOW = ".github/workflows/release.yml"
RELEASE_JOB = "host"
PACKAGES = {
    "merman-cli": "cli-release",
    "merman-lsp": "lsp-stdio-release",
}
DIST_INPUT_MANIFEST = "verified-local-dist-manifest.json"
DIST_OUTPUT_MANIFEST = "dist-manifest.json"

JSON_MAX_BYTES = 4 * 1024 * 1024
INSTALLER_MAX_BYTES = 4 * 1024 * 1024
CHECKSUM_INDEX_MAX_BYTES = 1024 * 1024

_ARCHIVE_RE = re.compile(r"(merman-cli|merman-lsp)-(.+)\.(tar\.xz|zip)\Z")
_INSTALLER_RE = re.compile(r"(merman-cli|merman-lsp)-installer\.(sh|ps1)\Z")
_SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
_SOURCE_SHA_RE = re.compile(r"[0-9a-fA-F]{40}\Z")
_VERSION_RE = re.compile(r"[0-9A-Za-z][0-9A-Za-z.+-]{0,127}\Z")


class ReleaseArtifactError(RuntimeError):
    """Raised when the final Merman release asset set is inconsistent."""


def _json_without_duplicate_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ReleaseArtifactError(f"JSON document contains duplicate key {key!r}")
        result[key] = value
    return result


def _read_json_object(path: Path) -> dict[str, object]:
    path = Path(path)
    require_regular_input(path, "JSON document")
    if path.stat().st_size > JSON_MAX_BYTES:
        raise ReleaseArtifactError(f"JSON document exceeds verification budget: {path}")
    try:
        with path.open("r", encoding="utf-8") as source:
            value = json.load(source, object_pairs_hook=_json_without_duplicate_keys)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ReleaseArtifactError(f"cannot read JSON document {path}: {error}") from error
    if not isinstance(value, dict):
        raise ReleaseArtifactError(f"JSON document must be an object: {path}")
    return value


def _require_root_filename(name: object, *, label: str) -> str:
    if (
        not isinstance(name, str)
        or not name
        or Path(name).name != name
        or "/" in name
        or "\\" in name
        or any(character in name for character in "*?[")
    ):
        raise ReleaseArtifactError(f"{label} is not one exact root filename: {name!r}")
    return name


def _require_version(value: str) -> str:
    if _VERSION_RE.fullmatch(value) is None:
        raise ReleaseArtifactError(f"invalid release version: {value!r}")
    return value


def _require_source_sha(value: str) -> str:
    if _SOURCE_SHA_RE.fullmatch(value) is None:
        raise ReleaseArtifactError(f"invalid source commit SHA: {value!r}")
    return value.lower()


def _root_regular_files(root: Path) -> set[str]:
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ReleaseArtifactError(f"release asset root must be a directory: {root}")
    try:
        entries = tuple(root.iterdir())
    except OSError as error:
        raise ReleaseArtifactError(f"cannot inspect release asset directory {root}: {error}") from error
    names: set[str] = set()
    for path in entries:
        if path.is_symlink() or not path.is_file():
            raise ReleaseArtifactError(f"release asset root contains a non-file entry: {path}")
        names.add(path.name)
    return names


def _root_directories(root: Path) -> set[str]:
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ReleaseArtifactError(f"producer root must be a directory: {root}")
    names: set[str] = set()
    for path in root.iterdir():
        if path.is_symlink() or not path.is_dir():
            raise ReleaseArtifactError(f"producer root contains a non-directory entry: {path}")
        names.add(path.name)
    return names


def _asset_identity(name: str) -> tuple[str, dict[str, str]]:
    archive = _ARCHIVE_RE.fullmatch(name)
    if archive is not None:
        package, target, _extension = archive.groups()
        if release_archive_name_for(package, target) != name:
            raise ReleaseArtifactError(
                f"archive extension does not match target {target!r}: {name}"
            )
        return "archive", {
            "package": package,
            "profile": PACKAGES[package],
            "target": target,
        }
    if name.endswith(".sha256"):
        subject = name.removesuffix(".sha256")
        if _ARCHIVE_RE.fullmatch(subject) is None:
            raise ReleaseArtifactError(f"checksum has an unsupported subject: {name}")
        _asset_identity(subject)
        return "adjacent-checksum", {}
    if _INSTALLER_RE.fullmatch(name) is not None:
        return "installer", {}
    if name == "sha256.sum":
        return "checksum-index", {}
    raise ReleaseArtifactError(f"unsupported release asset name: {name}")


def _expected_asset_names(surfaces_path: Path) -> tuple[str, ...]:
    document = _read_json_object(Path(surfaces_path))
    surfaces = document.get("surfaces")
    if not isinstance(surfaces, list):
        raise ReleaseArtifactError("release surface document has no surfaces array")
    names: set[str] = set()
    for surface in surfaces:
        if not isinstance(surface, dict):
            raise ReleaseArtifactError("release surface entry must be an object")
        channels = surface.get("channels")
        if not isinstance(channels, list):
            raise ReleaseArtifactError("release surface entry has no channels array")
        for channel in channels:
            if not isinstance(channel, dict):
                raise ReleaseArtifactError("release channel entry must be an object")
            if (
                channel.get("workflow") != RELEASE_WORKFLOW
                or channel.get("workflow_job") != RELEASE_JOB
            ):
                continue
            patterns = channel.get("asset_patterns")
            if not isinstance(patterns, list):
                raise ReleaseArtifactError("release channel has no asset_patterns array")
            for pattern in patterns:
                if not isinstance(pattern, dict):
                    raise ReleaseArtifactError("release asset pattern must be an object")
                names.add(
                    _require_root_filename(
                        pattern.get("glob"),
                        label="release asset contract",
                    )
                )
    if MANIFEST_NAME not in names:
        raise ReleaseArtifactError(f"release asset contract must declare {MANIFEST_NAME}")
    names.remove(MANIFEST_NAME)
    _validate_asset_contract(names)
    return tuple(sorted(names))


def _validate_asset_contract(names: set[str]) -> None:
    identities = {name: _asset_identity(name) for name in names}
    archives = {name for name, (kind, _) in identities.items() if kind == "archive"}
    checksums = {
        name for name, (kind, _) in identities.items() if kind == "adjacent-checksum"
    }
    if checksums != {f"{name}.sha256" for name in archives}:
        raise ReleaseArtifactError("every archive must have exactly one adjacent checksum")
    if {name for name, (kind, _) in identities.items() if kind == "installer"} != {
        f"{package}-installer.{extension}"
        for package in PACKAGES
        for extension in ("sh", "ps1")
    }:
        raise ReleaseArtifactError("release asset contract has the wrong installer set")
    if {name for name, (kind, _) in identities.items() if kind == "checksum-index"} != {
        "sha256.sum"
    }:
        raise ReleaseArtifactError("release asset contract must contain one sha256.sum")
    targets_by_package = {package: set() for package in PACKAGES}
    for name in archives:
        _, identity = identities[name]
        targets_by_package[identity["package"]].add(identity["target"])
    target_sets = tuple(targets_by_package.values())
    if not target_sets[0] or any(targets != target_sets[0] for targets in target_sets[1:]):
        raise ReleaseArtifactError("CLI and LSP release target sets differ")


def _archive_names(names: tuple[str, ...]) -> tuple[str, ...]:
    return tuple(name for name in names if _asset_identity(name)[0] == "archive")


def _read_checksum_index(path: Path) -> dict[str, str]:
    require_regular_input(path, "checksum index")
    if path.stat().st_size > CHECKSUM_INDEX_MAX_BYTES:
        raise ReleaseArtifactError("sha256.sum exceeds verification budget")
    try:
        lines = path.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeError) as error:
        raise ReleaseArtifactError(f"cannot read checksum index {path}: {error}") from error
    if lines and not lines[-1]:
        lines.pop()
    result: dict[str, str] = {}
    for line in lines:
        fields = line.split()
        if len(fields) != 2 or _SHA256_RE.fullmatch(fields[0].lower()) is None:
            raise ReleaseArtifactError("sha256.sum contains a noncanonical line")
        name = _require_root_filename(
            fields[1].removeprefix("*"),
            label="sha256.sum subject",
        )
        if name in result:
            raise ReleaseArtifactError(f"sha256.sum contains duplicate name: {name}")
        result[name] = fields[0].lower()
    return result


def _verify_checksums(
    root: Path,
    names: tuple[str, ...],
    *,
    observed_digests: dict[str, str] | None = None,
) -> dict[str, str]:
    root = Path(root)
    subjects: dict[str, str] = {}
    for archive_name in _archive_names(names):
        archive = root / archive_name
        checksum = root / f"{archive_name}.sha256"
        require_regular_input(archive, "release archive")
        require_regular_input(checksum, "adjacent checksum")
        if archive.stat().st_size > DEFAULT_LIMITS.max_archive_size:
            raise ReleaseArtifactError(f"archive exceeds verification budget: {archive_name}")
        digest = read_checksum(checksum, archive_name)
        observed = (
            sha256_file(archive)
            if observed_digests is None
            else observed_digests.get(archive_name)
        )
        if observed != digest:
            raise ReleaseArtifactError(f"checksum mismatch for {archive_name}")
        subjects[archive_name] = digest
    if _read_checksum_index(root / "sha256.sum") != subjects:
        raise ReleaseArtifactError("sha256.sum differs from the verified archive set")
    return subjects


def _validate_plan_manifest(
    manifest: dict[str, object],
    *,
    names: tuple[str, ...],
    tag: str,
    version: str,
) -> dict[str, dict[str, object]]:
    if manifest.get("announcement_tag") != tag:
        raise ReleaseArtifactError("cargo-dist plan has the wrong release tag")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict) or set(artifacts) != set(names):
        raise ReleaseArtifactError("cargo-dist plan artifact set differs from release surfaces")
    releases = manifest.get("releases")
    if not isinstance(releases, list) or len(releases) != len(PACKAGES):
        raise ReleaseArtifactError("cargo-dist plan has the wrong release set")
    observed_releases = {}
    for release in releases:
        if not isinstance(release, dict):
            raise ReleaseArtifactError("cargo-dist release entry must be an object")
        package = release.get("app_name")
        release_version = release.get("app_version")
        if not isinstance(package, str) or package in observed_releases:
            raise ReleaseArtifactError("cargo-dist plan has a duplicate or invalid package")
        observed_releases[package] = release_version
    if observed_releases != {package: version for package in PACKAGES}:
        raise ReleaseArtifactError("cargo-dist plan package versions differ from the release")

    expected_kinds = {
        "archive": "executable-zip",
        "adjacent-checksum": "checksum",
        "installer": "installer",
        "checksum-index": "unified-checksum",
    }
    typed: dict[str, dict[str, object]] = {}
    for name in names:
        entry = artifacts.get(name)
        if not isinstance(entry, dict):
            raise ReleaseArtifactError(f"cargo-dist artifact must be an object: {name}")
        kind, identity = _asset_identity(name)
        if entry.get("kind") != expected_kinds[kind]:
            raise ReleaseArtifactError(f"cargo-dist artifact kind mismatch: {name}")
        if kind in {"archive", "adjacent-checksum"}:
            target = identity.get("target")
            if target is None:
                target = _asset_identity(name.removesuffix(".sha256"))[1]["target"]
            if entry.get("target_triples") != [target]:
                raise ReleaseArtifactError(f"cargo-dist target mismatch: {name}")
        typed[name] = entry
    return typed


def _validate_local_manifest(
    path: Path,
    *,
    tag: str,
    expected_archives: tuple[str, str],
    digests: dict[str, str],
) -> None:
    manifest = _read_json_object(path)
    if manifest.get("announcement_tag") != tag:
        raise ReleaseArtifactError(f"local dist manifest has the wrong tag: {path}")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict):
        raise ReleaseArtifactError(f"local dist manifest has no artifacts object: {path}")
    for name in expected_archives:
        entry = artifacts.get(name)
        if (
            not isinstance(entry, dict)
            or entry.get("kind") != "executable-zip"
            or entry.get("checksums") != {"sha256": digests[name]}
        ):
            raise ReleaseArtifactError(f"local dist manifest archive mismatch: {name}")


def prepare_global_inputs(
    local_producers: Path,
    verified_archives: Path,
    plan_manifest: Path,
    surfaces_path: Path,
    destination: Path,
    *,
    tag: str,
    version: str,
) -> Path:
    """Stage only verified local archives for trusted cargo-dist global generation."""
    version = _require_version(version)
    if tag != f"v{version}":
        raise ReleaseArtifactError("release tag and version differ")
    names = _expected_asset_names(surfaces_path)
    archives = _archive_names(names)
    targets = sorted({_asset_identity(name)[1]["target"] for name in archives})
    expected_producers = {f"artifacts-build-local-{target}" for target in targets}
    if _root_directories(local_producers) != expected_producers:
        raise ReleaseArtifactError("local artifact producer set differs from release targets")
    if _root_regular_files(verified_archives) != set(archives):
        raise ReleaseArtifactError("verified archive set differs from release targets")

    plan = _read_json_object(plan_manifest)
    plan_artifacts = _validate_plan_manifest(
        plan,
        names=names,
        tag=tag,
        version=version,
    )
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=False)
    try:
        digests: dict[str, str] = {}
        for target in targets:
            producer = Path(local_producers) / f"artifacts-build-local-{target}"
            target_archives = tuple(
                release_archive_name_for(package, target) for package in PACKAGES
            )
            expected_files = {
                *target_archives,
                *(f"{name}.sha256" for name in target_archives),
                f"{target}-dist-manifest.json",
            }
            if _root_regular_files(producer) != expected_files:
                raise ReleaseArtifactError(f"local producer payload differs for {target}")
            for name in target_archives:
                original = producer / name
                original_checksum = producer / f"{name}.sha256"
                verified = Path(verified_archives) / name
                digest = read_checksum(original_checksum, name)
                if sha256_file(original) != digest or sha256_file(verified) != digest:
                    raise ReleaseArtifactError(f"verified archive identity mismatch: {name}")
                shutil.copyfile(verified, destination / name)
                (destination / f"{name}.sha256").write_text(
                    f"{digest} *{name}\n",
                    encoding="ascii",
                )
                plan_artifacts[name]["checksums"] = {"sha256": digest}
                plan_artifacts[name].pop("path", None)
                digests[name] = digest
            _validate_local_manifest(
                producer / f"{target}-dist-manifest.json",
                tag=tag,
                expected_archives=target_archives,
                digests=digests,
            )

        plan["upload_files"] = []
        output = destination / DIST_INPUT_MANIFEST
        output.write_text(
            json.dumps(plan, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        return output
    except BaseException:
        shutil.rmtree(destination, ignore_errors=True)
        raise


def _validate_installers(root: Path, names: tuple[str, ...], *, version: str) -> None:
    archives = _archive_names(names)
    digests = {
        name: read_checksum(root / f"{name}.sha256", name) for name in archives
    }
    for package in PACKAGES:
        package_archives = tuple(name for name in archives if name.startswith(f"{package}-"))
        for extension in ("sh", "ps1"):
            path = root / f"{package}-installer.{extension}"
            require_regular_input(path, "generated installer")
            if path.stat().st_size > INSTALLER_MAX_BYTES:
                raise ReleaseArtifactError(f"installer exceeds verification budget: {path.name}")
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                raise ReleaseArtifactError(f"installer is not UTF-8: {path.name}") from error
            referenced_archives = (
                package_archives
                if extension == "sh"
                else tuple(name for name in package_archives if name.endswith(".zip"))
            )
            required = {
                package,
                version,
                f"https://github.com/Latias94/merman/releases/download/v{version}",
                *referenced_archives,
            }
            if extension == "sh":
                required.update(digests[name] for name in referenced_archives)
            missing = sorted(value for value in required if value not in text)
            if missing:
                raise ReleaseArtifactError(
                    f"installer contract is incomplete for {path.name}: {missing!r}"
                )


def assemble_bundle(
    generated_root: Path,
    verified_archives: Path,
    destination: Path,
    surfaces_path: Path,
    *,
    version: str,
    source_sha: str,
) -> Path:
    """Assemble final assets without accepting an unverified archive fallback."""
    version = _require_version(version)
    source_sha = _require_source_sha(source_sha)
    names = _expected_asset_names(surfaces_path)
    expected_generated = {*names, DIST_INPUT_MANIFEST, DIST_OUTPUT_MANIFEST}
    if _root_regular_files(generated_root) != expected_generated:
        raise ReleaseArtifactError("generated release payload differs from the asset contract")
    archives = _archive_names(names)
    if _root_regular_files(verified_archives) != set(archives):
        raise ReleaseArtifactError("verified archive set differs from the asset contract")
    generated_archive_digests = _verify_checksums(generated_root, names)
    _validate_installers(generated_root, names, version=version)

    generated_manifest = _read_json_object(Path(generated_root) / DIST_OUTPUT_MANIFEST)
    _validate_plan_manifest(
        generated_manifest,
        names=names,
        tag=f"v{version}",
        version=version,
    )

    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=False)
    try:
        assets = []
        for name in names:
            kind, identity = _asset_identity(name)
            source = (
                Path(verified_archives) / name
                if kind == "archive"
                else Path(generated_root) / name
            )
            require_regular_input(source, "verified release asset")
            digest = sha256_file(source)
            if kind == "archive" and digest != generated_archive_digests[name]:
                raise ReleaseArtifactError(
                    f"generated archive changed after verification: {name}"
                )
            shutil.copyfile(source, destination / name)
            assets.append(
                {
                    "name": name,
                    "kind": kind,
                    "sha256": digest,
                    "size": source.stat().st_size,
                    **identity,
                }
            )
        manifest = {
            "schema_version": MANIFEST_SCHEMA_VERSION,
            "version": version,
            "source_sha": source_sha,
            "assets": assets,
        }
        manifest_path = destination / MANIFEST_NAME
        manifest_path.write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        if _root_regular_files(destination) != {*names, MANIFEST_NAME}:
            raise ReleaseArtifactError("assembled bundle differs from the release contract")
        return manifest_path
    except BaseException:
        shutil.rmtree(destination, ignore_errors=True)
        raise


def _asset_size_budget(kind: str) -> int:
    return {
        "archive": DEFAULT_LIMITS.max_archive_size,
        "adjacent-checksum": 4096,
        "installer": INSTALLER_MAX_BYTES,
        "checksum-index": CHECKSUM_INDEX_MAX_BYTES,
    }[kind]


def _validated_manifest(root: Path) -> dict[str, object]:
    manifest = _read_json_object(root / MANIFEST_NAME)
    if set(manifest) != {"schema_version", "version", "source_sha", "assets"}:
        raise ReleaseArtifactError("release verification manifest has unexpected fields")
    if manifest.get("schema_version") != MANIFEST_SCHEMA_VERSION:
        raise ReleaseArtifactError("unsupported release verification manifest")
    version = manifest.get("version")
    if not isinstance(version, str):
        raise ReleaseArtifactError("release verification manifest has an invalid version")
    _require_version(version)
    source_sha = manifest.get("source_sha")
    if not isinstance(source_sha, str) or source_sha != _require_source_sha(source_sha):
        raise ReleaseArtifactError("release verification manifest has an invalid source SHA")
    assets = manifest.get("assets")
    if not isinstance(assets, list):
        raise ReleaseArtifactError("release verification manifest has no asset list")

    names: set[str] = set()
    for entry in assets:
        if not isinstance(entry, dict):
            raise ReleaseArtifactError("release verification asset entry must be an object")
        name = _require_root_filename(entry.get("name"), label="manifest asset name")
        if name in names:
            raise ReleaseArtifactError(f"release verification manifest duplicates {name!r}")
        names.add(name)
        kind, identity = _asset_identity(name)
        expected_fields = {"name", "kind", "sha256", "size", *identity}
        if set(entry) != expected_fields or entry.get("kind") != kind:
            raise ReleaseArtifactError(f"release verification identity mismatch: {name}")
        if any(entry.get(key) != value for key, value in identity.items()):
            raise ReleaseArtifactError(f"release verification archive metadata mismatch: {name}")
        digest = entry.get("sha256")
        size = entry.get("size")
        if not isinstance(digest, str) or _SHA256_RE.fullmatch(digest) is None:
            raise ReleaseArtifactError(f"release verification digest is invalid: {name}")
        if (
            not isinstance(size, int)
            or isinstance(size, bool)
            or size <= 0
            or size > _asset_size_budget(kind)
        ):
            raise ReleaseArtifactError(f"release verification size is invalid: {name}")
    _validate_asset_contract(names)
    return manifest


def _verified_bundle(
    root: Path,
    *,
    version: str | None,
    source_sha: str,
) -> dict[str, object]:
    root = Path(root)
    manifest = _validated_manifest(root)
    manifest_version = manifest["version"]
    assert isinstance(manifest_version, str)
    if version is not None and manifest_version != _require_version(version):
        raise ReleaseArtifactError("release verification manifest has the wrong version")
    if manifest["source_sha"] != _require_source_sha(source_sha):
        raise ReleaseArtifactError("release verification manifest has the wrong source identity")
    assets = manifest["assets"]
    assert isinstance(assets, list)
    expected = {entry["name"] for entry in assets}
    if _root_regular_files(root) != {*expected, MANIFEST_NAME}:
        raise ReleaseArtifactError("downloaded verified bundle has missing or extra files")
    observed_digests: dict[str, str] = {}
    for entry in assets:
        path = root / entry["name"]
        require_regular_input(path, "verified release asset")
        if path.stat().st_size != entry["size"]:
            raise ReleaseArtifactError(f"asset size mismatch: {path.name}")
        observed_digests[path.name] = sha256_file(path)
        if observed_digests[path.name] != entry["sha256"]:
            raise ReleaseArtifactError(f"asset digest mismatch: {path.name}")
    names = tuple(sorted(expected))
    _verify_checksums(root, names, observed_digests=observed_digests)
    _validate_installers(root, names, version=manifest_version)
    return manifest


def verify_bundle(root: Path, *, version: str, source_sha: str) -> None:
    _verified_bundle(root, version=version, source_sha=source_sha)


def write_native_receipt(
    bundle: Path,
    output: Path,
    *,
    target: str,
    source_sha: str,
) -> None:
    bundle = Path(bundle)
    source_sha = _require_source_sha(source_sha)
    manifest = _verified_bundle(bundle, version=None, source_sha=source_sha)
    assets = manifest["assets"]
    assert isinstance(assets, list)
    archives = {
        entry["package"]: entry["sha256"]
        for entry in assets
        if entry["kind"] == "archive" and entry["target"] == target
    }
    if set(archives) != set(PACKAGES):
        raise ReleaseArtifactError(f"release bundle has no complete archive set for {target}")
    receipt = {
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "source_sha": source_sha,
        "target": target,
        "manifest_sha256": sha256_file(bundle / MANIFEST_NAME),
        "archives": archives,
    }
    Path(output).write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify_native_receipts(bundle: Path, receipts_root: Path, *, source_sha: str) -> None:
    bundle = Path(bundle)
    receipts_root = Path(receipts_root)
    expected_source_sha = _require_source_sha(source_sha)
    manifest = _verified_bundle(
        bundle,
        version=None,
        source_sha=expected_source_sha,
    )
    assets = manifest["assets"]
    assert isinstance(assets, list)
    archive_digests: dict[tuple[str, str], str] = {}
    targets_by_package = {package: set() for package in PACKAGES}
    for entry in assets:
        if entry["kind"] != "archive":
            continue
        key = (entry["package"], entry["target"])
        if key in archive_digests:
            raise ReleaseArtifactError(f"duplicate archive identity in manifest: {key!r}")
        archive_digests[key] = entry["sha256"]
        targets_by_package[entry["package"]].add(entry["target"])
    target_sets = tuple(targets_by_package.values())
    if not target_sets[0] or any(targets != target_sets[0] for targets in target_sets[1:]):
        raise ReleaseArtifactError("CLI and LSP release target sets differ")
    targets = target_sets[0]
    expected_files = {f"native-release-verification-{target}.json" for target in targets}
    if _root_regular_files(receipts_root) != expected_files:
        raise ReleaseArtifactError("native verification receipt set differs from release targets")

    manifest_digest = sha256_file(bundle / MANIFEST_NAME)
    for target in targets:
        receipt = _read_json_object(
            receipts_root / f"native-release-verification-{target}.json"
        )
        if (
            set(receipt)
            != {"schema_version", "source_sha", "target", "manifest_sha256", "archives"}
            or receipt.get("schema_version") != MANIFEST_SCHEMA_VERSION
            or receipt.get("source_sha") != expected_source_sha
            or receipt.get("target") != target
            or receipt.get("manifest_sha256") != manifest_digest
        ):
            raise ReleaseArtifactError(f"native receipt identity mismatch for {target}")
        observed_archives = receipt.get("archives")
        if not isinstance(observed_archives, dict) or set(observed_archives) != set(PACKAGES):
            raise ReleaseArtifactError(f"native receipt archive set mismatch for {target}")
        for package in PACKAGES:
            if observed_archives.get(package) != archive_digests[(package, target)]:
                raise ReleaseArtifactError(f"native archive digest mismatch for {package}/{target}")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare-global")
    prepare.add_argument("local_producers", type=Path)
    prepare.add_argument("verified_archives", type=Path)
    prepare.add_argument("plan_manifest", type=Path)
    prepare.add_argument("surfaces", type=Path)
    prepare.add_argument("destination", type=Path)
    prepare.add_argument("--tag", required=True)
    prepare.add_argument("--version", required=True)

    assemble = commands.add_parser("assemble")
    assemble.add_argument("generated_root", type=Path)
    assemble.add_argument("verified_archives", type=Path)
    assemble.add_argument("destination", type=Path)
    assemble.add_argument("surfaces", type=Path)
    assemble.add_argument("--version", required=True)
    assemble.add_argument("--source-sha", required=True)

    verify = commands.add_parser("verify-bundle")
    verify.add_argument("root", type=Path)
    verify.add_argument("--version", required=True)
    verify.add_argument("--source-sha", required=True)

    receipt = commands.add_parser("write-receipt")
    receipt.add_argument("bundle", type=Path)
    receipt.add_argument("--output", required=True, type=Path)
    receipt.add_argument("--target", required=True)
    receipt.add_argument("--source-sha", required=True)

    receipts = commands.add_parser("verify-receipts")
    receipts.add_argument("bundle", type=Path)
    receipts.add_argument("receipts_root", type=Path)
    receipts.add_argument("--source-sha", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    if args.command == "prepare-global":
        prepare_global_inputs(
            args.local_producers,
            args.verified_archives,
            args.plan_manifest,
            args.surfaces,
            args.destination,
            tag=args.tag,
            version=args.version,
        )
    elif args.command == "assemble":
        assemble_bundle(
            args.generated_root,
            args.verified_archives,
            args.destination,
            args.surfaces,
            version=args.version,
            source_sha=args.source_sha,
        )
    elif args.command == "verify-bundle":
        verify_bundle(args.root, version=args.version, source_sha=args.source_sha)
    elif args.command == "write-receipt":
        write_native_receipt(
            args.bundle,
            args.output,
            target=args.target,
            source_sha=args.source_sha,
        )
    else:
        verify_native_receipts(
            args.bundle,
            args.receipts_root,
            source_sha=args.source_sha,
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveVerificationError, ReleaseArtifactError, OSError) as error:
        print(f"release_artifact_bundle.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
