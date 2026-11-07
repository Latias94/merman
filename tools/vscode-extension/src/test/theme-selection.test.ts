import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  editorThemeAppearance,
  importedThemeInspection,
  legacyThemeDiscovery,
  resolveThemeSelection,
} from "../theme-selection.js";
import { pairedThemeDiscovery } from "./theme-fixtures.js";

describe("theme selection", () => {
  it("keeps source/default unspecified in both editor appearances", () => {
    for (const host of ["light", "dark"] as const) {
      const selection = resolveThemeSelection(pairedThemeDiscovery(), "source", "follow-editor", host);
      assert.equal(selection.theme, undefined);
      assert.equal(selection.themePreset, undefined);
      assert.equal(selection.effectiveAppearance, undefined);
    }
  });

  it("resolves only the metadata-declared native pair to literal Mermaid IDs", () => {
    assert.equal(resolveThemeSelection(pairedThemeDiscovery(), "mermaid:neo", "follow-editor", "dark").theme, "neo-dark");
    assert.equal(resolveThemeSelection(pairedThemeDiscovery(), "mermaid:neo", "light", "dark").theme, "neo");
    assert.equal(resolveThemeSelection(pairedThemeDiscovery(), "default", "follow-editor", "dark").theme, "default");
    assert.equal(resolveThemeSelection(pairedThemeDiscovery(), "dark", "follow-editor", "light").theme, "dark");
  });

  it("falls back only for an undeclared Follow appearance, not an explicit request", () => {
    const selection = resolveThemeSelection(pairedThemeDiscovery(), "preset:ghibli", "follow-editor", "dark");
    assert.equal(selection.effectiveAppearance, "light");
    assert.match(selection.notice!, /does not provide dark/);
    assert.throws(() => resolveThemeSelection(pairedThemeDiscovery(), "preset:ghibli", "dark", "dark"), /does not provide a dark/);
    assert.throws(() => resolveThemeSelection(legacyThemeDiscovery(), "preset:editor", "follow-editor", "dark"), /unavailable/);
  });

  it("maps both high-contrast kinds to light/dark without inventing a third appearance", () => {
    assert.equal(editorThemeAppearance(1), "light");
    assert.equal(editorThemeAppearance(2), "dark");
    assert.equal(editorThemeAppearance(3), "dark");
    assert.equal(editorThemeAppearance(4), "light");
  });

  it("retains the exact admitted full recipe rather than a selected or parsed snapshot", () => {
    const content = '{ "schema_version": 1, "kind": "design", "design": { "appearances": {"light": {}, "dark": {}} } }\n';
    const imported = importedThemeInspection(content, "reading.json", {
      schema_version: 1,
      supported_appearances: ["light", "dark"],
      default_appearance: "light",
      appearances: [{ appearance: "light", references: [] }, { appearance: "dark", references: [] }],
    });
    assert.equal(resolveThemeSelection(pairedThemeDiscovery(), "file", "dark", "light", imported).themeFileContent, content);
    assert.throws(() => importedThemeInspection(content, "reading.json", {
      schema_version: 1,
      supported_appearances: ["light"],
      default_appearance: "dark",
      appearances: [],
    }), /inconsistent/);
  });
});
