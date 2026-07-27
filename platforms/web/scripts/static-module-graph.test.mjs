import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { assertDeclaredSurfaceModuleGraph } from "./package-dist-closure.mjs";
import {
  collectStaticModuleGraph,
  relativeModuleFiles,
} from "./static-module-graph.mjs";
import { webPackages } from "./surface-manifest.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = path.join(webRoot, "src");
const capabilityModulesByPackage = Object.freeze({
  analysis: {
    required: ["runtime-analysis.ts"],
    forbidden: ["runtime-ascii.ts", "runtime-editor.ts", "runtime-render.ts"],
  },
  ascii: {
    required: ["runtime-ascii.ts"],
    forbidden: ["runtime-analysis.ts", "runtime-editor.ts", "runtime-render.ts"],
  },
  editor: {
    required: ["runtime-analysis.ts", "runtime-editor.ts"],
    forbidden: ["runtime-ascii.ts", "runtime-render.ts"],
  },
  full: {
    required: [
      "runtime-analysis.ts",
      "runtime-ascii.ts",
      "runtime-editor.ts",
      "runtime-render.ts",
    ],
    forbidden: [],
  },
  render: {
    required: ["runtime-render.ts"],
    forbidden: ["runtime-analysis.ts", "runtime-ascii.ts", "runtime-editor.ts"],
  },
});

for (const descriptor of webPackages) {
  test(`${descriptor.id} source entry has the declared surface module graph`, () => {
    const graph = collectStaticModuleGraph({
      entry: path.join(sourceRoot, "package-entries", `${descriptor.id}.ts`),
      root: sourceRoot,
    });
    const files = relativeModuleFiles(graph);
    assert.doesNotThrow(() =>
      assertDeclaredSurfaceModuleGraph(graph, descriptor),
    );
    assert.equal(
      files.includes("index.ts"),
      false,
      `${descriptor.id} runtime closure must not import the public root aggregator`,
    );
    assert.deepEqual(
      graph.dynamicImports,
      ["../../artifacts/wasm/merman_wasm.js"],
    );
    const expected = capabilityModulesByPackage[descriptor.id];
    assert.ok(expected, `missing capability closure expectation for ${descriptor.id}`);
    for (const required of expected.required) {
      assert.equal(
        files.includes(required),
        true,
        `${descriptor.id} runtime closure must contain ${required}`,
      );
    }
    for (const forbidden of expected.forbidden) {
      assert.equal(
        files.includes(forbidden),
        false,
        `${descriptor.id} runtime closure must exclude ${forbidden}`,
      );
    }
  });
}

function temporaryGraph(t) {
  const parent = mkdtempSync(path.join(os.tmpdir(), "merman-module-graph-"));
  const root = path.join(parent, "root");
  const outside = path.join(parent, "outside");
  mkdirSync(root);
  mkdirSync(outside);
  t.after(() => rmSync(parent, { force: true, recursive: true }));
  return { outside, root };
}

test(
  "static module graph rejects a file symlink outside its root",
  { skip: process.platform === "win32" },
  (t) => {
    const { outside, root } = temporaryGraph(t);
    writeFileSync(path.join(root, "entry.js"), 'import "./escape.js";\n');
    writeFileSync(path.join(outside, "escape.js"), "export const escaped = true;\n");
    symlinkSync(path.join(outside, "escape.js"), path.join(root, "escape.js"));

    assert.throws(
      () => collectStaticModuleGraph({ entry: path.join(root, "entry.js"), root }),
      /dependency escapes/,
    );
  },
);

test(
  "static module graph rejects a directory symlink outside its root",
  { skip: process.platform === "win32" },
  (t) => {
    const { outside, root } = temporaryGraph(t);
    writeFileSync(path.join(root, "entry.js"), 'import "./escape/value.js";\n');
    writeFileSync(path.join(outside, "value.js"), "export const escaped = true;\n");
    symlinkSync(outside, path.join(root, "escape"), "dir");

    assert.throws(
      () => collectStaticModuleGraph({ entry: path.join(root, "entry.js"), root }),
      /dependency escapes/,
    );
  },
);

test("static module graph rejects extensionless local specifiers", (t) => {
  const { root } = temporaryGraph(t);
  writeFileSync(path.join(root, "entry.js"), 'import "./dependency";\n');
  writeFileSync(path.join(root, "dependency.js"), "export const value = true;\n");

  assert.throws(
    () => collectStaticModuleGraph({ entry: path.join(root, "entry.js"), root }),
    /must use an explicit \.js specifier/,
  );
});
