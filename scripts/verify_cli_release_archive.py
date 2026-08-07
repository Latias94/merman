#!/usr/bin/env python3
"""Verify a cargo-dist merman-cli release archive without trusting its paths."""

from __future__ import annotations

import argparse
from collections.abc import Iterable
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import xml.etree.ElementTree as ElementTree


if __package__:
    from .capability_surface_contract import (
        capability_surface_digest,
        validate_capability_authority,
    )
    from .release_archive import (
        ArchiveMember,
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        ExtractionLimits,
        VerificationReport,
        archive_member_path,
        binary_name_for,
        format_set_mismatch,
        git_tracked_legal_files,
        persist_verified_archive,
        regular_files_equal,
        release_archive_name_for,
        repository_tree_files,
        require_regular_input,
        require_repository_root,
        verified_archive_contents,
    )
    from .release_process import (
        CommandRunner,
        HostTargetChecker,
        run_checked,
        target_matches_host,
    )
else:
    from capability_surface_contract import (
        capability_surface_digest,
        validate_capability_authority,
    )
    from release_archive import (
        ArchiveMember,
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        ExtractionLimits,
        VerificationReport,
        archive_member_path,
        binary_name_for,
        format_set_mismatch,
        git_tracked_legal_files,
        persist_verified_archive,
        regular_files_equal,
        release_archive_name_for,
        repository_tree_files,
        require_regular_input,
        require_repository_root,
        verified_archive_contents,
    )
    from release_process import (
        CommandRunner,
        HostTargetChecker,
        run_checked,
        target_matches_host,
    )


__all__ = (
    "ArchiveVerificationError",
    "ExtractionLimits",
    "VerificationReport",
    "release_archive_name",
    "release_binary_archive_path",
    "verify_release_archive",
    "verify_runtime_contract",
)

PACKAGE_NAME = "merman-cli"
CAPABILITIES_SCHEMA_VERSION = 2
CLI_CONTRACT_VERSION = 3
SVG_SMOKE_SOURCE = b"flowchart LR\nA --> B\n"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
JPEG_START = b"\xff\xd8"
JPEG_END = b"\xff\xd9"
REPOSITORY_CONTRACT_MAX_BYTES = 4 * 1024 * 1024

ARTIFACT_PROFILES_PATH = "capabilities/artifact-profiles-v1.json"
CAPABILITY_SURFACE_PATH = "capabilities/feature-surface-v1.json"
UPSTREAM_REPOS_PATH = "tools/upstreams/REPOS.lock.json"
MERMAID_REFERENCE_BUNDLE_PATH = "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json"
ASSET_ROOTS = ("completions", "man")
PACKAGE_README_PATH = "README.md"
ROOT_RELEASE_PATHS = ("CHANGELOG.md", "LICENSE-APACHE", "LICENSE-MIT")
NOTICE_PATH = "THIRD_PARTY_NOTICES.md"
LICENSE_ROOT = "THIRD_PARTY_LICENSES"

def release_archive_name(target: str) -> str:
    """Return the cargo-dist archive name consumed by installation metadata."""
    return release_archive_name_for(PACKAGE_NAME, target)


def release_binary_archive_path(target: str) -> str:
    """Return the executable path before cargo-binstall strips the archive layout."""
    binary_name = _binary_name(target)
    if release_archive_name(target).endswith(".zip"):
        return binary_name
    archive_name = release_archive_name(target)
    wrapper = archive_name.removesuffix(".tar.xz")
    return f"{wrapper}/{binary_name}"


def _binary_name(target: str) -> str:
    return binary_name_for(PACKAGE_NAME, target)


def _repository_asset_files(repo_root: Path) -> dict[str, Path]:
    package_root = repo_root / "crates/merman-cli"
    result: dict[str, Path] = {}
    for asset_root in ASSET_ROOTS:
        source_root = f"assets/{asset_root}"
        files = repository_tree_files(package_root, source_root)
        if not files:
            raise ArchiveVerificationError(
                f"repository asset directory is empty: {package_root / source_root}"
            )
        for source_relative, source in files.items():
            archive_relative = source_relative.removeprefix("assets/")
            result[archive_relative] = source
    return result


