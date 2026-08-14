"""Generate or verify the complete Tree-sitter Mermaid artifact set."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable


CLI_VERSION = "0.26.12"
WASI_SDK_VERSION = "29.0"
GENERATED_ARTIFACTS = (
    Path("src/parser.c"),
    Path("src/grammar.json"),
    Path("src/node-types.json"),
    Path("src/tree_sitter/alloc.h"),
    Path("src/tree_sitter/array.h"),
    Path("src/tree_sitter/parser.h"),
    Path("wasm/tree-sitter-mermaid.wasm"),
)
SOURCE_ARTIFACTS = tuple(
    path for path in GENERATED_ARTIFACTS if path.parts[0] == "src"
)
AUTHORED_SOURCE_FILES = (Path("src/scanner.c"),)
SOURCE_PARITY_ARTIFACTS = (*SOURCE_ARTIFACTS, *AUTHORED_SOURCE_FILES)
C_HEADER = Path("bindings/c/tree_sitter/tree-sitter-mermaid.h")
C_HEADER_TEMPLATE = Path("bindings/c/tree_sitter/tree-sitter-mermaid.h.in")
C_BINDING_FILES = (
    C_HEADER,
    C_HEADER_TEMPLATE,
    Path("bindings/c/tree-sitter-mermaid.pc.in"),
)
QUERY_PROFILES = (
    ("portable", "highlights", Path("queries/portable/highlights.scm")),
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cli_command(package: Path) -> list[str]:
    node = os.environ.get("TREE_SITTER_MERMAID_NODE") or shutil.which("node")
    wrapper = package / "node_modules" / "tree-sitter-cli" / "cli.js"
    if node is None:
        raise SystemExit(
            "Node.js is missing; run generation through npm or add node to PATH"
        )
    if not wrapper.is_file():
        raise SystemExit(
            "package-local tree-sitter CLI is missing; run npm ci and rebuild tree-sitter-cli"
        )
    return [node, str(wrapper)]


def validate_cli_version(version: str) -> None:
    expected = f"tree-sitter {CLI_VERSION}"
    if version != expected:
        raise SystemExit(f"expected {expected}, found {version}")


def run(command: list[str], *, cwd: Path) -> int:
    started = time.perf_counter_ns()
    subprocess.run(command, cwd=cwd, check=True)
    return (time.perf_counter_ns() - started) // 1_000_000


def copy_generation_inputs(package: Path, destination: Path) -> None:
    shutil.copy2(package / "grammar.js", destination / "grammar.js")
    shutil.copy2(package / "tree-sitter.json", destination / "tree-sitter.json")
    shutil.copy2(package / "package.json", destination / "package.json")
    shutil.copytree(package / "grammar", destination / "grammar")
    shutil.copytree(package / "queries", destination / "queries")
    (destination / "src").mkdir()
    shutil.copy2(package / "src/scanner.c", destination / "src/scanner.c")


def generate_once(
    package: Path,
    cli: list[str],
    runtime: str,
    destination: Path,
    *,
    build_wasm: bool = True,
) -> dict[str, int]:
    copy_generation_inputs(package, destination)
    generate_milliseconds = run(
        [
            *cli,
            "generate",
            "--abi",
            "14",
            "--js-runtime",
            runtime,
            "--json-summary",
        ],
        cwd=destination,
    )
    wasm_milliseconds = 0
    if build_wasm:
        (destination / "wasm").mkdir()
        wasm_milliseconds = run(
            [
                *cli,
                "build",
                "--wasm",
                "--output",
                "wasm/tree-sitter-mermaid.wasm",
                ".",
            ],
            cwd=destination,
        )
    return {
        "generateMilliseconds": generate_milliseconds,
        "wasmBuildMilliseconds": wasm_milliseconds,
    }


def compare_sets(
    left: Path,
    right: Path,
    description: str,
    artifacts: tuple[Path, ...] = GENERATED_ARTIFACTS,
) -> list[str]:
    failures = []
    for relative in artifacts:
        left_file = left / relative
        right_file = right / relative
        if not left_file.is_file() or not right_file.is_file():
            failures.append(f"{description}: missing {relative.as_posix()}")
            continue
        if left_file.read_bytes() != right_file.read_bytes():
            failures.append(f"{description}: differs {relative.as_posix()}")
    return failures


def assert_exact_generated_set(
    package: Path, *, include_wasm: bool = True
) -> list[str]:
    expected_source = {
        *(path.as_posix() for path in GENERATED_ARTIFACTS if path.parts[0] == "src"),
        *(path.as_posix() for path in AUTHORED_SOURCE_FILES),
    }
    actual_source = {
        path.relative_to(package).as_posix()
        for path in (package / "src").rglob("*")
        if path.is_file()
    }
    expected_wasm = (
        {
            path.as_posix()
            for path in GENERATED_ARTIFACTS
            if path.parts[0] == "wasm"
        }
        if include_wasm
        else set()
    )
    actual_wasm = {
        path.relative_to(package).as_posix()
        for path in (package / "wasm").rglob("*")
        if path.is_file()
    }
    failures = []
    if actual_source != expected_source:
        failures.append(
            f"src artifact set mismatch: expected {sorted(expected_source)}, "
            f"found {sorted(actual_source)}"
        )
    if actual_wasm != expected_wasm:
        failures.append(
            f"wasm artifact set mismatch: expected {sorted(expected_wasm)}, "
            f"found {sorted(actual_wasm)}"
        )
    return failures


def package_artifact_set_failures(
    package: Path, *, require_complete: bool
) -> list[str]:
    expected = {
        *(path.as_posix() for path in GENERATED_ARTIFACTS),
        *(path.as_posix() for path in AUTHORED_SOURCE_FILES),
        *(path.as_posix() for path in C_BINDING_FILES),
    }
    roots = (package / "src", package / "wasm", package / "bindings/c")
    actual = {
        path.relative_to(package).as_posix()
        for root in roots
        if root.exists()
        for path in root.rglob("*")
        if path.is_file()
    }
    unexpected = actual - expected
    missing = expected - actual if require_complete else set()
    failures = []
    if unexpected:
        failures.append(f"unexpected package artifacts: {sorted(unexpected)}")
    if missing:
        failures.append(f"missing package artifacts: {sorted(missing)}")
    return failures


def receipt_inputs(package: Path) -> list[Path]:
    fixed = [
        package / "Cargo.toml",
        package / "package.json",
        package / "package-lock.json",
        package / "tree-sitter.json",
        package / "grammar.js",
        package / "src/scanner.c",
        package / "metadata/provenance.json",
        package / "metadata/derivations.json",
        package / "metadata/headers.json",
        package / "metadata/evidence/u2-mermaid-header-oracle.json",
        package / "metadata/fixtures/family-roots.json",
        package / "metadata/schema-version.json",
        package / "bindings/rust/build.rs",
        package / "bindings/rust/lib.rs",
        package / "bindings/node/binding.cc",
        package / "bindings/node/index.js",
        package / "bindings/node/index.d.ts",
        package / "bindings/wasm/index.js",
        package / C_HEADER_TEMPLATE,
        package / "bindings/c/tree-sitter-mermaid.pc.in",
        package / "binding.gyp",
        package / "scripts/generate.py",
        package / "scripts/header_receipt.js",
        package / "scripts/header_oracle.js",
        package / "scripts/mechanics_gate.js",
        package / "scripts/query_golden.js",
        package / "scripts/run_python.js",
        package / "scripts/header-oracle/package.json",
        package / "scripts/header-oracle/package-lock.json",
    ]
    discovered = list((package / "grammar").rglob("*.js"))
    if (package / "queries").exists():
        discovered.extend((package / "queries").rglob("*.scm"))
    paths = sorted({*fixed, *discovered})
    missing = [path for path in paths if not path.is_file()]
    if missing:
        joined = ", ".join(path.relative_to(package).as_posix() for path in missing)
        raise SystemExit(f"receipt input is missing: {joined}")
    return paths


def snapshot_receipt_inputs(package: Path) -> dict[str, tuple[str, int]]:
    return {
        path.relative_to(package).as_posix(): (sha256(path), path.stat().st_size)
        for path in receipt_inputs(package)
    }


def receipt_input_drift(
    package: Path, expected: dict[str, tuple[str, int]]
) -> list[str]:
    actual = snapshot_receipt_inputs(package)
    if actual != expected:
        return ["generation inputs changed while artifacts were being generated"]
    return []


def build_receipt(package: Path, artifact_root: Path | None = None) -> dict[str, object]:
    artifacts = artifact_root or package
    provenance = json.loads((package / "metadata/provenance.json").read_text())
    sources = {source["id"]: source for source in provenance["sources"]}
    body: dict[str, object] = {
        "schemaVersion": 1,
        "package": {
            "name": "tree-sitter-mermaid",
            "version": "0.1.0",
            "releaseState": "dry-run-only",
        },
        "language": {
            "symbol": "mermaid",
            "abi": 14,
            "cstSchemaVersion": 1,
            "querySchemaVersion": 1,
        },
        "toolchain": {
            "treeSitterCli": CLI_VERSION,
            "rustRuntime": "0.26.12",
            "nodeRuntime": "0.25.1",
            "webRuntime": "0.26.12",
            "wasiSdk": WASI_SDK_VERSION,
        },
        "baselines": {
            identifier: {
                "version": sources[identifier]["version"],
                "commit": sources[identifier]["commit"],
            }
            for identifier in ("merman-oracle", "mermaid", "zenuml-core")
        },
        "generation": {
            "grammarCommands": [
                "tree-sitter generate --abi 14 --js-runtime native --json-summary",
                "tree-sitter generate --abi 14 --js-runtime node --json-summary",
            ],
            "wasmCommand": (
                "tree-sitter build --wasm "
                "--output wasm/tree-sitter-mermaid.wasm ."
            ),
        },
        "queryProfiles": [
            {
                "profile": profile,
                "surface": surface,
                "path": relative.as_posix(),
                "sha256": sha256(package / relative),
                "bytes": (package / relative).stat().st_size,
            }
            for profile, surface, relative in QUERY_PROFILES
        ],
        "inputs": [
            {
                "path": path.relative_to(package).as_posix(),
                "sha256": sha256(path),
                "bytes": path.stat().st_size,
            }
            for path in receipt_inputs(package)
        ],
        "artifacts": [
            {
                "path": relative.as_posix(),
                "sha256": sha256(artifacts / relative),
                "bytes": (artifacts / relative).stat().st_size,
            }
            for relative in GENERATED_ARTIFACTS
        ],
    }
    canonical = json.dumps(body, sort_keys=True, separators=(",", ":")).encode()
    return {"receiptId": hashlib.sha256(canonical).hexdigest(), **body}


def receipt_bytes(receipt: dict[str, object]) -> bytes:
    return (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()


def render_c_header(package: Path, receipt_id: str) -> bytes:
    template = (package / C_HEADER_TEMPLATE).read_text(encoding="utf-8")
    marker = "@ARTIFACT_RECEIPT_ID@"
    if template.count(marker) != 1:
        raise RuntimeError(f"{C_HEADER_TEMPLATE.as_posix()} must contain one {marker}")
    return template.replace(marker, receipt_id).encode()


def install_artifacts_transactionally(
    generated: Path,
    package: Path,
    rendered_receipt: bytes,
    carriers: dict[Path, bytes],
    post_install_check: Callable[[Path], list[str]] | None = None,
) -> None:
    failures = assert_exact_generated_set(generated)
    failures.extend(package_artifact_set_failures(package, require_complete=False))
    if failures:
        raise RuntimeError("\n".join(failures))

    install_paths = (
        *GENERATED_ARTIFACTS,
        Path("metadata/artifact-receipt.json"),
        *carriers,
    )
    with tempfile.TemporaryDirectory(
        prefix=".tree-sitter-mermaid-install-", dir=package.parent
    ) as directory:
        transaction = Path(directory)
        staged = transaction / "staged"
        backups = transaction / "backups"
        for relative in GENERATED_ARTIFACTS:
            destination = staged / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(generated / relative, destination)
        receipt_path = staged / "metadata/artifact-receipt.json"
        receipt_path.parent.mkdir(parents=True, exist_ok=True)
        receipt_path.write_bytes(rendered_receipt)
        for relative, contents in carriers.items():
            carrier = staged / relative
            carrier.parent.mkdir(parents=True, exist_ok=True)
            carrier.write_bytes(contents)

        installed: list[tuple[Path, Path, bool]] = []
        try:
            for relative in install_paths:
                source = staged / relative
                destination = package / relative
                backup = backups / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                existed = destination.is_file()
                if existed:
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(destination, backup)
                installed.append((destination, backup, existed))
                os.replace(source, destination)
            failures = package_artifact_set_failures(package, require_complete=True)
            if post_install_check is not None:
                failures.extend(post_install_check(package))
            if failures:
                raise RuntimeError("\n".join(failures))
        except Exception:
            for destination, backup, existed in reversed(installed):
                if destination.is_file():
                    destination.unlink()
                if existed and backup.is_file():
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(backup, destination)
            raise


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace committed generated artifacts and receipt",
    )
    parser.add_argument(
        "--timings-json",
        type=Path,
        help="write generation stage timings to this path",
    )
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    package = Path(__file__).resolve().parents[1]
    cli = cli_command(package)
    version = subprocess.run(
        [*cli, "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    validate_cli_version(version)
    input_snapshot = snapshot_receipt_inputs(package)
    timings: list[dict[str, int | str]] = []
    total_started = time.perf_counter_ns()

    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-generate-") as first:
        with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-generate-") as second:
            native = Path(first)
            node = Path(second)
            timings.append({
                "stage": "native",
                **generate_once(package, cli, "native", native),
            })
            timings.append({
                "stage": "node-source",
                **generate_once(package, cli, "node", node, build_wasm=False),
            })
            # The runtime only affects grammar evaluation. One canonical WASM
            # build is sufficient after every generated and authored C input
            # is byte-identical across the runtime-specific trees.
            failures = compare_sets(
                native,
                node,
                "native and node source generation",
                SOURCE_PARITY_ARTIFACTS,
            )
            failures.extend(assert_exact_generated_set(native))
            failures.extend(assert_exact_generated_set(node, include_wasm=False))
            failures.extend(receipt_input_drift(package, input_snapshot))
            if not arguments.write:
                failures.extend(compare_sets(native, package, "committed generation"))
                failures.extend(
                    package_artifact_set_failures(package, require_complete=True)
                )
            if failures:
                raise SystemExit("\n".join(failures))

            receipt = build_receipt(package, native)
            expected_receipt = receipt_bytes(receipt)
            carriers = {
                C_HEADER: render_c_header(package, str(receipt["receiptId"])),
            }
            receipt_path = package / "metadata/artifact-receipt.json"
            if arguments.write:
                def check_installed_receipt(installed_package: Path) -> list[str]:
                    installed_failures = receipt_input_drift(
                        installed_package, input_snapshot
                    )
                    if receipt_bytes(build_receipt(installed_package)) != expected_receipt:
                        installed_failures.append(
                            "installed artifact receipt is inconsistent"
                        )
                    with tempfile.TemporaryDirectory(
                        prefix="tree-sitter-mermaid-post-install-"
                    ) as rebuilt_directory:
                        rebuilt = Path(rebuilt_directory)
                        timings.append({
                            "stage": "post-install-native",
                            **generate_once(installed_package, cli, "native", rebuilt),
                        })
                        installed_failures.extend(
                            compare_sets(
                                rebuilt,
                                installed_package,
                                "post-install clean generation",
                            )
                        )
                        installed_failures.extend(assert_exact_generated_set(rebuilt))
                    installed_failures.extend(
                        receipt_input_drift(installed_package, input_snapshot)
                    )
                    return installed_failures

                install_artifacts_transactionally(
                    native,
                    package,
                    expected_receipt,
                    carriers,
                    check_installed_receipt,
                )
            elif not receipt_path.is_file():
                raise SystemExit("committed receipt is missing")
            elif receipt_path.read_bytes() != expected_receipt:
                raise SystemExit("committed receipt is stale")
            else:
                stale_carriers = [
                    relative.as_posix()
                    for relative, contents in carriers.items()
                    if not (package / relative).is_file()
                    or (package / relative).read_bytes() != contents
                ]
                if stale_carriers:
                    raise SystemExit(
                        f"committed receipt carriers are stale: {stale_carriers}"
                    )

    if arguments.timings_json is not None:
        arguments.timings_json.parent.mkdir(parents=True, exist_ok=True)
        timings_payload = {
            "totalMilliseconds": (time.perf_counter_ns() - total_started) // 1_000_000,
            "stages": timings,
        }
        arguments.timings_json.write_text(
            json.dumps(timings_payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    print(f"verified receipt {receipt['receiptId']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
