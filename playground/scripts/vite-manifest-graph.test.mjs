import assert from "node:assert/strict";
import test from "node:test";

import {
  collectManifestClosure,
  emittedFiles,
  manifestKeysForSource,
  ownersOfAsset,
  parseViteManifest,
  requireUniqueManifestSource,
  ViteManifestContractError,
} from "./vite-manifest-graph.mjs";

test("validated manifest exposes static/reachable closures and asset owners", () => {
  const graph = parseViteManifest(validManifest());
  assert.equal(
    requireUniqueManifestSource(graph, "index.html", (chunk) => chunk.isEntry),
    "index.html",
  );
  assert.deepEqual(manifestKeysForSource(graph, "src/feature.ts"), ["feature"]);
  assert.deepEqual(
    [...collectManifestClosure(graph, ["index.html"], "static")].sort(),
    ["index.html", "shared"],
  );
  assert.deepEqual(
    [...collectManifestClosure(graph, ["index.html"], "reachable")].sort(),
    ["feature", "index.html", "shared", "wasm"],
  );
  assert.deepEqual(
    [...emittedFiles(graph, new Set(["feature", "shared"]))].sort(),
    ["assets/feature.js", "assets/shared.js"],
  );
  assert.deepEqual(ownersOfAsset(graph, "assets/engine.wasm"), ["feature"]);
});

test("manifest rejects unknown edges, duplicate outputs, and unsafe paths", () => {
  const unknown = validManifest();
  unknown["index.html"].imports = ["missing"];
  assert.throws(() => parseViteManifest(unknown), /unknown chunk missing/u);

  const duplicate = validManifest();
  duplicate.shared.file = duplicate.feature.file;
  assert.throws(() => parseViteManifest(duplicate), /owned by both/u);

  const unsafe = validManifest();
  unsafe.shared.file = "../escape.js";
  assert.throws(
    () => parseViteManifest(unsafe),
    ViteManifestContractError,
  );
});

function validManifest() {
  return {
    "index.html": {
      file: "assets/index.js",
      src: "index.html",
      isEntry: true,
      imports: ["shared"],
      dynamicImports: ["feature"],
    },
    shared: { file: "assets/shared.js" },
    feature: {
      file: "assets/feature.js",
      src: "src/feature.ts",
      isDynamicEntry: true,
      imports: ["shared"],
      dynamicImports: ["wasm"],
      assets: ["assets/engine.wasm"],
    },
    wasm: {
      file: "assets/wasm.js",
      src: "src/engine.ts",
      isDynamicEntry: true,
    },
  };
}
