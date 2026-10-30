#!/usr/bin/env python3
"""Package descriptor-selected SDKs from already built and host-tested artifacts."""

from __future__ import annotations

import argparse
from dataclasses import replace
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

if __package__:
    from .artifact_profile_recipe import cargo_build_args, load_artifact_profiles, load_artifact_profile as load_artifact_recipe
    from .release_archive import (
        ArchiveMember, ArchiveVerificationError, DEFAULT_LIMITS, _normalize_member_name,
        _validate_member_set, read_checksum, release_archive_name_for,
        persist_verified_archive, require_regular_input, require_repository_root, sha256_file,
        verified_archive_contents,
    )
else:
    from artifact_profile_recipe import cargo_build_args, load_artifact_profiles, load_artifact_profile as load_artifact_recipe
    from release_archive import (
        ArchiveMember, ArchiveVerificationError, DEFAULT_LIMITS, _normalize_member_name,
        _validate_member_set, read_checksum, release_archive_name_for,
        persist_verified_archive, require_regular_input, require_repository_root, sha256_file,
        verified_archive_contents,
    )


REPO_ROOT = Path(__file__).resolve().parents[1]
# SDK static libraries retain ThinLTO bitcode alongside native objects. A member may use
# the existing 512 MiB aggregate allowance; archive and total extraction budgets stay fixed.
SDK_LIMITS = replace(DEFAULT_LIMITS, max_member_size=DEFAULT_LIMITS.max_total_size)
PROFILES = tuple(f"sdk-{kind}-{flavor}" for kind in ("native", "wasi")
                 for flavor in ("standard", "math", "full"))
NATIVE_TARGETS = (
    "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-msvc",
)
WASI_TARGET = "wasm32-wasip1"
HEADERS = ("merman.h", "merman_text_measurement_abi.h", "merman_resource_contract.h")
WASI_EXAMPLE_FILES = (
    *(f"go-wasi/{name}" for name in ("go.mod", "go.sum", "client.go", "client_test.go",
                                    "cmd/render/main.go", "README.md")),
    *(f"python-wasi/{name}" for name in ("client.py", "render.py", "test_client.py",
                                        "requirements.txt", "README.md")),
)
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
FORBIDDEN_PARTS = frozenset({".git", "tests", "fixtures", "__pycache__", "target"})


def _legal_module():
    spec = importlib.util.spec_from_file_location(
        "sdk_legal_projection", REPO_ROOT / "scripts/sync-release-legal-materials.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def library_names(target: str) -> dict[str, str]:
    if target == WASI_TARGET:
        return {"module": "merman_wasi.wasm"}
    if target not in NATIVE_TARGETS:
        raise ArchiveVerificationError(f"unsupported SDK target: {target!r}")
    if "windows" in target:
        return {"static": "merman_ffi.lib", "dynamic": "merman_ffi.dll",
                "import": "merman_ffi.dll.lib"}
    return {"static": "libmerman_ffi.a",
            "dynamic": "libmerman_ffi.dylib" if "apple" in target else "libmerman_ffi.so"}


def library_layout(target: str) -> dict[str, str]:
    """Map library roles to their installed archive-relative paths."""
    return {kind: ("bin/" if kind == "dynamic" and "windows" in target else "lib/") + name
            for kind, name in library_names(target).items()}


def _object_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ArchiveVerificationError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result


def _json(path: Path) -> dict:
    require_regular_input(path, "JSON input")
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_object_pairs)
    except (ValueError, UnicodeError) as error:
        raise ArchiveVerificationError(f"cannot read JSON input {path}: {error}") from error
    if not isinstance(value, dict):
        raise ArchiveVerificationError(f"JSON input must be an object: {path}")
    return value


def _member_path(relative: str) -> str:
    normalized = _normalize_member_name(relative, is_directory=False)
    if FORBIDDEN_PARTS.intersection(normalized.split("/")):
        raise ArchiveVerificationError(f"non-public SDK payload path: {relative!r}")
    return normalized


