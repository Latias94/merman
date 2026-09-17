import assert from "node:assert/strict";
import test from "node:test";

import { initMerman } from "../dist/runtime-core.js";
import {
  describeThemeSupport,
  exportThemePreset,
  materializeTheme,
  renderSvg,
  renderSvgResult,
  renderSvgResultWithTextMeasurer,
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


test("SVG results preserve opaque evidence and forward policy through one execution", async () => {
  const calls = [];
  let evidence = { version: 1, theme_status: "future-theme", target_status: "future-target" };
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      transportApiVersion: () => 5,
      renderSvgResult(source, options) {
        calls.push({ source, options });
        return { svg: "<svg>用户</svg>", metadata: { version: 1, theme_execution_evidence: evidence } };
      },
      renderSvgResultWithTextMeasurer(source, options, callback) {
        calls.push({ source, options, callback });
        return { svg: "<svg/>", metadata: { version: 1, theme_execution_evidence: evidence } };
      },
    }),
    { initMerman, renderSvgResult, renderSvgResultWithTextMeasurer },
  );
  await runtime.initMerman();
  for (const value of [
    evidence,
    { version: 99, future: { target_status: "portable" } },
    { version: 1, theme_status: "verified", target_status: "future-target" },
  ]) {
    evidence = value;
    const result = runtime.renderSvgResult("classDiagram\nclass Account", {
      environment: { theme_portability: "best-effort" },
    });
    assert.deepEqual(result.metadata.theme_execution_evidence, value);
    const outcome = result.metadata.theme_execution_evidence;
    assert.equal(outcome.version === 1 && outcome.theme_status === "verified"
      && outcome.target_status === "portable", false, "unknown evidence must not grant portability");
  }
  assert.equal(calls.length, 3, "each result must come from one render operation");
  const callback = () => null;
  runtime.renderSvgResultWithTextMeasurer("flowchart LR\nA", callback, {
    environment: { theme_portability: "require-portable" },
  });
  assert.deepEqual(calls.at(-1), {
    source: "flowchart LR\nA", callback,
    options: JSON.stringify({ environment: { theme_portability: "require-portable" } }),
  });
  assert.throws(() => runtime.renderSvgResult("classDiagram\nclass Account", {
    theme: { spec: { typography: { default: { font_size_px: NaN } } } },
  }), TypeError);
  assert.equal(calls.length, 4, "invalid input must not reach the result transport");
});
