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

function applicationNote(overrides = {}) {
  return {
    id: "future-guidance",
    family_id: "future-family",
    message: "Review these settings before applying them.",
    recommended_config: null,
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
    schema_version: 1,
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
        export_kind: "design",
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
  const runtime = await runtimeReturning(themeCatalogFixture({ schema_version: 2 }));

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


test("family design scope is open, optional, and independent of qualification", async () => {
  const catalog = themeCatalogFixture();
  catalog.presets[0].family_designs = [
    { family_id: "class", treatment: "base_only" },
    { family_id: "future-family", treatment: "future-design" },
  ];
  const runtime = await runtimeReturning(catalog);
  const first = runtime.themeCatalog();
  assert.deepEqual(first.presets[0].family_designs, catalog.presets[0].family_designs);
  assert.deepEqual(first.presets[0].qualified_cells, []);
  assert.deepEqual(first.presets[1].family_designs, []);
  first.presets[0].family_designs[0].treatment = "mutated";
  assert.equal(runtime.themeCatalog().presets[0].family_designs[0].treatment, "base_only");
});

test("family design metadata rejects ambiguous and malformed declarations", async () => {
  for (const designs of [
    null, {}, [{ family_id: "class" }], [{ family_id: "", treatment: "base_only" }],
    [{ family_id: "class", treatment: "" }],
    [{ family_id: "class", treatment: "base_only" }, { family_id: "class", treatment: "dedicated" }],
    [{ family_id: "xychart", treatment: "base_only" }, { family_id: "class", treatment: "base_only" }],
  ]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].family_designs = designs;
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /family design/, JSON.stringify(designs));
  }
});

test("preset discovery projects the shared catalog without granting qualification", async () => {
  const presets = JSON.parse(readFileSync(new URL(
    "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json",
    import.meta.url,
  ), "utf8"));
  const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
  assert.deepEqual(runtime.themeCatalog().presets, presets);
  assert.ok(presets.every(({ maturity, qualified_cells }) =>
    maturity === "alpha" && qualified_cells.length === 0));
  const ghibli = runtime.themeCatalog().presets.find(({ id }) => id === "ghibli");
  assert.ok(ghibli, "the shared inventory must include Ghibli");
  assert.equal(ghibli.display_name, "Ghibli");
  assert.deepEqual(ghibli.family_designs, [{ family_id: "pie", treatment: "dedicated" }]);
});

test("legacy discovery defaults preview colors and application notes to independent empty arrays", async () => {
  const runtime = await runtimeReturning(themeCatalogFixture());
  const first = runtime.themeCatalog();
  for (const preset of first.presets) {
    assert.deepEqual(preset.preview_colors, []);
    assert.deepEqual(preset.application_notes, []);
  }
  first.presets[0].preview_colors.push("#123456");
  first.presets[0].application_notes.push(applicationNote());
  assert.deepEqual(first.presets[1].preview_colors, []);
  assert.deepEqual(first.presets[1].application_notes, []);
  assert.deepEqual(runtime.themeCatalog().presets[0].preview_colors, []);
  assert.deepEqual(runtime.themeCatalog().presets[0].application_notes, []);

  const catalog = themeCatalogFixture();
  catalog.presets[0].preview_colors = [];
  catalog.presets[0].application_notes = [];
  const empty = await runtimeReturning(catalog);
  assert.deepEqual(empty.themeCatalog().presets[0].preview_colors, []);
  assert.deepEqual(empty.themeCatalog().presets[0].application_notes, []);
});

test("discovery preserves curated order, future IDs, and nested recommendations with defensive copies", async () => {
  const catalog = themeCatalogFixture();
  const preset = catalog.presets[0];
  preset.preview_colors = ["#fedcba", "#123456", "#fedcba"];
  preset.application_notes = [
    applicationNote({
      id: "z-future-note",
      recommended_config: {
        look: "classic",
        future: { choices: ["one", { enabled: true, weight: 1.5, fallback: null }] },
      },
      future_field: "ignored",
    }),
    applicationNote({ id: "a-future-note" }),
  ];
  const expectedNotes = structuredClone(preset.application_notes);
  delete expectedNotes[0].future_field;
  const expectedColors = [...preset.preview_colors];
  const runtime = await runtimeReturning(catalog);
  const first = runtime.themeCatalog().presets[0];
  assert.deepEqual(first.preview_colors, expectedColors);
  assert.deepEqual(first.application_notes, expectedNotes);
  assert.equal(first.available, true);
  assert.deepEqual(first.family_designs, []);
  assert.deepEqual(first.qualified_cells, []);

  first.preview_colors.reverse();
  first.preview_colors[0] = "changed";
  first.application_notes[0].message = "changed";
  first.application_notes[0].recommended_config.future.choices[1].enabled = false;
  preset.preview_colors.length = 0;
  preset.application_notes[0].recommended_config.future.choices[1].weight = 9;
  preset.application_notes.length = 0;
  assert.deepEqual(runtime.themeCatalog().presets[0].preview_colors, expectedColors);
  assert.deepEqual(runtime.themeCatalog().presets[0].application_notes, expectedNotes);
});

test("application notes allow an omitted recommendation without inventing configuration", async () => {
  const catalog = themeCatalogFixture();
  const note = applicationNote();
  delete note.recommended_config;
  catalog.presets[0].application_notes = [note, applicationNote({ recommended_config: {} })];
  const runtime = await runtimeReturning(catalog);
  assert.deepEqual(runtime.themeCatalog().presets[0].application_notes, [
    { ...note, recommended_config: null },
    applicationNote({ recommended_config: {} }),
  ]);
});

test("preview colors reject malformed present fields without parsing CSS color syntax", async () => {
  for (const colors of [null, {}, 1, "#123456", [null], [1], [{}], [[]], [""]]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].preview_colors = colors;
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /preview color/, JSON.stringify(colors));
  }
});

test("application notes reject malformed present arrays and required fields", async () => {
  for (const notes of [null, {}, 1, "note", [null], [1], [[]], [{}]]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].application_notes = notes;
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /application note/, JSON.stringify(notes));
  }
  for (const field of ["id", "family_id", "message"]) {
    const invalidValues = [undefined, null, 1, "", {}, []];
    if (field !== "message") invalidValues.push("Invalid Value", "future_id");
    for (const invalid of invalidValues) {
      const catalog = themeCatalogFixture();
      const note = applicationNote({ [field]: invalid });
      if (invalid === undefined) delete note[field];
      catalog.presets[0].application_notes = [note];
      const runtime = await runtimeReturning(catalog);
      assert.throws(() => runtime.themeCatalog(), /application note/, `${field}: ${String(invalid)}`);
    }
  }
});

test("recommendations require a JSON object or null and reject non-JSON nested values", async () => {
  for (const config of [
    [], ["classic"], "classic", 1, false, new Date(),
    { nested: undefined }, { nested: () => {} }, { nested: 1n },
    { nested: NaN }, { nested: Infinity }, { nested: new Map() },
    { nested: [undefined] },
  ]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].application_notes = [applicationNote({ recommended_config: config })];
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /recommended config/, String(config));
  }
});
