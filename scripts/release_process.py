#!/usr/bin/env python3
"""Shared bounded-process primitives for native release verification."""

from __future__ import annotations

from collections.abc import Callable
import os
from pathlib import Path
import platform
import signal
import subprocess
import threading
import time
from typing import BinaryIO, TypeAlias

if __package__:
    from .release_archive import ArchiveVerificationError
else:
    from release_archive import ArchiveVerificationError


__all__ = (
    "CommandRunner",
    "HostTargetChecker",
    "drain_bounded_stream",
    "run_checked",
    "target_matches_host",
    "terminate_process_tree",
)


RUNTIME_OUTPUT_MAX_BYTES = 16 * 1024 * 1024
RUNTIME_TIMEOUT_SECONDS = 30

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[bytes]]
HostTargetChecker: TypeAlias = Callable[[str], bool]


def target_matches_host(target: str) -> bool:
    """Return whether TARGET exactly matches an admitted native release runner."""
    system = platform.system().lower()
    machine = platform.machine().lower()
    architecture = {
        "amd64": "x86_64",
        "x86_64": "x86_64",
        "arm64": "aarch64",
        "aarch64": "aarch64",
    }.get(machine)
    if architecture is None:
        return False
    abi: str | None = None
    if system == "linux":
        libc_name = platform.libc_ver()[0].lower()
        if libc_name in {"glibc", "gnu libc"}:
            abi = "gnu"
        elif "musl" in libc_name:
            abi = "musl"
        else:
            return False
    admitted_targets = {
        ("darwin", "aarch64", None): "aarch64-apple-darwin",
        ("darwin", "x86_64", None): "x86_64-apple-darwin",
        ("linux", "x86_64", "gnu"): "x86_64-unknown-linux-gnu",
        ("windows", "x86_64", None): "x86_64-pc-windows-msvc",
    }
    return admitted_targets.get((system, architecture, abi)) == target


def terminate_process_tree(process: subprocess.Popen[bytes]) -> None:
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except (PermissionError, ProcessLookupError):
            if process.poll() is None:
                process.kill()
    elif process.poll() is None:
        process.kill()


def drain_bounded_stream(
    stream: BinaryIO,
    output: bytearray,
    exceeded: threading.Event,
    max_bytes: int = RUNTIME_OUTPUT_MAX_BYTES,
) -> None:
    try:
        while True:
            chunk = stream.read(64 * 1024)
            if not chunk:
                return
            remaining = max_bytes - len(output)
            if len(chunk) > remaining:
                output.extend(chunk[:remaining])
                exceeded.set()
                return
            output.extend(chunk)
    finally:
        stream.close()


def _run_subprocess_bounded(
    command: list[str],
    *,
    stdin: bytes,
    cwd: Path,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[bytes]:
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=cwd,
        env=environment,
        start_new_session=os.name == "posix",
    )
    if process.stdin is None or process.stdout is None or process.stderr is None:
        terminate_process_tree(process)
        process.wait()
        raise ArchiveVerificationError("runtime process pipes were not created")

    stdout = bytearray()
    stderr = bytearray()
    exceeded = threading.Event()
    readers = [
        threading.Thread(
            target=drain_bounded_stream,
            args=(process.stdout, stdout, exceeded),
            daemon=True,
        ),
        threading.Thread(
            target=drain_bounded_stream,
            args=(process.stderr, stderr, exceeded),
            daemon=True,
        ),
    ]
    for reader in readers:
        reader.start()

    try:
        try:
            process.stdin.write(stdin)
            process.stdin.flush()
        except BrokenPipeError:
            pass
        finally:
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass

        deadline = time.monotonic() + RUNTIME_TIMEOUT_SECONDS
        timed_out = False
        while True:
            if exceeded.is_set():
                terminate_process_tree(process)
                break
            if process.poll() is not None and all(not reader.is_alive() for reader in readers):
                break
            if time.monotonic() >= deadline:
                timed_out = True
                terminate_process_tree(process)
                break
            time.sleep(0.01)

        process.wait()
        for reader in readers:
            reader.join()
        if timed_out:
            raise subprocess.TimeoutExpired(
                command,
                RUNTIME_TIMEOUT_SECONDS,
                output=bytes(stdout),
                stderr=bytes(stderr),
            )
        if exceeded.is_set():
            raise ArchiveVerificationError("runtime command output exceeds verification budget")
        return subprocess.CompletedProcess(
            command,
            process.returncode,
            stdout=bytes(stdout),
            stderr=bytes(stderr),
        )
    except BaseException:
        terminate_process_tree(process)
        process.wait()
        for reader in readers:
            reader.join()
        raise


def run_checked(
    command: list[str],
    *,
    stdin: bytes,
    cwd: Path,
    runner: CommandRunner,
) -> subprocess.CompletedProcess[bytes]:
    environment = os.environ.copy()
    environment["NO_COLOR"] = "1"
    if runner is subprocess.run:
        completed = _run_subprocess_bounded(
            command,
            stdin=stdin,
            cwd=cwd,
            environment=environment,
        )
    else:
        completed = runner(
            command,
            input=stdin,
            cwd=cwd,
            env=environment,
            capture_output=True,
            check=False,
            timeout=RUNTIME_TIMEOUT_SECONDS,
        )
    if not isinstance(completed.stdout, bytes) or not isinstance(completed.stderr, bytes):
        raise ArchiveVerificationError("runtime command runner must return byte output")
    if (
        len(completed.stdout) > RUNTIME_OUTPUT_MAX_BYTES
        or len(completed.stderr) > RUNTIME_OUTPUT_MAX_BYTES
    ):
        raise ArchiveVerificationError("runtime command output exceeds verification budget")
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip() or "no diagnostic"
        raise ArchiveVerificationError(
            f"runtime command failed with exit {completed.returncode}: {detail}"
        )
    return completed
