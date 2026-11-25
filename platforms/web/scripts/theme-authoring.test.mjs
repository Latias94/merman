import assert from "node:assert/strict";
import test from "node:test";

import * as web from "../dist/index.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";
import { customizeNodeColors } from "../examples/custom-theme.mjs";
import { webPackages } from "./surface-manifest.mjs";

const source = "flowchart LR\nA --> B";
const studio = { schema_version: 2, id: "studio", appearances: { light: {}, dark: {} } };

async function themeRuntime(exports) {
  const runtime = bindSurfaceRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => web.WEB_TRANSPORT_API_VERSION,
    ...exports,
  }), {
    initMerman: web.initMerman,
    exportThemePreset: web.exportThemePreset,
    resolveTheme: web.resolveTheme,
    renderSvg: web.renderSvg,
    renderSvgResult: web.renderSvgResult,
    renderSvgResultWithTextMeasurer: web.renderSvgResultWithTextMeasurer,
  });
  await runtime.initMerman();
  return runtime;
}

test("preset export takes an ID and returns a document accepted as SVG options", async () => {
  const calls = [];
  const web = await themeRuntime({
    exportThemePreset(...args) {
      calls.push(args);
      return studio;
    },
    renderSvg(...args) {
      calls.push(args);
      return "<svg/>";
    },
  });
  const restored = JSON.parse(JSON.stringify(web.exportThemePreset("studio")));
  const options = { theme: restored, background: "transparent" };
  assert.equal(web.renderSvg(source, options), "<svg/>");
  assert.deepEqual(restored, studio);
  assert.deepEqual(calls, [["studio"], [source, JSON.stringify(options)]]);
});

test("theme resolution encodes the options alongside the document and returns Rust's result", async () => {
  const requests = [];
  const success = { ok: true, resolved: { appearance: "dark" }, notices: [] };
  const diagnostic = { ok: false, line: 3, column: 14, path: ".", message: "invalid JSON" };
  const web = await themeRuntime({
    resolveTheme(requestJson) {
      const request = JSON.parse(requestJson);
      requests.push(request);
      return typeof request.document === "string" ? diagnostic : success;
    },
  });
  const options = { appearance: "dark", auto_fix_contrast: true };
  assert.equal(web.resolveTheme(studio, options), success);
  assert.equal(web.resolveTheme(studio), success);
  const raw = '{\r\n  "schema_version": 2,\r\n  "extends": ]\r\n}\r\n';
  assert.equal(web.resolveTheme(raw), diagnostic);
  assert.equal(web.resolveTheme('{"schema_version":2,"schema_version":2}'), diagnostic);
  assert.deepEqual(requests, [
    { document: studio, ...options }, { document: studio }, { document: raw },
    { document: '{"schema_version":2,"schema_version":2}' },
  ]);
});

test("object encoders reject non-finite theme values and keep explicit null site config", async () => {
  const calls = [];
  const web = await themeRuntime({
    resolveTheme: (...args) => calls.push(args),
    renderSvg: (...args) => calls.push(args),
  });
  for (const radius of [NaN, Infinity, -Infinity, { toJSON: () => NaN }]) {
    const document = { schema_version: 2, extends: "studio", form: { radius } };
    assert.throws(() => web.resolveTheme(document), /theme resolution request contains a non-finite number/);
    assert.throws(() => web.renderSvg(source, { theme: document }), /options contains a non-finite number/);
  }
  assert.deepEqual(calls, []);
  const options = { theme: "none", site_config: { fixed_today: null } };
  web.renderSvg(source, options);
  assert.deepEqual(calls, [[source, JSON.stringify(options)]]);
});

test("SVG result facades return the same execution's theme outcome", async () => {
  const calls = [];
  const result = {
    svg: "<svg/>",
    metadata: { version: 1, theme_outcome: {
      mode: "merman_native", preset_id: "studio", appearance: "dark",
      notices: [{ kind: "contrast_adjusted", foreground: "text.title", color: "#F8FAFC" }],
    } },
  };
  const web = await themeRuntime({
    renderSvgResult(...args) { calls.push(args); return result; },
    renderSvgResultWithTextMeasurer(...args) { calls.push(args); return result; },
  });
  const options = { theme: { preset: "studio", appearance: "dark", precedence: "source" } };
  const callback = () => null;
  assert.equal(web.renderSvgResult(source, options), result);
  assert.equal(web.renderSvgResultWithTextMeasurer(source, callback, options), result);
  assert.deepEqual(calls, [
    [source, JSON.stringify(options)], [source, JSON.stringify(options), callback],
  ]);
});

test("the full and render packages expose both theme authoring operations", () => {
  for (const descriptor of webPackages) {
    const expected = descriptor.id === "full" || descriptor.id === "render";
    for (const name of ["exportThemePreset", "resolveTheme"]) {
      assert.equal(typeof web[name], "function");
      assert.equal(descriptor.runtimeExportNames.includes(name), expected, descriptor.name);
      assert.equal(descriptor.wasmExportNames.includes(name), expected, descriptor.name);
    }
    assert.ok(descriptor.runtimeExportNames.includes("themeCatalog"));
    assert.ok(descriptor.wasmExportNames.includes("themeCatalog"));
  }
});

test("customization keeps other appearances and family overrides intact", () => {
  const original = {
    ...studio,
    roles: { "stroke.border": "#111111" },
    appearances: { ...studio.appearances, dark: {
      roles: { canvas: "#121212" }, canvas: { padding_px: 8 },
    } },
    overrides: [{ families: ["pie"], form: { chart: { pie_gap_px: 2 } } }],
  };
  const before = structuredClone(original);
  const result = customizeNodeColors(original, "dark", {
    background: "#142535", nodeBorder: "#FB7185", nodeFill: "#22354D",
  });
  assert.deepEqual(original, before);
  assert.deepEqual(result, { ...original, appearances: { ...original.appearances, dark: {
    canvas: { padding_px: 8 },
    roles: { canvas: "#142535", "stroke.border": "#FB7185", "surface.node": "#22354D" },
  } } });
});
