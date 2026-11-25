#!/usr/bin/env python3
"""Prepare, publish, and reconcile checksum-bound crates.io batches."""

from __future__ import annotations

import argparse
from collections.abc import Callable
from contextlib import AbstractContextManager, nullcontext
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import crates_io_archive, release_attempt_history, renderer_fork_consumer
from scripts.crates_io_auth import trusted_publishing_token
from scripts.release_version import parse_release_version
from scripts.verify_crate_package_legal_materials import verify_package_archive
from tools.publish import (
    PackageInfo,
    PublishPlan,
    _workspace_packages_by_name,
    cargo_metadata,
    crates_io_publish_plan,
    print_error,
    require_tool,
    run_command,
)


CRATES_IO_API = "https://crates.io/api/v1"
CRATES_IO_INDEX = "https://index.crates.io"
REGISTRY_OBSERVATION_SECONDS = 300.0
RECEIPT_SCHEMA = Path("distribution/crates-io/receipt-schema-v1.json")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_OBJECT = re.compile(r"[0-9a-f]{40}\Z")
RECEIPT_STATES = {
    "prepared",
    "complete",
    "preflight_failed",
    "pending_recovery",
    "mismatch",
}


class CratesIoPublishError(RuntimeError):
    """The receipt-bound crates.io publication cannot safely continue."""


class RegistryConflictError(CratesIoPublishError):
    """Registry evidence permanently conflicts with the immutable candidate."""


@dataclass(frozen=True)
class PreparedCrate:
    package: PackageInfo
    artifact_path: Path
    artifact_sha256: str
    artifact_size: int
    manifest_sha256: str


