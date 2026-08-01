#!/usr/bin/env python3
"""Build merman Android native slices and copy them into jniLibs."""

from __future__ import annotations

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import NamedTuple


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from artifact_profile_recipe import (
    CargoArtifactRecipe,
    cargo_build_args as project_cargo_build_args,
    load_artifact_profile,
)
from native_symbol_contract import (
    ANDROID_JNI_SYMBOL_CONTRACT,
    C_ABI_SYMBOL_CONTRACT,
    NativeSymbolContract,
    assert_symbol_contract,
    read_defined_dynamic_symbols,
)


ANDROID_ROOT = Path(__file__).resolve().parent
ANDROID_JNI_LIBS = ANDROID_ROOT / "src" / "main" / "jniLibs"
FLUTTER_JNI_LIBS = REPO_ROOT / "platforms" / "flutter" / "android" / "src" / "main" / "jniLibs"
VERSION_CATALOG = ANDROID_ROOT / "gradle" / "libs.versions.toml"
ANDROID_NATIVE_RECIPE = load_artifact_profile("android-native")
FLUTTER_ANDROID_NATIVE_RECIPE = load_artifact_profile("flutter-android-native")
ANDROID_ARTIFACT_PROFILE_IDS = (
    ANDROID_NATIVE_RECIPE.profile_id,
    FLUTTER_ANDROID_NATIVE_RECIPE.profile_id,
)


class AndroidArtifactPackaging(NamedTuple):
    package: str
    manifest: str
    target_name: str
    target_contract: tuple[str, ...]
    output_root: Path
    symbol_contract: NativeSymbolContract
    obsolete_library_filenames: tuple[str, ...]


ANDROID_ARTIFACT_PACKAGING = {
    ANDROID_NATIVE_RECIPE.profile_id: AndroidArtifactPackaging(
        package="merman-android-jni",
        manifest="crates/merman-android-jni/Cargo.toml",
        target_name="merman_android_jni",
        target_contract=("cdylib",),
        output_root=ANDROID_JNI_LIBS,
        symbol_contract=ANDROID_JNI_SYMBOL_CONTRACT,
        obsolete_library_filenames=("libmerman_ffi.so",),
    ),
    FLUTTER_ANDROID_NATIVE_RECIPE.profile_id: AndroidArtifactPackaging(
        package="merman-ffi",
        manifest="crates/merman-ffi/Cargo.toml",
        target_name="merman_ffi",
        target_contract=("cdylib", "rlib", "staticlib"),
        output_root=FLUTTER_JNI_LIBS,
        symbol_contract=C_ABI_SYMBOL_CONTRACT,
        obsolete_library_filenames=("libmerman_android_jni.so",),
    ),
}
TARGET_TO_ABI = {
    "aarch64-linux-android": "arm64-v8a",
    "x86_64-linux-android": "x86_64",
}


def validate_android_native_recipe(
    recipe: CargoArtifactRecipe = ANDROID_NATIVE_RECIPE,
) -> None:
    packaging = ANDROID_ARTIFACT_PACKAGING.get(recipe.profile_id)
    if packaging is None:
        raise RuntimeError(f"unsupported Android artifact profile: {recipe.profile_id}")
    if (
        recipe.package != packaging.package
        or recipe.manifest != packaging.manifest
        or recipe.cargo_profile != "native-sdk"
        or recipe.default_features
        or recipe.target_name != packaging.target_name
        or recipe.target_kinds != packaging.target_contract
        or recipe.crate_types != packaging.target_contract
        or recipe.build_target_kind != "target-set"
    ):
        raise RuntimeError(
            f"{recipe.profile_id} must remain its exact native-sdk transport recipe"
        )
    if set(recipe.build_targets) != set(TARGET_TO_ABI):
        raise RuntimeError(
            f"{recipe.profile_id} target set must exactly match the published Android ABI mapping"
        )
    manifest = REPO_ROOT / recipe.manifest
    if not manifest.is_file():
        raise RuntimeError(f"{recipe.profile_id} manifest does not exist: {manifest}")


validate_android_native_recipe()
validate_android_native_recipe(FLUTTER_ANDROID_NATIVE_RECIPE)


def android_native_recipe(profile_id: str) -> CargoArtifactRecipe:
    if profile_id not in ANDROID_ARTIFACT_PROFILE_IDS:
        raise RuntimeError(f"unsupported Android artifact profile: {profile_id}")
    recipe = load_artifact_profile(profile_id)
    validate_android_native_recipe(recipe)
    return recipe


def native_library_filename(recipe: CargoArtifactRecipe) -> str:
    validate_android_native_recipe(recipe)
    return f"lib{recipe.target_name.replace('-', '_')}.so"


