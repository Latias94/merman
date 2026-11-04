import assert from "node:assert/strict";
import test from "node:test";
import { readMermaidAppearanceOverrides } from "./mermaid-appearance.ts";
import { resolveMermaidCanvasTone } from "./mermaid-canvas-tone.ts";

test("appearance declarations use producer spans even when values match", () => {
  const declaration = (start: number, end: number) => ({
    value: "dark",
    origin: { kind: "directive" as const, directive_index: 0 },
    construct_span: { start, end },
  });
  const facts = {
    state: "known" as const,
    theme: declaration(25, 60),
    font: null,
    look: declaration(90, 125),
  };
  assert.deepEqual(readMermaidAppearanceOverrides(facts, { start: 25, end: 60 }), {
    theme: { value: "dark", origin: "config" },
    font: undefined,
    look: { value: "dark", origin: "source" },
  });
  assert.equal(readMermaidAppearanceOverrides({ state: "unknown" }, null), null);
  assert.equal(readMermaidAppearanceOverrides(undefined, null), null);
});

test("canvas tone reads only the rendered materialized palette", () => {
  for (const theme of ["dark", "neo-dark", "redux-dark", "redux-dark-color"]) {
    assert.equal(resolveMermaidCanvasTone(theme), "dark");
  }
  for (const theme of ["default", "forest", null, undefined, "null", "unknown", { theme: "dark" }]) {
    assert.equal(resolveMermaidCanvasTone(theme), "light");
  }
});
