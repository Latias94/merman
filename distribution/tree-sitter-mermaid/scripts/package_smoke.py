"""Install the staged npm, Cargo, WASM, and C package surfaces."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import textwrap
from pathlib import Path
import tomllib

from c_smoke import find_compiler, runtime_directory


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
PROVENANCE = json.loads(
    (PACKAGE_ROOT / "metadata/provenance.json").read_text(encoding="utf-8")
)
NODE_RUNTIME_VERSION = PROVENANCE["toolchain"]["nodeRuntime"]
RUST_RUNTIME_VERSION = PROVENANCE["toolchain"]["rustRuntime"]
WEB_RUNTIME_VERSION = PROVENANCE["toolchain"]["webRuntime"]


def run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=True,
        capture_output=capture_output,
        text=True,
        env=env,
    )


def safe_extract(archive: Path, destination: Path) -> Path:
    destination.mkdir()
    destination_root = destination.resolve()
    with tarfile.open(archive, "r:gz") as source:
        for member in source.getmembers():
            extracted = (destination / member.name).resolve()
            if extracted != destination_root and destination_root not in extracted.parents:
                raise SystemExit(f"unsafe archive path: {member.name}")
        source.extractall(destination)
    roots = [path for path in destination.iterdir() if path.is_dir()]
    if len(roots) != 1:
        raise SystemExit(f"expected one archive root, found {len(roots)}")
    return roots[0]


def pack_npm(package: Path, destination: Path, npm: str) -> Path:
    records = json.loads(
        run(
            [npm, "pack", str(package), "--pack-destination", str(destination), "--json"],
            cwd=destination,
            capture_output=True,
        ).stdout
    )
    if len(records) != 1:
        raise SystemExit(f"expected one npm package, found {len(records)}")
    record = records[0]
    paths = {item["path"] for item in record["files"]}
    required = {
        "CMakeLists.txt",
        "LICENSE",
        "Makefile",
        "README.md",
        "THIRD_PARTY_LICENSES/tree-sitter/LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "binding.gyp",
        "bindings/c/tree_sitter/tree-sitter-mermaid.h",
        "bindings/node/index.d.ts",
        "bindings/node/index.js",
        "grammar.js",
        "package.json",
        "queries/portable/highlights.scm",
        "src/node-types.json",
        "src/parser.c",
        "src/scanner.c",
        "tree-sitter-mermaid.wasm",
        "tree-sitter.json",
    }
    missing = sorted(required - paths)
    if missing:
        raise SystemExit(f"npm package is missing required files: {missing}")

    forbidden_prefixes = ("build/", "node_modules/", "scripts/", "test/", "tests/", "wasm/")
    forbidden = sorted(
        path for path in paths if any(path.startswith(prefix) for prefix in forbidden_prefixes)
    )
    if forbidden:
        raise SystemExit(f"npm package contains internal files: {forbidden}")
    if os.environ.get("TREE_SITTER_MERMAID_REQUIRE_PREBUILDS") == "1" and not any(
        path.startswith("prebuilds/") for path in paths
    ):
        raise SystemExit("npm release candidate has no native prebuild")
    return destination / record["filename"]


def run_npm_consumer(consumer: Path, tarball: Path, npm: str, node: str) -> None:
    manifest = {
        "name": "tree-sitter-mermaid-consumer-smoke",
        "private": True,
        "version": "0.0.0",
        "dependencies": {
            "tree-sitter": NODE_RUNTIME_VERSION,
            "tree-sitter-mermaid": tarball.resolve().as_uri(),
            "web-tree-sitter": WEB_RUNTIME_VERSION,
        },
    }
    (consumer / "package.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    environment = os.environ.copy()
    if os.environ.get("TREE_SITTER_MERMAID_REQUIRE_PREBUILDS") == "1":
        environment["PREBUILDS_ONLY"] = "1"
    run(
        [npm, "install", "--no-audit", "--no-fund"],
        cwd=consumer,
        env=environment,
    )
    native_smoke = r"""
const assert = require('node:assert/strict');
const Parser = require('tree-sitter');
const Mermaid = require('tree-sitter-mermaid');

const grammarMetadata = require('tree-sitter-mermaid/tree-sitter.json');
assert.equal(grammarMetadata.grammars[0].scope, 'source.mermaid');

const source = 'flowchart TD\nA --> B\n';
const parser = new Parser();
parser.setLanguage(Mermaid);
const tree = parser.parse(source);
assert.equal(tree.rootNode.hasError, false);
assert.equal(tree.rootNode.namedChildren[0].type, 'flowchart_diagram');
"""
    run([node, "-e", native_smoke], cwd=consumer, env=environment)

    wasm_smoke = r"""
const assert = require('node:assert/strict');
const { Language, Parser } = require('web-tree-sitter');

