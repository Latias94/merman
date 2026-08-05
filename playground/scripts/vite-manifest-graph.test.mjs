import assert from "node:assert/strict";
import test from "node:test";

import {
  collectManifestClosure,
  emittedFiles,
  emittedResources,
  htmlStaticAssets,
  manifestKeysForSource,
  manifestOutputs,
  missingManifestOutputs,
  missingStaticStylesheets,
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

test("manifest output verification includes CSS and emitted assets", () => {
  const graph = parseViteManifest(validManifest());
  assert.deepEqual(
    [...emittedResources(graph, new Set(["index.html", "shared"]))].sort(),
    [
      "assets/app.css",
      "assets/app.woff2",
      "assets/index.js",
      "assets/shared.js",
    ],
  );
  assert.deepEqual(
    manifestOutputs(graph).filter((output) => output.key === "index.html"),
    [
      { key: "index.html", kind: "file", file: "assets/index.js" },
      { key: "index.html", kind: "css", file: "assets/app.css" },
      { key: "index.html", kind: "asset", file: "assets/app.woff2" },
    ],
  );

  const available = new Set(
    manifestOutputs(graph)
      .map((output) => output.file)
      .filter((file) => file !== "assets/app.css" && file !== "assets/app.woff2"),
  );
  assert.deepEqual(missingManifestOutputs(graph, (file) => available.has(file)), [
    { key: "index.html", kind: "css", file: "assets/app.css" },
    { key: "index.html", kind: "asset", file: "assets/app.woff2" },
  ]);
  assert.deepEqual(
    missingStaticStylesheets(
      graph,
      new Set(["index.html", "shared"]),
      ["assets/app.css"],
    ),
    [],
  );
  assert.deepEqual(
    missingStaticStylesheets(graph, new Set(["index.html", "shared"]), []),
    ["assets/app.css"],
  );
});

test("HTML static assets include scripts, module preloads, and stylesheets", () => {
  assert.deepEqual(
    htmlStaticAssets(`
      <link href="/assets/shared.js" rel="modulepreload crossorigin">
      <link rel='stylesheet' href='/assets/app.css'>
      <link rel="icon" href="/favicon.svg">
      <script type="module" src="/assets/index.js"></script>
    `),
    [
      { kind: "script", url: "/assets/index.js" },
      { kind: "modulepreload", url: "/assets/shared.js" },
      { kind: "stylesheet", url: "/assets/app.css" },
    ],
  );
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
      css: ["assets/app.css"],
      assets: ["assets/app.woff2"],
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
