#!/usr/bin/env python3
"""Generate the merman UniFFI Python package and build a local wheel."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from email.parser import Parser
import shutil
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Iterator
from pathlib import Path

from artifact_profile_recipe import (
    CargoArtifactRecipe,
    cargo_build_args,
    load_artifact_profile,
    rustc_host_target,
)
from python_wheel_licenses import (
    install_target_report,
    verify_wheel_license_report,
)


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_GENERATED_SUPPORT_FILES = (
    "src/merman/__init__.py",
    "src/merman/_binding_contract.py",
    "src/merman/_resource_options.py",
    "src/merman/_runtime_catalog.py",
    "src/merman/_text_measurement_protocol.py",
)
PYTHON_STAGING_IGNORE = shutil.ignore_patterns(
    "build",
    "dist",
    "*.egg-info",
    "__pycache__",
    "merman_uniffi.py",
    "*.dll",
    "*.dylib",
    "*.so",
)


def production_cdylib_path(recipe: CargoArtifactRecipe, target: str) -> Path:
    library_stem = recipe.target_name.replace("-", "_")
    if "windows" in target:
        filename = f"{library_stem}.dll"
    elif "apple" in target:
        filename = f"lib{library_stem}.dylib"
    else:
        filename = f"lib{library_stem}.so"
    return REPO_ROOT / "target" / target / recipe.cargo_profile / filename


def validate_python_native_recipe(recipe: CargoArtifactRecipe) -> None:
    if (
        recipe.profile_id != "python-uniffi-native"
        or "cdylib" not in recipe.crate_types
        or not recipe.build_targets
    ):
        raise RuntimeError(
            "python-uniffi-native must publish a cdylib for at least one target"
        )
    manifest = REPO_ROOT / recipe.manifest
    if not manifest.is_file():
        raise RuntimeError(f"python-uniffi-native manifest does not exist: {manifest}")


def select_python_wheel_target(recipe: CargoArtifactRecipe) -> str:
    target = rustc_host_target()
    if target not in recipe.build_targets:
        raise RuntimeError(
            f"Python wheels are not published for Rust host target {target!r}; "
            f"supported targets: {', '.join(recipe.build_targets)}"
        )
    return target


def python_generator_args(
    recipe: CargoArtifactRecipe,
    cdylib: Path,
    package_dir: Path,
) -> list[str]:
    return [
        "cargo",
        "run",
        "--package",
        recipe.package,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
        "--locked",
        "--no-default-features",
        "--features",
        "binding-generation",
        "--example",
        "generate_python_package",
        "--",
        *(
            "--cdylib",
            str(cdylib),
            "--package-dir",
            str(package_dir),
        ),
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package-dir",
        default=str(REPO_ROOT / "platforms" / "python" / "merman"),
        help="Python package scaffold directory.",
    )
    parser.add_argument(
        "--wheel-dir",
        default=str(REPO_ROOT / "target" / "python-wheels"),
        help="Output directory for built wheels.",
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Python executable used for pip, venv, and smoke checks.",
    )
    parser.add_argument(
        "--run-smoke",
        action="store_true",
        help="Install the newest wheel into a temporary venv and run the checked-in smoke.",
    )
    return parser.parse_args()


def run(
    args: list[str],
    *,
    cwd: Path = REPO_ROOT,
) -> None:
    display_args = ("<inline-script>" if "\n" in arg else arg for arg in args)
    print("+", " ".join(display_args))
    subprocess.run(args, cwd=cwd, check=True)


def verify_generated_python_support_files(package_dir: Path, staged: Path) -> None:
    repository_paths: list[str] = []
    for relative in PYTHON_GENERATED_SUPPORT_FILES:
        source = package_dir / relative
        if not source.is_file():
            raise RuntimeError(f"generated Python support file is missing: {source}")
        try:
            repository_path = source.resolve().relative_to(REPO_ROOT.resolve()).as_posix()
        except ValueError as exc:
            raise RuntimeError(
                f"Python package source must be inside the repository: {package_dir}"
            ) from exc
        repository_paths.append(repository_path)

    run(["git", "ls-files", "--error-unmatch", "--", *repository_paths])
    for relative in PYTHON_GENERATED_SUPPORT_FILES:
        source = package_dir / relative
        generated = staged / relative
        if not generated.is_file():
            raise RuntimeError(
                f"Python generator did not produce required support file: {relative}"
            )
        if source.read_bytes() != generated.read_bytes():
            raise RuntimeError(
                "stale generated Python support file: "
                f"{relative}; regenerate and commit the source projection"
            )


@contextmanager
def staged_python_package(package_dir: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="merman-python-wheel-") as temp_dir:
        staged = Path(temp_dir) / package_dir.name
        shutil.copytree(package_dir, staged, ignore=PYTHON_STAGING_IGNORE)
        yield staged


def venv_python(venv_dir: Path) -> Path:
    windows_python = venv_dir / "Scripts" / "python.exe"
    if windows_python.exists():
        return windows_python
    unix_python = venv_dir / "bin" / "python"
    if unix_python.exists():
        return unix_python
    raise RuntimeError(f"Python executable not found in venv: {venv_dir}")


def newest_wheel(wheel_dir: Path) -> Path:
    wheels = sorted(
        wheel_dir.glob("merman-*.whl"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not wheels:
        raise RuntimeError(f"No merman wheel found under {wheel_dir}")
    return wheels[0]


def remove_stale_wheels(wheel_dir: Path) -> None:
    for wheel in wheel_dir.glob("merman-*.whl"):
        wheel.unlink()


def remove_stale_package_build(package_dir: Path) -> None:
    build_dir = package_dir / "build"
    if build_dir.exists():
        shutil.rmtree(build_dir)


def require_native_platform_wheel(wheel: Path) -> None:
    if wheel.name.endswith("-py3-none-any.whl"):
        raise RuntimeError(
            f"expected a platform wheel with the bundled native library, got universal wheel: {wheel.name}"
        )
    native_suffixes = (".dll", ".dylib", ".so")
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        wheel_metadata_path = next(
            (name for name in names if name.endswith(".dist-info/WHEEL")), None
        )
        if wheel_metadata_path is None:
            raise RuntimeError(f"{wheel.name} does not contain WHEEL metadata")

        metadata = Parser().parsestr(archive.read(wheel_metadata_path).decode("utf-8"))
        if metadata.get("Root-Is-Purelib") != "false":
            raise RuntimeError(
                f"{wheel.name} must set Root-Is-Purelib: false for bundled native libraries"
            )

        native_members = [
            name for name in names if name.lower().endswith(native_suffixes)
        ]
        if not native_members:
            raise RuntimeError(f"{wheel.name} does not contain a bundled native library")

        purelib_native_members = [
            name for name in native_members if ".data/purelib/" in name
        ]
        if purelib_native_members:
            joined = ", ".join(purelib_native_members)
            raise RuntimeError(
                f"{wheel.name} stores native libraries under purelib: {joined}"
            )


def main() -> int:
    args = parse_args()
    package_source = Path(args.package_dir).expanduser().resolve()
    wheel_dir = Path(args.wheel_dir).expanduser().resolve()

    recipe = load_artifact_profile("python-uniffi-native")
    validate_python_native_recipe(recipe)
    target = select_python_wheel_target(recipe)
    run(cargo_build_args(recipe, locked=True, target=target))
    cdylib = production_cdylib_path(recipe, target)
    if not cdylib.is_file():
        raise RuntimeError(f"expected production UniFFI library not found: {cdylib}")
    with staged_python_package(package_source) as package_dir:
        run(
            python_generator_args(recipe, cdylib, package_dir),
        )
        verify_generated_python_support_files(package_source, package_dir)
        install_target_report(REPO_ROOT, package_dir, target)

        remove_stale_package_build(package_dir)
        wheel_dir.mkdir(parents=True, exist_ok=True)
        remove_stale_wheels(wheel_dir)
        run(
            [
                args.python,
                "-m",
                "pip",
                "wheel",
                str(package_dir),
                "--no-deps",
                "--wheel-dir",
                str(wheel_dir),
            ]
        )
        wheel = newest_wheel(wheel_dir)
        require_native_platform_wheel(wheel)
        verify_wheel_license_report(
            wheel,
            root=REPO_ROOT,
            expected_target=target,
        )

    if args.run_smoke:
        venv_dir = REPO_ROOT / "target" / "python-wheel-smoke"
        if venv_dir.exists():
            shutil.rmtree(venv_dir)
        run([args.python, "-m", "venv", str(venv_dir)])
        python = venv_python(venv_dir)
        run([str(python), "-m", "pip", "install", "--no-deps", str(wheel)])
        example = package_source / "examples" / "smoke.py"
        if not example.is_file():
            raise RuntimeError(f"Python wheel smoke example is missing: {example}")
        run([str(python), str(example)])

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
