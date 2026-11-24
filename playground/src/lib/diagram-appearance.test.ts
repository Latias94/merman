import assert from "node:assert/strict";
import test from "node:test";
import { appearanceChanged, selectDiagramAppearance, themeChoiceFields, undoAppearanceFields } from "./diagram-appearance.ts";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";

test("Mermaid selection clears the custom theme and preserves the host font", () => {
  const before = selectDiagramAppearance({
    ...DEFAULT_WORKSPACE_SNAPSHOT, diagramFont: "georgia", mermaidConfig: '{"theme":"forest"}',
    themeDocumentJson: "{}", themeDocumentLabel: { name: "Nightfall", source: "exploration" },
  });
  const after = { ...before, ...themeChoiceFields({ kind: "mermaid", theme: "dark" }) };
  assert.equal(after.themeDocumentJson, null);
  assert.equal(after.themeDocumentLabel, null);
  assert.equal(after.diagramFont, "georgia");
  assert.equal(appearanceChanged(before, after), true);
  assert.deepEqual({ ...after, ...undoAppearanceFields(after, { before, after }) }, before);
});

test("undo restores only the fields the appearance change still owns", () => {
  const before = selectDiagramAppearance(DEFAULT_WORKSPACE_SNAPSHOT);
  const after = { ...before, diagramFont: "georgia" as const };
  const current = { ...after, diagramTheme: "forest" as const };
  assert.deepEqual(undoAppearanceFields(current, { before, after }), { diagramFont: "inherit" });
});

test("theme identity is restored atomically and labels do not depend on object identity", () => {
  const before = selectDiagramAppearance(DEFAULT_WORKSPACE_SNAPSHOT);
  const after = { ...before, ...themeChoiceFields({ kind: "document", documentJson: "{}", label: { name: "Atlas", source: "exploration" } }) };
  const clone = { ...after, themeDocumentLabel: { name: "Atlas", source: "exploration" as const } };
  assert.equal(appearanceChanged(after, clone), false);
  assert.deepEqual({ ...clone, ...undoAppearanceFields(clone, { before, after }) }, before);
  const different = { ...after, themeDocumentLabel: { name: "User edit", source: "imported" as const } };
  assert.deepEqual(undoAppearanceFields(different, { before, after }), {});
});
