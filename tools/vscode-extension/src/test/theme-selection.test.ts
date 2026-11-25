import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  editorThemeAppearance,
  resolveThemeSelection,
} from "../theme-selection.js";
import { v2ThemeDiscovery } from "./theme-fixtures.js";

describe("theme selection", () => {
  it("keeps source/default unspecified in both editor appearances", () => {
    for (const host of ["light", "dark"] as const) {
      const selection = resolveThemeSelection(v2ThemeDiscovery(), "source", "follow-editor", host);
      assert.equal(selection.theme, undefined);
      assert.equal(selection.effectiveAppearance, undefined);
    }
  });

  it("keeps the chosen Mermaid Classic ID and leaves its appearance to Mermaid", () => {
    const neo = resolveThemeSelection(v2ThemeDiscovery(), "neo", "follow-editor", "dark");
    assert.equal(neo.theme, "neo");
    assert.equal(neo.effectiveAppearance, undefined);
    assert.equal(resolveThemeSelection(v2ThemeDiscovery(), "default", "follow-editor", "dark").theme, "default");
    assert.equal(resolveThemeSelection(v2ThemeDiscovery(), "dark", "follow-editor", "light").theme, "dark");
  });

  it("uses a single-mode theme's default when the editor or saved preference requests the other mode", () => {
    const selection = resolveThemeSelection(v2ThemeDiscovery(), "paper-cut", "follow-editor", "dark");
    assert.equal(selection.effectiveAppearance, "light");
    assert.match(selection.notice!, /does not provide dark/);
    assert.equal(resolveThemeSelection(v2ThemeDiscovery(), "paper-cut", "dark", "dark").effectiveAppearance, "light");
  });

  it("maps both high-contrast kinds to light/dark without inventing a third appearance", () => {
    assert.equal(editorThemeAppearance(1), "light");
    assert.equal(editorThemeAppearance(2), "dark");
    assert.equal(editorThemeAppearance(3), "dark");
    assert.equal(editorThemeAppearance(4), "light");
  });

  it("retains the exact admitted theme document", () => {
    const content = '{ "schema_version": 2, "extends": "studio" }\n';
    const imported = {
      content,
      label: "reading.json",
      supportedAppearances: ["light", "dark"] as Array<"light" | "dark">,
      defaultAppearance: "light" as const,
    };
    assert.equal(resolveThemeSelection(v2ThemeDiscovery(), "file", "dark", "light", imported).themeFileContent, content);
  });
});