(async () => {
  await Parser.init();
  const language = await Language.load(
    require.resolve('tree-sitter-mermaid/tree-sitter-mermaid.wasm'),
  );
  assert.equal(language.abiVersion, 15);
  const wasmParser = new Parser();
  wasmParser.setLanguage(language);
  const wasmTree = wasmParser.parse('flowchart TD\nA --> B\n');
  assert.equal(wasmTree.rootNode.hasError, false);
  assert.equal(wasmTree.rootNode.namedChildren[0].type, 'flowchart_diagram');
  wasmTree.delete();
  wasmParser.delete();
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
"""
    # This validates the browser asset's packaged bytes and ABI without asking
    # Node 24's optimizing compiler to compile the generated lexer. Chromium owns
    # the default-runtime browser execution smoke.
    run([node, "--liftoff-only", "-e", wasm_smoke], cwd=consumer, env=environment)


def package_identity(package: Path) -> tuple[str, str]:
    manifest = tomllib.loads((package / "Cargo.toml").read_text(encoding="utf-8"))
    metadata = manifest["package"]
    return metadata["name"], metadata["version"]


def pack_cargo(
    workspace: Path, destination: Path, package_name: str, package_version: str
) -> Path:
    target = destination / "cargo-target"
    run(
        [
            os.environ.get("CARGO", "cargo"),
            "package",
            "--locked",
            "--allow-dirty",
            "--no-verify",
            "-p",
            package_name,
            "--target-dir",
            str(target),
        ],
        cwd=workspace,
    )
    crate = target / "package" / f"{package_name}-{package_version}.crate"
    if not crate.is_file():
        raise SystemExit(f"missing Cargo package: {crate}")
    return crate


def run_rust_consumer(consumer: Path, crate_root: Path) -> None:
    project = consumer / "rust-consumer"
    (project / "src").mkdir(parents=True)
    manifest = f"""
        [package]
        name = "tree-sitter-mermaid-consumer-smoke"
        version = "0.0.0"
        edition = "2024"

        [dependencies]
        tree-sitter = "={RUST_RUNTIME_VERSION}"
        tree-sitter-mermaid = {{ path = {json.dumps(str(crate_root))} }}
        """
    (project / "Cargo.toml").write_text(textwrap.dedent(manifest), encoding="utf-8")
    source = r"""
fn main() {
    let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
    assert_eq!(language.abi_version(), 15);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse("flowchart TD\nA --> B\n", None).unwrap();
    assert!(!tree.root_node().has_error());
    assert_eq!(tree.root_node().named_child(0).unwrap().kind(), "flowchart_diagram");
    tree_sitter::Query::new(&language, tree_sitter_mermaid::HIGHLIGHTS_QUERY).unwrap();
}
"""
    (project / "src/main.rs").write_text(source, encoding="utf-8")
    cargo = os.environ.get("CARGO", "cargo")
    run([cargo, "run"], cwd=project)


def run_cmake_consumer(
    package_root: Path,
    workspace_package: Path,
    smoke_source: Path,
    destination: Path,
) -> None:
    cmake = shutil.which("cmake")
    if cmake is None:
        raise SystemExit("cmake is required for the C package smoke")
    build = destination / "cmake-build"
    installed = destination / "cmake-install"
    run(
        [
            cmake,
            "-S",
            str(package_root),
            "-B",
            str(build),
            "-DBUILD_SHARED_LIBS=OFF",
            f"-DCMAKE_INSTALL_PREFIX={installed}",
        ],
        cwd=destination,
    )
    run([cmake, "--build", str(build), "--parallel", "1"], cwd=destination)
    run([cmake, "--install", str(build)], cwd=destination)

    runtime = runtime_directory(workspace_package)
    compiler = find_compiler()
    if Path(compiler[0]).name.lower().removesuffix(".exe") in {"cl", "clang-cl"}:
        raise SystemExit("the installed C archive smoke currently requires a Unix-style compiler")
    library = next((installed / "lib").glob("libtree-sitter-mermaid.a"), None)
    if library is None:
        library = next(installed.rglob("tree-sitter-mermaid.lib"), None)
    if library is None:
        raise SystemExit("CMake install did not produce the static grammar library")

    executable = destination / "tree-sitter-mermaid-installed-c-smoke"
    run(
        [
            *compiler,
            "-std=c11",
            f"-I{installed / 'include'}",
            f"-I{runtime / 'include'}",
            f"-I{runtime / 'src'}",
            str(smoke_source),
            str(runtime / "src/lib.c"),
            str(library),
            "-o",
            str(executable),
        ],
        cwd=destination,
    )
    run([str(executable)], cwd=destination)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="copy the exact smoke-tested npm and Cargo artifacts to this directory",
    )
    parser.add_argument(
        "--c-source-archive",
        type=Path,
        help="build, install, and link the C surface from this release source archive",
    )
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    package = PACKAGE_ROOT
    workspace = package.parents[1]
    package_name, package_version = package_identity(package)
    npm = shutil.which("npm")
    node = shutil.which("node")
    if npm is None or node is None:
        raise SystemExit("Node.js and npm are required for package smoke tests")

    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-package-") as directory:
        root = Path(directory)
        npm_consumer = root / "npm-consumer"
        npm_consumer.mkdir()
        tarball = pack_npm(package, npm_consumer, npm)
        npm_package = safe_extract(tarball, root / "npm-package")
        run_npm_consumer(npm_consumer, tarball, npm, node)

        crate = pack_cargo(workspace, root, package_name, package_version)
        crate_root = safe_extract(crate, root / "cargo-package")
        run_rust_consumer(root, crate_root)

        c_package = npm_package
        smoke_source = package / "tests/c_smoke.c"
        if arguments.c_source_archive is not None:
            c_package = safe_extract(
                arguments.c_source_archive.resolve(), root / "c-source-package"
            )
            smoke_source = c_package / "tests/c_smoke.c"
            if not smoke_source.is_file():
                raise SystemExit("C source archive is missing tests/c_smoke.c")
        run_cmake_consumer(c_package, package, smoke_source, root)

        if arguments.output_dir is not None:
            arguments.output_dir.mkdir(parents=True, exist_ok=True)
            shutil.copy2(tarball, arguments.output_dir / tarball.name)
            shutil.copy2(crate, arguments.output_dir / crate.name)

    print("verified npm, Node, WASM, Cargo, Rust, and C package consumers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
