#!/usr/bin/env python3
"""Generate stable Scoop and WinGet candidates from a verified CLI archive."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import tomllib
from typing import Any, TypeAlias
from urllib.parse import urlsplit

try:
    from scripts.release_version import parse_release_version
    from scripts.strict_json import StrictJsonContract
    from scripts.verify_cli_release_archive import (
        ArchiveVerificationError,
        VerificationReport,
        release_archive_name,
        release_binary_archive_path,
        verify_release_archive,
    )
except ModuleNotFoundError:
    from release_version import parse_release_version
    from strict_json import StrictJsonContract
    from verify_cli_release_archive import (
        ArchiveVerificationError,
        VerificationReport,
        release_archive_name,
        release_binary_archive_path,
        verify_release_archive,
    )


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE_ROOT = ROOT / "packaging" / "cli-registry"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
PACKAGE_IDENTIFIER = "Latias94.MermanCLI"
WINGET_MANIFEST_VERSION = "1.12.0"
SCOOP_OUTPUT = Path("scoop/merman-cli.json")
WINGET_ROOT = Path("winget/manifests/l/Latias94/MermanCLI")
RECEIPT_OUTPUT = Path("candidate-receipt.json")
PLACEHOLDER_RE = re.compile(r"\$\{[A-Z][A-Z0-9_]*\}")
REPOSITORY_COMPONENT_RE = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9_.-]{0,98}[A-Za-z0-9])?")

ArchiveVerifier: TypeAlias = Callable[..., VerificationReport]


class CandidateGenerationError(RuntimeError):
    """Raised when registry candidates cannot be bound to verified release bytes."""


JSON_CONTRACT = StrictJsonContract(
    error_factory=CandidateGenerationError,
    read_error_prefix="cannot read registry candidate template",
)


@dataclass(frozen=True)
class WorkspaceRelease:
    version: str
    repository_url: str


@dataclass(frozen=True)
class CandidateSet:
    version: str
    output_dir: Path
    receipt: Path
    manifests: tuple[Path, ...]


def _require_regular_file(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise CandidateGenerationError(f"{label} must be a regular file: {path}")


def _load_workspace_release(repo_root: Path, requested_version: str) -> WorkspaceRelease:
    manifest_path = repo_root / "Cargo.toml"
    _require_regular_file(manifest_path, "workspace manifest")
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = manifest["workspace"]["package"]
        workspace_version = package["version"]
        repository_url = package["repository"]
    except (OSError, UnicodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise CandidateGenerationError(
            f"cannot read workspace release metadata from {manifest_path}: {error}"
        ) from error
    if not isinstance(workspace_version, str) or workspace_version != requested_version:
        raise CandidateGenerationError(
            "requested version must exactly match workspace.package.version: "
            f"requested {requested_version!r}, workspace {workspace_version!r}"
        )
    if not isinstance(repository_url, str):
        raise CandidateGenerationError("workspace.package.repository must be a string")
    _validate_repository_url(repository_url)
    return WorkspaceRelease(
        version=workspace_version,
        repository_url=repository_url,
    )


def _validate_stable_version(value: str) -> str:
    try:
        version = parse_release_version(value, allow_v_prefix=False)
    except ValueError as error:
        raise CandidateGenerationError(str(error)) from error
    if version.kind != "stable" or version.build_metadata is not None:
        raise CandidateGenerationError(
            "Scoop and WinGet candidates require a stable X.Y.Z version without build metadata"
        )
    return version.canonical


def _validate_repository_url(value: str) -> tuple[str, str]:
    if value.strip() != value or value.endswith("/"):
        raise CandidateGenerationError(
            "repository URL must be a canonical immutable HTTPS GitHub repository URL"
        )
    try:
        parsed = urlsplit(value)
    except ValueError as error:
        raise CandidateGenerationError(
            "repository URL must be a canonical immutable HTTPS GitHub repository URL"
        ) from error
    if (
        parsed.scheme != "https"
        or parsed.netloc != "github.com"
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise CandidateGenerationError(
            "repository URL must be a canonical immutable HTTPS GitHub repository URL"
        )
    parts = parsed.path.removeprefix("/").split("/")
    if (
        len(parts) != 2
        or any(REPOSITORY_COMPONENT_RE.fullmatch(part) is None for part in parts)
        or parts[1].endswith(".git")
    ):
        raise CandidateGenerationError(
            "repository URL must contain exactly one GitHub owner and repository"
        )
    return parts[0], parts[1]


def _load_template(path: Path) -> dict[str, Any]:
    _require_regular_file(path, "registry candidate template")
    return JSON_CONTRACT.object(JSON_CONTRACT.load(path), str(path))


def _render_template(value: Any, replacements: Mapping[str, str], context: str = "template") -> Any:
    if isinstance(value, dict):
        if not all(isinstance(key, str) for key in value):
            raise CandidateGenerationError(f"{context} contains a non-string object key")
        return {
            key: _render_template(child, replacements, f"{context}.{key}")
            for key, child in value.items()
        }
    if isinstance(value, list):
        return [
            _render_template(child, replacements, f"{context}[{index}]")
            for index, child in enumerate(value)
        ]
    if isinstance(value, str):
        if value in replacements:
            return replacements[value]
        if PLACEHOLDER_RE.search(value):
            raise CandidateGenerationError(
                f"{context} contains an unknown or embedded placeholder: {value!r}"
            )
        return value
    if value is None or isinstance(value, (bool, int)):
        return value
    raise CandidateGenerationError(f"{context} contains unsupported JSON data")


def _expect_exact_fields(value: Any, fields: Sequence[str], context: str) -> dict[str, Any]:
    result = JSON_CONTRACT.object(value, context)
    JSON_CONTRACT.exact_fields(result, fields, context)
    return result


def _validate_scoop_manifest(
    template: dict[str, Any],
    *,
    version: str,
    repository_url: str,
    archive_url: str,
    digest: str,
    autoupdate_url: str,
) -> dict[str, Any]:
    _expect_exact_fields(template, ["schema_version", "manifest"], "Scoop template")
    if template["schema_version"] != 1:
        raise CandidateGenerationError("Scoop template schema_version must be 1")
    manifest = _expect_exact_fields(
        template["manifest"],
        [
            "version",
            "description",
            "homepage",
            "license",
            "architecture",
            "bin",
            "checkver",
            "autoupdate",
        ],
        "Scoop manifest",
    )
    architecture = _expect_exact_fields(manifest["architecture"], ["64bit"], "Scoop architecture")
    download = _expect_exact_fields(architecture["64bit"], ["url", "hash"], "Scoop x86_64 download")
    checkver = _expect_exact_fields(manifest["checkver"], ["github"], "Scoop checkver")
    autoupdate = _expect_exact_fields(manifest["autoupdate"], ["architecture"], "Scoop autoupdate")
    auto_arch = _expect_exact_fields(
        autoupdate["architecture"], ["64bit"], "Scoop autoupdate architecture"
    )
    auto_download = _expect_exact_fields(
        auto_arch["64bit"], ["url"], "Scoop autoupdate x86_64 download"
    )
    expected = {
        "version": version,
        "homepage": repository_url,
        "license": "MIT|Apache-2.0",
        "bin": "merman-cli.exe",
    }
    for field, expected_value in expected.items():
        if manifest[field] != expected_value:
            raise CandidateGenerationError(
                f"Scoop manifest {field} must be {expected_value!r}"
            )
    if not isinstance(manifest["description"], str) or not manifest["description"]:
        raise CandidateGenerationError("Scoop manifest description must be non-empty")
    if download != {"url": archive_url, "hash": digest}:
        raise CandidateGenerationError("Scoop download must match the verified Windows archive")
    if checkver != {"github": repository_url}:
        raise CandidateGenerationError("Scoop checkver must use the canonical repository URL")
    if auto_download != {"url": autoupdate_url}:
        raise CandidateGenerationError("Scoop autoupdate must preserve the immutable release shape")
    return manifest


def _validate_winget_template(
    template: dict[str, Any],
    *,
    version: str,
    repository_url: str,
    archive_url: str,
    digest: str,
    release_url: str,
) -> list[dict[str, Any]]:
    _expect_exact_fields(
        template,
        ["schema_version", "manifest_version", "documents"],
        "WinGet template",
    )
    if template["schema_version"] != 1:
        raise CandidateGenerationError("WinGet template schema_version must be 1")
    if template["manifest_version"] != WINGET_MANIFEST_VERSION:
        raise CandidateGenerationError(
            f"WinGet template manifest_version must be {WINGET_MANIFEST_VERSION}"
        )
    documents = JSON_CONTRACT.array(template["documents"], "WinGet documents")
    if len(documents) != 3:
        raise CandidateGenerationError("WinGet template must contain exactly three documents")
    by_kind: dict[str, dict[str, Any]] = {}
    expected_suffixes = {
        "version": "",
        "installer": ".installer",
        "defaultLocale": ".locale.en-US",
    }
    for index, item in enumerate(documents):
        document = _expect_exact_fields(
            item, ["kind", "suffix", "manifest"], f"WinGet document {index}"
        )
        kind = document["kind"]
        if not isinstance(kind, str):
            raise CandidateGenerationError("WinGet document kind must be a string")
        if kind not in expected_suffixes or kind in by_kind:
            raise CandidateGenerationError(f"unexpected or duplicate WinGet document kind: {kind!r}")
        if document["suffix"] != expected_suffixes[kind]:
            raise CandidateGenerationError(f"WinGet {kind} document has an invalid suffix")
        manifest = JSON_CONTRACT.object(document["manifest"], f"WinGet {kind} manifest")
        if manifest.get("PackageIdentifier") != PACKAGE_IDENTIFIER:
            raise CandidateGenerationError(f"WinGet {kind} manifest has an invalid package identifier")
        if manifest.get("PackageVersion") != version:
            raise CandidateGenerationError(f"WinGet {kind} manifest has an invalid package version")
        if manifest.get("ManifestType") != kind:
            raise CandidateGenerationError(f"WinGet {kind} manifest has an invalid manifest type")
        if manifest.get("ManifestVersion") != WINGET_MANIFEST_VERSION:
            raise CandidateGenerationError(f"WinGet {kind} manifest has an invalid schema version")
        by_kind[kind] = document

    version_manifest = by_kind["version"]["manifest"]
    _expect_exact_fields(
        version_manifest,
        ["PackageIdentifier", "PackageVersion", "DefaultLocale", "ManifestType", "ManifestVersion"],
        "WinGet version manifest",
    )
    if version_manifest["DefaultLocale"] != "en-US":
        raise CandidateGenerationError("WinGet default locale must be en-US")

    installer = by_kind["installer"]["manifest"]
    _expect_exact_fields(
        installer,
        [
            "PackageIdentifier",
            "PackageVersion",
            "InstallerType",
            "NestedInstallerType",
            "Commands",
            "Installers",
            "ManifestType",
            "ManifestVersion",
        ],
        "WinGet installer manifest",
    )
    if installer["InstallerType"] != "zip" or installer["NestedInstallerType"] != "portable":
        raise CandidateGenerationError("WinGet installer must be a ZIP with nested portable semantics")
    if installer["Commands"] != ["merman-cli"]:
        raise CandidateGenerationError("WinGet installer must expose only the merman-cli command")
    installers = JSON_CONTRACT.array(installer["Installers"], "WinGet installers")
    if len(installers) != 1:
        raise CandidateGenerationError("WinGet candidate must advertise only Windows x86_64")
    x64 = _expect_exact_fields(
        installers[0],
        [
            "Architecture",
            "InstallerUrl",
            "InstallerSha256",
            "Dependencies",
            "NestedInstallerFiles",
        ],
        "WinGet x86_64 installer",
    )
    if x64["Architecture"] != "x64":
        raise CandidateGenerationError("WinGet candidate must advertise only x64 architecture")
    if x64["InstallerUrl"] != archive_url or x64["InstallerSha256"] != digest.upper():
        raise CandidateGenerationError("WinGet installer must match the verified Windows archive")
    dependencies = _expect_exact_fields(
        x64["Dependencies"], ["PackageDependencies"], "WinGet x86_64 dependencies"
    )
    if dependencies["PackageDependencies"] != [
        {"PackageIdentifier": "Microsoft.VCRedist.2015+.x64"}
    ]:
        raise CandidateGenerationError(
            "WinGet x86_64 installer must declare the x64 MSVC runtime dependency"
        )
    if x64["NestedInstallerFiles"] != [
        {"RelativeFilePath": "merman-cli.exe", "PortableCommandAlias": "merman-cli"}
    ]:
        raise CandidateGenerationError("WinGet nested installer path must be merman-cli.exe")

    locale = by_kind["defaultLocale"]["manifest"]
    required_locale_fields = [
        "PackageIdentifier",
        "PackageVersion",
        "PackageLocale",
        "Publisher",
        "PublisherUrl",
        "PublisherSupportUrl",
        "PackageName",
        "PackageUrl",
        "License",
        "ShortDescription",
        "Description",
        "Moniker",
        "Tags",
        "ReleaseNotesUrl",
        "ManifestType",
        "ManifestVersion",
    ]
    _expect_exact_fields(locale, required_locale_fields, "WinGet default locale manifest")
    expected_locale = {
        "PackageLocale": "en-US",
        "Publisher": "Latias94",
        "PublisherUrl": repository_url,
        "PublisherSupportUrl": f"{repository_url}/issues",
        "PackageName": "Merman CLI",
        "PackageUrl": repository_url,
        "License": "MIT OR Apache-2.0",
        "Moniker": "merman-cli",
        "ReleaseNotesUrl": release_url,
    }
    for field, expected_value in expected_locale.items():
        if locale[field] != expected_value:
            raise CandidateGenerationError(
                f"WinGet locale {field} must be {expected_value!r}"
            )
    if not isinstance(locale["ShortDescription"], str) or not locale["ShortDescription"]:
        raise CandidateGenerationError("WinGet ShortDescription must be non-empty")
    if not isinstance(locale["Description"], str) or not locale["Description"]:
        raise CandidateGenerationError("WinGet Description must be non-empty")
    if locale["Tags"] != ["cli", "diagram", "headless", "mermaid", "renderer"]:
        raise CandidateGenerationError("WinGet Tags must match the reviewed candidate contract")
    return [by_kind[kind] for kind in ("version", "installer", "defaultLocale")]


def _yaml_scalar(value: Any) -> str:
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=True)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int) and not isinstance(value, bool):
        return str(value)
    if value is None:
        return "null"
    raise CandidateGenerationError(f"cannot serialize YAML scalar of type {type(value).__name__}")


def _yaml_lines(value: Any, indent: int = 0) -> list[str]:
    prefix = " " * indent
    if isinstance(value, dict):
        lines: list[str] = []
        for key, child in value.items():
            if not isinstance(key, str) or re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", key) is None:
                raise CandidateGenerationError(f"cannot serialize unsafe YAML key: {key!r}")
            if isinstance(child, (dict, list)):
                if not child:
                    lines.append(f"{prefix}{key}: {'{}' if isinstance(child, dict) else '[]'}")
                else:
                    lines.append(f"{prefix}{key}:")
                    lines.extend(_yaml_lines(child, indent + 2))
            else:
                lines.append(f"{prefix}{key}: {_yaml_scalar(child)}")
        return lines
    if isinstance(value, list):
        lines = []
        for child in value:
            if isinstance(child, dict):
                if not child:
                    lines.append(f"{prefix}- {{}}")
                    continue
                first_key, first_value = next(iter(child.items()))
                if isinstance(first_value, (dict, list)):
                    lines.append(f"{prefix}-")
                    lines.extend(_yaml_lines(child, indent + 2))
                else:
                    lines.append(f"{prefix}- {first_key}: {_yaml_scalar(first_value)}")
                    remaining = dict(list(child.items())[1:])
                    lines.extend(_yaml_lines(remaining, indent + 2))
            elif isinstance(child, list):
                lines.append(f"{prefix}-")
                lines.extend(_yaml_lines(child, indent + 2))
            else:
                lines.append(f"{prefix}- {_yaml_scalar(child)}")
        return lines
    return [f"{prefix}{_yaml_scalar(value)}"]


def _render_winget_yaml(manifest: dict[str, Any], kind: str) -> bytes:
    schema_kind = "defaultLocale" if kind == "defaultLocale" else kind
    header = (
        "# yaml-language-server: "
        f"$schema=https://aka.ms/winget-manifest.{schema_kind}."
        f"{WINGET_MANIFEST_VERSION}.schema.json"
    )
    return (header + "\n\n" + "\n".join(_yaml_lines(manifest)) + "\n").encode("utf-8")


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _write_new_file(root: Path, relative_path: Path, contents: bytes) -> Path:
    destination = root / relative_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        with destination.open("xb") as stream:
            stream.write(contents)
            stream.flush()
            os.fsync(stream.fileno())
    except FileExistsError as error:
        raise CandidateGenerationError(f"candidate output already exists: {destination}") from error
    return destination


def _validate_verification_report(
    report: object,
    *,
    archive_name: str,
    binary_path: str,
) -> VerificationReport:
    try:
        report_archive = report.archive
        report_target = report.target
        report_binary_path = report.binary_path
        report_digest = report.digest
    except AttributeError as error:
        raise CandidateGenerationError("archive verifier returned an invalid report")
    if not isinstance(report_archive, Path) or report_archive.name != archive_name:
        raise CandidateGenerationError(
            f"archive verifier returned an unexpected archive path: {report_archive!r}"
        )
    if report_target != WINDOWS_TARGET:
        raise CandidateGenerationError(
            f"archive verifier returned unexpected target {report_target!r}"
        )
    if report_binary_path != binary_path:
        raise CandidateGenerationError(
            f"archive verifier returned unexpected binary path {report_binary_path!r}"
        )
    if not isinstance(report_digest, str) or re.fullmatch(r"[0-9a-f]{64}", report_digest) is None:
        raise CandidateGenerationError("archive verifier returned a malformed SHA-256 digest")
    return report  # type: ignore[return-value]


def generate_candidates(
    verified_bundle: Path,
    output_dir: Path,
    *,
    version: str,
    repo_root: Path = ROOT,
    template_root: Path = TEMPLATE_ROOT,
    verifier: ArchiveVerifier = verify_release_archive,
) -> CandidateSet:
    version = _validate_stable_version(version)
    repo_root = Path(repo_root)
    workspace = _load_workspace_release(repo_root, version)
    verified_bundle = Path(verified_bundle)
    if verified_bundle.is_symlink() or not verified_bundle.is_dir():
        raise CandidateGenerationError(
            f"verified bundle must be a real directory: {verified_bundle}"
        )
    output_dir = Path(output_dir)
    if output_dir.exists() or output_dir.is_symlink():
        raise CandidateGenerationError(f"candidate output directory must not exist: {output_dir}")
    output_parent = output_dir.parent
    if output_parent.is_symlink() or not output_parent.is_dir():
        raise CandidateGenerationError(
            f"candidate output parent must be a real directory: {output_parent}"
        )

    archive_name = release_archive_name(WINDOWS_TARGET)
    archive = verified_bundle / archive_name
    checksum = verified_bundle / f"{archive_name}.sha256"
    _require_regular_file(archive, "verified Windows archive")
    _require_regular_file(checksum, "verified Windows archive checksum")

    with tempfile.TemporaryDirectory(prefix="merman-cli-registry-verify-") as directory:
        verification_root = Path(directory)
        untrusted_report = verifier(
            archive,
            checksum,
            target=WINDOWS_TARGET,
            version=version,
            repo_root=repo_root,
            verified_output=verification_root / archive_name,
        )
    expected_binary_path = release_binary_archive_path(WINDOWS_TARGET)
    report = _validate_verification_report(
        untrusted_report,
        archive_name=archive_name,
        binary_path=expected_binary_path,
    )

    release_tag = f"v{version}"
    release_url = f"{workspace.repository_url}/releases/tag/{release_tag}"
    archive_url = (
        f"{workspace.repository_url}/releases/download/{release_tag}/{archive_name}"
    )
    autoupdate_url = (
        f"{workspace.repository_url}/releases/download/v$version/{archive_name}"
    )
    replacements = {
        "${VERSION}": version,
        "${REPOSITORY_URL}": workspace.repository_url,
        "${ISSUES_URL}": f"{workspace.repository_url}/issues",
        "${RELEASE_URL}": release_url,
        "${ARCHIVE_URL}": archive_url,
        "${AUTOUPDATE_URL}": autoupdate_url,
        "${SHA256}": report.digest,
        "${SHA256_UPPER}": report.digest.upper(),
    }
    scoop_template = _render_template(
        _load_template(template_root / "scoop.template.json"), replacements
    )
    winget_template = _render_template(
        _load_template(template_root / "winget.template.json"), replacements
    )
    scoop_manifest = _validate_scoop_manifest(
        scoop_template,
        version=version,
        repository_url=workspace.repository_url,
        archive_url=archive_url,
        digest=report.digest,
        autoupdate_url=autoupdate_url,
    )
    winget_documents = _validate_winget_template(
        winget_template,
        version=version,
        repository_url=workspace.repository_url,
        archive_url=archive_url,
        digest=report.digest,
        release_url=release_url,
    )

    contents: dict[Path, bytes] = {
        SCOOP_OUTPUT: (
            json.dumps(scoop_manifest, indent=2, ensure_ascii=True) + "\n"
        ).encode("utf-8")
    }
    winget_version_root = WINGET_ROOT / version
    for document in winget_documents:
        path = winget_version_root / f"{PACKAGE_IDENTIFIER}{document['suffix']}.yaml"
        contents[path] = _render_winget_yaml(document["manifest"], document["kind"])
    manifest_entries = [
        {
            "channel": "scoop" if path == SCOOP_OUTPUT else "winget",
            "path": path.as_posix(),
            "sha256": _sha256_bytes(body),
        }
        for path, body in sorted(contents.items(), key=lambda item: item[0].as_posix())
    ]
    receipt = {
        "schema_version": 1,
        "package": "merman-cli",
        "version": version,
        "release_tag": release_tag,
        "target": WINDOWS_TARGET,
        "source": {
            "archive_name": archive_name,
            "archive_url": archive_url,
            "sha256": report.digest,
            "binary_path": expected_binary_path,
        },
        "manifests": manifest_entries,
    }
    contents[RECEIPT_OUTPUT] = (
        json.dumps(receipt, indent=2, ensure_ascii=True) + "\n"
    ).encode("utf-8")

    staging = Path(tempfile.mkdtemp(prefix=f".{output_dir.name}.", dir=output_parent))
    try:
        for relative_path, body in sorted(contents.items(), key=lambda item: item[0].as_posix()):
            _write_new_file(staging, relative_path, body)
        os.replace(staging, output_dir)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    manifests = tuple(output_dir / entry["path"] for entry in manifest_entries)
    return CandidateSet(
        version=version,
        output_dir=output_dir,
        receipt=output_dir / RECEIPT_OUTPUT,
        manifests=manifests,
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "verified_bundle",
        type=Path,
        help="directory containing the checksum-bound Windows archive and adjacent SHA-256 file",
    )
    parser.add_argument("--version", required=True, help="stable release version without a v prefix")
    parser.add_argument("--output-dir", type=Path, required=True, help="new candidate output directory")
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help="tagged Merman repository checkout (defaults to this repository)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
        result = generate_candidates(
            args.verified_bundle,
            args.output_dir,
            version=args.version,
            repo_root=args.repo_root,
        )
        print(f"generated {len(result.manifests)} registry manifests in {result.output_dir}")
        return 0
    except (CandidateGenerationError, ArchiveVerificationError, OSError) as error:
        print(f"generate_cli_registry_candidates.py: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
