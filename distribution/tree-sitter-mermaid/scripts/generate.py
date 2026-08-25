"""Generate or verify Tree-sitter Mermaid parser and WASM artifacts."""

from __future__ import annotations

import argparse
import filecmp
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
PROVENANCE = json.loads(
    (PACKAGE_ROOT / "metadata/provenance.json").read_text(encoding="utf-8")
)
CLI_VERSION = PROVENANCE["toolchain"]["treeSitterCli"]
LANGUAGE_ABI = int(PROVENANCE["language"]["abi"])
WASI_CLANG_VERSION = PROVENANCE["toolchain"]["wasiClang"]
WASM_IRREDUCIBLE_CFG_FLAG = "-wasm-disable-fix-irreducible-control-flow-pass"
PARSER_MAX_BYTES = 12 * 1024 * 1024
WASM_MAX_BYTES = 5 * 1024 * 1024
SUBPROCESS_TIMEOUT_SECONDS = 15 * 60

GENERATED_SOURCE_FILES = (
    Path("src/parser.c"),
    Path("src/grammar.json"),
    Path("src/node-types.json"),
    Path("src/tree_sitter/alloc.h"),
    Path("src/tree_sitter/array.h"),
    Path("src/tree_sitter/parser.h"),
)
AUTHORED_SOURCE_FILES = (Path("src/scanner.c"),)
WASM_FILE = Path("tree-sitter-mermaid.wasm")


def cli_command(package: Path) -> list[str]:
    node = os.environ.get("TREE_SITTER_MERMAID_NODE") or shutil.which("node")
    wrapper = package / "node_modules" / "tree-sitter-cli" / "cli.js"
    if node is None:
        raise SystemExit("Node.js is missing; install Node.js before generating the parser")
    if not wrapper.is_file():
        raise SystemExit("package-local tree-sitter CLI is missing; run npm ci")
    return [node, str(wrapper)]


def validate_cli_version(version: str) -> None:
    expected = f"tree-sitter {CLI_VERSION}"
    if version != expected:
        raise SystemExit(f"expected {expected}, found {version}")


def run(command: list[str], *, cwd: Path) -> None:
    try:
        subprocess.run(
            command,
            cwd=cwd,
            check=True,
            timeout=SUBPROCESS_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        raise SystemExit(
            f"command timed out after {SUBPROCESS_TIMEOUT_SECONDS} seconds: {command}"
        ) from error


def copy_generation_inputs(package: Path, destination: Path) -> None:
    for filename in ("grammar.js", "package.json", "tree-sitter.json"):
        shutil.copy2(package / filename, destination / filename)
    shutil.copytree(package / "grammar", destination / "grammar")
    (destination / "src").mkdir()
    shutil.copy2(package / "src/scanner.c", destination / "src/scanner.c")


def exact_source_set_failures(root: Path) -> list[str]:
    expected = {
        *(path.as_posix() for path in GENERATED_SOURCE_FILES),
        *(path.as_posix() for path in AUTHORED_SOURCE_FILES),
    }
    actual = {
        path.relative_to(root).as_posix()
        for path in (root / "src").rglob("*")
        if path.is_file()
    }
    if actual == expected:
        return []
    return [f"src artifact set mismatch: expected {sorted(expected)}, found {sorted(actual)}"]


def compare_files(left: Path, right: Path, paths: tuple[Path, ...]) -> list[str]:
    failures = []
    for relative in paths:
        left_file = left / relative
        right_file = right / relative
        if not left_file.is_file() or not right_file.is_file():
            failures.append(f"missing generated artifact: {relative.as_posix()}")
        elif not filecmp.cmp(left_file, right_file, shallow=False):
            failures.append(f"generated artifact is stale: {relative.as_posix()}")
    return failures


def install_files(source: Path, package: Path, paths: tuple[Path, ...]) -> None:
    with tempfile.TemporaryDirectory(
        prefix=".tree-sitter-mermaid-install-", dir=package.parent
    ) as directory:
        staged = Path(directory)
        for relative in paths:
            destination = staged / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / relative, destination)
        for relative in paths:
            destination = package / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            os.replace(staged / relative, destination)


def assert_language_abi(parser: Path) -> None:
    marker = f"#define LANGUAGE_VERSION {LANGUAGE_ABI}"
    with parser.open(encoding="utf-8") as source:
        for line in source:
            if line.startswith("#define LANGUAGE_VERSION "):
                if line.rstrip() == marker:
                    return
                break
    raise SystemExit(f"{parser} does not declare Tree-sitter ABI {LANGUAGE_ABI}")


def assert_size(path: Path, maximum: int) -> None:
    size = path.stat().st_size
    if size > maximum:
        raise SystemExit(f"{path} is {size} bytes; gross ceiling is {maximum} bytes")


def generate_sources(package: Path, cli: list[str], destination: Path) -> None:
    copy_generation_inputs(package, destination)
    run(
        [
            *cli,
            "generate",
            "--abi",
            str(LANGUAGE_ABI),
            "--js-runtime",
            "node",
            "--json-summary",
        ],
        cwd=destination,
    )
    failures = exact_source_set_failures(destination)
    if failures:
        raise SystemExit("\n".join(failures))
    assert_language_abi(destination / "src/parser.c")
    assert_size(destination / "src/parser.c", PARSER_MAX_BYTES)


