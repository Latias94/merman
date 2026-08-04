import assert from "node:assert/strict";
import test from "node:test";

import {
  OPTIONAL_FEATURE_SOURCES,
  inspectOptionalFeatureManifest,
} from "./optional-feature-manifest.mjs";

test("optional workbenches are dynamically reachable but absent from startup", () => {
  const manifest = validManifest();
  const result = inspectOptionalFeatureManifest(manifest);

  assert.deepEqual(result.violations, []);
  assert.equal(result.entryKey, "index.html");
  assert.deepEqual(result.featureRoots, {
    benchmark: OPTIONAL_FEATURE_SOURCES.benchmark,
    config: OPTIONAL_FEATURE_SOURCES.config,
    examples: OPTIONAL_FEATURE_SOURCES.examples,
  });
  assert.deepEqual([...result.initialStaticKeys].sort(), ["_shell.js", "index.html"]);
});

test("an optional workbench cannot enter the initial static closure", () => {
  const manifest = validManifest();
  manifest["index.html"].imports.push(OPTIONAL_FEATURE_SOURCES.benchmark);

  const result = inspectOptionalFeatureManifest(manifest);
  assert.match(result.violations.join("\n"), /benchmark is present/);
});

test("an optional workbench cannot alias an initial output file", () => {
  const manifest = validManifest();
  manifest[OPTIONAL_FEATURE_SOURCES.benchmark].file = "shell.js";

  const result = inspectOptionalFeatureManifest(manifest);
  assert.match(result.violations.join("\n"), /benchmark is present/);
});

test("every optional workbench must remain dynamically reachable", () => {
  const manifest = validManifest();
  manifest["index.html"].dynamicImports = manifest[
    "index.html"
  ].dynamicImports.filter((key) => key !== OPTIONAL_FEATURE_SOURCES.examples);

  const result = inspectOptionalFeatureManifest(manifest);
  assert.match(result.violations.join("\n"), /examples is not dynamically reachable/);
});

test("missing and ambiguous feature roots fail closed", () => {
  const manifest = validManifest();
  delete manifest[OPTIONAL_FEATURE_SOURCES.config];
  manifest["config-copy"] = {
    file: "config-copy.js",
    src: OPTIONAL_FEATURE_SOURCES.benchmark,
  };

  const result = inspectOptionalFeatureManifest(manifest);
  const violations = result.violations.join("\n");
  assert.match(violations, /config activation root.*found 0/);
  assert.match(violations, /benchmark activation root.*found 2/);
});

function validManifest() {
  return {
    "index.html": {
      file: "index.js",
      src: "index.html",
      isEntry: true,
      imports: ["_shell.js"],
      dynamicImports: Object.values(OPTIONAL_FEATURE_SOURCES),
    },
    "_shell.js": { file: "shell.js" },
    [OPTIONAL_FEATURE_SOURCES.benchmark]: {
      file: "benchmark.js",
      src: OPTIONAL_FEATURE_SOURCES.benchmark,
      isDynamicEntry: true,
      imports: ["_shell.js"],
    },
    [OPTIONAL_FEATURE_SOURCES.config]: {
      file: "config.js",
      src: OPTIONAL_FEATURE_SOURCES.config,
      isDynamicEntry: true,
      imports: ["_shell.js"],
    },
    [OPTIONAL_FEATURE_SOURCES.examples]: {
      file: "examples.js",
      src: OPTIONAL_FEATURE_SOURCES.examples,
      isDynamicEntry: true,
      imports: ["_shell.js"],
    },
  };
}
