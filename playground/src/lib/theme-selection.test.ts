import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";
import { utf8ByteLength } from "./utf8.ts";
import { currentRecipeIdentity, importedThemeName, selectedThemeKey } from "../components/theme-panel/theme-selection.ts";

const explorations = [{ id: "atlas", name: "Atlas", recipeJson: "{\n  \"kind\": \"design\"\n}\n" }];

test("an exploration keeps its name, provenance and original recipe", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: explorations[0].recipeJson,
    themeRecipeLabel: { name: "Atlas", source: "exploration" as const },
  };
  assert.equal(selectedThemeKey(workspace, explorations), "exploration:atlas");
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Atlas", source: "exploration", explorationId: "atlas", recipeJson: explorations[0].recipeJson,
  });
});

test("an imported copy of an exploration keeps the imported identity", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: explorations[0].recipeJson,
    themeRecipeLabel: { name: "Team review", source: "imported" as const },
  };
  assert.equal(selectedThemeKey(workspace, explorations), "recipe:current");
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Team review", source: "imported", explorationId: null, recipeJson: explorations[0].recipeJson,
  });
});

test("older recipes keep their JSON without inferring an exploration origin from matching content", () => {
  const exact = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: explorations[0].recipeJson };
  assert.equal(selectedThemeKey(exact, explorations), "recipe:current");
  assert.equal(currentRecipeIdentity(exact, explorations, "Imported theme")?.source, "imported");
  const authored = { ...exact, themeRecipeJson: JSON.stringify(JSON.parse(explorations[0].recipeJson)) };
  assert.equal(selectedThemeKey(authored, explorations), "recipe:current");
  assert.equal(currentRecipeIdentity(authored, explorations, "Imported theme")?.recipeJson, authored.themeRecipeJson);
});

test("unavailable presets retain their requested identity instead of selecting the base", () => {
  const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "future-preset" };
  assert.equal(selectedThemeKey(workspace, explorations), "preset:future-preset");
  assert.equal(selectedThemeKey(DEFAULT_WORKSPACE_SNAPSHOT, explorations), "mermaid:auto");
});

test("an exploration label survives recipe edits and a missing catalog entry", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: "{}",
    themeRecipeLabel: { name: "Atlas variant", source: "exploration" as const },
  };
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Atlas variant", source: "exploration", explorationId: null, recipeJson: "{}",
  });
});

test("file names provide readable imported names without losing the user's name", () => {
  assert.equal(importedThemeName("Team review.theme.json"), "Team review");
  assert.equal(importedThemeName("自定义.JSON"), "自定义");
  assert.equal(importedThemeName("nightfall.json"), "nightfall");
  assert.equal(importedThemeName(".json"), ".json");
  assert.equal(importedThemeName("  "), "Imported theme");
});

test("imported names fit the shared label envelope without cutting a Unicode character", () => {
  const name = importedThemeName(`${"🪴".repeat(80)}.theme.json`);
  assert.equal(utf8ByteLength(name), 256);
  assert.equal(name, "🪴".repeat(64));
  assert.equal(importedThemeName(`${"中".repeat(100)}.json`), "中".repeat(85));
  assert.equal(importedThemeName(`${"a".repeat(256)}.json`), "a".repeat(256));
});

test("a customized copy retains its original preset provenance when selected again", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: "{}",
    themeRecipeLabel: { name: "Editor", source: "imported" as const, derivedFrom: "preset:editor-dark" },
  };
  assert.equal(currentRecipeIdentity(workspace, [], "Imported")?.derivedFrom, "preset:editor-dark");
  assert.equal(selectedThemeKey(workspace, []), "recipe:current");
});
