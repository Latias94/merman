"""Install packaged artifacts in clean consumers without the grammar generator."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import textwrap
from pathlib import Path
import tomllib

from c_smoke import runtime_directory


NODE_RUNTIME_VERSION = "0.25.1"
WEB_TREE_SITTER_VERSION = "0.26.12"


def run(
    command: list[str], *, cwd: Path, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )


def package_metadata(package: Path) -> tuple[str, str]:
    metadata = tomllib.loads((package / "Cargo.toml").read_text(encoding="utf-8"))
    package_metadata = metadata["package"]
    return package_metadata["name"], package_metadata["version"]


def assert_no_packaged_work_directories(root: Path) -> None:
    forbidden = [
        ".git",
        "build",
        "node_modules",
        "target",
        "scripts/header-oracle/node_modules",
    ]
    present = [item for item in forbidden if (root / item).exists()]
    if present:
        raise SystemExit(f"package contains build or install directories: {present}")


def pack_npm_package(package: Path, consumer: Path, npm: str) -> Path:
    packed = json.loads(
        run(
            [
                npm,
                "pack",
                str(package),
                "--pack-destination",
                str(consumer),
                "--json",
            ],
            cwd=consumer,
        ).stdout
    )
    if len(packed) != 1:
        raise SystemExit(f"expected one npm tarball, found {len(packed)}")
    required = {
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "THIRD_PARTY_LICENSES/tree-sitter/LICENSE",
        "metadata/artifact-receipt.json",
        "metadata/evidence/u2-mermaid-header-oracle.json",
        "metadata/fixtures/family-roots.json",
        "queries/portable/highlights.scm",
        "scripts/header-oracle/package-lock.json",
        "src/parser.c",
        "src/scanner.c",
        "wasm/tree-sitter-mermaid.wasm",
    }
    forbidden_prefixes = (
        ".git/",
        "build/",
        "node_modules/",
        "target/",
        "scripts/header-oracle/node_modules/",
    )
    paths = {item["path"] for item in packed[0]["files"]}
    missing = sorted(required - paths)
    if missing:
        raise SystemExit(f"npm tarball is missing required paths: {missing}")
    forbidden = sorted(
        path for path in paths if path == ".git" or path.startswith(forbidden_prefixes)
    )
    if forbidden:
        raise SystemExit(f"npm tarball includes build or install directories: {forbidden}")
    return consumer / packed[0]["filename"]


def run_node_and_wasm_consumer(consumer: Path, tarball: Path, npm: str, node: str) -> None:
    manifest = {
        "name": "tree-sitter-mermaid-consumer-smoke",
        "private": True,
        "version": "0.0.0",
        "dependencies": {
            "tree-sitter": NODE_RUNTIME_VERSION,
            "tree-sitter-mermaid": tarball.resolve().as_uri(),
            "web-tree-sitter": WEB_TREE_SITTER_VERSION,
        },
    }
    (consumer / "package.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    run([npm, "install", "--no-audit", "--no-fund"], cwd=consumer)
    if (consumer / "node_modules/tree-sitter-cli").exists():
        raise SystemExit("clean consumer unexpectedly installed tree-sitter-cli")

    smoke = r"""
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');
const Mermaid = require('tree-sitter-mermaid');
const packageRoot = path.dirname(require.resolve('tree-sitter-mermaid/package.json'));
const fixtures = JSON.parse(fs.readFileSync(
  path.join(packageRoot, 'metadata/fixtures/family-roots.json'), 'utf8'));
assert.equal(fixtures.length, 35);
const parser = new Parser();
parser.setLanguage(Mermaid);
for (const fixture of fixtures) {
  const tree = parser.parse(fixture.source);
  assert.equal(tree.rootNode.hasError, false, fixture.publicId);
  const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
  assert.equal(roots.length, 1, fixture.publicId);
  assert.equal(roots[0].type, fixture.root, fixture.publicId);
}
const query = new Parser.Query(
  Mermaid,
  Mermaid.queryProfiles.portable.highlights.source,
);
const queryTree = parser.parse('flowchart TD\n  A --> B\n');
assert.ok(query.captures(queryTree.rootNode).some((capture) => capture.name === 'keyword'));
assert.equal(Mermaid.artifactReceipt.language.abi, 14);
"""
    run([node, "-e", smoke], cwd=consumer)

    wasm_smoke = r"""
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Language, Parser, Query } = require('web-tree-sitter');
const wasmBinding = require('tree-sitter-mermaid/bindings/wasm');

