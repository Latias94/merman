import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

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
