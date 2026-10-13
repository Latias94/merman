#!/usr/bin/env python3
"""Reject the complete published API 7 Swift projection against a current macOS library.

Pass the native library built by the artifact owner. This check neither builds Rust nor changes
checked-in generated bindings. The frozen alpha.7 projection has API 7 but incompatible nested
records; its removed presentation-catalog symbol must fail at dynamic loading, before decoding.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[3]
PUBLISHED_API7_SOURCE = "580e39b69cc1b0ca35c4f8272683e622b2e9b8db"  # v0.8.0-alpha.7
GENERATED = "platforms/apple/Sources/Merman/Generated"
REMOVED_SYMBOL = "uniffi_merman_uniffi_checksum_method_merman_presentation_catalog_json"


def test_published_api7_loader(library: Path) -> dict:
    if sys.platform != "darwin":
        raise RuntimeError("the complete Swift dynamic-loader check requires macOS")
    library = library.resolve(strict=True)
    source_digests = {}
    with tempfile.TemporaryDirectory(prefix="merman-published-api7-") as temporary:
        directory = Path(temporary)
        for source_name, output_name in (
            ("Merman.swift", "Merman.swift"),
            ("MermanFFI.h", "MermanFFI.h"),
            ("MermanFFI.modulemap", "module.modulemap"),
        ):
            source = subprocess.check_output(
                ["git", "show", f"{PUBLISHED_API7_SOURCE}:{GENERATED}/{source_name}"],
                cwd=ROOT,
            )
            source_digests[source_name] = hashlib.sha256(source).hexdigest()
            (directory / output_name).write_bytes(source)
        projection = directory / "libMermanPublishedApi7.dylib"
        subprocess.run(
            [
                "swiftc", "-emit-library", "-module-name", "MermanPublishedApi7",
                "-I", str(directory), str(directory / "Merman.swift"),
                "-Xlinker", "-undefined", "-Xlinker", "dynamic_lookup",
                "-o", str(projection),
            ],
            cwd=ROOT, check=True, capture_output=True, text=True, timeout=120,
        )
        # Load the complete compiled wrapper, not a hand-written checksum probe. Run in a child
        # process so a failing loader cannot leave partial Swift/Rust state in the test runner.
        loaded = subprocess.run(
            [sys.executable, "-c", """
import ctypes
import os
import sys

ctypes.CDLL(sys.argv[1], mode=os.RTLD_NOW | os.RTLD_GLOBAL)
try:
    ctypes.CDLL(sys.argv[2], mode=os.RTLD_NOW)
except OSError as error:
    print(error, file=sys.stderr)
    sys.exit(1)
sys.exit(0)
""", str(library), str(projection)],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        )
        if loaded.returncode != 1 or REMOVED_SYMBOL not in loaded.stderr:
            raise AssertionError(
                "the complete published API 7 projection must be rejected by the removed "
                f"catalog symbol; exit={loaded.returncode}, stderr={loaded.stderr}"
            )
        with library.open("rb") as stream:
            native_digest = hashlib.file_digest(stream, "sha256").hexdigest()
        return {
            "published_source_commit": PUBLISHED_API7_SOURCE,
            "published_projection_sha256": source_digests,
            "native_library": str(library),
            "native_library_sha256": native_digest,
            "loader_rejection": loaded.stderr.strip(),
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(test_published_api7_loader(args.library), indent=2))


if __name__ == "__main__":
    main()