def native_library_output_root(recipe: CargoArtifactRecipe) -> Path:
    validate_android_native_recipe(recipe)
    return ANDROID_ARTIFACT_PACKAGING[recipe.profile_id].output_root


def native_symbol_contract(recipe: CargoArtifactRecipe) -> NativeSymbolContract:
    validate_android_native_recipe(recipe)
    return ANDROID_ARTIFACT_PACKAGING[recipe.profile_id].symbol_contract


def obsolete_native_library_filenames(recipe: CargoArtifactRecipe) -> tuple[str, ...]:
    validate_android_native_recipe(recipe)
    return ANDROID_ARTIFACT_PACKAGING[recipe.profile_id].obsolete_library_filenames


def android_toolchain_versions(catalog: Path = VERSION_CATALOG) -> dict[str, str]:
    with catalog.open("rb") as handle:
        document = tomllib.load(handle)
    versions = document.get("versions")
    if not isinstance(versions, dict):
        raise RuntimeError(f"Android version catalog has no [versions] table: {catalog}")

    required = ("java", "ndk")
    missing = [key for key in required if not isinstance(versions.get(key), str)]
    if missing:
        raise RuntimeError(
            f"Android version catalog is missing string versions: {', '.join(missing)}"
        )
    return {key: versions[key] for key in required}


ANDROID_TOOLCHAIN_VERSIONS = android_toolchain_versions()
PINNED_NDK_VERSION = ANDROID_TOOLCHAIN_VERSIONS["ndk"]
PINNED_JAVA_MAJOR = int(ANDROID_TOOLCHAIN_VERSIONS["java"])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact-profile",
        choices=ANDROID_ARTIFACT_PROFILE_IDS,
        default=ANDROID_NATIVE_RECIPE.profile_id,
        help=(
            "Descriptor-owned Android artifact recipe. The default builds the JNI AAR; "
            "Flutter selects its JNI-free C ABI recipe."
        ),
    )
    parser.add_argument(
        "--targets",
        nargs="+",
        default=None,
        help="Rust Android targets to build. Defaults to the descriptor-owned target set.",
    )
    parser.add_argument(
        "--ndk-home",
        default=os.environ.get("ANDROID_NDK_HOME"),
        help="Android NDK path. Defaults to the pinned revision under ANDROID_HOME/ANDROID_SDK_ROOT.",
    )
    parser.add_argument(
        "--print-version",
        choices=sorted(ANDROID_TOOLCHAIN_VERSIONS),
        help="Print one canonical Android toolchain version and exit.",
    )
    parser.add_argument(
        "--install-missing-ndk",
        action="store_true",
        help="Install the pinned NDK with sdkmanager when it is missing.",
    )
    parser.add_argument(
        "--java-home",
        help="JDK home used for AAR assembly. Defaults to an auto-detected pinned JDK.",
    )
    parser.add_argument(
        "--assemble-aar",
        action="store_true",
        help="Assemble and verify the release AAR after building native slices.",
    )
    return parser.parse_args()


def run(args: list[str], *, cwd: Path = REPO_ROOT, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, env=env, check=True)


def ndk_host_tag() -> str:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Windows":
        return "windows-x86_64"
    if system == "Linux":
        return "linux-x86_64"
    if system == "Darwin":
        if machine in {"arm64", "aarch64"}:
            # Modern NDKs ship darwin-x86_64 prebuilt tools that run on Apple Silicon.
            return "darwin-x86_64"
        return "darwin-x86_64"
    raise RuntimeError(f"unsupported host platform for Android NDK: {system}")


def clang_name(target: str) -> str:
    api = "23"
    if target == "aarch64-linux-android":
        base = f"aarch64-linux-android{api}-clang"
    elif target == "x86_64-linux-android":
        base = f"x86_64-linux-android{api}-clang"
    else:
        raise RuntimeError(f"unsupported Android Rust target: {target}")
    if platform.system() == "Windows":
        return f"{base}.cmd"
    return base


def ndk_revision(ndk: Path) -> str:
    source_properties = ndk / "source.properties"
    if not source_properties.is_file():
        raise RuntimeError(f"Android NDK source.properties not found: {source_properties}")
    for line in source_properties.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator and key.strip() == "Pkg.Revision":
            return value.strip()
    raise RuntimeError(f"Android NDK revision not found in: {source_properties}")


def require_pinned_ndk(ndk: Path) -> Path:
    resolved = ndk.expanduser().resolve()
    actual = ndk_revision(resolved)
    if actual != PINNED_NDK_VERSION:
        raise RuntimeError(
            f"Android NDK revision {actual} does not match pinned {PINNED_NDK_VERSION}: {resolved}"
        )
    return resolved