def _uses_assets_prefixed_layout(path: str) -> bool:
    return path == "assets" or any(
        path == f"assets/{root}" or path.startswith(f"assets/{root}/")
        for root in ASSET_ROOTS
    )


def _require_distribution_contents(
    root: Path,
    members: Iterable[ArchiveMember],
    *,
    target: str,
    source_files: dict[str, Path],
) -> None:
    old_layout = sorted(
        member.logical_path
        for member in members
        if _uses_assets_prefixed_layout(member.logical_path)
    )
    if old_layout:
        raise ArchiveVerificationError(
            "archive uses the unsupported assets-prefixed CLI layout: "
            + ", ".join(old_layout)
        )

    regular = {
        member.logical_path: member
        for member in members
        if not member.is_directory
    }
    binary_name = _binary_name(target)
    binary_candidates = [
        path for path in regular if PurePosixPath(path).name == binary_name
    ]
    if binary_candidates != [binary_name]:
        raise ArchiveVerificationError(
            f"archive must contain exactly one root {binary_name!r} binary; "
            f"found {sorted(binary_candidates)!r}"
        )

    expected = {binary_name, *source_files}
    if set(regular) != expected:
        raise ArchiveVerificationError(
            format_set_mismatch("CLI payload", expected, set(regular))
        )

    for path in sorted(expected):
        member = regular[path]
        if member.size == 0 or archive_member_path(root, path).stat().st_size == 0:
            raise ArchiveVerificationError(f"required archive file is empty: {path!r}")

    for archive_relative, source in source_files.items():
        archived = archive_member_path(root, archive_relative)
        if not regular_files_equal(archived, source):
            raise ArchiveVerificationError(
                f"archive content differs from repository file {archive_relative!r}"
            )


def _repository_distribution_files(repo_root: Path) -> dict[str, Path]:
    return {
        PACKAGE_README_PATH: repo_root / "crates/merman-cli/README.md",
        **{relative: repo_root / relative for relative in ROOT_RELEASE_PATHS},
        **git_tracked_legal_files(repo_root),
        **_repository_asset_files(repo_root),
    }


