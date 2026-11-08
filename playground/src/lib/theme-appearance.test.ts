import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";
import {
  compatibleAppearancePreference, exportSelectedThemeRecipe, inspectThemeRecipe, preferredAppearanceForSelection,
  resolveThemeSelection, selectedPresetCatalogEntry, themeAppearanceSupport,
} from "./theme-appearance.ts";
import { sameThemeEditOrigin } from "./theme-draft.ts";
import { themeCatalogFixture, themeInspectionFixture } from "./theme-appearance.test-fixture.ts";
import { decodeShareHash, encodeShareHash } from "./share.ts";
import {
  freezeRenderOperation, matchesWorkspaceAppearance, renderOperationWithSvgPipeline, sameRenderOperation,
} from "../runtime/merman-operation-input.ts";
import { selectedThemeKey } from "../components/theme-panel/theme-selection.ts";
import { captureProjectHandoff, generateProjectHandoff, ProjectHandoffError, projectHandoffMatchesWorkspace } from "./project-handoff.ts";
import type { MermanDomainFacade } from "../runtime/merman-core.ts";
import type { RenderCoordinatorState } from "../runtime/render-coordinator.ts";

const catalog = themeCatalogFixture();
const inspection = themeInspectionFixture();
const recipe = readFileSync(new URL("../themes/recipes/reading.theme.json", import.meta.url), "utf8");
const paired = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: recipe, themeAppearance: "follow" as const };
const environment = {
  asciiEnabled: false, compareEnabled: true, diagnosticsEnabled: false,
  layoutEnvironment: { containerWidth: 800, containerHeight: 600 }, viewport: { width: 800, height: 600 },
  versions: { merman: "matching-test-contract", mermaid: "12.3.0" }, themeCatalog: catalog,
  themeRecipeInspection: inspection,
};

test("Follow resolves from application appearance while concrete pins and source/default stay fixed", () => {
  assert.equal(resolveThemeSelection(paired, "light", catalog, inspection).appearance, "light");
  assert.equal(resolveThemeSelection(paired, "dark", catalog, inspection).appearance, "dark");
  assert.equal(resolveThemeSelection({ ...paired, themeAppearance: "light" }, "dark", catalog, inspection).appearance, "light");
  const source = resolveThemeSelection(DEFAULT_WORKSPACE_SNAPSHOT, "dark", catalog);
  assert.equal(source.mermaidTheme, "auto");
  assert.equal(source.appearance, null);
  assert.equal(source.preference, "default");
});

test("only catalog-declared native pairs resolve to another literal ID", () => {
  const native = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "neo" as const, themeAppearance: "follow" as const };
  assert.equal(resolveThemeSelection(native, "dark", catalog).mermaidTheme, "neo-dark");
  assert.equal(resolveThemeSelection({ ...native, diagramTheme: "default" }, "dark", catalog).mermaidTheme, "default");
  assert.equal(resolveThemeSelection({ ...native, diagramTheme: "forest" }, "dark", catalog).mermaidTheme, "forest");
  const old = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "neo-dark" as const };
  assert.equal(resolveThemeSelection(old, "light", catalog).mermaidTheme, "neo-dark");
  assert.equal(selectedThemeKey(old, [], catalog), "mermaid:neo");
});

test("a concrete preset ID displays its grouped identity while keeping the exact input and pin", () => {
  const value = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "editor-dark" };
  assert.equal(selectedThemeKey(value, [], catalog), "preset:editor");
  const selection = selectedPresetCatalogEntry(catalog, value.themePresetId);
  assert.equal(selection?.preset.id, "editor");
  assert.equal(selection?.pinned, "dark");
  const resolved = resolveThemeSelection(value, "light", catalog);
  assert.equal(resolved.appearance, "dark");
  assert.deepEqual(resolved.supported_appearances, ["dark"]);
  const operation = freezeRenderOperation({ ...environment, workspace: value, hostAppearance: "light" });
  assert.equal(operation.themePresetId, "editor-dark");
  assert.deepEqual(operation.bindingOptions.theme, { preset: "editor-dark", appearance: "dark" });
  catalog.presets[0].available = false;
  try {
    const unavailable = freezeRenderOperation({ ...environment, workspace: value, hostAppearance: "light" });
    assert.equal(unavailable.unavailableThemePresetId, "editor-dark");
    assert.equal(unavailable.bindingOptions.theme, undefined);
  } finally { catalog.presets[0].available = true; }
});

