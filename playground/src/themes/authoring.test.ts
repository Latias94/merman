import assert from "node:assert/strict";
import test from "node:test";

import type { ResolvedThemeV2, ThemeDocumentV2, ThemeFamilyOverride, ThemeRoles } from "@mermanjs/web";
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
  assert.ok(fixed);
  assert.equal(fixed.extends, "studio");
  assert.equal(fixed.roles, undefined);
  assert.deepEqual(fixed.appearances, {
    light: { roles: { "text.primary": "#555555" }, data: { ink: ["#777777"], fill_ink: "auto" } },
    dark: { roles: { "text.primary": "#EEEEEE" } },
  });
});

const studioAppearances = ["light", "dark"] as const;

function familyTheme(roles: Partial<ThemeRoles>) {
  return { ...resolvedThemeFixture, roles: { ...resolvedThemeFixture.roles, ...roles } };
}

function darkOverrides(count: number): ThemeFamilyOverride[] {
  return Array.from({ length: count }, () => ({ families: ["flowchart"], appearance: "dark", form: { radius: 5 } }));
}

test("contrast fixes for all 34 families share one appearance-scoped override in the last slot", () => {
  const families = Array.from({ length: 34 }, (_, i) => `family-${i}`);
  const overrides = [...darkOverrides(30), { families, roles: { "text.primary": "#FFFFFF" } }];
  const document: ThemeDocumentV2 = { schema_version: 2, extends: "studio", overrides };
  const before = {
    ...resolvedThemeFixture,
    family_overrides: Object.fromEntries(families.map((id) => [id, familyTheme({ "text.primary": "#FFFFFF" })])),
  };
  const after = {
    ...before,
    family_overrides: Object.fromEntries(families.map((id) => [id, familyTheme({ "text.primary": "#666666" })])),
  };

  const fixed = applyContrastFix(document, before, after, studioAppearances);

  assert.deepEqual(fixed?.overrides, [...overrides, { families, appearance: "light", roles: { "text.primary": "#666666" } }]);
  assert.equal(document.overrides?.length, 31);
});

test("contrast fixes reuse writable role winners at capacity while preserving overlapping rules", () => {
  const overrides: ThemeFamilyOverride[] = [
    ...darkOverrides(28),
    { families: ["flowchart", "sequence"], appearance: "light", roles: { "text.primary": "#FFFFFF", "stroke.border": "#AAAAAA" } },
    { families: ["sequence"], appearance: "light", roles: { "text.primary": "#FFFF99" } },
    { families: ["flowchart"], roles: { "text.muted": "#FFFFFF" } },
    { families: ["flowchart"], appearance: "light", form: { radius: 10 } },
  ];
  const document: ThemeDocumentV2 = { schema_version: 2, extends: "studio", overrides };
  const before = { ...resolvedThemeFixture, family_overrides: {
    flowchart: familyTheme({ "text.primary": "#FFFFFF", "text.muted": "#FFFFFF" }),
    sequence: familyTheme({ "text.primary": "#FFFF99" }),
  } };
  const after = { ...before, family_overrides: {
    flowchart: familyTheme({ "text.primary": "#666666", "text.muted": "#777777" }),
    sequence: familyTheme({ "text.primary": "#555522" }),
  } };
  const original = structuredClone(document);

  const fixed = applyContrastFix(document, before, after, studioAppearances);

  assert.deepEqual(fixed?.overrides, [
    ...overrides.slice(0, 28),
    { ...overrides[28], roles: { "text.primary": "#666666", "stroke.border": "#AAAAAA" } },
    { ...overrides[29], roles: { "text.primary": "#555522" } },
    overrides[30],
    { ...overrides[31], roles: { "text.muted": "#777777" } },
  ]);
  assert.deepEqual(document, original);
});

test("contrast fixes account for corrected base colors and inherited role writers", () => {
  const base: ThemeDocumentV2 = {
    ...themeDocumentFixture,
    overrides: [{ families: ["class"], roles: { "text.primary": "#EEEEEE" } }],
  };
  const overrides = [{ families: ["flowchart", "sequence"], form: { radius: 10 } }];
  const document: ThemeDocumentV2 = { schema_version: 2, extends: "studio", roles: { "text.primary": "#EEEEEE" }, overrides };
  const unfixed = familyTheme({ "text.primary": "#EEEEEE" });
  const corrected = familyTheme({ "text.primary": "#555555" });
  const before = { ...unfixed, family_overrides: { class: unfixed, flowchart: unfixed, sequence: unfixed } };
  const after = { ...corrected, family_overrides: { class: corrected, flowchart: corrected, sequence: unfixed } };

  const fixed = applyContrastFix(document, before, after, studioAppearances, base);

  assert.deepEqual(fixed?.overrides, [
    ...overrides,
    { families: ["class"], appearance: "light", roles: { "text.primary": "#555555" } },
    { families: ["sequence"], appearance: "light", roles: { "text.primary": "#EEEEEE" } },
  ]);
  assert.deepEqual(fixed?.appearances?.dark?.roles, document.roles);
});

test("contrast fixes decline a full document when the last role writer is shared", () => {
  const overrides: ThemeFamilyOverride[] = [
    ...darkOverrides(30),
    { families: ["flowchart"], appearance: "light", roles: { "text.primary": "#222222" } },
    { families: ["flowchart"], roles: { "text.primary": "#FFFFFF" } },
  ];
  const document: ThemeDocumentV2 = { schema_version: 2, extends: "studio", overrides };
  const before = { ...resolvedThemeFixture, family_overrides: { flowchart: familyTheme({ "text.primary": "#FFFFFF" }) } };
  const after = { ...before, family_overrides: { flowchart: familyTheme({ "text.primary": "#666666" }) } };
  const original = structuredClone(document);

  assert.equal(applyContrastFix(document, before, after, studioAppearances), null);
  assert.deepEqual(document, original);
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
