import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { initMerman as initMermanCore } from "../dist/runtime-core.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";
import { packageEntrySource, replaceDirectory } from "./build-surface-packages.mjs";
import {
  resourceContractValueExportNames,
  webPackages,
} from "./surface-manifest.mjs";
import { loadTypeScriptContract } from "./typescript-contract.mjs";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

describe("browser package assembly", () => {
  it("executes its command through directory aliases", (context) => {
    const temporary = mkdtempSync(path.join(os.tmpdir(), "merman-web-entrypoint-"));
    context.after(() => rmSync(temporary, { recursive: true, force: true }));
    const alias = path.join(temporary, "scripts-alias");
    symlinkSync(path.join(webRoot, "scripts"), alias,
      process.platform === "win32" ? "junction" : "dir");
    const result = spawnSync(process.execPath,
      [path.join(alias, "build-surface-packages.mjs")], { encoding: "utf8" });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 2, "invalid inputs must fail rather than skip main");
    assert.match(result.stderr, /usage: node scripts\/build-surface-packages/);
  });

  it("projects generated resource discovery values from every public package", () => {
    for (const descriptor of webPackages) {
      for (const name of resourceContractValueExportNames) {
        assert.ok(
          descriptor.valueExportNames.includes(name),
          `${descriptor.id} package omitted ${name}`,
        );
      }
      assert.match(
        packageEntrySource(descriptor),
        /from "\.\.\/generated\/resource-contract\.js"/,
      );
    }
  });

  it("keeps the root barrel synchronized with resource contract exports", () => {
    const contract = loadTypeScriptContract({
      tsconfigPath: path.join(webRoot, "tsconfig.json"),
    });
    const diagnostics = contract.diagnostics();
    assert.equal(diagnostics.length, 0, contract.formatDiagnostics(diagnostics));
    const rootExports = contract.exportedValueNames(
      path.join(webRoot, "src", "index.ts"),
    );
    assert.deepEqual(
      resourceContractValueExportNames.filter((name) => !rootExports.has(name)),
      [],
    );
  });

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
    assert.match(source, /wasmModulePromise \?\?= loadPackageWasmModule\(\)/);
    assert.match(
      source,
      /input \?\? \{ module_or_path: MERMAN_WASM_URL \}/,
    );
    assert.match(source, /assertBrowserRuntime, bindSurfaceRuntime/);
    assert.match(
      source,
      /bindSurfaceRuntime\(loadMermanWasmModule, implementation, MERMAN_WASM_URL\)/,
    );
    assert.match(source, /from "\.\.\/runtime-core\.js"/);
    assert.match(source, /from "\.\.\/runtime-analysis\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-ascii\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-render\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/runtime-editor\.js"/);
    assert.doesNotMatch(source, /from "\.\.\/svg-safety\.js"/);
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

  it("supplies the package WASM URL to a caller loader without replacing its module", async () => {
    const wasmUrl = "https://example.test/merman_wasm_bg.wasm";
    let receivedInput = null;
    const module = {
      async default(input) {
        receivedInput = input;
      },
      transportApiVersion: () => 5,
    };
    const runtime = bindSurfaceRuntime(
      async () => {
        throw new Error("default loader should not run");
      },
      { initMerman: initMermanCore },
      wasmUrl,
    );

    const loaded = await runtime.initMerman(async () => module);

    assert.equal(loaded, module);
    assert.deepEqual(receivedInput, { module_or_path: wasmUrl });
  });
});
