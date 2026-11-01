#!/usr/bin/env python3
"""Exercise frozen Flutter target assets and unauthenticated public readback."""

from functools import partial
import gzip
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import tempfile
from threading import Thread
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from flutter_native_assets import (
    NativeAssetsError,
    TARGET_LIBRARIES,
    stage,
    validate_manifest,
    verify_assets,
    verify_public,
)


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


class FlutterNativeAssetsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.native = self.root / "native"
        for target, library in TARGET_LIBRARIES.items():
            path = self.native / target / library
            path.parent.mkdir(parents=True)
            path.write_bytes(f"frozen {target}".encode())

    def stage(self, directory="assets", base_url="https://example.org/flutter-v0.9.3/"):
        return stage(
            self.native, self.root / directory, version="0.9.3",
            source_sha="1" * 40, source_tree="2" * 40, base_url=base_url,
        )

    def test_staging_binds_every_target_to_exact_frozen_bytes(self):
        manifest = self.stage()
        self.assertEqual(set(manifest["targets"]), set(TARGET_LIBRARIES))
        for target, entry in manifest["targets"].items():
            with self.subTest(target=target):
                compressed = (self.root / "assets" / entry["asset"]).read_bytes()
                library = (self.native / target / TARGET_LIBRARIES[target]).read_bytes()
                self.assertEqual(gzip.decompress(compressed), library)
                self.assertEqual(entry["sha256"], hashlib.sha256(compressed).hexdigest())
                self.assertEqual(entry["library_sha256"], hashlib.sha256(library).hexdigest())
                self.assertIn("0.9.3", entry["url"])
        self.assertEqual(manifest, self.stage("again"))
        self.assertEqual(json.loads((self.root / "assets/native-assets.json").read_text()), manifest)
        verify_assets(manifest, self.root / "assets")

    def test_missing_slice_and_symlink_are_rejected(self):
        path = self.native / "macos/arm64/libmerman_ffi.dylib"
        path.unlink()
        with self.assertRaisesRegex(NativeAssetsError, "missing|regular"):
            self.stage()
        path.symlink_to(self.native / "macos/x86_64/libmerman_ffi.dylib")
        with self.assertRaisesRegex(NativeAssetsError, "link|regular"):
            self.stage()

    def test_manifest_rejects_wrong_identity_and_target_path(self):
        manifest = self.stage()
        with self.assertRaisesRegex(NativeAssetsError, "identity"):
            validate_manifest(manifest, version="0.9.4")
        with self.assertRaisesRegex(NativeAssetsError, "URL identity"):
            validate_manifest(manifest, base_url="https://other.example/flutter-v0.9.3/")
        manifest["targets"]["macos/arm64"]["asset"] = "../escape.gz"
        with self.assertRaisesRegex(NativeAssetsError, "asset"):
            validate_manifest(manifest)

    def test_public_download_is_read_back_without_credentials_and_mismatch_fails(self):
        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), partial(QuietHandler, directory=str(self.root / "assets"))
        )
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        manifest = self.stage(base_url=f"http://127.0.0.1:{server.server_port}/")
        verify_public(manifest)
        entry = manifest["targets"]["macos/arm64"]
        (self.root / "assets" / entry["asset"]).write_bytes(b"wrong frozen bytes")
        with self.assertRaisesRegex(NativeAssetsError, "digest|size"):
            verify_public(manifest)

    def test_local_asset_verification_rejects_changed_gzip(self):
        manifest = self.stage()
        entry = manifest["targets"]["ios/arm64-simulator"]
        (self.root / "assets" / entry["asset"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(NativeAssetsError, "digest|size"):
            verify_assets(manifest, self.root / "assets")


if __name__ == "__main__":
    unittest.main()