def _strict_json_object(
    data: bytes,
    *,
    label: str = "capabilities output",
) -> dict[str, object]:
    try:
        text = data.decode("utf-8")
        value = json.loads(
            text,
            object_pairs_hook=lambda pairs: _json_object_without_duplicates(
                pairs,
                label=label,
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ArchiveVerificationError(
            f"{label} is not valid UTF-8 JSON: {error}"
        ) from error
    if not isinstance(value, dict):
        raise ArchiveVerificationError(f"{label} must be one JSON object")
    return value


def _json_object_without_duplicates(
    pairs: list[tuple[str, object]],
    *,
    label: str,
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ArchiveVerificationError(
                f"{label} contains duplicate JSON key {key!r}"
            )
        result[key] = value
    return result


def _read_repository_json(repo_root: Path, relative: str) -> dict[str, object]:
    path = repo_root / relative
    require_regular_input(path, f"repository contract {relative!r}")
    try:
        with path.open("rb") as stream:
            data = stream.read(REPOSITORY_CONTRACT_MAX_BYTES + 1)
    except OSError as error:
        raise ArchiveVerificationError(
            f"cannot read repository contract {path}: {error}"
        ) from error
    if len(data) > REPOSITORY_CONTRACT_MAX_BYTES:
        raise ArchiveVerificationError(
            f"repository contract exceeds {REPOSITORY_CONTRACT_MAX_BYTES} bytes: {path}"
        )
    return _strict_json_object(data, label=f"repository contract {relative!r}")


def _require_json_object(
    value: object,
    *,
    label: str,
    fields: set[str] | None = None,
) -> dict[str, object]:
    if type(value) is not dict:
        raise ArchiveVerificationError(f"{label} must be a JSON object")
    result = value
    if fields is not None:
        observed = set(result)
        if observed != fields:
            missing = sorted(fields - observed)
            extra = sorted(observed - fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise ArchiveVerificationError(f"{label} has " + "; ".join(details))
    return result


def _require_json_array(value: object, *, label: str) -> list[object]:
    if type(value) is not list:
        raise ArchiveVerificationError(f"{label} must be a JSON array")
    return value


def _require_json_string(value: object, *, label: str) -> str:
    if type(value) is not str or not value:
        raise ArchiveVerificationError(f"{label} must be a non-empty JSON string")
    return value


def _require_string_array(value: object, *, label: str) -> list[str]:
    values = _require_json_array(value, label=label)
    result = [
        _require_json_string(item, label=f"{label}[{index}]")
        for index, item in enumerate(values)
    ]
    if len(set(result)) != len(result):
        raise ArchiveVerificationError(f"{label} contains duplicate values")
    return result


def _cli_release_runtime_ids(
    profiles: dict[str, object],
    *,
    surface: dict[str, object],
) -> tuple[list[str], list[str]]:
    candidates = []
    for index, value in enumerate(
        _require_json_array(profiles.get("profiles"), label="artifact profiles")
    ):
        profile = _require_json_object(
            value,
            label=f"artifact profiles[{index}]",
        )
        if profile.get("id") == "cli-release":
            candidates.append(profile)
    if len(candidates) != 1:
        raise ArchiveVerificationError(
            "artifact profiles must contain exactly one cli-release profile"
        )
    profile = candidates[0]
    _require_exact_json(
        "cli-release semantic_target",
        profile.get("semantic_target"),
        "native",
    )
    cargo = _require_json_object(
        profile.get("cargo"),
        label="cli-release cargo",
    )
    _require_exact_json(
        "cli-release cargo.package",
        cargo.get("package"),
        PACKAGE_NAME,
    )
    _require_exact_json(
        "cli-release cargo.default_features",
        cargo.get("default_features"),
        False,
    )
    cargo_features = _require_string_array(
        cargo.get("features"),
        label="cli-release cargo.features",
    )
    expected = _require_json_object(
        profile.get("expected"),
        label="cli-release expected",
        fields={"capabilities", "runtime_ids", "outputs"},
    )
    capability_ids = _require_string_array(
        expected["capabilities"],
        label="cli-release expected capabilities",
    )
    runtime_ids = _require_string_array(
        expected["runtime_ids"],
        label="cli-release expected runtime_ids",
    )
    output_ids = _require_string_array(
        expected["outputs"],
        label="cli-release expected outputs",
    )
    _require_exact_json(
        "cli-release expected capabilities",
        capability_ids,
        runtime_ids,
    )
    _require_exact_json(
        "cli-release cargo.features",
        cargo_features,
        runtime_ids,
    )
    for label, values in (
        ("cli-release expected runtime_ids", runtime_ids),
        ("cli-release expected outputs", output_ids),
    ):
        if values != sorted(values):
            raise ArchiveVerificationError(f"{label} must be sorted")
    declared_ids = {
        capability["id"]
        for capability in surface["capabilities"]
    }
    unknown = sorted(set(runtime_ids) - declared_ids)
    if unknown:
        raise ArchiveVerificationError(
            "cli-release references unknown capabilities: " + ", ".join(unknown)
        )
    return runtime_ids, output_ids


def _repository_compatibility(repo_root: Path) -> dict[str, str]:
    repos = _read_repository_json(repo_root, UPSTREAM_REPOS_PATH)
    bundle = _read_repository_json(repo_root, MERMAID_REFERENCE_BUNDLE_PATH)
    locked_repos = _require_json_object(
        repos.get("repos"),
        label="upstream repository lock repos",
    )
    locked_mermaid = _require_json_object(
        locked_repos.get("mermaid"),
        label="upstream repository lock mermaid",
    )
    locked_mermaid_cli = _require_json_object(
        locked_repos.get("mermaid-cli"),
        label="upstream repository lock mermaid-cli",
    )
    locked_ref = _require_json_string(
        locked_mermaid.get("ref"),
        label="upstream repository lock mermaid.ref",
    )
    if not locked_ref.startswith("mermaid@") or len(locked_ref) == len("mermaid@"):
        raise ArchiveVerificationError(
            "upstream repository lock mermaid.ref must use mermaid@VERSION"
        )
    mermaid_version = locked_ref.removeprefix("mermaid@")
    release = _require_json_object(
        bundle.get("release"),
        label="Mermaid reference bundle release",
    )
    _require_exact_json(
        "Mermaid reference bundle release.version",
        release.get("version"),
        mermaid_version,
    )
    release_source = _require_json_object(
        release.get("source"),
        label="Mermaid reference bundle release.source",
    )
    _require_exact_json(
        "Mermaid source commit",
        release_source.get("commit"),
        _require_json_string(
            locked_mermaid.get("commit"),
            label="upstream repository lock mermaid.commit",
        ),
    )

    reference_cli = _require_json_object(
        bundle.get("referenceCli"),
        label="Mermaid reference bundle referenceCli",
    )
    reference_package = _require_json_object(
        reference_cli.get("package"),
        label="Mermaid reference bundle referenceCli.package",
    )
    _require_exact_json(
        "Mermaid reference CLI package",
        reference_package.get("package"),
        "@mermaid-js/mermaid-cli",
    )
    reference_source = _require_json_object(
        reference_package.get("source"),
        label="Mermaid reference bundle referenceCli.package.source",
    )
    locked_url = _require_json_string(
        locked_mermaid_cli.get("url"),
        label="upstream repository lock mermaid-cli.url",
    ).removesuffix(".git")
    reference_url = _require_json_string(
        reference_source.get("repository"),
        label="Mermaid reference bundle referenceCli.package.source.repository",
    ).removesuffix(".git")
    _require_exact_json(
        "Mermaid reference CLI repository",
        reference_url,
        locked_url,
    )
    return {
        "mermaid": mermaid_version,
        "mmdc": _require_json_string(
            reference_package.get("version"),
            label="Mermaid reference bundle referenceCli.package.version",
        ),
    }


def _cli_release_commands(runtime_ids: list[str]) -> list[str]:
    enabled = set(runtime_ids)
    commands = {"capabilities", "detect", "parse"}
    if enabled.intersection({"ascii", "svg"}):
        commands.add("render")
    for capability, gated_commands in (
        ("analysis", ("fix", "lint", "lint-rules")),
        ("markdown", ("batch",)),
        ("shell-completions", ("completion",)),
        ("svg", ("layout", "mmdc")),
    ):
        if capability in enabled:
            commands.update(gated_commands)
    return sorted(commands)


def _release_capabilities_contract(
    repo_root: Path,
    *,
    version: str,
) -> dict[str, object]:
    profiles = _read_repository_json(repo_root, ARTIFACT_PROFILES_PATH)
    surface = validate_capability_authority(
        profiles,
        _read_repository_json(repo_root, CAPABILITY_SURFACE_PATH),
        expected_path=CAPABILITY_SURFACE_PATH,
        error_factory=ArchiveVerificationError,
        profiles_context="artifact profiles",
        capability_context="capability surface",
        expected_schema_version=1,
        require_sorted_compiled_prerequisites=True,
    )
    digest = capability_surface_digest(surface)
    runtime_ids, expected_output_ids = _cli_release_runtime_ids(
        profiles,
        surface=surface,
    )
    enabled = set(runtime_ids)
    capabilities = [
        {
            "id": capability["id"],
            "kind": capability["kind"],
            "description": capability["description"],
            "implications": capability["implications"],
        }
        for capability in surface["capabilities"]
        if capability["id"] in enabled
    ]
    outputs = [
        {
            "id": output["id"],
            "description": output["description"],
            "media_type": output["media_type"],
        }
        for output in surface["outputs"]
        if output["capability"] in enabled
    ]
    observed_output_ids = [output["id"] for output in outputs]
    _require_exact_json(
        "cli-release expected outputs",
        expected_output_ids,
        observed_output_ids,
    )
    return {
        "schema_version": CAPABILITIES_SCHEMA_VERSION,
        "cli_contract_version": CLI_CONTRACT_VERSION,
        "package": {"name": PACKAGE_NAME, "version": version},
        "compatibility": _repository_compatibility(repo_root),
        "descriptor": {
            "schema_version": surface["schema_version"],
            "digest": digest,
        },
        "commands": _cli_release_commands(runtime_ids),
        "capabilities": capabilities,
        "outputs": outputs,
    }


def _require_exact_json(label: str, observed: object, expected: object) -> None:
    if type(observed) is not type(expected):
        raise ArchiveVerificationError(
            f"{label} has the wrong JSON type: "
            f"expected {type(expected).__name__}, got {type(observed).__name__}"
        )
    if isinstance(expected, dict):
        observed_object = observed
        expected_fields = set(expected)
        observed_fields = set(observed_object)
        if observed_fields != expected_fields:
            missing = sorted(expected_fields - observed_fields)
            extra = sorted(observed_fields - expected_fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise ArchiveVerificationError(f"{label} has " + "; ".join(details))
        for key, expected_value in expected.items():
            _require_exact_json(
                f"{label}.{key}",
                observed_object[key],
                expected_value,
            )
        return
    if isinstance(expected, list):
        observed_array = observed
        if len(observed_array) != len(expected):
            raise ArchiveVerificationError(
                f"{label} has {len(observed_array)} entries; expected {len(expected)}"
            )
        for index, expected_value in enumerate(expected):
            _require_exact_json(
                f"{label}[{index}]",
                observed_array[index],
                expected_value,
            )
        return
    if observed != expected:
        raise ArchiveVerificationError(
            f"{label} differs from the repository contract: "
            f"expected {expected!r}, got {observed!r}"
        )


def _validate_runtime_capabilities(
    observed: dict[str, object],
    expected: dict[str, object],
) -> None:
    _require_exact_json("capabilities document", observed, expected)


def _require_quiet_success(
    result: subprocess.CompletedProcess[bytes],
    *,
    label: str,
) -> bytes:
    if result.stderr:
        raise ArchiveVerificationError(f"{label} emitted unexpected stderr")
    if not result.stdout:
        raise ArchiveVerificationError(f"{label} emitted empty stdout")
    return result.stdout


def _validate_png(payload: bytes) -> None:
    if (
        len(payload) < 45
        or not payload.startswith(PNG_SIGNATURE + b"\x00\x00\x00\rIHDR")
        or not payload.endswith(b"\x00\x00\x00\x00IEND\xaeB`\x82")
    ):
        raise ArchiveVerificationError("minimal PNG render has an invalid container signature")
    width = int.from_bytes(payload[16:20], "big")
    height = int.from_bytes(payload[20:24], "big")
    if width == 0 or height == 0:
        raise ArchiveVerificationError("minimal PNG render has zero dimensions")


def _validate_jpeg(payload: bytes) -> None:
    if len(payload) < 4 or not payload.startswith(JPEG_START) or not payload.endswith(JPEG_END):
        raise ArchiveVerificationError("minimal JPEG render has an invalid container signature")


def _validate_pdf(payload: bytes) -> None:
    if (
        len(payload) < 16
        or not payload.startswith(b"%PDF-")
        or not payload.rstrip().endswith(b"%%EOF")
        or b"/Type /Page" not in payload
    ):
        raise ArchiveVerificationError("minimal PDF render has an invalid container signature")


def verify_runtime_contract(
    binary: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    runner: CommandRunner = subprocess.run,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> None:
    """Execute an explicitly approved host binary and verify stable CLI behavior."""
    if not host_target_checker(target):
        raise ArchiveVerificationError(
            f"refusing to execute archive target {target!r} on this host"
        )
    repo_root = require_repository_root(Path(repo_root))
    expected_capabilities = _release_capabilities_contract(
        repo_root,
        version=version,
    )
    command = str(binary)
    version_result = run_checked(
        [command, "--version"],
        stdin=b"",
        cwd=binary.parent,
        runner=runner,
    )
    expected_version = f"{PACKAGE_NAME} {version}\n".encode()
    if version_result.stdout != expected_version or version_result.stderr:
        raise ArchiveVerificationError(
            "--version must emit exactly one stable line on stdout and no stderr"
        )

    capabilities_result = run_checked(
        [command, "capabilities", "--json"],
        stdin=b"",
        cwd=binary.parent,
        runner=runner,
    )
    if capabilities_result.stderr:
        raise ArchiveVerificationError("capabilities --json emitted unexpected stderr")
    capabilities = _strict_json_object(capabilities_result.stdout)
    _validate_runtime_capabilities(capabilities, expected_capabilities)

    completion_result = run_checked(
        [command, "completion", "bash"],
        stdin=b"",
        cwd=binary.parent,
        runner=runner,
    )
    completion = _require_quiet_success(completion_result, label="Bash completion")
    completion_snapshot = (
        repo_root
        / "crates/merman-cli/assets/completions/merman-cli.bash"
    ).read_bytes()
    if completion != completion_snapshot:
        raise ArchiveVerificationError(
            "runtime Bash completion differs from the release source snapshot"
        )

    render_result = run_checked(
        [command, "render", "--format", "svg", "-"],
        stdin=SVG_SMOKE_SOURCE,
        cwd=binary.parent,
        runner=runner,
    )
    if render_result.stderr:
        raise ArchiveVerificationError("minimal SVG render emitted unexpected stderr")
    try:
        root = ElementTree.fromstring(render_result.stdout)
    except ElementTree.ParseError as error:
        raise ArchiveVerificationError(f"minimal render is not valid XML: {error}") from error
    if root.tag.rsplit("}", maxsplit=1)[-1] != "svg":
        raise ArchiveVerificationError("minimal render root element is not SVG")

    output_validators = (
        ("png", _validate_png),
        ("jpg", _validate_jpeg),
        ("pdf", _validate_pdf),
    )
    for output_format, validator in output_validators:
        result = run_checked(
            [command, "render", "--format", output_format, "-"],
            stdin=SVG_SMOKE_SOURCE,
            cwd=binary.parent,
            runner=runner,
        )
        payload = _require_quiet_success(
            result,
            label=f"minimal {output_format.upper()} render",
        )
        validator(payload)


def verify_release_archive(
    archive: Path,
    checksum: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    verified_output: Path | None = None,
    execute: bool = False,
    limits: ExtractionLimits = DEFAULT_LIMITS,
    runner: CommandRunner = subprocess.run,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> VerificationReport:
    """Verify one archive and optionally persist its checksum-bound bytes."""
    archive = Path(archive)
    checksum = Path(checksum)
    repo_root = require_repository_root(Path(repo_root))
    source_files = _repository_distribution_files(repo_root)
    with verified_archive_contents(
        archive,
        checksum,
        package_name=PACKAGE_NAME,
        target=target,
        version=version,
        limits=limits,
    ) as extracted:
        _require_distribution_contents(
            extracted.root,
            extracted.members,
            target=target,
            source_files=source_files,
        )
        if execute:
            verify_runtime_contract(
                archive_member_path(extracted.root, extracted.binary_path),
                target=target,
                version=version,
                repo_root=repo_root,
                runner=runner,
                host_target_checker=host_target_checker,
            )
        persisted = (
            persist_verified_archive(extracted, verified_output, limits=limits)
            if verified_output is not None
            else archive.resolve()
        )
        return VerificationReport(
            archive=persisted,
            digest=extracted.digest,
            target=target,
            member_count=len(extracted.members),
            total_uncompressed_bytes=sum(member.size for member in extracted.members),
            binary_path=extracted.binary_path,
        )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="cargo-dist .tar.xz or .zip archive")
    parser.add_argument(
        "--checksum",
        type=Path,
        help="adjacent .sha256 file (defaults to ARCHIVE.sha256)",
    )
    parser.add_argument("--target", required=True, help="Rust target triple carried by the archive")
    parser.add_argument("--version", required=True, help="expected merman-cli package version")
    parser.add_argument(
        "--repo-root",
        type=Path,
        required=True,
        help="repository root containing the exact CLI and tracked legal assets",
    )
    parser.add_argument(
        "--verified-output",
        type=Path,
        help="optional new persistent path for the checksum-bound verified archive",
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="execute the binary after structural verification when TARGET matches the host",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    checksum = args.checksum or args.archive.with_name(f"{args.archive.name}.sha256")
    report = verify_release_archive(
        args.archive,
        checksum,
        target=args.target,
        version=args.version,
        repo_root=args.repo_root,
        verified_output=args.verified_output,
        execute=args.execute,
    )
    print(f"verified {report.archive}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveVerificationError, OSError, subprocess.TimeoutExpired) as error:
        print(f"verify_cli_release_archive.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