def _input(root: Path, relative: str) -> Path:
    relative = _member_path(relative)
    current = root
    for part in relative.split("/"):
        current = current / part
        if current.is_symlink():
            raise ArchiveVerificationError(f"SDK input must not traverse a symlink: {current}")
    require_regular_input(current, "SDK input")
    if current.stat().st_size > SDK_LIMITS.max_member_size:
        raise ArchiveVerificationError(f"SDK input exceeds archive member budget: {current}")
    return current


def _artifact_input(root, original_root, path):
    absolute = Path(path).absolute()
    for prefix in (original_root, root):
        try:
            relative = absolute.relative_to(prefix)
        except ValueError:
            continue
        return _input(root, relative.as_posix())
    raise ArchiveVerificationError("catalog and Rust license report must be inside artifact-dir")


def _recipe(profile, target, descriptor, repo_root):
    if profile not in PROFILES:
        raise ArchiveVerificationError(f"unsupported SDK profile: {profile!r}")
    library_names(target)
    if profile.startswith("sdk-wasi-") != (target == WASI_TARGET):
        raise ArchiveVerificationError("profile and target select different SDK transports")
    recipe = load_artifact_recipe(profile, descriptor)
    if recipe.build_target_kind != "target-set" or target not in recipe.build_targets:
        raise ArchiveVerificationError(f"profile {profile!r} does not declare target {target!r}")
    wasi = target == WASI_TARGET
    if recipe.package != ("merman-wasi" if wasi else "merman-ffi"):
        raise ArchiveVerificationError("SDK recipe package does not match its transport")
    if recipe.target_name != ("merman_wasi" if wasi else "merman_ffi"):
        raise ArchiveVerificationError("SDK recipe library name does not match its transport")
    required_types = {"cdylib"} if wasi else {"cdylib", "staticlib"}
    if not required_types.issubset(recipe.crate_types):
        raise ArchiveVerificationError("SDK recipe does not build the required library types")
    return recipe


def build_command(profile: str, target: str, descriptor: Path, repo_root: Path) -> list[str]:
    recipe = _recipe(profile, target, descriptor, repo_root)
    return cargo_build_args(recipe, target=target, locked=True, repo_root=repo_root)


def require_clean_source_tree(repo_root: Path) -> None:
    repo_root = require_repository_root(Path(repo_root))
    completed = subprocess.run(
        ["git", "-C", str(repo_root), "status", "--porcelain=v1",
         "--untracked-files=all"],
        check=False, capture_output=True, text=True,
    )
    if completed.returncode:
        raise ArchiveVerificationError("cannot verify that the SDK source tree is clean")
    if completed.stdout.strip():
        raise ArchiveVerificationError("SDK source tree has uncommitted changes")


def resolve_source_commit(repo_root: Path, expected: str | None) -> str:
    if expected is not None and not COMMIT.fullmatch(expected):
        raise ArchiveVerificationError("source_commit must be a lowercase 40-character Git commit")
    completed = subprocess.run(
        ["git", "-C", str(repo_root), "rev-parse", "--verify", "HEAD^{commit}"],
        check=False, capture_output=True, text=True,
    )
    commit = completed.stdout.strip()
    if completed.returncode or not COMMIT.fullmatch(commit):
        raise ArchiveVerificationError("cannot resolve SDK source from the checked-out Git HEAD")
    if expected is not None and expected != commit:
        raise ArchiveVerificationError("source_commit does not match the checked-out Git HEAD")
    return commit


def _workspace_version(repo_root):
    document = tomllib.loads(_input(repo_root, "Cargo.toml").read_text())
    version = document["workspace"]["package"]["version"]
    if not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.+-]+)?", version):
        raise ArchiveVerificationError("workspace version is missing or invalid")
    return version


