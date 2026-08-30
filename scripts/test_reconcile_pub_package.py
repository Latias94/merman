#!/usr/bin/env python3
"""Tests for pub.dev Flutter package reconciliation."""

from __future__ import annotations

import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

from scripts import flutter_release_archive
from scripts import reconcile_pub_package as reconcile


class Response:
    def __init__(self, payload: dict | None = None, data: bytes | None = None):
        self.payload = payload
        self.data = data

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, size: int = -1):
        if self.payload is not None:
            return json.dumps(self.payload).encode()
        if self.data is None:
            return b""
        if size < 0:
            value, self.data = self.data, b""
            return value
        value, self.data = self.data[:size], self.data[size:]
        return value


class PubPackageReconciliationTests(unittest.TestCase):
    def package_archive(
        self,
        root: Path,
        name: str = "merman-flutter-package.tar.gz",
        source: str = "void main() {}\n",
    ) -> Path:
        package = root / "package"
        package.mkdir()
        (package / "pubspec.yaml").write_text("name: merman\nversion: 0.8.0-alpha.6\n")
        (package / "LICENSE").write_text("license\n")
        (package / "THIRD_PARTY_NOTICES.md").write_text("notices\n")
        (package / "lib").mkdir()
        (package / "lib/example.dart").write_text(source)
        archive = root / name
        receipt = root / "merman-flutter-package.receipt.json"
        flutter_release_archive.create_archive(
            package,
            archive,
            receipt,
            source_sha="a" * 40,
            source_tree="b" * 40,
            version="0.8.0-alpha.6",
        )
        return archive

    def test_missing_pub_version_is_uploadable(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive = self.package_archive(Path(temp_dir))

            def opener(*_args, **_kwargs):
                error = reconcile.urllib.error.HTTPError(
                    "url", 404, "missing", {}, io.BytesIO()
                )
                error.close()
                raise error

            self.assertEqual(
                reconcile.reconcile(archive, "merman", "0.8.0-alpha.6", opener=opener),
                "missing",
            )

    def test_exact_pub_archive_contents_are_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive = self.package_archive(Path(temp_dir))
            pub_archive = Path(temp_dir) / "pub-archive.tar.gz"
            directories: set[str] = set()
            with tarfile.open(archive, mode="r:gz") as source, tarfile.open(
                pub_archive, mode="w:gz"
            ) as target:
                for member in source:
                    parts = member.name.split("/")
                    for index in range(1, len(parts)):
                        directory = "/".join(parts[:index])
                        if directory in directories:
                            continue
                        directories.add(directory)
                        info = tarfile.TarInfo(directory)
                        info.type = tarfile.DIRTYPE
                        info.mode = 0o755
                        info.mtime = 0
                        info.uname = info.gname = "pub"
                        target.addfile(info)
                    info = tarfile.TarInfo(member.name)
                    info.size = member.size
                    info.mode = member.mode
                    info.mtime = 1_786_551_402
                    info.uname = info.gname = "pub"
                    source_file = source.extractfile(member)
                    self.assertIsNotNone(source_file)
                    with source_file:
                        target.addfile(info, source_file)
            data = pub_archive.read_bytes()
            payload = {
                "versions": [
                    {
                        "version": "0.8.0-alpha.6",
                        "archive_url": "https://pub.dev/archive.tar.gz",
                    }
                ]
            }

            def opener(request, **_kwargs):
                if request.full_url.endswith("archive.tar.gz"):
                    return Response(data=data)
                return Response(payload=payload)

            self.assertEqual(
                reconcile.reconcile(archive, "merman", "0.8.0-alpha.6", opener=opener),
                "exact",
            )

    def test_different_pub_archive_contents_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive = self.package_archive(Path(temp_dir))
            with tempfile.TemporaryDirectory() as changed_dir:
                changed = self.package_archive(
                    Path(changed_dir),
                    source="void main() { print('different'); }\n",
                )
                payload = {
                    "versions": [
                        {
                            "version": "0.8.0-alpha.6",
                            "archive_url": "https://pub.dev/archive.tar.gz",
                        }
                    ]
                }

                def opener(request, **_kwargs):
                    if request.full_url.endswith("archive.tar.gz"):
                        return Response(data=changed.read_bytes())
                    return Response(payload=payload)

                with self.assertRaisesRegex(reconcile.PubReconciliationError, "contents differ"):
                    reconcile.reconcile(archive, "merman", "0.8.0-alpha.6", opener=opener)

    def test_registry_download_stops_at_compressed_size_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            destination = Path(temp_dir) / "registry.tar.gz"
            with self.assertRaisesRegex(
                reconcile.PubReconciliationError,
                "compressed size budget",
            ):
                reconcile._download_with_opener(
                    "https://pub.dev/archive.tar.gz",
                    destination,
                    lambda *_args, **_kwargs: Response(data=b"12345"),
                    max_bytes=4,
                )
            self.assertEqual(destination.read_bytes(), b"")


if __name__ == "__main__":
    unittest.main()
