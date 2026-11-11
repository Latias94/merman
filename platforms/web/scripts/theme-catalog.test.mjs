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

function appearance(mode, overrides = {}) {
  return {
    appearance: mode, concrete_preset_id: `editor-${mode}`,
    available: true, availability_reason_ids: [],
    family_designs: [], preview_colors: [], application_notes: [], qualified_cells: [],
    ...overrides,
  };
}

function themeCatalogFixture(overrides = {}) {
  return {
    schema_version: 1,
    structured_spec_available: true,
    recipe_schema_version: 1,
    authoring_request_schema_version: 1,
    authoring_result_schema_version: 1,
    supported_output_ids: ["future-output", "svg"],
    presets: [{
      id: "editor", display_name: "Editor", maturity: "alpha",
      supported_appearances: ["light", "dark"], default_appearance: "light",
      appearances: [appearance("light"), appearance("dark")],
      available: true, availability_reason_ids: [],
      license_expression: "MIT OR Apache-2.0", required_attribution: null, export_kind: "design",
    }],
    mermaid_theme_designs: [
      { id: "default", display_name: "Default", default_appearance: "light",
        appearances: [{ appearance: "light", theme_id: "default" }] },
      { id: "dark", display_name: "Dark", default_appearance: "dark",
        appearances: [{ appearance: "dark", theme_id: "dark" }] },
      { id: "neo", display_name: "Neo", default_appearance: "light",
        appearances: [
          { appearance: "light", theme_id: "neo" },
          { appearance: "dark", theme_id: "neo-dark" },
        ] },
    ],
    known_capability_ids: ["future-capability", "semantic-rules"],
    known_text_capability_ids: ["opentype-shaping"],
    known_font_container_ids: ["woff2"],
    known_font_source_ids: ["embedded"],
    known_semantic_target_ids: ["future-target", "node"],
    known_variant_ids: ["default", "future-variant"],
    resource_limits: [{
      id: "max_theme_encoded_bytes", phase: "theme_input",
      description: "Maximum encoded theme input bytes.", effective_value: 2_097_152, hard_cap: false,
    }],
    ...overrides,
  };
}

function emptyCatalog() {
  return themeCatalogFixture({
    structured_spec_available: false,
    recipe_schema_version: null, authoring_request_schema_version: null,
    authoring_result_schema_version: null,
    supported_output_ids: [], presets: [], mermaid_theme_designs: [],
    known_capability_ids: [], known_text_capability_ids: [],
    known_font_container_ids: [], known_font_source_ids: [],
    known_semantic_target_ids: [], known_variant_ids: [], resource_limits: [],
  });
}

async function runtimeReturning(catalog, onRead = () => {}) {
  const runtime = bindSurfaceRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => coreRuntime.WEB_TRANSPORT_API_VERSION,
    themeCatalog() { onRead(); return catalog; },
  }), { initMerman: coreRuntime.initMerman, themeCatalog: coreRuntime.themeCatalog });
  await runtime.initMerman();
  return runtime;
}

test("grouped discovery preserves appearance identity, caches per surface, and copies nested data", async () => {
  let calls = 0;
  const catalog = themeCatalogFixture();
  catalog.future_field = { enabled: true };
  catalog.presets[0].future_field = { scope: ["shared"] };
  catalog.presets[0].appearances[1].future_field = { scope: ["dark"] };
  catalog.presets[0].appearances[1].qualified_cells = [qualifiedCell()];
  const full = await runtimeReturning(catalog, () => { calls += 1; });
  const analysis = await runtimeReturning(emptyCatalog());
  const first = full.themeCatalog();
  assert.equal(first.schema_version, 1);
  assert.equal(first.recipe_schema_version, 1);
  assert.deepEqual(first.presets[0].supported_appearances, ["light", "dark"]);
  assert.equal(first.presets[0].appearances[1].concrete_preset_id, "editor-dark");
  assert.equal(first.presets[0].appearances[1].qualified_cells[0].profile_id, "host-fonts-v1");
  first.presets[0].appearances[1].future_field.scope.push("changed");
  first.presets[0].supported_appearances.reverse();
  first.future_field.enabled = false;
  assert.deepEqual(full.themeCatalog(), catalog);
  assert.deepEqual(analysis.themeCatalog(), emptyCatalog());
  assert.equal(calls, 1);
});

test("official themes use explicit upstream IDs and keep Default and Dark separate", async () => {
  const runtime = await runtimeReturning(themeCatalogFixture());
  const designs = runtime.themeCatalog().mermaid_theme_designs;
  assert.equal(designs[0].id, "default");
  assert.deepEqual(designs[0].appearances, [{ appearance: "light", theme_id: "default" }]);
  assert.equal(designs[1].id, "dark");
  assert.equal(designs[2].appearances[1].theme_id, "neo-dark");
});

