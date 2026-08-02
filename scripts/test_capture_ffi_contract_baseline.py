#!/usr/bin/env python3
"""Tests for Git-object materialization and atomic FFI baseline publication."""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import ExitStack, contextmanager
import hashlib
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import capture_ffi_contract_baseline as capture  # noqa: E402


class GitTreeSnapshotTests(unittest.TestCase):
    def test_tree_loader_disables_replace_refs_and_rejects_casefold_collisions(
        self,
    ) -> None:
        output = (
            b"100644 blob " + b"1" * 40 + b" 1\tA\0"
            b"100644 blob " + b"2" * 40 + b" 1\ta\0"
        )
        with mock.patch.object(
            capture.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 0, output, b""),
        ) as run:
            with self.assertRaisesRegex(
                capture.BaselineCaptureError,
                "duplicate filesystem path",
            ):
                capture.load_git_tree_entries(Path("/repository"))
        command = run.call_args.args[0]
        self.assertEqual(command[0], "/usr/bin/git")
        self.assertIn("--no-replace-objects", command)
        self.assertIn("--full-tree", command)

    def test_snapshot_change_detection_covers_content_mode_add_and_delete(self) -> None:
        mutations = ("content", "mode", "add", "delete", "empty-directory")
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as td:
                root = Path(td)
                source = root / "source.txt"
                source.write_bytes(b"source")
                oid = hashlib.sha1(b"blob 6\0source").hexdigest()
                entries = (
                    capture.GitTreeEntry(
                        PurePosixPath("source.txt"),
                        0o100644,
                        oid,
                        6,
                    ),
                )
                digest = capture.source_snapshot_sha256(entries)
                capture.require_unchanged_snapshot(root, entries, digest)
                if mutation == "content":
                    source.write_bytes(b"change")
                elif mutation == "mode":
                    source.chmod(0o755)
                elif mutation == "add":
                    (root / "extra.txt").write_text("extra", encoding="utf-8")
                elif mutation == "delete":
                    source.unlink()
                else:
                    (root / "empty").mkdir()
                with self.assertRaisesRegex(
                    capture.BaselineCaptureError,
                    "snapshot changed",
                ):
                    capture.require_unchanged_snapshot(root, entries, digest)

    def test_snapshot_change_detection_covers_directory_mode(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            nested = root / "nested"
            nested.mkdir()
            source = nested / "source.txt"
            source.write_bytes(b"source")
            oid = hashlib.sha1(b"blob 6\0source").hexdigest()
            entries = (
                capture.GitTreeEntry(
                    PurePosixPath("nested/source.txt"),
                    0o100644,
                    oid,
                    6,
                ),
            )
            digest = capture.source_snapshot_sha256(entries)
            capture.require_unchanged_snapshot(root, entries, digest)
            nested.chmod(0o700)

            with self.assertRaisesRegex(
                capture.BaselineCaptureError,
                "directory mode changed",
            ):
                capture.require_unchanged_snapshot(root, entries, digest)


class AtomicCaptureTests(unittest.TestCase):
    def test_native_failure_leaves_no_published_partial_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source_repo = root / "repo"
            source_repo.mkdir()
            output = root / "baseline"
            with self._patched_capture(
                artifact_side_effect=capture.BaselineCaptureError("artifact failed")
            ):
                with self.assertRaisesRegex(
                    capture.BaselineCaptureError,
                    "artifact failed",
                ):
                    capture.capture_baseline_bundle(source_repo, output)
            self.assertFalse(output.exists())

    def test_success_publishes_both_reports_and_finalized_lock_together(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source_repo = root / "repo"
            source_repo.mkdir()
            output = root / "baseline"
            with self._patched_capture(materialize_native_outputs=True):
                capture.capture_baseline_bundle(source_repo, output)
            self.assertEqual(
                sorted(path.name for path in output.iterdir()),
                [
                    "baseline-lock.proposed.json",
                    "dependency-closures.json",
                    "native-artifact-sizes.json",
                ],
            )

    @staticmethod
    @contextmanager
    def _patched_capture(
        *,
        artifact_side_effect: Exception | None = None,
        materialize_native_outputs: bool = False,
    ) -> Iterator[None]:
        entries = (
            capture.GitTreeEntry(
                PurePosixPath("fixture"),
                0o100644,
                "1" * 40,
                1,
            ),
        )

        def materialize(_source: Path, destination: Path) -> tuple[object, ...]:
            destination.mkdir()
            return entries

        def artifact_report(
            _source: Path,
            output_root: Path,
            **_kwargs: object,
        ) -> dict[str, str]:
            if artifact_side_effect is not None:
                raise artifact_side_effect
            if materialize_native_outputs:
                (output_root / "build").mkdir(parents=True)
                (output_root / "artifacts").mkdir()
            return {"report": "native"}

        with ExitStack() as stack:
            stack.enter_context(mock.patch.object(capture, "validate_source_repository"))
            stack.enter_context(
                mock.patch.object(capture, "reject_ffi_contract_environment")
            )
            stack.enter_context(mock.patch.object(capture, "reject_cargo_configuration"))
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "materialize_git_snapshot",
                    side_effect=materialize,
                )
            )
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "source_snapshot_sha256",
                    return_value="sha256:" + "3" * 64,
                )
            )
            stack.enter_context(mock.patch.object(capture, "require_unchanged_snapshot"))
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "rust_toolchain_provenance",
                    return_value={"toolchain": "fixture"},
                )
            )
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "capture_dependency_report",
                    return_value={"report": "dependency"},
                )
            )
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "capture_artifact_report",
                    side_effect=artifact_report,
                )
            )
            stack.enter_context(
                mock.patch.object(
                    capture,
                    "finalized_lock",
                    return_value={"lock": "finalized"},
                )
            )
            stack.enter_context(mock.patch.object(capture, "load_dependency_baseline"))
            stack.enter_context(
                mock.patch.object(capture, "load_native_artifact_baseline")
            )
            yield


if __name__ == "__main__":
    unittest.main()
