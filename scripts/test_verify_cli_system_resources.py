"""Failure-path checks for the native release resource probes."""

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

from scripts import verify_cli_release_archive as archive
from scripts import verify_cli_system_resources as resources


class ResourceProbeTests(unittest.TestCase):
    def test_resource_options_require_native_execution_and_a_pair(self):
        for extra in (
            {"resource_certificate": Path("cert")},
            {"resource_key": Path("key")},
            {"resource_certificate": Path("cert"), "resource_key": Path("key")},
            {"resource_certificate": Path("cert"), "resource_key": Path("key"), "execute": True},
        ):
            with self.subTest(extra=extra), self.assertRaises(archive.ArchiveVerificationError):
                archive.verify_release_archive(
                    Path("missing"), Path("missing.sha256"),
                    target="x86_64-unknown-linux-gnu", version="0.8.0", repo_root=Path("."),
                    **extra,
                )

    def test_system_trust_probe_rejects_ca_environment_overrides(self):
        with mock.patch.dict(resources.os.environ, {"SSL_CERT_FILE": "custom.pem"}):
            with self.assertRaisesRegex(resources.ArchiveVerificationError, "default system trust"):
                resources.verify_system_resources(Path("cli"), Path("cert"), Path("key"))

    def test_font_probe_requires_successful_open_and_real_pdf_text(self):
        for failure in (None, "failed-open", "missing-text", "missing-font"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                font = root / "DejaVuSans.ttf"
                font.write_bytes(b"host font fixture")
                formats = []

                def command_result(command, **kwargs):
                    # Require the same arguments as the bounded production runner.
                    self.assertIn("runner", kwargs)
                    stdout = b""
                    if command[0] == "fc-match":
                        stdout = str(font).encode()
                    elif command[0] == "strace":
                        format = command[command.index("--format") + 1]
                        formats.append(format)
                        result = "-1 ENOENT" if failure == "failed-open" else "3"
                        Path(command[command.index("-o") + 1]).write_text(
                            f'openat(AT_FDCWD, "{font}", O_RDONLY) = {result}\n', encoding="utf-8",
                        )
                        Path(command[command.index("--output") + 1]).write_bytes(b"rendered fixture")
                    elif command[0] == "pdftotext":
                        stdout = b"" if failure == "missing-text" else b"NativeFontEviden\nce\n"
                    elif command[0] == "pdffonts":
                        stdout = b"OtherFont" if failure == "missing-font" else b"ABCDEF+DejaVuSans"
                    return subprocess.CompletedProcess(command, 0, stdout, b"")

                with mock.patch.object(resources, "run_checked", side_effect=command_result):
                    if failure:
                        with self.assertRaises(resources.ArchiveVerificationError):
                            resources.probe_fonts(Path("cli"), root)
                    else:
                        resources.probe_fonts(Path("cli"), root)
                        self.assertEqual(formats, ["png", "jpg", "pdf"])


if __name__ == "__main__":
    unittest.main()