test("complete-document rejection applies to every appearance without granting qualification", async () => {
  const catalog = themeCatalogFixture();
  const preset = catalog.presets[0];
  preset.available = false;
  preset.availability_reason_ids = ["theme-preset.resource-policy-rejected"];
  for (const mode of preset.appearances) {
    mode.available = false;
    mode.availability_reason_ids = [...preset.availability_reason_ids];
  }
  const runtime = await runtimeReturning(catalog);
  assert.deepEqual(runtime.themeCatalog(), catalog);
});

test("grouped discovery accepts the canonical producer fixture without flattening its appearances", async () => {
  const presets = JSON.parse(readFileSync(new URL(
    "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json",
    import.meta.url,
  ), "utf8"));
  const runtime = await runtimeReturning(themeCatalogFixture({ presets }));
  const actual = runtime.themeCatalog().presets;
  assert.deepEqual(actual, presets);
  const editor = actual.find(({ id }) => id === "editor");
  assert.deepEqual(editor.supported_appearances, ["light", "dark"]);
  assert.equal(editor.appearances[1].concrete_preset_id, "editor-dark");
  const ghibli = actual.find(({ id }) => id === "ghibli");
  assert.deepEqual(ghibli.appearances[0].family_designs, [
    { family_id: "class", treatment: "base_only" },
    { family_id: "flowchart", treatment: "dedicated" },
    { family_id: "pie", treatment: "dedicated" },
    { family_id: "sequence", treatment: "dedicated" },
    { family_id: "xychart", treatment: "dedicated" },
  ]);
  assert.ok(actual.every(({ maturity, appearances }) => maturity === "alpha" &&
    appearances.every(({ qualified_cells }) => qualified_cells.length === 0)));
});

test("discovery rejects inconsistent appearance sets, defaults, concrete IDs, and availability", async () => {
  for (const mutate of [
    (preset) => { preset.supported_appearances = []; },
    (preset) => { preset.supported_appearances = ["light", "light"]; },
    (preset) => { preset.supported_appearances = ["dark", "light"]; },
    (preset) => { preset.default_appearance = "auto"; },
    (preset) => { preset.supported_appearances = ["dark"]; preset.appearances.shift(); },
    (preset) => { preset.appearances[1].appearance = "light"; },
    (preset) => { preset.appearances.pop(); },
    (preset) => { preset.appearances[1].concrete_preset_id = "editor-light"; },
    (preset) => { delete preset.appearances[0].concrete_preset_id; },
    (preset) => { preset.appearances[1].available = false; },
    (preset) => { preset.appearances[1].availability_reason_ids = ["future-reason"]; },
    (preset) => { preset.available = false; },
    (preset) => { preset.availability_reason_ids = ["future-reason"]; },
  ]) {
    const catalog = themeCatalogFixture();
    mutate(catalog.presets[0]);
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /appearance|availability|concrete preset|required fields/);
  }
});

test("discovery rejects malformed schema, maturity, export kind, and duplicated group IDs", async () => {
  for (const schema_version of [0, 2, null, "1"]) {
    const runtime = await runtimeReturning(themeCatalogFixture({ schema_version }));
    assert.throws(() => runtime.themeCatalog(), /unsupported theme catalog schema/);
  }
  for (const [field, values] of [
    ["maturity", [undefined, null, 1, "", "Alpha Candidate"]],
    ["export_kind", [undefined, "complete_spec", "future-export"]],
  ]) {
    for (const value of values) {
      const catalog = themeCatalogFixture();
      catalog.presets[0][field] = value;
      const runtime = await runtimeReturning(catalog);
      assert.throws(() => runtime.themeCatalog(), /maturity|export kind/);
    }
  }
  const catalog = themeCatalogFixture();
  catalog.presets.push(structuredClone(catalog.presets[0]));
  const runtime = await runtimeReturning(catalog);
  assert.throws(() => runtime.themeCatalog(), /preset IDs must be unique/);
});

test("discovery rejects malformed Mermaid design relationships", async () => {
  for (const mutate of [
    (catalog) => { catalog.mermaid_theme_designs[2].default_appearance = "auto"; },
    (catalog) => { catalog.mermaid_theme_designs[2].default_appearance = "dark"; },
    (catalog) => { catalog.mermaid_theme_designs[2].appearances[1].appearance = "light"; },
    (catalog) => { catalog.mermaid_theme_designs[2].appearances[1].theme_id = "default"; },
    (catalog) => { catalog.mermaid_theme_designs[2].appearances = []; },
    (catalog) => { catalog.mermaid_theme_designs[2].appearances[1].theme_id = "editor-dark"; },
    (catalog) => { catalog.mermaid_theme_designs[2].id = "default"; },
  ]) {
    const catalog = themeCatalogFixture();
    mutate(catalog);
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /Mermaid/);
  }
});