def _ids(value, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise ArchiveVerificationError(f"{label} must be a string array")
    if len(value) != len(set(value)):
        raise ArchiveVerificationError(f"{label} contains duplicate IDs")
    return sorted(value)


def _catalog(sidecar: dict, *, profile, target, commit, version, expected, binaries):
    for field, value in (("profile", profile), ("target", target), ("source_commit", commit)):
        if sidecar.get(field) != value:
            raise ArchiveVerificationError(f"runtime catalog sidecar {field} mismatch")
    if type(sidecar.get("schema_version")) is not int or sidecar["schema_version"] != 1:
        raise ArchiveVerificationError("runtime catalog sidecar schema_version must be 1")
    digests = {path.name: sha256_file(path) for path in binaries.values()}
    if sidecar.get("artifacts") != digests:
        raise ArchiveVerificationError("runtime catalog artifact filenames or SHA-256 digests mismatch")
    catalog = sidecar.get("runtime_catalog")
    if not isinstance(catalog, dict):
        raise ArchiveVerificationError("an actual artifact runtime_catalog object is required")
    abi = 1 if target == WASI_TARGET else 3
    for field, value in (("schema_version", 1), ("transport_api_version", abi),
                         ("package_version", version)):
        if type(catalog.get(field)) is not type(value) or catalog[field] != value:
            raise ArchiveVerificationError(f"artifact runtime catalog {field} mismatch")
    schemas = catalog.get("options_schema_versions")
    if not isinstance(schemas, list) or not schemas or any(type(item) is not int or item < 1 for item in schemas):
        raise ArchiveVerificationError("catalog must advertise options schema versions")
    caps = catalog.get("capabilities")
    registry = catalog.get("registry")
    if not isinstance(caps, dict) or not isinstance(registry, dict) or not isinstance(expected, dict):
        raise ArchiveVerificationError("catalog or descriptor expected surface is missing")
    comparisons = (
        (caps.get("capability_ids"), expected.get("runtime_ids"), "runtime capabilities"),
        (caps.get("output_ids"), expected.get("outputs"), "outputs"),
        (registry.get("diagram_families"), expected.get("diagram_families"), "diagram families"),
    )
    for observed, required, label in comparisons:
        if _ids(observed, f"catalog {label}") != _ids(required, f"recipe {label}"):
            raise ArchiveVerificationError(f"artifact catalog {label} differs from recipe expected surface")
    measurement = caps.get("text_measurement")
    if not isinstance(measurement, dict) or "native-fonts" not in _ids(
        measurement.get("provider_ids"), "text measurement providers"
    ):
        raise ArchiveVerificationError("SDK runtime catalog must expose the native-fonts provider")
    if "native-font-context" not in _ids(catalog.get("constructor_service_ids"), "constructor services"):
        raise ArchiveVerificationError("SDK runtime catalog must expose the native-font-context constructor service")
    capability_ids = caps["capability_ids"]
    flavor = profile.rsplit("-", 1)[1]
    if ("math" in capability_ids) != (flavor != "standard") or ("layout-elk" in capability_ids) != (flavor == "full"):
        raise ArchiveVerificationError("SDK flavor disagrees with actual math/ELK capabilities")
    return catalog


def _system_libraries(target):
    if "windows" in target:
        return ["advapi32", "bcrypt", "kernel32", "ntdll", "userenv", "ws2_32"]
    if "apple" in target:
        return ["-framework CoreFoundation", "-framework Security", "-framework SystemConfiguration", "c++"]
    return ["dl", "pthread", "m", "rt", "util"]


def write_catalog_sidecar(*, repo_root: Path, profile: str, target: str,
                          artifact_dir: Path, catalog_in: Path, output: Path | None = None,
                          source_commit: str | None = None, descriptor: Path | None = None) -> Path:
    repo_root = require_repository_root(Path(repo_root))
    artifact_dir = require_repository_root(Path(artifact_dir))
    require_clean_source_tree(repo_root)
    descriptor = Path(descriptor) if descriptor else repo_root / "capabilities/artifact-profiles-v2.json"
    _recipe(profile, target, descriptor, repo_root)
    document = next(item for item in _json(descriptor)["profiles"] if item["id"] == profile)
    commit = resolve_source_commit(repo_root, source_commit)
    binaries = {kind: _input(artifact_dir, name) for kind, name in library_names(target).items()}
    raw_root = require_repository_root(Path(catalog_in).parent)
    raw_catalog = _json(_input(raw_root, Path(catalog_in).name))
    sidecar = {"schema_version": 1, "profile": profile, "target": target,
               "source_commit": commit, "artifacts": {path.name: sha256_file(path) for path in binaries.values()},
               "runtime_catalog": raw_catalog}
    _catalog(sidecar, profile=profile, target=target, commit=commit,
             version=_workspace_version(repo_root), expected=document.get("expected"), binaries=binaries)
    output = Path(output) if output else artifact_dir / "runtime-catalog.json"
    if output.parent.resolve() != artifact_dir or output.parent.is_symlink():
        raise ArchiveVerificationError("catalog sidecar output must be directly inside artifact-dir")
    _member_path(output.name)
    if output.exists() or output.is_symlink():
        raise ArchiveVerificationError("catalog sidecar output already exists")
    with output.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(sidecar, indent=2, sort_keys=True) + "\n")
    return output.resolve()


