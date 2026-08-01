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
    "harden_installers",
    "prepare_global_inputs",
    "verify_bundle",
)


MANIFEST_NAME = "release-verification.json"
MANIFEST_SCHEMA_VERSION = 1
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

_SHELL_SHA256_FAIL_OPEN = """        sha256)
            if ! check_cmd sha256sum; then
                say "skipping sha256 checksum verification (it requires the 'sha256sum' command)"
                return 0
            fi
            _calculated_checksum="$(sha256sum -b "$_file" | awk '{printf $1}')"
            ;;
"""
_SHELL_SHA256_FAIL_CLOSED = """        sha256)
            if check_cmd sha256sum; then
                _calculated_checksum="$(sha256sum -b "$_file" | awk '{printf $1}')"
            elif check_cmd shasum; then
                _calculated_checksum="$(shasum -a 256 -b "$_file" | awk '{printf $1}')"
            elif check_cmd openssl; then
                _calculated_checksum="$(openssl dgst -sha256 "$_file" | awk '{printf $NF}')"
            else
                err "cannot verify sha256 checksum: install sha256sum, shasum, or openssl"
            fi
            ;;
"""
_POWERSHELL_DOWNLOAD_ANCHOR = """  Invoke-DownloadFile -client $wc -url $url -path $dir_path

  Write-Verbose "Unpacking to $tmp"
"""
_POWERSHELL_VERIFY_TEMPLATE = """  Invoke-DownloadFile -client $wc -url $url -path $dir_path

  $observed_sha256 = (Get-FileHash -LiteralPath $dir_path -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
  if ($observed_sha256 -ne $archive_sha256) {
    throw "SHA-256 mismatch for $artifact_name"
  }

  Write-Verbose "Unpacking to $tmp"
"""


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
    tag: str,
    version: str,
) -> tuple[tuple[str, ...], dict[str, dict[str, object]]]:
    if manifest.get("announcement_tag") != tag:
        raise ReleaseArtifactError("cargo-dist plan has the wrong release tag")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict):
        raise ReleaseArtifactError("cargo-dist plan has no artifact set")
    names = {
        _require_root_filename(name, label="cargo-dist artifact name")
        for name in artifacts
    }
    _validate_asset_contract(names)
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
    for name in sorted(names):
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
    return tuple(sorted(names)), typed


def _staged_plan_asset_names(root: Path, *, version: str) -> tuple[str, ...]:
    plan = _read_json_object(Path(root) / DIST_INPUT_MANIFEST)
    names, _ = _validate_plan_manifest(
        plan,
        tag=f"v{version}",
        version=version,
    )
    return names


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


def _replace_exactly_once(text: str, old: str, new: str, *, label: str) -> str:
    if text.count(old) != 1:
        raise ReleaseArtifactError(f"cargo-dist installer template drifted at {label}")
    return text.replace(old, new, 1)


def _read_installer(path: Path) -> str:
    require_regular_input(path, "generated installer")
    if path.stat().st_size > INSTALLER_MAX_BYTES:
        raise ReleaseArtifactError(f"installer exceeds verification budget: {path.name}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise ReleaseArtifactError(f"installer is not UTF-8: {path.name}") from error


def _require_shell_archive_digest_pairs(
    text: str,
    archives: tuple[str, ...],
    digests: dict[str, str],
    *,
    installer_name: str,
) -> None:
    for name in archives:
        marker = f'        "{name}")\n'
        if text.count(marker) != 1:
            raise ReleaseArtifactError(
                f"installer archive selection is ambiguous for {installer_name}: {name}"
            )
        start = text.index(marker)
        end = text.find("\n            ;;", start)
        if end < 0 or f'_checksum_value="{digests[name]}"' not in text[start:end]:
            raise ReleaseArtifactError(
                f"installer archive checksum mapping differs for {installer_name}: {name}"
            )


def harden_installers(root: Path, *, version: str) -> None:
    """Make cargo-dist 0.32.0 script installers fail closed on SHA-256 verification."""
    root = Path(root)
    version = _require_version(version)
    names = _staged_plan_asset_names(root, version=version)
    archives = _archive_names(names)
    digests = {
        name: read_checksum(root / f"{name}.sha256", name) for name in archives
    }
    for package in PACKAGES:
        package_archives = tuple(name for name in archives if name.startswith(f"{package}-"))
        windows_archives = tuple(name for name in package_archives if name.endswith(".zip"))
        if len(windows_archives) != 1:
            raise ReleaseArtifactError(
                f"PowerShell hardening requires one Windows archive for {package}"
            )

        shell_path = root / f"{package}-installer.sh"
        shell = _read_installer(shell_path)
        shell = _replace_exactly_once(
            shell,
            _SHELL_SHA256_FAIL_OPEN,
            _SHELL_SHA256_FAIL_CLOSED,
            label=f"{shell_path.name} SHA-256 verifier",
        )
        shell_path.write_text(shell, encoding="utf-8")

        windows_archive = windows_archives[0]
        powershell_path = root / f"{package}-installer.ps1"
        powershell = _read_installer(powershell_path)
        version_anchor = f"$app_version = '{version}'\n"
        powershell = _replace_exactly_once(
            powershell,
            version_anchor,
            version_anchor + f"$archive_sha256 = '{digests[windows_archive]}'\n",
            label=f"{powershell_path.name} checksum declaration",
        )
        powershell = _replace_exactly_once(
            powershell,
            _POWERSHELL_DOWNLOAD_ANCHOR,
            _POWERSHELL_VERIFY_TEMPLATE,
            label=f"{powershell_path.name} download verifier",
        )
        powershell_path.write_text(powershell, encoding="utf-8")

    _validate_installers(root, names, version=version)


def prepare_global_inputs(
    local_producers: Path,
    verified_archives: Path,
    plan_manifest: Path,
    destination: Path,
    *,
    tag: str,
    version: str,
) -> Path:
    """Stage only central-verifier snapshots for cargo-dist global generation."""
    version = _require_version(version)
    if tag != f"v{version}":
        raise ReleaseArtifactError("release tag and version differ")
    plan = _read_json_object(plan_manifest)
    names, plan_artifacts = _validate_plan_manifest(
        plan,
        tag=tag,
        version=version,
    )
    archives = _archive_names(names)
    targets = sorted({_asset_identity(name)[1]["target"] for name in archives})
    expected_producers = {f"artifacts-build-local-{target}" for target in targets}
    if _root_directories(local_producers) != expected_producers:
        raise ReleaseArtifactError("local artifact producer set differs from release targets")
    if _root_regular_files(verified_archives) != set(archives):
        raise ReleaseArtifactError("verified archive set differs from release targets")

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
            text = _read_installer(path)
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
                *(digests[name] for name in referenced_archives),
            }
            if extension == "sh":
                _require_shell_archive_digest_pairs(
                    text,
                    referenced_archives,
                    digests,
                    installer_name=path.name,
                )
                required.update(
                    {
                        _SHELL_SHA256_FAIL_CLOSED,
                        "sha256sum -b",
                        "shasum -a 256 -b",
                        "openssl dgst -sha256",
                    }
                )
            else:
                required.update(
                    {
                        "$archive_sha256 =",
                        "Get-FileHash -LiteralPath $dir_path -Algorithm SHA256 -ErrorAction Stop",
                        "throw \"SHA-256 mismatch for $artifact_name\"",
                    }
                )
            missing = sorted(value for value in required if value not in text)
            if missing:
                raise ReleaseArtifactError(
                    f"installer contract is incomplete for {path.name}: {missing!r}"
                )
            if extension == "sh" and _SHELL_SHA256_FAIL_OPEN in text:
                raise ReleaseArtifactError(
                    f"installer retains fail-open SHA-256 verification: {path.name}"
                )