test("known single appearances reject explicit unsupported choices while Follow uses the declared default", () => {
  const single = { supported_appearances: ["light" as const], default_appearance: "light" as const };
  const followed = resolveThemeSelection(paired, "dark", catalog, single);
  assert.equal(followed.appearance, "light");
  assert.equal(followed.error, undefined);
  const explicit = { ...paired, themeAppearance: "dark" as const };
  const rejected = resolveThemeSelection(explicit, "light", catalog, single);
  assert.equal(rejected.preference, "dark");
  assert.equal(rejected.appearance, null);
  assert.match(rejected.error ?? "", /does not support the "dark" appearance.*light/);
  const operation = freezeRenderOperation({ ...environment, workspace: explicit, themeRecipeInspection: single });
  assert.equal(operation.configurationError?.summary, rejected.error);
  assert.equal(operation.resolvedThemeAppearance, null);
  assert.equal(resolveThemeSelection(explicit, "light", catalog).error, undefined);
  assert.equal(compatibleAppearancePreference("dark", single), null);
  assert.equal(compatibleAppearancePreference("follow", single), "follow");
  assert.equal(preferredAppearanceForSelection(single), "light");
  assert.equal(preferredAppearanceForSelection(inspection), "follow");
  assert.equal(themeAppearanceSupport(paired, catalog).default_appearance, null);
});

test("s5 Forest plus explicit Dark is readable but cannot become a valid operation or adoption", () => {
  const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "forest" as const, themeAppearance: "dark" as const };
  const hash = encodeShareHash(workspace);
  assert.match(hash, /^#s5:/);
  const restored = decodeShareHash(hash)!;
  assert.deepEqual(restored, workspace);
  const selection = resolveThemeSelection(restored, "dark", catalog);
  assert.equal(selection.preference, "dark");
  assert.equal(selection.mermaidTheme, "forest");
  assert.equal(selection.appearance, null);
  assert.match(selection.error ?? "", /Mermaid theme "forest" does not support the "dark" appearance/);
  const operation = freezeRenderOperation({ ...environment, workspace: restored, hostAppearance: "dark" });
  assert.equal(operation.configurationError?.summary, selection.error);
  assert.equal(operation.resolvedThemeAppearance, null);
  const snapshot = captureProjectHandoff({
    workspace: restored, renderState: { status: "empty" }, hostAppearance: "dark", themeCatalog: catalog,
    capturedAt: "2026-11-07T13:00:00Z",
  });
  for (const target of ["rust", "cli", "web"] as const) {
    for (const scope of ["diagram", "theme"] as const) {
      assert.throws(() => generateProjectHandoff(snapshot, { target, scope }),
        (error) => error instanceof ProjectHandoffError && error.code === "invalid-configuration" && error.message === selection.error);
    }
  }
  const followed = freezeRenderOperation({ ...environment, workspace: { ...restored, themeAppearance: "follow" }, hostAppearance: "dark" });
  assert.equal(followed.configurationError, null);
  assert.equal(followed.resolvedThemeAppearance, "light");
  assert.equal(followed.mermaidTheme, "forest");
});

