import assert from "node:assert/strict";
import test from "node:test";

import { initMerman, themeCatalog, WEB_TRANSPORT_API_VERSION } from "../dist/index.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

const classicCatalog = {
  schema_version: 2,
  presets: [],
  mermaid_themes: ["default", "dark", "neo", "neo-dark"],
  families: [],
};

function nativeCatalog() {
  return {
    ...structuredClone(classicCatalog),
    presets: [{
      id: "studio",
      name: "Studio",
      description: "Editor-inspired theme.",
      category: "editor",
      appearances: ["light", "dark"],
      default_appearance: "light",
      swatches: ["#FFFFFF", "#EDEFF4", "#F8FAFC", "#0F172A", "#334155", "#BFDBFE"],
    }],
    families: [{ id: "flowchart", capabilities: 16419 }],
  };
}

async function runtimeReturning(catalog, onRead = () => {}) {
  const runtime = bindSurfaceRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => WEB_TRANSPORT_API_VERSION,
    themeCatalog() { onRead(); return catalog; },
  }), { initMerman, themeCatalog });
  await runtime.initMerman();
  return runtime;
}

test("theme catalogs preserve the shared payload and cache it per artifact", async () => {
  let reads = 0;
  const catalog = nativeCatalog();
  const expected = structuredClone(catalog);
  const full = await runtimeReturning(catalog, () => { reads += 1; });
  const analysis = await runtimeReturning(classicCatalog);
  const first = full.themeCatalog();
  assert.deepEqual(first, expected);
  first.presets[0].swatches.reverse();
  first.mermaid_themes.pop();
  first.families[0].capabilities = 0;
  catalog.presets[0].appearances.pop();
  assert.deepEqual(full.themeCatalog(), expected);
  assert.deepEqual(analysis.themeCatalog(), classicCatalog);
  assert.equal(reads, 1);
});

test("the catalog facade requires schema 2", async () => {
  for (const catalog of [null, { ...nativeCatalog(), schema_version: 1 }]) {
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /unsupported theme catalog schema/);
  }
});
