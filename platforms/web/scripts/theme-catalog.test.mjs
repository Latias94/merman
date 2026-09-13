import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import * as coreRuntime from "../dist/runtime-core.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

function qualifiedCell(overrides = {}) {
  return {
    family_id: "flowchart", output_id: "svg",
    profile_id: "host-fonts-v1", admission_status: "host_dependent",
    ...overrides,
  };
}

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
    schema_version: 3,
    structured_spec_available: true,
    supported_output_ids: ["future-output", "svg"],
    presets: [
      {
        id: "editor-light",
        display_name: "Editor Light",
        appearance: "light",
        maturity: "alpha",
        available: true,
        availability_reason_ids: [],
        qualified_cells: [],
        license_expression: "MIT OR Apache-2.0",
        required_attribution: null,
        export_kind: "definition",
      },
      {
        id: "future-theme",
        display_name: "Future Theme",
        appearance: "adaptive",
        maturity: "experimental",
        available: false,
        availability_reason_ids: ["future-resource-unavailable"],
        qualified_cells: [qualifiedCell()],
        license_expression: "LicenseRef-Future",
        required_attribution: "Future Theme authors",
        export_kind: "complete_spec",
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
  assert.equal(firstFull.presets[0].maturity, "alpha");
  assert.equal(firstFull.presets[1].maturity, "experimental");
  assert.equal(firstFull.presets[0].available, true);
  assert.equal(firstFull.presets[1].available, false);
  assert.deepEqual(firstFull.presets[1].qualified_cells, [
    qualifiedCell(),
  ]);
  assert.equal(firstFull.known_semantic_target_ids[0], "future-target");
  assert.equal(firstFull.resource_limits[0].hard_cap, true);
  firstFull.presets[0].display_name = "mutated-by-caller";
  firstFull.presets[1].qualified_cells[0].family_id = "mutated-by-caller";
  firstFull.resource_limits[0].description = "mutated-by-caller";

  assert.equal(analysis.themeCatalog().presets.length, 0);
  assert.equal(full.themeCatalog().presets[0].display_name, "Editor Light");
  assert.equal(full.themeCatalog().presets[1].qualified_cells[0].family_id, "flowchart");
  assert.notEqual(full.themeCatalog().resource_limits[0].description, "mutated-by-caller");
  assert.equal(fullCalls, 1);
  assert.equal(analysisCalls, 1);
});

test("theme catalog rejects missing or malformed preset maturity", async () => {
  const invalidMaturities = [
    { label: "missing", mutate: (preset) => delete preset.maturity },
    { label: "null", mutate: (preset) => { preset.maturity = null; } },
    { label: "numeric", mutate: (preset) => { preset.maturity = 1; } },
    { label: "empty", mutate: (preset) => { preset.maturity = ""; } },
    { label: "malformed", mutate: (preset) => { preset.maturity = "Alpha Candidate"; } },
  ];

  for (const { label, mutate } of invalidMaturities) {
    const presets = themeCatalogFixture().presets;
    mutate(presets[0]);
    const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
    assert.throws(() => runtime.themeCatalog(), /maturity|required fields/, label);
  }
});

test("theme catalog validates preset availability, export kind, and qualified cells", async () => {
  for (const [label, mutate, expected] of [
    [
      "available with reasons",
      (preset) => { preset.availability_reason_ids = ["unexpected-reason"]; },
      /availability/,
    ],
    [
      "unavailable without reasons",
      (preset) => { preset.available = false; },
      /availability/,
    ],
    [
      "unknown export kind",
      (preset) => { preset.export_kind = "future-export"; },
      /export kind/,
    ],
    [
      "duplicate qualified cell",
      (preset) => {
        preset.qualified_cells = [
          qualifiedCell(),
          qualifiedCell(),
        ];
      },
      /duplicate qualified cells/,
    ],
    [
      "unsorted qualified cells",
      (preset) => {
        preset.qualified_cells = [
          qualifiedCell({ family_id: "state" }),
          qualifiedCell(),
        ];
      },
      /qualified cells must be sorted/,
    ],
  ]) {
    const presets = themeCatalogFixture().presets;
    mutate(presets[0]);
    const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
    assert.throws(() => runtime.themeCatalog(), expected, label);
  }
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

test("qualified cells round-trip the shared open-ID golden with defensive copies", async () => {
  const vectors = JSON.parse(readFileSync(new URL(
    "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/qualified-cells.json",
    import.meta.url,
  ), "utf8"));
  for (const { id, cell } of vectors) {
    const presets = themeCatalogFixture().presets;
    presets[0].qualified_cells = [cell];
    const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
    assert.deepEqual(runtime.themeCatalog().presets[0].qualified_cells, [cell], id);
    const copy = runtime.themeCatalog();
    copy.presets[0].qualified_cells[0].profile_id = "changed";
    copy.presets[0].qualified_cells[0].admission_status = "changed";
    assert.deepEqual(runtime.themeCatalog().presets[0].qualified_cells, [cell], id);
  }
  const cells = ["future-profile-portable", "host-dependent"]
    .map((id) => vectors.find((vector) => vector.id === id).cell);
  const presets = themeCatalogFixture().presets;
  presets[0].qualified_cells = cells;
  const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
  assert.deepEqual(runtime.themeCatalog().presets[0].qualified_cells, cells);
  const copy = runtime.themeCatalog();
  copy.presets[0].qualified_cells[1].profile_id = "changed";
  copy.presets[0].qualified_cells[1].admission_status = "changed";
  assert.deepEqual(runtime.themeCatalog().presets[0].qualified_cells, cells);
});

test("qualified cells reject missing conditions and conflicting admissions in the same profile", async () => {
  for (const field of ["profile_id", "admission_status"]) {
    for (const invalid of [undefined, null, "", 1, "Invalid Value"]) {
      const presets = themeCatalogFixture().presets;
      const cell = qualifiedCell();
      if (invalid === undefined) delete cell[field];
      else cell[field] = invalid;
      presets[0].qualified_cells = [cell];
      const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
      assert.throws(() => runtime.themeCatalog(), /required fields|qualified profile|qualified admission/);
    }
  }
  const presets = themeCatalogFixture().presets;
  presets[0].qualified_cells = [qualifiedCell(), qualifiedCell({ admission_status: "portable" })];
  const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
  assert.throws(() => runtime.themeCatalog(), /duplicate qualified cells/);
});