test("catalog schema discovery reflects the artifact boundary without a second version table", async () => {
  for (const field of [
    "recipe_schema_version", "authoring_request_schema_version", "authoring_result_schema_version",
  ]) {
    for (const invalid of [null, 0, "1", 1.5, Number.MAX_SAFE_INTEGER + 1]) {
      const runtime = await runtimeReturning(themeCatalogFixture({ [field]: invalid }));
      assert.throws(() => runtime.themeCatalog(), /theme/);
    }
    const runtime = await runtimeReturning({ ...emptyCatalog(), [field]: 1 });
    assert.throws(() => runtime.themeCatalog(), /unavailable theme authoring/);
  }
});

test("catalog ID arrays and resource limits remain sorted, unique, and safely representable", async () => {
  for (const field of [
    "supported_output_ids", "known_capability_ids", "known_text_capability_ids",
    "known_font_container_ids", "known_font_source_ids", "known_semantic_target_ids", "known_variant_ids",
  ]) {
    const runtime = await runtimeReturning(themeCatalogFixture({ [field]: ["z", "a"] }));
    assert.throws(() => runtime.themeCatalog(), /must be sorted and unique/);
  }
  for (const limits of [
    [{ id: "z-limit" }, { id: "a-limit" }], [{ id: "a-limit" }, { id: "a-limit" }],
    [{ effective_value: -1 }], [{ effective_value: Number.MAX_SAFE_INTEGER + 1 }],
  ]) {
    const catalog = themeCatalogFixture();
    catalog.resource_limits = limits.map((value) => ({ ...catalog.resource_limits[0], ...value }));
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /resource limit/);
  }
});

test("qualification preserves shared open IDs without inferring appearance or visual scope", async () => {
  const vectors = JSON.parse(readFileSync(new URL(
    "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/qualified-cells.json",
    import.meta.url,
  ), "utf8"));
  for (const { id, cell } of vectors) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].appearances[0].qualified_cells = [cell];
    const runtime = await runtimeReturning(catalog);
    assert.deepEqual(runtime.themeCatalog().presets[0].appearances[0].qualified_cells, [cell], id);
    assert.deepEqual(runtime.themeCatalog().presets[0].appearances[0].family_designs, []);
  }
  for (const cells of [
    [qualifiedCell(), qualifiedCell({ admission_status: "portable" })],
    [qualifiedCell({ family_id: "state" }), qualifiedCell()],
    [{ ...qualifiedCell(), profile_id: null }], [{ ...qualifiedCell(), admission_status: "" }],
  ]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].appearances[0].qualified_cells = cells;
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /qualified/);
  }
});

test("curated colors and notes preserve order, independent modes, and nested recommendations", async () => {
  const catalog = themeCatalogFixture();
  const light = catalog.presets[0].appearances[0];
  light.preview_colors = ["#fedcba", "#123456", "#fedcba"];
  light.family_designs = [{ family_id: "future-family", treatment: "future_design" }];
  light.application_notes = [{
    id: "future-note", family_id: "future-family", message: "Review settings before applying.",
    recommended_config: { future: { choices: ["one", { enabled: true, fallback: null }] } },
  }];
  const expected = structuredClone(light);
  const runtime = await runtimeReturning(catalog);
  const first = runtime.themeCatalog();
  assert.deepEqual(first.presets[0].appearances[0], expected);
  assert.deepEqual(first.presets[0].appearances[1].application_notes, []);
  first.presets[0].appearances[0].application_notes[0].recommended_config.future.choices[1].enabled = false;
  light.application_notes.length = 0;
  assert.deepEqual(runtime.themeCatalog().presets[0].appearances[0], expected);
});

test("appearance discovery rejects malformed present collections and non-JSON recommendations", async () => {
  for (const [field, invalids] of [
    ["preview_colors", [null, {}, "#123456", [null], [1], [""]]],
    ["family_designs", [null, {}, [{ family_id: "class" }],
      [{ family_id: "xychart", treatment: "base_only" }, { family_id: "class", treatment: "base_only" }]]],
    ["application_notes", [null, {}, [null], [{}]]],
    ["qualified_cells", [undefined, null, {}]],
  ]) {
    for (const invalid of invalids) {
      const catalog = themeCatalogFixture();
      catalog.presets[0].appearances[0][field] = invalid;
      const runtime = await runtimeReturning(catalog);
      assert.throws(() => runtime.themeCatalog(), /preview color|family design|application note|qualified/);
    }
  }
  for (const config of [
    [], "classic", 1, false, new Date(), { nested: undefined }, { nested: () => {} },
    { nested: 1n }, { nested: NaN }, { nested: Infinity }, { nested: new Map() },
  ]) {
    const catalog = themeCatalogFixture();
    catalog.presets[0].appearances[0].application_notes = [{
      id: "future-note", family_id: "flowchart", message: "Review.",
      recommended_config: config,
    }];
    const runtime = await runtimeReturning(catalog);
    assert.throws(() => runtime.themeCatalog(), /recommended config/);
  }
});
