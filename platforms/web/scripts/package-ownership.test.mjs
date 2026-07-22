import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";

import {
  assertPackageOutputOwnership,
  pruneUnownedGeneratedDirectories,
  publicSurfaceDirectoryNames,
} from "./wasm-build/package-ownership.mjs";

const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("published WASM package ownership", () => {
  it("derives every allowed sibling directory from the public surface descriptor", () => {
    const root = fixtureRoot();
    for (const directory of ["snippets", ...publicSurfaceDirectoryNames()]) {
      mkdirSync(path.join(root, directory), { recursive: true });
    }
    assert.doesNotThrow(() => assertPackageOutputOwnership(root));
  });

  it("fails closed for stale or undeclared package directories", () => {
    const root = fixtureRoot();
    mkdirSync(path.join(root, "math"));
    mkdirSync(path.join(root, "full-no-elk"));
    assert.throws(
      () => assertPackageOutputOwnership(root),
      /pkg\/full-no-elk[\s\S]*pkg\/math/,
    );
  });

  it("lets a root rebuild remove only unowned generated directories", () => {
    const root = fixtureRoot();
    mkdirSync(path.join(root, "editor"));
    mkdirSync(path.join(root, "math"));
    mkdirSync(path.join(root, ".editor.merman-wasm.lock"));
    assert.deepEqual(pruneUnownedGeneratedDirectories(root), ["math"]);
    assert.throws(() => assertPackageOutputOwnership(root), /\.editor\.merman-wasm\.lock/);
  });

  it("fails closed for unknown root files and nested surface entries", () => {
    const root = fixtureRoot();
    writeFileSync(path.join(root, "unexpected.wasm"), "stale");
    assert.throws(() => assertPackageOutputOwnership(root), /unexpected\.wasm/);
    rmSync(path.join(root, "unexpected.wasm"));

    mkdirSync(path.join(root, "editor", "unknown"), { recursive: true });
    assert.throws(
      () => assertPackageOutputOwnership(root),
      /pkg\/editor\/unknown/,
    );
  });
});

function fixtureRoot() {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-ownership-"));
  roots.push(root);
  return root;
}
