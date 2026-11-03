#!/usr/bin/env python3
"""Exercise frozen Flutter target assets and unauthenticated public readback."""

from contextlib import redirect_stderr, redirect_stdout
import copy
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import flutter_native_assets as native_assets
from flutter_native_assets import (
    NativeAssetsError,
    TARGET_LIBRARIES,
    stage,
    validate_manifest,
    verify_assets,
    verify_public,
)


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

    def open_public_asset(self, url, **_kwargs):
        stream = (self.root / "assets" / url.rsplit("/", 1)[-1]).open("rb")
        stream.url = url
        return stream

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
        manifest = self.stage()
        with mock.patch.object(native_assets, "urlopen", side_effect=self.open_public_asset):
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

    def test_materialize_reuses_only_requested_library_without_changing_frozen_inputs(self):
        manifest = self.stage()
        original_manifest = copy.deepcopy(manifest)
        manifest_path = self.root / "assets/native-assets.json"
        manifest_bytes = manifest_path.read_bytes()
        entry = manifest["targets"]["macos/arm64"]
        asset_path = self.root / "assets" / entry["asset"]
        compressed = asset_path.read_bytes()
        for path in asset_path.parent.glob("*.gz"):
            if path != asset_path:
                path.unlink()
        recovered = self.root / "recovered"
        with mock.patch.object(native_assets, "CHUNK_BYTES", 4):
            native_assets.materialize(manifest, asset_path.parent, recovered, "macos/arm64")
        library = recovered / "macos/arm64/libmerman_ffi.dylib"
        self.assertEqual(library.read_bytes(), (self.native / "macos/arm64/libmerman_ffi.dylib").read_bytes())
        self.assertEqual([path for path in recovered.rglob("*") if path.is_file()], [library])
        self.assertEqual(manifest, original_manifest)
        self.assertEqual(manifest_path.read_bytes(), manifest_bytes)
        self.assertEqual(asset_path.read_bytes(), compressed)

    def test_materialize_rejects_compressed_and_library_hash_or_size_mismatches(self):
        original = self.stage()
        entry = original["targets"]["macos/arm64"]
        cases = (
            ("sha256", "0" * 64),
            ("size", entry["size"] - 1),
            ("size", entry["size"] + 1),
            ("library_sha256", "0" * 64),
            ("library_size", entry["library_size"] - 1),
            ("library_size", entry["library_size"] + 1),
        )
        for index, (field, value) in enumerate(cases):
            with self.subTest(field=field, value=value):
                manifest = copy.deepcopy(original)
                manifest["targets"]["macos/arm64"][field] = value
                recovered = self.root / f"recovered-{index}"
                with mock.patch.object(native_assets, "CHUNK_BYTES", 4):
                    with self.assertRaisesRegex(NativeAssetsError, "digest|size"):
                        native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
                self.assertFalse(any(path.is_file() for path in recovered.rglob("*")))

    def test_materialize_bounds_gzip_expansion_and_cleans_partial_output(self):
        (self.native / "macos/arm64/libmerman_ffi.dylib").write_bytes(b"a" * (2 * native_assets.CHUNK_BYTES))
        manifest = self.stage()
        manifest["targets"]["macos/arm64"]["library_size"] = 5
        recovered = self.root / "recovered"
        with mock.patch.object(native_assets, "CHUNK_BYTES", 4):
            with self.assertRaisesRegex(NativeAssetsError, "size"):
                native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
        self.assertFalse(any(path.is_file() for path in recovered.rglob("*")))

    def test_materialize_rejects_invalid_gzip_and_cleans_partial_output(self):
        manifest = self.stage()
        entry = manifest["targets"]["macos/arm64"]
        asset_path = self.root / "assets" / entry["asset"]
        compressed = asset_path.read_bytes()
        for index, payload in enumerate((b"not gzip", compressed[:-4], compressed[:10] + b"\x07" + compressed[11:])):
            with self.subTest(index=index):
                asset_path.write_bytes(payload)
                entry.update(size=len(payload), sha256=hashlib.sha256(payload).hexdigest())
                recovered = self.root / f"recovered-{index}"
                with mock.patch.object(native_assets, "CHUNK_BYTES", 4):
                    with self.assertRaises(NativeAssetsError):
                        native_assets.materialize(manifest, asset_path.parent, recovered, "macos/arm64")
                self.assertFalse(any(path.is_file() for path in recovered.rglob("*")))

    def test_materialize_rejects_unsupported_missing_or_unsafe_targets(self):
        original = self.stage()
        recovered = self.root / "recovered"
        for target in ("../escape", "macos/mips", "/macos/arm64"):
            with self.subTest(target=target):
                with self.assertRaisesRegex(NativeAssetsError, "target"):
                    native_assets.materialize(original, self.root / "assets", recovered, target)
        for field, value in (("library", "../escape"), ("asset", "../escape.gz")):
            with self.subTest(field=field):
                manifest = copy.deepcopy(original)
                manifest["targets"]["macos/arm64"][field] = value
                with self.assertRaises(NativeAssetsError):
                    native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
        del original["targets"]["macos/arm64"]
        with self.assertRaisesRegex(NativeAssetsError, "target"):
            native_assets.materialize(original, self.root / "assets", recovered, "macos/arm64")
        self.assertFalse(recovered.exists())

    def test_materialize_refuses_existing_library_and_partial_paths(self):
        manifest = self.stage()
        sentinel = self.root / "sentinel"
        sentinel.write_bytes(b"do not replace")
        for suffix in ("", ".part"):
            for kind in ("file", "directory", "symlink", "dangling-symlink"):
                with self.subTest(suffix=suffix, kind=kind):
                    recovered = self.root / f"recovered-{suffix}-{kind}"
                    library = recovered / "macos/arm64/libmerman_ffi.dylib"
                    library.parent.mkdir(parents=True)
                    occupied = library.with_name(library.name + suffix)
                    if kind == "file":
                        occupied.write_bytes(b"do not replace")
                    elif kind == "directory":
                        occupied.mkdir()
                    else:
                        occupied.symlink_to(sentinel if kind == "symlink" else self.root / "missing")
                    with self.assertRaisesRegex(NativeAssetsError, "exists|symlink"):
                        native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
                    self.assertTrue(occupied.exists() or occupied.is_symlink())
                    if kind == "file":
                        self.assertEqual(occupied.read_bytes(), b"do not replace")
                    self.assertEqual(sentinel.read_bytes(), b"do not replace")
                    if suffix:
                        self.assertFalse(library.exists())
                    else:
                        self.assertFalse(library.with_name(library.name + ".part").exists())

    def test_materialize_rejects_symlink_directories_and_source_asset(self):
        manifest = self.stage()
        outside = self.root / "outside"
        outside.mkdir()
        for index, components in enumerate(((), ("macos",), ("macos", "arm64"))):
            with self.subTest(components=components):
                recovered = self.root / f"recovered-{index}"
                link = recovered.joinpath(*components)
                link.parent.mkdir(parents=True, exist_ok=True)
                link.symlink_to(outside, target_is_directory=True)
                with self.assertRaisesRegex(NativeAssetsError, "symlink"):
                    native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
                self.assertTrue(link.is_symlink())
                self.assertEqual(list(outside.iterdir()), [])
        entry = manifest["targets"]["macos/arm64"]
        asset_path = self.root / "assets" / entry["asset"]
        source = self.root / "original.gz"
        asset_path.rename(source)
        asset_path.symlink_to(source)
        with self.assertRaisesRegex(NativeAssetsError, "regular|symlink"):
            native_assets.materialize(manifest, asset_path.parent, self.root / "recovered", "macos/arm64")
        self.assertTrue(asset_path.is_symlink())

    def test_materialize_does_not_overwrite_a_concurrently_created_library(self):
        manifest = self.stage()
        recovered = self.root / "recovered"
        library = recovered / "macos/arm64/libmerman_ffi.dylib"
        original_link = native_assets.os.link

        def competing_link(source, destination):
            library.write_bytes(b"other worker")
            return original_link(source, destination)

        with mock.patch.object(native_assets.os, "link", side_effect=competing_link):
            with self.assertRaises((NativeAssetsError, OSError)):
                native_assets.materialize(manifest, self.root / "assets", recovered, "macos/arm64")
        self.assertEqual(library.read_bytes(), b"other worker")
        self.assertEqual(list(library.parent.iterdir()), [library])

    def test_cli_read_commands_accept_optional_exact_manifest_identity(self):
        manifest = self.stage()
        identities = {"version": "0.9.3", "source_sha": "1" * 40, "source_tree": "2" * 40}
        variants = [(None, None), (identities, None)]
        for field, value in (("version", "0.9.4"), ("source_sha", "3" * 40), ("source_tree", "4" * 40)):
            variants.append(({**identities, field: value}, field))
        for command in ("verify", "verify-public", "list", "materialize"):
            for index, (expected, mismatch) in enumerate(variants):
                with self.subTest(command=command, mismatch=mismatch, expected=expected):
                    arguments = [command, "--manifest", str(self.root / "assets/native-assets.json"),
                                 "--base-url", "https://example.org/flutter-v0.9.3/"]
                    if command in {"verify", "materialize"}:
                        arguments.extend(["--assets-root", str(self.root / "assets")])
                    if command == "materialize":
                        arguments.extend(["--native-root", str(self.root / f"cli-{index}"), "--target", "macos/arm64"])
                    for field, value in (expected or {}).items():
                        arguments.extend([f"--{field.replace('_', '-')}", value])
                    output, error = io.StringIO(), io.StringIO()
                    with mock.patch.object(native_assets, "urlopen", side_effect=self.open_public_asset):
                        with redirect_stdout(output), redirect_stderr(error):
                            result = native_assets.main(arguments)
                    self.assertEqual(result, 1 if mismatch else 0, error.getvalue())
                    if mismatch:
                        self.assertIn(f"{mismatch} identity differs", error.getvalue())
                    elif command == "list":
                        self.assertEqual(set(output.getvalue().splitlines()), {entry["asset"] for entry in manifest["targets"].values()})


if __name__ == "__main__":
    unittest.main()
