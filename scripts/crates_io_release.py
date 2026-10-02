#!/usr/bin/env python3
"""Prepare, publish, and reconcile checksum-bound crates.io batches."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

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
    if attempts <= 0 or delay_seconds < 0:
        raise ValueError("registry reconciliation bounds must be non-negative")
    pending = {item.package.name: item for item in prepared}
    checksums: dict[str, str | None] = {name: None for name in pending}
    index_checksums: dict[str, str | None] = {name: None for name in pending}
    errors: dict[str, str] = {}
    mismatches: dict[str, str] = {}
    deadline = time.monotonic() + REGISTRY_OBSERVATION_SECONDS
    for attempt in range(attempts):
        for name, item in list(pending.items()):
            if time.monotonic() >= deadline:
                break
            try:
                checksum = fetch_crates_io_checksum(
                    name, item.package.version, api_url=registry_api,
                    timeout=max(1, min(30, deadline - time.monotonic())),
                )
                checksums[name] = checksum
                if checksum is not None and checksum != item.artifact_sha256:
                    mismatches[name] = checksum
                    continue
                if time.monotonic() >= deadline:
                    break
                index_checksum = fetch_crates_io_index_checksum(
                    name, item.package.version, index_url=registry_index,
                    timeout=max(1, min(30, deadline - time.monotonic())),
                )
                index_checksums[name] = index_checksum
                if index_checksum is not None and index_checksum != item.artifact_sha256:
                    mismatches[name] = index_checksum
                    continue
                errors.pop(name, None)
                if checksum is not None and index_checksum is not None:
                    pending.pop(name)
                elif checksum is not None:
                    errors[name] = "exact archive accepted; waiting for Cargo sparse index propagation"
                elif index_checksum is not None:
                    errors[name] = "exact Cargo sparse index entry visible; waiting for crates.io API propagation"
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
    registry_token: str,
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
    if not registry_token:
        raise CratesIoPublishError("crates.io publication requires a registry token")
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
        publish_environment = dict(os.environ)
        publish_environment["CARGO_REGISTRY_TOKEN"] = registry_token
        for item in missing:
            name = item.package.name
            print(f"Publishing {name} {item.package.version}")
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
    plan = crates_io_publish_plan(metadata)
    package = plan.packages.get(package_name)
    if package is not None:
        return package
    workspace = _workspace_packages_by_name(metadata)
    raw = workspace.get(package_name)
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


def preflight_independent_crate(
    repo_root: Path,
    metadata: dict,
    *,
    package_name: str,
    expected_version: str,
    registry_api: str = CRATES_IO_API,
) -> bool:
    """Verify one independently owned crate and return whether it already exists.

    Existing registry versions are accepted only when the locally packaged
    archive has the exact registry checksum.  Missing versions must pass a
    credential-free publish dry-run.  The package is intentionally resolved
    outside the coupled publish graph.
    """
    package = _workspace_package(metadata, package_name)
    if package.version != expected_version:
        raise CratesIoPublishError(
            f"{package_name} version {package.version} does not match requested "
            f"version {expected_version}"
        )
    target_directory_raw = metadata.get("target_directory")
    if not isinstance(target_directory_raw, str) or not target_directory_raw:
        raise CratesIoPublishError("cargo metadata is missing target_directory")
    prepared = _prepare_crate(repo_root, Path(target_directory_raw).resolve(), package)
    registry_checksum = fetch_crates_io_checksum(
        package.name,
        package.version,
        api_url=registry_api,
    )
    if registry_checksum is not None:
        if registry_checksum != prepared.artifact_sha256:
            raise CratesIoPublishError(
                f"registry checksum mismatch for {package.name} {package.version}: "
                f"local {prepared.artifact_sha256}, registry {registry_checksum}"
            )
        print(
            f"Skipping {package.name} {package.version}; the exact version already exists"
        )
        return True

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
    print(f"{package.name} {package.version} is not yet published; dry-run passed")
    return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=["preflight-initial", "preflight-independent", "publish-receipted"],
    )
    parser.add_argument("--package")
    parser.add_argument("--version")
    parser.add_argument("--source-sha")
    parser.add_argument("--source-tree")
    parser.add_argument("--receipts-dir", type=Path)
    parser.add_argument("--recovery-receipts-dir", type=Path)
    parser.add_argument("--observe-only", action="store_true")
    parser.add_argument("--publication-run-id")
    parser.add_argument("--publication-attempt", type=int)
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

    repo_root = args.repo_root.resolve()
    registry_token = os.environ.pop("CARGO_REGISTRY_TOKEN", None)
    os.environ.pop("CARGO_REGISTRIES_CRATES_IO_TOKEN", None)
    try:
        require_tool("cargo")
        if args.command == "publish-receipted":
            require_tool("git")
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

    if args.command == "preflight-independent":
        if not args.package or not args.version:
            parser.error("preflight-independent requires --package and --version")
        try:
            preflight_independent_crate(
                repo_root,
                metadata,
                package_name=args.package,
                expected_version=args.version,
                registry_api=args.registry_api,
            )
        except (CratesIoPublishError, OSError, ValueError) as error:
            print_error(str(error))
            return 1
        return 0

    if not args.source_sha or not args.source_tree or args.receipts_dir is None:
        parser.error(
            "publish-receipted requires --source-sha, --source-tree, and --receipts-dir"
        )
    if not registry_token:
        print_error("CARGO_REGISTRY_TOKEN is required for crates.io publication")
        return 2
    try:
        publish_receipted_release(
            repo_root,
            metadata,
            source_sha=args.source_sha,
            source_tree=args.source_tree,
            receipts_dir=args.receipts_dir,
            registry_token=registry_token,
            recovery_receipts_dir=args.recovery_receipts_dir,
            registry_api=args.registry_api,
            registry_index=args.registry_index,
            visibility_attempts=args.visibility_attempts,
            visibility_delay=args.visibility_delay,
            observe_only=args.observe_only,
            publication_run_id=args.publication_run_id, publication_attempt=args.publication_attempt,
            recovery_run_id=args.recovery_run_id, recovery_attempt=args.recovery_attempt,
        )
    except (CratesIoPublishError, OSError, ValueError) as error:
        print_error(str(error))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
