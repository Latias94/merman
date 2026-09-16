import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { resolveNodeTarget } from "../src/native-loader.mjs";
import { resolveInstalledEntrypoint } from "../scripts/smoke-installed-package.mjs";

test("installed-package smoke resolves the ESM-only public loader entrypoint", (context) => {
  const project = mkdtempSync(path.join(os.tmpdir(), "merman-node-installed-smoke-"));
  context.after(() => rmSync(project, { recursive: true, force: true }));

  const packageRoot = path.join(project, "node_modules", "@mermanjs", "node");
  mkdirSync(path.join(packageRoot, "dist"), { recursive: true });
  writeFileSync(path.join(project, "package.json"), '{"type":"module"}\n');
  writeFileSync(
    path.join(packageRoot, "package.json"),
    `${JSON.stringify(
      {
        name: "@mermanjs/node",
        version: "0.0.0-test",
        type: "module",
        exports: { ".": { import: "./dist/index.mjs" } },
      },
      null,
      2,
    )}\n`,
  );
  const expectedEntrypoint = path.join(packageRoot, "dist", "index.mjs");
  writeFileSync(expectedEntrypoint, "export const installed = true;\n");

  assert.equal(
    realpathSync(resolveInstalledEntrypoint(project)),
    realpathSync(expectedEntrypoint),
  );
});

test("installed-package smoke rejects a native target different from the executing host", () => {
  const target = resolveNodeTarget() === "win32-x64-msvc" ? "darwin-arm64" : "win32-x64-msvc";
  const result = spawnSync(process.execPath, [
    fileURLToPath(new URL("../scripts/smoke-installed-package.mjs", import.meta.url)),
    "--project", os.tmpdir(), "--version", "0.0.0-test", "--target", target,
  ], { encoding: "utf8" });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Installed native package smoke target must match the current host/);
});

test("release and benchmark commands execute through directory aliases", (context) => {
  const temporary = mkdtempSync(path.join(os.tmpdir(), "merman-script-entrypoint-"));
  context.after(() => rmSync(temporary, { recursive: true, force: true }));
  const cases = [
    ["../scripts/smoke-installed-package.mjs", [], 1, /usage: node smoke-installed-package/],
    ["../scripts/build-candidate.mjs", [], 1, /--candidate is required/],
    ["../scripts/assemble-packages.mjs", [], 1, /usage: node scripts\/assemble-packages/],
    ["../scripts/verify-packages.mjs", ["--packed-root", path.join(temporary, "absent")],
      1, /contains no recognized package/],
    ["../scripts/benchmark/run.mjs", [], 1, /--native and --wasm are required/],
    ["../scripts/benchmark/worker.mjs", [], 1, /requires an input JSON path/],
    ["../../web/scripts/build-surface-packages.mjs", [], 2, /usage: node scripts\/build-surface-packages/],
  ];
  for (const [index, [relative, args, status, diagnostic]] of cases.entries()) {
    const script = fileURLToPath(new URL(relative, import.meta.url));
    const alias = path.join(temporary, `scripts-alias-${index}`);
    symlinkSync(path.dirname(script), alias, process.platform === "win32" ? "junction" : "dir");
    const result = spawnSync(process.execPath, [path.join(alias, path.basename(script)), ...args], {
      encoding: "utf8",
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status, status, `${relative}: invalid inputs must fail rather than skip main`);
    assert.match(result.stderr, diagnostic);
  }
});
