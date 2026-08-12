#!/usr/bin/env python3
"""Contract tests for the standalone LSP release-archive verifier."""

from __future__ import annotations

import hashlib
from io import BytesIO
import json
import lzma
import os
from pathlib import Path
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import verify_lsp_release_archive as verifier


LINUX_TARGET = "x86_64-unknown-linux-gnu"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
VERSION = "0.8.0-alpha.4"
LEGAL_FILES = {
    verifier.NOTICE_PATH: b"Third-party notices\n",
    f"{verifier.LICENSE_ROOT}/example/LICENSE": b"Example license\n",
}
ROOT_RELEASE_FILES = {
    "CHANGELOG.md": b"Release changes\n",
    "LICENSE-APACHE": b"Apache license\n",
    "LICENSE-MIT": b"MIT license\n",
}
PACKAGE_README = b"Merman LSP package readme\n"


def write_repository(root: Path) -> None:
    package_readme = root / "crates/merman-lsp/README.md"
    package_readme.parent.mkdir(parents=True, exist_ok=True)
    package_readme.write_bytes(PACKAGE_README)
    for relative, content in ROOT_RELEASE_FILES.items():
        (root / relative).write_bytes(content)
    for relative, content in LEGAL_FILES.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(
        ["git", "-C", str(root), "add", "--", *LEGAL_FILES],
        check=True,
    )


def archive_files(target: str) -> dict[str, bytes]:
    binary = verifier.binary_name_for(verifier.PACKAGE_NAME, target)
    return {
        binary: b"standalone-lsp-binary\n",
        verifier.PACKAGE_README_PATH: PACKAGE_README,
        **ROOT_RELEASE_FILES,
        **LEGAL_FILES,
    }


def write_checksum(archive: Path) -> Path:
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_name(f"{archive.name}.sha256")
    checksum.write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    return checksum


def write_tar(root: Path, files: dict[str, bytes]) -> tuple[Path, Path]:
    archive = root / f"{verifier.PACKAGE_NAME}-{LINUX_TARGET}.tar.xz"
    wrapper = archive.name.removesuffix(".tar.xz")
    with tarfile.open(archive, "w:xz") as output:
        directory = tarfile.TarInfo(wrapper)
        directory.type = tarfile.DIRTYPE
        directory.mode = 0o755
        output.addfile(directory)
        for relative, content in files.items():
            info = tarfile.TarInfo(f"{wrapper}/{relative}")
            info.size = len(content)
            info.mode = 0o755 if relative == verifier.PACKAGE_NAME else 0o644
            output.addfile(info, BytesIO(content))
    return archive, write_checksum(archive)


def write_zip(root: Path, files: dict[str, bytes]) -> tuple[Path, Path]:
    archive = root / f"{verifier.PACKAGE_NAME}-{WINDOWS_TARGET}.zip"
    binary = verifier.binary_name_for(verifier.PACKAGE_NAME, WINDOWS_TARGET)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for relative, content in files.items():
            info = zipfile.ZipInfo(relative)
            info.create_system = 3
            mode = 0o755 if relative == binary else 0o644
            info.external_attr = (stat.S_IFREG | mode) << 16
            output.writestr(info, content)
    return archive, write_checksum(archive)


def response_frame(request_id: int, result: object) -> bytes:
    body = json.dumps(
        {"jsonrpc": "2.0", "id": request_id, "result": result},
        separators=(",", ":"),
    ).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


def response_frame_with_headers(
    request_id: int,
    result: object,
    *headers: str,
) -> bytes:
    body = json.dumps(
        {"jsonrpc": "2.0", "id": request_id, "result": result},
        separators=(",", ":"),
    ).encode()
    lines = [*headers, f"content-length:{len(body)}"]
    return ("\r\n".join(lines) + "\r\n\r\n").encode("ascii") + body


def notification_frame(method: str, params: object) -> bytes:
    body = json.dumps(
        {"jsonrpc": "2.0", "method": method, "params": params},
        separators=(",", ":"),
    ).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


def valid_session_output() -> bytes:
    return response_frame(1, {"capabilities": {}}) + response_frame(2, None)