def _native_metadata(files, repo_root, target, version, names):
    dynamic_path = library_layout(target)["dynamic"]
    template = _input(repo_root, "distribution/sdk/cmake/MermanConfig.cmake.in").read_text()
    import_library = (f'  set_property(TARGET Merman::Merman PROPERTY IMPORTED_IMPLIB "${{_merman_prefix}}/lib/{names["import"]}")'
                      if "import" in names else "")
    for key, value in {"DYNAMIC_PATH": dynamic_path, "STATIC_NAME": names["static"],
                       "IMPORT_LIBRARY": import_library,
                       "SYSTEM_LIBRARIES": ";".join(_system_libraries(target))}.items():
        template = template.replace(f"@{key}@", value)
    files["lib/cmake/Merman/MermanConfig.cmake"] = template.encode()
    private = " ".join(item if item.startswith("-") else f"-l{item}" for item in _system_libraries(target))
    link_name = "merman_ffi.dll" if "windows" in target else "merman_ffi"
    pc = ("prefix=${pcfiledir}/../..\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n\n"
          f"Name: Merman\nDescription: Merman C ABI SDK\nVersion: {version}\n"
          f"Libs: -L${{libdir}} -l{link_name}\nLibs.private: {private}\nCflags: -I${{includedir}}\n")
    files["lib/pkgconfig/merman.pc"] = pc.encode()
    if "windows" in target:
        files["lib/pkgconfig/merman-static.pc"] = pc.replace(
            "-lmerman_ffi.dll", "-lmerman_ffi").encode()


def native_consumer_files(repo_root: Path, target: str, version: str) -> dict[str, bytes]:
    """Generate the same native headers, examples and integration files for every consumer."""
    names = library_names(target)
    if target == WASI_TARGET:
        raise ArchiveVerificationError("native consumer files require a native SDK target")
    files = {f"include/{name}": _input(repo_root, f"crates/merman-ffi/include/{name}").read_bytes()
             for name in HEADERS}
    for name in ("CMakeLists.txt", "examples/render.c", "examples/probe.c", "examples/render.cpp"):
        files[name] = _input(repo_root, f"distribution/sdk/{name}").read_bytes()
    _native_metadata(files, repo_root, target, version, names)
    return files


def _header_version(data, macro):
    matches = re.findall(rb"^#define[ \t]+" + macro.encode() + rb"[ \t]+([0-9]+)[uU]?[ \t]*\r?$",
                         data, re.MULTILINE)
    if len(matches) != 1:
        raise ArchiveVerificationError(f"generated SDK header is missing {macro}")
    return int(matches[0])


