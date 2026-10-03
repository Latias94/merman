#!/usr/bin/env python3
"""Publish verified lockstep package groups with npm Trusted Publisher."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any, Protocol


INTEGRITY_RE = re.compile(r"sha512-[A-Za-z0-9+/]+={0,2}\Z")
NPM_DIST_TAG_RE = re.compile(r"[a-z][a-z0-9-]*\Z")
NPMJS_REGISTRY_URL = "https://registry.npmjs.org"
# npm metadata and dist-tags can take several minutes to become globally visible after a publish.
DEFAULT_REGISTRY_OBSERVATION_ATTEMPTS = 12
DEFAULT_REGISTRY_OBSERVATION_DELAY_SECONDS = 5.0
REGISTRY_OBSERVATION_SECONDS = 300.0


def npm_executable(*, os_name: str | None = None) -> str:
    """Return the npm executable name accepted by subprocess on this platform."""

    return "npm.cmd" if (os_name or os.name) == "nt" else "npm"


class PackageGroupError(ValueError):
    """The package group artifact or registry state is invalid."""


class RegistryPendingError(PackageGroupError):
    """An attempted upload needs later observation, never an automatic re-upload."""


class ReconciliationError(PackageGroupError):
    """A registry mutation failed after reportable reconciliation state existed."""

    def __init__(self, message: str, report: dict[str, Any]) -> None:
        super().__init__(message)
        self.report = report


class NpmClient(Protocol):
    def version_integrity(self, package: str, version: str) -> str | None: ...

    def dist_tag(self, package: str, tag: str) -> str | None: ...

    def publish(self, tarball: Path, tag: str) -> None: ...


@dataclass
class NpmCli:
    registry: str | None = None

    def _command(self, *args: str) -> list[str]:
        command = [npm_executable(), *args]
        if self.registry:
            command.extend(["--registry", self.registry])
        return command

    def _run(self, *args: str) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            self._command(*args),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip() or "npm command failed"
            raise PackageGroupError(f"npm {' '.join(args)} failed: {diagnostic}")
        return result

    def _view(self, command: list[str]) -> subprocess.CompletedProcess[str]:
        command.extend(["--prefer-online", "--fetch-retries=0", "--fetch-timeout=20000"])
        try:
            return subprocess.run(command, check=False, capture_output=True, text=True, timeout=30)
        except subprocess.TimeoutExpired as exc:
            raise PackageGroupError("npm metadata lookup exceeded its 30-second bound") from exc

    def version_integrity(self, package: str, version: str) -> str | None:
        result = self._view(
            self._command("view", f"{package}@{version}", "dist.integrity", "--json"),
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if re.search(r"\bE404\b", diagnostic):
                return None
            raise PackageGroupError(
                f"npm view {package}@{version} failed: {diagnostic or 'unknown error'}"
            )
        try:
            value = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise PackageGroupError(
                f"npm view {package}@{version} returned invalid JSON"
            ) from exc
        if (
            not isinstance(value, list)
            or len(value) != 1
            or not isinstance(value[0], str)
            or not INTEGRITY_RE.fullmatch(value[0])
        ):
            raise PackageGroupError(
                f"npm view {package}@{version} returned invalid dist.integrity"
            )
        return value[0]

    def dist_tag(self, package: str, tag: str) -> str | None:
        result = self._view(
            self._command("view", package, "dist-tags", "--json"),
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if re.search(r"\bE404\b", diagnostic):
                return None
            raise PackageGroupError(
                f"npm view {package} dist-tags failed: {diagnostic or 'unknown error'}"
            )
        try:
            value = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise PackageGroupError(
                f"npm view {package} dist-tags returned invalid JSON"
            ) from exc
        if not isinstance(value, list) or len(value) != 1 or not isinstance(value[0], dict):
            raise PackageGroupError(f"npm view {package} dist-tags returned invalid JSON")
        observed = value[0].get(tag)
        if observed is not None and not isinstance(observed, str):
            raise PackageGroupError(
                f"npm view {package} dist-tag {tag!r} is not a version string"
            )
        return observed

    def publish(self, tarball: Path, tag: str) -> None:
        self._run(
            "publish",
            str(tarball),
            "--ignore-scripts",
            "--access",
            "public",
            "--tag",
            tag,
        )


@dataclass
class DryRunNpmClient:
    manifest: dict[str, Any]
    versions: dict[tuple[str, str], str] = field(default_factory=dict)
    tags: dict[tuple[str, str], str] = field(default_factory=dict)
    operations: list[str] = field(default_factory=list)

    def version_integrity(self, package: str, version: str) -> str | None:
        return self.versions.get((package, version))

    def dist_tag(self, package: str, tag: str) -> str | None:
        return self.tags.get((package, tag))

    def publish(self, tarball: Path, tag: str) -> None:
        record = next(
            item for item in self.manifest["packages"] if item["tarball"] == tarball.name
        )
        self.operations.append(
            f"publish {record['name']}@{self.manifest['version']} --tag {tag}"
        )
        self.versions[(record["name"], self.manifest["version"])] = record["integrity"]
        self.tags[(record["name"], tag)] = self.manifest["version"]


def validate_registry_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    version = manifest.get("version")
    target_tag = manifest.get("target_dist_tag")
    packages = manifest.get("packages")
    if not isinstance(version, str) or not version:
        raise PackageGroupError("npm package group version must be a non-empty string")
    if not isinstance(target_tag, str) or not NPM_DIST_TAG_RE.fullmatch(target_tag):
        raise PackageGroupError("npm package group target_dist_tag must be lowercase")
    if not isinstance(packages, list) or not packages:
        raise PackageGroupError("npm package group must contain packages")

    names: set[str] = set()
    tarballs: set[str] = set()
    for index, record in enumerate(packages):
        owner = f"npm package group packages[{index}]"
        if not isinstance(record, dict):
            raise PackageGroupError(f"{owner} must be an object")
        name = record.get("name")
        tarball = record.get("tarball")
        integrity = record.get("integrity")
        if not isinstance(name, str) or not name.startswith("@") or "/" not in name:
            raise PackageGroupError(f"{owner}.name must be a scoped npm package")
        if not isinstance(tarball, str) or PurePosixPath(tarball).name != tarball:
            raise PackageGroupError(f"{owner}.tarball must be a file name")
        if not isinstance(integrity, str) or not INTEGRITY_RE.fullmatch(integrity):
            raise PackageGroupError(f"{owner}.integrity must be an npm sha512 integrity")
        if name in names:
            raise PackageGroupError(f"duplicate npm package name {name}")
        if tarball in tarballs:
            raise PackageGroupError(f"duplicate npm package tarball {tarball}")
        names.add(name)
        tarballs.add(tarball)
    return manifest


def _wait_for_published_state(
    record: dict[str, Any],
    version: str,
    target_tag: str,
    client: NpmClient,
    *,
    attempts: int,
    delay_seconds: float,
) -> None:
    """Wait for one accepted publish to become consistently observable."""

    last_observation = "the version was not visible"
    deadline = time.monotonic() + REGISTRY_OBSERVATION_SECONDS
    for attempt in range(1, attempts + 1):
        if time.monotonic() >= deadline:
            break
        try:
            observed_integrity = client.version_integrity(record["name"], version)
        except PackageGroupError as exc:
            last_observation = f"integrity lookup failed: {exc}"
        else:
            if observed_integrity is not None and observed_integrity != record["integrity"]:
                raise PackageGroupError(
                    f"{record['name']}@{version}: registry integrity differs from the "
                    "verified tarball"
                )
            if observed_integrity is None:
                last_observation = "the version was not visible"
            else:
                if time.monotonic() >= deadline:
                    break
                try:
                    observed_tag = client.dist_tag(record["name"], target_tag)
                except PackageGroupError as exc:
                    last_observation = f"dist-tag lookup failed: {exc}"
                else:
                    if observed_tag == version:
                        return
                    last_observation = (
                        f"dist-tag {target_tag!r} pointed to {observed_tag!r}"
                    )

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        if attempt < attempts and delay_seconds > 0:
            delay = min(delay_seconds * (2 ** (attempt - 1)), 30.0, remaining)
            print(f"{record['name']}@{version}: observation {attempt}/{attempts}: "
                  f"{last_observation}; retrying in {delay:g}s", flush=True)
            time.sleep(delay)

    raise RegistryPendingError(
        f"{record['name']}@{version}: registry state was not confirmed after "
        f"{attempt} attempts ({last_observation}); preserve the original artifacts and "
        "reconciliation report, then resume observation; do not republish this version"
    )


def write_reconciliation_report(path: Path, report: dict[str, Any]) -> None:
    """Persist attempted uploads before a subprocess or interruption can lose them."""

    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def reconcile_group(
    manifest: dict[str, Any],
    artifact_dir: Path,
    client: NpmClient,
    *,
    observation_attempts: int = DEFAULT_REGISTRY_OBSERVATION_ATTEMPTS,
    observation_delay_seconds: float = DEFAULT_REGISTRY_OBSERVATION_DELAY_SECONDS,
    recovery_report: dict[str, Any] | None = None,
    report_path: Path | None = None,
    observe_only: bool = False,
    publication_run_id: str | None = None,
    publication_attempt: int | None = None,
    recovery_run_id: str | None = None,
    recovery_attempt: int | None = None,
) -> dict[str, Any]:
    """Publish missing versions once, preserving ambiguous outcomes across recovery.

    Recovery uses the original manifest and tarballs. A recorded upload attempt is
    observation-only, including when registry replicas still return 404. Existing
    exact versions must still match both integrity and the requested final tag.
    """

    if observation_attempts < 1:
        raise PackageGroupError("registry observation attempts must be positive")
    if observation_delay_seconds < 0:
        raise PackageGroupError("registry observation delay must not be negative")

    if (publication_run_id is None) != (publication_attempt is None):
        raise PackageGroupError("publication run and attempt must be supplied together")
    if (recovery_run_id is None) != (recovery_attempt is None):
        raise PackageGroupError("recovery run and attempt must be supplied together")
    if recovery_run_id is not None and recovery_report is None:
        observe_only = True
    if recovery_report is not None:
        expected = {"run_id": recovery_run_id, "attempt": recovery_attempt}
        if (recovery_run_id is None or not isinstance(recovery_report, dict)
                or recovery_report.get("publication") != expected):
            # An older surviving report cannot rule out an upload in a missing attempt.
            observe_only = True
    manifest = validate_registry_manifest(manifest)
    version = manifest["version"]
    target_tag = manifest["target_dist_tag"]
    identity = hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    names = {record["name"] for record in manifest["packages"]}
    states = {name: "not_attempted" for name in names}
    if isinstance(recovery_report, dict) and recovery_report.get("schema_version") == 2:
        if (recovery_report.get("version") != version
                or recovery_report.get("target_dist_tag") != target_tag):
            raise PackageGroupError("legacy recovery report does not match the verified version and tag")
        states = {name: "upload_outcome_unknown" for name in names}
        observe_only = True
        recovery_report = None
    if recovery_report is not None:
        prior_states = recovery_report.get("package_states") if isinstance(recovery_report, dict) else None
        allowed_states = {
            "not_attempted", "upload_started", "accepted_pending_visibility",
            "upload_outcome_unknown", "verified", "already_published",
        }
        if (
            not isinstance(recovery_report, dict)
            or recovery_report.get("schema_version") != 3
            or recovery_report.get("manifest_sha256") != identity
            or recovery_report.get("source_sha") != manifest.get("source_sha")
            or recovery_report.get("version") != version
            or recovery_report.get("target_dist_tag") != target_tag
            or not isinstance(prior_states, dict)
            or set(prior_states) != names
            or any(not isinstance(value, str) or value not in allowed_states for value in prior_states.values())
        ):
            raise PackageGroupError("recovery report does not match the verified package-group manifest")
        if "observe_only" in recovery_report and type(recovery_report["observe_only"]) is not bool:
            raise PackageGroupError("recovery report observe_only must be a boolean")
        observe_only = observe_only or recovery_report.get("observe_only", True)
        states.update(prior_states)
    if observe_only:
        # Persist the whole uncertain history before the first registry lookup can fail.
        states = {
            name: "upload_outcome_unknown" if state == "not_attempted" else state
            for name, state in states.items()
        }
    attempted = {name for name, state in states.items() if state != "not_attempted"}
    report: dict[str, Any] = {
        "schema_version": 3,
        **({"publication": {"run_id": publication_run_id, "attempt": publication_attempt}}
           if publication_run_id is not None else {}),
        "manifest_sha256": identity,
        "source_sha": manifest.get("source_sha"),
        "version": version,
        "target_dist_tag": target_tag,
        "already_published": [],
        "published": [],
        "package_states": states,
        "observe_only": observe_only,
        "status": "running",
    }

    def persist() -> None:
        if report_path is not None:
            write_reconciliation_report(report_path, report)

    def failed(error: PackageGroupError, status: str) -> ReconciliationError:
        report["status"] = status
        report["error"] = str(error)
        persist()
        return ReconciliationError(str(error), report)

    persist()
    missing: list[dict[str, Any]] = []
    observing: list[dict[str, Any]] = []
    try:
        # Complete every existing-version preflight before performing any upload.
        for record in manifest["packages"]:
            name = record["name"]
            observed_integrity = client.version_integrity(name, version)
            if observed_integrity is None:
                (observing if name in attempted or observe_only else missing).append(record)
                continue
            if observed_integrity != record["integrity"]:
                raise PackageGroupError(
                    f"{name}@{version}: registry integrity differs from the verified tarball"
                )
            observed_tag = client.dist_tag(name, target_tag)
            if observed_tag != version:
                if states[name] in {"upload_started", "accepted_pending_visibility", "upload_outcome_unknown"}:
                    observing.append(record)
                    continue
                raise PackageGroupError(
                    f"{name}@{version}: dist-tag {target_tag!r} points to "
                    f"{observed_tag!r}; npm Trusted Publisher cannot repair dist-tags, "
                    "so restore the tag with maintainer credentials before rerunning"
                )
            states[name] = "already_published"
            report["already_published"].append(name)
        persist()
    except PackageGroupError as exc:
        raise failed(exc, "failed-before-publish") from exc

    for record in observing:
        try:
            _wait_for_published_state(
                record, version, target_tag, client,
                attempts=observation_attempts, delay_seconds=observation_delay_seconds,
            )
        except RegistryPendingError as exc:
            raise failed(exc, "pending-registry-visibility") from exc
        except PackageGroupError as exc:
            raise failed(exc, "registry-conflict") from exc
        states[record["name"]] = "already_published"
        report["already_published"].append(record["name"])
        persist()

    for record in missing:
        name = record["name"]
        publish_error: PackageGroupError | None = None
        states[name] = "upload_started"
        persist()
        try:
            client.publish(artifact_dir / record["tarball"], target_tag)
            states[name] = "accepted_pending_visibility"
            report["published"].append(name)
        except PackageGroupError as exc:
            publish_error = exc
            states[name] = "upload_outcome_unknown"
        persist()
        try:
            _wait_for_published_state(
                record, version, target_tag, client,
                attempts=observation_attempts, delay_seconds=observation_delay_seconds,
            )
        except PackageGroupError as observation_error:
            error = observation_error
            if publish_error is not None:
                error = PackageGroupError(
                    f"{publish_error}; registry observation could not confirm whether "
                    f"npm accepted the publish: {observation_error}"
                )
            status = (
                "pending-registry-visibility" if publish_error is None
                else "publish-outcome-unknown"
            ) if isinstance(observation_error, RegistryPendingError) else "registry-conflict"
            raise failed(error, status) from observation_error
        if publish_error is not None:
            report["published"].append(name)
        states[name] = "verified"
        persist()

    report["status"] = "released"
    persist()
    return report


def main(argv: list[str] | None = None) -> int:
    """Observe the independently assembled grammar tarball without publishing it."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["inspect", "observe"])
    parser.add_argument("--package", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--tarball", type=Path, required=True)
    parser.add_argument("--registry", default=NPMJS_REGISTRY_URL)
    args = parser.parse_args(argv)
    try:
        integrity = "sha512-" + base64.b64encode(hashlib.sha512(args.tarball.read_bytes()).digest()).decode()
        record = {"name": args.package, "tarball": args.tarball.name, "integrity": integrity}
        validate_registry_manifest({"version": args.version, "target_dist_tag": args.tag, "packages": [record]})
        client = NpmCli(args.registry)
        if args.command == "inspect":
            observed = client.version_integrity(args.package, args.version)
            if observed is not None and observed != integrity:
                raise PackageGroupError("registry integrity differs from the verified tarball")
            print("missing" if observed is None else "exact")
            return 0
        _wait_for_published_state(
            record, args.version, args.tag, client,
            attempts=DEFAULT_REGISTRY_OBSERVATION_ATTEMPTS,
            delay_seconds=DEFAULT_REGISTRY_OBSERVATION_DELAY_SECONDS,
        )
    except (PackageGroupError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
