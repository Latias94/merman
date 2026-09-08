"""Verify rejection of the immutable alpha.6 API 6 generated consumer.

Run after building merman-uniffi, passing --library PATH to its cdylib. The
historical commit must be available locally; no network or Cargo build is used.
On macOS, also compile the actual released Swift projection and verify that its
removed version probe prevents linking, before any changed error is decoded.
"""

import argparse
import ctypes
from pathlib import Path
import re
import subprocess
import sys
import tempfile


RELEASE_COMMIT = "d529f858ea3d337a1bdc8fe12e44e1403ededf2e"
PROBE = "uniffi_merman_uniffi_checksum_method_merman_binding_api_version_v6"
GENERATED = "platforms/apple/Sources/Merman/Generated"


def released_source(root: Path, name: str) -> str:
    return subprocess.check_output(
        ["git", "show", f"{RELEASE_COMMIT}:{GENERATED}/{name}"],
        cwd=root,
        text=True,
    )


def verify(library: Path) -> None:
    root = Path(__file__).resolve().parents[3]
    source = released_source(root, "Merman.swift")
    checksums = re.findall(
        r"if \((uniffi_merman_uniffi_checksum_\w+)\(\) != (\d+)\)", source
    )
    assert len(checksums) == 64, "unexpected published API 6 checksum fixture"
    native = ctypes.CDLL(str(library))
    missing = []
    mismatched = []
    for symbol, expected in checksums:
        try:
            checksum = getattr(native, symbol)
        except AttributeError:
            missing.append(symbol)
            continue
        checksum.restype = ctypes.c_uint16
        actual = checksum()
        if actual != int(expected):
            mismatched.append((symbol, int(expected), actual))
    print(
        f"Published API 6: {len(checksums)} checksums; "
        f"missing={missing}; mismatched={mismatched}",
        flush=True,
    )

    if sys.platform == "darwin":
        with tempfile.TemporaryDirectory(prefix="merman-api6-consumer-") as temporary:
            work = Path(temporary)
            for name in ["Merman.swift", "MermanFFI.h"]:
                (work / name).write_text(released_source(root, name))
            (work / "module.modulemap").write_text(
                released_source(root, "MermanFFI.modulemap")
            )
            (work / "main.swift").write_text(
                'let client = Merman()\n'
                'print("legacy-initialized")\n'
                'do {\n'
                '    _ = try client.metadataJson(id: "not-a-metadata-id")\n'
                '    fatalError("expected a structured binding error")\n'
                '} catch {\n'
                '    print("legacy-error: \\(String(reflecting: error))")\n'
                '}\n'
            )
            compiled = subprocess.run(
                [
                    "swiftc", "-module-name", "LegacyMermanConsumer", "-I", str(work),
                    str(work / "Merman.swift"), str(work / "main.swift"),
                    str(library), "-o", str(work / "consumer"),
                ],
                text=True,
                capture_output=True,
            )
            if compiled.returncode == 0:
                result = subprocess.run(
                    [str(work / "consumer")], text=True, capture_output=True
                )
                print(result.stdout, end="", flush=True)
                print(result.stderr, end="", flush=True)
                raise AssertionError(
                    "released API 6 projection linked against the changed error layout"
                )
            assert "Undefined symbols" in compiled.stderr, compiled.stderr
            assert "binding_api_version_v6" in compiled.stderr, compiled.stderr
            print("Released Swift projection rejected at the removed API 6 probe.")

    assert missing == [PROBE], "API 6 must fail before lifting any wire value"
    assert not mismatched, "unrelated published method signatures unexpectedly changed"
    assert not hasattr(native, "uniffi_merman_uniffi_fn_method_merman_binding_api_version_v6")
    assert hasattr(native, "uniffi_merman_uniffi_fn_method_merman_binding_api_version_v7")
    assert hasattr(native, "uniffi_merman_uniffi_checksum_method_merman_binding_api_version_v7")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    arguments = parser.parse_args()
    verify(arguments.library.resolve(strict=True))
