import assert from "node:assert/strict";
import test from "node:test";
import { presetLibraryDesigns } from "../themes/preset-library.ts";
import { themeCatalogFixture } from "./theme-appearance.test-fixture.ts";
import { filterThemeLibrary, createThemeLibraryFilters, themeDesignTreatment, themeSampleFor, themeSampleIds } from "./theme-library.ts";

test("preset designs use canonical runtime exports and retain reviewed shared families", () => {
  const catalog = themeCatalogFixture();
  for (const appearance of catalog.presets[0].appearances) {
    appearance.preview_colors = [appearance.appearance === "light" ? "#ffffff" : "#151515"];
    appearance.family_designs = [{ family_id: "class", treatment: "base_only" }];
  }
  const exported: string[] = [];
  const recipe = { schema_version: 1, kind: "design", design: { appearances: { light: {}, dark: {} } } };
  const [design] = presetLibraryDesigns([], catalog, (id) => { exported.push(id); return recipe; });
  assert.deepEqual(exported, ["editor"]);
  assert.deepEqual(JSON.parse(design.recipeJson), recipe);
  assert.equal(design.presetId, "editor");
  assert.deepEqual(design.appearances?.map((entry) => entry.appearance), ["light", "dark"]);
  assert.deepEqual(design.families, []);
  assert.deepEqual(design.sharedFamilies, ["class"]);
  assert.deepEqual(themeSampleIds(design), ["class"]);
  assert.equal(themeSampleFor(design, null), "class");
  assert.equal(themeDesignTreatment(design, "class"), "base_only");
  assert.equal(filterThemeLibrary([design], createThemeLibraryFilters("class")).length, 1);
  assert.equal(filterThemeLibrary([design], createThemeLibraryFilters("pie")).length, 0);
});

test("mapped or unavailable presets do not become duplicate library recipes", () => {
  const catalog = themeCatalogFixture();
  const unexpected = () => { throw new Error("must not export"); };
  assert.deepEqual(presetLibraryDesigns([{ presetId: "editor" }], catalog, unexpected), []);
  catalog.presets[0].available = false;
  assert.deepEqual(presetLibraryDesigns([], catalog, unexpected), []);
  assert.deepEqual(presetLibraryDesigns([], null, unexpected), []);
});
