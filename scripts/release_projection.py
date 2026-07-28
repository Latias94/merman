"""Workspace release-version authority, projections, and updates."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import stat
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

try:
    from scripts.release_version import ReleaseVersion, parse_release_version
except ModuleNotFoundError:
    from release_version import ReleaseVersion, parse_release_version

try:
    from scripts import web_package_group
except ModuleNotFoundError:
    import web_package_group

try:
    from scripts import release_readme
except ModuleNotFoundError:
    import release_readme


ROOT_MANIFEST = Path("Cargo.toml")
ROOT_LOCK = Path("Cargo.lock")
README = Path("README.md")
PROJECTED_READMES = tuple(
    Path(path) for path in release_readme.projected_readme_paths()
)
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
PYTHON_MANIFEST = Path("platforms/python/merman/pyproject.toml")
ANDROID_MANIFEST = Path("platforms/android/build.gradle.kts")
FLUTTER_MANIFEST = Path("platforms/flutter/pubspec.yaml")
FLUTTER_ANDROID_MANIFEST = Path("platforms/flutter/android/build.gradle")
FLUTTER_IOS_PODSPEC = Path("platforms/flutter/ios/merman.podspec")
FLUTTER_MACOS_PODSPEC = Path("platforms/flutter/macos/merman.podspec")
FLUTTER_IOS_BUILD = Path("platforms/flutter/build-ios.sh")
FLUTTER_PACKAGE_VERSION = Path(
    "platforms/flutter/lib/src/generated/package_version.dart"
)


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
    repository_url: str
    readme_registry_version: str | None
    coupled_packages: Mapping[str, Path]
    member_manifests: tuple[Path, ...]
    root_data: Mapping[str, Any]


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


def _mapping(value: Any, label: str) -> Mapping[str, Any]:
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
    repository_url = _string(
        package.get("repository"),
        "Cargo.toml workspace.package.repository",
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
    readme_registry_raw = release_metadata.get("readme-registry-version")
    readme_registry_version = None
    if readme_registry_raw is not None:
        readme_registry_version = parse_release_version(
            _string(
                readme_registry_raw,
                "Cargo.toml workspace.metadata.merman-release.readme-registry-version",
            ),
            allow_v_prefix=False,
        ).canonical
        if readme_registry_version != authority.canonical:
            raise ReleaseProjectionError(
                "Cargo.toml workspace.metadata.merman-release.readme-registry-version "
                f"must match workspace.package.version {authority.canonical}, "
                f"found {readme_registry_version}"
            )

    members = workspace.get("members")
    if not isinstance(members, list) or not members:
        raise ReleaseProjectionError("Cargo.toml workspace.members must be a non-empty array")

    coupled: dict[str, Path] = {}
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
            _string(version_source, f"{manifest_path} package.version")
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
        repository_url=repository_url,
        readme_registry_version=readme_registry_version,
        coupled_packages=coupled,
        member_manifests=tuple(member_manifests),
        root_data=root_data,
    )


def verify_repository(
    root: Path,
    *,
    expected_version: str | None = None,
    required_readme_mode: str | None = None,
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
    _collect_fuzz_lock_versions(view, catalog, observations, errors)
    _collect_node_candidate_projection(view, catalog, observations, errors)
    _collect_platform_versions(view, catalog.authority, observations)
    _collect_readme_projection(
        view,
        catalog,
        errors,
        required_mode=required_readme_mode,
    )

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
    inherited_by_package: dict[str, str] = {}

    for dependency_key, raw_spec in dependencies.items():
        if not isinstance(raw_spec, dict) or "path" not in raw_spec:
            continue
        dependency_path = Path(
            _string(raw_spec.get("path"), f"workspace dependency {dependency_key}.path")
        )
        resolved = (view.root / dependency_path).resolve()
        package_name = coupled_dirs.get(resolved)
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
    _observe(
        observations,
        "Node candidate Cargo package",
        NODE_CARGO_MANIFEST,
        canonical,
        cargo_package.get("version"),
    )

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
    _observe(
        observations,
        "Node candidate package surface",
        NODE_DESCRIPTOR,
        canonical,
        node_catalog.version,
    )

    workspace_manifest = view.json(NODE_WORKSPACE_PACKAGE)
    if (
        workspace_manifest.get("name") != "merman-node-candidate-workspace"
        or workspace_manifest.get("private") is not True
    ):
        raise ReleaseProjectionError(
            f"{NODE_WORKSPACE_PACKAGE} must remain the private Node candidate workspace"
        )
    _observe(
        observations,
        "Node candidate workspace",
        NODE_WORKSPACE_PACKAGE,
        canonical,
        workspace_manifest.get("version"),
    )

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
    _observe(
        observations,
        "Node candidate workspace lock",
        NODE_WORKSPACE_LOCK,
        canonical,
        workspace_lock.get("version"),
    )
    _observe(
        observations,
        "Node candidate workspace lock package",
        NODE_WORKSPACE_LOCK,
        canonical,
        lock_root.get("version"),
    )

    for entry in node_catalog.entries:
        manifest = view.json(entry.manifest_path)
        _observe(
            observations,
            f"Node candidate package {entry.name}",
            entry.manifest_path,
            canonical,
            manifest.get("version"),
        )


def _plan_node_candidate_projection(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    release: ReleaseVersion,
) -> dict[Path, str]:
    node_catalog = _node_package_catalog(view)
    updates = {
        NODE_CARGO_MANIFEST: _replace_toml_section_string(
            view.text(NODE_CARGO_MANIFEST),
            "package",
            "version",
            release.canonical,
        ),
        NODE_DESCRIPTOR: _replace_json_paths(
            view.text(NODE_DESCRIPTOR),
            {("version",): release.canonical},
        ),
        NODE_WORKSPACE_PACKAGE: _replace_json_paths(
            view.text(NODE_WORKSPACE_PACKAGE),
            {("version",): release.canonical},
        ),
        NODE_WORKSPACE_LOCK: _replace_json_paths(
            view.text(NODE_WORKSPACE_LOCK),
            {
                ("version",): release.canonical,
                ("packages", "", "version"): release.canonical,
            },
        ),
    }
    node_lock_packages = _local_coupled_lock_packages(
        view,
        catalog,
        NODE_CARGO_LOCK,
    ) | {NODE_CARGO_PACKAGE, NODE_BINDINGS_PACKAGE}
    updates[NODE_CARGO_LOCK] = _replace_lock_workspace_versions(
        view.text(NODE_CARGO_LOCK),
        node_lock_packages,
        release.canonical,
        lock_path=NODE_CARGO_LOCK,
    )
    for entry in node_catalog.entries:
        replacements = {("version",): release.canonical}
        if entry == node_catalog.root:
            replacements.update(
                {
                    ("optionalDependencies", target.name): release.canonical
                    for target in node_catalog.targets
                }
            )
        updates[entry.manifest_path] = _replace_json_paths(
            view.text(entry.manifest_path),
            replacements,
        )
    return updates


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
    base = release.base

    web_workspace = view.json(WEB_WORKSPACE_PACKAGE)
    if web_workspace.get("private") is not True:
        raise ReleaseProjectionError("platforms/web/package.json must be a private workspace owner")
    _observe(
        observations,
        "Web workspace",
        WEB_WORKSPACE_PACKAGE,
        canonical,
        web_workspace.get("version"),
    )
    web_entries = _web_package_entries(view)
    for entry, package_dir in web_entries:
        manifest_path = package_dir / "package.json"
        manifest = view.json(manifest_path)
        _observe(
            observations,
            f"Web package {entry['name']}",
            manifest_path,
            canonical,
            manifest.get("version"),
        )
    web_lock = view.json(WEB_LOCK)
    web_lock_packages = _mapping(web_lock.get("packages"), "Web lock packages")
    web_lock_workspace = _mapping(
        web_lock_packages.get(""), "Web lock workspace package"
    )
    _observe(
        observations,
        "Web workspace lock",
        WEB_LOCK,
        canonical,
        web_lock.get("version"),
    )
    _observe(
        observations,
        "Web workspace lock package",
        WEB_LOCK,
        canonical,
        web_lock_workspace.get("version"),
    )
    for entry, package_dir in web_entries:
        lock_key = package_dir.relative_to(WEB_DESCRIPTOR.parent).as_posix()
        local_package = _mapping(
            web_lock_packages.get(lock_key),
            f"Web lock package {lock_key}",
        )
        _observe(
            observations,
            f"Web lock package {entry['name']}",
            WEB_LOCK,
            canonical,
            local_package.get("version"),
        )

    playground_web_packages = _playground_web_dependencies(view, web_entries)
    playground_lock = view.json(PLAYGROUND_LOCK)
    playground_packages = _mapping(
        playground_lock.get("packages"), "Playground lock packages"
    )
    playground_web_workspace = _mapping(
        playground_packages.get("../platforms/web"),
        "Playground lock local Web workspace",
    )
    _observe(
        observations,
        "Playground local Web workspace lock",
        PLAYGROUND_LOCK,
        canonical,
        playground_web_workspace.get("version"),
    )
    for entry, package_dir in playground_web_packages:
        lock_key = f"../{package_dir.as_posix()}"
        local_package = _mapping(
            playground_packages.get(lock_key),
            f"Playground lock local {entry['name']} package",
        )
        _observe(
            observations,
            f"Playground local Web lock {entry['name']}",
            PLAYGROUND_LOCK,
            canonical,
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
    python = view.toml(PYTHON_MANIFEST)
    python_project = _mapping(python.get("project"), "Python [project]")
    _observe(
        observations,
        "Python package",
        PYTHON_MANIFEST,
        pep440,
        python_project.get("version"),
    )
    _observe_assignment(
        view,
        observations,
        "Android package",
        ANDROID_MANIFEST,
        r'^version\s*=\s*"([^"]+)"\s*$',
        canonical,
    )
    _observe_assignment(
        view,
        observations,
        "Flutter package",
        FLUTTER_MANIFEST,
        r"^version:\s*([^\s#]+)\s*$",
        canonical,
    )
    _observe_assignment(
        view,
        observations,
        "Flutter bundled native package version",
        FLUTTER_PACKAGE_VERSION,
        r"^const String mermanPackageVersion = '([^']+)';\s*$",
        canonical,
    )
    _observe_assignment(
        view,
        observations,
        "Flutter Android package",
        FLUTTER_ANDROID_MANIFEST,
        r"^version\s*=\s*'([^']+)'\s*$",
        canonical,
    )
    _observe_assignment(
        view,
        observations,
        "Flutter iOS Podspec",
        FLUTTER_IOS_PODSPEC,
        r"^\s*s\.version\s*=\s*'([^']+)'\s*$",
        canonical,
    )
    _observe_assignment(
        view,
        observations,
        "Flutter macOS Podspec",
        FLUTTER_MACOS_PODSPEC,
        r"^\s*s\.version\s*=\s*'([^']+)'\s*$",
        canonical,
    )

    build_text = view.text(FLUTTER_IOS_BUILD)
    short_version = _plist_value(build_text, "CFBundleShortVersionString")
    bundle_version = _plist_value(build_text, "CFBundleVersion")
    _observe(
        observations,
        "Flutter iOS framework short version",
        FLUTTER_IOS_BUILD,
        base,
        short_version,
    )
    _observe(
        observations,
        "Flutter iOS framework bundle version",
        FLUTTER_IOS_BUILD,
        base,
        bundle_version,
    )


def _collect_readme_projection(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    errors: list[str],
    *,
    required_mode: str | None = None,
) -> None:
    mode = (
        release_readme.REGISTRY_MODE
        if catalog.readme_registry_version is not None
        else release_readme.SOURCE_MODE
    )
    if required_mode is not None:
        if required_mode not in release_readme.MODES:
            raise ReleaseProjectionError(
                f"unsupported README installation mode {required_mode!r}; "
                f"expected one of {sorted(release_readme.MODES)}"
            )
        if mode != required_mode:
            errors.append(
                f"README installation mode is {mode!r}, expected "
                f"{required_mode!r}; run `python3 scripts/release-version.py "
                f"set-readme-mode --mode {required_mode} --version "
                f"{catalog.authority.canonical}`"
            )
    try:
        release_readme.verify_readme(
            view.text(README),
            catalog.authority,
            mode=mode,
            repository_url=catalog.repository_url,
        )
    except release_readme.ReleaseReadmeError as exc:
        errors.append(f"{README}: {exc}")
        return
    for path in PROJECTED_READMES:
        try:
            release_readme.verify_projected_readme(
                path.as_posix(),
                view.text(path),
                catalog.authority,
                mode=mode,
                repository_url=catalog.repository_url,
            )
        except release_readme.ReleaseReadmeError as exc:
            errors.append(f"{path}: {exc}")
            continue


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


def _plist_value(text: str, key: str) -> str:
    pattern = rf"<key>{re.escape(key)}</key>\s*<string>([^<]+)</string>"
    matches = re.findall(pattern, text)
    if len(matches) != 1:
        raise ReleaseProjectionError(
            f"{FLUTTER_IOS_BUILD} must contain exactly one {key}; found {len(matches)}"
        )
    return matches[0]


def _render_readme_projections(
    view: RepositoryView,
    release: ReleaseVersion,
    mode: str,
    repository_url: str,
) -> dict[Path, str]:
    rendered = {
        README: release_readme.render_readme(
            view.text(README),
            release,
            mode,
            repository_url,
        )
    }
    for path in PROJECTED_READMES:
        rendered[path] = release_readme.render_projected_readme(
            path.as_posix(),
            view.text(path),
            release,
            mode,
            repository_url,
        )
    return rendered


def plan_version_update(
    root: Path,
    version: str,
    *,
    overrides: Mapping[Path | str, str] | None = None,
) -> dict[Path, str]:
    updates, _originals = _plan_version_update(
        root,
        version,
        overrides=overrides,
    )
    return updates


def _plan_version_update(
    root: Path,
    version: str,
    *,
    overrides: Mapping[Path | str, str] | None = None,
) -> tuple[dict[Path, str], dict[Path, str]]:
    release = parse_release_version(version, allow_v_prefix=False)
    view = RepositoryView(root, overrides)
    catalog = load_workspace_catalog(view)
    updates: dict[Path, str] = {}
    version_changed = catalog.authority.canonical != release.canonical
    readme_mode = (
        release_readme.REGISTRY_MODE
        if not version_changed and catalog.readme_registry_version is not None
        else release_readme.SOURCE_MODE
    )

    root_text = view.text(ROOT_MANIFEST)
    root_text = _replace_toml_section_string(
        root_text, "workspace.package", "version", release.canonical
    )
    if version_changed:
        root_text = _set_optional_toml_section_string(
            root_text,
            "workspace.metadata.merman-release",
            "readme-registry-version",
            None,
        )
    workspace_dependencies = _mapping(
        _mapping(catalog.root_data["workspace"], "Cargo.toml [workspace]").get(
            "dependencies"
        ),
        "Cargo.toml [workspace.dependencies]",
    )
    coupled_dirs = _coupled_package_dirs(view, catalog)
    for dependency_key, spec in workspace_dependencies.items():
        if not isinstance(spec, dict) or not isinstance(spec.get("path"), str):
            continue
        if (view.root / spec["path"]).resolve() in coupled_dirs:
            root_text = _replace_toml_inline_string(
                root_text,
                "workspace.dependencies",
                dependency_key,
                "version",
                release.canonical,
            )
    updates[ROOT_MANIFEST] = root_text
    updates[ROOT_LOCK] = _replace_lock_workspace_versions(
        view.text(ROOT_LOCK),
        set(catalog.coupled_packages),
        release.canonical,
        lock_path=ROOT_LOCK,
    )
    fuzz_lock_packages = _local_coupled_lock_packages(view, catalog, FUZZ_LOCK)
    if not fuzz_lock_packages:
        raise ReleaseProjectionError(
            "fuzz/Cargo.lock does not contain any workspace-coupled package entries"
        )
    updates[FUZZ_LOCK] = _replace_lock_workspace_versions(
        view.text(FUZZ_LOCK),
        fuzz_lock_packages,
        release.canonical,
        lock_path=FUZZ_LOCK,
    )

    updates.update(_plan_node_candidate_projection(view, catalog, release))

    web_entries = _web_package_entries(view)
    updates[WEB_WORKSPACE_PACKAGE] = _replace_json_paths(
        view.text(WEB_WORKSPACE_PACKAGE), {("version",): release.canonical}
    )
    for _entry, package_dir in web_entries:
        manifest_path = package_dir / "package.json"
        updates[manifest_path] = _replace_json_paths(
            view.text(manifest_path), {("version",): release.canonical}
        )
    web_lock_updates = {
        ("packages", package_dir.relative_to(WEB_DESCRIPTOR.parent).as_posix(), "version"): release.canonical
        for _entry, package_dir in web_entries
    }
    web_lock_updates.update(
        {
            ("version",): release.canonical,
            ("packages", "", "version"): release.canonical,
        }
    )
    updates[WEB_LOCK] = _replace_json_paths(
        view.text(WEB_LOCK),
        web_lock_updates,
    )
    playground_web_packages = _playground_web_dependencies(view, web_entries)
    playground_lock_updates = {
        (
            "packages",
            f"../{package_dir.as_posix()}",
            "version",
        ): release.canonical
        for _entry, package_dir in playground_web_packages
    }
    playground_lock_updates[("packages", "../platforms/web", "version")] = (
        release.canonical
    )
    playground_lock = _replace_json_paths(
        view.text(PLAYGROUND_LOCK),
        playground_lock_updates,
    )
    updates[PLAYGROUND_LOCK] = playground_lock
    playground_license_report = _replace_one(
        view.text(PLAYGROUND_LICENSE_REPORT),
        r"^(package-lock\.json SHA-256: )[0-9a-f]{64}$",
        rf"\g<1>{hashlib.sha256(playground_lock.encode('utf-8')).hexdigest()}",
        PLAYGROUND_LICENSE_REPORT,
        "package-lock.json digest",
    )
    updates[PLAYGROUND_LICENSE_REPORT] = playground_license_report
    updates[PYTHON_MANIFEST] = _replace_toml_section_string(
        view.text(PYTHON_MANIFEST), "project", "version", release.to_pep440()
    )
    updates[ANDROID_MANIFEST] = _replace_one(
        view.text(ANDROID_MANIFEST),
        r'^(version\s*=\s*")[^"]+("\s*)$',
        rf"\g<1>{release.canonical}\g<2>",
        ANDROID_MANIFEST,
        "Android version",
    )
    updates[FLUTTER_MANIFEST] = _replace_one(
        view.text(FLUTTER_MANIFEST),
        r"^(version:\s*)[^\s#]+(\s*)$",
        rf"\g<1>{release.canonical}\g<2>",
        FLUTTER_MANIFEST,
        "Flutter version",
    )
    updates[FLUTTER_PACKAGE_VERSION] = _replace_one(
        view.text(FLUTTER_PACKAGE_VERSION),
        r"^(const String mermanPackageVersion = ')[^']+(';\s*)$",
        rf"\g<1>{release.canonical}\g<2>",
        FLUTTER_PACKAGE_VERSION,
        "Flutter bundled native package version",
    )
    updates[FLUTTER_ANDROID_MANIFEST] = _replace_one(
        view.text(FLUTTER_ANDROID_MANIFEST),
        r"^(version\s*=\s*')[^']+('\s*)$",
        rf"\g<1>{release.canonical}\g<2>",
        FLUTTER_ANDROID_MANIFEST,
        "Flutter Android version",
    )
    for podspec in (FLUTTER_IOS_PODSPEC, FLUTTER_MACOS_PODSPEC):
        updates[podspec] = _replace_one(
            view.text(podspec),
            r"^(\s*s\.version\s*=\s*')[^']+('\s*)$",
            rf"\g<1>{release.canonical}\g<2>",
            podspec,
            "Podspec version",
        )
    build_text = view.text(FLUTTER_IOS_BUILD)
    for plist_key in ("CFBundleShortVersionString", "CFBundleVersion"):
        build_text = _replace_one(
            build_text,
            rf"(<key>{plist_key}</key>\s*<string>)[^<]+(</string>)",
            rf"\g<1>{release.base}\g<2>",
            FLUTTER_IOS_BUILD,
            plist_key,
            flags=0,
        )
    updates[FLUTTER_IOS_BUILD] = build_text
    updates.update(
        _render_readme_projections(
            view,
            release,
            readme_mode,
            catalog.repository_url,
        )
    )

    result = verify_repository(
        root,
        expected_version=release.canonical,
        overrides={**view.overrides, **updates},
    )
    if not result.ok:
        detail = "\n".join(format_verification_failures(result))
        raise ReleaseProjectionError(
            f"planned release version projection did not verify:\n{detail}"
        )
    changed = {
        path: content
        for path, content in updates.items()
        if content != view.text(path)
    }
    originals = {path: view.text(path) for path in changed}
    return changed, originals


def apply_version_update(root: Path, version: str) -> tuple[Path, ...]:
    updates, originals = _plan_version_update(root, version)
    _replace_projection_files(root.resolve(), updates, expected=originals)
    result = verify_repository(root, expected_version=version)
    if not result.ok:
        raise ReleaseProjectionError(
            "release version projection changed on disk but did not verify; "
            "keep the worktree, resolve any concurrent edit, and rerun the same "
            "command: "
            + "; ".join(format_verification_failures(result))
        )
    return tuple(sorted(updates))


def plan_readme_install_mode(
    root: Path,
    version: str,
    mode: str,
    *,
    overrides: Mapping[Path | str, str] | None = None,
) -> dict[Path, str]:
    updates, _originals = _plan_readme_install_mode(
        root,
        version,
        mode,
        overrides=overrides,
    )
    return updates


def _plan_readme_install_mode(
    root: Path,
    version: str,
    mode: str,
    *,
    overrides: Mapping[Path | str, str] | None = None,
) -> tuple[dict[Path, str], dict[Path, str]]:
    release = parse_release_version(version, allow_v_prefix=False)
    view = RepositoryView(root, overrides)
    catalog = load_workspace_catalog(view)
    if catalog.authority.canonical != release.canonical:
        raise ReleaseProjectionError(
            "README installation version must match Cargo workspace authority: "
            f"expected {catalog.authority.canonical}, found {release.canonical}"
        )
    if mode not in release_readme.MODES:
        raise ReleaseProjectionError(
            f"unsupported README installation mode {mode!r}; "
            f"expected one of {sorted(release_readme.MODES)}"
        )

    root_text = _set_optional_toml_section_string(
        view.text(ROOT_MANIFEST),
        "workspace.metadata.merman-release",
        "readme-registry-version",
        release.canonical if mode == release_readme.REGISTRY_MODE else None,
    )
    try:
        projected_readmes = _render_readme_projections(
            view,
            release,
            mode,
            catalog.repository_url,
        )
    except release_readme.ReleaseReadmeError as exc:
        raise ReleaseProjectionError(str(exc)) from exc

    result = verify_repository(
        root,
        expected_version=release.canonical,
        overrides={
            **view.overrides,
            ROOT_MANIFEST: root_text,
            **projected_readmes,
        },
    )
    if not result.ok:
        raise ReleaseProjectionError(
            "planned README installation projection did not verify: "
            + "; ".join(format_verification_failures(result))
        )
    updates = {
        path: content
        for path, content in {
            ROOT_MANIFEST: root_text,
            **projected_readmes,
        }.items()
        if content != view.text(path)
    }
    originals = {path: view.text(path) for path in updates}
    return updates, originals


def apply_readme_install_mode(
    root: Path,
    version: str,
    mode: str,
) -> tuple[Path, ...]:
    updates, originals = _plan_readme_install_mode(root, version, mode)
    if not updates:
        return ()
    _replace_projection_files(root.resolve(), updates, expected=originals)
    result = verify_repository(
        root,
        expected_version=version,
        required_readme_mode=mode,
    )
    if not result.ok:
        raise ReleaseProjectionError(
            "README installation projection changed on disk but did not verify; "
            "keep the worktree, resolve any concurrent edit, and rerun the same "
            "command: "
            + "; ".join(format_verification_failures(result))
        )
    return tuple(sorted(updates))


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


def _set_optional_toml_section_string(
    text: str,
    section: str,
    key: str,
    value: str | None,
) -> str:
    lines = text.splitlines(keepends=True)
    section_header = f"[{section}]"
    section_indexes = [
        index for index, line in enumerate(lines) if line.strip() == section_header
    ]
    if len(section_indexes) != 1:
        raise ReleaseProjectionError(
            f"expected one {section_header} section; found {len(section_indexes)}"
        )
    section_start = section_indexes[0]
    section_end = next(
        (
            index
            for index in range(section_start + 1, len(lines))
            if lines[index].strip().startswith("[")
            and lines[index].strip().endswith("]")
        ),
        len(lines),
    )
    assignment = re.compile(
        rf'^(\s*{re.escape(key)}\s*=\s*)"[^"]*"(\s*(?:#.*)?(?:\r?\n)?)$'
    )
    matches = [
        index
        for index in range(section_start + 1, section_end)
        if assignment.match(lines[index])
    ]
    if len(matches) > 1:
        raise ReleaseProjectionError(
            f"expected at most one {section}.{key} assignment; found {len(matches)}"
        )

    if value is None:
        if matches:
            del lines[matches[0]]
    elif matches:
        index = matches[0]
        lines[index] = assignment.sub(rf'\g<1>"{value}"\g<2>', lines[index])
    else:
        insert_at = section_end
        while insert_at > section_start + 1 and not lines[insert_at - 1].strip():
            insert_at -= 1
        newline = "\r\n" if "\r\n" in text else "\n"
        lines.insert(insert_at, f'{key} = "{value}"{newline}')

    candidate = "".join(lines)
    try:
        parsed = tomllib.loads(candidate)
        table: Any = parsed
        for component in section.split("."):
            table = table[component]
        actual = table.get(key)
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


def _replace_lock_workspace_versions(
    text: str,
    package_names: set[str],
    version: str,
    *,
    lock_path: Path = ROOT_LOCK,
) -> str:
    marker = "[[package]]"
    prefix, *raw_blocks = text.split(marker)
    seen: set[str] = set()
    blocks: list[str] = []
    for raw_block in raw_blocks:
        document = f"{marker}{raw_block}"
        try:
            parsed = tomllib.loads(document)["package"][0]
        except (KeyError, TypeError, tomllib.TOMLDecodeError) as exc:
            raise ReleaseProjectionError(f"invalid Cargo.lock package block: {exc}") from exc
        name = parsed.get("name")
        if name in package_names and "source" not in parsed:
            document = _replace_one(
                document,
                r'^(version\s*=\s*")[^"]+("\s*)$',
                rf"\g<1>{version}\g<2>",
                lock_path,
                f"local package {name} version",
            )
            seen.add(name)
        blocks.append(document)
    missing = package_names - seen
    if missing:
        raise ReleaseProjectionError(
            f"{lock_path} is missing local workspace packages: "
            + ", ".join(sorted(missing))
        )
    return prefix + "".join(blocks)


def _local_coupled_lock_packages(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    lock_path: Path,
) -> set[str]:
    lock = view.toml(lock_path)
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise ReleaseProjectionError(f"{lock_path} package must be an array of tables")
    return {
        package["name"]
        for package in packages
        if isinstance(package, dict)
        and package.get("name") in catalog.coupled_packages
        and "source" not in package
    }


def _replace_json_paths(
    text: str,
    replacements: Mapping[tuple[str, ...], str],
) -> str:
    try:
        data = json.loads(text, object_pairs_hook=_reject_duplicate_keys)
    except json.JSONDecodeError as exc:
        raise ReleaseProjectionError(f"cannot update invalid JSON: {exc}") from exc
    changed = False
    for path, value in replacements.items():
        target = data
        for component in path[:-1]:
            target = _mapping(target.get(component), f"JSON path {'.'.join(path)}")
        if path[-1] not in target:
            raise ReleaseProjectionError(f"missing JSON path {'.'.join(path)}")
        if target[path[-1]] != value:
            changed = True
        target[path[-1]] = value
    if not changed:
        return text
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


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


def _replace_projection_files(
    root: Path,
    updates: Mapping[Path, str],
    *,
    expected: Mapping[Path, str],
) -> None:
    """Install validated projections in an exclusive Git worktree.

    Each file replacement is atomic, but the group is intentionally not a
    filesystem transaction. The workspace authority is installed last, so an
    interrupted update can be completed by rerunning the same command.
    """

    root = root.resolve()
    prepared: list[tuple[Path, Path, Path, bytes, int]] = []
    installed: list[Path] = []
    try:
        for relative, content in sorted(
            updates.items(),
            key=lambda item: (item[0] == ROOT_MANIFEST, str(item[0])),
        ):
            normalized, target = _projection_target(root, relative)
            try:
                expected_text = expected[relative]
            except KeyError as exc:
                raise ReleaseProjectionError(
                    f"missing expected pre-update content for {relative}"
                ) from exc
            original, mode = _read_projection_preimage(target, normalized)
            expected_bytes = expected_text.encode("utf-8")
            if original != expected_bytes:
                raise ReleaseProjectionError(
                    f"{normalized} changed while planning the release update; "
                    "no files were written"
                )
            temp_path = _write_projection_temp(
                target,
                content.encode("utf-8"),
                mode,
            )
            prepared.append(
                (normalized, target, temp_path, expected_bytes, mode)
            )

        # Catch edits made while temporary files were being prepared before
        # replacing any tracked file.
        for relative, target, _temp_path, expected_bytes, mode in prepared:
            _require_projection_preimage(
                target,
                relative,
                expected_bytes,
                mode,
            )

        for relative, target, temp_path, expected_bytes, mode in prepared:
            # This is a cooperative check, not a claim to serialize arbitrary
            # editors. Release preparation requires an exclusive worktree.
            _require_projection_preimage(
                target,
                relative,
                expected_bytes,
                mode,
            )
            os.replace(temp_path, target)
            installed.append(relative)

        for directory in {target.parent for _, target, _, _, _ in prepared}:
            _sync_projection_directory(directory)
    except BaseException as exc:
        if installed:
            raise ReleaseProjectionError(
                "release projection update stopped after changing "
                f"{', '.join(str(path) for path in installed)}; keep the "
                "worktree and rerun the same command"
            ) from exc
        raise
    finally:
        for _relative, _target, temp_path, _expected_bytes, _mode in prepared:
            temp_path.unlink(missing_ok=True)


def _projection_target(root: Path, relative: Path) -> tuple[Path, Path]:
    normalized = Path(os.path.normpath(relative))
    if (
        relative.is_absolute()
        or normalized.is_absolute()
        or normalized == Path(".")
        or ".." in normalized.parts
    ):
        raise ReleaseProjectionError(
            f"release projection path escapes repository root: {relative}"
        )

    target = root / normalized
    current = root
    for component in normalized.parts[:-1]:
        current /= component
        try:
            metadata = current.lstat()
        except OSError as exc:
            raise ReleaseProjectionError(
                f"release projection has an inaccessible parent: {normalized}"
            ) from exc
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ReleaseProjectionError(
                f"release projection has an unsafe parent: {normalized}"
            )
    return normalized, target


def _read_projection_preimage(target: Path, relative: Path) -> tuple[bytes, int]:
    try:
        metadata = target.lstat()
    except OSError as exc:
        raise ReleaseProjectionError(
            f"cannot inspect release projection {relative}: {exc}"
        ) from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise ReleaseProjectionError(
            f"release projection must be a regular non-symlink file: {relative}"
        )
    try:
        return target.read_bytes(), stat.S_IMODE(metadata.st_mode)
    except OSError as exc:
        raise ReleaseProjectionError(
            f"cannot read release projection {relative}: {exc}"
        ) from exc


def _require_projection_preimage(
    target: Path,
    relative: Path,
    expected: bytes,
    expected_mode: int,
) -> None:
    content, mode = _read_projection_preimage(target, relative)
    if content != expected or mode != expected_mode:
        raise ReleaseProjectionError(
            f"{relative} changed while preparing the release update; "
            "no further files were written"
        )


def _write_projection_temp(target: Path, content: bytes, mode: int) -> Path:
    descriptor, raw_path = tempfile.mkstemp(
        prefix=f".{target.name}.release-version-",
        dir=target.parent,
    )
    temp_path = Path(raw_path)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.chmod(temp_path, mode)
            os.fsync(handle.fileno())
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise
    return temp_path


def _sync_projection_directory(path: Path) -> None:
    if os.name == "nt":
        return
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def format_verification_failures(result: VerificationResult) -> list[str]:
    failures = list(result.errors)
    failures.extend(
        f"{item.label} ({item.path}): {item.actual!r} != {item.expected!r}"
        for item in result.observations
        if not item.matches
    )
    return failures
