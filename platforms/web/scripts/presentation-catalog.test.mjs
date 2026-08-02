import assert from "node:assert/strict";
import test from "node:test";

import {
  BUNDLED_THEME_PRESETS,
  SUPPORTED_HOST_THEME_PRESETS,
} from "../dist/public-catalog.js";
import * as coreRuntime from "../dist/runtime-core.js";
import { supportedHostThemePresets } from "../dist/runtime-render.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

function presentationCatalogFixture({ themePresets, profiles } = {}) {
  return {
    schema_version: 1,
    theme_presets: themePresets ?? [
      {
        id: "editor-light",
        appearance: "light",
        fully_available: true,
        missing_capability_ids: [],
      },
      {
        id: "future-theme",
        appearance: "adaptive",
        fully_available: true,
        missing_capability_ids: [],
      },
    ],
    profiles: profiles ?? [
      {
        id: "future-profile",
        fully_available: false,
        missing_capability_ids: ["future-capability"],
        aspects: [
          {
            id: "future-aspect",
            applicability: { kind: "future-scope", family_id: null },
            required_capability_id: "future-capability",
            available: false,
            missing_capability_ids: ["future-capability"],
          },
        ],
      },
    ],
  };
}

function presentationRuntime(loader) {
  return bindSurfaceRuntime(loader, {
    initMerman: coreRuntime.initMerman,
    presentationCatalog: coreRuntime.presentationCatalog,
  });
}

test("presentation catalog accepts future IDs, caches per surface, and returns defensive copies", async () => {
  let fullCalls = 0;
  let analysisCalls = 0;
  const full = presentationRuntime(async () => ({
    default: async () => {},
    presentationCatalog() {
      fullCalls += 1;
      return presentationCatalogFixture();
    },
  }));
  const analysis = presentationRuntime(async () => ({
    default: async () => {},
    presentationCatalog() {
      analysisCalls += 1;
      return presentationCatalogFixture({ themePresets: [], profiles: [] });
    },
  }));

  await full.initMerman();
  await analysis.initMerman();

  const firstFull = full.presentationCatalog();
  assert.equal(firstFull.theme_presets[1].id, "future-theme");
  assert.equal(firstFull.profiles[0].id, "future-profile");
  firstFull.theme_presets[0].id = "mutated-by-caller";

  assert.equal(analysis.presentationCatalog().theme_presets.length, 0);
  assert.equal(full.presentationCatalog().theme_presets[0].id, "editor-light");
  assert.equal(fullCalls, 1);
  assert.equal(analysisCalls, 1);
});

test("legacy host-theme discovery is a bundled compatibility view over the runtime catalog", async () => {
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      presentationCatalog: () => presentationCatalogFixture(),
    }),
    {
      initMerman: coreRuntime.initMerman,
      presentationCatalog: coreRuntime.presentationCatalog,
      supportedHostThemePresets,
    },
  );
  await runtime.initMerman();

  assert.deepEqual(runtime.supportedHostThemePresets(), ["editor-light"]);
  assert.deepEqual(SUPPORTED_HOST_THEME_PRESETS, BUNDLED_THEME_PRESETS);
  assert.deepEqual(BUNDLED_THEME_PRESETS, [
    "editor-light",
    "editor-dark",
    "one-dark",
    "gruvbox-light",
    "gruvbox-dark",
    "ayu-light",
    "ayu-dark",
  ]);
});

test("presentation catalog rejects unsupported schemas", async () => {
  const runtime = presentationRuntime(async () => ({
    default: async () => {},
    presentationCatalog: () => ({ schema_version: 2, theme_presets: [], profiles: [] }),
  }));
  await runtime.initMerman();

  assert.throws(
    () => runtime.presentationCatalog(),
    /unsupported presentation catalog schema/,
  );
});