test("valid native theme adoption pins the already-resolved literal and its captured appearance", () => {
  for (const diagramTheme of ["forest", "neo"] as const) {
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme, themeAppearance: "follow" as const };
    const snapshot = captureProjectHandoff({
      workspace, renderState: { status: "empty" }, hostAppearance: "dark", themeCatalog: catalog,
      capturedAt: "2026-11-07T13:00:00Z",
    });
    for (const target of ["rust", "cli", "web"] as const) {
      const bundle = generateProjectHandoff(snapshot, { target, scope: "theme" });
      assert.equal(JSON.parse(bundle.files["options.json"]).site_config.theme, diagramTheme === "neo" ? "neo-dark" : "forest");
      assert.equal(JSON.parse(bundle.files["handoff.json"]).theme.resolved_appearance, diagramTheme === "neo" ? "dark" : "light");
    }
  }
});

test("unclassified complete specs reject explicit appearances without inventing branches", () => {
  const unclassified = { ...inspection, supported_appearances: [], default_appearance: null, appearances: [] };
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: '{"schema_version":1,"kind":"complete_spec","complete_spec":{}}',
    themeAppearance: "dark" as const,
  };
  const selection = resolveThemeSelection(workspace, "dark", catalog, unclassified);
  assert.equal(selection.appearance, null);
  assert.match(selection.error ?? "", /does not support the "dark" appearance.*none declared/);
  const operation = freezeRenderOperation({ ...environment, workspace, themeRecipeInspection: unclassified });
  assert.equal(operation.configurationError?.summary, selection.error);
  for (const preference of ["default", "follow"] as const) {
    const allowed = freezeRenderOperation({ ...environment, workspace: { ...workspace, themeAppearance: preference }, themeRecipeInspection: unclassified });
    assert.equal(allowed.configurationError, null);
    assert.equal(allowed.resolvedThemeAppearance, null);
  }
});

test("single preset metadata and concrete pins reject unsupported explicit appearances", () => {
  const singleCatalog = themeCatalogFixture();
  singleCatalog.presets[0].supported_appearances = ["light"];
  singleCatalog.presets[0].appearances = singleCatalog.presets[0].appearances.slice(0, 1);
  for (const themePresetId of ["editor", "editor-light"]) {
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId, themeAppearance: "dark" as const };
    const operation = freezeRenderOperation({ ...environment, workspace, themeCatalog: singleCatalog });
    assert.match(operation.configurationError?.summary ?? "", /does not support the "dark" appearance.*light/);
    assert.equal(operation.resolvedThemeAppearance, null);
    const followed = freezeRenderOperation({ ...environment, workspace: { ...workspace, themeAppearance: "follow" }, themeCatalog: singleCatalog, hostAppearance: "dark" });
    assert.equal(followed.configurationError, null);
    assert.equal(followed.resolvedThemeAppearance, "light");
  }
  const unknown = resolveThemeSelection({ ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "future-preset", themeAppearance: "dark" }, "dark", catalog);
  assert.equal(unknown.error, undefined);
  assert.equal(unknown.appearance, "dark");
});

test("complete raw inspection has no preview selector and never normalizes duplicate fields", () => {
  const raw = '{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"light":{}},"default_appearance":"light"}}';
  let input: unknown;
  const facade = { materializeTheme(value: unknown) { input = value; return inspection; } } as Pick<MermanDomainFacade, "materializeTheme">;
  assert.equal(inspectThemeRecipe(facade, raw), inspection);
  assert.equal(input, `{"schema_version":1,"action":"inspect","recipe":${raw}}`);
  const rejected = { materializeTheme() { throw new Error("duplicate appearance"); } } as Pick<MermanDomainFacade, "materializeTheme">;
  assert.throws(() => inspectThemeRecipe(rejected, raw), /duplicate appearance/);
});

test("plain preset export preserves the complete authored document", () => {
  let input: unknown;
  const facade = { exportThemePreset(id: string) { input = id; return JSON.parse(recipe); } } as Pick<MermanDomainFacade, "exportThemePreset">;
  const exported = exportSelectedThemeRecipe(facade, "editor-dark");
  assert.equal(input, "editor-dark");
  assert.deepEqual(Object.keys(JSON.parse(exported).design.appearances), ["light", "dark"]);
});

