import assert from "node:assert/strict";
import { describe, it } from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  cargoMetadataForPreset,
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

const expectedCrateDirectories = {
  analysis: [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    "merman-wasm",
  ],
  render: [
    "dugong",
    "dugong-graphlib",
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    "merman-render",
    "merman-wasm",
    "roughr",
  ],
  ascii: [
    "merman",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    "merman-wasm",
  ],
  editor: [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    "merman-editor-core",
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
    "merman-editor-core",
    "merman-elk-layered",
    "merman-layout-elk",
    "merman-render",
    "merman-wasm",
    "roughr",
  ],
};

describe("WASM artifact Cargo closure", () => {
  for (const descriptor of webPackageDescriptors) {
    it(`${descriptor.artifact_profile.id} owns exactly its selected local normal/build closure`, () => {
      const profile = wasmArtifactProfile(descriptor);
      const metadata = cargoMetadataForPreset({ preset: profile, repoRoot: repositoryRoot });
      const entries = collectWasmInputEntries({ metadata, repoRoot: repositoryRoot });
      const actual = entries
        .map((entry) => /^crates\/([^/]+)\/Cargo\.toml$/.exec(entry.path)?.[1])
        .filter(Boolean)
        .sort();
      assert.deepEqual(actual, expectedCrateDirectories[descriptor.id]);
    });
  }

  it("keeps unadmitted math out of all browser artifact recipes", () => {
    for (const descriptor of webPackageDescriptors) {
      const profile = wasmArtifactProfile(descriptor);
      assert.equal(profile.features.includes("math"), false, descriptor.id);
      assert.equal(profile.runtime_capability_ids.includes("math"), false, descriptor.id);
    }
  });

  it("owns every renderer asset embedded into the candidate SVG build", () => {
    const descriptor = webPackageDescriptors.find((item) => item.id === "render");
    const metadata = cargoMetadataForPreset({
      preset: wasmArtifactProfile(descriptor),
      repoRoot: repositoryRoot,
    });
    const paths = new Set(
      collectWasmInputEntries({ metadata, repoRoot: repositoryRoot }).map(
        (entry) => entry.path,
      ),
    );
    assert.equal(
      paths.has("crates/merman-render/src/svg/parity/sequence/sequence_base_defs_11_16_0.svgfrag"),
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
  });
});
