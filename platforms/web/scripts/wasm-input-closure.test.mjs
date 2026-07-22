import assert from "node:assert/strict";
import { describe, it } from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  cargoMetadataForPreset,
  collectWasmInputEntries,
} from "./wasm-build/input-manifest.mjs";
import { webPresetDescriptors } from "./wasm-build/web-surface-descriptor.mjs";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

const expectedCrateDirectories = {
  "browser-bridge": [
    "merman",
    "merman-bindings-core",
    "merman-core",
    "merman-wasm",
  ],
  "browser-core": [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    "merman-wasm",
  ],
  "browser-render": [
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
  "browser-render-only": [
    "dugong",
    "dugong-graphlib",
    "merman",
    "merman-bindings-core",
    "merman-core",
    "merman-render",
    "merman-wasm",
    "roughr",
  ],
  "browser-ascii": [
    "merman",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    "merman-wasm",
  ],
  "browser-editor": [
    "merman",
    "merman-analysis",
    "merman-bindings-core",
    "merman-core",
    "merman-editor-core",
    "merman-wasm",
  ],
  "browser-full": [
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
  "browser-full-no-elk": [
    "dugong",
    "dugong-graphlib",
    "manatee",
    "merman",
    "merman-analysis",
    "merman-ascii",
    "merman-bindings-core",
    "merman-core",
    "merman-editor-core",
    "merman-render",
    "merman-wasm",
    "roughr",
  ],
  "browser-ratex-math": [
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

describe("WASM preset Cargo closure", () => {
  for (const preset of webPresetDescriptors) {
    it(`${preset.name} owns exactly its selected local normal/build closure`, () => {
      const metadata = cargoMetadataForPreset({ preset, repoRoot: repositoryRoot });
      const entries = collectWasmInputEntries({ metadata, repoRoot: repositoryRoot });
      const actual = entries
        .map((entry) => /^crates\/([^/]+)\/Cargo\.toml$/.exec(entry.path)?.[1])
        .filter(Boolean)
        .sort();
      assert.deepEqual(actual, expectedCrateDirectories[preset.name]);
    });
  }

  it("keeps ELK out of the explicit no-ELK surface", () => {
    assert.equal(
      expectedCrateDirectories["browser-full-no-elk"].some((name) =>
        name.includes("elk"),
      ),
      false,
    );
    assert.deepEqual(
      expectedCrateDirectories["browser-full"].filter((name) => name.includes("elk")),
      ["merman-elk-layered", "merman-layout-elk"],
    );
  });

  it("owns every renderer asset embedded into the WASM build", () => {
    const preset = webPresetDescriptors.find((item) => item.name === "browser-render");
    const metadata = cargoMetadataForPreset({ preset, repoRoot: repositoryRoot });
    const paths = new Set(
      collectWasmInputEntries({ metadata, repoRoot: repositoryRoot }).map(
        (entry) => entry.path,
      ),
    );
    assert.equal(
      paths.has(
        "crates/merman-render/src/svg/parity/sequence/sequence_base_defs_11_16_0.svgfrag",
      ),
      true,
    );
    assert.equal(
      paths.has(
        "crates/merman-render/src/svg/parity/c4/c4_database_d_11_16_0.txt",
      ),
      true,
    );
    assert.equal(
      paths.has("crates/merman-render/assets/katex_flowchart_probe.cjs"),
      false,
      "runtime-only Node audit assets must not invalidate browser WASM",
    );
  });
});