def android_sdk_root() -> Path:
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        raise RuntimeError("ANDROID_NDK_HOME or ANDROID_HOME/ANDROID_SDK_ROOT must be set")
    return Path(sdk).expanduser().resolve()


def sdkmanager_command(sdk: Path) -> Path:
    executable = "sdkmanager.bat" if os.name == "nt" else "sdkmanager"
    candidates = [
        sdk / "cmdline-tools" / "latest" / "bin" / executable,
        sdk / "tools" / "bin" / executable,
    ]
    discovered = shutil.which("sdkmanager")
    if discovered:
        candidates.append(Path(discovered))
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise RuntimeError(
        "sdkmanager not found. Install Android SDK Command-line Tools and set ANDROID_HOME."
    )


def install_pinned_ndk(sdk: Path) -> None:
    run([str(sdkmanager_command(sdk)), "--install", f"ndk;{PINNED_NDK_VERSION}"])


def default_ndk_home(explicit: str | None, *, install_missing: bool = False) -> Path:
    if explicit:
        return require_pinned_ndk(Path(explicit))

    sdk = android_sdk_root()
    ndk = sdk / "ndk" / PINNED_NDK_VERSION
    if not ndk.is_dir() and install_missing:
        install_pinned_ndk(sdk)
    if not ndk.is_dir():
        raise RuntimeError(
            f"pinned Android NDK {PINNED_NDK_VERSION} is not installed: {ndk}. "
            f'Install it with sdkmanager --install "ndk;{PINNED_NDK_VERSION}".'
        )
    return require_pinned_ndk(ndk)


def gradle_wrapper_command() -> list[str]:
    wrapper = ANDROID_ROOT / ("gradlew.bat" if os.name == "nt" else "gradlew")
    if not wrapper.is_file():
        raise RuntimeError(f"Android Gradle wrapper not found: {wrapper}")
    if os.name == "nt":
        return ["cmd.exe", "/d", "/s", "/c", str(wrapper)]
    return [str(wrapper)]


def parse_java_major(version_output: str) -> int:
    match = re.search(r'version\s+"(?P<version>\d+(?:\.\d+)*)', version_output)
    if match is None:
        raise RuntimeError(f"could not parse Java version output: {version_output.strip()}")
    components = match.group("version").split(".")
    if components[0] == "1" and len(components) > 1:
        return int(components[1])
    return int(components[0])


