#!/usr/bin/env python3
"""Verify and reconcile the public @mermanjs/node npm package group."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
import tarfile
from pathlib import Path, PurePosixPath
from typing import Any

try:
    from scripts.npm_package_group import (
        DryRunNpmClient,
        NPMJS_REGISTRY_URL,
        NpmCli,
        PackageGroupError,
        ReconciliationError,
        reconcile_group,
        validate_registry_manifest,
    )
except ModuleNotFoundError:
    from npm_package_group import (
        DryRunNpmClient,
        NPMJS_REGISTRY_URL,
        NpmCli,
        PackageGroupError,
        ReconciliationError,
        reconcile_group,
        validate_registry_manifest,
    )


MANIFEST_NAME = "node-package-group.json"
MAX_PACKED_BYTES = 64 * 1024 * 1024
MAX_UNPACKED_BYTES = 128 * 1024 * 1024
MAX_MEMBERS = 5000
REPOSITORY = {
    "type": "git",
    "url": "git+https://github.com/Latias94/merman.git",
}
HOMEPAGE = "https://github.com/Latias94/merman/tree/main/platforms/node#readme"
EXPECTED_ROOT = {
    "name": "@mermanjs/node",
    "directory": "packages/node",
}
EXPECTED_WASM = {
    "name": "@mermanjs/node-wasm",
    "directory": "packages/node-wasm",
    "artifact_directory": "artifact",
    "node_artifact": "merman_node.cjs",
    "wasm_artifact": "merman_node_bg.wasm",
}
EXPECTED_TARGETS = [
    {
        "target": "darwin-arm64",
        "name": "@mermanjs/node-darwin-arm64",
        "directory": "packages/node-darwin-arm64",
        "node_artifact": "merman.node",
        "os": "darwin",
        "cpu": "arm64",
    },
    {
        "target": "darwin-x64",
        "name": "@mermanjs/node-darwin-x64",
        "directory": "packages/node-darwin-x64",
        "node_artifact": "merman.node",
        "os": "darwin",
        "cpu": "x64",
    },
    {
        "target": "linux-x64-gnu",
        "name": "@mermanjs/node-linux-x64-gnu",
        "directory": "packages/node-linux-x64-gnu",
        "node_artifact": "merman.node",
        "os": "linux",
        "cpu": "x64",
        "libc": "glibc",
    },
    {
        "target": "linux-x64-musl",
        "name": "@mermanjs/node-linux-x64-musl",
        "directory": "packages/node-linux-x64-musl",
        "node_artifact": "merman.node",
        "os": "linux",
        "cpu": "x64",
        "libc": "musl",
    },
    {
        "target": "win32-x64-msvc",
        "name": "@mermanjs/node-win32-x64-msvc",
        "directory": "packages/node-win32-x64-msvc",
        "node_artifact": "merman.node",
        "os": "win32",
        "cpu": "x64",
    },
]
REQUIRED_COMMON_FILES = {
    "package/package.json",
    "package/README.md",
    "package/LICENSE-APACHE",
    "package/LICENSE-MIT",
    "package/THIRD_PARTY_NOTICES.md",
}
REQUIRED_LOADER_FILES = {
    "package/CHANGELOG.md",
    "package/dist/index.mjs",
    "package/dist/index.d.ts",
    "package/dist/native-loader.mjs",
    "package/dist/generated/binding-contract.mjs",
    "package/dist/generated/capability-surface.mjs",
    "package/dist/generated/node-wire-contract.json",
}
REQUIRED_WASM_FILES = {
    "package/CHANGELOG.md",
    "package/dist/index.mjs",
    "package/dist/index.d.ts",
    "package/dist/candidates/wasm.mjs",
    "package/dist/candidates/wrap-engine.mjs",
    "package/dist/engine.mjs",
    "package/dist/errors.mjs",
    "package/dist/native-loader.mjs",
    "package/dist/generated/binding-contract.mjs",
    "package/dist/generated/capability-surface.mjs",
    "package/dist/generated/node-wire-contract.json",
    "package/artifact/merman_node.cjs",
    "package/artifact/merman_node_bg.wasm",
}


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise PackageGroupError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise PackageGroupError(f"{path} must contain a JSON object")
    return value


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_descriptor(path: Path) -> dict[str, Any]:
    descriptor = load_json(path)
    if (
        descriptor.get("schema_version") != 1
        or descriptor.get("admission_status") != "public-alpha"
    ):
        raise PackageGroupError("Node descriptor must define the schema-1 public alpha group")
    if not isinstance(descriptor.get("version"), str):
        raise PackageGroupError("Node descriptor version must be a string")
    if descriptor.get("node_engine") != ">=22.0.0":
        raise PackageGroupError("Node descriptor must require Node.js 22 or newer")
    if descriptor.get("root") != EXPECTED_ROOT:
        raise PackageGroupError("Node descriptor loader package does not match the public contract")
    if descriptor.get("wasm") != EXPECTED_WASM:
        raise PackageGroupError("Node descriptor WASM package does not match the public contract")
    if descriptor.get("targets") != EXPECTED_TARGETS:
        raise PackageGroupError("Node descriptor target packages do not match the public contract")
    return descriptor


def digest(path: Path, algorithm: str) -> bytes:
    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            hasher.update(chunk)
    return hasher.digest()


def npm_integrity(path: Path) -> str:
    return "sha512-" + base64.b64encode(digest(path, "sha512")).decode("ascii")


def sha256(path: Path) -> str:
    return "sha256:" + digest(path, "sha256").hex()


def safe_member_name(member: tarfile.TarInfo) -> str:
    name = PurePosixPath(member.name)
    if name.is_absolute() or ".." in name.parts or not name.parts or name.parts[0] != "package":
        raise PackageGroupError(f"unsafe npm tarball member {member.name!r}")
    if member.issym() or member.islnk() or member.isdev():
        raise PackageGroupError(f"unsupported npm tarball member type {member.name!r}")
    return name.as_posix()


def read_tarball_manifest(path: Path) -> tuple[dict[str, Any], set[str], int]:
    if not path.is_file() or path.stat().st_size > MAX_PACKED_BYTES:
        raise PackageGroupError(f"Node npm tarball is missing or too large: {path}")
    try:
        with tarfile.open(path, "r:gz") as archive:
            members = archive.getmembers()
            if len(members) > MAX_MEMBERS:
                raise PackageGroupError(f"{path.name} contains too many files")
            names: set[str] = set()
            unpacked = 0
            package_json_member: tarfile.TarInfo | None = None
            for member in members:
                name = safe_member_name(member)
                if name in names:
                    raise PackageGroupError(f"{path.name} contains duplicate member {name}")
                names.add(name)
                if member.isfile():
                    unpacked += member.size
                    if unpacked > MAX_UNPACKED_BYTES:
                        raise PackageGroupError(f"{path.name} expands beyond the size limit")
                if name == "package/package.json":
                    package_json_member = member
            if package_json_member is None:
                raise PackageGroupError(f"{path.name} has no package/package.json")
            handle = archive.extractfile(package_json_member)
            if handle is None:
                raise PackageGroupError(f"cannot read package.json from {path.name}")
            manifest = json.loads(handle.read().decode("utf-8"))
    except (tarfile.TarError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PackageGroupError(f"invalid npm tarball {path}: {exc}") from exc
    if not isinstance(manifest, dict):
        raise PackageGroupError(f"{path.name} package.json must be an object")
    return manifest, names, unpacked


def descriptor_entries(descriptor: dict[str, Any]) -> dict[str, tuple[str, dict[str, Any]]]:
    entries: dict[str, tuple[str, dict[str, Any]]] = {}
    for target in descriptor["targets"]:
        entries[target["name"]] = ("platform", target)
    entries[descriptor["wasm"]["name"]] = ("wasm", descriptor["wasm"])
    entries[descriptor["root"]["name"]] = ("loader", descriptor["root"])
    return entries


def inspect_tarball(
    path: Path,
    descriptor: dict[str, Any],
    expected_version: str,
) -> dict[str, Any]:
    manifest, names, unpacked = read_tarball_manifest(path)
    package_name = manifest.get("name")
    entry = descriptor_entries(descriptor).get(package_name)
    if entry is None:
        raise PackageGroupError(f"{path.name} contains unexpected package {package_name!r}")
    role, package_descriptor = entry
    if manifest.get("version") != expected_version:
        raise PackageGroupError(f"{package_name} version must be {expected_version}")
    if manifest.get("private") is True or manifest.get("publishConfig", {}).get("access") != "public":
        raise PackageGroupError(f"{package_name} must be publishable with public access")
    if manifest.get("repository") != REPOSITORY or manifest.get("homepage") != HOMEPAGE:
        raise PackageGroupError(f"{package_name} repository metadata is invalid")
    if manifest.get("license") != "MIT OR Apache-2.0":
        raise PackageGroupError(f"{package_name} license metadata is invalid")
    if manifest.get("engines") != {"node": descriptor["node_engine"]}:
        raise PackageGroupError(f"{package_name} Node.js engine constraint is invalid")
    scripts = manifest.get("scripts", {})
    if isinstance(scripts, dict) and any(key in scripts for key in ("preinstall", "install", "postinstall")):
        raise PackageGroupError(f"{package_name} must not run npm install lifecycle scripts")
    missing_common = REQUIRED_COMMON_FILES - names
    if missing_common:
        raise PackageGroupError(
            f"{package_name} is missing required files: {', '.join(sorted(missing_common))}"
        )
    node_files = sorted(name for name in names if name.endswith(".node"))
    wasm_files = sorted(name for name in names if name.endswith(".wasm"))
    if role != "wasm" and wasm_files:
        raise PackageGroupError(f"{package_name} must not contain WASM artifacts")
    if role == "loader":
        missing_loader = REQUIRED_LOADER_FILES - names
        if missing_loader or node_files:
            raise PackageGroupError(
                f"{package_name} loader contents are invalid"
            )
        expected_dependencies = {
            target["name"]: expected_version for target in descriptor["targets"]
        }
        if manifest.get("optionalDependencies") != expected_dependencies:
            raise PackageGroupError(
                f"{package_name} must depend on every platform package at {expected_version}"
            )
        if any(key in manifest for key in ("os", "cpu", "libc")):
            raise PackageGroupError(f"{package_name} loader must not declare platform constraints")
    elif role == "wasm":
        missing_wasm = REQUIRED_WASM_FILES - names
        if missing_wasm:
            raise PackageGroupError(
                f"{package_name} is missing required WASM files: {', '.join(sorted(missing_wasm))}"
            )
        if node_files:
            raise PackageGroupError(f"{package_name} WASM package must not contain native artifacts")
        if wasm_files != [f"package/{package_descriptor['artifact_directory']}/{package_descriptor['wasm_artifact']}"]:
            raise PackageGroupError(f"{package_name} must contain exactly one canonical WASM artifact")
        cjs_files = sorted(name for name in names if name.endswith(".cjs"))
        if cjs_files != [f"package/{package_descriptor['artifact_directory']}/{package_descriptor['node_artifact']}"]:
            raise PackageGroupError(f"{package_name} must contain exactly one Node WASM loader")
        if manifest.get("main") != "./dist/index.mjs" or manifest.get("types") != "./dist/index.d.ts":
            raise PackageGroupError(f"{package_name} Node WASM entry point is invalid")
        if any(key in manifest for key in ("os", "cpu", "libc")):
            raise PackageGroupError(f"{package_name} WASM package must not declare platform constraints")
        if any(key in manifest for key in ("dependencies", "optionalDependencies", "peerDependencies")):
            raise PackageGroupError(f"{package_name} WASM package must not declare runtime dependencies")
    elif node_files != [f"package/{package_descriptor['node_artifact']}"]:
        raise PackageGroupError(
            f"{package_name} must contain exactly package/{package_descriptor['node_artifact']}"
        )
    else:
        for field in ("os", "cpu", "libc"):
            expected = package_descriptor.get(field)
            observed = manifest.get(field)
            if observed != ([expected] if expected is not None else None):
                raise PackageGroupError(
                    f"{package_name} {field} constraint does not match the public contract"
                )
        if manifest.get("main") != f"./{package_descriptor['node_artifact']}":
            raise PackageGroupError(f"{package_name} native entry point is invalid")
        if "optionalDependencies" in manifest:
            raise PackageGroupError(f"{package_name} must not depend on other native packages")
    return {
        "name": package_name,
        "role": role,
        **({"target": package_descriptor["target"]} if role == "platform" else {}),
        "tarball": path.name,
        "bytes": path.stat().st_size,
        "unpacked_bytes": unpacked,
        "sha256": sha256(path),
        "integrity": npm_integrity(path),
    }


def validate_manifest(
    manifest: dict[str, Any], descriptor: dict[str, Any] | None = None
) -> dict[str, Any]:
    if manifest.get("schema_version") != 1:
        raise PackageGroupError("Node package group manifest schema_version must be 1")
    validate_registry_manifest(manifest)
    source_sha = manifest.get("source_sha")
    if not isinstance(source_sha, str) or len(source_sha) != 40 or any(
        character not in "0123456789abcdef" for character in source_sha
    ):
        raise PackageGroupError("Node package group source_sha must be a full lowercase Git SHA")
    if descriptor is not None:
        expected_names = [target["name"] for target in descriptor["targets"]] + [
            descriptor["wasm"]["name"],
            descriptor["root"]["name"],
        ]
        observed_names = [record["name"] for record in manifest["packages"]]
        if observed_names != expected_names:
            raise PackageGroupError("Node package group must list every platform and WASM package before the loader")
    return manifest


def create_manifest(
    artifact_dir: Path,
    descriptor_path: Path,
    *,
    version: str,
    source_sha: str,
    target_dist_tag: str,
    output: Path | None = None,
) -> Path:
    descriptor = load_descriptor(descriptor_path)
    if descriptor["version"] != version:
        raise PackageGroupError(
            f"Node descriptor version is {descriptor['version']}, expected {version}"
        )
    records = [
        inspect_tarball(path, descriptor, version)
        for path in sorted(artifact_dir.glob("*.tgz"))
    ]
    by_name = {record["name"]: record for record in records}
    expected_names = [target["name"] for target in descriptor["targets"]] + [
        descriptor["wasm"]["name"],
        descriptor["root"]["name"],
    ]
    if set(by_name) != set(expected_names) or len(records) != len(expected_names):
        raise PackageGroupError("artifact directory must contain exactly the seven Node npm packages")
    manifest = validate_manifest(
        {
            "schema_version": 1,
            "version": version,
            "source_sha": source_sha,
            "target_dist_tag": target_dist_tag,
            "packages": [by_name[name] for name in expected_names],
        },
        descriptor,
    )
    destination = output or artifact_dir / MANIFEST_NAME
    write_json(destination, manifest)
    return destination


def verify_artifact(
    manifest_path: Path,
    artifact_dir: Path,
    *,
    descriptor_path: Path | None = None,
    expected_version: str | None = None,
    expected_source_sha: str | None = None,
    expected_target_dist_tag: str | None = None,
) -> dict[str, Any]:
    descriptor = load_descriptor(descriptor_path) if descriptor_path else None
    manifest = validate_manifest(load_json(manifest_path), descriptor)
    for field, expected in (
        ("version", expected_version),
        ("source_sha", expected_source_sha),
        ("target_dist_tag", expected_target_dist_tag),
    ):
        if expected is not None and manifest[field] != expected:
            raise PackageGroupError(f"Node package group {field} must be {expected}")
    if descriptor is None:
        raise PackageGroupError("descriptor is required to verify Node package contents")
    observed = {
        record["name"]: inspect_tarball(
            artifact_dir / record["tarball"], descriptor, manifest["version"]
        )
        for record in manifest["packages"]
    }
    for record in manifest["packages"]:
        if observed[record["name"]] != record:
            raise PackageGroupError(
                f"{record['name']} tarball no longer matches the Node package group manifest"
            )
    return manifest


def cli() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    create = subparsers.add_parser("create-manifest")
    create.add_argument("--artifact-dir", type=Path, required=True)
    create.add_argument("--descriptor", type=Path, required=True)
    create.add_argument("--version", required=True)
    create.add_argument("--source-sha", required=True)
    create.add_argument("--target-dist-tag", required=True)
    create.add_argument("--output", type=Path)

    verify = subparsers.add_parser("verify-artifact")
    verify.add_argument("--manifest", type=Path, required=True)
    verify.add_argument("--artifact-dir", type=Path, required=True)
    verify.add_argument("--descriptor", type=Path, required=True)
    verify.add_argument("--version")
    verify.add_argument("--source-sha")
    verify.add_argument("--target-dist-tag")

    reconcile = subparsers.add_parser("reconcile")
    reconcile.add_argument("--manifest", type=Path, required=True)
    reconcile.add_argument("--artifact-dir", type=Path, required=True)
    reconcile.add_argument("--descriptor", type=Path, required=True)
    reconcile.add_argument("--registry", default=NPMJS_REGISTRY_URL)
    reconcile.add_argument("--report", type=Path, required=True)
    reconcile.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = cli().parse_args(argv)
    try:
        if args.command == "create-manifest":
            print(
                create_manifest(
                    args.artifact_dir,
                    args.descriptor,
                    version=args.version,
                    source_sha=args.source_sha,
                    target_dist_tag=args.target_dist_tag,
                    output=args.output,
                )
            )
            return 0
        manifest = verify_artifact(
            args.manifest,
            args.artifact_dir,
            descriptor_path=args.descriptor,
            expected_version=getattr(args, "version", None),
            expected_source_sha=getattr(args, "source_sha", None),
            expected_target_dist_tag=getattr(args, "target_dist_tag", None),
        )
        if args.command == "verify-artifact":
            print(f"validated {len(manifest['packages'])} packed Node package(s)")
            return 0
        client = DryRunNpmClient(manifest) if args.dry_run else NpmCli(args.registry)
        report = reconcile_group(manifest, args.artifact_dir, client)
        if isinstance(client, DryRunNpmClient):
            report["dry_run_operations"] = client.operations
        write_json(args.report, report)
        print(args.report)
        return 0
    except ReconciliationError as exc:
        write_json(args.report, exc.report)
        print(f"error: {exc}", file=sys.stderr)
        return 1
    except PackageGroupError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
