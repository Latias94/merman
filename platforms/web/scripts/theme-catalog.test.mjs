import assert from "node:assert/strict";
import test from "node:test";

import * as coreRuntime from "../dist/runtime-core.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

function themeResourceLimit(overrides = {}) {
  return {
    id: "max_theme_encoded_bytes",
    phase: "theme_input",
    description: "Maximum encoded theme input bytes.",
    effective_value: 2_097_152,
    hard_cap: false,
    ...overrides,
  };
}

function themeCatalogFixture(overrides = {}) {
  return {
    schema_version: 2,
    structured_spec_available: true,
    supported_output_ids: ["future-output", "svg"],
    presets: [
      {
        id: "editor-light",
        appearance: "light",
        required_capability_ids: ["semantic-rules"],
        required_text_capability_ids: [],
      },
      {
        id: "future-theme",
        appearance: "adaptive",
        required_capability_ids: ["future-capability", "semantic-rules"],
        required_text_capability_ids: ["opentype-shaping"],
      },
    ],
    known_capability_ids: ["future-capability", "semantic-rules"],
    known_text_capability_ids: ["opentype-shaping"],
    known_font_container_ids: ["woff2"],
    known_font_source_ids: ["embedded"],
    known_semantic_target_ids: ["future-target", "node"],
    known_variant_ids: ["default", "future-variant"],
    resource_limits: [
      themeResourceLimit({
        id: "font_assets_hard_cap",
        phase: "font_catalog",
        description: "Maximum font assets accepted by the implementation.",
        effective_value: 256,
        hard_cap: true,
      }),
      themeResourceLimit(),
    ],
    ...overrides,
  };
}

function themeRuntime(loader) {
  return bindSurfaceRuntime(loader, {
    initMerman: coreRuntime.initMerman,
    themeCatalog: coreRuntime.themeCatalog,
  });
}

async function runtimeReturning(catalog) {
  const runtime = themeRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => coreRuntime.WEB_TRANSPORT_API_VERSION,
    themeCatalog: () => catalog,
  }));
  await runtime.initMerman();
  return runtime;
}

test("theme catalog accepts future IDs, caches per surface, and returns defensive copies", async () => {
  let fullCalls = 0;
  let analysisCalls = 0;
  const full = themeRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => coreRuntime.WEB_TRANSPORT_API_VERSION,
    themeCatalog() {
      fullCalls += 1;
      return themeCatalogFixture();
    },
  }));
  const analysis = themeRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => coreRuntime.WEB_TRANSPORT_API_VERSION,
    themeCatalog() {
      analysisCalls += 1;
      return themeCatalogFixture({
        structured_spec_available: false,
        supported_output_ids: [],
        presets: [],
        known_capability_ids: [],
        known_text_capability_ids: [],
        known_font_container_ids: [],
        known_font_source_ids: [],
        known_semantic_target_ids: [],
        known_variant_ids: [],
        resource_limits: [],
      });
    },
  }));

  await full.initMerman();
  await analysis.initMerman();

  const firstFull = full.themeCatalog();
  assert.equal(firstFull.presets[1].id, "future-theme");
  assert.equal(firstFull.known_semantic_target_ids[0], "future-target");
  assert.equal(firstFull.resource_limits[0].hard_cap, true);
  firstFull.presets[0].required_capability_ids[0] = "mutated-by-caller";
  firstFull.resource_limits[0].description = "mutated-by-caller";

  assert.equal(analysis.themeCatalog().presets.length, 0);
  assert.deepEqual(full.themeCatalog().presets[0].required_capability_ids, [
    "semantic-rules",
  ]);
  assert.notEqual(full.themeCatalog().resource_limits[0].description, "mutated-by-caller");
  assert.equal(fullCalls, 1);
  assert.equal(analysisCalls, 1);
});

test("theme catalog rejects unsupported schemas", async () => {
  const runtime = await runtimeReturning(themeCatalogFixture({ schema_version: 1 }));

  assert.throws(
    () => runtime.themeCatalog(),
    /unsupported theme catalog schema/,
  );
});

test("theme catalog requires every ID array to be sorted and unique", async () => {
  const catalogIdArrays = [
    "supported_output_ids",
    "known_capability_ids",
    "known_text_capability_ids",
    "known_font_container_ids",
    "known_font_source_ids",
    "known_semantic_target_ids",
    "known_variant_ids",
  ];
  for (const field of catalogIdArrays) {
    const runtime = await runtimeReturning(themeCatalogFixture({ [field]: ["z", "a"] }));
    assert.throws(() => runtime.themeCatalog(), /must be sorted and unique/, field);
  }

  for (const field of ["required_capability_ids", "required_text_capability_ids"]) {
    const presets = themeCatalogFixture().presets;
    presets[0] = { ...presets[0], [field]: ["duplicate", "duplicate"] };
    const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
    assert.throws(() => runtime.themeCatalog(), /must be sorted and unique/, field);
  }
});

test("theme catalog validates sorted resource limits and their values", async () => {
  const unsorted = await runtimeReturning(themeCatalogFixture({
    resource_limits: [
      themeResourceLimit({ id: "z-limit" }),
      themeResourceLimit({ id: "a-limit" }),
    ],
  }));
  assert.throws(() => unsorted.themeCatalog(), /resource limit IDs must be sorted and unique/);

  const duplicate = await runtimeReturning(themeCatalogFixture({
    resource_limits: [
      themeResourceLimit({ id: "a-limit" }),
      themeResourceLimit({ id: "a-limit" }),
    ],
  }));
  assert.throws(() => duplicate.themeCatalog(), /resource limit IDs must be sorted and unique/);

  const unsafeInteger = await runtimeReturning(themeCatalogFixture({
    resource_limits: [themeResourceLimit({ effective_value: Number.MAX_SAFE_INTEGER + 1 })],
  }));
  assert.throws(() => unsafeInteger.themeCatalog(), /invalid theme resource limit .* effective value/);

  const negativeValue = await runtimeReturning(themeCatalogFixture({
    resource_limits: [themeResourceLimit({ effective_value: -1 })],
  }));
  assert.throws(() => negativeValue.themeCatalog(), /invalid theme resource limit .* effective value/);
});
