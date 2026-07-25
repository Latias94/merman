#!/usr/bin/env python3
"""Run local platform binding verification gates."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from artifact_profile_recipe import (
    CargoArtifactRecipe,
    cargo_build_args as project_cargo_build_args,
    load_artifact_profile,
)


C_ABI_NATIVE_RECIPE = load_artifact_profile("c-abi-native")
FLUTTER_ROOT = REPO_ROOT / "platforms" / "flutter"
ANDROID_ROOT = REPO_ROOT / "platforms" / "android"
APPLE_ROOT = REPO_ROOT / "platforms" / "apple"
ANDROID_COMPILE_SDK = 35
ANDROID_JAR_OUT = REPO_ROOT / "target" / "platforms" / "android" / "merman-android.jar"
ANDROID_RELEASE_AAR = ANDROID_ROOT / "build" / "outputs" / "aar" / "merman-android-release.aar"
ANDROID_MAVEN_MODULE_ROOT = (
    ANDROID_ROOT / "build" / "repo" / "io" / "merman" / "merman-android"
)
ANDROID_TEST_RESULTS_ROOT = ANDROID_ROOT / "build" / "outputs" / "androidTest-results"
ANDROID_WRAPPER_CLASSES = [
    "io/merman/MermanEngine.class",
    "io/merman/MermanErrorKind.class",
    "io/merman/MermanReusableEngine.class",
    "io/merman/MermanException.class",
    "io/merman/MermanResourceOptions.class",
    "io/merman/MermanTextMeasureRequest.class",
    "io/merman/MermanTextMeasureResult.class",
    "io/merman/MermanTextMeasurementOperation.class",
    "io/merman/MermanTextMeasurementResultKind.class",
    "io/merman/MermanTextMeasurer.class",
]
ANDROID_NATIVE_LIBRARIES = [
    "jni/arm64-v8a/libmerman_ffi.so",
    "jni/x86_64/libmerman_ffi.so",
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
FLUTTER_JAR_OUT = REPO_ROOT / "target" / "platforms" / "flutter" / "merman-flutter-android-plugin.jar"
FLUTTER_GENERATED_ABI = (
    FLUTTER_ROOT / "lib" / "src" / "generated" / "native_abi.dart"
)


def validate_c_abi_native_recipe(
    recipe: CargoArtifactRecipe = C_ABI_NATIVE_RECIPE,
) -> None:
    expected_target_contract = ("cdylib", "rlib", "staticlib")
    if (
        recipe.profile_id != "c-abi-native"
        or recipe.package != "merman-ffi"
        or recipe.manifest != "crates/merman-ffi/Cargo.toml"
        or recipe.cargo_profile != "native-sdk"
        or recipe.default_features
        or recipe.target_name != "merman_ffi"
        or recipe.target_kinds != expected_target_contract
        or recipe.crate_types != expected_target_contract
        or recipe.build_target_kind != "host"
        or recipe.build_targets
    ):
        raise RuntimeError(
            "c-abi-native must remain the exact host native-sdk merman-ffi "
            "complete native SDK recipe"
        )
    manifest = REPO_ROOT / recipe.manifest
    if not manifest.is_file():
        raise RuntimeError(f"c-abi-native manifest does not exist: {manifest}")


validate_c_abi_native_recipe()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-android-slices", action="store_true")
    parser.add_argument("--run-flutter-android-smoke", action="store_true")
    parser.add_argument("--run-android-gradle-build", action="store_true")
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
    parser.add_argument("--run-android-instrumentation-smoke", action="store_true")
    parser.add_argument(
        "--only-android-instrumentation-smoke",
        action="store_true",
        help="Build missing Android native slices and run only the Android instrumentation smoke.",
    )
    parser.add_argument("--gradle-path", default=os.environ.get("MERMAN_GRADLE"))
    parser.add_argument(
        "--build-apple-xcframework",
        action="store_true",
        help="Build the Apple XCFramework after scaffold checks. Requires macOS/Xcode.",
    )
    parser.add_argument(
        "--apple-platform",
        choices=["all", "ios", "macos"],
        default="all",
        help="Apple platforms to pass to scripts/build-apple-xcframework.sh.",
    )
    return parser.parse_args()


def step(name: str) -> None:
    print()
    print(f"==> {name}")


def run(args: list[str], *, cwd: Path = REPO_ROOT) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, check=True)


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


def bash_path(path: Path) -> str:
    resolved = path.resolve()
    if os.name == "nt":
        drive = resolved.drive.rstrip(":").lower()
        parts = [part for part in resolved.parts[1:]]
        if drive:
            return "/mnt/" + drive + "/" + "/".join(parts)
    return str(resolved)


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
            ANDROID_ROOT / "src" / "main" / "jniLibs" / "arm64-v8a" / "libmerman_ffi.so",
            ANDROID_ROOT / "src" / "main" / "jniLibs" / "x86_64" / "libmerman_ffi.so",
        ]
    )


def android_kotlin_compile_sources(android_root: Path = ANDROID_ROOT) -> list[Path]:
    source_root = android_root / "src" / "main" / "kotlin" / "io" / "merman"
    sources = sorted(source_root.glob("*.kt"))
    smoke_source = android_root / "examples" / "MermanSmoke.kt"
    if not sources:
        raise RuntimeError(f"Android Kotlin source set is empty: {source_root}")
    if not smoke_source.exists():
        raise RuntimeError(f"Android Kotlin smoke source not found: {smoke_source}")
    return [*sources, smoke_source]


def android_compile_jar() -> Path:
    configured_roots = [
        os.environ.get("ANDROID_HOME"),
        os.environ.get("ANDROID_SDK_ROOT"),
    ]
    candidate_roots = [Path(root).expanduser() for root in configured_roots if root]
    candidate_roots.extend(
        [
            Path.home() / "Library" / "Android" / "sdk",
            Path.home() / "Android" / "Sdk",
        ]
    )
    for root in candidate_roots:
        android_jar = root / "platforms" / f"android-{ANDROID_COMPILE_SDK}" / "android.jar"
        if android_jar.is_file():
            return android_jar
    raise RuntimeError(
        "Android SDK platform android-35 is required to compile the Kotlin wrapper. "
        "Set ANDROID_HOME or ANDROID_SDK_ROOT to an SDK containing platforms/android-35/android.jar."
    )


def ensure_android_native_slices() -> None:
    if android_jni_libs_ready():
        return
    step("Android native slices")
    run(
        [
            sys.executable,
            str(ANDROID_ROOT / "build-android.py"),
            "--targets",
            "aarch64-linux-android",
            "x86_64-linux-android",
        ]
    )


def run_android_instrumentation_smoke(gradle_path: str | None) -> None:
    ensure_android_native_slices()
    gradle = resolve_gradle_command(gradle_path)
    run([gradle, "-p", str(ANDROID_ROOT), "assembleRelease", "--stacktrace"])
    assert_android_aar_contract()
    run([gradle, "-p", str(ANDROID_ROOT), "connectedAndroidTest", "--stacktrace"])
    assert_android_instrumentation_smoke_report()


def android_expected_resource_entries(android_root: Path = ANDROID_ROOT) -> list[str]:
    resources_root = android_root / "src" / "main" / "resources"
    if not resources_root.is_dir():
        raise RuntimeError(f"Android resource directory not found: {resources_root}")
    return sorted(
        path.relative_to(resources_root).as_posix()
        for path in resources_root.rglob("*")
        if path.is_file()
    )


def assert_android_aar_contract(
    aar_path: Path = ANDROID_RELEASE_AAR,
    android_root: Path = ANDROID_ROOT,
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

    with zipfile.ZipFile(io.BytesIO(classes_jar)) as jar:
        names = set(jar.namelist())

    missing_wrappers = [class_name for class_name in ANDROID_WRAPPER_CLASSES if class_name not in names]
    if missing_wrappers:
        raise RuntimeError(
            "Android release AAR is missing Kotlin wrapper classes: "
            + ", ".join(missing_wrappers)
        )

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


def _assert_sha256(artifact: Path) -> None:
    checksum_path = artifact.with_name(f"{artifact.name}.sha256")
    if not checksum_path.is_file():
        raise RuntimeError(f"Android Maven artifact is missing SHA-256: {checksum_path}")
    expected = checksum_path.read_text(encoding="ascii").strip().lower()
    actual = hashlib.sha256(artifact.read_bytes()).hexdigest()
    if expected != actual:
        raise RuntimeError(
            f"Android Maven SHA-256 mismatch for {artifact.name}: "
            f"expected {expected}, computed {actual}"
        )


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
    expected_sources = {
        path.relative_to(android_root / "src" / "main" / "kotlin").as_posix()
        for path in android_kotlin_compile_sources(android_root)[:-1]
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
        "merman-android/io.merman/-merman-engine/index.html",
        "merman-android/io.merman/-merman-reusable-engine/index.html",
    }
    with zipfile.ZipFile(javadoc_jar) as archive:
        names = set(archive.namelist())
    missing = sorted(required_entries - names)
    if missing:
        raise RuntimeError(
            "Android Maven javadoc JAR is missing generated API documentation: "
            + ", ".join(missing)
        )


def _assert_android_gradle_module(
    module_path: Path,
    version: str,
    published_artifacts: dict[str, Path],
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

    for name, artifact in published_artifacts.items():
        entries = variant_files.get(name, [])
        if not entries:
            raise RuntimeError(f"Android Gradle module does not declare artifact: {name}")
        actual_sha256 = hashlib.sha256(artifact.read_bytes()).hexdigest()
        actual_size = artifact.stat().st_size
        for entry in entries:
            if entry.get("sha256") != actual_sha256 or entry.get("size") != actual_size:
                raise RuntimeError(
                    f"Android Gradle module digest or size mismatch for artifact: {name}"
                )


def assert_android_maven_publication(
    module_root: Path = ANDROID_MAVEN_MODULE_ROOT,
    android_root: Path = ANDROID_ROOT,
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
    for artifact in artifacts.values():
        if not artifact.is_file():
            raise RuntimeError(f"Android Maven publication is missing artifact: {artifact}")
        _assert_sha256(artifact)

    _assert_android_maven_pom(artifacts["pom"], version)
    assert_android_aar_contract(artifacts["aar"], android_root)
    _assert_android_source_jar(artifacts["sources"], android_root)
    _assert_android_javadoc_jar(artifacts["javadoc"])
    _assert_android_gradle_module(
        artifacts["module"],
        version,
        {
            artifacts["aar"].name: artifacts["aar"],
            artifacts["sources"].name: artifacts["sources"],
            artifacts["javadoc"].name: artifacts["javadoc"],
        },
    )
    return version_dir


def assert_android_instrumentation_smoke_report(
    results_root: Path = ANDROID_TEST_RESULTS_ROOT,
) -> None:
    reports = list(results_root.rglob("*.xml")) if results_root.exists() else []
    for report in reports:
        text = report.read_text(encoding="utf-8", errors="ignore")
        if (
            "MermanInstrumentedSmokeTest" in text
            and "runsPublicSmokeIncludingThrowingTextMeasurerFallback" in text
        ):
            return
    raise RuntimeError(
        "Android instrumentation output did not include MermanInstrumentedSmokeTest results."
    )


def host_dynamic_library(
    recipe: CargoArtifactRecipe = C_ABI_NATIVE_RECIPE,
    *,
    host_system: str | None = None,
) -> Path:
    validate_c_abi_native_recipe(recipe)
    system = platform.system() if host_system is None else host_system
    library_stem = recipe.target_name.replace("-", "_")
    if system == "Windows":
        filename = f"{library_stem}.dll"
    elif system == "Darwin":
        filename = f"lib{library_stem}.dylib"
    else:
        filename = f"lib{library_stem}.so"
    return REPO_ROOT / "target" / recipe.cargo_profile / filename


def run_dart_ffi_native_smoke(
    dart: str,
    recipe: CargoArtifactRecipe = C_ABI_NATIVE_RECIPE,
    *,
    host_system: str | None = None,
) -> None:
    validate_c_abi_native_recipe(recipe)
    run(project_cargo_build_args(recipe, locked=True))
    run(
        [
            dart,
            "run",
            "example/smoke.dart",
            str(host_dynamic_library(recipe, host_system=host_system)),
        ],
        cwd=FLUTTER_ROOT,
    )


def flutter_android_embedding_jar() -> Path:
    flutter_root_env = os.environ.get("FLUTTER_ROOT")
    candidates: list[Path] = []
    if flutter_root_env:
        candidates.append(Path(flutter_root_env) / "bin" / "cache" / "artifacts" / "engine" / "android-arm64" / "flutter.jar")

    flutter = shutil.which("flutter")
    if flutter:
        flutter_bin = Path(flutter).resolve().parent
        candidates.append(flutter_bin.parent / "bin" / "cache" / "artifacts" / "engine" / "android-arm64" / "flutter.jar")

    for candidate in candidates:
        if candidate.exists():
            return candidate

    raise RuntimeError("Flutter Android embedding jar not found. Set FLUTTER_ROOT or run flutter doctor.")


def apple_build_args(apple_platform: str) -> list[str]:
    args = ["bash", "scripts/build-apple-xcframework.sh"]
    if apple_platform == "ios":
        args.append("--ios")
    elif apple_platform == "macos":
        args.append("--macos")
    return args


def main() -> int:
    args = parse_args()

    try:
        if args.verify_android_aar:
            assert_android_aar_contract()
            print(f"Android AAR contract verified: {ANDROID_RELEASE_AAR}")
            return 0

        if args.verify_android_maven:
            version_dir = assert_android_maven_publication()
            print(f"Android Maven publication contract verified: {version_dir}")
            return 0

        if args.only_android_instrumentation_smoke:
            step("Android instrumentation smoke")
            run_android_instrumentation_smoke(args.gradle_path)
            print()
            print("Android instrumentation smoke completed.")
            return 0

        step("Rust FFI host tests")
        run(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                "merman-ffi",
                "--no-default-features",
                "--features",
                C_ABI_NATIVE_RECIPE.feature_argument,
            ]
        )

        step("Android Rust target check")
        run(["rustup", "target", "add", "aarch64-linux-android"])
        run(
            [
                "cargo",
                "check",
                "-p",
                "merman-ffi",
                "--no-default-features",
                "--features",
                C_ABI_NATIVE_RECIPE.feature_argument,
                "--target",
                "aarch64-linux-android",
            ]
        )
        run(
            [
                "cargo",
                "clippy",
                "--no-deps",
                "-p",
                "merman-ffi",
                "--no-default-features",
                "--features",
                C_ABI_NATIVE_RECIPE.feature_argument,
                "--target",
                "aarch64-linux-android",
                "--",
                "-D",
                "warnings",
            ]
        )

        step("Android Kotlin wrapper compile")
        kotlinc = require_command("kotlinc")
        ANDROID_JAR_OUT.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                kotlinc,
                *(str(path) for path in android_kotlin_compile_sources()),
                "-classpath",
                str(android_compile_jar()),
                "-d",
                str(ANDROID_JAR_OUT),
            ]
        )

        if args.build_android_slices:
            step("Android native slices")
            run(
                [
                    sys.executable,
                    str(ANDROID_ROOT / "build-android.py"),
                    "--targets",
                    "aarch64-linux-android",
                    "x86_64-linux-android",
                ]
            )

        step("Flutter/Dart package checks")
        flutter = require_command("flutter")
        dart = require_command("dart")
        run([flutter, "pub", "get"], cwd=FLUTTER_ROOT)
        run([dart, "run", "ffigen", "--config", "ffigen.yaml"], cwd=FLUTTER_ROOT)
        verify_tracked_generated_file(FLUTTER_GENERATED_ABI)
        run([flutter, "analyze"], cwd=FLUTTER_ROOT)
        run([dart, "format", "--set-exit-if-changed", "lib", "example", "tool"], cwd=FLUTTER_ROOT)
        run([dart, "run", "tool/abi3_contract_test.dart"], cwd=FLUTTER_ROOT)

        step("Flutter Android plugin Kotlin compile")
        flutter_jar = flutter_android_embedding_jar()
        FLUTTER_JAR_OUT.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                kotlinc,
                str(FLUTTER_ROOT / "android" / "src" / "main" / "kotlin" / "io" / "merman" / "flutter" / "MermanFlutterPlugin.kt"),
                "-classpath",
                str(flutter_jar),
                "-d",
                str(FLUTTER_JAR_OUT),
            ]
        )

        step("Flutter native packaging scaffold checks")
        bash = require_command("bash")
        for path in [
            FLUTTER_ROOT / "build-ios.sh",
            FLUTTER_ROOT / "build-desktop.sh",
            FLUTTER_ROOT / "ios" / "merman.podspec",
            FLUTTER_ROOT
            / "ios"
            / "merman"
            / "Sources"
            / "merman"
            / "MermanFlutterPlugin.swift",
            FLUTTER_ROOT / "macos" / "merman.podspec",
            FLUTTER_ROOT
            / "macos"
            / "merman"
            / "Sources"
            / "merman"
            / "MermanFlutterPlugin.swift",
            FLUTTER_ROOT / "linux" / "CMakeLists.txt",
            FLUTTER_ROOT / "linux" / "include" / "merman" / "merman_flutter_plugin.h",
            FLUTTER_ROOT / "windows" / "CMakeLists.txt",
            FLUTTER_ROOT / "windows" / "include" / "merman" / "merman_flutter_plugin_c_api.h",
        ]:
            if not path.exists():
                raise RuntimeError(f"required Flutter packaging file not found: {path}")
        run([bash, "-n", bash_path(FLUTTER_ROOT / "build-ios.sh")])
        run([bash, "-n", bash_path(FLUTTER_ROOT / "build-desktop.sh")])

        step("Dart FFI native smoke")
        run_dart_ffi_native_smoke(dart)

        if args.run_android_gradle_build:
            ensure_android_native_slices()

            step("Android Gradle library assemble")
            gradle = resolve_gradle_command(args.gradle_path)
            run([gradle, "-p", str(ANDROID_ROOT), "assembleRelease", "--stacktrace"])
            assert_android_aar_contract()

        if args.run_android_instrumentation_smoke:
            step("Android instrumentation smoke")
            run_android_instrumentation_smoke(args.gradle_path)

        step("Apple Swift package scaffold checks")
        for path in [
            REPO_ROOT / "Package.swift",
            REPO_ROOT / "scripts" / "build-apple-xcframework.sh",
            REPO_ROOT / "platforms" / "ios" / "build-ios.sh",
            APPLE_ROOT / "Sources" / "Merman" / "Generated" / "Merman.swift",
            APPLE_ROOT / "Sources" / "Merman" / "Generated" / "MermanFFI.h",
            APPLE_ROOT / "Sources" / "Merman" / "Generated" / "MermanFFI.modulemap",
            REPO_ROOT / "crates" / "merman-uniffi" / "uniffi.toml",
            REPO_ROOT / "crates" / "merman-uniffi" / "examples" / "generate_swift_bindings.rs",
        ]:
            if not path.exists():
                raise RuntimeError(f"required Apple binding file not found: {path}")
        run([bash, "-n", bash_path(REPO_ROOT / "scripts" / "build-apple-xcframework.sh")])
        run([bash, "-n", bash_path(REPO_ROOT / "platforms" / "ios" / "build-ios.sh")])

        if args.build_apple_xcframework:
            if platform.system() != "Darwin":
                raise RuntimeError("--build-apple-xcframework requires macOS")
            step("Apple XCFramework build")
            run(apple_build_args(args.apple_platform))

        if args.run_flutter_android_smoke:
            step("Flutter Android APK packaging smoke")
            run(
                [
                    sys.executable,
                    str(FLUTTER_ROOT / "tool" / "android-smoke.py"),
                    "--targets",
                    "aarch64-linux-android",
                ]
            )

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
