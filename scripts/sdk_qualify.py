#!/usr/bin/env python3
"""Build, qualify and archive one descriptor-selected SDK on its actual host."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

if __package__:
    from . import sdk_archive
    from .artifact_profile_recipe import load_artifact_profile, rustc_host_target
    from .release_archive import require_regular_input, verified_archive_contents
else:
    import sdk_archive
    from artifact_profile_recipe import load_artifact_profile, rustc_host_target
    from release_archive import require_regular_input, verified_archive_contents


REPO_ROOT = Path(__file__).resolve().parents[1]


def run(command, *, repo_root, environment=None, **kwargs):
    print("RUN " + " ".join(str(argument) for argument in command), file=sys.stderr, flush=True)
    return subprocess.run(
        [str(argument) for argument in command], cwd=repo_root, env=environment,
        check=True, **kwargs,
    )


def consumer_environment(prefix: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment["PATH"] = str(prefix / "bin") + os.pathsep + environment.get("PATH", "")
    return environment


def build_consumer(prefix: Path, build: Path, *, repo_root: Path) -> Path:
    environment = consumer_environment(prefix)
    run(["cmake", "-S", prefix, "-B", build, "-DCMAKE_PREFIX_PATH=" + str(prefix)],
        repo_root=repo_root, environment=environment)
    run(["cmake", "--build", build, "--config", "Release", "--parallel", "2"],
        repo_root=repo_root, environment=environment)
    # Visual Studio and other multi-config generators place executables in Release/.
    return build / "Release" if (build / "Release").is_dir() else build


def executable(directory: Path, name: str, target: str) -> Path:
    return directory / (name + (".exe" if "windows" in target else ""))


def stage_libraries(built: Path, artifact: Path, target: str, *, repo_root: Path) -> None:
    for name in sdk_archive.library_names(target).values():
        source = built / name
        require_regular_input(source, "just-built SDK library")
        shutil.copyfile(source, artifact / name)
    if "apple" in target:
        library = artifact / sdk_archive.library_names(target)["dynamic"]
        run(["install_name_tool", "-id", "@rpath/" + library.name, library], repo_root=repo_root)
        run(["codesign", "--force", "--sign", "-", "--timestamp=none", library], repo_root=repo_root)


def native_probe(artifact: Path, target: str, version: str, *, repo_root: Path) -> tuple[Path, dict]:
    prefix = artifact / "consumer-prefix"
    for kind, relative in sdk_archive.library_layout(target).items():
        destination = prefix / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(artifact / sdk_archive.library_names(target)[kind], destination)
    for relative, data in sdk_archive.native_consumer_files(repo_root, target, version).items():
        destination = prefix / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    binaries = build_consumer(prefix, artifact / "consumer-build", repo_root=repo_root)
    return executable(binaries, "merman-probe", target), consumer_environment(prefix)


def verify_installed_consumer(archive: Path, profile: str, target: str, version: str,
                              *, repo_root: Path) -> None:
    with verified_archive_contents(
        archive, Path(str(archive) + ".sha256"), package_name="merman-" + profile,
        target=target, version=version,
    ) as extracted, tempfile.TemporaryDirectory(prefix="merman-sdk-consumer-") as temporary:
        build = Path(temporary)
        binaries = build_consumer(extracted.root, build, repo_root=repo_root)
        environment = consumer_environment(extracted.root)
        if "apple" in target:
            consumer = executable(binaries, "merman-render", target)
            linked = run(["otool", "-L", consumer], repo_root=repo_root,
                         capture_output=True, text=True).stdout
            if "@rpath/libmerman_ffi.dylib" not in linked:
                raise RuntimeError("consumer retains a producer install name")
            loaded = run(
                [consumer], repo_root=repo_root,
                environment=dict(environment, DYLD_PRINT_LIBRARIES="1"),
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
            )
            library = str(extracted.root / "lib" / "libmerman_ffi.dylib")
            if library not in loaded.stderr:
                raise RuntimeError("consumer did not load the extracted SDK")
        run(["ctest", "--test-dir", build, "-C", "Release", "--output-on-failure"],
            repo_root=repo_root, environment=environment)


def qualify_sdk(*, profile: str, target: str, work_dir: Path, output_dir: Path,
                repo_root: Path = REPO_ROOT, source_commit: str | None = None,
                wasi_dir: Path | None = None) -> dict:
    repo_root = Path(repo_root).resolve()
    descriptor = repo_root / "capabilities/artifact-profiles-v2.json"
    command = sdk_archive.build_command(profile, target, descriptor, repo_root)
    recipe = load_artifact_profile(profile, descriptor)
    commit = sdk_archive.resolve_source_commit(repo_root, source_commit)
    version = tomllib.loads((repo_root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    wasi = target == sdk_archive.WASI_TARGET
    if not wasi and rustc_host_target() != target:
        raise RuntimeError("native SDK qualification requires the actual Rust target host")
    if wasi and wasi_dir is not None:
        raise RuntimeError("--wasi-dir is only meaningful for native/WASI comparison")
    artifact = Path(work_dir).absolute()
    # A failed run is retained for diagnosis; retry in a new directory, never overwrite it.
    artifact.mkdir(parents=True, exist_ok=False)
    artifact = artifact.resolve()
    run(command, repo_root=repo_root)
    metadata = run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        repo_root=repo_root, capture_output=True, text=True,
    )
    cargo_target = Path(json.loads(metadata.stdout)["target_directory"])
    stage_libraries(cargo_target / target / recipe.cargo_profile, artifact, target, repo_root=repo_root)
    flavor = profile.rsplit("-", 1)[1]
    common = ["--flavor", flavor, "--catalog-out", str(artifact / "raw-catalog.json")]
    smoke_name = "wasi-smoke" if wasi else "native-smoke"
    if wasi:
        run([sys.executable, repo_root / "platforms/wasi/smoke.py", "--module",
             artifact / "merman_wasi.wasm", *common, "--output-dir", artifact / smoke_name],
            repo_root=repo_root)
    else:
        probe, environment = native_probe(artifact, target, version, repo_root=repo_root)
        parity = ["--wasi-dir", str(Path(wasi_dir).resolve())] if wasi_dir else []
        run([sys.executable, repo_root / "platforms/wasi/native_smoke.py", "--probe", probe,
             *common, "--output-dir", artifact / smoke_name, *parity],
            repo_root=repo_root, environment=environment)
    sdk_archive.write_catalog_sidecar(
        repo_root=repo_root, profile=profile, target=target, artifact_dir=artifact,
        catalog_in=artifact / "raw-catalog.json", source_commit=commit,
    )
    run([sys.executable, repo_root / "scripts/generate-rust-license-report.py", "--write",
         "--artifact-profile", profile, "--target", target,
         "--output", artifact / "rust-cargo-dependencies.json"], repo_root=repo_root)
    archive = sdk_archive.package_sdk(
        repo_root=repo_root, profile=profile, target=target, artifact_dir=artifact,
        output_dir=output_dir, source_commit=commit,
    )
    sdk_archive.verify_sdk_archive(
        repo_root=repo_root, profile=profile, target=target, archive=archive,
        checksum=Path(str(archive) + ".sha256"), source_commit=commit, version=version,
    )
    if not wasi:
        verify_installed_consumer(archive, profile, target, version, repo_root=repo_root)
    evidence = artifact / "evidence"
    shutil.copytree(artifact / smoke_name, evidence / smoke_name)
    report = {"schema_version": 1, "ok": True, "source_commit": commit, "version": version,
              "profile": profile, "target": target, "archive": str(archive),
              "evidence_dir": str(evidence)}
    with (artifact / "qualification.json").open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, choices=sdk_archive.PROFILES)
    parser.add_argument("--target", required=True)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--source-commit")
    parser.add_argument("--wasi-dir", type=Path,
                        help="existing two-runtime smoke root for native PNG comparison")
    args = parser.parse_args()
    try:
        report = qualify_sdk(**vars(args))
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"SDK qualification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
