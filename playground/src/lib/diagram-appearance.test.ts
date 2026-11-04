import assert from "node:assert/strict";
import test from "node:test";
import { appearanceChanged, selectDiagramAppearance, themeChoiceFields, undoAppearanceFields } from "./diagram-appearance.ts";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";

test("native selection removes extension identity without resetting font or authored config", () => {
  const before = selectDiagramAppearance({
    ...DEFAULT_WORKSPACE_SNAPSHOT, diagramFont: "georgia", mermaidConfig: '{"theme":"forest"}',
    themeRecipeJson: "{}", themeRecipeLabel: { name: "Nightfall", source: "exploration" },
  });
  const after = { ...before, ...themeChoiceFields({ kind: "mermaid", theme: "dark" }) };
  assert.equal(after.themeRecipeJson, null);
  assert.equal(after.themeRecipeLabel, null);
  assert.equal(after.diagramFont, "georgia");
  assert.equal(after.mermaidConfig, before.mermaidConfig);
  assert.equal(appearanceChanged(before, after), true);
  assert.deepEqual({ ...after, ...undoAppearanceFields(after, { before, after }) }, before);
});

test("undo restores only its owned fields and leaves newer edits intact", () => {
  const before = selectDiagramAppearance(DEFAULT_WORKSPACE_SNAPSHOT);
  const after = { ...before, diagramFont: "georgia" as const, mermaidConfig: '{"look":"classic"}' };
  const current = { ...after, mermaidConfig: '{"look":"neo"}' };
  assert.deepEqual(undoAppearanceFields(current, { before, after }), { diagramFont: "inherit" });
});

test("theme identity is restored atomically and labels do not depend on object identity", () => {
  const before = selectDiagramAppearance(DEFAULT_WORKSPACE_SNAPSHOT);
  const after = { ...before, ...themeChoiceFields({ kind: "recipe", recipeJson: "{}", label: { name: "Atlas", source: "exploration" } }) };
  const clone = { ...after, themeRecipeLabel: { name: "Atlas", source: "exploration" as const } };
  assert.equal(appearanceChanged(after, clone), false);
  assert.deepEqual({ ...clone, ...undoAppearanceFields(clone, { before, after }) }, before);
  const different = { ...after, themeRecipeLabel: { name: "User edit", source: "imported" as const } };
  assert.deepEqual(undoAppearanceFields(different, { before, after }), {});
});
