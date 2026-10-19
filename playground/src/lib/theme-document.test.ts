import assert from "node:assert/strict";
import test from "node:test";
import type { ThemeRecipeV1 } from "@mermanjs/web";

import {
  appendThemeRule,
  removeThemeCanvasBackground,
  restoreThemeCanvas,
  setThemeDesignPath,
  copyThemeReferenceValue,
  themeTokenDomain,
  themeDesignPointer,
  themeTokenFitsDomain,
  setCompleteTypography,
  setThemeCanvasBase,
  setThemeCanvasLayers,
  setThemeRuleFacet,
  setThemeRuleSelector,
  moveThemeRule,
  removeThemeRule,
  validateThemeDocument,
} from "./theme-document.ts";

test("validates original bytes before decoding duplicate fields or unknown versions", () => {
  for (const raw of [
    '{"schema_version":1,"schema_version":2,"kind":"design","design":{}}',
    '{"schema_version":99,"kind":"design","design":{}}',
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
    schema_version: 1, kind: "complete_spec", complete_spec: {
      styles: [{ kind: "rule", family: "sequence", variant: "actor", target: "node", style: {} }],
    },
  };
  const changed = setThemeRuleSelector(recipe, 0, { target: "edge" });
  assert.equal(changed.kind, "complete_spec");
  if (changed.kind !== "complete_spec") return;
  assert.deepEqual(changed.complete_spec.styles?.[0], { kind: "rule", family: "sequence", variant: "actor", target: "edge", style: {} });
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

test("design shared value edits preserve explicit bindings and unrelated styles", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "design", design: {
      tokens: { text: { type: "color", label: "Text", value: "#111111" }, font: { type: "text-style", value: { font_stack: ["Arial"], font_weight: 500 } } },
      styles: { "node-label": { fill: { kind: "solid", color: { ref: "text" } } } },
      typography: { default: { ref: "font" } },
    },
  };
  const edited = setThemeDesignPath(setThemeDesignPath(original.design, ["tokens", "text", "value"], "#333333"), ["tokens", "font", "value", "font_size_px"], 18);
  assert.deepEqual(edited, {
    ...original.design,
    tokens: { ...original.design.tokens, text: { type: "color", label: "Text", value: "#333333" }, font: { type: "text-style", value: { font_stack: ["Arial"], font_weight: 500, font_size_px: 18 } } },
  });
  assert.equal(original.design.tokens?.text.value, "#111111");
  assert.throws(() => setThemeDesignPath(edited, ["typography", "default", "font_size_px"], 20), /local copy/);
  assert.deepEqual(setThemeDesignPath(edited, ["canvas", "layers"], [] ).canvas?.layers, []);
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

test("design omitted, transparent and restored canvas remain distinct", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "design", design: { tokens: { text: { type: "color", value: "#123456" } } },
  };
  const removed = removeThemeCanvasBackground(original);
  assert.equal(removed.kind, "design");
  if (removed.kind !== "design") return;
  assert.equal(removed.design.canvas?.base, "transparent");
  const omitted = setThemeCanvasBase(removed, undefined);
  assert.equal(omitted.kind === "design" && omitted.design.canvas?.base, undefined);
  assert.deepEqual(restoreThemeCanvas(removed, original), original);
  assert.equal(setThemeCanvasBase(original, { kind: "solid", color: "#123456" }).kind, "design");
});

test("local resource copies follow exact aliases and retain nested bindings", () => {
  const design = { tokens: {
    "brand.color": { type: "color", value: "#123456" },
    alias: { type: "color", value: { ref: "brand.color" } },
    paint: { type: "paint", value: { kind: "solid", color: { ref: "alias" } } },
  } } as const;
  assert.equal(copyThemeReferenceValue(design, "alias"), "#123456");
  assert.deepEqual(copyThemeReferenceValue(design, "paint"), { kind: "solid", color: { ref: "alias" } });
  assert.throws(() => copyThemeReferenceValue(design, "brand"), /Unknown theme value/);
  assert.throws(() => copyThemeReferenceValue({ tokens: { a: { type: "color", value: { ref: "b" } }, b: { type: "color", value: { ref: "a" } } } }, "a"), /Circular/);
  assert.equal(themeDesignPointer(["tokens", "a/b~c", "value"]), "/tokens/a~1b~0c/value");
  assert.equal(themeTokenFitsDomain(design, "alias", "color"), true);
  assert.equal(themeTokenFitsDomain(design, "paint", "color"), false);
  const lengths = { tokens: { ratio: { type: "length", value: { percent: 50 } }, pixels: { type: "length", value: 10 } } } as const;
  assert.equal(themeTokenFitsDomain(lengths, "ratio", "length", { value_type: "length", units: ["px"] }), false);
  assert.equal(themeTokenFitsDomain(lengths, "pixels", "length", { value_type: "length", min: 0, max: 8 }), false);
});

test("shared controls intersect Rust consumer limits instead of inventing a global domain", () => {
  const domain = themeTokenDomain("weight", "number", [
    { token: "weight", path: "/styles/node/opacity", domain: { value_type: "number", min: 0, max: 1, units: ["px", "percent"] } },
    { token: "weight", path: "/rules/0/style/opacity", domain: { value_type: "number", min: 0.2, max: 0.8, units: ["px"] } },
    { token: "other", path: "/styles/edge/opacity", domain: { value_type: "number", min: 100 } },
  ]);
  assert.deepEqual(domain, { value_type: "number", min: 0.2, max: 0.8, units: ["px"], choices: undefined });
});

test("canvas layer editing preserves base, bleed, resources, effects and scoped styles", () => {
  const original: ThemeRecipeV1 = {
    schema_version: 1, kind: "complete_spec", complete_spec: {
      canvas: { base: "#123456", bleed: 12, layers: [{ paint: "#abcdef", opacity: 0.3 }] },
      effects: [{ kind: "graph", id: "blur", primitives: [{ kind: "gaussian-blur", std_deviation: 2 }] }],
      styles: [{ kind: "rule", target: "node", family: "class", style: { opacity: 0.5 } }],
    },
  };
  const edited = setThemeCanvasLayers(original, [{ paint: "transparent", offset_x: -2 }]);
  assert.equal(edited.kind, "complete_spec");
  if (edited.kind !== "complete_spec") return;
  assert.deepEqual(edited.complete_spec, { ...original.complete_spec, canvas: { ...original.complete_spec.canvas, layers: [{ paint: "transparent", offset_x: -2 }] } });
  const removed = setThemeCanvasLayers(edited, []);
  assert.equal(removed.kind, "complete_spec");
  if (removed.kind === "complete_spec") assert.deepEqual(removed.complete_spec.canvas, { base: "#123456", bleed: 12 });
  assert.equal(original.complete_spec.canvas?.layers?.[0].paint, "#abcdef");
});

test("renaming shared IDs rewrites explicit aliases and consumers atomically", async () => {
  const { renameThemeToken } = await import("./theme-document.ts");
  const design = { tokens: { ink: { type: "color" as const, value: "red" }, alias: { type: "color" as const, value: { ref: "ink" } } }, styles: { node: { fill: { kind: "solid" as const, color: { ref: "ink" } } } } };
  const changed = renameThemeToken(design, "ink", "brand");
  assert.equal(Object.hasOwn(changed.tokens!, "ink"), false);
  assert.deepEqual(changed.tokens!.alias.value, { ref: "brand" });
  assert.equal(JSON.stringify(changed).includes('"ref":"ink"'), false);
  assert.equal(JSON.stringify(design).includes('"ref":"ink"'), true);
  assert.throws(() => renameThemeToken(design, "ink", "alias"));
  assert.throws(() => renameThemeToken(design, "ink", "x".repeat(129)));
});

test("open theme IDs remain own properties without mutating object prototypes", async () => {
  const { setThemeDesignPath, renameThemeToken } = await import("./theme-document.ts");
  const next = setThemeDesignPath({}, ["tokens", "__proto__"], { type: "color", value: "red" });
  assert.equal(Object.hasOwn(next.tokens!, "__proto__"), true);
  const renamed = renameThemeToken(next, "__proto__", "constructor");
  assert.equal(Object.hasOwn(renamed.tokens!, "constructor"), true);
  assert.equal(Object.hasOwn(Object.prototype, "type"), false);
});
