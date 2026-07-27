import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";

import { packageEntrySource, replaceDirectory } from "./build-surface-packages.mjs";
import { webPackages } from "./surface-manifest.mjs";

describe("browser package assembly", () => {
  it("restores the existing package projection when final replacement fails", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-package-"));
    try {
      const target = path.join(root, "artifacts");
      const stage = path.join(root, ".artifacts-stage");
      const backup = path.join(root, ".artifacts-backup");
      mkdirSync(target, { recursive: true });
      mkdirSync(stage, { recursive: true });
      writeFileSync(path.join(target, "current.txt"), "current");
      writeFileSync(path.join(stage, "generated.txt"), "generated");

      const fsOps = {
        existsSync,
        rmSync,
        renameSync(source, destination) {
          if (source === stage && destination === target) {
            throw new Error("simulated final rename failure");
          }
          renameSync(source, destination);
        },
      };

      assert.throws(
        () => replaceDirectory({ target, stage, backup, fsOps }),
        /simulated final rename failure/,
      );
      assert.equal(readFileSync(path.join(target, "current.txt"), "utf8"), "current");
      assert.equal(existsSync(backup), false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("generates a package-owned loader and no sibling package import", () => {
    const descriptor = webPackages.find(({ id }) => id === "analysis");
    assert.ok(descriptor);
    const source = packageEntrySource(descriptor);
    assert.match(source, /artifacts\/wasm\/merman_wasm\.js/);
    assert.match(source, /MERMAN_WASM_URL/);
    assert.match(source, /assertBrowserRuntime, bindSurfaceRuntime/);
    assert.match(source, /from "\.\.\/runtime-core\.js"/);
    assert.match(source, /from "\.\.\/runtime-analysis\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-ascii\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-render\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-editor\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/svg-safety\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/generated\/token-descriptor\.js"/);
    assert.doesNotMatch(
      source,
      /export \{[\s\S]*?\} from "\.\.\/index\.js"/,
    );
    assert.match(source, /import type \{[\s\S]*?\} from "\.\.\/index\.js"/);
    assert.match(source, /export type \* from "\.\.\/index\.js"/);
    assert.doesNotMatch(source, /function assertBrowserRuntime/);
    assert.doesNotMatch(source, /pkg\//);
    assert.doesNotMatch(source, /@mermanjs\/web\//);
    assert.match(
      source,
      /export type MermanWasmModule = Required<Pick<SharedMermanWasmModule,[\s\S]*\| "analyze"/,
    );
    assert.doesNotMatch(source, /\| "renderSvg"/);
  });
});