def java_major(java: Path) -> int | None:
    if not java.is_file():
        return None
    result = subprocess.run(
        [str(java), "-version"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    try:
        return parse_java_major(f"{result.stdout}\n{result.stderr}")
    except RuntimeError:
        return None


def java_home_candidates() -> list[Path]:
    candidates: list[Path] = []
    if configured := os.environ.get("JAVA_HOME"):
        candidates.append(Path(configured))

    if active_java := shutil.which("java"):
        candidates.append(Path(active_java).resolve().parent.parent)

    if platform.system() == "Darwin":
        java_home = Path("/usr/libexec/java_home")
        if java_home.is_file():
            result = subprocess.run(
                [str(java_home), "-v", str(PINNED_JAVA_MAJOR)],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode == 0 and result.stdout.strip():
                candidates.append(Path(result.stdout.strip()))
        if brew := shutil.which("brew"):
            result = subprocess.run(
                [brew, "--prefix", f"openjdk@{PINNED_JAVA_MAJOR}"],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode == 0 and result.stdout.strip():
                prefix = Path(result.stdout.strip())
                candidates.extend(
                    [prefix / "libexec" / "openjdk.jdk" / "Contents" / "Home", prefix]
                )
    elif platform.system() == "Linux":
        candidates.extend(sorted(Path("/usr/lib/jvm").glob("*")))
    elif platform.system() == "Windows":
        for environment_name in ("ProgramFiles", "ProgramFiles(x86)"):
            if program_files := os.environ.get(environment_name):
                candidates.extend(sorted((Path(program_files) / "Java").glob("*")))

    candidates.extend(
        sorted((Path.home() / ".sdkman" / "candidates" / "java").glob("*"))
    )
    deduplicated: list[Path] = []
    seen: set[Path] = set()
    for candidate in candidates:
        resolved = candidate.expanduser().resolve()
        if resolved not in seen:
            seen.add(resolved)
            deduplicated.append(resolved)
    return deduplicated


def resolve_pinned_java_home(explicit: str | None = None) -> Path:
    candidates = [Path(explicit).expanduser().resolve()] if explicit else java_home_candidates()
    executable = "java.exe" if os.name == "nt" else "java"
    discovered_majors: list[str] = []
    for candidate in candidates:
        actual = java_major(candidate / "bin" / executable)
        if actual == PINNED_JAVA_MAJOR:
            return candidate
        if actual is not None:
            discovered_majors.append(f"{candidate} (Java {actual})")

    if explicit:
        raise RuntimeError(
            f"--java-home does not point to a JDK {PINNED_JAVA_MAJOR}: {explicit}"
        )
    discovered = ", ".join(discovered_majors) or "no usable JDK installations"
    raise RuntimeError(
        f"Java {PINNED_JAVA_MAJOR} is required to build the Android AAR; found {discovered}. "
        f"Install JDK {PINNED_JAVA_MAJOR} or pass --java-home."
    )


def gradle_environment(java_home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["JAVA_HOME"] = str(java_home)
    env["PATH"] = os.pathsep.join([str(java_home / "bin"), env.get("PATH", "")])
    return env


def clang_for_target(target: str, ndk: Path) -> Path:
    clang = ndk / "toolchains" / "llvm" / "prebuilt" / ndk_host_tag() / "bin" / clang_name(target)
    if not clang.exists():
        raise RuntimeError(f"Android clang not found: {clang}")
    return clang


def llvm_nm_for_ndk(ndk: Path) -> Path:
    executable = "llvm-nm.exe" if os.name == "nt" else "llvm-nm"
    llvm_nm = ndk / "toolchains" / "llvm" / "prebuilt" / ndk_host_tag() / "bin" / executable
    if not llvm_nm.is_file():
        raise RuntimeError(f"Android NDK llvm-nm not found: {llvm_nm}")
    return llvm_nm


def cargo_env_with_linker(target: str, clang: Path) -> dict[str, str]:
    env = os.environ.copy()
    env_name = f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"
    env[env_name] = str(clang)
    return env


def cargo_build_args(
    target: str,
    recipe: CargoArtifactRecipe = ANDROID_NATIVE_RECIPE,
) -> list[str]:
    return project_cargo_build_args(
        recipe,
        locked=True,
        target=target,
    )


def build_target(target: str, ndk: Path, recipe: CargoArtifactRecipe) -> None:
    abi = TARGET_TO_ABI.get(target)
    if abi is None:
        raise RuntimeError(f"unsupported Android Rust target: {target}")

    clang = clang_for_target(target, ndk)
    env = cargo_env_with_linker(target, clang)

    print(f"==> Building {recipe.package} ({recipe.profile_id}) for {target} ({abi})")
    run(["rustup", "target", "add", target])
    run(cargo_build_args(target, recipe), env=env)

    profile_dir = recipe.cargo_profile
    filename = native_library_filename(recipe)
    artifact = REPO_ROOT / "target" / target / profile_dir / filename
    if not artifact.exists():
        raise RuntimeError(f"expected Android library not found: {artifact}")

    symbols = read_defined_dynamic_symbols(artifact, llvm_nm_for_ndk(ndk))
    assert_symbol_contract(
        symbols,
        native_symbol_contract(recipe),
        label=f"{recipe.profile_id} {target}",
    )

    dest = native_library_output_root(recipe) / abi
    dest.mkdir(parents=True, exist_ok=True)
    for obsolete in obsolete_native_library_filenames(recipe):
        (dest / obsolete).unlink(missing_ok=True)
    shutil.copy2(artifact, dest / filename)
    print(f"Copied {abi} library to {dest}")


def main() -> int:
    args = parse_args()
    if args.print_version:
        print(ANDROID_TOOLCHAIN_VERSIONS[args.print_version])
        return 0
    try:
        recipe = android_native_recipe(args.artifact_profile)
        targets = args.targets or list(recipe.build_targets)
        java_home = None
        if args.assemble_aar:
            if recipe.profile_id != ANDROID_NATIVE_RECIPE.profile_id:
                raise RuntimeError("--assemble-aar requires the android-native JNI recipe")
            java_home = resolve_pinned_java_home(args.java_home)
            print(f"Using Java {PINNED_JAVA_MAJOR}: {java_home}")
        ndk = default_ndk_home(
            args.ndk_home,
            install_missing=args.install_missing_ndk,
        )
        print(f"Using Android NDK: {ndk}")
        for target in targets:
            build_target(target, ndk, recipe)
        if args.assemble_aar:
            assert java_home is not None
            run(
                [
                    *gradle_wrapper_command(),
                    "-p",
                    str(ANDROID_ROOT),
                    "assembleRelease",
                    "--stacktrace",
                ],
                env=gradle_environment(java_home),
            )
            run(
                [
                    sys.executable,
                    str(REPO_ROOT / "scripts" / "verify-platform-bindings.py"),
                    "--verify-android-aar",
                    "--android-ndk-home",
                    str(ndk),
                ]
            )
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
