import assert from "node:assert/strict";
import test from "node:test";

import { resolveThemeSelection } from "./theme-appearance.ts";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";
import { themeCatalogFixture, themeDocumentFixture } from "./theme-appearance.test-fixture.ts";

const catalog = themeCatalogFixture();

test("native preset appearance uses its declared default or follows the host when available", () => {
  const studio = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "studio" };
  assert.equal(resolveThemeSelection(studio, "dark", catalog).appearance, "light");
  assert.equal(resolveThemeSelection({ ...studio, themeAppearance: "follow" }, "dark", catalog).appearance, "dark");
  assert.equal(resolveThemeSelection({ ...studio, themePresetId: "paper-cut", themeAppearance: "follow" }, "dark", catalog).appearance, "light");
});

test("custom documents can inherit and add an appearance", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeDocumentJson: JSON.stringify({ schema_version: 2, extends: "paper-cut", appearances: { dark: themeDocumentFixture.appearances?.dark } }),
    themeAppearance: "follow" as const,
  };
  const selection = resolveThemeSelection(workspace, "dark", catalog);
  assert.deepEqual(selection.supported_appearances, ["light", "dark"]);
  assert.equal(selection.default_appearance, "light");
  assert.equal(selection.appearance, "dark");
  assert.equal(resolveThemeSelection({ ...workspace, themeAppearance: "light" }, "dark", catalog).appearance, "light");
});

test("classic pairs resolve a literal Mermaid ID while source/default stays automatic", () => {
  const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeAppearance: "follow" as const };
  assert.equal(resolveThemeSelection(workspace, "dark", catalog).mermaidTheme, "auto");
  assert.equal(resolveThemeSelection({ ...workspace, diagramTheme: "neo" }, "dark", catalog).mermaidTheme, "neo-dark");
  assert.equal(resolveThemeSelection({ ...workspace, diagramTheme: "forest" }, "dark", catalog).mermaidTheme, "forest");
});
