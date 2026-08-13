"""Workspace release-version authority, projections, and updates."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, Sequence

try:
    from scripts.release_version import ReleaseVersion, parse_release_version
except ModuleNotFoundError:
    from release_version import ReleaseVersion, parse_release_version

try:
    from scripts import web_package_group
except ModuleNotFoundError:
    import web_package_group

try:
    from scripts import release_version_owners
except ModuleNotFoundError:
    import release_version_owners

ROOT_MANIFEST = Path("Cargo.toml")
ROOT_LOCK = Path("Cargo.lock")
RUST_TOOLCHAIN = Path("rust-toolchain.toml")
FUZZ_MANIFEST = Path("fuzz/Cargo.toml")
FUZZ_LOCK = Path("fuzz/Cargo.lock")
WEB_WORKSPACE_PACKAGE = Path("platforms/web/package.json")
WEB_DESCRIPTOR = Path("platforms/web/web-surface-descriptor.json")
WEB_LOCK = Path("platforms/web/package-lock.json")
NODE_ROOT = Path("platforms/node")
NODE_CARGO_MANIFEST = Path("crates/merman-node/Cargo.toml")
NODE_CARGO_LOCK = Path("crates/merman-node/Cargo.lock")
NODE_DESCRIPTOR = NODE_ROOT / "package-surfaces.json"
NODE_WORKSPACE_PACKAGE = NODE_ROOT / "package.json"
NODE_WORKSPACE_LOCK = NODE_ROOT / "package-lock.json"
NODE_CARGO_PACKAGE = "merman-node-candidate"
NODE_BINDINGS_PACKAGE = "merman-bindings-core"
PLAYGROUND_PACKAGE = Path("playground/package.json")
PLAYGROUND_LOCK = Path("playground/package-lock.json")
PLAYGROUND_LICENSE_REPORT = Path(
    "playground/public/THIRD_PARTY_LICENSES/npm-production-dependencies.txt"
)
PYTHON_MANIFEST = release_version_owners.PYTHON_MANIFEST
ANDROID_MANIFEST = release_version_owners.ANDROID_MANIFEST
FLUTTER_MANIFEST = release_version_owners.FLUTTER_MANIFEST
FLUTTER_PACKAGE_VERSION = release_version_owners.FLUTTER_PACKAGE_VERSION
NPM_REGISTRY = "https://registry.npmjs.org/"


class ReleaseProjectionError(ValueError):
    """The release projection graph is malformed or cannot be updated safely."""


@dataclass(frozen=True)
class VersionObservation:
    label: str
    path: Path
    expected: str
    actual: str

    @property
    def matches(self) -> bool:
        return self.actual == self.expected


@dataclass(frozen=True)
class VerificationResult:
    authority: ReleaseVersion
    observations: tuple[VersionObservation, ...]
    errors: tuple[str, ...]

    @property
    def ok(self) -> bool:
        return not self.errors and all(item.matches for item in self.observations)


@dataclass(frozen=True)
class WorkspaceCatalog:
    authority: ReleaseVersion
    coupled_packages: Mapping[str, Path]
    independent_packages: Mapping[str, tuple[Path, str]]
    member_manifests: tuple[Path, ...]
    root_data: Mapping[str, Any]


@dataclass(frozen=True)
class _ReleasePreimage:
    head: str
    index_digest: str


@dataclass(frozen=True)
class _NodePackageEntry:
    name: str
    directory: Path

    @property
    def manifest_path(self) -> Path:
        return NODE_ROOT / self.directory / "package.json"


@dataclass(frozen=True)
class _NodePackageCatalog:
    version: str
    root: _NodePackageEntry
    targets: tuple[_NodePackageEntry, ...]

    @property
    def entries(self) -> tuple[_NodePackageEntry, ...]:
        return (self.root, *self.targets)


class RepositoryView:
    def __init__(
        self,
        root: Path,
        overrides: Mapping[Path | str, str] | None = None,
    ) -> None:
        self.root = root.resolve()
        self.overrides = {
            self._relative(Path(path)): text for path, text in (overrides or {}).items()
        }
        self._cache: dict[Path, str] = {}
        self._toml_cache: dict[Path, tuple[str, Mapping[str, Any]]] = {}
        self._json_cache: dict[Path, tuple[str, Mapping[str, Any]]] = {}

    def _relative(self, path: Path) -> Path:
        if path.is_absolute():
            try:
                path = path.resolve().relative_to(self.root)
            except ValueError as exc:
                raise ReleaseProjectionError(
                    f"release projection path escapes repository root: {path}"
                ) from exc
        normalized = Path(os.path.normpath(path))
        if normalized.is_absolute() or ".." in normalized.parts:
            raise ReleaseProjectionError(
                f"release projection path escapes repository root: {path}"
            )
        return normalized

    def text(self, path: Path | str) -> str:
        relative = self._relative(Path(path))
        if relative in self.overrides:
            return self.overrides[relative]
        if relative in self._cache:
            return self._cache[relative]
        try:
            text = (self.root / relative).read_text(encoding="utf-8")
        except OSError as exc:
            raise ReleaseProjectionError(f"cannot read {relative}: {exc}") from exc
        self._cache[relative] = text
        return text

    def toml(self, path: Path | str) -> Mapping[str, Any]:
        relative = self._relative(Path(path))
        text = self.text(relative)
        cached = self._toml_cache.get(relative)
        if cached is not None and cached[0] == text:
            return copy.deepcopy(cached[1])
        try:
            value = tomllib.loads(text)
        except tomllib.TOMLDecodeError as exc:
            raise ReleaseProjectionError(f"invalid TOML in {relative}: {exc}") from exc
        if not isinstance(value, dict):
            raise ReleaseProjectionError(f"expected a TOML document in {relative}")
        self._toml_cache[relative] = (text, value)
        return copy.deepcopy(value)

    def json(self, path: Path | str) -> Mapping[str, Any]:
        relative = self._relative(Path(path))
        text = self.text(relative)
        cached = self._json_cache.get(relative)
        if cached is not None and cached[0] == text:
            return copy.deepcopy(cached[1])
        try:
            value = json.loads(text, object_pairs_hook=_reject_duplicate_keys)
        except json.JSONDecodeError as exc:
            raise ReleaseProjectionError(f"invalid JSON in {relative}: {exc}") from exc
        if not isinstance(value, dict):
            raise ReleaseProjectionError(f"expected a JSON object in {relative}")
        self._json_cache[relative] = (text, value)
        return copy.deepcopy(value)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ReleaseProjectionError(f"duplicate JSON object key: {key!r}")
        result[key] = value
    return result


def _mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReleaseProjectionError(f"{label} must be a table/object")
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ReleaseProjectionError(f"{label} must be a non-empty string")
    return value


def load_workspace_catalog(view: RepositoryView) -> WorkspaceCatalog:
    root_data = view.toml(ROOT_MANIFEST)
    workspace = _mapping(root_data.get("workspace"), "Cargo.toml [workspace]")
    package = _mapping(workspace.get("package"), "Cargo.toml [workspace.package]")
    authority = parse_release_version(
        _string(package.get("version"), "Cargo.toml workspace.package.version"),
        allow_v_prefix=False,
    )
    metadata = _mapping(
        workspace.get("metadata", {}), "Cargo.toml [workspace.metadata]"
    )
    release_metadata = _mapping(
        metadata.get("merman-release"),
        "Cargo.toml [workspace.metadata.merman-release]",
    )
    independent_raw = release_metadata.get("independent-packages")
    if not isinstance(independent_raw, list) or not all(
        isinstance(name, str) and name for name in independent_raw
    ):
        raise ReleaseProjectionError(
            "Cargo.toml workspace.metadata.merman-release.independent-packages "
            "must be a string array"
        )
    independent = set(independent_raw)
    if len(independent) != len(independent_raw):
        raise ReleaseProjectionError(
            "Cargo.toml independent package declarations must be unique"
        )
    members = workspace.get("members")
    if not isinstance(members, list) or not members:
        raise ReleaseProjectionError("Cargo.toml workspace.members must be a non-empty array")

    coupled: dict[str, Path] = {}
    independent_packages: dict[str, tuple[Path, str]] = {}
    seen_names: set[str] = set()
    member_manifests: list[Path] = []
    for member in members:
        member_path = Path(_string(member, "Cargo.toml workspace member"))
        if any(part in {"..", "*", "?", "["} for part in member_path.parts):
            raise ReleaseProjectionError(
                f"workspace release gate requires an explicit member path: {member}"
            )
        manifest_path = member_path / "Cargo.toml"
        manifest = view.toml(manifest_path)
        member_manifests.append(manifest_path)
        member_package = _mapping(
            manifest.get("package"), f"{manifest_path} [package]"
        )
        name = _string(member_package.get("name"), f"{manifest_path} package.name")
        if name in seen_names:
            raise ReleaseProjectionError(f"duplicate workspace package name: {name}")
        seen_names.add(name)

        version_source = member_package.get("version")
        if name in independent:
            if isinstance(version_source, dict) and version_source.get("workspace") is True:
                raise ReleaseProjectionError(
                    f"independently versioned package {name} must not inherit the workspace version"
                )
            independent_packages[name] = (
                member_path,
                _string(version_source, f"{manifest_path} package.version"),
            )
            continue

        if version_source != {"workspace": True}:
            raise ReleaseProjectionError(
                f"{manifest_path} package {name} must use version.workspace = true; "
                "declare an intentional independent axis in "
                "workspace.metadata.merman-release.independent-packages"
            )
        coupled[name] = member_path

    unknown_independent = independent - seen_names
    if unknown_independent:
        raise ReleaseProjectionError(
            "independent package declarations are not workspace members: "
            + ", ".join(sorted(unknown_independent))
        )

    return WorkspaceCatalog(
        authority=authority,
        coupled_packages=coupled,
        independent_packages=independent_packages,
        member_manifests=tuple(member_manifests),
        root_data=root_data,
    )


def verify_repository(
    root: Path,
    *,
    expected_version: str | None = None,
    overrides: Mapping[Path | str, str] | None = None,
) -> VerificationResult:
    view = RepositoryView(root, overrides)
    catalog = load_workspace_catalog(view)
    expected = (
        parse_release_version(expected_version).canonical
        if expected_version is not None
        else catalog.authority.canonical
    )
    observations: list[VersionObservation] = [
        VersionObservation(
            "Cargo workspace authority",
            ROOT_MANIFEST,
            expected,
            catalog.authority.canonical,
        )
    ]
    errors: list[str] = []

    _collect_workspace_dependency_versions(view, catalog, observations, errors)
    _collect_member_dependency_policy(view, catalog, errors)
    _collect_cargo_lock_versions(
        view,
        catalog,
        ROOT_LOCK,
        set(catalog.coupled_packages),
        observations,
        errors,
    )
    _collect_independent_lock_versions(
        view,
        catalog,
        ROOT_LOCK,
        observations,
        errors,
        require_all=True,
    )
    _collect_fuzz_lock_versions(view, catalog, observations, errors)
    _collect_independent_lock_versions(
        view,
        catalog,
        FUZZ_LOCK,
        observations,
        errors,
    )
    _collect_node_candidate_projection(view, catalog, observations, errors)
    _collect_platform_versions(view, catalog.authority, observations)

    return VerificationResult(
        authority=catalog.authority,
        observations=tuple(observations),
        errors=tuple(errors),
    )


def _collect_workspace_dependency_versions(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    observations: list[VersionObservation],
    errors: list[str],
) -> None:
    workspace = _mapping(catalog.root_data["workspace"], "Cargo.toml [workspace]")
    dependencies = _mapping(
        workspace.get("dependencies"), "Cargo.toml [workspace.dependencies]"
    )
    coupled_dirs = _coupled_package_dirs(view, catalog)
    independent_dirs = {
        (view.root / member).resolve(): (name, version)
        for name, (member, version) in catalog.independent_packages.items()
    }
    inherited_by_package: dict[str, str] = {}
    projected_independent: dict[str, str] = {}

    for dependency_key, raw_spec in dependencies.items():
        if not isinstance(raw_spec, dict) or "path" not in raw_spec:
            continue
        dependency_path = Path(
            _string(raw_spec.get("path"), f"workspace dependency {dependency_key}.path")
        )
        resolved = (view.root / dependency_path).resolve()
        package_name = coupled_dirs.get(resolved)
        independent_package = independent_dirs.get(resolved)
        if independent_package is not None:
            independent_name, independent_version = independent_package
            declared_package = raw_spec.get("package", dependency_key)
            if declared_package != independent_name:
                errors.append(
                    f"Cargo.toml workspace dependency {dependency_key} resolves to "
                    f"{independent_name}, not {declared_package}"
                )
            version = raw_spec.get("version")
            observations.append(
                VersionObservation(
                    f"Cargo workspace independent dependency {dependency_key}",
                    ROOT_MANIFEST,
                    independent_version,
                    version if isinstance(version, str) else "<missing>",
                )
            )
            previous_key = projected_independent.get(independent_name)
            if previous_key is not None:
                errors.append(
                    "Cargo.toml declares duplicate local independent projections for "
                    f"{independent_name}: {previous_key} and {dependency_key}"
                )
            projected_independent[independent_name] = dependency_key
            continue
        if package_name is None:
            continue
        declared_package = raw_spec.get("package", dependency_key)
        if declared_package != package_name:
            errors.append(
                f"Cargo.toml workspace dependency {dependency_key} resolves to {package_name}, "
                f"not {declared_package}"
            )
        version = raw_spec.get("version")
        actual = version if isinstance(version, str) else "<missing>"
        observations.append(
            VersionObservation(
                f"Cargo workspace dependency {dependency_key}",
                ROOT_MANIFEST,
                catalog.authority.canonical,
                actual,
            )
        )
        previous_key = inherited_by_package.get(package_name)
        if previous_key is not None:
            errors.append(
                f"Cargo.toml declares duplicate local workspace projections for {package_name}: "
                f"{previous_key} and {dependency_key}"
            )
        inherited_by_package[package_name] = dependency_key

    catalog_dependencies = set(inherited_by_package)
    for manifest_path in catalog.member_manifests:
        manifest = view.toml(manifest_path)
        for _kind, dependency_key, spec in _dependency_specs(manifest):
            if not isinstance(spec, dict) or spec.get("workspace") is not True:
                continue
            root_spec = dependencies.get(dependency_key)
            if not isinstance(root_spec, dict):
                continue
            target_name = root_spec.get("package", dependency_key)
            if target_name in catalog.coupled_packages and target_name not in catalog_dependencies:
                errors.append(
                    f"{manifest_path} inherits {dependency_key}, but its root workspace dependency "
                    "is not a versioned local path projection"
                )


def _dependency_specs(
    manifest: Mapping[str, Any],
) -> list[tuple[str, str, Any]]:
    result: list[tuple[str, str, Any]] = []
    for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = manifest.get(kind, {})
        if isinstance(table, dict):
            result.extend((kind, key, spec) for key, spec in table.items())
    targets = manifest.get("target", {})
    if isinstance(targets, dict):
        for target_name, target in targets.items():
            if not isinstance(target, dict):
                continue
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                table = target.get(kind, {})
                if isinstance(table, dict):
                    result.extend(
                        (f"target.{target_name}.{kind}", key, spec)
                        for key, spec in table.items()
                    )
    return result


def _coupled_package_dirs(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
) -> dict[Path, str]:
    return {
        (view.root / member).resolve(): name
        for name, member in catalog.coupled_packages.items()
    }


def _collect_member_dependency_policy(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    errors: list[str],
) -> None:
    coupled_dirs = _coupled_package_dirs(view, catalog)
    for manifest_path in catalog.member_manifests:
        manifest = view.toml(manifest_path)
        package = _mapping(manifest.get("package"), f"{manifest_path} [package]")
        publish = package.get("publish", True)
        is_publishable = publish is not False and publish != []
        manifest_dir = (view.root / manifest_path).parent
        for kind, dependency_key, raw_spec in _dependency_specs(manifest):
            if not isinstance(raw_spec, dict):
                continue
            if raw_spec.get("workspace") is True:
                if "version" in raw_spec or "path" in raw_spec:
                    errors.append(
                        f"{manifest_path} {kind}.{dependency_key} mixes workspace inheritance "
                        "with a local version/path override"
                    )
                continue
            path_value = raw_spec.get("path")
            if not isinstance(path_value, str):
                continue
            target_name = coupled_dirs.get((manifest_dir / path_value).resolve())
            if target_name is None:
                continue
            if "version" in raw_spec:
                errors.append(
                    f"{manifest_path} {kind}.{dependency_key} duplicates the workspace release "
                    "version; inherit the root workspace dependency"
                )
            if is_publishable and not kind.endswith("dev-dependencies"):
                errors.append(
                    f"publishable {manifest_path} {kind}.{dependency_key} must inherit its "
                    "versioned root workspace dependency"
                )


def _collect_cargo_lock_versions(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    lock_path: Path,
    required_packages: set[str],
    observations: list[VersionObservation],
    errors: list[str],
    *,
    label: str | None = None,
) -> None:
    lock = view.toml(lock_path)
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise ReleaseProjectionError(f"{lock_path} package must be an array of tables")
    local_packages = {
        package.get("name")
        for package in packages
        if isinstance(package, dict)
        and package.get("name") in catalog.coupled_packages
        and "source" not in package
    }
    for package_name in sorted(local_packages | required_packages):
        matches = [
            package
            for package in packages
            if isinstance(package, dict)
            and package.get("name") == package_name
            and "source" not in package
        ]
        if len(matches) != 1:
            errors.append(
                f"{lock_path} must contain exactly one local package entry for {package_name}; "
                f"found {len(matches)}"
            )
            continue
        actual = matches[0].get("version")
        observations.append(
            VersionObservation(
                f"{label or lock_path} package {package_name}",
                lock_path,
                catalog.authority.canonical,
                actual if isinstance(actual, str) else "<missing>",
            )
        )


def _collect_independent_lock_versions(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    lock_path: Path,
    observations: list[VersionObservation],
    errors: list[str],
    *,
    label: str | None = None,
    require_all: bool = False,
) -> None:
    lock = view.toml(lock_path)
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise ReleaseProjectionError(f"{lock_path} package must be an array of tables")
    for package_name, (_member, expected_version) in sorted(
        catalog.independent_packages.items()
    ):
        matches = [
            package
            for package in packages
            if isinstance(package, dict)
            and package.get("name") == package_name
            and "source" not in package
        ]
        if not matches and not require_all:
            continue
        if len(matches) != 1:
            errors.append(
                f"{lock_path} must contain exactly one local independent package entry for "
                f"{package_name}; found {len(matches)}"
            )
            continue
        actual = matches[0].get("version")
        observations.append(
            VersionObservation(
                f"{label or lock_path} independent package {package_name}",
                lock_path,
                expected_version,
                actual if isinstance(actual, str) else "<missing>",
            )
        )


def _collect_fuzz_lock_versions(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    observations: list[VersionObservation],
    errors: list[str],
) -> None:
    manifest = view.toml(FUZZ_MANIFEST)
    coupled_dirs = _coupled_package_dirs(view, catalog)
    manifest_dir = (view.root / FUZZ_MANIFEST).parent
    required: set[str] = set()
    for _kind, _dependency_key, raw_spec in _dependency_specs(manifest):
        if not isinstance(raw_spec, dict) or not isinstance(raw_spec.get("path"), str):
            continue
        package_name = coupled_dirs.get((manifest_dir / raw_spec["path"]).resolve())
        if package_name is not None:
            required.add(package_name)
    if not required:
        raise ReleaseProjectionError(
            "fuzz/Cargo.toml must retain at least one path dependency on a workspace-coupled crate"
        )
    _collect_cargo_lock_versions(
        view,
        catalog,
        FUZZ_LOCK,
        required,
        observations,
        errors,
    )


def _web_package_entries(view: RepositoryView) -> list[tuple[dict[str, Any], Path]]:
    try:
        descriptor = web_package_group.validate_descriptor(view.json(WEB_DESCRIPTOR))
    except web_package_group.PackageGroupError as exc:
        raise ReleaseProjectionError(f"{WEB_DESCRIPTOR}: {exc}") from exc
    entries: list[tuple[dict[str, Any], Path]] = []
    for entry in descriptor["packages"]:
        entries.append(
            (
                entry,
                WEB_DESCRIPTOR.parent / web_package_group.descriptor_package_path(entry),
            )
        )
    return entries


def _node_package_entry(
    raw_entry: Any,
    label: str,
) -> _NodePackageEntry:
    entry = _mapping(raw_entry, label)
    name = _string(entry.get("name"), f"{label}.name")
    directory = Path(_string(entry.get("directory"), f"{label}.directory"))
    if directory.is_absolute() or ".." in directory.parts or directory == Path("."):
        raise ReleaseProjectionError(
            f"{label}.directory must stay inside {NODE_ROOT}: {directory}"
        )
    return _NodePackageEntry(
        name=name,
        directory=directory,
    )


def _node_package_catalog(view: RepositoryView) -> _NodePackageCatalog:
    descriptor = view.json(NODE_DESCRIPTOR)
    version = _string(descriptor.get("version"), f"{NODE_DESCRIPTOR}.version")
    try:
        parse_release_version(
            version,
            allow_v_prefix=False,
        )
    except ValueError as exc:
        raise ReleaseProjectionError(
            f"{NODE_DESCRIPTOR}.version is invalid: {exc}"
        ) from exc

    root = _node_package_entry(
        descriptor.get("root"),
        f"{NODE_DESCRIPTOR}.root",
    )
    raw_targets = descriptor.get("targets")
    if not isinstance(raw_targets, list):
        raise ReleaseProjectionError(
            f"{NODE_DESCRIPTOR}.targets must be an array"
        )
    targets = tuple(
        _node_package_entry(
            raw_target,
            f"{NODE_DESCRIPTOR}.targets[{index}]",
        )
        for index, raw_target in enumerate(raw_targets)
    )
    return _NodePackageCatalog(version=version, root=root, targets=targets)


def _collect_node_candidate_projection(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    observations: list[VersionObservation],
    errors: list[str],
) -> None:
    canonical = catalog.authority.canonical
    node_catalog = _node_package_catalog(view)

    def observe(label: str, path: Path, actual: Any) -> None:
        _observe(observations, label, path, canonical, actual)

    cargo_manifest = view.toml(NODE_CARGO_MANIFEST)
    cargo_package = _mapping(
        cargo_manifest.get("package"),
        f"{NODE_CARGO_MANIFEST} [package]",
    )
    if cargo_package.get("name") != NODE_CARGO_PACKAGE:
        raise ReleaseProjectionError(
            f"{NODE_CARGO_MANIFEST} must define {NODE_CARGO_PACKAGE}"
        )
    if cargo_package.get("publish") is not False:
        raise ReleaseProjectionError(
            f"{NODE_CARGO_MANIFEST} must remain a private, non-publishable candidate"
        )
    if not isinstance(cargo_manifest.get("workspace"), dict):
        raise ReleaseProjectionError(
            f"{NODE_CARGO_MANIFEST} must remain a detached private workspace "
            "until Node admission"
        )
    observe("Node candidate Cargo package", NODE_CARGO_MANIFEST, cargo_package.get("version"))

    dependencies = _mapping(
        cargo_manifest.get("dependencies"),
        f"{NODE_CARGO_MANIFEST} [dependencies]",
    )
    bindings_dependency = _mapping(
        dependencies.get(NODE_BINDINGS_PACKAGE),
        f"{NODE_CARGO_MANIFEST} dependency {NODE_BINDINGS_PACKAGE}",
    )
    bindings_path = Path(
        _string(
            bindings_dependency.get("path"),
            f"{NODE_CARGO_MANIFEST} dependency {NODE_BINDINGS_PACKAGE}.path",
        )
    )
    bindings_member = catalog.coupled_packages.get(NODE_BINDINGS_PACKAGE)
    if bindings_member is None:
        raise ReleaseProjectionError(
            f"root workspace must contain the coupled {NODE_BINDINGS_PACKAGE} package"
        )
    expected_bindings_dir = (view.root / bindings_member).resolve()
    actual_bindings_dir = (
        (view.root / NODE_CARGO_MANIFEST).parent / bindings_path
    ).resolve()
    if actual_bindings_dir != expected_bindings_dir:
        errors.append(
            f"{NODE_CARGO_MANIFEST} must depend on the local "
            f"{NODE_BINDINGS_PACKAGE} package"
        )

    _collect_cargo_lock_versions(
        view,
        catalog,
        NODE_CARGO_LOCK,
        {NODE_CARGO_PACKAGE, NODE_BINDINGS_PACKAGE},
        observations,
        errors,
        label="Node candidate Cargo.lock",
    )
    _collect_independent_lock_versions(
        view,
        catalog,
        NODE_CARGO_LOCK,
        observations,
        errors,
        label="Node candidate Cargo.lock",
    )
    observe("Node candidate package surface", NODE_DESCRIPTOR, node_catalog.version)

    workspace_manifest = view.json(NODE_WORKSPACE_PACKAGE)
    if (
        workspace_manifest.get("name") != "merman-node-candidate-workspace"
        or workspace_manifest.get("private") is not True
    ):
        raise ReleaseProjectionError(
            f"{NODE_WORKSPACE_PACKAGE} must remain the private Node candidate workspace"
        )
    observe("Node candidate workspace", NODE_WORKSPACE_PACKAGE, workspace_manifest.get("version"))

    workspace_lock = view.json(NODE_WORKSPACE_LOCK)
    lock_packages = _mapping(
        workspace_lock.get("packages"),
        f"{NODE_WORKSPACE_LOCK}.packages",
    )
    lock_root = _mapping(
        lock_packages.get(""),
        f"{NODE_WORKSPACE_LOCK} root package",
    )
    if (
        workspace_lock.get("name") != workspace_manifest.get("name")
        or lock_root.get("name") != workspace_manifest.get("name")
    ):
        errors.append(
            f"{NODE_WORKSPACE_LOCK} root package must match "
            f"{NODE_WORKSPACE_PACKAGE}"
        )
    observe("Node candidate workspace lock", NODE_WORKSPACE_LOCK, workspace_lock.get("version"))
    observe(
        "Node candidate workspace lock package",
        NODE_WORKSPACE_LOCK,
        lock_root.get("version"),
    )

    for entry in node_catalog.entries:
        manifest = view.json(entry.manifest_path)
        observe(f"Node candidate package {entry.name}", entry.manifest_path, manifest.get("version"))


def _playground_web_dependencies(
    view: RepositoryView,
    entries: list[tuple[dict[str, Any], Path]],
) -> list[tuple[dict[str, Any], Path]]:
    playground = view.json(PLAYGROUND_PACKAGE)
    dependencies = _mapping(playground.get("dependencies"), "Playground dependencies")
    by_name = {entry["name"]: (entry, package_dir) for entry, package_dir in entries}
    default_entry = next(entry for entry, _package_dir in entries if entry["id"] == view.json(WEB_DESCRIPTOR)["default_package"])
    if default_entry["name"] not in dependencies:
        raise ReleaseProjectionError(
            f"playground must consume the default Web package {default_entry['name']}"
        )

    selected: list[tuple[dict[str, Any], Path]] = []
    for name, dependency in dependencies.items():
        if not isinstance(name, str) or not name.startswith("@mermanjs/web"):
            continue
        item = by_name.get(name)
        if item is None:
            raise ReleaseProjectionError(f"playground depends on undeclared Web package {name}")
        entry, package_dir = item
        if entry["visibility"] != "public":
            raise ReleaseProjectionError(f"playground must not consume candidate Web package {name}")
        expected = f"file:../{package_dir.as_posix()}"
        if dependency != expected:
            raise ReleaseProjectionError(
                f"playground must consume {name} from {expected}, found {dependency!r}"
            )
        selected.append(item)
    if not selected:
        raise ReleaseProjectionError("playground must consume at least one public Web package")
    return selected


def _collect_platform_versions(
    view: RepositoryView,
    release: ReleaseVersion,
    observations: list[VersionObservation],
) -> None:
    canonical = release.canonical
    pep440 = release.to_pep440()

    def observe(label: str, path: Path, actual: Any, expected: str = canonical) -> None:
        _observe(observations, label, path, expected, actual)

    web_workspace = view.json(WEB_WORKSPACE_PACKAGE)
    if web_workspace.get("private") is not True:
        raise ReleaseProjectionError("platforms/web/package.json must be a private workspace owner")
    observe("Web workspace", WEB_WORKSPACE_PACKAGE, web_workspace.get("version"))
    web_entries = _web_package_entries(view)
    for entry, package_dir in web_entries:
        manifest_path = package_dir / "package.json"
        manifest = view.json(manifest_path)
        observe(f"Web package {entry['name']}", manifest_path, manifest.get("version"))
    web_lock = view.json(WEB_LOCK)
    web_lock_packages = _mapping(web_lock.get("packages"), "Web lock packages")
    web_lock_workspace = _mapping(
        web_lock_packages.get(""), "Web lock workspace package"
    )
    observe("Web workspace lock", WEB_LOCK, web_lock.get("version"))
    observe("Web workspace lock package", WEB_LOCK, web_lock_workspace.get("version"))
    for entry, package_dir in web_entries:
        lock_key = package_dir.relative_to(WEB_DESCRIPTOR.parent).as_posix()
        local_package = _mapping(
            web_lock_packages.get(lock_key),
            f"Web lock package {lock_key}",
        )
        observe(f"Web lock package {entry['name']}", WEB_LOCK, local_package.get("version"))

    playground = view.json(PLAYGROUND_PACKAGE)
    if playground.get("private") is not True:
        raise ReleaseProjectionError("playground/package.json must be private")
    observe("Playground application", PLAYGROUND_PACKAGE, playground.get("version"))
    playground_web_packages = _playground_web_dependencies(view, web_entries)
    playground_lock = view.json(PLAYGROUND_LOCK)
    playground_packages = _mapping(
        playground_lock.get("packages"), "Playground lock packages"
    )
    playground_lock_package = _mapping(
        playground_packages.get(""), "Playground lock root package"
    )
    observe("Playground application lock", PLAYGROUND_LOCK, playground_lock.get("version"))
    observe(
        "Playground application lock package",
        PLAYGROUND_LOCK,
        playground_lock_package.get("version"),
    )
    playground_web_workspace = _mapping(
        playground_packages.get("../platforms/web"),
        "Playground lock local Web workspace",
    )
    observe(
        "Playground local Web workspace lock",
        PLAYGROUND_LOCK,
        playground_web_workspace.get("version"),
    )
    for entry, package_dir in playground_web_packages:
        lock_key = f"../{package_dir.as_posix()}"
        local_package = _mapping(
            playground_packages.get(lock_key),
            f"Playground lock local {entry['name']} package",
        )
        observe(
            f"Playground local Web lock {entry['name']}",
            PLAYGROUND_LOCK,
            local_package.get("version"),
        )
    license_report = view.text(PLAYGROUND_LICENSE_REPORT)
    lock_digest = hashlib.sha256(view.text(PLAYGROUND_LOCK).encode("utf-8")).hexdigest()
    _observe_text_match(
        observations,
        "Playground license lock digest",
        PLAYGROUND_LICENSE_REPORT,
        license_report,
        r"^package-lock\.json SHA-256: ([0-9a-f]{64})$",
        lock_digest,
    )
    observe(
        "Python package",
        PYTHON_MANIFEST,
        _mapping(view.toml(PYTHON_MANIFEST).get("project"), "Python [project]").get("version"),
        pep440,
    )
    assignments = (
        ("Android package", ANDROID_MANIFEST, r'^version\s*=\s*"([^"]+)"\s*$'),
        ("Flutter package", FLUTTER_MANIFEST, r"^version:\s*([^\s#]+)\s*$"),
        (
            "Flutter bundled native package version",
            FLUTTER_PACKAGE_VERSION,
            r"^const String mermanPackageVersion = '([^']+)';\s*$",
        ),
    )
    for label, path, pattern in assignments:
        _observe_assignment(view, observations, label, path, pattern, canonical)


def _observe(
    observations: list[VersionObservation],
    label: str,
    path: Path,
    expected: str,
    actual: Any,
) -> None:
    observations.append(
        VersionObservation(
            label,
            path,
            expected,
            actual if isinstance(actual, str) else "<missing>",
        )
    )


def _observe_assignment(
    view: RepositoryView,
    observations: list[VersionObservation],
    label: str,
    path: Path,
    pattern: str,
    expected: str,
) -> None:
    matches = re.findall(pattern, view.text(path), flags=re.MULTILINE)
    if len(matches) != 1:
        raise ReleaseProjectionError(
            f"{path} must contain exactly one {label} version assignment; found {len(matches)}"
        )
    _observe(observations, label, path, expected, matches[0])


def _observe_text_match(
    observations: list[VersionObservation],
    label: str,
    path: Path,
    text: str,
    pattern: str,
    expected: str,
) -> None:
    matches = re.findall(pattern, text, flags=re.MULTILINE)
    if len(matches) != 1:
        raise ReleaseProjectionError(
            f"{path} must contain exactly one {label}; found {len(matches)}"
        )
    _observe(observations, label, path, expected, matches[0])


def apply_version_update(root: Path, version: str) -> tuple[Path, ...]:
    release = parse_release_version(version, allow_v_prefix=False)
    root = root.resolve()
    preimage = _capture_release_preimage(root)
    changed_paths, patch = _prepare_release_patch(root, release, preimage)
    if patch:
        _apply_release_patch(root, patch, preimage)
    return changed_paths


def _run_command(
    args: Sequence[str],
    *,
    cwd: Path,
    input_bytes: bytes | None = None,
) -> bytes:
    command = [str(item) for item in args]
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            input=input_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        ).stdout
    except subprocess.CalledProcessError as exc:
        detail = exc.stderr.decode("utf-8", errors="replace").strip()
        raise ReleaseProjectionError(
            f"release owner command failed ({exc.returncode}): "
            f"{' '.join(command)}{': ' + detail if detail else ''}"
        ) from exc
    except OSError as exc:
        raise ReleaseProjectionError(
            f"cannot run release owner command {command[0]}: {exc}"
        ) from exc


def _run_git(root: Path, *args: str, input_bytes: bytes | None = None) -> bytes:
    return _run_command(
        ["git", "-C", str(root), *args], cwd=root, input_bytes=input_bytes
    )


def _capture_release_preimage(root: Path) -> _ReleasePreimage:
    root = root.resolve()
    metadata = os.fsdecode(_run_git(
        root,
        "rev-parse",
        "--show-toplevel",
        "--path-format=absolute",
        "--git-dir",
        "--git-common-dir",
        "HEAD",
    )).splitlines()
    if len(metadata) != 4:
        raise ReleaseProjectionError("Git did not report the release worktree preimage")
    top_level, git_directory, common_directory, head = metadata
    top_level = Path(top_level).resolve()
    if top_level != root:
        raise ReleaseProjectionError(
            f"release version set must run at the Git worktree root: {top_level}"
        )
    if Path(git_directory).resolve() == Path(common_directory).resolve():
        raise ReleaseProjectionError(
            "release version set requires a linked release worktree, not the primary checkout"
        )

    if _run_git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all"):
        raise ReleaseProjectionError(
            "release version set requires a clean release worktree, including no untracked files"
        )

    index = _run_git(root, "ls-files", "--stage", "-z")
    index_digest = hashlib.sha256(head.encode("ascii") + b"\0" + index).hexdigest()
    return _ReleasePreimage(head=head, index_digest=index_digest)


def _prepare_release_patch(
    root: Path,
    release: ReleaseVersion,
    preimage: _ReleasePreimage,
) -> tuple[tuple[Path, ...], bytes]:
    with tempfile.TemporaryDirectory(prefix="merman-release-version-") as temp_dir:
        prepared_root = Path(temp_dir) / "worktree"
        _run_git(
            root,
            "worktree",
            "add",
            "--detach",
            "--quiet",
            str(prepared_root),
            preimage.head,
        )
        try:
            _prepare_cargo_versions(prepared_root, release)
            _prepare_npm_versions(prepared_root, release)
            try:
                release_version_owners.prepare_python_version(prepared_root, release)
                release_version_owners.prepare_android_version(prepared_root, release)
                release_version_owners.prepare_flutter_version(prepared_root, release)
            except release_version_owners.ReleaseOwnerError as exc:
                raise ReleaseProjectionError(str(exc)) from exc

            verification = verify_repository(
                prepared_root,
                expected_version=release.canonical,
            )
            if not verification.ok:
                raise ReleaseProjectionError(
                    "prepared release projection did not verify:\n"
                    + "\n".join(format_verification_failures(verification))
                )

            status = [
                os.fsdecode(item)
                for item in _run_git(
                    prepared_root, "status", "--porcelain=v1", "-z", "--untracked-files=all"
                ).split(b"\0")
                if item
            ]
            if any(item.startswith("?? ") for item in status):
                raise ReleaseProjectionError("release owners created untracked files")
            if any(item[:1] in {"R", "C"} or item[1:2] in {"R", "C"} for item in status):
                raise ReleaseProjectionError("release owners must not rename projection files")
            changed_paths = tuple(sorted(Path(item[3:]) for item in status))
            expected_paths = frozenset(item.path for item in verification.observations)
            unrelated = set(changed_paths) - expected_paths
            if unrelated:
                raise ReleaseProjectionError(
                    "release owners changed paths outside their projection: "
                    + ", ".join(str(path) for path in sorted(unrelated))
                )

            patch = _run_git(
                prepared_root,
                "diff",
                "--binary",
                "--full-index",
                "HEAD",
                "--",
            )
            return changed_paths, patch
        finally:
            _run_git(root, "worktree", "remove", "--force", str(prepared_root))


def _apply_release_patch(
    root: Path,
    patch: bytes,
    preimage: _ReleasePreimage,
) -> None:
    for check in (True, False):
        try:
            actual_preimage = _capture_release_preimage(root)
        except ReleaseProjectionError as exc:
            raise ReleaseProjectionError(
                "release source preimage changed while preparing the patch; no patch was applied"
            ) from exc
        if actual_preimage != preimage:
            raise ReleaseProjectionError(
                "release source preimage changed while preparing the patch; no patch was applied"
            )
        args = ["apply"]
        if check:
            args.append("--check")
        args.extend(["--binary", "--whitespace=nowarn", "-"])
        try:
            _run_git(root, *args, input_bytes=patch)
        except ReleaseProjectionError as exc:
            action = "validate" if check else "apply"
            raise ReleaseProjectionError(
                f"cannot {action} release patch: {exc}"
            ) from exc


def _require_exact_tool_version(
    tool: str,
    expected: str,
    *,
    cwd: Path | None = None,
) -> str:
    executable = shutil.which(tool)
    if executable is None:
        raise ReleaseProjectionError(f"required pinned tool {tool} {expected} is not installed")
    output = _run_command([executable, "--version"], cwd=cwd or Path.cwd()).decode(
        "utf-8", errors="replace"
    ).strip()
    match = re.match(r"^cargo\s+([^\s]+)", output) if tool == "cargo" else None
    actual = match.group(1) if match else output.splitlines()[0] if output else ""
    if actual != expected:
        raise ReleaseProjectionError(
            f"required pinned tool {tool} {expected}, found {actual or '<unknown>'}"
        )
    return executable


def _write_relative(root: Path, path: Path, content: str) -> None:
    target = root / path
    if target.read_text(encoding="utf-8") != content:
        target.write_text(content, encoding="utf-8")


def _assert_cargo_lock_dependency_state(
    lock_path: Path,
    before: str,
    after: str,
    local_packages: frozenset[str],
) -> None:
    def normalized(text: str) -> Mapping[str, Any]:
        try:
            data = tomllib.loads(text)
        except tomllib.TOMLDecodeError as exc:
            raise ReleaseProjectionError(f"invalid {lock_path}: {exc}") from exc
        packages = data.get("package")
        if not isinstance(packages, list):
            raise ReleaseProjectionError(f"{lock_path} package must be an array")
        for package in packages:
            if not isinstance(package, dict):
                continue
            if package.get("name") in local_packages and "source" not in package:
                package["version"] = "<workspace-release>"
        return data

    if normalized(before) != normalized(after):
        raise ReleaseProjectionError(
            f"{lock_path} has unrelated dependency drift after Cargo version preparation"
        )


def _prepare_cargo_versions(root: Path, release: ReleaseVersion) -> None:
    view = RepositoryView(root)
    catalog = load_workspace_catalog(view)
    toolchain = _mapping(view.toml(RUST_TOOLCHAIN).get("toolchain"), "Rust toolchain")
    cargo = _require_exact_tool_version(
        "cargo", _string(toolchain.get("channel"), "Rust toolchain channel"), cwd=root
    )

    coupled = frozenset(catalog.coupled_packages)
    lock_owners = {
        ROOT_LOCK: coupled,
        FUZZ_LOCK: coupled,
        NODE_CARGO_LOCK: coupled | {NODE_CARGO_PACKAGE},
    }
    before_locks = {path: view.text(path) for path in lock_owners}

    root_text = _replace_toml_section_string(
        view.text(ROOT_MANIFEST),
        "workspace.package",
        "version",
        release.canonical,
    )
    dependencies = _mapping(
        _mapping(catalog.root_data["workspace"], "Cargo.toml [workspace]").get(
            "dependencies"
        ),
        "Cargo.toml [workspace.dependencies]",
    )
    coupled_dirs = _coupled_package_dirs(view, catalog)
    for dependency_key, spec in dependencies.items():
        if not isinstance(spec, dict) or not isinstance(spec.get("path"), str):
            continue
        if (root / spec["path"]).resolve() in coupled_dirs:
            root_text = _replace_toml_inline_string(
                root_text,
                "workspace.dependencies",
                dependency_key,
                "version",
                release.canonical,
            )
    _write_relative(root, ROOT_MANIFEST, root_text)
    _write_relative(
        root,
        NODE_CARGO_MANIFEST,
        _replace_toml_section_string(
            view.text(NODE_CARGO_MANIFEST),
            "package",
            "version",
            release.canonical,
        ),
    )

    for manifest in (ROOT_MANIFEST, FUZZ_MANIFEST, NODE_CARGO_MANIFEST):
        _run_command(
            [
                cargo,
                "update",
                "--offline",
                "--workspace",
                "--manifest-path",
                str(root / manifest),
            ],
            cwd=root,
        )

    for lock_path, local_packages in lock_owners.items():
        _assert_cargo_lock_dependency_state(
            lock_path,
            before_locks[lock_path],
            (root / lock_path).read_text(encoding="utf-8"),
            local_packages,
        )


def _load_json_object(text: str, label: str) -> dict[str, Any]:
    try:
        data = json.loads(text, object_pairs_hook=_reject_duplicate_keys)
    except json.JSONDecodeError as exc:
        raise ReleaseProjectionError(f"invalid JSON in {label}: {exc}") from exc
    if not isinstance(data, dict):
        raise ReleaseProjectionError(f"{label} must contain a JSON object")
    return data


def _assert_npm_lock_dependency_state(
    path: Path,
    before: str,
    after: str,
    *,
    local_package_keys: set[str] | frozenset[str],
) -> None:
    keys = frozenset(local_package_keys)

    def normalized(text: str) -> Mapping[str, Any]:
        data = _load_json_object(text, str(path))
        data["version"] = "<workspace-release>"
        packages = _mapping(data.get("packages"), f"{path}.packages")
        for package_key in keys:
            package = _mapping(packages.get(package_key), f"{path}:{package_key}")
            if "version" in package:
                package["version"] = "<workspace-release>"
        return data

    if normalized(before) != normalized(after):
        raise ReleaseProjectionError(
            f"{path} has unrelated dependency drift after npm version preparation"
        )


def _prepare_npm_versions(root: Path, release: ReleaseVersion) -> None:
    view = RepositoryView(root)
    package_manager = view.json(PLAYGROUND_PACKAGE).get("packageManager")
    npm_version_match = (
        re.fullmatch(r"npm@([0-9]+\.[0-9]+\.[0-9]+)", package_manager)
        if isinstance(package_manager, str)
        else None
    )
    if npm_version_match is None:
        raise ReleaseProjectionError(
            f"{PLAYGROUND_PACKAGE} packageManager must pin npm as npm@x.y.z"
        )
    npm = _require_exact_tool_version("npm", npm_version_match.group(1), cwd=root)

    def run(package_root: Path, *args: str) -> None:
        _run_command([npm, *args], cwd=root / package_root)

    def write_json(path: Path, data: Mapping[str, Any]) -> None:
        _write_relative(root, path, json.dumps(data, ensure_ascii=False, indent=2) + "\n")

    web_entries = _web_package_entries(view)
    node_catalog = _node_package_catalog(view)
    playground_web_packages = _playground_web_dependencies(view, web_entries)
    before_locks = {
        path: view.text(path) for path in (WEB_LOCK, NODE_WORKSPACE_LOCK, PLAYGROUND_LOCK)
    }

    version_args = (
        "version",
        release.canonical,
        "--allow-same-version",
        "--no-git-tag-version",
        "--ignore-scripts",
    )
    workspace_args = (*version_args, "--workspaces", "--include-workspace-root")
    run(WEB_DESCRIPTOR.parent, *workspace_args)
    run(NODE_ROOT, *version_args)
    for entry in node_catalog.entries:
        run(NODE_ROOT / entry.directory, *version_args)
    descriptor = _load_json_object(view.text(NODE_DESCRIPTOR), str(NODE_DESCRIPTOR))
    descriptor["version"] = release.canonical
    write_json(NODE_DESCRIPTOR, descriptor)
    loader_path = node_catalog.root.manifest_path
    loader = _load_json_object((root / loader_path).read_text(encoding="utf-8"), str(loader_path))
    optional_dependencies = _mapping(
        loader.get("optionalDependencies"), f"{loader_path}.optionalDependencies"
    )
    for target in node_catalog.targets:
        if target.name not in optional_dependencies:
            raise ReleaseProjectionError(f"{loader_path} is missing {target.name}")
        optional_dependencies[target.name] = release.canonical
    write_json(loader_path, loader)
    run(PLAYGROUND_PACKAGE.parent, *version_args)
    lock_args = (
        "install",
        "--package-lock-only",
        "--ignore-scripts",
        "--no-audit",
        "--no-fund",
        f"--registry={NPM_REGISTRY}",
    )
    for package_root in (
        WEB_DESCRIPTOR.parent,
        NODE_ROOT,
        PLAYGROUND_PACKAGE.parent,
    ):
        run(package_root, *lock_args)

    playground_local_keys = {
        "../platforms/web",
        *(f"../{package_dir.as_posix()}" for _entry, package_dir in playground_web_packages),
    }
    playground_lock = _load_json_object(
        (root / PLAYGROUND_LOCK).read_text(encoding="utf-8"), str(PLAYGROUND_LOCK)
    )
    playground_packages = _mapping(playground_lock.get("packages"), "Playground lock packages")
    for key in playground_local_keys:
        _mapping(playground_packages.get(key), f"Playground lock package {key}")["version"] = release.canonical
    playground_lock_text = json.dumps(
        playground_lock, ensure_ascii=False, indent=2
    ) + "\n"
    _write_relative(root, PLAYGROUND_LOCK, playground_lock_text)

    lock_owners = (
        (
            WEB_LOCK,
            {""}
            | {
                package_dir.relative_to(WEB_DESCRIPTOR.parent).as_posix()
                for _entry, package_dir in web_entries
            },
        ),
        (NODE_WORKSPACE_LOCK, {""}),
        (PLAYGROUND_LOCK, {""} | playground_local_keys),
    )
    for lock_path, local_keys in lock_owners:
        _assert_npm_lock_dependency_state(
            lock_path,
            before_locks[lock_path],
            playground_lock_text
            if lock_path == PLAYGROUND_LOCK
            else (root / lock_path).read_text(encoding="utf-8"),
            local_package_keys=local_keys,
        )

    _write_relative(
        root,
        PLAYGROUND_LICENSE_REPORT,
        _replace_one(
            (root / PLAYGROUND_LICENSE_REPORT).read_text(encoding="utf-8"),
            r"^(package-lock\.json SHA-256: )[0-9a-f]{64}$",
            rf"\g<1>{hashlib.sha256(playground_lock_text.encode('utf-8')).hexdigest()}",
            PLAYGROUND_LICENSE_REPORT,
            "package-lock.json digest",
        ),
    )


def _replace_toml_section_string(
    text: str,
    section: str,
    key: str,
    value: str,
) -> str:
    lines = text.splitlines(keepends=True)
    section_header = f"[{section}]"
    in_section = False
    matches: list[int] = []
    assignment = re.compile(rf"^(\s*{re.escape(key)}\s*=\s*)\"[^\"]*\"(\s*(?:#.*)?(?:\r?\n)?)$")
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_section = stripped == section_header
            continue
        if in_section and assignment.match(line):
            matches.append(index)
    if len(matches) != 1:
        raise ReleaseProjectionError(
            f"expected one {section}.{key} string assignment; found {len(matches)}"
        )
    index = matches[0]
    lines[index] = assignment.sub(rf'\g<1>"{value}"\g<2>', lines[index])
    candidate = "".join(lines)
    try:
        parsed = tomllib.loads(candidate)
        actual: Any = parsed
        for component in (*section.split("."), key):
            actual = actual[component]
    except (KeyError, TypeError, tomllib.TOMLDecodeError) as exc:
        raise ReleaseProjectionError(
            f"failed to structurally update {section}.{key}: {exc}"
        ) from exc
    if actual != value:
        raise ReleaseProjectionError(f"failed to update {section}.{key}")
    return candidate


def _replace_toml_inline_string(
    text: str,
    section: str,
    key: str,
    field: str,
    value: str,
) -> str:
    lines = text.splitlines(keepends=True)
    in_section = False
    section_header = f"[{section}]"
    key_pattern = re.compile(rf"^\s*{re.escape(key)}\s*=")
    indexes: list[int] = []
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_section = stripped == section_header
            continue
        if in_section and key_pattern.match(line):
            indexes.append(index)
    if len(indexes) != 1:
        raise ReleaseProjectionError(
            f"expected one {section}.{key} assignment; found {len(indexes)}"
        )
    index = indexes[0]
    field_pattern = re.compile(rf"(\b{re.escape(field)}\s*=\s*)\"[^\"]*\"")
    replacement, count = field_pattern.subn(rf'\g<1>"{value}"', lines[index])
    if count != 1:
        raise ReleaseProjectionError(
            f"expected one {field} field in {section}.{key}; found {count}"
        )
    lines[index] = replacement
    candidate = "".join(lines)
    try:
        actual = tomllib.loads(candidate)[section.split(".")[0]][section.split(".")[1]][key][field]
    except (KeyError, TypeError, tomllib.TOMLDecodeError) as exc:
        raise ReleaseProjectionError(
            f"failed to structurally update {section}.{key}.{field}: {exc}"
        ) from exc
    if actual != value:
        raise ReleaseProjectionError(f"failed to update {section}.{key}.{field}")
    return candidate


def _replace_one(
    text: str,
    pattern: str,
    replacement: str,
    path: Path,
    label: str,
    *,
    flags: int = re.MULTILINE,
) -> str:
    candidate, count = re.subn(pattern, replacement, text, flags=flags)
    if count != 1:
        raise ReleaseProjectionError(
            f"{path} must contain exactly one {label}; found {count}"
        )
    return candidate


def format_verification_failures(result: VerificationResult) -> list[str]:
    failures = list(result.errors)
    failures.extend(
        f"{item.label} ({item.path}): {item.actual!r} != {item.expected!r}"
        for item in result.observations
        if not item.matches
    )
    return failures
