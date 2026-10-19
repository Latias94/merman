import assert from "node:assert/strict";
import test from "node:test";
import type { ThemeRecipeV1 } from "@mermanjs/web";

import {
  appendThemeRule,
  removeThemeCanvasBackground,
  restoreThemeCanvas,
  setDefinitionToken,
  setDefinitionTypography,
  setCompleteTypography,
  setThemeCanvasBase,
  setThemeRuleFacet,
  setThemeRuleSelector,
  moveThemeRule,
  removeThemeRule,
  validateThemeDocument,
} from "./theme-document.ts";

test("validates original bytes before decoding duplicate fields or unknown versions", () => {
  for (const raw of [
    '{"schema_version":1,"schema_version":2,"kind":"definition","definition":{}}',
    '{"schema_version":99,"kind":"definition","definition":{}}',
  ]) {
    const observed: string[] = [];
    assert.throws(() => validateThemeDocument(raw, (input) => {
      observed.push(input);
      throw new Error("Rust rejected recipe");
    }), /Rust rejected recipe/);
    assert.deepEqual(observed, [raw]);
  }
});

test("complete recipe edits retain ordered rules, layers, effects and assets", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: {
      mermaid: { theme: "base", dark_mode: true },
      canvas: {
        base: "#101020",
        layers: [{ paint: { kind: "pattern", pattern: "grid", cell_width: 20, cell_height: 20, foreground: "#112233" }, opacity: 0.5 }],
        bleed: 8,
      },
      effects: [{ kind: "graph", id: "glow", primitives: [{ kind: "gaussian-blur", std_deviation: 2 }] }],
      assets: { aliases: [{ alias: "Diagram", target: "Arial" }], available_sources: ["system"] },
      requirements: { capabilities: ["svg-filters"] },
      typography: { default: { font_stack: ["Arial"], font_weight: 600 } },
      styles: [
        { kind: "rule", family: "sequence", target: "actor", style: { fill: "#112233", effect: "glow" } },
        { kind: "ordinal-palette", target: "series", colors: ["#112233", "#445566"] },
      ],
    },
  };
  const before = structuredClone(original);
  const edited = appendThemeRule(setThemeCanvasBase(original, "white"), {
    family: "class", target: "node",
  }, { stroke: { paint: "#ff00ff", width: 0 } });
  assert.equal(edited.kind, "complete_spec");
  if (edited.kind !== "complete_spec") return;
  assert.deepEqual(original, before);
  assert.deepEqual(edited.complete_spec, {
    ...before.complete_spec,
    canvas: { ...before.complete_spec.canvas, base: "white" },
    styles: [...before.complete_spec.styles!, {
      kind: "rule", family: "class", target: "node", style: { stroke: { paint: "#ff00ff", width: 0 } },
    }],
  });
  const restored = validateThemeDocument(JSON.stringify(edited), (raw) => raw);
  assert.deepEqual(restored.document, edited);
});

test("rule order controls preserve non-rule entries and allow deletion", () => {
  const recipe: ThemeRecipeV1 = {
    schema_version: 1, kind: "complete_spec", complete_spec: {
      styles: [
        { kind: "rule", target: "node", style: { fill: "#111111" } },
        { kind: "ordinal-palette", target: "series", colors: ["#222222"] },
        { kind: "rule", target: "edge", style: { stroke: { paint: "#333333" } } },
      ],
    },
  };
  const moved = moveThemeRule(recipe, 2, "up");
  assert.equal(moved.kind, "complete_spec");
  if (moved.kind !== "complete_spec") return;
  assert.equal(moved.complete_spec.styles?.[1]?.kind, "rule");
  assert.equal(moved.complete_spec.styles?.[2]?.kind, "ordinal-palette");
  const removed = removeThemeRule(moved, 1);
  assert.equal(removed.kind, "complete_spec");
  if (removed.kind !== "complete_spec") return;
  assert.deepEqual(removed.complete_spec.styles?.map((entry) => entry.kind), ["rule", "ordinal-palette"]);
});

test("editing one rule selector field preserves the other selector fields", () => {
  const recipe: ThemeRecipeV1 = {
    schema_version: 1, kind: "definition", definition: {
      authoring_schema_version: 1, expansion_version: 1,
      tokens: {}, styles: [{ kind: "rule", family: "sequence", variant: "actor", target: "node", style: {} }],
    },
  };
  const changed = setThemeRuleSelector(recipe, 0, { target: "edge" });
  assert.equal(changed.kind, "definition");
  if (changed.kind !== "definition") return;
  assert.deepEqual(changed.definition.styles?.[0], { kind: "rule", family: "sequence", variant: "actor", target: "edge", style: {} });
});