test("the frozen selection includes policy, concrete appearance, and byte-preserved whole recipe", () => {
  const light = freezeRenderOperation({ ...environment, workspace: paired, hostAppearance: "light" });
  const dark = freezeRenderOperation({ ...environment, workspace: paired, hostAppearance: "dark" });
  assert.equal(sameRenderOperation(light, dark), false);
  assert.equal(dark.themeAppearancePreference, "follow");
  assert.equal(dark.resolvedThemeAppearance, "dark");
  assert.ok(dark.bindingOptionsJson?.includes(`"recipe":${recipe},"appearance":"dark"`));
  assert.equal(matchesWorkspaceAppearance(light, paired, "dark"), false);
  assert.equal(matchesWorkspaceAppearance(dark, paired, "dark"), true);
  const raster = renderOperationWithSvgPipeline(dark, "resvg-safe");
  assert.equal(JSON.parse(raster.bindingOptionsJson!).theme.appearance, "dark");
  assert.equal(raster.themeRecipeJson, recipe);
  const native = freezeRenderOperation({ ...environment, workspace: {
    ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "neo", themeAppearance: "follow",
  }, hostAppearance: "dark" });
  assert.equal(native.theme, "neo");
  assert.equal(native.mermaidTheme, "neo-dark");
  assert.equal(native.bindingOptions.site_config?.theme, "neo-dark");
  assert.equal(native.bindingOptions.theme, undefined);
  const composed = freezeRenderOperation({ ...environment, workspace: { ...paired, diagramTheme: "forest" }, hostAppearance: "dark" });
  assert.equal(composed.mermaidTheme, "forest");
  assert.equal(composed.bindingOptions.site_config?.theme, "forest");
});

test("customization owns theme identity while later source, layout and font changes remain independent", () => {
  const origin = { ...paired, diagramTheme: "forest" as const };
  assert.equal(sameThemeEditOrigin(origin, { ...origin, code: "new source", mermaidConfig: '{"layout":"elk"}', diagramFont: "georgia" }), true);
  assert.equal(sameThemeEditOrigin(origin, { ...origin, themeAppearance: "light" }), false);
  assert.equal(sameThemeEditOrigin(origin, { ...origin, themeRecipeJson: `${recipe}\n` }), false);
  assert.equal(sameThemeEditOrigin(origin, { ...origin, themeRecipeLabel: { name: "Other", source: "imported" } }), false);
});

test("adoption freezes a concrete appearance for every consumer while keeping all authored branches", () => {
  const operation = freezeRenderOperation({ ...environment, workspace: paired, hostAppearance: "dark" });
  const state = { status: "success", snapshot: { operation, publicationId: 1 }, merman: { status: "success" } } as RenderCoordinatorState;
  const snapshot = captureProjectHandoff({
    workspace: paired, renderState: state, hostAppearance: "dark", themeCatalog: catalog,
    themeRecipeInspection: inspection, validateThemeRecipe: () => undefined, capturedAt: "2026-11-07T12:00:00Z",
  });
  assert.equal(projectHandoffMatchesWorkspace(snapshot, paired, "light"), false);
  assert.equal(snapshot.selection.appearance, "dark");
  for (const target of ["rust", "cli", "web"] as const) {
    for (const scope of ["diagram", "theme"] as const) {
      const bundle = generateProjectHandoff(snapshot, { target, scope });
      assert.equal(bundle.files["theme.json"], recipe);
      assert.equal(JSON.parse(bundle.files["options.json"]).theme.appearance, "dark");
      assert.equal(JSON.parse(bundle.files["handoff.json"]).theme.resolved_appearance, "dark");
      if (target === "rust") assert.match(bundle.code, /compile_document\(recipe, Some\(ThemeAppearance::Dark\)\)/);
      if (target === "cli") assert.match(bundle.code, /--theme-file theme\.json --theme-appearance dark/);
    }
  }
  assert.equal(captureProjectHandoff({
    workspace: paired, renderState: state, hostAppearance: "light", themeRecipeInspection: inspection, capturedAt: "later",
  }).operation, null);
});