def wasi_sdk_candidates() -> list[Path]:
    configured = os.environ.get("TREE_SITTER_WASI_SDK_PATH")
    if configured:
        return [Path(configured)]

    candidates = []
    if cache_home := os.environ.get("XDG_CACHE_HOME"):
        candidates.append(Path(cache_home) / "tree-sitter" / "wasi-sdk")
    if local_app_data := os.environ.get("LOCALAPPDATA"):
        candidates.append(Path(local_app_data) / "tree-sitter" / "wasi-sdk")
    candidates.append(Path.home() / ".cache" / "tree-sitter" / "wasi-sdk")
    if sys.platform == "darwin":
        candidates.append(Path.home() / "Library" / "Caches" / "tree-sitter" / "wasi-sdk")
    return list(dict.fromkeys(candidates))


def find_wasi_clang() -> Path | None:
    executable_names = (
        ("clang.exe", "wasm32-unknown-wasi-clang.exe", "wasm32-wasi-clang.exe")
        if os.name == "nt"
        else ("clang", "wasm32-unknown-wasi-clang", "wasm32-wasi-clang")
    )
    for sdk in wasi_sdk_candidates():
        for name in executable_names:
            candidate = sdk / "bin" / name
            if candidate.is_file():
                return candidate
    return None


def provision_wasi_sdk(cli: list[str]) -> None:
    if os.environ.get("TREE_SITTER_WASI_SDK_PATH"):
        raise SystemExit("TREE_SITTER_WASI_SDK_PATH has no supported clang executable")
    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-wasi-probe-") as directory:
        probe = Path(directory)
        (probe / "grammar.js").write_text(
            "module.exports = grammar({name: 'merman_wasi_probe', "
            "rules: {source_file: ($) => 'x'}});\n",
            encoding="utf-8",
        )
        run(
            [*cli, "generate", "--abi", str(LANGUAGE_ABI), "--js-runtime", "node"],
            cwd=probe,
        )
        run(
            [
                *cli,
                "build",
                "--wasm",
                "--debug",
                "--output",
                "tree-sitter-merman-wasi-probe.wasm",
                ".",
            ],
            cwd=probe,
        )


def resolve_wasi_clang(cli: list[str]) -> Path:
    clang = find_wasi_clang()
    if clang is None:
        provision_wasi_sdk(cli)
        clang = find_wasi_clang()
    if clang is None:
        locations = ", ".join(str(path) for path in wasi_sdk_candidates())
        raise SystemExit(f"wasi-sdk clang is missing; checked {locations}")

    version = subprocess.run(
        [str(clang), "--version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    ).stdout.splitlines()[0]
    expected = f"clang version {WASI_CLANG_VERSION}"
    if not version.startswith(expected):
        raise SystemExit(f"expected {expected}, found {version}")
    return clang


def build_wasm(package: Path, cli: list[str], destination: Path) -> None:
    assert_language_abi(package / "src/parser.c")
    source = destination / "src"
    source.mkdir()
    for relative in (Path("parser.c"), Path("scanner.c")):
        shutil.copy2(package / "src" / relative, source / relative)
    shutil.copytree(package / "src/tree_sitter", source / "tree_sitter")

    output = destination / WASM_FILE
    clang = resolve_wasi_clang(cli)
    run(
        [
            str(clang),
            "--target=wasm32-unknown-wasi",
            "-o",
            str(output),
            "-fPIC",
            "-shared",
            "-Os",
            "-Wl,--export=tree_sitter_mermaid",
            "-Wl,--allow-undefined",
            "-Wl,--no-entry",
            "-nostdlib",
            "-fno-exceptions",
            "-fvisibility=hidden",
            "-I",
            ".",
            "-mllvm",
            WASM_IRREDUCIBLE_CFG_FLAG,
            "parser.c",
            "scanner.c",
        ],
        cwd=source,
    )
    # The published module is a browser asset. Use V8's baseline compiler for this
    # Node-side structural smoke so Node 24 does not optimize the generated lexer as
    # one unusually large function; browser execution is covered separately.
    run(
        [
            cli[0],
            "--liftoff-only",
            str(package / "scripts/validate_wasm.js"),
            str(output),
        ],
        cwd=package,
    )
    assert_size(output, WASM_MAX_BYTES)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument(
        "--wasm",
        action="store_true",
        help="build or verify only the root language WASM artifact",
    )
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    package = PACKAGE_ROOT
    cli = cli_command(package)
    version = subprocess.run(
        [*cli, "--version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    ).stdout.strip()
    validate_cli_version(version)
    source_failures = exact_source_set_failures(package)
    if source_failures:
        raise SystemExit("\n".join(source_failures))

    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-generate-") as directory:
        generated = Path(directory)
        if arguments.wasm:
            build_wasm(package, cli, generated)
            paths = (WASM_FILE,)
        else:
            generate_sources(package, cli, generated)
            paths = GENERATED_SOURCE_FILES

        if arguments.write:
            install_files(generated, package, paths)
            action = "updated"
        else:
            failures = compare_files(generated, package, paths)
            if failures:
                raise SystemExit("\n".join(failures))
            action = "verified"

    artifact = "language WASM" if arguments.wasm else "generated parser sources"
    print(f"{action} {artifact} for ABI {LANGUAGE_ABI}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