@dataclass(frozen=True)
class RegistryBarrier:
    state: str
    checksums: dict[str, str | None]
    errors: dict[str, str]
    mismatches: dict[str, str]
    index_checksums: dict[str, str | None]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _captured_command(repo_root: Path, command: list[str]) -> str:
    completed = run_command(
        command,
        cwd=repo_root,
        capture=True,
        quiet=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise CratesIoPublishError(
            f"command failed ({' '.join(command)}): {detail or completed.returncode}"
        )
    return (completed.stdout or "").strip()


def assert_release_source(repo_root: Path, source_sha: str, source_tree: str) -> None:
    if GIT_OBJECT.fullmatch(source_sha) is None:
        raise CratesIoPublishError(f"invalid source commit: {source_sha!r}")
    if GIT_OBJECT.fullmatch(source_tree) is None:
        raise CratesIoPublishError(f"invalid source tree: {source_tree!r}")
    actual_sha = _captured_command(repo_root, ["git", "rev-parse", "HEAD^{commit}"])
    actual_tree = _captured_command(repo_root, ["git", "rev-parse", "HEAD^{tree}"])
    status = _captured_command(
        repo_root,
        ["git", "status", "--porcelain", "--untracked-files=normal"],
    )
    if (actual_sha, actual_tree) != (source_sha, source_tree):
        raise CratesIoPublishError(
            "publish checkout no longer matches the receipt source: "
            f"commit {actual_sha}, tree {actual_tree}"
        )
    if status:
        raise CratesIoPublishError(
            "crates.io publication requires an unchanged clean source tree"
        )


def _plan_digest(repo_root: Path, plan: PublishPlan) -> str:
    payload = {
        "batches": [list(batch) for batch in plan.batches],
        "packages": [
            {
                "name": package.name,
                "version": package.version,
                "manifest_path": package.manifest_path.resolve()
                .relative_to(repo_root)
                .as_posix(),
                "internal_dependencies": list(package.internal_deps),
            }
            for package in (plan.packages[name] for name in plan.order)
        ],
    }
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _package_receipt_entry(
    repo_root: Path,
    prepared: PreparedCrate,
    registry: dict | None = None,
) -> dict:
    repo_root = repo_root.resolve()
    package = prepared.package
    observed = registry or {}
    return {
        "name": package.name,
        "version": package.version,
        "manifest_path": package.manifest_path.resolve()
        .relative_to(repo_root)
        .as_posix(),
        "manifest_sha256": prepared.manifest_sha256,
        "internal_dependencies": list(package.internal_deps),
        "artifact": {
            "sha256": prepared.artifact_sha256,
            "size": prepared.artifact_size,
        },
        "registry": {
            "status": observed.get("status", "unobserved"),
            "observed_checksum": observed.get("checksum"),
            "publish_returncode": observed.get("returncode"),
            **({"error": observed["error"]} if observed.get("error") else {}),
            **(
                {"observed_index_checksum": observed["index_checksum"]}
                if "index_checksum" in observed else {}
            ),
        },
    }


def _batch_receipt(
    repo_root: Path,
    prepared: list[PreparedCrate],
    *,
    state: str,
    source_sha: str,
    source_tree: str,
    cargo_version: str,
    rustc_version: str,
    plan_sha256: str,
    batch_index: int,
    registry: dict[str, dict] | None = None,
    publication: dict | None = None,
    observe_only: bool = False,
) -> dict:
    registry = registry or {}
    return {
        "schema_version": 1,
        "observe_only": observe_only,
        **({"publication": publication} if publication is not None else {}),
        "schema": RECEIPT_SCHEMA.as_posix(),
        "channel": "crates.io",
        "kind": "topological-batch",
        "state": state,
        "source": {"commit": source_sha, "tree": source_tree},
        "toolchain": {"cargo": cargo_version, "rustc": rustc_version},
        "plan_sha256": plan_sha256,
        "batch_index": batch_index,
        "packages": [
            _package_receipt_entry(
                repo_root,
                item,
                registry.get(item.package.name),
            )
            for item in prepared
        ],
    }


def validate_crates_io_receipt(receipt: dict) -> None:
    if (
        receipt.get("schema_version") != 1
        or receipt.get("schema") != RECEIPT_SCHEMA.as_posix()
        or receipt.get("channel") != "crates.io"
        or receipt.get("kind") != "topological-batch"
        or receipt.get("state") not in RECEIPT_STATES
        or SHA256.fullmatch(str(receipt.get("plan_sha256", ""))) is None
        or not isinstance(receipt.get("batch_index"), int)
        or receipt["batch_index"] < 0
    ):
        raise CratesIoPublishError("invalid crates.io receipt envelope")
    if "observe_only" in receipt and type(receipt["observe_only"]) is not bool:
        raise CratesIoPublishError("crates.io receipt observe_only must be a boolean")
    publication = receipt.get("publication")
    if publication is not None and (
        not isinstance(publication, dict)
        or not isinstance(publication.get("run_id"), str)
        or not publication["run_id"].isdigit()
        or type(publication.get("attempt")) is not int
        or publication["attempt"] < 1
    ):
        raise CratesIoPublishError("invalid crates.io receipt publication attempt")
    source = receipt.get("source")
    if not isinstance(source, dict) or any(
        GIT_OBJECT.fullmatch(str(source.get(key, ""))) is None
        for key in ("commit", "tree")
    ):
        raise CratesIoPublishError("invalid crates.io receipt source")
    toolchain = receipt.get("toolchain")
    if not isinstance(toolchain, dict) or not all(
        isinstance(toolchain.get(key), str) and toolchain[key]
        for key in ("cargo", "rustc")
    ):
        raise CratesIoPublishError("invalid crates.io receipt toolchain")
    packages = receipt.get("packages")
    if not isinstance(packages, list) or not packages:
        raise CratesIoPublishError("crates.io receipt packages must be non-empty")
    names: set[str] = set()
    for package in packages:
        name = package.get("name") if isinstance(package, dict) else None
        artifact = package.get("artifact") if isinstance(package, dict) else None
        if (
            not isinstance(name, str)
            or not name
            or name in names
            or not isinstance(artifact, dict)
            or SHA256.fullmatch(str(artifact.get("sha256", ""))) is None
            or not isinstance(artifact.get("size"), int)
            or artifact["size"] <= 0
        ):
            raise CratesIoPublishError("invalid crates.io receipt package identity")
        observed = package.get("registry")
        if (
            not isinstance(observed, dict)
            or not isinstance(observed.get("status"), str)
            or not observed["status"]
            or any(
                observed.get(key) is not None
                and SHA256.fullmatch(str(observed[key])) is None
                for key in ("observed_checksum", "observed_index_checksum")
            )
            or (
                observed.get("publish_returncode") is not None
                and type(observed["publish_returncode"]) is not int
            )
        ):
            raise CratesIoPublishError("invalid crates.io receipt registry observation")
        names.add(name)


def _receipt_identity(receipt: dict) -> dict:
    validate_crates_io_receipt(receipt)
    return {
        key: receipt[key]
        for key in (
            "schema_version",
            "schema",
            "channel",
            "kind",
            "source",
            "toolchain",
            "plan_sha256",
            "batch_index",
        )
    } | {
        "packages": [
            {key: value for key, value in package.items() if key != "registry"}
            for package in receipt["packages"]
        ]
    }


def _load_recovery_receipts(
    recovery_dir: Path | None,
) -> dict[int, list[tuple[Path, dict]]]:
    if recovery_dir is None:
        return {}
    recovery_dir = recovery_dir.resolve()
    if not recovery_dir.is_dir():
        raise CratesIoPublishError(
            f"recovery receipt directory does not exist: {recovery_dir}"
        )
    grouped: dict[int, list[tuple[Path, dict]]] = {}
    for path in sorted(recovery_dir.rglob("batch-*.json")):
        try:
            receipt = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise CratesIoPublishError(f"cannot read recovery receipt {path}: {error}") from error
        validate_crates_io_receipt(receipt)
        grouped.setdefault(receipt["batch_index"], []).append((path, receipt))
    if not grouped:
        raise CratesIoPublishError(
            f"recovery directory contains no crates.io batch receipts: {recovery_dir}"
        )
    if set(grouped) != set(range(max(grouped) + 1)):
        raise CratesIoPublishError("recovery receipts must cover a contiguous batch prefix")
    for batch_index, receipts in grouped.items():
        if not any(receipt["state"] == "prepared" for _path, receipt in receipts):
            raise CratesIoPublishError(
                f"recovery batch {batch_index} has result evidence without a prepared receipt"
            )
    return grouped


def _require_recovery_identity(
    current: dict,
    prior: list[tuple[Path, dict]],
) -> None:
    current_identity = _receipt_identity(current)
    for path, receipt in prior:
        if _receipt_identity(receipt) != current_identity:
            raise CratesIoPublishError(
                f"recovery receipt identity differs from the current package bytes: {path}"
            )


def _write_receipt(path: Path, receipt: dict) -> None:
    validate_crates_io_receipt(receipt)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def _prepare_crate(
    repo_root: Path,
    target_directory: Path,
    package: PackageInfo,
) -> PreparedCrate:
    completed = run_command(
        ["cargo", "package", "-p", package.name, "--locked"],
        cwd=repo_root,
    )
    if completed.returncode != 0:
        raise CratesIoPublishError(
            f"cargo package failed for {package.name} {package.version}"
        )
    artifact = target_directory / "package" / f"{package.name}-{package.version}.crate"
    if artifact.is_symlink() or not artifact.is_file():
        raise CratesIoPublishError(f"cargo package did not create {artifact}")
    return PreparedCrate(
        package=package,
        artifact_path=artifact,
        artifact_sha256=sha256_file(artifact),
        artifact_size=artifact.stat().st_size,
        manifest_sha256=sha256_file(package.manifest_path),
    )


def _assert_artifact_unchanged(prepared: PreparedCrate) -> None:
    if (
        prepared.artifact_path.is_symlink()
        or not prepared.artifact_path.is_file()
        or prepared.artifact_path.stat().st_size != prepared.artifact_size
        or sha256_file(prepared.artifact_path) != prepared.artifact_sha256
    ):
        raise CratesIoPublishError(
            f"prepared artifact changed after receipt: {prepared.artifact_path}"
        )


def fetch_crates_io_checksum(
    crate_name: str,
    version: str,
    *,
    api_url: str = CRATES_IO_API,
    timeout: int = 30,
) -> str | None:
    request = urllib.request.Request(
        f"{api_url.rstrip('/')}/crates/"
        f"{urllib.parse.quote(crate_name, safe='')}/"
        f"{urllib.parse.quote(version, safe='')}",
        headers={"User-Agent": "merman-release-operator/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise CratesIoPublishError(
            f"crates.io returned HTTP {error.code} for {crate_name} {version}"
        ) from error
    except (OSError, json.JSONDecodeError) as error:
        raise CratesIoPublishError(
            f"cannot observe crates.io checksum for {crate_name} {version}: {error}"
        ) from error
    version_record = payload.get("version") if isinstance(payload, dict) else None
    if isinstance(version_record, dict) and version_record.get("yanked") is True:
        raise RegistryConflictError(f"crates.io version is yanked: {crate_name} {version}")
    checksum = version_record.get("checksum") if isinstance(version_record, dict) else None
    if not isinstance(checksum, str) or SHA256.fullmatch(checksum) is None:
        raise CratesIoPublishError(
            f"crates.io returned an invalid checksum for {crate_name} {version}"
        )
    return checksum


def fetch_crates_io_index_checksum(
    crate_name: str,
    version: str,
    *,
    index_url: str = CRATES_IO_INDEX,
    timeout: int = 30,
) -> str | None:
    """Read the exact version from Cargo's sparse index, independently of the API."""

    name = crate_name.lower()
    if not re.fullmatch(r"[a-z0-9_-]+", name):
        raise CratesIoPublishError(f"invalid crate name: {crate_name!r}")
    if len(name) < 3:
        prefix = str(len(name))
    elif len(name) == 3:
        prefix = f"3/{name[0]}"
    else:
        prefix = f"{name[:2]}/{name[2:4]}"
    request = urllib.request.Request(
        f"{index_url.rstrip('/')}/{prefix}/{name}",
        headers={"User-Agent": "merman-release-operator/1", "Cache-Control": "no-cache"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            entries = [json.loads(line) for line in response if line.strip()]
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise CratesIoPublishError(
            f"Cargo sparse index returned HTTP {error.code} for {crate_name} {version}"
        ) from error
    except (OSError, ValueError) as error:
        raise CratesIoPublishError(
            f"cannot observe Cargo sparse index for {crate_name} {version}: {error}"
        ) from error
    matches = [
        entry for entry in entries
        if isinstance(entry, dict) and entry.get("vers") == version
    ]
    if not matches:
        return None
    if len(matches) != 1 or matches[0].get("name") != crate_name:
        raise CratesIoPublishError(f"invalid Cargo sparse index entry for {crate_name} {version}")
    entry = matches[0]
    checksum = entry.get("cksum")
    if not isinstance(checksum, str) or SHA256.fullmatch(checksum) is None:
        raise CratesIoPublishError(f"invalid Cargo sparse index checksum for {crate_name} {version}")
    if entry.get("yanked") is not False:
        raise RegistryConflictError(f"Cargo sparse index version is yanked or invalid: {crate_name} {version}")
    return checksum


def reconcile_registry_barrier(
    prepared: list[PreparedCrate],
    *,
    registry_api: str,
    attempts: int,
    delay_seconds: int,
    registry_index: str = CRATES_IO_INDEX,
) -> RegistryBarrier:
    return _observe_registry(
        {item.package.name: (item.package.version, item.artifact_sha256) for item in prepared},
        registry_api=registry_api, registry_index=registry_index,
        attempts=attempts, delay_seconds=delay_seconds,
    )


def _observe_registry(
    expected: dict[str, tuple[str, str]],
    *,
    registry_api: str,
    registry_index: str,
    attempts: int,
    delay_seconds: int,
    accept_absent_single: bool = False,
) -> RegistryBarrier:
    if attempts <= 0 or delay_seconds < 0:
        raise ValueError("registry reconciliation bounds must be non-negative")
    pending = expected.copy()
    checksums: dict[str, str | None] = {name: None for name in pending}
    index_checksums: dict[str, str | None] = {name: None for name in pending}
    errors: dict[str, str] = {}
    mismatches: dict[str, str] = {}
    deadline = time.monotonic() + REGISTRY_OBSERVATION_SECONDS
    for attempt in range(attempts):
        for name, (version, digest) in list(pending.items()):
            if time.monotonic() >= deadline:
                break
            try:
                checksum = fetch_crates_io_checksum(
                    name, version, api_url=registry_api,
                    timeout=max(1, min(30, deadline - time.monotonic())),
                )
                checksums[name] = checksum
                if checksum is not None and checksum != digest:
                    mismatches[name] = checksum
                    continue
                if time.monotonic() >= deadline:
                    break
                index_checksum = fetch_crates_io_index_checksum(
                    name, version, index_url=registry_index,
                    timeout=max(1, min(30, deadline - time.monotonic())),
                )
                index_checksums[name] = index_checksum
                if index_checksum is not None and index_checksum != digest:
                    mismatches[name] = index_checksum
                    continue
                errors.pop(name, None)
                if checksum is not None and index_checksum is not None:
                    pending.pop(name)
                elif checksum is not None:
                    errors[name] = "exact archive accepted; waiting for Cargo sparse index propagation"
                elif index_checksum is not None:
                    errors[name] = "exact Cargo sparse index entry visible; waiting for crates.io API propagation"
                elif accept_absent_single and len(expected) == 1:
                    return RegistryBarrier("missing", checksums, errors, mismatches, index_checksums)
                else:
                    errors[name] = "version not yet visible through the crates.io API or Cargo sparse index"
            except RegistryConflictError as error:
                mismatches[name] = str(error)
                errors[name] = str(error)
            except CratesIoPublishError as error:
                errors[name] = str(error)
        if mismatches:
            return RegistryBarrier("mismatch", checksums, errors, mismatches, index_checksums)
        if not pending:
            return RegistryBarrier("complete", checksums, errors, mismatches, index_checksums)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        if attempt + 1 < attempts and delay_seconds:
            delay = min(delay_seconds * (2 ** attempt), 30, remaining)
            print(f"Registry observation {attempt + 1}/{attempts}: "
                  f"{errors}; retrying in {delay:g}s", flush=True)
            time.sleep(delay)
    return RegistryBarrier("pending_recovery", checksums, errors, mismatches, index_checksums)


def _update_registry_from_barrier(
    registry: dict[str, dict],
    prepared: list[PreparedCrate],
    barrier: RegistryBarrier,
) -> None:
    for item in prepared:
        name = item.package.name
        checksum = barrier.checksums[name]
        entry = registry.setdefault(name, {})
        entry["checksum"] = checksum
        entry["index_checksum"] = barrier.index_checksums[name]
        if name in barrier.mismatches:
            entry["status"] = "checksum_mismatch"
            entry["error"] = barrier.errors.get(
                name, "crates.io API or Cargo sparse index checksum differs from the prepared artifact"
            )
        elif checksum == item.artifact_sha256 and barrier.index_checksums[name] == item.artifact_sha256:
            entry.pop("error", None)
            if entry.get("status") not in {
                "already_published",
                "published_after_response_loss",
            }:
                entry["status"] = "published"
        else:
            entry["status"] = (
                "accepted_pending_index" if checksum == item.artifact_sha256
                else "publish_response_lost" if entry.get("status") in {
                    "publish_started", "publish_returned", "publish_response_lost",
                    "already_published", "published", "published_after_response_loss",
                    "accepted_pending_index",
                } else "pending_recovery"
            )
            if error := barrier.errors.get(name):
                entry["error"] = error


def publish_receipted_release(
    repo_root: Path,
    metadata: dict,
    *,
    source_sha: str,
    source_tree: str,
    receipts_dir: Path,
    registry_token: str | None = None,
    token_provider: Callable[[], AbstractContextManager[str]] | None = None,
    recovery_receipts_dir: Path | None = None,
    registry_api: str = CRATES_IO_API,
    registry_index: str = CRATES_IO_INDEX,
    visibility_attempts: int = 12,
    visibility_delay: int = 15,
    observe_only: bool = False,
    publication_run_id: str | None = None,
    publication_attempt: int | None = None,
    recovery_run_id: str | None = None,
    recovery_attempt: int | None = None,
) -> None:
    repo_root = repo_root.resolve()
    if visibility_attempts < 1 or visibility_delay < 0:
        raise CratesIoPublishError("registry observation requires positive attempts and a non-negative delay")
    if not registry_token and token_provider is None:
        raise CratesIoPublishError("crates.io publication requires a registry token")
    if registry_token and token_provider is not None:
        raise CratesIoPublishError("choose either a registry token or Trusted Publishing")
    credential_keys = {
        "CARGO_REGISTRY_TOKEN",
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
    }
    if credential_keys.intersection(os.environ):
        raise CratesIoPublishError(
            "registry credentials must be passed only to the cargo publish subprocess"
        )
    target_directory_raw = metadata.get("target_directory")
    if not isinstance(target_directory_raw, str):
        raise CratesIoPublishError("cargo metadata is missing target_directory")
    target_directory = Path(target_directory_raw).resolve()
    receipts_dir = receipts_dir.resolve()
    plan = crates_io_publish_plan(metadata)
    recovery = _load_recovery_receipts(recovery_receipts_dir)
    if (publication_run_id is None) != (publication_attempt is None):
        raise CratesIoPublishError("publication run and attempt must be supplied together")
    if (recovery_run_id is None) != (recovery_attempt is None):
        raise CratesIoPublishError("recovery run and attempt must be supplied together")
    publication = (
        {"run_id": publication_run_id, "attempt": publication_attempt}
        if publication_run_id is not None else None
    )
    if recovery and recovery_run_id is None:
        observe_only = True
    if recovery_run_id is not None:
        expected = {"run_id": recovery_run_id, "attempt": recovery_attempt}
        if not recovery or any(
            prior_receipt.get("publication") != expected
            for receipts in recovery.values() for _path, prior_receipt in receipts
        ):
            observe_only = True
    # Every batch carries the run-wide uncertainty, including prepared-only receipts.
    # A completed prefix cannot prove that an absent later batch was never uploaded.
    if any(
        prior_receipt.get("observe_only", True)
        for receipts in recovery.values() for _path, prior_receipt in receipts
    ):
        observe_only = True
    unknown_batches = sorted(set(recovery) - set(range(len(plan.batches))))
    if unknown_batches:
        raise CratesIoPublishError(
            "recovery receipts reference unknown publish batches: "
            + ", ".join(map(str, unknown_batches))
        )
    plan_sha256 = _plan_digest(repo_root, plan)
    cargo_version = _captured_command(repo_root, ["cargo", "-Vv"])
    rustc_version = _captured_command(repo_root, ["rustc", "-Vv"])
    assert_release_source(repo_root, source_sha, source_tree)

    for batch_index, batch in enumerate(plan.batches):
        print(f"\n== Preparing crates.io batch {batch_index + 1} ==\n")
        prepared = [
            _prepare_crate(
                repo_root,
                target_directory,
                plan.packages[name],
            )
            for name in batch
        ]
        assert_release_source(repo_root, source_sha, source_tree)
        registry: dict[str, dict] = {}

        def receipt(state: str) -> dict:
            return _batch_receipt(
                repo_root,
                prepared,
                state=state,
                source_sha=source_sha,
                source_tree=source_tree,
                cargo_version=cargo_version,
                rustc_version=rustc_version,
                plan_sha256=plan_sha256,
                batch_index=batch_index,
                registry=registry,
                publication=publication,
                observe_only=observe_only,
            )

        def record(state: str, suffix: str = "result") -> None:
            _write_receipt(
                receipts_dir / f"batch-{batch_index:03d}-{suffix}.json",
                receipt(state),
            )

        prepared_receipt = receipt("prepared")
        if prior := recovery.get(batch_index):
            _require_recovery_identity(prepared_receipt, prior)
        _write_receipt(
            receipts_dir / f"batch-{batch_index:03d}-prepared.json",
            prepared_receipt,
        )

        # Any prior attempted upload is observation-only, even if a replica returns 404.
        attempted = {
            package["name"]
            for _path, prior_receipt in recovery.get(batch_index, [])
            for package in prior_receipt["packages"]
            if (
                package["registry"].get("publish_returncode") is not None
                and package["registry"]["status"] != "dry_run_failed"
            )
            or package["registry"]["status"] in {
                "publish_started", "publish_returned", "publish_response_lost",
                "published", "published_after_response_loss", "already_published",
                "accepted_pending_index",
            }
        }
        if observe_only:
            attempted.update(item.package.name for item in prepared)
        registry.update({name: {"status": "publish_started"} for name in attempted})
        missing: list[PreparedCrate] = []
        for item in prepared:
            name = item.package.name
            try:
                checksum = fetch_crates_io_checksum(
                    name,
                    item.package.version,
                    api_url=registry_api,
                )
            except CratesIoPublishError as error:
                registry.setdefault(name, {"status": "observation_failed"})["error"] = str(error)
                record("pending_recovery")
                raise
            if checksum is None:
                registry[name] = {"status": "publish_started" if name in attempted else "missing"}
                if name not in attempted:
                    missing.append(item)
            elif checksum == item.artifact_sha256:
                registry[name] = {
                    "status": "already_published",
                    "checksum": checksum,
                }
            else:
                registry[name] = {
                    "status": "checksum_mismatch",
                    "checksum": checksum,
                }
                record("mismatch")
                raise CratesIoPublishError(
                    f"registry checksum mismatch for {name} {item.package.version}: "
                    f"local {item.artifact_sha256}, registry {checksum}"
                )

        recovering = [item for item in prepared if item.package.name in attempted]
        if recovering:
            barrier = reconcile_registry_barrier(
                recovering, registry_api=registry_api, registry_index=registry_index,
                attempts=visibility_attempts, delay_seconds=visibility_delay,
            )
            _update_registry_from_barrier(registry, recovering, barrier)
            record(barrier.state if barrier.state != "complete" else "pending_recovery")
            if barrier.state != "complete":
                raise CratesIoPublishError(
                    f"previously attempted uploads remain {barrier.state}; do not republish; "
                    "resume with these receipts after registry propagation"
                )

        dry_run_environment = dict(os.environ)
        dry_run_environment.pop("CARGO_REGISTRY_TOKEN", None)
        for item in missing:
            name = item.package.name
            print(f"Dry-running {name} {item.package.version}")
            completed = run_command(
                [
                    "cargo",
                    "publish",
                    "-p",
                    name,
                    "--locked",
                    "--dry-run",
                    "--registry",
                    "crates-io",
                ],
                cwd=repo_root,
                env=dry_run_environment,
            )
            assert_release_source(repo_root, source_sha, source_tree)
            _assert_artifact_unchanged(item)
            if completed.returncode != 0:
                registry[name] = {
                    "status": "dry_run_failed",
                    "returncode": completed.returncode,
                }
                record("preflight_failed")
                raise CratesIoPublishError(f"cargo publish dry-run failed for {name}")

        for item in prepared:
            _assert_artifact_unchanged(item)
        for item in missing:
            name = item.package.name
            print(f"Publishing {name} {item.package.version}")
            credential = token_provider() if token_provider is not None else nullcontext(registry_token)
            with credential as upload_token:
                publish_environment = dict(os.environ)
                publish_environment["CARGO_REGISTRY_TOKEN"] = upload_token
                registry[name] = {"status": "publish_started"}
                record("pending_recovery")
                completed = run_command(
                    [
                        "cargo",
                        "publish",
                        "-p",
                        name,
                        "--locked",
                        "--no-verify",
                        "--registry",
                        "crates-io",
                    ],
                    cwd=repo_root,
                    env=publish_environment,
                )
            assert_release_source(repo_root, source_sha, source_tree)
            _assert_artifact_unchanged(item)
            registry[name] = {
                "status": "publish_returned"
                if completed.returncode == 0
                else "publish_response_lost",
                "returncode": completed.returncode,
            }
            record("pending_recovery")
            if completed.returncode == 0:
                continue
            response_loss = reconcile_registry_barrier(
                [item],
                registry_api=registry_api,
                registry_index=registry_index,
                attempts=visibility_attempts,
                delay_seconds=visibility_delay,
            )
            _update_registry_from_barrier(registry, [item], response_loss)
            if response_loss.state == "complete":
                registry[name]["status"] = "published_after_response_loss"
                continue
            record(response_loss.state)
            raise CratesIoPublishError(
                f"publish response for {name} was nonzero and registry reconciliation "
                f"ended in {response_loss.state}"
            )

        barrier = reconcile_registry_barrier(
            prepared,
            registry_api=registry_api,
            registry_index=registry_index,
            attempts=visibility_attempts,
            delay_seconds=visibility_delay,
        )
        _update_registry_from_barrier(registry, prepared, barrier)
        record(barrier.state)
        if barrier.state != "complete":
            raise CratesIoPublishError(
                f"crates.io batch {batch_index + 1} API/index barrier ended in {barrier.state}: "
                f"{barrier.errors or barrier.mismatches}; preserve receipts and resume after propagation; "
                "do not republish an attempted version"
            )
        print(f"crates.io batch {batch_index + 1} API/index checksum barrier complete")


def preflight_initial_batch(
    repo_root: Path,
    metadata: dict,
    *,
    registry_api: str = CRATES_IO_API,
) -> None:
    plan = crates_io_publish_plan(metadata)
    target_directory_raw = metadata.get("target_directory")
    if not isinstance(target_directory_raw, str) or not target_directory_raw:
        raise CratesIoPublishError("cargo metadata is missing target_directory")
    target_directory = Path(target_directory_raw).resolve()
    environment = dict(os.environ)
    environment.pop("CARGO_REGISTRY_TOKEN", None)
    environment.pop("CARGO_REGISTRIES_CRATES_IO_TOKEN", None)
    for name in plan.batches[0]:
        package = plan.packages[name]
        registry_checksum = fetch_crates_io_checksum(
            name,
            package.version,
            api_url=registry_api,
        )
        if registry_checksum is not None:
            prepared = _prepare_crate(repo_root, target_directory, package)
            if registry_checksum != prepared.artifact_sha256:
                raise CratesIoPublishError(
                    f"registry checksum mismatch for {name} {package.version}: "
                    f"local {prepared.artifact_sha256}, registry {registry_checksum}"
                )
            print(
                f"Skipping {name} {package.version}; the exact version already exists"
            )
            continue
        completed = run_command(
            [
                "cargo",
                "publish",
                "-p",
                name,
                "--locked",
                "--dry-run",
                "--registry",
                "crates-io",
            ],
            cwd=repo_root,
            env=environment,
        )
        if completed.returncode != 0:
            raise CratesIoPublishError(
                f"registry-independent publish dry-run failed for {name} {package.version}"
            )


def _workspace_package(metadata: dict, package_name: str) -> PackageInfo:
    raw = _workspace_packages_by_name(metadata).get(package_name)
    if raw is None:
        raise CratesIoPublishError(f"unknown workspace package: {package_name}")
    manifest_path = raw.get("manifest_path")
    version = raw.get("version")
    if not isinstance(manifest_path, str) or not isinstance(version, str):
        raise CratesIoPublishError(
            f"workspace package metadata is incomplete for {package_name}"
        )
    return PackageInfo(
        name=package_name,
        version=version,
        manifest_path=Path(manifest_path),
        internal_deps=(),
    )


def independent_version_exists(
    name: str, version: str, digest: str, *, registry_api: str, registry_index: str,
    require_published: bool = False, attempts: int = 1, delay_seconds: int = 0,
) -> bool:
    observation = _observe_registry(
        {name: (version, digest)}, registry_api=registry_api, registry_index=registry_index,
        attempts=attempts if require_published else 1, delay_seconds=delay_seconds,
        accept_absent_single=not require_published,
    )
    if observation.state == "missing":
        return False
    if observation.state != "complete":
        detail = observation.errors or observation.mismatches
        raise CratesIoPublishError(
            f"cannot confirm registry state for {name} {version} against {digest}: {detail}"
        )
    return True


def preflight_independent_crate(
    repo_root: Path,
    metadata: dict,
    *,
    package_name: str,
    expected_version: str,
    registry_api: str = CRATES_IO_API,
    registry_index: str = CRATES_IO_INDEX,
    verify_fork_consumer: bool = False,
    stage_candidate: Callable[[PreparedCrate], None] | None = None,
    require_published: bool = False,
) -> tuple[bool, PreparedCrate]:
    """Return the publication state and the verified archive.

    Existing registry versions are accepted only when the locally packaged
    archive has the exact registry checksum.  Missing versions must pass a
    credential-free publish dry-run.  The package is intentionally resolved
    outside the coupled publish graph. Later renderer fork releases can also
    compile the prepared archive with published siblings. Capture the candidate
    bytes before observing the registry or running Cargo's dry-run.
    """
    if verify_fork_consumer and package_name not in renderer_fork_consumer.FORK_REQUIREMENTS:
        raise CratesIoPublishError("the renderer fork consumer requires a renderer fork package")
    package = _workspace_package(metadata, package_name)
    if package.version != expected_version:
        raise CratesIoPublishError(
            f"{package_name} version {package.version} does not match requested "
            f"version {expected_version}"
        )
    target_directory_raw = metadata.get("target_directory")
    if not isinstance(target_directory_raw, str) or not target_directory_raw:
        raise CratesIoPublishError("cargo metadata is missing target_directory")
    target_directory = Path(target_directory_raw).resolve()
    prepared = _prepare_crate(repo_root, target_directory, package)
    verify_package_archive(
        _workspace_packages_by_name(metadata)[package_name], prepared.artifact_path, repo_root=repo_root,
    )
    if verify_fork_consumer:
        renderer_fork_consumer.verify(
            prepared.artifact_path, candidate=package.name, version=package.version,
            repo_root=repo_root, target_directory=target_directory,
        )
    if stage_candidate is not None:
        stage_candidate(prepared)
    exists = independent_version_exists(
        package.name, package.version, prepared.artifact_sha256,
        registry_api=registry_api, registry_index=registry_index, require_published=require_published,
    )
    if exists:
        if stage_candidate is None:
            _assert_artifact_unchanged(prepared)
        print(
            f"Skipping {package.name} {package.version}; the exact version already exists"
        )
        return True, prepared

    environment = dict(os.environ)
    environment.pop("CARGO_REGISTRY_TOKEN", None)
    environment.pop("CARGO_REGISTRIES_CRATES_IO_TOKEN", None)
    completed = run_command(
        [
            "cargo",
            "publish",
            "-p",
            package.name,
            "--locked",
            "--dry-run",
            "--registry",
            "crates-io",
        ],
        cwd=repo_root,
        env=environment,
    )
    if completed.returncode != 0:
        raise CratesIoPublishError(
            f"independent publish dry-run failed for {package.name} {package.version}"
        )
    _assert_artifact_unchanged(prepared)
    print(f"{package.name} {package.version} is not yet published; dry-run passed")
    return False, prepared


def _independent_candidate_identity(args: argparse.Namespace) -> tuple[dict, dict | None]:
    if not GIT_OBJECT.fullmatch(args.source_sha or "") or not GIT_OBJECT.fullmatch(args.source_tree or ""):
        raise CratesIoPublishError("frozen independent releases require the exact source commit and tree")
    run, attempt = args.publication_run_id, args.publication_attempt
    if (run is None) != (attempt is None) or (
        run is not None and (not re.fullmatch(r"[1-9][0-9]*", run) or attempt < 1)
    ):
        raise CratesIoPublishError("frozen independent releases require a valid producing run and attempt")
    return {"commit": args.source_sha, "tree": args.source_tree}, (
        {"run_id": run, "attempt": attempt} if run is not None else None
    )


def _independent_outputs(**values: str) -> None:
    if path := os.environ.get("GITHUB_OUTPUT"):
        with Path(path).open("a", encoding="utf-8") as stream:
            for name, value in values.items():
                print(f"{name}={value}", file=stream)


def _require_current_independent_attempt(publication: dict | None) -> None:
    current = {
        "run_id": os.environ.get("GITHUB_RUN_ID"),
        "attempt": int(os.environ.get("GITHUB_RUN_ATTEMPT", "0")),
    }
    if os.environ.get("GITHUB_ACTIONS") != "true" or publication != current:
        raise CratesIoPublishError("independent upload must use the current GitHub run and attempt")


def _load_independent_candidate(args: argparse.Namespace) -> tuple[crates_io_archive.FrozenCrate, dict | None]:
    source, publication = _independent_candidate_identity(args)
    return (
        crates_io_archive.load_candidate(
            args.candidate_dir, name=args.package, version=args.version, source=source, publication=publication,
            receipt_sha256=args.receipt_sha256, archive_sha256=args.sha256,
        ),
        publication,
    )


def _independent_command(
    args: argparse.Namespace, repo_root: Path, metadata: dict | None, token: str | None,
    *, gh_env: dict[str, str] | None,
) -> None:
    name, version = args.package, args.version
    if name != "roughr-merman" and name not in renderer_fork_consumer.FORK_REQUIREMENTS:
        raise CratesIoPublishError(f"unknown independent package: {name!r}")
    parse_release_version(version, allow_v_prefix=False)

    registry = {"registry_api": args.registry_api, "registry_index": args.registry_index}
    repository = args.publication_repository
    if args.command == "independent-history" or (
        args.publication_run_id is not None and args.command in {"preflight-independent", "publish-independent"}
    ):
        repository = os.environ["GITHUB_REPOSITORY"]
    if repository is None and args.candidate_dir is not None:
        raise CratesIoPublishError("manual independent candidates require --publication-repository")
    origin = (release_attempt_history.read_independent_receipt(repository, name, version, gh_env=gh_env)
              if repository is not None else None)
    legacy = version in release_attempt_history.LEGACY_INDEPENDENT_VERSIONS.get(name, ())
    if args.command == "independent-history":
        if legacy and origin is None:
            raise CratesIoPublishError(f"{name} {version} predates shared receipts; reconcile it manually")
        _independent_outputs(sha256=origin.sha256 if origin else "", source_sha=origin.source_sha if origin else "")
        return

    def check_digest(digest: str) -> None:
        if origin is not None and origin.sha256 != digest:
            raise CratesIoPublishError("independent release receipts belong to a different archive")

    def record_shared(digest: str, state: str) -> None:
        nonlocal origin
        if repository is None or (origin is not None and origin.state == state):
            return
        source = origin.source_sha if origin is not None else args.source_sha
        if not GIT_OBJECT.fullmatch(source or ""):
            raise CratesIoPublishError("a shared independent crate receipt requires its original --source-sha")
        receipt = release_attempt_history.IndependentCrateReceipt(digest, source, state)
        release_attempt_history.record_independent_receipt(repository, name, version, receipt, gh_env=gh_env)
        origin = receipt

    def confirm(digest: str, candidate: crates_io_archive.FrozenCrate | None) -> None:
        try:
            record_shared(digest, "registry-confirmed")
        finally:
            if candidate is not None:
                crates_io_archive.retain_receipt(candidate, args.candidate_dir / "registry-confirmed.json")

    def replay_confirmation(digest: str, candidate: crates_io_archive.FrozenCrate | None) -> None:
        if origin is not None and origin.state == "registry-confirmed":
            return
        if candidate is not None and crates_io_archive.has_receipt(
            candidate, args.candidate_dir / "registry-confirmed.json",
        ):
            record_shared(digest, "registry-confirmed")
            print(f"{name} {version}: restored shared registry confirmation (SHA-256 {digest})")

    outputs = {}
    if args.command == "preflight-independent":
        publication = None
        candidate = None
        if args.candidate_dir is not None:
            source, publication = _independent_candidate_identity(args)
            assert_release_source(repo_root, args.source_sha, args.source_tree)
            if publication is not None:
                _require_current_independent_attempt(publication)

        def stage_candidate(prepared: PreparedCrate) -> None:
            nonlocal candidate
            check_digest(prepared.artifact_sha256)
            payload = crates_io_archive.publish_metadata(repo_root, prepared.artifact_path, name, version)
            candidate = crates_io_archive.freeze_candidate(
                prepared.artifact_path, payload, archive_sha256=prepared.artifact_sha256,
                name=name, version=version, source=source, publication=publication,
            )

        exists, prepared = preflight_independent_crate(
            repo_root, metadata, package_name=name, expected_version=version,
            verify_fork_consumer=args.verify_fork_consumer,
            stage_candidate=stage_candidate if args.candidate_dir is not None else None,
            require_published=origin is not None or legacy, **registry,
        )
        digest = prepared.artifact_sha256
        if candidate is not None:
            assert_release_source(repo_root, args.source_sha, args.source_tree)
            try:
                if exists:
                    record_shared(digest, "registry-confirmed")
            finally:
                crates_io_archive.retain_candidate(
                    args.candidate_dir, candidate, name=name, version=version, registry_confirmed=exists,
                )
            outputs["receipt_sha256"] = candidate.receipt_sha256
            print(f"Frozen receipt SHA-256: {candidate.receipt_sha256}")
    elif args.command == "registry-independent":
        candidate = None
        if args.candidate_dir is not None:
            candidate, _ = _load_independent_candidate(args)
        digest = args.sha256
        check_digest(digest)
        replay_confirmation(digest, candidate)
        exists = independent_version_exists(
            name, version, digest, require_published=args.observe_only,
            attempts=args.visibility_attempts, delay_seconds=args.visibility_delay, **registry,
        )
        if exists:
            confirm(digest, candidate)
        if args.verify_fork_consumer:
            renderer_fork_consumer.verify(
                None, candidate=name, version=version, repo_root=repo_root,
                target_directory=repo_root / os.environ.get("CARGO_TARGET_DIR", "target"),
            )
    elif args.command == "publish-independent":
        candidate, publication = _load_independent_candidate(args)
        digest = candidate.archive_sha256
        if publication is not None or os.environ.get("GITHUB_ACTIONS") == "true":
            _require_current_independent_attempt(publication)
            assert_release_source(repo_root, args.source_sha, args.source_tree)
        check_digest(digest)
        replay_confirmation(digest, candidate)
        if independent_version_exists(name, version, digest, **registry):
            confirm(digest, candidate)
            print(f"{name} {version}: published (SHA-256 {digest})")
            return
        if legacy or origin is not None or crates_io_archive.has_receipt(
            candidate, args.candidate_dir / "upload-attempted.json",
        ):
            raise CratesIoPublishError("this candidate has publication evidence; observe registry propagation")
        if not token:
            raise CratesIoPublishError("independent publication requires CARGO_REGISTRY_TOKEN")
        crates_io_archive.upload(
            candidate, token=token, retain_intent=lambda: record_shared(digest, "upload-intent"),
        )
        return
    else:
        raise AssertionError(f"Unknown independent command {args.command}")

    print(f"{name} {version}: {'published' if exists else 'missing'} (SHA-256 {digest})")
    _independent_outputs(sha256=digest, publish=str(not exists).lower(), **outputs)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=["preflight-initial", "preflight-independent", "registry-independent", "publish-receipted",
                 "independent-history", "publish-independent"],
    )
    parser.add_argument("--package")
    parser.add_argument("--version")
    parser.add_argument("--sha256", help="preflight archive checksum")
    parser.add_argument("--candidate-dir", type=Path, help="directory holding the frozen independent candidate")
    parser.add_argument("--receipt-sha256", help="preflight receipt checksum")
    parser.add_argument(
        "--verify-fork-consumer", action="store_true",
        help="compile the candidate or the confirmed registry fork with its published siblings",
    )
    parser.add_argument("--source-sha")
    parser.add_argument("--source-tree")
    parser.add_argument("--receipts-dir", type=Path)
    parser.add_argument("--recovery-receipts-dir", type=Path)
    parser.add_argument("--observe-only", action="store_true")
    parser.add_argument("--trusted-publishing", action="store_true")
    parser.add_argument("--publication-run-id")
    parser.add_argument("--publication-attempt", type=int)
    parser.add_argument("--publication-repository", help="share manual independent crate receipts with this GitHub repository")
    parser.add_argument("--recovery-run-id")
    parser.add_argument("--recovery-attempt", type=int)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
    )
    parser.add_argument("--registry-api", default=CRATES_IO_API)
    parser.add_argument("--registry-index", default=CRATES_IO_INDEX)
    parser.add_argument("--visibility-attempts", type=int, default=12)
    parser.add_argument("--visibility-delay", type=int, default=15)
    args = parser.parse_args()
    if args.observe_only and args.command not in {"registry-independent", "publish-receipted"}:
        parser.error("--observe-only requires registry-independent or publish-receipted")
    if args.verify_fork_consumer and not (
        args.command == "preflight-independent" or
        (args.command == "registry-independent" and args.observe_only)
    ):
        parser.error("--verify-fork-consumer requires preflight-independent or registry-independent --observe-only")

    independent = args.command not in {"preflight-initial", "publish-receipted"}
    if independent and (not args.package or not args.version):
        parser.error(f"{args.command} requires --package and --version")
    if args.command == "registry-independent" and not SHA256.fullmatch(args.sha256 or ""):
        parser.error("registry-independent requires the preflight --sha256")
    if args.command == "publish-independent":
        if args.candidate_dir is None:
            parser.error("publish-independent requires --candidate-dir")
        if not SHA256.fullmatch(args.sha256 or "") or not SHA256.fullmatch(args.receipt_sha256 or ""):
            parser.error(f"{args.command} requires the preflight --sha256 and --receipt-sha256")
        if (args.registry_api, args.registry_index) != (CRATES_IO_API, CRATES_IO_INDEX):
            parser.error(f"{args.command} requires the crates.io API and sparse index")
    if args.publication_repository is not None:
        if (args.publication_run_id is not None
                or (args.registry_api, args.registry_index) != (CRATES_IO_API, CRATES_IO_INDEX)
                or not ((args.command in {"preflight-independent", "publish-independent"} and args.candidate_dir is not None)
                        or (args.command == "registry-independent" and args.observe_only))):
            parser.error("--publication-repository requires a manual candidate or registry observation on crates.io")
    repo_root = args.repo_root.resolve()
    registry_token = os.environ.pop("CARGO_REGISTRY_TOKEN", None)
    os.environ.pop("CARGO_REGISTRIES_CRATES_IO_TOKEN", None)
    oidc_request_url = os.environ.pop("ACTIONS_ID_TOKEN_REQUEST_URL", None)
    oidc_request_token = os.environ.pop("ACTIONS_ID_TOKEN_REQUEST_TOKEN", None)
    gh_env = None
    if independent:
        gh_credentials = {
            name: value for name in ("GH_TOKEN", "GITHUB_TOKEN")
            if (value := os.environ.pop(name, None)) is not None
        }
        if gh_credentials:
            gh_env = dict(os.environ) | gh_credentials
    metadata = None
    try:
        if args.command == "publish-receipted" or (
            (args.command == "preflight-independent" and args.candidate_dir is not None)
            or (args.command == "publish-independent" and args.publication_run_id is not None)
        ):
            require_tool("git")
        if args.command in {"preflight-initial", "preflight-independent", "publish-receipted"}:
            require_tool("cargo")
            metadata = cargo_metadata(repo_root, quiet=True)
    except (OSError, RuntimeError, ValueError) as error:
        print_error(str(error))
        return 2

    if args.command == "preflight-initial":
        try:
            preflight_initial_batch(
                repo_root,
                metadata,
                registry_api=args.registry_api,
            )
        except (CratesIoPublishError, OSError, ValueError) as error:
            print_error(str(error))
            return 1
        return 0

    if independent:
        lock_candidate = (args.command in {"publish-independent", "registry-independent"}
                          and args.candidate_dir is not None and args.publication_repository is not None)
        try:
            with crates_io_archive.candidate_lock(args.candidate_dir) if lock_candidate else nullcontext():
                _independent_command(args, repo_root, metadata, registry_token, gh_env=gh_env)
        except (RuntimeError, OSError, ValueError, KeyError, tarfile.TarError) as error:
            print_error(str(error))
            return 1
        return 0

    if not args.source_sha or not args.source_tree or args.receipts_dir is None:
        parser.error(
            "publish-receipted requires --source-sha, --source-tree, and --receipts-dir"
        )
    if not registry_token and not args.trusted_publishing:
        print_error("crates.io publication requires --trusted-publishing or CARGO_REGISTRY_TOKEN")
        return 2
    try:
        publish_receipted_release(
            repo_root,
            metadata,
            source_sha=args.source_sha,
            source_tree=args.source_tree,
            receipts_dir=args.receipts_dir,
            registry_token=registry_token,
            token_provider=(
                lambda: trusted_publishing_token(
                    request_url=oidc_request_url, request_token=oidc_request_token,
                )
            ) if args.trusted_publishing else None,
            recovery_receipts_dir=args.recovery_receipts_dir,
            registry_api=args.registry_api,
            registry_index=args.registry_index,
            visibility_attempts=args.visibility_attempts,
            visibility_delay=args.visibility_delay,
            observe_only=args.observe_only,
            publication_run_id=args.publication_run_id, publication_attempt=args.publication_attempt,
            recovery_run_id=args.recovery_run_id, recovery_attempt=args.recovery_attempt,
        )
    except (RuntimeError, OSError, ValueError) as error:
        print_error(str(error))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
