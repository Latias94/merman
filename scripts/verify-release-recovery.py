#!/usr/bin/env python3
"""Verify that a release recovery commit changes only admitted packaging metadata."""

from __future__ import annotations

import argparse
import copy
from pathlib import Path
import subprocess
import sys
import tomllib


RECOVERY_PATHS = (
    "crates/merman-cli/Cargo.toml",
    "crates/merman-lsp/Cargo.toml",
    "scripts/test_verify_cli_release_archive.py",
    "scripts/test_verify_lsp_release_archive.py",
    "scripts/verify_cli_release_archive.py",
    "scripts/verify_lsp_release_archive.py",
)
EXPECTED_DIST_INCLUDES = {
    "crates/merman-cli/Cargo.toml": [
        "../../CHANGELOG.md",
        "../../LICENSE-APACHE",
        "../../LICENSE-MIT",
        "../../THIRD_PARTY_NOTICES.md",
        "../../THIRD_PARTY_LICENSES/",
        "README.md",
        "assets/completions/",
        "assets/man/",
    ],
    "crates/merman-lsp/Cargo.toml": [
        "../../CHANGELOG.md",
        "../../LICENSE-APACHE",
        "../../LICENSE-MIT",
        "../../THIRD_PARTY_NOTICES.md",
        "../../THIRD_PARTY_LICENSES/",
        "README.md",
    ],
}
TRUSTED_FILES = (
    "scripts/ascii_capability_contract.py",
    "scripts/test_verify_cli_release_archive.py",
    "scripts/test_verify_lsp_release_archive.py",
    "scripts/verify_cli_release_archive.py",
    "scripts/verify_lsp_release_archive.py",
)


class RecoveryVerificationError(ValueError):
    """Raised when a recovery source changes release semantics."""


def run_git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def resolve_commit(root: Path, revision: str, label: str) -> str:
    result = run_git(root, "rev-parse", "--verify", f"{revision}^{{commit}}")
    resolved = result.stdout.strip()
    if resolved != revision:
        raise RecoveryVerificationError(
            f"{label} must be the canonical 40-character commit SHA; resolved {resolved}"
        )
    return resolved


def read_toml_at(root: Path, revision: str, relative: str) -> dict[str, object]:
    result = run_git(root, "show", f"{revision}:{relative}")
    return tomllib.loads(result.stdout)


def verify_manifest_recovery(root: Path, tag_sha: str, relative: str) -> None:
    current = tomllib.loads((root / relative).read_text(encoding="utf-8"))
    tagged = read_toml_at(root, tag_sha, relative)
    actual_include = current["package"]["metadata"]["dist"]["include"]
    expected_include = EXPECTED_DIST_INCLUDES[relative]
    if actual_include != expected_include:
        raise RecoveryVerificationError(
            f"{relative} has an unexpected recovery include list"
        )

    normalized = copy.deepcopy(current)
    normalized["package"]["metadata"]["dist"]["include"] = tagged["package"][
        "metadata"
    ]["dist"]["include"]
    if normalized != tagged:
        raise RecoveryVerificationError(
            f"{relative} changes Cargo semantics beyond package.metadata.dist.include"
        )


def blob_oid(root: Path, revision: str, relative: str) -> str:
    return run_git(root, "rev-parse", f"{revision}:{relative}").stdout.strip()


def verify_recovery_paths(changed: tuple[str, ...]) -> None:
    if not changed:
        raise RecoveryVerificationError("recovery commit does not change any admitted path")

    unexpected = tuple(sorted(set(changed).difference(RECOVERY_PATHS)))
    if unexpected:
        raise RecoveryVerificationError(
            "recovery commit changes unexpected paths: " + ", ".join(unexpected)
        )


def verify_recovery(
    repo_root: Path,
    *,
    tag_sha: str,
    source_sha: str,
    trusted_sha: str,
) -> None:
    root = repo_root.resolve()
    resolve_commit(root, tag_sha, "tag_sha")
    resolve_commit(root, source_sha, "source_sha")
    resolve_commit(root, trusted_sha, "trusted_sha")

    if run_git(
        root,
        "merge-base",
        "--is-ancestor",
        tag_sha,
        source_sha,
        check=False,
    ).returncode:
        raise RecoveryVerificationError(
            f"recovery commit {source_sha} must descend from release tag commit {tag_sha}"
        )

    changed = tuple(
        sorted(
            line
            for line in run_git(
                root,
                "diff",
                "--name-only",
                f"{tag_sha}..{source_sha}",
            ).stdout.splitlines()
            if line
        )
    )
    verify_recovery_paths(changed)

    for relative in EXPECTED_DIST_INCLUDES:
        verify_manifest_recovery(root, tag_sha, relative)

    for relative in TRUSTED_FILES:
        if blob_oid(root, source_sha, relative) != blob_oid(root, trusted_sha, relative):
            raise RecoveryVerificationError(
                f"recovery file {relative} does not match trusted source {trusted_sha}"
            )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--tag-sha", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--trusted-sha", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    verify_recovery(
        args.repo_root,
        tag_sha=args.tag_sha,
        source_sha=args.source_sha,
        trusted_sha=args.trusted_sha,
    )
    print(
        "release recovery source is limited to admitted cargo-dist metadata and trusted verifiers"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, tomllib.TOMLDecodeError, RecoveryVerificationError) as error:
        print(f"verify-release-recovery.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
