#!/usr/bin/env python3
"""Freeze per-target Flutter libraries and verify their public release downloads."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit
from urllib.request import urlopen

if __package__:
    from .release_archive import sha256_file
else:
    from release_archive import sha256_file


TARGET_LIBRARIES = {
    "android/armeabi-v7a": "libmerman_ffi.so",
    "android/arm64-v8a": "libmerman_ffi.so",
    "android/x86_64": "libmerman_ffi.so",
    "ios/arm64": "libmerman_ffi.dylib",
    "ios/arm64-simulator": "libmerman_ffi.dylib",
    "ios/x86_64-simulator": "libmerman_ffi.dylib",
    "macos/arm64": "libmerman_ffi.dylib",
    "macos/x86_64": "libmerman_ffi.dylib",
    "linux/aarch64": "libmerman_ffi.so",
    "linux/x86_64": "libmerman_ffi.so",
    "windows/x86_64": "merman_ffi.dll",
}
MANIFEST_NAME = "native-assets.json"
MANIFEST_PATH = f"hook/{MANIFEST_NAME}"
CHUNK_BYTES = 1024 * 1024


class NativeAssetsError(RuntimeError):
    pass


def _asset_name(version: str, target: str) -> str:
    return f"merman-flutter-native-{version}-{target.replace('/', '-')}.gz"


def _download_url(url: str, *, redirect: bool = False) -> bool:
    parsed = urlsplit(url)
    return (
        bool(parsed.hostname) and not parsed.username and not parsed.password
        and (redirect or not parsed.query) and not parsed.fragment
        and (parsed.scheme == "https" or (
            parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost", "::1"}
        ))
    )


def validate_manifest(
    manifest: object, *, version: str | None = None,
    source_sha: str | None = None, source_tree: str | None = None,
    base_url: str | None = None,
) -> dict:
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise NativeAssetsError("unsupported Flutter native manifest schema")
    if manifest.get("package") != "merman":
        raise NativeAssetsError("Flutter native manifest package identity differs")
    for field, expected in (("version", version), ("source_sha", source_sha), ("source_tree", source_tree)):
        value = manifest.get(field)
        pattern = r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.+-]+)?" if field == "version" else r"[0-9a-f]{40}"
        if not isinstance(value, str) or not re.fullmatch(pattern, value) or (expected is not None and value != expected):
            raise NativeAssetsError(f"Flutter native manifest {field} identity differs")
    targets = manifest.get("targets")
    if not isinstance(targets, dict) or set(targets) != set(TARGET_LIBRARIES):
        raise NativeAssetsError("Flutter native manifest must contain exactly the supported targets")
    for target, entry in targets.items():
        if not isinstance(entry, dict) or entry.get("asset") != _asset_name(manifest["version"], target):
            raise NativeAssetsError(f"invalid Flutter native asset name: {target}")
        if entry.get("library") != TARGET_LIBRARIES[target]:
            raise NativeAssetsError(f"invalid Flutter native library name: {target}")
        for field in ("sha256", "library_sha256"):
            if not isinstance(entry.get(field), str) or not re.fullmatch(r"[0-9a-f]{64}", entry[field]):
                raise NativeAssetsError(f"invalid Flutter native {field}: {target}")
        for field in ("size", "library_size"):
            if type(entry.get(field)) is not int or entry[field] <= 0:
                raise NativeAssetsError(f"invalid Flutter native {field}: {target}")
        url = entry.get("url")
        if not isinstance(url, str) or not _download_url(url) or urlsplit(url).path.rsplit("/", 1)[-1] != entry["asset"]:
            raise NativeAssetsError(f"invalid Flutter native asset URL: {target}")
        if base_url is not None and url != f"{base_url.rstrip('/')}/{entry['asset']}":
            raise NativeAssetsError(f"Flutter native asset URL identity differs: {target}")
    return manifest


def load_manifest(path: Path, **identity) -> dict:
    try:
        return validate_manifest(json.loads(path.read_bytes()), **identity)
    except (OSError, ValueError) as error:
        raise NativeAssetsError(f"cannot read Flutter native manifest: {error}") from error


def stage(
    native_root: Path, output: Path, *, version: str, source_sha: str,
    source_tree: str, base_url: str,
) -> dict:
    for target, library in TARGET_LIBRARIES.items():
        path = native_root / target / library
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise NativeAssetsError(f"missing regular Flutter native library: {path}")
    if output.exists():
        raise NativeAssetsError(f"native asset staging output already exists: {output}")
    output.mkdir(parents=True)
    manifest = {
        "schema_version": 1, "package": "merman", "version": version,
        "source_sha": source_sha, "source_tree": source_tree, "targets": {},
    }
    for target, library in TARGET_LIBRARIES.items():
        asset = _asset_name(version, target)
        path = output / asset
        library_digest = hashlib.sha256()
        library_size = 0
        with (native_root / target / library).open("rb") as source, path.open("xb") as raw:
            with gzip.GzipFile(filename="", fileobj=raw, mode="wb", compresslevel=6, mtime=0) as compressed:
                for chunk in iter(lambda: source.read(CHUNK_BYTES), b""):
                    library_digest.update(chunk)
                    library_size += len(chunk)
                    compressed.write(chunk)
        digest = sha256_file(path)
        manifest["targets"][target] = {
            "asset": asset, "library": library,
            "url": f"{base_url.rstrip('/')}/{asset}",
            "sha256": digest, "size": path.stat().st_size,
            "library_sha256": library_digest.hexdigest(), "library_size": library_size,
        }
    validate_manifest(manifest)
    (output / MANIFEST_NAME).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def _verify_stream(stream, entry: dict) -> None:
    digest, size = hashlib.sha256(), 0
    while chunk := stream.read(min(CHUNK_BYTES, entry["size"] - size + 1)):
        size += len(chunk)
        if size > entry["size"]:
            raise NativeAssetsError(f"native asset size differs: {entry['asset']}")
        digest.update(chunk)
    if size != entry["size"] or digest.hexdigest() != entry["sha256"]:
        raise NativeAssetsError(f"native asset digest or size differs: {entry['asset']}")


def verify_assets(manifest: dict, assets_root: Path) -> None:
    validate_manifest(manifest)
    for entry in manifest["targets"].values():
        path = assets_root / entry["asset"]
        if path.is_symlink() or not path.is_file():
            raise NativeAssetsError(f"missing regular native asset: {path}")
        with path.open("rb") as stream:
            _verify_stream(stream, entry)


def verify_public(manifest: dict) -> None:
    validate_manifest(manifest)
    for entry in manifest["targets"].values():
        try:
            with urlopen(entry["url"], timeout=60) as response:
                if not _download_url(response.url, redirect=True):
                    raise NativeAssetsError("native asset redirected to an unsupported URL")
                _verify_stream(response, entry)
        except OSError as error:
            raise NativeAssetsError(f"cannot read public native asset {entry['asset']}: {error}") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    freeze = commands.add_parser("stage")
    freeze.add_argument("--native-root", type=Path, required=True)
    freeze.add_argument("--output", type=Path, required=True)
    freeze.add_argument("--base-url", required=True)
    for flag in ("version", "source-sha", "source-tree"):
        freeze.add_argument(f"--{flag}", required=True)
    for command in ("verify", "verify-public", "list"):
        child = commands.add_parser(command)
        child.add_argument("--manifest", type=Path, required=True)
        child.add_argument("--base-url", help="Require the exact release URL prefix.")
        if command == "verify":
            child.add_argument("--assets-root", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "stage":
            stage(args.native_root, args.output, version=args.version, source_sha=args.source_sha,
                  source_tree=args.source_tree, base_url=args.base_url)
        else:
            manifest = load_manifest(args.manifest, base_url=args.base_url)
            if args.command == "verify":
                verify_assets(manifest, args.assets_root)
            elif args.command == "verify-public":
                verify_public(manifest)
            else:
                print("\n".join(entry["asset"] for entry in manifest["targets"].values()))
    except (NativeAssetsError, OSError) as error:
        print(f"Flutter native assets: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
