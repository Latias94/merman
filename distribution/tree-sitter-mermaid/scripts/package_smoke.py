"""Install the npm tarball in a clean consumer without the grammar generator."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from pathlib import Path


NODE_RUNTIME_VERSION = "0.25.1"


def run(command: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )


def main() -> int:
    package = Path(__file__).resolve().parents[1]
    npm = shutil.which("npm")
    node = shutil.which("node")
    if npm is None or node is None:
        raise SystemExit("npm and node are required for the clean consumer smoke")
    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-consumer-") as directory:
        consumer = Path(directory)
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
        tarball = consumer / packed[0]["filename"]
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
        paths = {item["path"] for item in packed[0]["files"]}
        missing = sorted(required - paths)
        if missing:
            raise SystemExit(f"npm tarball is missing required paths: {missing}")

        manifest = {
            "name": "tree-sitter-mermaid-consumer-smoke",
            "private": True,
            "version": "0.0.0",
            "dependencies": {
                "tree-sitter": NODE_RUNTIME_VERSION,
                "tree-sitter-mermaid": tarball.resolve().as_uri(),
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

    print("verified clean npm consumer without tree-sitter-cli")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
