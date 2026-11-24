import assert from "node:assert/strict";
import test from "node:test";

import type { ResolvedThemeV2, ThemeDocumentV2 } from "@mermanjs/web";
import { resolvedThemeFixture, themeDocumentFixture } from "../lib/theme-appearance.test-fixture.ts";
import { applyContrastFix, contrastGrade, expandThemeDocument } from "./authoring.ts";
import { formatThemeDocument } from "./v2.ts";

test("standalone export retains both appearance layers and uses native overlay order", () => {
  const base: ThemeDocumentV2 = {
    ...themeDocumentFixture,
    id: "studio",
    seed: { accent: "#0000FF" },
    roles: { "text.muted": "#202020" },
    form: { chart: { pie_opacity: 0.9 }, elevation: "soft" },
    appearances: {
      ...themeDocumentFixture.appearances,
      dark: { ...themeDocumentFixture.appearances?.dark, elevation: "soft", canvas: { textures: [{ kind: "grid", step_px: 20 }], padding_px: 8 } },
    },
    overrides: [{ families: ["flowchart"], node_ordinal_palette: true }],
  };
  const overlay: ThemeDocumentV2 = {
    schema_version: 2, extends: "studio", name: "My diagram",
    seed: { surface: "#EFEFEF" },
    form: { chart: { bar_radius: 3 }, elevation: "flat" },
    appearances: { dark: { seed: { accent: "#FF0000" }, canvas: { textures: [] } } },
    overrides: [{ families: ["flowchart"], appearance: "dark", roles: { "text.title": "#EEEEEE" } }],
  };
  const exported = JSON.parse(formatThemeDocument(expandThemeDocument(overlay, base)));
  assert.equal(exported.extends, undefined);
  assert.equal(exported.id, undefined);
  assert.equal(exported.default_appearance, "light");
  assert.deepEqual(exported.appearances.light.seed, { canvas: "#ffffff", ink: "#000000", accent: "#0000FF" });
  assert.equal(exported.appearances.dark.seed.accent, "#FF0000");
  assert.deepEqual(exported.appearances.dark.canvas, { textures: [], padding_px: 8 });
  assert.equal(exported.appearances.dark.elevation, undefined);
  assert.deepEqual(exported.appearances.light.roles, base.roles);
  assert.deepEqual(exported.form.chart, { pie_opacity: 0.9, bar_radius: 3 });
  assert.deepEqual(exported.overrides, [...base.overrides!, ...overlay.overrides!]);
});

test("contrast adjustments update the active editable appearance and preserve the other one", () => {
  const document: ThemeDocumentV2 = { schema_version: 2, extends: "studio", roles: { "text.primary": "#EEEEEE" } };
  const before: ResolvedThemeV2 = {
    ...resolvedThemeFixture,
    roles: { ...resolvedThemeFixture.roles, "text.primary": "#EEEEEE" },
    data: { ink: ["#AAAAAA"], fill: ["#EEEEEE"], fill_ink: ["#AAAAAA"], pattern_encoding: false },
  };
  const fixed = applyContrastFix(document, before, {
    ...before, roles: { ...before.roles, "text.primary": "#555555" }, data: { ...before.data, ink: ["#777777"], fill_ink: ["#666666"] },
  }, ["light", "dark"]);
  assert.equal(fixed.extends, "studio");
  assert.equal(fixed.roles, undefined);
  assert.deepEqual(fixed.appearances, {
    light: { roles: { "text.primary": "#555555" }, data: { ink: ["#777777"], fill_ink: "auto" } },
    dark: { roles: { "text.primary": "#EEEEEE" } },
  });
});

test("contrast badges reserve AAA for text with at least seven-to-one contrast", () => {
  const resolved: ResolvedThemeV2 = {
    ...resolvedThemeFixture,
    data: { ...resolvedThemeFixture.data, fill: ["#FFFFFF"], fill_ink: ["#000000"] },
  };
  assert.equal(contrastGrade({ ok: true, resolved, notices: [] }), "AAA");
  assert.equal(contrastGrade({ ok: true, resolved: { ...resolved, roles: { ...resolved.roles, "text.muted": "#666666" } }, notices: [] }), "AA");
  assert.equal(contrastGrade({ ok: true, resolved, notices: [
    { kind: "low_contrast", foreground: "text.primary", background: "canvas", ratio: 2, minimum: 4.5 },
  ] }), "warning");
});
