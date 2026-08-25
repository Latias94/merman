import assert from "node:assert/strict";
import test from "node:test";
import type { editor } from "monaco-editor";

import { MERMAID_SYNTAX_TOKEN_TYPES } from "./syntax-tokens.ts";
import {
  registerWorkbenchEditorThemes,
  WORKBENCH_EDITOR_THEMES,
} from "./workbench-editor-theme.ts";

test("workbench editor themes cover every Mermaid semantic token", () => {
  assert.equal(WORKBENCH_EDITOR_THEMES.dark.name, "merman-dark");
  assert.equal(WORKBENCH_EDITOR_THEMES.dark.data.base, "vs-dark");
  assert.equal(WORKBENCH_EDITOR_THEMES.light.name, "merman-light");
  assert.equal(WORKBENCH_EDITOR_THEMES.light.data.base, "vs");

  for (const { data } of Object.values(WORKBENCH_EDITOR_THEMES)) {
    const themedTokens = data.rules.map((rule) => rule.token).sort();
    assert.deepEqual(themedTokens, [...MERMAID_SYNTAX_TOKEN_TYPES].sort());
    assert.match(data.colors["editor.background"] ?? "", /^#[0-9A-F]{6}$/);
    assert.match(data.colors["editor.foreground"] ?? "", /^#[0-9A-F]{6}$/);
  }
});

test("theme registration publishes the stable light and dark themes", () => {
  const registrations = new Map<string, editor.IStandaloneThemeData>();

  registerWorkbenchEditorThemes({
    editor: {
      defineTheme(name, theme) {
        registrations.set(name, theme);
      },
    },
  });

  assert.deepEqual([...registrations.keys()].sort(), [
    "merman-dark",
    "merman-light",
  ]);
  assert.equal(
    registrations.get("merman-dark"),
    WORKBENCH_EDITOR_THEMES.dark.data,
  );
  assert.equal(
    registrations.get("merman-light"),
    WORKBENCH_EDITOR_THEMES.light.data,
  );
});