test("facet omission, Clear, transparent and zero remain distinct", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "complete_spec", complete_spec: {
      canvas: { layers: [] },
      styles: [{ kind: "rule", target: "node", style: { stroke: { paint: "#123456", width: 2 }, effect: "glow" } }],
    },
  };
  const omitted = setThemeRuleFacet(original, 0, "stroke.paint", undefined);
  const cleared = setThemeRuleFacet(original, 0, "stroke.paint", null);
  const transparent = setThemeRuleFacet(original, 0, "stroke.paint", "transparent");
  const zero = setThemeRuleFacet(original, 0, "stroke.width", 0);
  for (const result of [omitted, cleared, transparent, zero]) {
    assert.equal(result.kind, "complete_spec");
    if (result.kind !== "complete_spec") continue;
    const rule = result.complete_spec.styles![0];
    assert.equal(rule.kind, "rule");
    if (rule.kind === "rule") assert.equal(rule.style.effect, "glow");
  }
  assert.match(JSON.stringify(omitted), /"stroke":\{"width":2\}/);
  assert.match(JSON.stringify(cleared), /"paint":null/);
  assert.match(JSON.stringify(transparent), /"paint":"transparent"/);
  assert.match(JSON.stringify(zero), /"width":0/);
  assert.deepEqual(setThemeCanvasBase(original, undefined), original);
  assert.match(JSON.stringify(setThemeCanvasBase(original, "transparent")), /"base":"transparent"/);
});

test("definition token and typography edits preserve unrelated tokens and styles", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "definition", definition: {
      authoring_schema_version: 1, expansion_version: 1,
      tokens: { text: "#111111", series: ["#222222"], typography: { font_stack: ["Arial"], font_weight: 500 } },
      styles: [{ kind: "rule", target: "node", style: { radius: 0 } }],
    },
  };
  const edited = setDefinitionTypography(setDefinitionToken(original, "text", "#333333"), "font_size_px", 18);
  assert.deepEqual(edited, {
    ...original, definition: {
      ...original.definition,
      tokens: { ...original.definition.tokens, text: "#333333", typography: { font_stack: ["Arial"], font_weight: 500, font_size_px: 18 } },
    },
  });
  const omitted = setDefinitionToken(edited, "series", undefined);
  assert.equal(omitted.kind, "definition");
  if (omitted.kind === "definition") assert.equal("series" in omitted.definition.tokens, false);
});

test("complete typography edits preserve family typography and styles", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "complete_spec", complete_spec: {
      typography: { default: { font_stack: ["Arial"], font_weight: 500 }, families: { sequence: { font_weight: 700 } } },
      styles: [{ kind: "rule", target: "node", style: { radius: 4 } }],
    },
  };
  const edited = setCompleteTypography(original, "font_size_px", 18);
  assert.equal(edited.kind, "complete_spec");
  if (edited.kind !== "complete_spec") return;
  assert.deepEqual(edited.complete_spec.typography, {
    default: { font_stack: ["Arial"], font_weight: 500, font_size_px: 18 },
    families: { sequence: { font_weight: 700 } },
  });
});

test("removing a layered canvas and restoring it keeps intervening style edits", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "complete_spec", complete_spec: {
      canvas: { base: "#123456", layers: [{ paint: "#789abc", opacity: 0.3 }], bleed: 8 },
      styles: [{ kind: "rule", target: "node", style: { fill: "#abcdef" } }],
      mermaid: { theme: "dark" },
    },
  };
  const removed = removeThemeCanvasBackground(original);
  assert.deepEqual(removed, {
    ...original, complete_spec: { ...original.complete_spec, canvas: { base: "transparent", bleed: 8 } },
  });
  const edited = setThemeRuleFacet(removed, 0, "fill", "#fedcba");
  const restored = restoreThemeCanvas(edited, original);
  assert.equal(restored.kind, "complete_spec");
  if (restored.kind !== "complete_spec" || edited.kind !== "complete_spec") return;
  assert.deepEqual(restored.complete_spec.canvas, original.complete_spec.canvas);
  assert.deepEqual(restored.complete_spec.styles, edited.complete_spec.styles);
  assert.equal(original.complete_spec.canvas?.base, "#123456");
});

test("definition default, transparent and restored canvas remain distinct", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "definition", definition: {
      authoring_schema_version: 1, expansion_version: 1, tokens: { text: "#123456" },
    },
  };
  const removed = removeThemeCanvasBackground(original);
  assert.equal(removed.kind, "definition");
  if (removed.kind !== "definition") return;
  assert.equal(removed.definition.tokens.canvas, "transparent");
  assert.deepEqual(setThemeCanvasBase(removed, undefined), original);
  assert.deepEqual(restoreThemeCanvas(removed, original), original);
  assert.throws(() => setThemeCanvasBase(original, { kind: "solid", color: "#123456" }), /require a color/);
});
