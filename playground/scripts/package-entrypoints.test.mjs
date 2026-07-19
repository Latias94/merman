import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const packageJson = JSON.parse(
  await readFile(path.resolve(import.meta.dirname, "..", "package.json"), "utf8")
);
const npmrc = await readFile(
  path.resolve(import.meta.dirname, "..", ".npmrc"),
  "utf8"
);

test("dev, build, and test fail closed on both consumed WASM surfaces", () => {
  assert.match(npmrc, /^ignore-scripts=true$/mu);
  assert.equal(packageJson.packageManager, "npm@11.17.0");
  assert.equal(packageJson.engines.node, ">=22.12.0");
  assert.equal(packageJson.engines.npm, ">=11.17.0");
  assert.equal(packageJson.scripts.predev, undefined);
  assert.equal(packageJson.scripts.prebuild, undefined);
  assert.equal(packageJson.scripts.pretest, undefined);
  assert.equal(packageJson.scripts.postbuild, undefined);
  for (const script of ["dev", "build", "test"]) {
    assert.match(
      packageJson.scripts[script],
      /^npm run prepare:browser-runtime(?: && |$)/u
    );
  }
  assert.match(packageJson.scripts.build, /npm run verify:dist$/u);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /verify-wasm-inputs\.mjs/);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /--preset browser-editor/);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /--out-dir-rel pkg\/editor/);
  assert.match(packageJson.scripts["prepare:browser-runtime"], /build:opaque-realm/);
  assert.match(packageJson.scripts["prepare:browser-runtime"], /verify:opaque-realm/);
});