def assemble_bundle(
    generated_root: Path,
    verified_archives: Path,
    destination: Path,
    *,
    version: str,
    source_sha: str,
) -> Path:
    """Assemble final assets without accepting an unverified archive fallback."""
    version = _require_version(version)
    source_sha = _require_source_sha(source_sha)
    names = _staged_plan_asset_names(generated_root, version=version)
    expected_generated = {*names, DIST_INPUT_MANIFEST, DIST_OUTPUT_MANIFEST}
    if _root_regular_files(generated_root) != expected_generated:
        raise ReleaseArtifactError("generated release payload differs from the asset contract")
    archives = _archive_names(names)
    if _root_regular_files(verified_archives) != set(archives):
        raise ReleaseArtifactError("verified archive set differs from the asset contract")
    generated_archive_digests = _verify_checksums(generated_root, names)
    _validate_installers(generated_root, names, version=version)

    generated_manifest = _read_json_object(Path(generated_root) / DIST_OUTPUT_MANIFEST)
    generated_names, _ = _validate_plan_manifest(
        generated_manifest,
        tag=f"v{version}",
        version=version,
    )
    if generated_names != names:
        raise ReleaseArtifactError("cargo-dist generated artifact set differs from its plan")

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


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare-global")
    prepare.add_argument("local_producers", type=Path)
    prepare.add_argument("verified_archives", type=Path)
    prepare.add_argument("plan_manifest", type=Path)
    prepare.add_argument("destination", type=Path)
    prepare.add_argument("--tag", required=True)
    prepare.add_argument("--version", required=True)

    assemble = commands.add_parser("assemble")
    assemble.add_argument("generated_root", type=Path)
    assemble.add_argument("verified_archives", type=Path)
    assemble.add_argument("destination", type=Path)
    assemble.add_argument("--version", required=True)
    assemble.add_argument("--source-sha", required=True)

    harden = commands.add_parser("harden-installers")
    harden.add_argument("generated_root", type=Path)
    harden.add_argument("--version", required=True)

    verify = commands.add_parser("verify-bundle")
    verify.add_argument("root", type=Path)
    verify.add_argument("--version", required=True)
    verify.add_argument("--source-sha", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    if args.command == "prepare-global":
        prepare_global_inputs(
            args.local_producers,
            args.verified_archives,
            args.plan_manifest,
            args.destination,
            tag=args.tag,
            version=args.version,
        )
    elif args.command == "assemble":
        assemble_bundle(
            args.generated_root,
            args.verified_archives,
            args.destination,
            version=args.version,
            source_sha=args.source_sha,
        )
    elif args.command == "harden-installers":
        harden_installers(
            args.generated_root,
            version=args.version,
        )
    elif args.command == "verify-bundle":
        verify_bundle(args.root, version=args.version, source_sha=args.source_sha)
    else:
        raise AssertionError(f"unhandled command: {args.command}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveVerificationError, ReleaseArtifactError, OSError) as error:
        print(f"release_artifact_bundle.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
