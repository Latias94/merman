#!/usr/bin/env python3
"""Run local platform binding verification gates."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from artifact_profile_recipe import (
    CargoArtifactRecipe,
    cargo_build_args as project_cargo_build_args,
    load_artifact_profile,
    rustc_host_target,
)
from native_symbol_contract import (
    ANDROID_JNI_SYMBOL_CONTRACT,
    assert_symbol_contract,
    read_defined_dynamic_symbols,
)
ANDROID_NATIVE_RECIPE = load_artifact_profile("android-native")
FLUTTER_ANDROID_NATIVE_RECIPE = load_artifact_profile("flutter-android-native")
FLUTTER_DESKTOP_NATIVE_RECIPE = load_artifact_profile("flutter-desktop-native")
FLUTTER_ROOT = REPO_ROOT / "platforms" / "flutter"
ANDROID_ROOT = REPO_ROOT / "platforms" / "android"
ANDROID_RELEASE_AAR = ANDROID_ROOT / "build" / "outputs" / "aar" / "merman-android-release.aar"
ANDROID_MAVEN_MODULE_ROOT = (
    ANDROID_ROOT / "build" / "repo" / "io" / "merman" / "merman-android"
)
ANDROID_PACKAGING_SENTINEL_CLASSES = [
    "io/merman/Merman.class",
    "io/merman/MermanEngine.class",
    "io/merman/MermanEngineServices.class",
    "io/merman/MermanIconPack.class",
    "io/merman/MermanIconPackSet.class",
    "io/merman/MermanOperationMetadata.class",
    "io/merman/MermanOperationResult.class",
    "io/merman/MermanOutputPlan.class",
    "io/merman/MermanPdfFilterImagesOutputPlan.class",
    "io/merman/MermanRasterOutputPlan.class",
    "io/merman/MermanResourceOptions.class",
    "io/merman/MermanTextMeasurer.class",
    "io/merman/MermanUnknownOutputPlan.class",
]
ANDROID_FORBIDDEN_PACKAGING_CLASSES = [
    "io/merman/MermanIconRegistry.class",
]
ANDROID_FORBIDDEN_JAVADOC_ENTRIES = {
    "merman-android/io.merman/-merman-engine-services/icon-registry.html",
    "merman-android/io.merman/-merman-icon-registry/index.html",
}
ANDROID_NATIVE_LIBRARIES = [
    "jni/arm64-v8a/libmerman_android_jni.so",
    "jni/x86_64/libmerman_android_jni.so",
]
ANDROID_MAVEN_COORDINATES = ("io.merman", "merman-android")
ANDROID_MAVEN_LICENSES = {
    ("MIT License", "https://opensource.org/license/mit", "repo"),
    (
        "Apache License, Version 2.0",
        "https://www.apache.org/licenses/LICENSE-2.0",
        "repo",
    ),
}
ANDROID_MAVEN_DEVELOPER = (
    "frankorz",
    "Mingzhen Zhuang",
    "superfrankie621@gmail.com",
)
ANDROID_MAVEN_SCM = (
    "scm:git:https://github.com/Latias94/merman.git",
    "scm:git:ssh://git@github.com/Latias94/merman.git",
    "https://github.com/Latias94/merman",
)
FLUTTER_GENERATED_ABI = (
    FLUTTER_ROOT / "lib" / "src" / "generated" / "native_abi.dart"
)
def cargo_android_clippy_args(
    recipe: CargoArtifactRecipe,
    target: str,
) -> list[str]:
    if recipe.build_target_kind != "target-set" or target not in recipe.build_targets:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} does not declare Android target {target!r}"
        )
    args = [
        "cargo",
        "clippy",
        "--locked",
        "--profile",
        recipe.cargo_profile,
        "--package",
        recipe.package,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
        "--lib",
        "--no-deps",
    ]
    if not recipe.default_features:
        args.append("--no-default-features")
    if recipe.features:
        args.extend(["--features", recipe.feature_argument])
    args.extend(["--target", target])
    return args


def flutter_format_paths(root: Path = FLUTTER_ROOT) -> list[str]:
    generated_root = root / "lib" / "src" / "generated"
    paths = [
        path.relative_to(root).as_posix()
        for source_root in (root / "lib", root / "example", root / "tool")
        for path in sorted(source_root.rglob("*.dart"))
        if not path.is_relative_to(generated_root)
    ]
    if not paths:
        raise RuntimeError("Flutter format contract found no handwritten Dart sources")
    return paths


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--verify-android-aar",
        action="store_true",
        help="Verify the assembled Android AAR contract and exit.",
    )
    parser.add_argument(
        "--verify-android-maven",
        action="store_true",
        help="Verify the staged Android Maven publication contract and exit.",
    )
    parser.add_argument(
        "--only-android-instrumentation-smoke",
        action="store_true",
        help="Build missing Android native slices and run only the Android instrumentation smoke.",
    )
    parser.add_argument("--gradle-path", default=os.environ.get("MERMAN_GRADLE"))
    parser.add_argument(
        "--android-ndk-home",
        default=os.environ.get("ANDROID_NDK_HOME") or os.environ.get("ANDROID_NDK_ROOT"),
        help="Android NDK used for fail-closed AAR dynamic-symbol inspection.",
    )
    return parser.parse_args()


def step(name: str) -> None:
    print()
    print(f"==> {name}")


def run(args: list[str], *, cwd: Path = REPO_ROOT) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, check=True)


def run_capture(args: list[str], *, cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(args))
    completed = subprocess.run(
        args,
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.stderr:
        print(completed.stderr, end="", file=sys.stderr)
    completed.check_returncode()
    return completed


def verify_tracked_generated_file(path: Path) -> None:
    try:
        relative = path.resolve().relative_to(REPO_ROOT.resolve()).as_posix()
    except ValueError as exc:
        raise RuntimeError(
            f"generated file must be inside the repository: {path}"
        ) from exc
    if not path.is_file():
        raise RuntimeError(f"generated file does not exist: {relative}")
    run(["git", "ls-files", "--error-unmatch", "--", relative])
    run(["git", "diff", "--exit-code", "--", relative])


def require_command(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise RuntimeError(f"{name} not found on PATH")
    return path


def resolve_gradle_command(path: str | None) -> str:
    if path:
        command = shutil.which(path)
        if command:
            return command
        resolved = Path(path).expanduser().resolve()
        if resolved.is_dir():
            gradle_bat = resolved / "gradle.bat"
            if gradle_bat.exists():
                return str(gradle_bat)
            gradle = resolved / "gradle"
            if gradle.exists():
                return str(gradle)
            raise RuntimeError(f"Gradle executable not found under: {resolved}")
        if not resolved.exists():
            raise RuntimeError(f"Gradle executable not found: {resolved}")
        return str(resolved)

    wrapper_name = "gradlew.bat" if os.name == "nt" else "gradlew"
    wrapper = ANDROID_ROOT / wrapper_name
    if wrapper.is_file():
        return str(wrapper)

    gradle = shutil.which("gradle")
    if not gradle:
        raise RuntimeError(
            "Android Gradle wrapper not found and gradle is not on PATH. "
            "Pass --gradle-path or set MERMAN_GRADLE."
        )
    return gradle


def android_jni_libs_ready() -> bool:
    return all(
        path.exists()
        for path in [
            ANDROID_ROOT
            / "src"
            / "main"
            / "jniLibs"
            / "arm64-v8a"
            / "libmerman_android_jni.so",
            ANDROID_ROOT
            / "src"
            / "main"
            / "jniLibs"
            / "x86_64"
            / "libmerman_android_jni.so",
        ]
    )


def android_ndk_build_args(ndk_home: str | Path | None) -> list[str]:
    if ndk_home is None:
        return []
    return ["--ndk-home", str(Path(ndk_home).expanduser().resolve())]


def ensure_android_native_slices(ndk_home: str | Path | None = None) -> None:
    if android_jni_libs_ready():
        return
    step("Android native slices")
    run(
        [
            sys.executable,
            str(ANDROID_ROOT / "build-android.py"),
            *android_ndk_build_args(ndk_home),
            "--targets",
            "aarch64-linux-android",
            "x86_64-linux-android",
        ]
    )


def run_android_instrumentation_smoke(
    gradle_path: str | None,
    ndk_home: str | Path | None = None,
) -> None:
    ensure_android_native_slices(ndk_home)
    gradle = resolve_gradle_command(gradle_path)
    run([gradle, "-p", str(ANDROID_ROOT), "connectedAndroidTest", "--stacktrace"])


def android_expected_resource_entries(android_root: Path = ANDROID_ROOT) -> list[str]:
    resources_root = android_root / "src" / "main" / "resources"
    if not resources_root.is_dir():
        raise RuntimeError(f"Android resource directory not found: {resources_root}")
    return sorted(
        path.relative_to(resources_root).as_posix()
        for path in resources_root.rglob("*")
        if path.is_file()
    )


def resolve_android_llvm_nm(ndk_home: str | Path | None = None) -> Path:
    if ndk_home is None:
        ndk_home = os.environ.get("ANDROID_NDK_HOME") or os.environ.get("ANDROID_NDK_ROOT")
        if ndk_home is None:
            raise RuntimeError(
                "Android NDK is required for native symbol verification; pass "
                "--android-ndk-home or set ANDROID_NDK_HOME"
            )
    ndk = Path(ndk_home).expanduser().resolve()
    executable = "llvm-nm.exe" if os.name == "nt" else "llvm-nm"
    candidates = sorted(
        path
        for path in (ndk / "toolchains" / "llvm" / "prebuilt").glob(
            f"*/bin/{executable}"
        )
        if path.is_file()
    )
    if len(candidates) != 1:
        rendered = ", ".join(str(path) for path in candidates) or "none"
        raise RuntimeError(
            f"expected exactly one Android NDK {executable}, found: {rendered}"
        )
    return candidates[0]


def android_library_symbols(library: Path, llvm_nm: Path | None = None) -> set[str]:
    tool = resolve_android_llvm_nm() if llvm_nm is None else llvm_nm
    return read_defined_dynamic_symbols(library, tool)


def android_class_api(classes_jar: bytes, class_name: str) -> str:
    javap = shutil.which("javap")
    if javap is None:
        raise RuntimeError("Android AAR verification requires javap from a JDK")
    with tempfile.TemporaryDirectory(prefix="merman-android-javap-") as temp_dir:
        jar_path = Path(temp_dir) / "classes.jar"
        jar_path.write_bytes(classes_jar)
        completed = subprocess.run(
            [javap, "-classpath", str(jar_path), "-public", class_name],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        suffix = f": {detail}" if detail else ""
        raise RuntimeError(f"javap failed for Android class {class_name}{suffix}")
    return completed.stdout


def _assert_android_engine_services_api(classes_jar: bytes) -> None:
    api = android_class_api(classes_jar, "io.merman.MermanEngineServices")
    if "getIconPackSet();" not in api:
        raise RuntimeError(
            "Android release AAR MermanEngineServices is missing getIconPackSet"
        )
    if "getIconRegistry();" in api:
        raise RuntimeError(
            "Android release AAR MermanEngineServices retains removed getIconRegistry"
        )


def assert_android_aar_contract(
    aar_path: Path = ANDROID_RELEASE_AAR,
    android_root: Path = ANDROID_ROOT,
    llvm_nm: Path | None = None,
) -> None:
    if not aar_path.exists():
        raise RuntimeError(f"Android release AAR not found: {aar_path}")

    with zipfile.ZipFile(aar_path) as aar:
        aar_names = set(aar.namelist())
        try:
            classes_jar = aar.read("classes.jar")
        except KeyError as error:
            raise RuntimeError(f"Android release AAR is missing classes.jar: {aar_path}") from error

    missing_native = [name for name in ANDROID_NATIVE_LIBRARIES if name not in aar_names]
    if missing_native:
        raise RuntimeError(
            "Android release AAR is missing native libraries: " + ", ".join(missing_native)
        )

    expected_native = set(ANDROID_NATIVE_LIBRARIES)
    actual_merman_native = {
        name
        for name in aar_names
        if name.startswith("jni/")
        and Path(name).name.startswith("libmerman_")
        and name.endswith(".so")
    }
    unexpected_native = sorted(actual_merman_native - expected_native)
    if unexpected_native:
        raise RuntimeError(
            "Android release AAR contains unexpected Merman native libraries: "
            + ", ".join(unexpected_native)
        )

    with tempfile.TemporaryDirectory(prefix="merman-android-symbols-") as temp_dir:
        temp_root = Path(temp_dir)
        with zipfile.ZipFile(aar_path) as aar:
            for entry in ANDROID_NATIVE_LIBRARIES:
                library = temp_root / entry
                library.parent.mkdir(parents=True, exist_ok=True)
                library.write_bytes(aar.read(entry))
                assert_symbol_contract(
                    android_library_symbols(library, llvm_nm),
                    ANDROID_JNI_SYMBOL_CONTRACT,
                    label=f"Android AAR {entry}",
                )

    with zipfile.ZipFile(io.BytesIO(classes_jar)) as jar:
        names = set(jar.namelist())

    missing_sentinels = [
        class_name
        for class_name in ANDROID_PACKAGING_SENTINEL_CLASSES
        if class_name not in names
    ]
    if missing_sentinels:
        raise RuntimeError(
            "Android release AAR is missing Kotlin packaging sentinels: "
            + ", ".join(missing_sentinels)
        )
    forbidden_classes = sorted(
        class_name
        for class_name in ANDROID_FORBIDDEN_PACKAGING_CLASSES
        if class_name in names
    )
    if forbidden_classes:
        raise RuntimeError(
            "Android release AAR contains removed Kotlin API classes: "
            + ", ".join(forbidden_classes)
        )
    _assert_android_engine_services_api(classes_jar)

    missing_resources = [
        name for name in android_expected_resource_entries(android_root) if name not in names
    ]
    if missing_resources:
        raise RuntimeError(
            "Android release AAR is missing projected resources: "
            + ", ".join(missing_resources)
        )


def _single_path(paths: list[Path], description: str) -> Path:
    if len(paths) != 1:
        rendered = ", ".join(str(path) for path in paths) or "none"
        raise RuntimeError(f"Expected exactly one {description}, found: {rendered}")
    return paths[0]


def _xml_text(parent: ET.Element, child_name: str) -> str:
    child = parent.find(f"{{*}}{child_name}")
    return (child.text or "").strip() if child is not None else ""


def _require_xml_child(parent: ET.Element, child_name: str, context: str) -> ET.Element:
    child = parent.find(f"{{*}}{child_name}")
    if child is None:
        raise RuntimeError(f"Android Maven POM is missing {context}/{child_name}")
    return child


def _sha256_and_size(artifact: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with artifact.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def _assert_sha256(artifact: Path) -> tuple[str, int]:
    checksum_path = artifact.with_name(f"{artifact.name}.sha256")
    if not checksum_path.is_file():
        raise RuntimeError(f"Android Maven artifact is missing SHA-256: {checksum_path}")
    expected = checksum_path.read_text(encoding="ascii").strip().lower()
    actual, size = _sha256_and_size(artifact)
    if expected != actual:
        raise RuntimeError(
            f"Android Maven SHA-256 mismatch for {artifact.name}: "
            f"expected {expected}, computed {actual}"
        )
    return actual, size


def _assert_android_maven_pom(pom_path: Path, version: str) -> None:
    root = ET.parse(pom_path).getroot()
    group_id, artifact_id = ANDROID_MAVEN_COORDINATES
    expected_fields = {
        "groupId": group_id,
        "artifactId": artifact_id,
        "version": version,
        "packaging": "aar",
        "name": artifact_id,
        "description": "Android JNI bindings for merman headless Mermaid rendering.",
        "url": "https://github.com/Latias94/merman",
    }
    for field, expected in expected_fields.items():
        actual = _xml_text(root, field)
        if actual != expected:
            raise RuntimeError(
                f"Android Maven POM {field} mismatch: expected {expected!r}, got {actual!r}"
            )

    licenses = _require_xml_child(root, "licenses", "project")
    actual_licenses = {
        (
            _xml_text(license_node, "name"),
            _xml_text(license_node, "url"),
            _xml_text(license_node, "distribution"),
        )
        for license_node in licenses.findall("{*}license")
    }
    if actual_licenses != ANDROID_MAVEN_LICENSES:
        raise RuntimeError(
            "Android Maven POM license metadata mismatch: "
            f"expected {sorted(ANDROID_MAVEN_LICENSES)!r}, got {sorted(actual_licenses)!r}"
        )

    developers = _require_xml_child(root, "developers", "project")
    developer = _single_path(
        list(developers.findall("{*}developer")),
        "Android Maven POM developer",
    )
    actual_developer = tuple(
        _xml_text(developer, field) for field in ("id", "name", "email")
    )
    if actual_developer != ANDROID_MAVEN_DEVELOPER:
        raise RuntimeError(
            "Android Maven POM developer metadata mismatch: "
            f"expected {ANDROID_MAVEN_DEVELOPER!r}, got {actual_developer!r}"
        )

    scm = _require_xml_child(root, "scm", "project")
    actual_scm = tuple(
        _xml_text(scm, field) for field in ("connection", "developerConnection", "url")
    )
    if actual_scm != ANDROID_MAVEN_SCM:
        raise RuntimeError(
            "Android Maven POM SCM metadata mismatch: "
            f"expected {ANDROID_MAVEN_SCM!r}, got {actual_scm!r}"
        )

    dependencies = root.find("{*}dependencies")
    kotlin_dependencies = [] if dependencies is None else [
        dependency
        for dependency in dependencies.findall("{*}dependency")
        if (
            _xml_text(dependency, "groupId"),
            _xml_text(dependency, "artifactId"),
        )
        == ("org.jetbrains.kotlin", "kotlin-stdlib")
    ]
    kotlin_dependency = _single_path(
        kotlin_dependencies,
        "Kotlin standard-library dependency in the Android Maven POM",
    )
    if not _xml_text(kotlin_dependency, "version"):
        raise RuntimeError("Android Maven POM Kotlin dependency has no version")
    if _xml_text(kotlin_dependency, "scope") != "compile":
        raise RuntimeError("Android Maven POM Kotlin dependency must use compile scope")


def _assert_android_source_jar(source_jar: Path, android_root: Path) -> None:
    source_root = android_root / "src" / "main" / "kotlin"
    expected_sources = {
        path.relative_to(source_root).as_posix()
        for path in source_root.rglob("*.kt")
    }
    with zipfile.ZipFile(source_jar) as archive:
        actual_sources = {name for name in archive.namelist() if name.endswith(".kt")}
    if actual_sources != expected_sources:
        raise RuntimeError(
            "Android Maven sources JAR does not match the public Kotlin source set: "
            f"expected {sorted(expected_sources)!r}, got {sorted(actual_sources)!r}"
        )


def _assert_android_javadoc_jar(javadoc_jar: Path) -> None:
    required_entries = {
        "index.html",
        "merman-android/package-list",
        "merman-android/io.merman/index.html",
        "merman-android/io.merman/-merman-engine-services/index.html",
        "merman-android/io.merman/-merman-engine-services/icon-pack-set.html",
        "merman-android/io.merman/-merman-icon-pack-set/index.html",
    }
    with zipfile.ZipFile(javadoc_jar) as archive:
        names = set(archive.namelist())
    missing = sorted(required_entries - names)
    if missing:
        raise RuntimeError(
            "Android Maven javadoc JAR is missing generated API documentation: "
            + ", ".join(missing)
        )
    removed = sorted(ANDROID_FORBIDDEN_JAVADOC_ENTRIES & names)
    if removed:
        raise RuntimeError(
            "Android Maven javadoc JAR contains removed API documentation: "
            + ", ".join(removed)
        )


def _assert_android_gradle_module(
    module_path: Path,
    version: str,
    published_artifacts: dict[str, tuple[str, int]],
) -> None:
    module = json.loads(module_path.read_text(encoding="utf-8"))
    group_id, artifact_id = ANDROID_MAVEN_COORDINATES
    expected_component = {
        "group": group_id,
        "module": artifact_id,
        "version": version,
    }
    component = module.get("component", {})
    for field, expected in expected_component.items():
        if component.get(field) != expected:
            raise RuntimeError(
                f"Android Gradle module component.{field} mismatch: "
                f"expected {expected!r}, got {component.get(field)!r}"
            )

    variant_files: dict[str, list[dict[str, object]]] = {}
    documentation_types: set[str] = set()
    has_aar_variant = False
    for variant in module.get("variants", []):
        attributes = variant.get("attributes", {})
        if attributes.get("org.gradle.libraryelements") == "aar":
            has_aar_variant = True
        docs_type = attributes.get("org.gradle.docstype")
        if isinstance(docs_type, str):
            documentation_types.add(docs_type)
        for file_entry in variant.get("files", []):
            name = file_entry.get("name")
            if isinstance(name, str):
                variant_files.setdefault(name, []).append(file_entry)

    if not has_aar_variant:
        raise RuntimeError("Android Gradle module has no AAR library variant")
    missing_documentation = {"sources", "javadoc"} - documentation_types
    if missing_documentation:
        raise RuntimeError(
            "Android Gradle module is missing documentation variants: "
            + ", ".join(sorted(missing_documentation))
        )

    for name, (actual_sha256, actual_size) in published_artifacts.items():
        entries = variant_files.get(name, [])
        if not entries:
            raise RuntimeError(f"Android Gradle module does not declare artifact: {name}")
        for entry in entries:
            if entry.get("sha256") != actual_sha256 or entry.get("size") != actual_size:
                raise RuntimeError(
                    f"Android Gradle module digest or size mismatch for artifact: {name}"
                )


def assert_android_maven_publication(
    module_root: Path = ANDROID_MAVEN_MODULE_ROOT,
    android_root: Path = ANDROID_ROOT,
    llvm_nm: Path | None = None,
) -> Path:
    if not module_root.is_dir():
        raise RuntimeError(f"Android Maven module repository not found: {module_root}")
    version_dir = _single_path(
        sorted(path for path in module_root.iterdir() if path.is_dir()),
        "Android Maven publication version directory",
    )
    version = version_dir.name
    _, artifact_id = ANDROID_MAVEN_COORDINATES
    base_name = f"{artifact_id}-{version}"
    artifacts = {
        "pom": version_dir / f"{base_name}.pom",
        "module": version_dir / f"{base_name}.module",
        "aar": version_dir / f"{base_name}.aar",
        "sources": version_dir / f"{base_name}-sources.jar",
        "javadoc": version_dir / f"{base_name}-javadoc.jar",
    }
    verified_artifacts = {}
    for kind, artifact in artifacts.items():
        if not artifact.is_file():
            raise RuntimeError(f"Android Maven publication is missing artifact: {artifact}")
        verified_artifacts[kind] = _assert_sha256(artifact)

    _assert_android_maven_pom(artifacts["pom"], version)
    assert_android_aar_contract(artifacts["aar"], android_root, llvm_nm)
    _assert_android_source_jar(artifacts["sources"], android_root)
    _assert_android_javadoc_jar(artifacts["javadoc"])
    _assert_android_gradle_module(
        artifacts["module"],
        version,
        {
            artifacts["aar"].name: verified_artifacts["aar"],
            artifacts["sources"].name: verified_artifacts["sources"],
            artifacts["javadoc"].name: verified_artifacts["javadoc"],
        },
    )
    return version_dir


def cargo_dynamic_library(cargo_stdout: str, recipe: CargoArtifactRecipe) -> Path:
    libraries: set[Path] = set()
    for raw in cargo_stdout.splitlines():
        try:
            message = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(message, dict) or message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        if not isinstance(target, dict) or target.get("name") != recipe.target_name:
            continue
        crate_types = target.get("crate_types")
        if not isinstance(crate_types, list) or "cdylib" not in crate_types:
            continue
        filenames = message.get("filenames")
        if not isinstance(filenames, list):
            continue
        libraries.update(
            Path(filename)
            for filename in filenames
            if isinstance(filename, str)
            and Path(filename).suffix.lower() in {".dll", ".dylib", ".so"}
        )
    if len(libraries) != 1:
        rendered = ", ".join(sorted(str(path) for path in libraries)) or "none"
        raise RuntimeError(
            f"Expected exactly one Cargo cdylib artifact for {recipe.target_name}, found: {rendered}"
        )
    library = next(iter(libraries))
    if not library.is_file():
        raise RuntimeError(f"Cargo reported a missing cdylib artifact: {library}")
    return library


def run_dart_ffi_native_smoke(
    dart: str,
    recipe: CargoArtifactRecipe = FLUTTER_DESKTOP_NATIVE_RECIPE,
    *,
    target: str | None = None,
) -> None:
    selected_target = rustc_host_target() if target is None else target
    build_args = project_cargo_build_args(recipe, locked=True, target=selected_target)
    build_args.append("--message-format=json-render-diagnostics")
    build = run_capture(build_args)
    library = cargo_dynamic_library(
        build.stdout,
        recipe,
    )
    run(
        [
            dart,
            "run",
            "example/smoke.dart",
            str(library),
        ],
        cwd=FLUTTER_ROOT,
    )


def main() -> int:
    args = parse_args()

    try:
        if args.verify_android_aar:
            llvm_nm = resolve_android_llvm_nm(args.android_ndk_home)
            assert_android_aar_contract(llvm_nm=llvm_nm)
            print(f"Android AAR contract verified: {ANDROID_RELEASE_AAR}")
            return 0

        if args.verify_android_maven:
            llvm_nm = resolve_android_llvm_nm(args.android_ndk_home)
            version_dir = assert_android_maven_publication(llvm_nm=llvm_nm)
            print(f"Android Maven publication contract verified: {version_dir}")
            return 0

        if args.only_android_instrumentation_smoke:
            step("Android instrumentation smoke")
            run_android_instrumentation_smoke(args.gradle_path, args.android_ndk_home)
            print()
            print("Android instrumentation smoke completed.")
            return 0

        step("Android Rust transport target checks")
        for recipe in (ANDROID_NATIVE_RECIPE, FLUTTER_ANDROID_NATIVE_RECIPE):
            run(
                [
                    *cargo_android_clippy_args(
                        recipe,
                        "aarch64-linux-android",
                    ),
                    "--",
                    "-D",
                    "warnings",
                ]
            )

        step("Flutter/Dart package checks")
        flutter = require_command("flutter")
        dart = require_command("dart")
        run([flutter, "pub", "get"], cwd=FLUTTER_ROOT)
        run([dart, "run", "ffigen", "--config", "ffigen.yaml"], cwd=FLUTTER_ROOT)
        verify_tracked_generated_file(FLUTTER_GENERATED_ABI)
        run([flutter, "analyze"], cwd=FLUTTER_ROOT)
        run(
            [dart, "format", "--set-exit-if-changed", *flutter_format_paths()],
            cwd=FLUTTER_ROOT,
        )
        run([dart, "run", "tool/abi3_contract_test.dart"], cwd=FLUTTER_ROOT)

        step("Dart FFI native smoke")
        run_dart_ffi_native_smoke(dart)

        print()
        print("Platform binding verification completed.")
        return 0
    except subprocess.CalledProcessError as exc:
        print(f"command failed with exit code {exc.returncode}: {' '.join(exc.cmd)}", file=sys.stderr)
        return exc.returncode
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
