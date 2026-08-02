#!/usr/bin/env python3
"""Atomically capture fixed FFI evidence from the commit 5117 Git object."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tempfile
from typing import Any, Sequence
import unicodedata

from artifact_profile_recipe import REPO_ROOT
from ffi_contract_baseline_contract import (
    BASELINE_INPUT_PATHS,
    BASELINE_LOCK_SCHEMA_VERSION,
    FfiBaselineContractError,
    file_sha256,
    input_records,
)
from ffi_contract_dependency_probes import (
    BASELINE_COMMIT,
    BASELINE_TREE,
    load_dependency_probes,
    probe_registry_sha256,
)
from ffi_contract_reproducibility import (
    FfiContractReproducibilityError,
    ffi_contract_subprocess_environment,
    reject_cargo_configuration,
    reject_ffi_contract_environment,
    rust_toolchain_provenance,
)
from strict_json import canonical_sha256
from verify_artifact_dependency_closures import (
    BASELINE_SCHEMA_VERSION as DEPENDENCY_REPORT_SCHEMA_VERSION,
    capture_probe_observations,
    dependency_baseline_report,
    load_dependency_baseline,
)
from verify_native_artifact_sizes import (
    SCHEMA_VERSION as NATIVE_ARTIFACT_SCHEMA_VERSION,
    capture_native_artifact_measurements,
    load_native_artifact_baseline,
    load_native_artifact_profiles,
    native_artifact_report,
)


class BaselineCaptureError(RuntimeError):
    """The Git object, capture environment, or evidence destination is unsafe."""


GIT = "/usr/bin/git"
GIT_OBJECT_RE = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class GitTreeEntry:
    path: PurePosixPath
    mode: int
    oid: str
    size: int


def run_text(command: Sequence[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        env=ffi_contract_subprocess_environment(),
        text=True,
    )


def checked_output(command: Sequence[str], cwd: Path) -> str:
    completed = run_text(command, cwd)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise BaselineCaptureError(
            f"command failed with {completed.returncode}: {' '.join(command)}: {detail}"
        )
    if not completed.stdout.strip():
        raise BaselineCaptureError(f"command produced no text: {' '.join(command)}")
    return completed.stdout.strip()


def validate_source_repository(source_repo: Path) -> None:
    if source_repo.is_symlink() or not source_repo.is_dir():
        raise BaselineCaptureError(
            f"baseline source repository must be a real directory: {source_repo}"
        )
    git_top_level = Path(
        checked_output(
            (GIT, "--no-replace-objects", "rev-parse", "--show-toplevel"),
            source_repo,
        )
    ).resolve()
    if git_top_level != source_repo.resolve():
        raise BaselineCaptureError(
            "baseline source repository must be the Git top-level directory"
        )
    commit = checked_output(
        (
            GIT,
            "--no-replace-objects",
            "rev-parse",
            f"{BASELINE_COMMIT}^{{commit}}",
        ),
        source_repo,
    )
    if commit != BASELINE_COMMIT:
        raise BaselineCaptureError(
            f"repository does not resolve the fixed baseline commit {BASELINE_COMMIT}"
        )
    tree = checked_output(
        (
            GIT,
            "--no-replace-objects",
            "rev-parse",
            f"{BASELINE_COMMIT}^{{tree}}",
        ),
        source_repo,
    )
    if tree != BASELINE_TREE:
        raise BaselineCaptureError(
            f"fixed baseline tree must be {BASELINE_TREE}; found {tree}"
        )


def prepare_output_destination(output_root: Path) -> None:
    if output_root.exists() or output_root.is_symlink():
        raise BaselineCaptureError(
            f"baseline output directory must not already exist: {output_root}"
        )
    output_root.parent.mkdir(parents=True, exist_ok=True)


def materialize_git_snapshot(
    source_repo: Path,
    snapshot_root: Path,
) -> tuple[GitTreeEntry, ...]:
    if snapshot_root.exists() or snapshot_root.is_symlink():
        raise BaselineCaptureError(
            f"snapshot destination must not already exist: {snapshot_root}"
        )
    snapshot_root.mkdir(parents=True)
    entries = load_git_tree_entries(source_repo)
    _materialize_git_blobs(source_repo, snapshot_root, entries)
    for relative in BASELINE_INPUT_PATHS:
        path = snapshot_root / relative
        if path.is_symlink() or not path.is_file():
            raise BaselineCaptureError(
                f"fixed Git tree is missing a regular baseline input: {relative}"
            )
    return entries


def load_git_tree_entries(source_repo: Path) -> tuple[GitTreeEntry, ...]:
    completed = subprocess.run(
        (
            GIT,
            "--no-replace-objects",
            "ls-tree",
            "-rz",
            "--full-tree",
            "--long",
            BASELINE_TREE,
        ),
        cwd=source_repo,
        check=False,
        capture_output=True,
        env=ffi_contract_subprocess_environment(),
        text=False,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or b"").decode(
            "utf-8", errors="replace"
        ).strip()
        raise BaselineCaptureError(f"git ls-tree failed: {detail}")
    entries: list[GitTreeEntry] = []
    path_keys: set[str] = set()
    filesystem_keys: set[str] = set()
    for raw_record in completed.stdout.split(b"\0"):
        if not raw_record:
            continue
        try:
            header, raw_path = raw_record.split(b"\t", 1)
            raw_mode, object_type, raw_oid, raw_size = header.split()
            path_text = raw_path.decode("utf-8")
            mode_text = raw_mode.decode("ascii")
            type_text = object_type.decode("ascii")
            oid = raw_oid.decode("ascii")
            size = int(raw_size)
        except (UnicodeError, ValueError) as error:
            raise BaselineCaptureError(
                f"fixed Git tree contains a malformed entry: {raw_record!r}"
            ) from error
        path = PurePosixPath(path_text)
        if (
            not path_text
            or path.is_absolute()
            or "." in path.parts
            or ".." in path.parts
            or path.as_posix() != path_text
            or type_text != "blob"
            or mode_text not in {"100644", "100755"}
            or not GIT_OBJECT_RE.fullmatch(oid)
            or size < 0
        ):
            raise BaselineCaptureError(
                f"fixed Git tree contains an unsupported entry: {path_text!r}"
            )
        path_key = path.as_posix()
        filesystem_key = unicodedata.normalize("NFC", path_key).casefold()
        if path_key in path_keys or filesystem_key in filesystem_keys:
            raise BaselineCaptureError(
                f"fixed Git tree contains a duplicate filesystem path: {path_key!r}"
            )
        path_keys.add(path_key)
        filesystem_keys.add(filesystem_key)
        entries.append(GitTreeEntry(path, int(mode_text, 8), oid, size))
    if not entries:
        raise BaselineCaptureError("fixed Git tree must not be empty")
    return tuple(sorted(entries, key=lambda entry: entry.path.as_posix()))


def _materialize_git_blobs(
    source_repo: Path,
    snapshot_root: Path,
    entries: Sequence[GitTreeEntry],
) -> None:
    for relative in expected_snapshot_directories(entries):
        directory = snapshot_root.joinpath(*relative.parts)
        directory.mkdir()
        directory.chmod(0o755)
    process = subprocess.Popen(
        (GIT, "--no-replace-objects", "cat-file", "--batch"),
        cwd=source_repo,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=ffi_contract_subprocess_environment(),
    )
    if process.stdin is None or process.stdout is None or process.stderr is None:
        process.kill()
        raise BaselineCaptureError("cannot open git cat-file batch pipes")
    try:
        for entry in entries:
            process.stdin.write(entry.oid.encode("ascii") + b"\n")
            process.stdin.flush()
            header = process.stdout.readline().rstrip(b"\n")
            expected_header = f"{entry.oid} blob {entry.size}".encode("ascii")
            if header != expected_header:
                raise BaselineCaptureError(
                    f"git cat-file returned unexpected metadata for {entry.path}"
                )
            destination = snapshot_root.joinpath(*entry.path.parts)
            descriptor = _open_exclusive_regular_file(destination, entry.mode)
            try:
                digest = hashlib.sha1()
                digest.update(f"blob {entry.size}\0".encode("ascii"))
                remaining = entry.size
                while remaining:
                    chunk = process.stdout.read(min(1024 * 1024, remaining))
                    if not chunk:
                        raise BaselineCaptureError(
                            f"git cat-file truncated blob data for {entry.path}"
                        )
                    _write_all(descriptor, chunk)
                    digest.update(chunk)
                    remaining -= len(chunk)
                if process.stdout.read(1) != b"\n":
                    raise BaselineCaptureError(
                        f"git cat-file omitted blob delimiter for {entry.path}"
                    )
                if digest.hexdigest() != entry.oid:
                    raise BaselineCaptureError(
                        f"git blob identity mismatch while materializing {entry.path}"
                    )
                os.fchmod(descriptor, 0o755 if entry.mode == 0o100755 else 0o644)
            finally:
                os.close(descriptor)
        process.stdin.close()
        return_code = process.wait()
        if return_code != 0:
            detail = process.stderr.read().decode("utf-8", errors="replace").strip()
            raise BaselineCaptureError(f"git cat-file failed: {detail}")
    except BaseException:
        process.kill()
        process.wait()
        raise


def _open_exclusive_regular_file(path: Path, git_mode: int) -> int:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        return os.open(path, flags, 0o755 if git_mode == 0o100755 else 0o644)
    except OSError as error:
        raise BaselineCaptureError(
            f"cannot create fixed Git snapshot file {path}: {error}"
        ) from error


def _write_all(descriptor: int, value: bytes) -> None:
    offset = 0
    while offset < len(value):
        written = os.write(descriptor, value[offset:])
        if written <= 0:
            raise BaselineCaptureError("short write while materializing Git blob")
        offset += written


def source_snapshot_sha256(entries: Sequence[GitTreeEntry]) -> str:
    projection = [
        {
            "path": entry.path.as_posix(),
            "mode": f"{entry.mode:o}",
            "oid": entry.oid,
            "size": entry.size,
        }
        for entry in entries
    ]
    return f"sha256:{canonical_sha256({'domain': 'merman-ffi-git-tree-v1', 'entries': projection})}"


def require_unchanged_snapshot(
    snapshot_root: Path,
    expected_entries: Sequence[GitTreeEntry],
    expected_sha256: str,
) -> None:
    observed_entries, observed_directories = _observe_snapshot_entries(snapshot_root)
    observed_sha256 = source_snapshot_sha256(observed_entries)
    if (
        tuple(observed_entries) != tuple(expected_entries)
        or observed_directories != expected_snapshot_directories(expected_entries)
        or observed_sha256 != expected_sha256
    ):
        raise BaselineCaptureError(
            "fixed Git snapshot changed while capturing FFI evidence"
        )


def expected_snapshot_directories(
    entries: Sequence[GitTreeEntry],
) -> tuple[PurePosixPath, ...]:
    directories: set[PurePosixPath] = set()
    for entry in entries:
        parent = entry.path.parent
        while parent != PurePosixPath("."):
            directories.add(parent)
            parent = parent.parent
    return tuple(
        sorted(
            directories,
            key=lambda path: (len(path.parts), path.as_posix()),
        )
    )


def _observe_snapshot_entries(
    snapshot_root: Path,
) -> tuple[tuple[GitTreeEntry, ...], tuple[PurePosixPath, ...]]:
    entries: list[GitTreeEntry] = []
    directories: list[PurePosixPath] = []
    for path in sorted(snapshot_root.rglob("*"), key=lambda item: item.as_posix()):
        metadata = path.lstat()
        relative = PurePosixPath(path.relative_to(snapshot_root).as_posix())
        if stat.S_ISDIR(metadata.st_mode):
            if stat.S_IMODE(metadata.st_mode) != 0o755:
                raise BaselineCaptureError(
                    f"fixed Git snapshot directory mode changed: {relative}"
                )
            directories.append(relative)
            continue
        if not stat.S_ISREG(metadata.st_mode):
            raise BaselineCaptureError(
                f"fixed Git snapshot gained a non-regular entry: {relative}"
            )
        mode = 0o100755 if metadata.st_mode & 0o111 else 0o100644
        oid, size = _git_blob_identity(path)
        entries.append(GitTreeEntry(relative, mode, oid, size))
    return tuple(entries), tuple(
        sorted(
            directories,
            key=lambda path: (len(path.parts), path.as_posix()),
        )
    )


def _git_blob_identity(path: Path) -> tuple[str, int]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode):
                raise OSError("not a regular file")
            digest = hashlib.sha1()
            digest.update(f"blob {before.st_size}\0".encode("ascii"))
            with os.fdopen(descriptor, "rb", closefd=False) as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
            after = os.fstat(descriptor)
            if (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            ) != (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            ):
                raise OSError("file changed while hashing")
        finally:
            os.close(descriptor)
    except OSError as error:
        raise BaselineCaptureError(
            f"cannot inspect fixed Git snapshot file {path}: {error}"
        ) from error
    return digest.hexdigest(), before.st_size


def capture_dependency_report(
    source_root: Path,
    *,
    toolchain: dict[str, Any],
    snapshot_sha256: str,
) -> dict[str, Any]:
    reject_ffi_contract_environment()
    reject_cargo_configuration(source_root)
    probes = load_dependency_probes(source_root)
    observations = capture_probe_observations(
        probes,
        repo_root=source_root,
        cargo_path=toolchain["cargo"]["path"],
        rustc_path=toolchain["rustc"]["path"],
        runner=lambda command: run_text(command, source_root),
    )
    return dependency_baseline_report(
        observations,
        repo_root=source_root,
        toolchain=toolchain,
        source_snapshot_sha256=snapshot_sha256,
    )


def capture_artifact_report(
    source_root: Path,
    output_root: Path,
    *,
    toolchain: dict[str, Any],
    snapshot_sha256: str,
) -> dict[str, Any]:
    measurements = capture_native_artifact_measurements(
        load_native_artifact_profiles(source_root),
        repo_root=source_root,
        output_root=output_root,
        rust_toolchain=toolchain,
        runner=run_text,
    )
    return native_artifact_report(
        measurements,
        repo_root=source_root,
        toolchain=toolchain,
        source_snapshot_sha256=snapshot_sha256,
    )


def write_json(path: Path, value: dict[str, Any]) -> None:
    if path.exists() or path.is_symlink():
        raise BaselineCaptureError(f"capture output already exists: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def finalized_lock(
    *,
    dependency_report: Path,
    native_artifact_report_path: Path,
    source_root: Path,
    snapshot_sha256: str,
) -> dict[str, Any]:
    probes = load_dependency_probes(source_root)
    try:
        baseline_inputs = input_records(source_root, BASELINE_INPUT_PATHS)
        dependency_sha256 = file_sha256(dependency_report)
        native_sha256 = file_sha256(native_artifact_report_path)
    except FfiBaselineContractError as error:
        raise BaselineCaptureError(str(error)) from error
    return {
        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
        "baseline_commit": BASELINE_COMMIT,
        "baseline_input_sha256": {
            record["path"]: record["sha256"] for record in baseline_inputs
        },
        "source_snapshot_sha256": snapshot_sha256,
        "baseline_tree": BASELINE_TREE,
        "dependency_report_schema_version": DEPENDENCY_REPORT_SCHEMA_VERSION,
        "dependency_report_file_sha256": dependency_sha256,
        "native_artifact_report_schema_version": NATIVE_ARTIFACT_SCHEMA_VERSION,
        "native_artifact_report_file_sha256": native_sha256,
        "probe_registry_sha256": probe_registry_sha256(probes),
    }


def capture_baseline_bundle(
    source_repo: Path,
    output_root: Path,
) -> None:
    validate_source_repository(source_repo)
    reject_ffi_contract_environment()
    reject_cargo_configuration(source_repo)
    prepare_output_destination(output_root)
    with tempfile.TemporaryDirectory(
        prefix=".ffi-contract-capture-",
        dir=output_root.parent,
    ) as temporary_directory:
        staging_root = Path(temporary_directory)
        source_root = staging_root / "source"
        bundle_root = staging_root / "bundle"
        native_output_root = staging_root / "native-measurements"
        bundle_root.mkdir()
        source_entries = materialize_git_snapshot(source_repo, source_root)
        reject_cargo_configuration(source_root)
        snapshot_sha256 = source_snapshot_sha256(source_entries)
        require_unchanged_snapshot(
            source_root,
            source_entries,
            snapshot_sha256,
        )
        try:
            toolchain = rust_toolchain_provenance(
                lambda command: run_text(command, source_root)
            )
        except FfiContractReproducibilityError as error:
            raise BaselineCaptureError(str(error)) from error

        dependency_path = bundle_root / "dependency-closures.json"
        write_json(
            dependency_path,
            capture_dependency_report(
                source_root,
                toolchain=toolchain,
                snapshot_sha256=snapshot_sha256,
            ),
        )
        require_unchanged_snapshot(
            source_root,
            source_entries,
            snapshot_sha256,
        )

        artifact_path = bundle_root / "native-artifact-sizes.json"
        write_json(
            artifact_path,
            capture_artifact_report(
                source_root,
                native_output_root,
                toolchain=toolchain,
                snapshot_sha256=snapshot_sha256,
            ),
        )
        require_unchanged_snapshot(
            source_root,
            source_entries,
            snapshot_sha256,
        )

        try:
            observed_toolchain = rust_toolchain_provenance(
                lambda command: run_text(command, source_root)
            )
        except FfiContractReproducibilityError as error:
            raise BaselineCaptureError(str(error)) from error
        if observed_toolchain != toolchain:
            raise BaselineCaptureError(
                "Rust toolchain identity changed while capturing FFI evidence"
            )
        lock_path = bundle_root / "baseline-lock.proposed.json"
        write_json(
            lock_path,
            finalized_lock(
                dependency_report=dependency_path,
                native_artifact_report_path=artifact_path,
                source_root=source_root,
                snapshot_sha256=snapshot_sha256,
            ),
        )
        load_dependency_baseline(
            dependency_path,
            lock_path=lock_path,
            repo_root=source_root,
        )
        load_native_artifact_baseline(
            artifact_path,
            lock_path=lock_path,
            repo_root=source_root,
        )
        require_unchanged_snapshot(
            source_root,
            source_entries,
            snapshot_sha256,
        )
        reject_ffi_contract_environment()
        reject_cargo_configuration(source_root)
        validate_source_repository(source_repo)
        bundle_root.rename(output_root)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-root",
        type=Path,
        required=True,
        help="Git repository containing the fixed baseline commit object.",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=REPO_ROOT / "target" / "ffi-contract-baseline",
    )
    args = parser.parse_args(argv)

    source_repo = args.source_root.expanduser().absolute()
    output_root = args.output_root.expanduser().absolute()
    try:
        capture_baseline_bundle(source_repo, output_root)
    except (BaselineCaptureError, RuntimeError, OSError) as error:
        print(error, file=sys.stderr)
        return 1

    print(f"captured atomic FFI contract baseline under {output_root}")
    print(
        "review baseline-lock.proposed.json and update "
        "abi/ffi-contract-baseline through a normal code review"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