(async () => {
  const packageRoot = path.dirname(require.resolve('tree-sitter-mermaid/package.json'));
  const fixtures = JSON.parse(fs.readFileSync(
    path.join(packageRoot, 'metadata/fixtures/family-roots.json'), 'utf8'));
  await Parser.init({ wasmMemory: new WebAssembly.Memory({ initial: 512, maximum: 32768 }) });
  const language = await Language.load(wasmBinding.languagePath);
  assert.equal(language.abiVersion, 14);

  const parser = new Parser();
  parser.setLanguage(language);
  for (const fixture of fixtures) {
    const tree = parser.parse(fixture.source);
    assert.equal(tree.rootNode.hasError, false, fixture.publicId);
    const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
    assert.equal(roots.length, 1, fixture.publicId);
    assert.equal(roots[0].type, fixture.root, fixture.publicId);
    tree.delete();
  }
  const highlights = wasmBinding.queryProfiles.portable.highlights;
  const query = new Query(language, highlights.source);
  const tree = parser.parse('flowchart TD\n  A --> B\n');
  assert.ok(query.captures(tree.rootNode).some((capture) => capture.name === 'keyword'));
  query.delete();
  tree.delete();
  parser.delete();
})();
"""
    run([node, "-e", wasm_smoke], cwd=consumer)


def extract_crate(crate_file: Path, destination: Path) -> Path:
    destination.mkdir()
    destination_root = destination.resolve()
    with tarfile.open(crate_file, "r:gz") as archive:
        for member in archive.getmembers():
            member_path = (destination / member.name).resolve()
            if member_path != destination_root and destination_root not in member_path.parents:
                raise SystemExit(f"unsafe crate archive path: {member.name}")
        archive.extractall(destination)
    roots = [path for path in destination.iterdir() if path.is_dir()]
    if len(roots) != 1:
        raise SystemExit(f"expected one crate root, found {len(roots)}")
    assert_no_packaged_work_directories(roots[0])
    return roots[0]


def pack_cargo_crate(package: Path, workspace: Path, consumer: Path) -> Path:
    cargo = os.environ.get("CARGO", "cargo")
    package_name, package_version = package_metadata(package)
    target_directory = consumer / "cargo-target"
    run(
        [
            cargo,
            "package",
            "--locked",
            "-p",
            package_name,
            "--allow-dirty",
            "--target-dir",
            str(target_directory),
        ],
        cwd=workspace,
    )
    crate_file = target_directory / "package" / f"{package_name}-{package_version}.crate"
    if not crate_file.is_file():
        raise SystemExit(f"missing cargo crate artifact: {crate_file}")
    return crate_file


def assert_cargo_crate_files(crate_root: Path) -> None:
    required = {
        "Cargo.toml",
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "THIRD_PARTY_LICENSES/tree-sitter/LICENSE",
        "metadata/artifact-receipt.json",
        "metadata/fixtures/family-roots.json",
        "queries/portable/highlights.scm",
        "src/parser.c",
        "src/scanner.c",
        "src/tree_sitter/parser.h",
        "src/node-types.json",
        "bindings/c/tree_sitter/tree-sitter-mermaid.h",
        "bindings/rust/build.rs",
        "bindings/rust/lib.rs",
        "tests/c_smoke.c",
        "scripts/c_smoke.py",
        "wasm/tree-sitter-mermaid.wasm",
    }
    paths = {str(path.relative_to(crate_root)) for path in crate_root.rglob("*") if path.is_file()}
    missing = sorted(required - paths)
    if missing:
        raise SystemExit(f"cargo crate is missing required paths: {missing}")


def run_rust_consumer(consumer: Path, crate_root: Path) -> None:
    root = consumer / "rust-consumer"
    (root / "src").mkdir(parents=True)
    manifest = f"""
        [package]
        name = "tree-sitter-mermaid-rust-consumer-smoke"
        version = "0.0.0"
        edition = "2024"
        publish = false

        [dependencies]
        serde = {{ version = "1.0", features = ["derive"] }}
        serde_json = "1.0"
        tree-sitter = "=0.26.12"
        tree-sitter-mermaid = {{ path = {json.dumps(str(crate_root))} }}
        """
    (root / "Cargo.toml").write_text(textwrap.dedent(manifest), encoding="utf-8")
    source = f"""
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Fixture {{
            #[serde(rename = "publicId")]
            public_id: String,
            root: String,
            source: String,
        }}

        fn main() {{
            let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
            assert_eq!(language.abi_version(), 14);

            let fixtures_path = {json.dumps(str(crate_root / "metadata/fixtures/family-roots.json"))};
            let fixtures: Vec<Fixture> = serde_json::from_str(
                &std::fs::read_to_string(fixtures_path).expect("fixtures must be readable"),
            )
            .expect("fixtures must be valid JSON");
            assert_eq!(fixtures.len(), 35);

            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&language).expect("language must load");
            for fixture in fixtures {{
                let tree = parser
                    .parse(&fixture.source, None)
                    .unwrap_or_else(|| panic!("parse returned None for {{}}", fixture.public_id));
                let root = tree.root_node();
                assert_eq!(root.kind(), "source_file", "{{}}", fixture.public_id);
                assert!(!root.has_error(), "{{}}", fixture.public_id);

                let mut cursor = root.walk();
                let roots: Vec<_> = root
                    .named_children(&mut cursor)
                    .filter(|node| node.kind().ends_with("_diagram"))
                    .collect();
                assert_eq!(roots.len(), 1, "{{}}", fixture.public_id);
                assert_eq!(roots[0].kind(), fixture.root, "{{}}", fixture.public_id);
            }}

            let profile = tree_sitter_mermaid::query_profile("portable", "highlights")
                .expect("portable highlight query must be packaged");
            tree_sitter::Query::new(&language, profile.source)
                .expect("portable highlight query must compile");
            assert!(tree_sitter_mermaid::ARTIFACT_RECEIPT.contains("\\\"receiptId\\\""));
        }}
        """
    (root / "src/main.rs").write_text(textwrap.dedent(source), encoding="utf-8")
    run(
        [
            os.environ.get("CARGO", "cargo"),
            "run",
            "--manifest-path",
            str(root / "Cargo.toml"),
            "--quiet",
        ],
        cwd=root,
    )


def run_c_consumer(source_package: Path, crate_root: Path) -> None:
    env = os.environ.copy()
    env["TREE_SITTER_RUNTIME_DIR"] = str(runtime_directory(source_package))
    run([sys.executable, str(crate_root / "scripts/c_smoke.py")], cwd=crate_root, env=env)


def main() -> int:
    package = Path(__file__).resolve().parents[1]
    workspace = package.parents[1]
    npm = shutil.which("npm")
    node = shutil.which("node")
    if npm is None or node is None:
        raise SystemExit("npm and node are required for the clean consumer smoke")
    if shutil.which(os.environ.get("CARGO", "cargo")) is None:
        raise SystemExit("cargo is required for the clean Rust/C consumer smoke")
    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-consumer-") as directory:
        consumer = Path(directory)
        tarball = pack_npm_package(package, consumer, npm)
        run_node_and_wasm_consumer(consumer, tarball, npm, node)

        crate_file = pack_cargo_crate(package, workspace, consumer)
        crate_root = extract_crate(crate_file, consumer / "crate")
        assert_cargo_crate_files(crate_root)
        run_rust_consumer(consumer, crate_root)
        run_c_consumer(package, crate_root)

    print("verified clean npm, language-WASM, Rust, and C consumers without tree-sitter-cli")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