def write_test_server(root: Path, behavior: str) -> Path:
    script = root / f"lsp-{behavior}.py"
    source = r'''#!/usr/bin/env python3
import json
import sys
import time

BEHAVIOR = __BEHAVIOR__

def read_message():
    header = sys.stdin.buffer.readline()
    if not header.startswith(b"Content-Length: "):
        raise SystemExit(91)
    length = int(header.removeprefix(b"Content-Length: ").strip())
    if sys.stdin.buffer.readline() != b"\r\n":
        raise SystemExit(92)
    return json.loads(sys.stdin.buffer.read(length))

def respond(request_id, result):
    body = json.dumps(
        {"jsonrpc": "2.0", "id": request_id, "result": result},
        separators=(",", ":"),
    ).encode()
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    sys.stdout.buffer.flush()

initialize = read_message()
if initialize.get("method") != "initialize":
    raise SystemExit(93)
if BEHAVIOR == "timeout":
    time.sleep(60)
respond(1, {"capabilities": {}})
if read_message().get("method") != "initialized":
    raise SystemExit(94)
shutdown = read_message()
if shutdown.get("method") != "shutdown":
    raise SystemExit(95)
respond(2, None)
if read_message().get("method") != "exit":
    raise SystemExit(96)
if BEHAVIOR == "stderr":
    sys.stderr.buffer.write(b"x" * (__STDERR_LIMIT__ + 1))
    sys.stderr.buffer.flush()
if BEHAVIOR == "nonzero":
    raise SystemExit(7)
'''
    script.write_text(
        source.replace("__BEHAVIOR__", repr(behavior)).replace(
            "__STDERR_LIMIT__", str(verifier.LSP_STDERR_MAX_BYTES)
        ),
        encoding="utf-8",
    )
    script.chmod(0o755)
    return script


class LspArchiveTests(unittest.TestCase):
    def test_linux_archive_is_verified_and_persisted_without_execution(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            repo = temp / "repo"
            repo.mkdir()
            write_repository(repo)
            archive, checksum = write_tar(temp, archive_files(LINUX_TARGET))
            output_dir = temp / "verified"
            output_dir.mkdir()
            output = output_dir / archive.name

            report = verifier.verify_release_archive(
                archive,
                checksum,
                target=LINUX_TARGET,
                version=VERSION,
                repo_root=repo,
                verified_output=output,
            )

            self.assertEqual(report.archive, output.resolve())
            self.assertEqual(output.read_bytes(), archive.read_bytes())
            self.assertEqual(report.binary_path, verifier.PACKAGE_NAME)

    def test_windows_archive_is_verified(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            repo = temp / "repo"
            repo.mkdir()
            write_repository(repo)
            archive, checksum = write_zip(temp, archive_files(WINDOWS_TARGET))
            output_dir = temp / "verified"
            output_dir.mkdir()

            report = verifier.verify_release_archive(
                archive,
                checksum,
                target=WINDOWS_TARGET,
                version=VERSION,
                repo_root=repo,
            )

            self.assertEqual(report.binary_path, "merman-lsp.exe")
            self.assertEqual(report.archive, archive.resolve())
            self.assertEqual(tuple(output_dir.iterdir()), ())

    def test_archive_rejects_extra_regular_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            repo = temp / "repo"
            repo.mkdir()
            write_repository(repo)
            files = archive_files(LINUX_TARGET)
            files["unexpected.txt"] = b"unexpected\n"
            archive, checksum = write_tar(temp, files)
            output_dir = temp / "verified"
            output_dir.mkdir()

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "unexpected unexpected.txt",
            ):
                verifier.verify_release_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                    verified_output=output_dir / archive.name,
                )

    def test_archive_rejects_dll_and_hidden_payloads(self) -> None:
        for relative in ("helper.dll", ".hidden"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                repo = temp / "repo"
                repo.mkdir()
                write_repository(repo)
                files = archive_files(WINDOWS_TARGET)
                files[relative] = b"unexpected\n"
                archive, checksum = write_zip(temp, files)

                with self.assertRaisesRegex(
                    verifier.ArchiveVerificationError,
                    "unexpected",
                ):
                    verifier.verify_release_archive(
                        archive,
                        checksum,
                        target=WINDOWS_TARGET,
                        version=VERSION,
                        repo_root=repo,
                    )

    def test_archive_rejects_modified_legal_material(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            repo = temp / "repo"
            repo.mkdir()
            write_repository(repo)
            files = archive_files(LINUX_TARGET)
            files[verifier.NOTICE_PATH] = b"different\n"
            archive, checksum = write_tar(temp, files)
            output_dir = temp / "verified"
            output_dir.mkdir()

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "content differs",
            ):
                verifier.verify_release_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                    verified_output=output_dir / archive.name,
                )

    def test_archive_rejects_modified_readme_and_root_release_files(self) -> None:
        for relative in (
            verifier.PACKAGE_README_PATH,
            *verifier.ROOT_RELEASE_PATHS,
        ):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                repo = temp / "repo"
                repo.mkdir()
                write_repository(repo)
                files = archive_files(LINUX_TARGET)
                files[relative] = b"different\n"
                archive, checksum = write_tar(temp, files)

                with self.assertRaisesRegex(
                    verifier.ArchiveVerificationError,
                    "content differs",
                ):
                    verifier.verify_release_archive(
                        archive,
                        checksum,
                        target=LINUX_TARGET,
                        version=VERSION,
                        repo_root=repo,
                    )

    def test_shared_archive_core_rejects_trailing_xz_stream_data(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            repo = temp / "repo"
            repo.mkdir()
            write_repository(repo)
            archive, _checksum = write_tar(temp, archive_files(LINUX_TARGET))
            archive.write_bytes(archive.read_bytes() + lzma.compress(b"trailing"))
            checksum = write_checksum(archive)
            output_dir = temp / "verified"
            output_dir.mkdir()

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "trailing or concatenated",
            ):
                verifier.verify_release_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                    verified_output=output_dir / archive.name,
                )


