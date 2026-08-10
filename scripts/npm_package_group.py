#!/usr/bin/env python3
"""Shared npm registry reconciliation for verified lockstep package groups."""

from __future__ import annotations

import json
import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any, Protocol


INTEGRITY_RE = re.compile(r"sha512-[A-Za-z0-9+/]+={0,2}\Z")
NPM_DIST_TAG_RE = re.compile(r"[a-z][a-z0-9-]*\Z")
NPMJS_REGISTRY_URL = "https://registry.npmjs.org"


class PackageGroupError(ValueError):
    """The package group artifact or registry state is invalid."""


class ReconciliationError(PackageGroupError):
    """A registry mutation failed after reportable reconciliation state existed."""

    def __init__(self, message: str, report: dict[str, Any]) -> None:
        super().__init__(message)
        self.report = report


class NpmClient(Protocol):
    def version_integrity(self, package: str, version: str) -> str | None: ...

    def dist_tag(self, package: str, tag: str) -> str | None: ...

    def publish(self, tarball: Path, tag: str) -> None: ...

    def add_tag(self, package: str, version: str, tag: str) -> None: ...

    def remove_tag(self, package: str, tag: str) -> None: ...


@dataclass
class NpmCli:
    registry: str | None = None

    def _command(self, *args: str) -> list[str]:
        command = ["npm", *args]
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

    def version_integrity(self, package: str, version: str) -> str | None:
        result = subprocess.run(
            self._command("view", f"{package}@{version}", "dist.integrity", "--json"),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if "E404" in diagnostic or "404" in diagnostic:
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
        if not isinstance(value, str) or not INTEGRITY_RE.fullmatch(value):
            raise PackageGroupError(
                f"npm view {package}@{version} returned invalid dist.integrity"
            )
        return value

    def dist_tag(self, package: str, tag: str) -> str | None:
        result = subprocess.run(
            self._command("view", package, "dist-tags", "--json"),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if "E404" in diagnostic or "404" in diagnostic:
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
        if not isinstance(value, dict):
            raise PackageGroupError(f"npm view {package} dist-tags returned invalid JSON")
        observed = value.get(tag)
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

    def add_tag(self, package: str, version: str, tag: str) -> None:
        self._run("dist-tag", "add", f"{package}@{version}", tag)

    def remove_tag(self, package: str, tag: str) -> None:
        self._run("dist-tag", "rm", package, tag)


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

    def add_tag(self, package: str, version: str, tag: str) -> None:
        self.operations.append(f"dist-tag add {package}@{version} {tag}")
        self.tags[(package, tag)] = version

    def remove_tag(self, package: str, tag: str) -> None:
        self.operations.append(f"dist-tag rm {package} {tag}")
        self.tags.pop((package, tag), None)


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


def staging_tag(version: str) -> str:
    return "staging-v" + re.sub(r"[^a-z0-9]+", "-", version.lower()).strip("-")


def stage_group(
    manifest: dict[str, Any], artifact_dir: Path, client: NpmClient
) -> dict[str, Any]:
    manifest = validate_registry_manifest(manifest)
    version = manifest["version"]
    stage = staging_tag(version)
    report: dict[str, Any] = {
        "schema_version": 1,
        "version": version,
        "target_dist_tag": manifest["target_dist_tag"],
        "staging_dist_tag": stage,
        "published": [],
        "status": "running",
    }
    for record in manifest["packages"]:
        try:
            observed_integrity = client.version_integrity(record["name"], version)
            if observed_integrity is None:
                client.publish(artifact_dir / record["tarball"], stage)
                report["published"].append(record["name"])
                observed_integrity = client.version_integrity(record["name"], version)
        except PackageGroupError as exc:
            report["status"] = "failed-before-promotion"
            report["error"] = str(exc)
            raise ReconciliationError(str(exc), report) from exc
        if observed_integrity != record["integrity"]:
            message = (
                f"{record['name']}@{version}: registry integrity differs from the verified tarball"
            )
            report["status"] = "failed-before-promotion"
            report["error"] = message
            raise ReconciliationError(message, report)
    report["status"] = "staged"
    return report


def restore_tags(
    client: NpmClient,
    changed: list[dict[str, Any]],
    previous: dict[str, str | None],
    tag: str,
) -> list[str]:
    failures: list[str] = []
    for record in reversed(changed):
        name = record["name"]
        prior = previous[name]
        try:
            if prior is None:
                client.remove_tag(name, tag)
                observed = client.dist_tag(name, tag)
                if observed is not None:
                    raise PackageGroupError(f"tag still points to {observed!r}")
            else:
                client.add_tag(name, prior, tag)
                observed = client.dist_tag(name, tag)
                if observed != prior:
                    raise PackageGroupError(f"tag points to {observed!r}, expected {prior!r}")
        except PackageGroupError as exc:
            failures.append(f"{name}: {exc}")
    return failures


def promote_group(manifest: dict[str, Any], client: NpmClient) -> dict[str, Any]:
    manifest = validate_registry_manifest(manifest)
    version = manifest["version"]
    target_tag = manifest["target_dist_tag"]
    report: dict[str, Any] = {
        "schema_version": 1,
        "version": version,
        "target_dist_tag": target_tag,
        "staging_dist_tag": staging_tag(version),
        "promoted": [],
        "previous_tags": {},
        "status": "running",
    }
    try:
        previous = {
            record["name"]: client.dist_tag(record["name"], target_tag)
            for record in manifest["packages"]
        }
    except PackageGroupError as exc:
        report["status"] = "failed-before-promotion"
        report["error"] = str(exc)
        raise ReconciliationError(str(exc), report) from exc
    report["previous_tags"] = previous
    changed: list[dict[str, Any]] = []
    try:
        for record in manifest["packages"]:
            if previous[record["name"]] == version:
                continue
            changed.append(record)
            client.add_tag(record["name"], version, target_tag)
            observed_tag = client.dist_tag(record["name"], target_tag)
            if observed_tag != version:
                raise PackageGroupError(
                    f"{record['name']}: dist-tag {target_tag!r} points to "
                    f"{observed_tag!r} after promotion"
                )
    except PackageGroupError as exc:
        rollback_failures = restore_tags(client, changed, previous, target_tag)
        detail = str(exc)
        if rollback_failures:
            detail += "; rollback failed: " + "; ".join(rollback_failures)
        report["status"] = "failed-during-promotion"
        report["error"] = detail
        report["promoted"] = [record["name"] for record in changed]
        report["rollback_failures"] = rollback_failures
        raise ReconciliationError(detail, report) from exc
    report["status"] = "promoted"
    report["promoted"] = [record["name"] for record in changed]
    return report


def reconcile_group(
    manifest: dict[str, Any], artifact_dir: Path, client: NpmClient
) -> dict[str, Any]:
    staged = stage_group(manifest, artifact_dir, client)
    promoted = promote_group(manifest, client)
    return {
        **staged,
        "promoted": promoted["promoted"],
        "previous_tags": promoted["previous_tags"],
        "status": "reconciled",
    }
