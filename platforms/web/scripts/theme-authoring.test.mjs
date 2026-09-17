import assert from "node:assert/strict";
import test from "node:test";

import { initMerman } from "../dist/runtime-core.js";
import {
  describeThemeSupport,
  exportThemePreset,
  materializeTheme,
  renderSvg,
} from "../dist/runtime-render.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

test("theme authoring rejects non-finite numbers before they become Clear", async () => {
  const calls = [];
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      transportApiVersion: () => 5,
      materializeTheme(definition, options) {
        calls.push({ definition, options });
        return { schema_version: 1 };
      },
      describeThemeSupport(query, options) {
        calls.push({ query, options });
        return { schema_version: 1 };
      },
    }),
    { initMerman, materializeTheme, describeThemeSupport },
  );
  await runtime.initMerman();

  for (const value of [NaN, Infinity, -Infinity]) {
    assert.throws(
      () => runtime.materializeTheme({
        authoring_schema_version: 1,
        expansion_version: 1,
        tokens: {},
        styles: [{ kind: "rule", target: "text", style: { opacity: value } }],
      }),
      { name: "TypeError", message: /theme definition contains a non-finite number/ },
    );
    assert.throws(
      () => runtime.describeThemeSupport({
        schema_version: value,
        family: "pie",
        output: "standalone-svg",
        target: "text",
        facet: "fill",
      }),
      { name: "TypeError", message: /theme support query contains a non-finite number/ },
    );
    assert.throws(
      () => runtime.materializeTheme({
        authoring_schema_version: 1,
        expansion_version: 1,
        tokens: {},
        styles: [{
          kind: "rule",
          target: "text",
          style: { opacity: { toJSON: () => value } },
        }],
      }),
      TypeError,
    );
  }
  assert.deepEqual(calls, [], "invalid inputs must not reach the WASM transport");

  for (const opacity of [null, 0, 0.5, 1]) {
    const definition = {
      authoring_schema_version: 1,
      expansion_version: 1,
      tokens: {},
      styles: [{ kind: "rule", target: "text", style: { opacity } }],
    };
    runtime.materializeTheme(definition, { theme: "dark" });
    assert.deepEqual(calls.at(-1), {
      definition: JSON.stringify(definition),
      options: JSON.stringify({ theme: "dark" }),
    });
  }
  const raw = '{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}';
  runtime.materializeTheme(raw);
  assert.equal(calls.at(-1).definition, raw);
});


test("ordinary SVG options reject non-finite numbers while preserving explicit null", async () => {
  const calls = [];
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      transportApiVersion: () => 5,
      renderSvg(source, options) {
        calls.push({ source, options });
        return "<svg/>";
      },
    }),
    { initMerman, renderSvg },
  );
  await runtime.initMerman();

  for (const value of [NaN, Infinity, -Infinity]) {
    assert.throws(
      () => runtime.renderSvg("flowchart LR\nA --> B", {
        version: 3,
        site_config: { fixed_today: value },
      }),
      { name: "TypeError", message: /options contains a non-finite number/ },
    );
  }
  assert.deepEqual(calls, [], "invalid ordinary options must not reach the WASM transport");

  runtime.renderSvg("flowchart LR\nA --> B", {
    version: 3,
    theme: null,
    site_config: { fixed_today: null },
  });
  assert.deepEqual(calls.at(-1), {
    source: "flowchart LR\nA --> B",
    options: JSON.stringify({
      version: 3,
      theme: null,
      site_config: { fixed_today: null },
    }),
  });
});

test("exported recipes survive JSON storage and enter SVG options without unpacking", async () => {
  const calls = [];
  const recipe = {
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: { styles: [] },
  };
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      transportApiVersion: () => 5,
      exportThemePreset(presetId, options) {
        calls.push({ presetId, options });
        return structuredClone(recipe);
      },
      renderSvg(source, options) {
        calls.push({ source, options });
        return "<svg/>";
      },
    }),
    { initMerman, exportThemePreset, renderSvg },
  );
  await runtime.initMerman();

  const stored = JSON.stringify(runtime.exportThemePreset("editor-light"));
  const restored = JSON.parse(stored);
  runtime.renderSvg("flowchart LR\nA --> B", { theme: restored });
  assert.deepEqual(calls, [
    { presetId: "editor-light", options: undefined },
    {
      source: "flowchart LR\nA --> B",
      options: JSON.stringify({ theme: recipe }),
    },
  ]);
});
