import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { loadTypeScriptContract } from "./typescript-contract.mjs";

test("reads the resolved TypeScript contract instead of matching source spelling", () => {
  withProject(
    {
      "index.ts": `
        export interface RuntimeModule {
          readonly version: 2;
          render(source: string): string;
        }
        export type Operation = "measure" | "bbox-x";
        export function wrapper(): void {}

        const unrelated = { draw: (_input: unknown) => "dead" };
        export function bindRuntime() {
          const renamed = unrelated;
          return {
            render(source: string) { return source; },
            nested: renamed,
          };
        }
      `,
    },
    ({ root, contract }) => {
      const entry = path.join(root, "index.ts");

      assert.deepEqual([...contract.exportedValueNames(entry)].sort(), [
        "bindRuntime",
        "wrapper",
      ]);
      assert.deepEqual([...contract.declaredValueExportNames(entry)].sort(), [
        "bindRuntime",
        "wrapper",
      ]);
      assert.deepEqual([...contract.exportedTypeNames(entry)].sort(), [
        "Operation",
        "RuntimeModule",
      ]);
      assert.deepEqual(
        [...contract.exportedTypePropertyNames(entry, "RuntimeModule")].sort(),
        ["render", "version"],
      );
      assert.equal(
        contract.exportedTypePropertyText(entry, "RuntimeModule", "version"),
        "2",
      );
      assert.deepEqual(
        [...contract.exportedStringLiteralMembers(entry, "Operation")].sort(),
        ["bbox-x", "measure"],
      );
      assert.deepEqual(
        [...contract.exportedFunctionReturnPropertyNames(entry, "bindRuntime")].sort(),
        ["nested", "render"],
      );
      assert.equal(
        contract.exportedFunctionReturnPropertyNames(entry, "bindRuntime").has("draw"),
        false,
        "a same-named property in an unrelated object must not satisfy the runtime contract",
      );
      assert.deepEqual(contract.diagnostics(), []);
    },
  );
});

test("resolves value and type re-exports through the compiler", () => {
  withProject(
    {
      "types.ts": `
        export interface Options { readonly enabled: boolean }
        export const VALUE = 2;
      `,
      "index.ts": `
        export type { Options as PublicOptions } from "./types.js";
        export { VALUE as ABI_VERSION } from "./types.js";
        export type * from "./types.js";
      `,
    },
    ({ root, contract }) => {
      const entry = path.join(root, "index.ts");
      assert.deepEqual(
        [...contract.exportedValueNames(entry)].sort(),
        ["ABI_VERSION", "VALUE"],
        "the checker resolves type-star targets, including their dual-space symbols",
      );
      assert.deepEqual(
        [...contract.exportedTypeNames(entry)].sort(),
        ["Options", "PublicOptions"],
      );
      assert.deepEqual(
        [...contract.exportedTypePropertyNames(entry, "PublicOptions")],
        ["enabled"],
      );
      assert.deepEqual([...contract.declaredValueExportNames(entry)], ["ABI_VERSION"]);
      assert.deepEqual([...contract.typeOnlyStarExportSpecifiers(entry)], ["./types.js"]);
      assert.deepEqual([...contract.valueStarExportSpecifiers(entry)], []);
    },
  );
});

function withProject(files, run) {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-ts-contract-"));
  try {
    writeFileSync(
      path.join(root, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: {
          strict: true,
          module: "ES2020",
          moduleResolution: "Bundler",
          target: "ES2020",
        },
        include: ["*.ts"],
      }),
    );
    for (const [relative, source] of Object.entries(files)) {
      writeFileSync(path.join(root, relative), source);
    }
    const contract = loadTypeScriptContract({
      tsconfigPath: path.join(root, "tsconfig.json"),
    });
    run({ root, contract });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
