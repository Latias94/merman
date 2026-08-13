#!/usr/bin/env python3
"""Generate stable Scoop and WinGet draft candidates from a verified CLI archive."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Mapping
from dataclasses import dataclass
import json
from pathlib import Path
import re
import sys
import tempfile
import tomllib
from typing import Any

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
TEMPLATE_ROOT = ROOT / "distribution" / "cli" / "registry-templates"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
PACKAGE_IDENTIFIER = "Latias94.MermanCLI"
SCOOP_OUTPUT = Path("scoop/merman-cli.json")
WINGET_ROOT = Path("winget/manifests/l/Latias94/MermanCLI")
WINGET_TEMPLATES = (
    ("", "winget.version.template.yaml"),
    (".installer", "winget.installer.template.yaml"),
    (".locale.en-US", "winget.locale.en-US.template.yaml"),
)
PLACEHOLDER_RE = re.compile(r"\$\{[A-Z][A-Z0-9_]*\}")
GITHUB_REPOSITORY_RE = re.compile(r"https://github\.com/[^/?#\s]+/[^/?#\s]+")

ArchiveVerifier = Callable[..., VerificationReport]


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
    manifests: tuple[Path, ...]


def _load_workspace_release(repo_root: Path, requested_version: str) -> WorkspaceRelease:
    manifest_path = repo_root / "Cargo.toml"
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = manifest["workspace"]["package"]
        workspace_version = package["version"]
        repository_url = package["repository"]
    except (OSError, UnicodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise CandidateGenerationError(
            f"cannot read workspace release metadata from {manifest_path}: {error}"
        ) from error
    if workspace_version != requested_version:
        raise CandidateGenerationError(
            "requested version must exactly match workspace.package.version: "
            f"requested {requested_version!r}, workspace {workspace_version!r}"
        )
    if not isinstance(repository_url, str):
        raise CandidateGenerationError("workspace.package.repository must be a string")
    _validate_repository_url(repository_url)
    return WorkspaceRelease(workspace_version, repository_url)


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


def _validate_repository_url(value: str) -> None:
    if GITHUB_REPOSITORY_RE.fullmatch(value) is None or value.endswith(".git"):
        raise CandidateGenerationError("workspace repository must be a canonical GitHub URL")


def _load_template(path: Path) -> dict[str, Any]:
    return JSON_CONTRACT.object(JSON_CONTRACT.load(path), str(path))


def _render_template(value: Any, replacements: Mapping[str, str], context: str) -> Any:
    if isinstance(value, dict):
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
            raise CandidateGenerationError(f"{context} contains unknown placeholder {value!r}")
    return value


def _render_scoop_template(path: Path, replacements: Mapping[str, str]) -> dict[str, Any]:
    return JSON_CONTRACT.object(
        _render_template(_load_template(path), replacements, "Scoop manifest"),
        "Scoop manifest",
    )


def _render_text_template(path: Path, replacements: Mapping[str, str]) -> bytes:
    try:
        rendered = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise CandidateGenerationError(f"cannot read registry candidate template {path}: {error}") from error
    for placeholder, replacement in replacements.items():
        rendered = rendered.replace(placeholder, replacement)
    unknown = PLACEHOLDER_RE.search(rendered)
    if unknown is not None:
        raise CandidateGenerationError(
            f"{path} contains unknown placeholder {unknown.group(0)!r}"
        )
    return rendered.encode("utf-8")


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
    if not verified_bundle.is_dir():
        raise CandidateGenerationError(f"verified bundle must be a directory: {verified_bundle}")
    output_dir = Path(output_dir)
    if output_dir.exists():
        raise CandidateGenerationError(f"candidate output directory must not exist: {output_dir}")
    if not output_dir.parent.is_dir():
        raise CandidateGenerationError(
            f"candidate output parent must be a directory: {output_dir.parent}"
        )

    archive_name = release_archive_name(WINDOWS_TARGET)
    archive = verified_bundle / archive_name
    checksum = verified_bundle / f"{archive_name}.sha256"
    if not archive.is_file() or not checksum.is_file():
        raise CandidateGenerationError("verified bundle is missing the Windows archive or checksum")

    with tempfile.TemporaryDirectory(prefix="merman-cli-registry-verify-") as directory:
        report = verifier(
            archive,
            checksum,
            target=WINDOWS_TARGET,
            version=version,
            repo_root=repo_root,
            verified_output=Path(directory) / archive_name,
        )
    expected_binary_path = release_binary_archive_path(WINDOWS_TARGET)
    if report.target != WINDOWS_TARGET or report.binary_path != expected_binary_path:
        raise CandidateGenerationError("archive verifier returned a mismatched Windows contract")
    if not isinstance(report.digest, str) or re.fullmatch(r"[0-9a-f]{64}", report.digest) is None:
        raise CandidateGenerationError("archive verifier returned an invalid SHA-256 digest")

    release_tag = f"v{version}"
    release_url = f"{workspace.repository_url}/releases/tag/{release_tag}"
    archive_url = f"{workspace.repository_url}/releases/download/{release_tag}/{archive_name}"
    replacements = {
        "${VERSION}": version,
        "${REPOSITORY_URL}": workspace.repository_url,
        "${ISSUES_URL}": f"{workspace.repository_url}/issues",
        "${RELEASE_URL}": release_url,
        "${ARCHIVE_URL}": archive_url,
        "${AUTOUPDATE_URL}": (
            f"{workspace.repository_url}/releases/download/v$version/{archive_name}"
        ),
        "${SHA256}": report.digest,
        "${SHA256_UPPER}": report.digest.upper(),
    }
    scoop_manifest = _render_scoop_template(
        template_root / "scoop.template.json", replacements
    )
    contents: dict[Path, bytes] = {
        SCOOP_OUTPUT: (json.dumps(scoop_manifest, indent=2) + "\n").encode("utf-8")
    }
    winget_version_root = WINGET_ROOT / version
    for suffix, template_name in WINGET_TEMPLATES:
        path = winget_version_root / f"{PACKAGE_IDENTIFIER}{suffix}.yaml"
        contents[path] = _render_text_template(template_root / template_name, replacements)

    output_dir.mkdir()
    for relative_path, body in sorted(contents.items(), key=lambda item: item[0].as_posix()):
        destination = output_dir / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(body)

    manifests = tuple(
        output_dir / path for path in sorted(contents, key=Path.as_posix)
    )
    return CandidateSet(version, output_dir, manifests)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("verified_bundle", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
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