class LspRuntimeTests(unittest.TestCase):
    def test_lifecycle_frames_have_the_required_order(self) -> None:
        methods = []
        for frame in verifier.lifecycle_frames():
            header, body = frame.split(b"\r\n\r\n", maxsplit=1)
            self.assertEqual(header, f"Content-Length: {len(body)}".encode())
            methods.append(json.loads(body)["method"])

        self.assertEqual(methods, ["initialize", "initialized", "shutdown", "exit"])

    def test_runtime_contract_accepts_complete_lifecycle(self) -> None:
        observed: list[tuple[bytes, bytes, bytes, bytes]] = []

        def runner(
            _binary: Path,
            frames: tuple[bytes, bytes, bytes, bytes],
        ) -> subprocess.CompletedProcess[bytes]:
            observed.append(frames)
            return subprocess.CompletedProcess(
                ["merman-lsp"],
                0,
                stdout=valid_session_output(),
                stderr=b"normal LSP diagnostics\n",
            )

        verifier.verify_runtime_contract(
            Path("merman-lsp"),
            target=LINUX_TARGET,
            runner=runner,
            host_target_checker=lambda _target: True,
        )

        self.assertEqual(len(observed), 1)
        self.assertEqual(observed[0], verifier.lifecycle_frames())

    def test_runtime_contract_accepts_notifications_around_shutdown_response(self) -> None:
        initialize = response_frame(1, {"capabilities": {}})
        shutdown = response_frame(2, None)
        initialized_log = notification_frame(
            "window/logMessage",
            {"type": 3, "message": "merman-lsp initialized"},
        )

        for output in (
            initialize + initialized_log + shutdown,
            initialize + shutdown + initialized_log,
        ):
            with self.subTest(output=output):
                verifier.verify_runtime_contract(
                    Path("merman-lsp"),
                    target=LINUX_TARGET,
                    runner=lambda _binary, _frames, output=output: subprocess.CompletedProcess(
                        ["merman-lsp"], 0, stdout=output, stderr=b""
                    ),
                    host_target_checker=lambda _target: True,
                )

    def test_runtime_contract_accepts_legal_header_representations(self) -> None:
        output = response_frame_with_headers(
            1,
            {"capabilities": {}},
            "Content-Type: application/vscode-jsonrpc; charset=utf-8",
            "X-Trace-Id: smoke",
        ) + response_frame_with_headers(2, None)

        verifier.verify_runtime_contract(
            Path("merman-lsp"),
            target=LINUX_TARGET,
            runner=lambda _binary, _frames: subprocess.CompletedProcess(
                ["merman-lsp"], 0, stdout=output, stderr=b""
            ),
            host_target_checker=lambda _target: True,
        )

    def test_runtime_contract_rejects_duplicate_content_length(self) -> None:
        body = b'{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}'
        output = (
            f"Content-Length: {len(body)}\r\ncontent-length: {len(body)}\r\n\r\n".encode()
            + body
            + response_frame(2, None)
        )

        with self.assertRaisesRegex(
            verifier.ArchiveVerificationError,
            "more than one Content-Length",
        ):
            verifier.verify_runtime_contract(
                Path("merman-lsp"),
                target=LINUX_TARGET,
                runner=lambda _binary, _frames: subprocess.CompletedProcess(
                    ["merman-lsp"], 0, stdout=output, stderr=b""
                ),
                host_target_checker=lambda _target: True,
            )

    def test_runtime_contract_rejects_duplicate_shutdown_response(self) -> None:
        output = valid_session_output() + response_frame(2, None)

        with self.assertRaisesRegex(
            verifier.ArchiveVerificationError,
            "more than one shutdown response",
        ):
            verifier.verify_runtime_contract(
                Path("merman-lsp"),
                target=LINUX_TARGET,
                runner=lambda _binary, _frames: subprocess.CompletedProcess(
                    ["merman-lsp"], 0, stdout=output, stderr=b""
                ),
                host_target_checker=lambda _target: True,
            )

    @unittest.skipUnless(os.name == "posix", "native process fixture requires POSIX shebangs")
    def test_native_process_driver_completes_the_stdio_lifecycle(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            server = write_test_server(Path(temp_dir), "success")
            verifier.verify_runtime_contract(
                server,
                target=LINUX_TARGET,
                host_target_checker=lambda _target: True,
            )

    @unittest.skipUnless(os.name == "posix", "native process fixture requires POSIX shebangs")
    def test_native_process_driver_fails_closed(self) -> None:
        for behavior, error, timeout in (
            ("timeout", subprocess.TimeoutExpired, 0.2),
            ("stderr", verifier.ArchiveVerificationError, 2.0),
            ("nonzero", verifier.ArchiveVerificationError, 2.0),
        ):
            with self.subTest(behavior=behavior), tempfile.TemporaryDirectory() as temp_dir:
                server = write_test_server(Path(temp_dir), behavior)
                with (
                    mock.patch.object(verifier, "LSP_TIMEOUT_SECONDS", timeout),
                    self.assertRaises(error),
                ):
                    verifier.verify_runtime_contract(
                        server,
                        target=LINUX_TARGET,
                        host_target_checker=lambda _target: True,
                    )

    def test_runtime_contract_refuses_cross_target_execution_before_spawn(self) -> None:
        spawned = False

        def runner(
            _binary: Path,
            _frames: tuple[bytes, bytes, bytes, bytes],
        ) -> subprocess.CompletedProcess[bytes]:
            nonlocal spawned
            spawned = True
            raise AssertionError("runner must not be called")

        with self.assertRaisesRegex(
            verifier.ArchiveVerificationError,
            "refusing to execute",
        ):
            verifier.verify_runtime_contract(
                Path("merman-lsp"),
                target=WINDOWS_TARGET,
                runner=runner,
                host_target_checker=lambda _target: False,
            )
        self.assertFalse(spawned)

    def test_runtime_contract_rejects_wrong_response_id(self) -> None:
        output = response_frame(9, {"capabilities": {}}) + response_frame(2, None)

        with self.assertRaisesRegex(verifier.ArchiveVerificationError, "request id"):
            verifier.verify_runtime_contract(
                Path("merman-lsp"),
                target=LINUX_TARGET,
                runner=lambda _binary, _frames: subprocess.CompletedProcess(
                    ["merman-lsp"], 0, stdout=output, stderr=b""
                ),
                host_target_checker=lambda _target: True,
            )

    def test_runtime_contract_rejects_error_or_non_null_shutdown(self) -> None:
        cases = {
            "initialize error": (
                b'Content-Length: 52\r\n\r\n'
                b'{"jsonrpc":"2.0","id":1,"error":{"code":-32603}}'
                + response_frame(2, None)
            ),
            "shutdown result": (
                response_frame(1, {"capabilities": {}})
                + response_frame(2, {"unexpected": True})
            ),
        }
        for label, output in cases.items():
            with self.subTest(label=label), self.assertRaises(verifier.ArchiveVerificationError):
                verifier.verify_runtime_contract(
                    Path("merman-lsp"),
                    target=LINUX_TARGET,
                    runner=lambda _binary, _frames, output=output: subprocess.CompletedProcess(
                        ["merman-lsp"], 0, stdout=output, stderr=b""
                    ),
                    host_target_checker=lambda _target: True,
                )

    def test_runtime_contract_rejects_non_frame_trailing_output(self) -> None:
        with self.assertRaisesRegex(verifier.ArchiveVerificationError, "trailing data"):
            verifier.verify_runtime_contract(
                Path("merman-lsp"),
                target=LINUX_TARGET,
                runner=lambda _binary, _frames: subprocess.CompletedProcess(
                    ["merman-lsp"],
                    0,
                    stdout=valid_session_output() + b"noise",
                    stderr=b"",
                ),
                host_target_checker=lambda _target: True,
            )


if __name__ == "__main__":
    unittest.main()
