#!/usr/bin/env python3
"""Verify one cargo-dist merman-lsp archive and its native stdio contract."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable
import json
import os
from pathlib import Path, PurePosixPath
import queue
import subprocess
import sys
import threading
from typing import BinaryIO, TypeAlias

if __package__:
    from .release_archive import (
        ArchiveMember,
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        ExtractionLimits,
        VerificationReport,
        archive_member_path,
        binary_name_for,
        format_set_mismatch,
        git_tracked_legal_files,
        persist_verified_archive,
        regular_files_equal,
        require_repository_root,
        verified_archive_contents,
    )
    from .release_process import (
        drain_bounded_stream,
        HostTargetChecker,
        target_matches_host,
        terminate_process_tree,
    )
else:
    from release_archive import (
        ArchiveMember,
        ArchiveVerificationError,
        DEFAULT_LIMITS,
        ExtractionLimits,
        VerificationReport,
        archive_member_path,
        binary_name_for,
        format_set_mismatch,
        git_tracked_legal_files,
        persist_verified_archive,
        regular_files_equal,
        require_repository_root,
        verified_archive_contents,
    )
    from release_process import (
        drain_bounded_stream,
        HostTargetChecker,
        target_matches_host,
        terminate_process_tree,
    )


PACKAGE_NAME = "merman-lsp"
PACKAGE_README_PATH = "README.md"
ROOT_RELEASE_PATHS = ("CHANGELOG.md", "LICENSE-APACHE", "LICENSE-MIT")
NOTICE_PATH = "THIRD_PARTY_NOTICES.md"
LICENSE_ROOT = "THIRD_PARTY_LICENSES"
LSP_HEADER_MAX_BYTES = 8 * 1024
LSP_BODY_MAX_BYTES = 4 * 1024 * 1024
LSP_SESSION_MAX_BYTES = 8 * 1024 * 1024
LSP_STDERR_MAX_BYTES = 1024 * 1024
LSP_SESSION_MAX_FRAMES = 64
LSP_TIMEOUT_SECONDS = 30

SessionRunner: TypeAlias = Callable[
    [Path, tuple[bytes, bytes, bytes, bytes]], subprocess.CompletedProcess[bytes]
]


def _json_frame(message: dict[str, object]) -> bytes:
    body = json.dumps(message, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    return f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body


def lifecycle_frames() -> tuple[bytes, bytes, bytes, bytes]:
    """Return the ordered client messages for one complete LSP stdio session."""
    return (
        _json_frame(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": None,
                    "rootUri": None,
                    "capabilities": {},
                },
            }
        ),
        _json_frame(
            {
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {},
            }
        ),
        _json_frame(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
            }
        ),
        _json_frame(
            {
                "jsonrpc": "2.0",
                "method": "exit",
                "params": None,
            }
        ),
    )


def _read_exact(stream: BinaryIO, size: int) -> bytes:
    result = bytearray()
    while len(result) < size:
        chunk = stream.read(size - len(result))
        if not chunk:
            raise ArchiveVerificationError("LSP stdout ended inside a protocol frame")
        result.extend(chunk)
    return bytes(result)


def _read_lsp_frame(
    stream: BinaryIO,
    *,
    session_bytes: list[int],
) -> tuple[bytes, bytes]:
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        byte = stream.read(1)
        if not byte:
            raise ArchiveVerificationError("LSP stdout ended before a protocol response")
        header.extend(byte)
        if len(header) > LSP_HEADER_MAX_BYTES:
            raise ArchiveVerificationError("LSP response header exceeds verification budget")

    try:
        lines = header[:-4].decode("ascii").split("\r\n")
    except UnicodeDecodeError as error:
        raise ArchiveVerificationError(f"LSP response header is not ASCII: {error}") from error
    content_lengths = [
        line.removeprefix("Content-Length: ")
        for line in lines
        if line.startswith("Content-Length: ")
    ]
    if len(content_lengths) != 1 or len(lines) != 1:
        raise ArchiveVerificationError(
            "LSP response must contain exactly one Content-Length header"
        )
    try:
        content_length = int(content_lengths[0])
    except ValueError as error:
        raise ArchiveVerificationError("LSP Content-Length must be a decimal integer") from error
    if content_length < 0 or content_length > LSP_BODY_MAX_BYTES:
        raise ArchiveVerificationError("LSP response body exceeds verification budget")
    session_bytes[0] += len(header) + content_length
    if session_bytes[0] > LSP_SESSION_MAX_BYTES:
        raise ArchiveVerificationError("LSP session output exceeds verification budget")
    body = _read_exact(stream, content_length)
    return bytes(header) + body, body


def _interactive_session(
    process: subprocess.Popen[bytes],
    frames: tuple[bytes, bytes, bytes, bytes],
) -> tuple[bytes, bytes]:
    if process.stdin is None or process.stdout is None or process.stderr is None:
        terminate_process_tree(process)
        process.wait()
        raise ArchiveVerificationError("LSP process pipes were not created")

    stderr = bytearray()
    stderr_exceeded = threading.Event()
    stderr_reader = threading.Thread(
        target=drain_bounded_stream,
        args=(process.stderr, stderr, stderr_exceeded, LSP_STDERR_MAX_BYTES),
        daemon=True,
    )
    stderr_reader.start()
    responses: queue.Queue[bytes | BaseException] = queue.Queue(maxsize=1)

    def exchange() -> None:
        try:
            session_bytes = [0]
            process.stdin.write(frames[0])
            process.stdin.flush()
            initialize, initialize_body = _read_lsp_frame(
                process.stdout,
                session_bytes=session_bytes,
            )
            _validate_response(
                initialize_body,
                request_id=1,
                label="initialize response",
            )
            process.stdin.write(frames[1] + frames[2])
            process.stdin.flush()
            observed = [initialize]
            for _frame_index in range(LSP_SESSION_MAX_FRAMES - 1):
                frame, body = _read_lsp_frame(
                    process.stdout,
                    session_bytes=session_bytes,
                )
                observed.append(frame)
                message = _strict_json_object(body, label="LSP server message")
                if message.get("id") == 2:
                    _require_response(message, request_id=2, label="shutdown response")
                    break
                if (
                    message.get("jsonrpc") != "2.0"
                    or not isinstance(message.get("method"), str)
                    or "id" in message
                ):
                    raise ArchiveVerificationError(
                        "LSP server emitted an unexpected message before shutdown"
                    )
            else:
                raise ArchiveVerificationError(
                    "LSP server did not return shutdown within the frame budget"
                )
            process.stdin.write(frames[3])
            process.stdin.flush()
            process.stdin.close()
            while chunk := process.stdout.read(64 * 1024):
                session_bytes[0] += len(chunk)
                if session_bytes[0] > LSP_SESSION_MAX_BYTES:
                    raise ArchiveVerificationError(
                        "LSP session output exceeds verification budget"
                    )
                observed.append(chunk)
            responses.put(b"".join(observed))
        except BaseException as error:
            responses.put(error)

    worker = threading.Thread(target=exchange, daemon=True)
    worker.start()
    try:
        try:
            exchanged = responses.get(timeout=LSP_TIMEOUT_SECONDS)
        except queue.Empty as error:
            raise subprocess.TimeoutExpired([str(process.args)], LSP_TIMEOUT_SECONDS) from error
        if isinstance(exchanged, BaseException):
            raise exchanged
        try:
            return_code = process.wait(timeout=LSP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            raise
        worker.join()
        stderr_reader.join()
        if stderr_exceeded.is_set():
            raise ArchiveVerificationError("LSP stderr exceeds verification budget")
        if return_code != 0:
            detail = stderr.decode("utf-8", errors="replace").strip() or "no diagnostic"
            raise ArchiveVerificationError(
                f"LSP lifecycle failed with exit {return_code}: {detail}"
            )
        return exchanged, bytes(stderr)
    except BaseException:
        terminate_process_tree(process)
        process.wait()
        worker.join()
        stderr_reader.join()
        raise


def run_lsp_session(
    binary: Path,
    frames: tuple[bytes, bytes, bytes, bytes],
) -> subprocess.CompletedProcess[bytes]:
    """Run one ordered, bounded stdio lifecycle against a native LSP binary."""
    environment = os.environ.copy()
    environment["NO_COLOR"] = "1"
    process = subprocess.Popen(
        [str(binary)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=binary.parent,
        env=environment,
        start_new_session=os.name == "posix",
    )
    try:
        stdout, stderr = _interactive_session(process, frames)
    finally:
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None:
                stream.close()
    return subprocess.CompletedProcess([str(binary)], 0, stdout=stdout, stderr=stderr)


def _strict_json_object(payload: bytes, *, label: str) -> dict[str, object]:
    def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ArchiveVerificationError(f"{label} contains duplicate field {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(payload.decode("utf-8"), object_pairs_hook=reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ArchiveVerificationError(f"{label} is not valid UTF-8 JSON: {error}") from error
    if not isinstance(value, dict):
        raise ArchiveVerificationError(f"{label} must be one JSON object")
    return value


def _require_response(
    response: dict[str, object],
    *,
    request_id: int,
    label: str,
) -> None:
    response_id = response.get("id")
    if (
        response.get("jsonrpc") != "2.0"
        or type(response_id) is not int
        or response_id != request_id
    ):
        raise ArchiveVerificationError(f"{label} has the wrong JSON-RPC version or request id")
    if "error" in response or "result" not in response:
        raise ArchiveVerificationError(f"{label} must be a successful JSON-RPC response")


def _validate_response(payload: bytes, *, request_id: int, label: str) -> dict[str, object]:
    response = _strict_json_object(payload, label=label)
    _require_response(response, request_id=request_id, label=label)
    return response


def _decode_session(payload: bytes) -> list[dict[str, object]]:
    from io import BytesIO

    stream = BytesIO(payload)
    session_bytes = [0]
    messages = []
    while stream.tell() < len(payload):
        if len(messages) >= LSP_SESSION_MAX_FRAMES:
            raise ArchiveVerificationError("LSP session exceeds the frame-count budget")
        try:
            _, body = _read_lsp_frame(stream, session_bytes=session_bytes)
        except ArchiveVerificationError as error:
            if messages:
                raise ArchiveVerificationError(
                    "LSP stdout contains incomplete or non-frame trailing data"
                ) from error
            raise
        messages.append(_strict_json_object(body, label="LSP server message"))
    return messages


def verify_runtime_contract(
    binary: Path,
    *,
    target: str,
    runner: SessionRunner = run_lsp_session,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> None:
    """Execute one native initialize/shutdown/exit lifecycle."""
    if not host_target_checker(target):
        raise ArchiveVerificationError(
            f"refusing to execute archive target {target!r} on this host"
        )
    result = runner(binary, lifecycle_frames())
    if not isinstance(result.stdout, bytes) or not isinstance(result.stderr, bytes):
        raise ArchiveVerificationError("LSP session runner must return byte output")
    if len(result.stderr) > LSP_STDERR_MAX_BYTES:
        raise ArchiveVerificationError("LSP stderr exceeds verification budget")
    if result.returncode != 0:
        raise ArchiveVerificationError("LSP lifecycle must exit successfully")
    messages = _decode_session(result.stdout)
    if len(messages) < 2:
        raise ArchiveVerificationError("LSP lifecycle returned too few protocol frames")
    initialize = messages[0]
    _require_response(
        initialize,
        request_id=1,
        label="initialize response",
    )
    capabilities = initialize["result"]
    if not isinstance(capabilities, dict) or not isinstance(capabilities.get("capabilities"), dict):
        raise ArchiveVerificationError(
            "initialize response result must contain a capabilities object"
        )
    shutdown: dict[str, object] | None = None
    for message in messages[1:]:
        if message.get("id") == 2:
            if shutdown is not None:
                raise ArchiveVerificationError(
                    "LSP lifecycle emitted more than one shutdown response"
                )
            _require_response(
                message,
                request_id=2,
                label="shutdown response",
            )
            shutdown = message
            continue
        if (
            message.get("jsonrpc") != "2.0"
            or not isinstance(message.get("method"), str)
            or "id" in message
        ):
            raise ArchiveVerificationError(
                "LSP lifecycle emitted an unexpected intermediate message"
            )
    if shutdown is None:
        raise ArchiveVerificationError("LSP lifecycle did not return a shutdown response")
    if shutdown["result"] is not None:
        raise ArchiveVerificationError("shutdown response result must be null")


def _require_distribution_contents(
    root: Path,
    members: Iterable[ArchiveMember],
    *,
    target: str,
    source_files: dict[str, Path],
) -> None:
    regular = {
        member.logical_path: member
        for member in members
        if not member.is_directory
    }
    binary_name = binary_name_for(PACKAGE_NAME, target)
    expected = {binary_name, *source_files}
    if set(regular) != expected:
        raise ArchiveVerificationError(
            format_set_mismatch("LSP payload", expected, set(regular))
        )
    binary_candidates = [
        path for path in regular if PurePosixPath(path).name == binary_name
    ]
    if binary_candidates != [binary_name]:
        raise ArchiveVerificationError(
            f"archive must contain exactly one root {binary_name!r} binary"
        )
    for relative in sorted(expected):
        member = regular[relative]
        archived = archive_member_path(root, relative)
        if member.size == 0 or archived.stat().st_size == 0:
            raise ArchiveVerificationError(f"required archive file is empty: {relative!r}")
        source = source_files.get(relative)
        if source is not None and not regular_files_equal(archived, source):
            raise ArchiveVerificationError(
                f"archive content differs from repository file {relative!r}"
            )


def _repository_distribution_files(repo_root: Path) -> dict[str, Path]:
    return {
        PACKAGE_README_PATH: repo_root / "crates/merman-lsp/README.md",
        **{relative: repo_root / relative for relative in ROOT_RELEASE_PATHS},
        **git_tracked_legal_files(repo_root),
    }


def verify_release_archive(
    archive: Path,
    checksum: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    verified_output: Path | None = None,
    execute: bool = False,
    limits: ExtractionLimits = DEFAULT_LIMITS,
    runner: SessionRunner = run_lsp_session,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> VerificationReport:
    """Verify one LSP archive and optionally persist its checksum-bound bytes."""
    repo_root = require_repository_root(Path(repo_root))
    source_files = _repository_distribution_files(repo_root)
    with verified_archive_contents(
        Path(archive),
        Path(checksum),
        package_name=PACKAGE_NAME,
        target=target,
        version=version,
        limits=limits,
    ) as extracted:
        _require_distribution_contents(
            extracted.root,
            extracted.members,
            target=target,
            source_files=source_files,
        )
        if execute:
            verify_runtime_contract(
                archive_member_path(extracted.root, extracted.binary_path),
                target=target,
                runner=runner,
                host_target_checker=host_target_checker,
            )
        persisted = (
            persist_verified_archive(extracted, verified_output, limits=limits)
            if verified_output is not None
            else Path(archive).resolve()
        )
        return VerificationReport(
            archive=persisted,
            digest=extracted.digest,
            target=target,
            member_count=len(extracted.members),
            total_uncompressed_bytes=sum(member.size for member in extracted.members),
            binary_path=extracted.binary_path,
        )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="cargo-dist .tar.xz or .zip archive")
    parser.add_argument(
        "--checksum",
        type=Path,
        help="adjacent .sha256 file (defaults to ARCHIVE.sha256)",
    )
    parser.add_argument("--target", required=True, help="Rust target triple carried by the archive")
    parser.add_argument("--version", required=True, help="expected merman-lsp package version")
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--verified-output", type=Path)
    parser.add_argument(
        "--execute",
        action="store_true",
        help="run a stdio lifecycle when TARGET matches the current host",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    checksum = args.checksum or args.archive.with_name(f"{args.archive.name}.sha256")
    report = verify_release_archive(
        args.archive,
        checksum,
        target=args.target,
        version=args.version,
        repo_root=args.repo_root,
        verified_output=args.verified_output,
        execute=args.execute,
    )
    print(f"verified {report.archive}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveVerificationError, OSError, subprocess.TimeoutExpired) as error:
        print(f"verify_lsp_release_archive.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
