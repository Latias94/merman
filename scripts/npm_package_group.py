#!/usr/bin/env python3
"""Publish verified lockstep package groups with npm Trusted Publisher."""

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


def reconcile_group(
    manifest: dict[str, Any], artifact_dir: Path, client: NpmClient
) -> dict[str, Any]:
    """Publish missing versions directly under the final tag in manifest order.

    npm Trusted Publisher credentials authorize ``npm publish`` but not a later
    ``npm dist-tag`` mutation. Existing versions therefore have to be complete
    and correctly tagged before this function mutates the registry. A retry can
    safely skip packages published by an earlier partial run.
    """

    manifest = validate_registry_manifest(manifest)
    version = manifest["version"]
    target_tag = manifest["target_dist_tag"]
    report: dict[str, Any] = {
        "schema_version": 2,
        "version": version,
        "target_dist_tag": target_tag,
        "already_published": [],
        "published": [],
        "status": "running",
    }
    missing: list[dict[str, Any]] = []

    try:
        for record in manifest["packages"]:
            name = record["name"]
            observed_integrity = client.version_integrity(name, version)
            if observed_integrity is None:
                missing.append(record)
                continue
            if observed_integrity != record["integrity"]:
                raise PackageGroupError(
                    f"{name}@{version}: registry integrity differs from the verified tarball"
                )
            observed_tag = client.dist_tag(name, target_tag)
            if observed_tag != version:
                raise PackageGroupError(
                    f"{name}@{version}: dist-tag {target_tag!r} points to "
                    f"{observed_tag!r}; npm Trusted Publisher cannot repair dist-tags, "
                    "so restore the tag with maintainer credentials before rerunning"
                )
            report["already_published"].append(name)
    except PackageGroupError as exc:
        report["status"] = "failed-before-publish"
        report["error"] = str(exc)
        raise ReconciliationError(str(exc), report) from exc

    for record in missing:
        name = record["name"]
        try:
            client.publish(artifact_dir / record["tarball"], target_tag)
            report["published"].append(name)
            observed_integrity = client.version_integrity(name, version)
            if observed_integrity != record["integrity"]:
                raise PackageGroupError(
                    f"{name}@{version}: registry integrity differs after publish"
                )
            observed_tag = client.dist_tag(name, target_tag)
            if observed_tag != version:
                raise PackageGroupError(
                    f"{name}@{version}: dist-tag {target_tag!r} points to "
                    f"{observed_tag!r} after publish"
                )
        except PackageGroupError as exc:
            report["status"] = "failed-during-publish"
            report["error"] = str(exc)
            raise ReconciliationError(str(exc), report) from exc

    report["status"] = "released"
    return report
