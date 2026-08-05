import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  packageDistClosure,
  packageRuntimeDistClosure,
} from "./package-dist-closure.mjs";

test("package dist closure follows JavaScript and declaration graphs exactly", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    const closure = packageDistClosure(root, "analysis");
    assert.deepEqual(closure.javascriptModules, [
      "package-entries/analysis.js",
      "public-catalog.js",
      "runtime-core.js",
    ]);
    assert.deepEqual(closure.declarationModules, [
      "index.d.ts",
      "package-entries/analysis.d.ts",
      "public-types.d.ts",
      "runtime-core.d.ts",
      "token-only.d.ts",
    ]);
    assert.deepEqual(closure.files, [
      "index.d.ts",
      "index.d.ts.map",
      "package-entries/analysis.d.ts",
      "package-entries/analysis.d.ts.map",
      "package-entries/analysis.js",
      "package-entries/analysis.js.map",
      "public-catalog.js",
      "public-catalog.js.map",
      "public-types.d.ts",
      "public-types.d.ts.map",
      "runtime-core.d.ts",
      "runtime-core.d.ts.map",
      "runtime-core.js",
      "runtime-core.js.map",
      "token-only.d.ts",
      "token-only.d.ts.map",
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("runtime package closure is independent from declaration artifacts", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-runtime-closure-"));
  try {
    writeClosureFixture(root);
    rmSync(path.join(root, "package-entries", "analysis.d.ts"));
    assert.deepEqual(packageRuntimeDistClosure(root, "analysis").javascriptModules, [
      "package-entries/analysis.js",
      "public-catalog.js",
      "runtime-core.js",
    ]);
    assert.throws(() => packageDistClosure(root, "analysis"), /Cannot resolve/u);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure fails closed on unresolved local modules", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "runtime-core.js"),
      'import "./missing.js";\nexport const core = 1;\n',
    );
    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /Cannot resolve \.\/missing\.js/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure allows only the owned WASM dynamic import", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "runtime-core.js"),
      'import("./late-runtime.js");\nexport const core = 1;\n',
    );
    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /must dynamically import only/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure rejects modules declared for another surface", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "runtime-core.js"),
      'import "./runtime-render.js";\nexport const core = 1;\n',
    );
    writeFileSync(
      path.join(root, "runtime-render.js"),
      "export const render = true;\n",
    );

    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /declared for another surface: runtime-render\.js/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure rejects undeclared external imports", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "runtime-core.js"),
      'import "unowned-package";\nexport const core = 1;\n',
    );

    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /undeclared external imports: unowned-package/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure rejects sibling package entries", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "package-entries", "analysis.js"),
      [
        'import "./full.js";',
        'import("../../artifacts/wasm/merman_wasm.js");',
        "export const value = true;",
        "",
      ].join("\n"),
    );
    writeFileSync(
      path.join(root, "package-entries", "full.js"),
      "export const full = true;\n",
    );

    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /reaches sibling package entries: package-entries\/full\.js/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package dist closure rejects declaration dynamic imports", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-dist-closure-"));
  try {
    writeClosureFixture(root);
    writeFileSync(
      path.join(root, "package-entries", "analysis.d.ts"),
      'import("./late.js");\nexport declare const value: true;\n',
    );

    assert.throws(
      () => packageDistClosure(root, "analysis"),
      /declaration closure must not use dynamic imports/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function writeClosureFixture(root) {
  write(
    root,
    "package-entries/analysis.js",
    [
      'import { core } from "../runtime-core.js";',
      'export { catalog } from "../public-catalog.js";',
      'import("../../artifacts/wasm/merman_wasm.js");',
      "export const value = core;",
      "",
    ].join("\n"),
  );
  write(
    root,
    "package-entries/analysis.d.ts",
    [
      'import type { Shared } from "../index.js";',
      'export { core } from "../runtime-core.js";',
      "export type Value = Shared;",
      "",
    ].join("\n"),
  );
  write(
    root,
    "runtime-core.js",
    'import { catalog } from "./public-catalog.js";\nexport const core = catalog;\n',
  );
  write(root, "runtime-core.d.ts", "export declare const core = 1;\n");
  write(root, "public-catalog.js", "export const catalog = 1;\n");
  write(root, "index.d.ts", 'export type { Shared } from "./public-types.js";\n');
  write(
    root,
    "public-types.d.ts",
    [
      'export declare const token: typeof import("./token-only.js").TOKEN;',
      "export interface Shared { value: string }",
      "",
    ].join("\n"),
  );
  write(root, "token-only.d.ts", "export declare const TOKEN: unique symbol;\n");

  for (const relative of [
    "package-entries/analysis.js",
    "package-entries/analysis.d.ts",
    "runtime-core.js",
    "runtime-core.d.ts",
    "public-catalog.js",
    "index.d.ts",
    "public-types.d.ts",
    "token-only.d.ts",
  ]) {
    write(root, `${relative}.map`, "{}\n");
  }
}

function write(root, relative, contents) {
  const target = path.join(root, ...relative.split("/"));
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
}