def _legal_payload(files, repo_root, profile, report_path, descriptor):
    legal = _legal_module()
    scope_id = profile if profile.startswith("sdk-native-") else "sdk-" + profile.rsplit("-", 1)[1]
    contract = _json(_input(repo_root, "docs/release/THIRD_PARTY_COMPONENTS.json"))
    try:
        scope, components = legal.resolve_artifact_components(contract, scope_id)
        files["THIRD_PARTY_NOTICES.md"] = legal.artifact_scope_notice(scope, components)
    except legal.LegalMaterialError as error:
        raise ArchiveVerificationError(str(error)) from error
    for component in components:
        for entry in component.get("license_files", []):
            relative = _member_path(entry["path"])
            if not relative.startswith("THIRD_PARTY_LICENSES/"):
                raise ArchiveVerificationError("scoped legal file must be under THIRD_PARTY_LICENSES")
            data = _input(repo_root, relative).read_bytes()
            if hashlib.sha256(data).hexdigest() != entry.get("sha256"):
                raise ArchiveVerificationError(f"scoped license checksum mismatch: {relative}")
            files[relative] = data
    report = _json(report_path)
    projection = next(item.report_projection() for item in load_artifact_profiles(descriptor)
                      if item.profile_id == profile)
    if report.get("artifact_profile") != projection:
        raise ArchiveVerificationError("Rust license report does not match the exact SDK artifact profile")
    if not isinstance(report.get("licenses"), list) or not report["licenses"]:
        raise ArchiveVerificationError("SDK Rust license report must contain dependency license texts")
    for entry in report["licenses"]:
        if not isinstance(entry, dict) or not isinstance(entry.get("text"), str) or not entry["text"]:
            raise ArchiveVerificationError("SDK Rust license report is missing a license text")
    files["THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"] = (
        json.dumps(report, indent=2, sort_keys=True) + "\n").encode()
    if not any(name.startswith("THIRD_PARTY_LICENSES/sdk-cousine/") for name in files):
        raise ArchiveVerificationError("SDK legal scope is missing the bundled Cousine license")


