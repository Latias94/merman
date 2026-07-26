"""Workspace release-version authority, projections, and transactional updates."""

from __future__ import annotations

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


ROOT_MANIFEST = Path("Cargo.toml")
ROOT_LOCK = Path("Cargo.lock")
FUZZ_MANIFEST = Path("fuzz/Cargo.toml")
FUZZ_LOCK = Path("fuzz/Cargo.lock")
WEB_WORKSPACE_PACKAGE = Path("platforms/web/package.json")
WEB_DESCRIPTOR = Path("platforms/web/web-surface-descriptor.json")
WEB_LOCK = Path("platforms/web/package-lock.json")
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
    member_manifests: tuple[Path, ...]
    root_data: Mapping[str, Any]


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
        try:
            value = tomllib.loads(self.text(relative))
        except tomllib.TOMLDecodeError as exc:
            raise ReleaseProjectionError(f"invalid TOML in {relative}: {exc}") from exc
        if not isinstance(value, dict):
            raise ReleaseProjectionError(f"expected a TOML document in {relative}")
        return value

    def json(self, path: Path | str) -> Mapping[str, Any]:
        relative = self._relative(Path(path))
        try:
            value = json.loads(self.text(relative), object_pairs_hook=_reject_duplicate_keys)
        except json.JSONDecodeError as exc:
            raise ReleaseProjectionError(f"invalid JSON in {relative}: {exc}") from exc
        if not isinstance(value, dict):
            raise ReleaseProjectionError(f"expected a JSON object in {relative}")
        return value


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
        coupled_packages=coupled,
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
    _collect_fuzz_lock_versions(view, catalog, observations, errors)
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
    coupled_dirs = {
        (view.root / member).resolve(): name
        for name, member in catalog.coupled_packages.items()
    }
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


def _collect_member_dependency_policy(
    view: RepositoryView,
    catalog: WorkspaceCatalog,
    errors: list[str],
) -> None:
    coupled_dirs = {
        (view.root / member).resolve(): name
        for name, member in catalog.coupled_packages.items()
    }
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
                f"{lock_path} package {package_name}",
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
    coupled_dirs = {
        (view.root / member).resolve(): name
        for name, member in catalog.coupled_packages.items()
    }
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


def plan_version_update(root: Path, version: str) -> dict[Path, str]:
    updates, _originals = _plan_version_update(root, version)
    return updates


def _plan_version_update(
    root: Path,
    version: str,
) -> tuple[dict[Path, str], dict[Path, str]]:
    release = parse_release_version(version, allow_v_prefix=False)
    view = RepositoryView(root)
    catalog = load_workspace_catalog(view)
    updates: dict[Path, str] = {}

    root_text = view.text(ROOT_MANIFEST)
    root_text = _replace_toml_section_string(
        root_text, "workspace.package", "version", release.canonical
    )
    workspace_dependencies = _mapping(
        _mapping(catalog.root_data["workspace"], "Cargo.toml [workspace]").get(
            "dependencies"
        ),
        "Cargo.toml [workspace.dependencies]",
    )
    coupled_dirs = {
        (view.root / member).resolve()
        for member in catalog.coupled_packages.values()
    }
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
        view.text(ROOT_LOCK), set(catalog.coupled_packages), release.canonical
    )
    fuzz_lock_packages = _local_coupled_lock_packages(view, catalog, FUZZ_LOCK)
    if not fuzz_lock_packages:
        raise ReleaseProjectionError(
            "fuzz/Cargo.lock does not contain any workspace-coupled package entries"
        )
    updates[FUZZ_LOCK] = _replace_lock_workspace_versions(
        view.text(FUZZ_LOCK), fuzz_lock_packages, release.canonical
    )

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

    result = verify_repository(
        root,
        expected_version=release.canonical,
        overrides=updates,
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
    _atomic_replace(root.resolve(), updates, expected=originals)
    result = verify_repository(root, expected_version=version)
    if not result.ok:
        raise ReleaseProjectionError(
            "release version projection changed on disk but did not verify: "
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
                ROOT_LOCK,
                f"local package {name} version",
            )
            seen.add(name)
        blocks.append(document)
    missing = package_names - seen
    if missing:
        raise ReleaseProjectionError(
            "Cargo.lock is missing local workspace packages: " + ", ".join(sorted(missing))
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
    for path, value in replacements.items():
        target = data
        for component in path[:-1]:
            target = _mapping(target.get(component), f"JSON path {'.'.join(path)}")
        if path[-1] not in target:
            raise ReleaseProjectionError(f"missing JSON path {'.'.join(path)}")
        target[path[-1]] = value
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


def _atomic_replace(
    root: Path,
    updates: Mapping[Path, str],
    *,
    expected: Mapping[Path, str] | None = None,
) -> None:
    root = root.resolve()
    prepared: list[tuple[Path, Path, Path]] = []
    replaced: list[tuple[Path, Path]] = []
    try:
        for relative, content in sorted(updates.items(), key=lambda item: str(item[0])):
            path = (root / relative).resolve()
            try:
                path.relative_to(root)
            except ValueError as exc:
                raise ReleaseProjectionError(f"update path escapes repository: {relative}") from exc
            if path.is_symlink() or not path.is_file():
                raise ReleaseProjectionError(
                    f"release projection must be a regular non-symlink file: {relative}"
                )
            original = path.read_bytes()
            if expected is not None:
                expected_text = expected.get(relative)
                if expected_text is None:
                    raise ReleaseProjectionError(
                        f"missing expected pre-update content for {relative}"
                    )
                if original != expected_text.encode("utf-8"):
                    raise ReleaseProjectionError(
                        f"{relative} changed while planning the release update; no files were written"
                    )
            mode = stat.S_IMODE(path.stat().st_mode)
            new_temp = _write_temp(path, content.encode("utf-8"), mode)
            rollback_temp = _write_temp(path, original, mode)
            prepared.append((path, new_temp, rollback_temp))

        for path, new_temp, rollback_temp in prepared:
            os.replace(new_temp, path)
            replaced.append((path, rollback_temp))
        for directory in {path.parent for path, _, _ in prepared}:
            _sync_directory(directory)
    except Exception as exc:
        rollback_errors: list[str] = []
        for path, rollback_temp in reversed(replaced):
            try:
                os.replace(rollback_temp, path)
            except OSError as rollback_exc:
                rollback_errors.append(f"{path}: {rollback_exc}")
        if rollback_errors:
            raise ReleaseProjectionError(
                f"release version update failed ({exc}); rollback also failed: "
                + "; ".join(rollback_errors)
            ) from exc
        raise
    finally:
        for _path, new_temp, rollback_temp in prepared:
            for temp_path in (new_temp, rollback_temp):
                try:
                    temp_path.unlink()
                except FileNotFoundError:
                    pass


def _write_temp(target: Path, content: bytes, mode: int) -> Path:
    descriptor, raw_path = tempfile.mkstemp(
        prefix=f".{target.name}.release-version-",
        dir=target.parent,
    )
    temp_path = Path(raw_path)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temp_path, mode)
    except Exception:
        temp_path.unlink(missing_ok=True)
        raise
    return temp_path


def _sync_directory(path: Path) -> None:
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
