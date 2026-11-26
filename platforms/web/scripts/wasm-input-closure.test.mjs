import assert from "node:assert/strict";
import { describe, it } from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  cargoMetadataForPreset,
  cargoRepositoryMetadata,
  collectWasmInputEntries,
} from "./wasm-build/input-manifest.mjs";
import { wasmArtifactProfile } from "./wasm-build/build.mjs";
import { webPackageDescriptors } from "./wasm-build/web-surface-descriptor.mjs";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

const parserClusters = [
  "merman-core-flow",
  "merman-core-graph",
  "merman-core-preprocess",
  "merman-core-sequence",
  "merman-core-structured",
];

const rendererClusters = [
  "merman-render-charts",
  "merman-render-core",
  "merman-render-flow",
  "merman-render-graph",
  "merman-render-sequence",
  "merman-render-structured",
];

const expectedCrateDirectories = {
  analysis: [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    ...parserClusters,
    "merman-markup",
    "merman-model",
    "merman-wasm",
  ],
  render: [
    "dugong",
    "dugong-graphlib",
    "manatee",
    "merman",
    "merman-bindings-core",
    "merman-core",
    ...parserClusters,
    "merman-elk-layered",
    "merman-layout-elk",
    "merman-markup",
    "merman-model",
    "merman-render",
    ...rendererClusters,
    "merman-theme-contract",
    "merman-unicode-linebreak",
    "merman-wasm",
    "roughr",
  ],
  ascii: [
    "dugong",
    "dugong-graphlib",
    "merman",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    ...parserClusters,
    "merman-markup",
    "merman-model",
    "merman-theme-contract",
    "merman-wasm",
  ],
  editor: [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    ...parserClusters,
    "merman-editor-core",
    "merman-markup",
    "merman-model",
    "merman-wasm",
  ],
  full: [
    "dugong",
    "dugong-graphlib",
    "manatee",
    "merman",
    "merman-analysis",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    ...parserClusters,
    "merman-editor-core",
    "merman-elk-layered",
    "merman-layout-elk",
    "merman-markup",
    "merman-model",
    "merman-render",
    ...rendererClusters,
    "merman-theme-contract",
    "merman-unicode-linebreak",
    "merman-wasm",
    "roughr",
  ],
};

describe("WASM artifact Cargo closure", () => {
  let repositoryMetadata;
  const inputsByProduct = new Map();
  function inputsFor(descriptor) {
    if (inputsByProduct.has(descriptor.id)) return inputsByProduct.get(descriptor.id);
    repositoryMetadata ??= cargoRepositoryMetadata({ repoRoot: repositoryRoot });
    const profile = wasmArtifactProfile(descriptor);
    const metadata = cargoMetadataForPreset({
      preset: profile,
      repoRoot: repositoryRoot,
      repositoryMetadata,
    });
    const entries = collectWasmInputEntries({ metadata, repoRoot: repositoryRoot });
    const inputs = { profile, metadata, paths: new Set(entries.map((entry) => entry.path)) };
    inputsByProduct.set(descriptor.id, inputs);
    return inputs;
  }

  for (const descriptor of webPackageDescriptors) {
    it(`${descriptor.artifact_profile.id} owns exactly its selected local normal/build closure`, () => {
      const { profile, metadata, paths } = inputsFor(descriptor);
      const actual = [...paths]
        .map((entry) => /^crates\/([^/]+)\/Cargo\.toml$/.exec(entry)?.[1])
        .filter(Boolean)
        .sort();
      assert.deepEqual(actual, expectedCrateDirectories[descriptor.id]);
      const expectsMath = descriptor.id === "full" || descriptor.id === "render";
      const selected = new Set(metadata.wasm_input_package_ids);
      assert.equal(profile.features.includes("math"), expectsMath);
      assert.equal(profile.runtime_capability_ids.includes("math"), expectsMath);
      assert.equal(
        metadata.packages.some((item) => item.name === "ratex-svg" && selected.has(item.id)),
        expectsMath,
      );
      for (const relative of [
        "crates/merman-model/src/families/gantt.rs",
        "crates/merman-markup/src/generated/dompurify_defaults.rs",
        "crates/merman-markup/src/url.rs",
      ]) {
        assert.equal(paths.has(relative), true, `${descriptor.id}: ${relative}`);
      }
    });
  }

  it("owns every renderer asset embedded into the SVG build", () => {
    const descriptor = webPackageDescriptors.find((item) => item.id === "render");
    const { paths } = inputsFor(descriptor);
    assert.equal(
      paths.has("crates/merman-render-sequence/src/svg/sequence_base_defs_11_16_0.svgfrag"),
      true,
    );
    assert.equal(
      paths.has("crates/merman-render/src/svg/parity/c4/c4_database_d_11_16_0.txt"),
      true,
    );
    assert.equal(
      paths.has("crates/merman-render/assets/katex_flowchart_probe.cjs"),
      false,
      "runtime-only Node audit assets must not invalidate browser WASM",
    );
    assert.equal(
      paths.has("crates/merman-bindings-core/src/generated/capability_surface.rs"),
      true,
      "the packaged bindings runtime owns its generated capability projection",
    );
  });
});