def package_sdk(*, repo_root: Path, profile: str, target: str, artifact_dir: Path,
                output_dir: Path, runtime_catalog: Path | None = None,
                source_commit: str | None = None, descriptor: Path | None = None,
                rust_license_report: Path | None = None) -> Path:
    repo_root = require_repository_root(Path(repo_root))
    original_artifact_dir = Path(artifact_dir).absolute()
    artifact_dir = require_repository_root(original_artifact_dir)
    require_clean_source_tree(repo_root)
    descriptor = Path(descriptor) if descriptor else repo_root / "capabilities/artifact-profiles-v2.json"
    require_regular_input(descriptor, "artifact descriptor")
    recipe = _recipe(profile, target, descriptor, repo_root)
    profile_document = next(item for item in _json(descriptor)["profiles"] if item["id"] == profile)
    source_commit = resolve_source_commit(repo_root, source_commit)
    version = _workspace_version(repo_root)
    names = library_names(target)
    binaries = {kind: _input(artifact_dir, name) for kind, name in names.items()}
    for path in binaries.values():
        if not path.stat().st_size:
            raise ArchiveVerificationError(f"empty built artifact: {path}")
    catalog_path = Path(runtime_catalog) if runtime_catalog else artifact_dir / "runtime-catalog.json"
    catalog_path = _artifact_input(artifact_dir, original_artifact_dir, catalog_path)
    sidecar = _json(catalog_path)
    catalog = _catalog(sidecar, profile=profile, target=target, commit=source_commit,
                       version=version, expected=profile_document.get("expected"), binaries=binaries)

    files: dict[str, bytes] = {}

    def add(source_root, relative, destination=None):
        destination = _member_path(destination or relative)
        if destination in files:
            raise ArchiveVerificationError(f"duplicate SDK payload member: {destination}")
        files[destination] = _input(source_root, relative).read_bytes()

    layout = library_layout(target)
    for kind, name in names.items():
        add(artifact_dir, name, layout[kind])
    for name, digest in sidecar["artifacts"].items():
        destination = next(layout[kind] for kind, filename in names.items() if filename == name)
        if hashlib.sha256(files[destination]).hexdigest() != digest:
            raise ArchiveVerificationError("built artifact changed while preparing the SDK archive")
    add(repo_root, "distribution/sdk/README.md", "README.md")
    files["runtime-catalog.json"] = (json.dumps(sidecar, indent=2, sort_keys=True) + "\n").encode()
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        add(repo_root, name)
    report_path = Path(rust_license_report) if rust_license_report else artifact_dir / "rust-cargo-dependencies.json"
    report_path = _artifact_input(artifact_dir, original_artifact_dir, report_path)
    _legal_payload(files, repo_root, profile, report_path, descriptor)
    flavor = profile.rsplit("-", 1)[1]
    legal_names = "\n".join(name.lower() for name in files if name.startswith("THIRD_PARTY_LICENSES/"))
    if flavor != "full" and any(name in legal_names for name in ("eclipse-elk/", "elkjs/")):
        raise ArchiveVerificationError("non-full SDK legal projection contains ELK material")
    if flavor == "standard" and any(name in legal_names for name in ("ratex/", "katex-fonts/")):
        raise ArchiveVerificationError("standard SDK legal projection contains math material")
    if flavor == "full" and "eclipse-elk/" not in legal_names:
        raise ArchiveVerificationError("full SDK is missing EPL/ELK legal material")

    font_root = "crates/merman-export/assets/fonts"
    fonts = _json(_input(repo_root, f"{font_root}/manifest.json"))
    entries = fonts.get("files")
    if not isinstance(entries, list) or not entries:
        raise ArchiveVerificationError("font manifest must list production resources")
    font_names = set()
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            raise ArchiveVerificationError("font manifest has an invalid file entry")
        name = _member_path(entry["path"])
        if "/" in name or name in font_names or Path(name).suffix not in {".ttf", ".otf", ".txt"}:
            raise ArchiveVerificationError(f"unexpected font resource: {name!r}")
        font_names.add(name)
        font = _input(repo_root, f"{font_root}/{name}")
        if not isinstance(entry.get("sha256"), str) or not SHA256.fullmatch(entry["sha256"]) or sha256_file(font) != entry["sha256"]:
            raise ArchiveVerificationError(f"font checksum mismatch: {name}")
        add(repo_root, f"{font_root}/{name}", f"fonts/{name}")
        if hashlib.sha256(files[f"fonts/{name}"]).hexdigest() != entry["sha256"]:
            raise ArchiveVerificationError(f"font resource changed during SDK preparation: {name}")
    if "OFL.txt" not in font_names or not any(name.endswith((".ttf", ".otf")) for name in font_names):
        raise ArchiveVerificationError("font manifest must include font bytes and OFL.txt")
    add(repo_root, f"{font_root}/manifest.json", "fonts/manifest.json")

    wasi = target == WASI_TARGET
    if not wasi:
        contract_path = "contracts/abi/merman-v3.json"
        contract = _json(_input(repo_root, contract_path))
        if type(contract.get("abi_version")) is not int or contract["abi_version"] != catalog["transport_api_version"]:
            raise ArchiveVerificationError("packaged ABI contract disagrees with artifact catalog")
        add(repo_root, contract_path)
        files.update(native_consumer_files(repo_root, target, version))
        if _header_version(files["include/merman.h"], "MERMAN_NATIVE_ABI_VERSION") != catalog["transport_api_version"]:
            raise ArchiveVerificationError("native header ABI version disagrees with artifact catalog")
        if _header_version(files["include/merman_resource_contract.h"], "MERMAN_BINDING_OPTIONS_SCHEMA_VERSION") not in catalog["options_schema_versions"]:
            raise ArchiveVerificationError("native header options schema disagrees with artifact catalog")
        measurement_path = "contracts/abi/text-measurement-v1.json"
        measurement = _json(_input(repo_root, measurement_path))
        if _header_version(files["include/merman_text_measurement_abi.h"], "MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION") != measurement.get("protocol_version"):
            raise ArchiveVerificationError("text measurement header disagrees with its protocol contract")
        add(repo_root, measurement_path)
    else:
        add(repo_root, "crates/merman-wasi/README.md", "docs/WASI_ABI.md")
        for name in WASI_EXAMPLE_FILES:
            add(repo_root, "distribution/sdk/examples/" + name, "examples/" + name)
    source_paths = [
        "Cargo.lock", recipe.manifest, "crates/merman", "crates/merman-model",
        "crates/merman-markup", "crates/merman-render", "crates/merman-export",
    ]
    if flavor == "full":
        source_paths.append("crates/merman-layout-elk")
    availability = (f"# Corresponding source\n\nSource commit: `{source_commit}`.\n\n"
                    f"Repository: https://github.com/Latias94/merman/tree/{source_commit}\n\n"
                    "Retrieve the exact source, not a moving branch:\n\n```text\n"
                    "git clone https://github.com/Latias94/merman.git\n"
                    f"git -C merman checkout --detach {source_commit}\n```\n\n"
                    + "\n".join(f"- `{path}`" for path in source_paths) + "\n\n"
                    "Full profiles include the Rust ELK implementation in `crates/merman-layout-elk`; "
                    "its corresponding source and EPL notices remain available at the exact commit above. "
                    "Third-party source references and obligations are recorded in THIRD_PARTY_NOTICES.md "
                    "and THIRD_PARTY_LICENSES/. This is a source retrieval guide, not a claim of source-archive verification.\n")
    files["SOURCE_AVAILABILITY.md"] = availability.encode()
    manifest = {
        "schema_version": 1, "version": version, "source_commit": source_commit,
        "target": target, "profile": profile, "abi_version": catalog["transport_api_version"],
        "options_schema_versions": catalog["options_schema_versions"],
        "capabilities": catalog["capabilities"]["capability_ids"],
        "text_measurement": catalog["capabilities"]["text_measurement"],
        "constructor_service_ids": catalog["constructor_service_ids"],
        "outputs": catalog["capabilities"]["output_ids"],
        "recipe": {"package": recipe.package, "cargo_profile": recipe.cargo_profile,
                   "features": list(recipe.features), "default_features": recipe.default_features,
                   "target_name": recipe.target_name, "build_target": target},
        "files": {name: {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}
                  for name, data in sorted(files.items())},
    }
    files["manifest.json"] = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    _validate_member_set([ArchiveMember(name, _member_path(name), False, len(data), 0o644, None)
                          for name, data in files.items()], limits=SDK_LIMITS)
    output_dir = Path(output_dir)
    if output_dir.is_symlink():
        raise ArchiveVerificationError("SDK output directory must not be a symlink")
    output_dir.mkdir(parents=True, exist_ok=True)
    output_dir = output_dir.resolve()
    archive_name = release_archive_name_for(f"merman-{profile}", target)
    archive_path = output_dir / archive_name
    checksum_path = output_dir / f"{archive_name}.sha256"
    if archive_path.exists() or archive_path.is_symlink() or checksum_path.exists() or checksum_path.is_symlink():
        raise ArchiveVerificationError("SDK output already exists; refusing to replace it")
    with tempfile.TemporaryDirectory(prefix=".sdk-", dir=output_dir) as temporary:
        staged = Path(temporary) / archive_name
        if target.endswith("windows-msvc"):
            with zipfile.ZipFile(staged, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for name, data in sorted(files.items()):
                    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                    info.create_system = 3
                    info.external_attr = 0o100644 << 16
                    archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
        else:
            wrapper = archive_name.removesuffix(".tar.xz")
            with tarfile.open(staged, "w:xz") as archive:
                for name, data in sorted(files.items()):
                    info = tarfile.TarInfo(f"{wrapper}/{name}")
                    info.size = len(data)
                    info.mode = 0o644
                    info.mtime = 0
                    archive.addfile(info, io.BytesIO(data))
        if staged.stat().st_size > SDK_LIMITS.max_archive_size:
            raise ArchiveVerificationError("SDK archive exceeds compressed size budget")
        digest = sha256_file(staged)
        with archive_path.open("xb") as destination, staged.open("rb") as source:
            shutil.copyfileobj(source, destination)
        with checksum_path.open("x", encoding="ascii") as stream:
            stream.write(f"{digest}  {archive_name}\n")
    return archive_path


def verify_sdk_archive(*, repo_root: Path, profile: str, target: str, archive: Path,
                       checksum: Path, source_commit: str, version: str,
                       descriptor: Path | None = None,
                       verified_output_dir: Path | None = None) -> Path:
    repo_root = require_repository_root(Path(repo_root))
    descriptor = Path(descriptor) if descriptor else repo_root / "capabilities/artifact-profiles-v2.json"
    _recipe(profile, target, descriptor, repo_root)
    document = next(item for item in _json(descriptor)["profiles"] if item["id"] == profile)
    source_commit = resolve_source_commit(repo_root, source_commit)
    if version != _workspace_version(repo_root):
        raise ArchiveVerificationError("SDK archive version differs from its pinned source")
    with verified_archive_contents(
        Path(archive), Path(checksum), package_name=f"merman-{profile}", target=target, version=version,
        limits=SDK_LIMITS,
    ) as extracted:
        manifest = _json(_input(extracted.root, "manifest.json"))
        for key, value in (("schema_version", 1), ("version", version), ("source_commit", source_commit),
                           ("profile", profile), ("target", target)):
            if type(manifest.get(key)) is not type(value) or manifest[key] != value:
                raise ArchiveVerificationError(f"SDK archive manifest {key} mismatch")
        declared = manifest.get("files")
        if not isinstance(declared, dict):
            raise ArchiveVerificationError("SDK archive manifest must enumerate payload files")
        observed = {_member_path(member.logical_path) for member in extracted.members if not member.is_directory}
        if set(declared) != observed - {"manifest.json"}:
            raise ArchiveVerificationError("SDK archive manifest member set mismatch")
        for name, details in declared.items():
            path = _input(extracted.root, name)
            if (not isinstance(details, dict) or type(details.get("size")) is not int or
                    details != {"size": path.stat().st_size, "sha256": sha256_file(path)}):
                raise ArchiveVerificationError(f"SDK archive payload checksum mismatch: {name}")
        binaries = {kind: _input(extracted.root, path)
                    for kind, path in library_layout(target).items()}
        catalog = _catalog(_json(_input(extracted.root, "runtime-catalog.json")),
                           profile=profile, target=target, commit=source_commit, version=version,
                           expected=document.get("expected"), binaries=binaries)
        for key, value in (("abi_version", catalog["transport_api_version"]),
                           ("capabilities", catalog["capabilities"]["capability_ids"]),
                           ("text_measurement", catalog["capabilities"]["text_measurement"]),
                           ("constructor_service_ids", catalog["constructor_service_ids"]),
                           ("options_schema_versions", catalog["options_schema_versions"]),
                           ("outputs", catalog["capabilities"]["output_ids"])):
            if manifest.get(key) != value:
                raise ArchiveVerificationError(f"SDK archive manifest {key} differs from actual catalog")
        if verified_output_dir is not None:
            destination = persist_verified_archive(
                extracted, Path(verified_output_dir) / extracted.archive.name, limits=SDK_LIMITS)
            with Path(str(destination) + ".sha256").open("x", encoding="ascii") as stream:
                stream.write(f"{extracted.digest}  {destination.name}\n")
            return destination
        return Path(archive).resolve()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("package", "build-command", "catalog-sidecar", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--profile", choices=PROFILES, required=True)
        command.add_argument("--target", required=True)
        command.add_argument("--repo-root", type=Path, default=REPO_ROOT)
        command.add_argument("--descriptor", type=Path)
        if name == "package":
            command.add_argument("--artifact-dir", type=Path, required=True)
            command.add_argument("--output-dir", type=Path, required=True)
            command.add_argument("--runtime-catalog", type=Path)
            command.add_argument("--rust-license-report", type=Path)
            command.add_argument("--source-commit")
        elif name == "catalog-sidecar":
            command.add_argument("--artifact-dir", type=Path, required=True)
            command.add_argument("--catalog-in", type=Path, required=True)
            command.add_argument("--output", type=Path)
            command.add_argument("--source-commit")
        elif name == "verify":
            command.add_argument("--archive", type=Path, required=True)
            command.add_argument("--checksum", type=Path, required=True)
            command.add_argument("--source-commit", required=True)
            command.add_argument("--version", required=True)
            command.add_argument("--verified-output-dir", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "build-command":
            command = build_command(args.profile, args.target,
                                    args.descriptor or args.repo_root / "capabilities/artifact-profiles-v2.json",
                                    args.repo_root)
            print(json.dumps(command))
        else:
            values = vars(args)
            action = values.pop("command")
            handler = {"package": package_sdk, "catalog-sidecar": write_catalog_sidecar,
                       "verify": verify_sdk_archive}[action]
            print(handler(**values))
        return 0
    except (ArchiveVerificationError, RuntimeError, OSError, ValueError, KeyError) as error:
        print(f"SDK archive error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
