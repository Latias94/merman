import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
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
