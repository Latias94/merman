#!/usr/bin/env python3
"""Tests for PyPI wheel reconciliation."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from zipfile import ZipFile, ZIP_DEFLATED
from unittest import mock

from scripts import reconcile_pypi_wheels as reconcile


def wheel(path: Path, *, name: str = "merman", version: str = "0.8.0a6", payload: bytes = b"wheel") -> None:
    dist_info = "merman-0.8.0a6.dist-info"
    with ZipFile(path, "w", ZIP_DEFLATED) as archive:
        archive.writestr(
            f"{dist_info}/METADATA",
            f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n",
        )
        archive.writestr("merman/example.py", payload)


class PyPIWheelReconciliationTests(unittest.TestCase):
    def test_workspace_semver_is_projected_to_pep440(self) -> None:
        self.assertEqual(
            reconcile.normalize_version("0.8.0-alpha.6+Build-1"),
            "0.8.0a6+build.1",
        )

    def response(self, payload: dict):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return json.dumps(payload).encode()

        return Response()

    def test_missing_project_is_safe_to_upload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)

            def opener(*_args, **_kwargs):
                error = reconcile.urllib.error.HTTPError(
                    "url", 404, "missing", {}, io.BytesIO()
                )
                error.close()
                raise error

            self.assertFalse(reconcile.reconcile(Path(temp_dir), "merman", "0.8.0a6", opener=opener))

    def test_exact_existing_wheel_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            payload = {"releases": {"0.8.0a6": [{"filename": path.name, "digests": {"sha256": digest}}]}}
            self.assertTrue(
                reconcile.reconcile(
                    Path(temp_dir),
                    "merman",
                    "0.8.0-alpha.6",
                    opener=lambda *_args, **_kwargs: self.response(payload),
                )
            )

    def test_different_existing_wheel_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)
            payload = {"releases": {"0.8.0a6": [{"filename": path.name, "digests": {"sha256": "0" * 64}}]}}
            with self.assertRaisesRegex(reconcile.PyPIReconciliationError, "checksum mismatch"):
                reconcile.reconcile(
                    Path(temp_dir),
                    "merman",
                    "0.8.0a6",
                    opener=lambda *_args, **_kwargs: self.response(payload),
                )

    def test_existing_wheel_without_digest_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)
            payload = {
                "releases": {
                    "0.8.0a6": [{"filename": path.name, "digests": {}}]
                }
            }
            with self.assertRaisesRegex(
                reconcile.PyPIReconciliationError,
                "no valid SHA-256 metadata",
            ):
                reconcile.reconcile(
                    Path(temp_dir),
                    "merman",
                    "0.8.0a6",
                    opener=lambda *_args, **_kwargs: self.response(payload),
                )

    def test_extra_registry_wheel_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            payload = {
                "releases": {
                    "0.8.0a6": [
                        {"filename": path.name, "digests": {"sha256": digest}},
                        {
                            "filename": "merman-0.8.0a6-cp312-cp312-win_amd64.whl",
                            "digests": {"sha256": "1" * 64},
                        },
                    ]
                }
            }
            with self.assertRaisesRegex(
                reconcile.PyPIReconciliationError,
                "outside the local release set",
            ):
                reconcile.reconcile(
                    Path(temp_dir),
                    "merman",
                    "0.8.0a6",
                    opener=lambda *_args, **_kwargs: self.response(payload),
                )

    def test_require_exact_returns_retryable_status_when_registry_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "merman-0.8.0a6-py3-none-any.whl"
            wheel(path)
            with mock.patch.object(reconcile, "reconcile", return_value=False):
                status = reconcile.main(
                    [
                        "--directory",
                        temp_dir,
                        "--project",
                        "merman",
                        "--version",
                        "0.8.0a6",
                        "--require-exact",
                    ]
                )
        self.assertEqual(status, 3)


if __name__ == "__main__":
    unittest.main()
