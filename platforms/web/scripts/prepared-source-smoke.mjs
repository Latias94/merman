import assert from "node:assert/strict";
import * as core from "../dist/runtime-core.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

/** Exercise the built class through the public runtime and the actual wasm-bindgen owner. */
export async function preparedSourceSmoke(api, raw, measurementResult) {
  const { PreparedSource: omitted, ...stale } = raw;
  void omitted;
  const wrongRuntime = bindSurfaceRuntime(async () => stale, { initMerman: core.initMerman, isMermanInitialized: core.isMermanInitialized }, undefined, ["PreparedSource"]);
  await assert.rejects(wrongRuntime.initMerman(), /missing required exports: PreparedSource/);
  assert.equal(wrongRuntime.isMermanInitialized(), false);
  assert.equal(typeof raw.PreparedSource, "function");
  const source = "flowchart LR\nA[Retained] --> B[Output]";
  const options = { site_config: { flowchart: { htmlLabels: false } }, svg: { diagram_id: "prepared-source" } };
  const expiredOptions = { timeout_ms: 0, parse: { suppress_errors: "invalid" } };
  for (const run of [() => api.renderSvg(source, expiredOptions), () => api.prepareSource(source, expiredOptions)]) {
    assert.throws(run, error => error.code_name === "MERMAN_CANCELLED");
  }
  const prepared = api.prepareSource(source, options);
  const first = prepared.renderSvg();
  assert.equal(first, api.renderSvg(source, options));
  assert.equal(prepared.renderSvgResult().svg, first);
  assert.deepEqual(JSON.parse(prepared.layoutJson()), JSON.parse(api.layoutJson(source, options)));
  assert.deepEqual(JSON.parse(prepared.edgeGeometryJson()), JSON.parse(api.edgeGeometryJson(source, options)));
  assert.throws(() => prepared.renderSvg({ timeout_ms: 0 }), error => error.code_name === "MERMAN_CANCELLED");
  assert.equal(prepared.renderSvg(), first);
  assert.throws(() => prepared.renderSvg({ resources: { limits: { max_source_bytes: 1 } } }), error => error.code_name === "MERMAN_RESOURCE_LIMIT_EXCEEDED");
  assert.equal(prepared.renderSvg(), first);
  for (const frozen of [{ parse: { suppress_errors: true } }, { site_config: {} }, { fixed_today: "2030-01-01" }, { fixed_local_offset_minutes: 60 }, { theme: null }]) {
    assert.throws(() => prepared.renderSvg(frozen), error => error.code_name === "MERMAN_OPTIONS_JSON_ERROR");
  }
  let measured = 0;
  const fallback = () => { measured++; return undefined; };
  assert.match(prepared.renderSvgWithTextMeasurer(fallback), /<svg/);
  assert.match(prepared.renderSvgResultWithTextMeasurer(fallback).svg, /<svg/);
  assert.equal(typeof prepared.layoutJsonWithTextMeasurer(fallback), "string");
  assert.equal(typeof prepared.edgeGeometryJsonWithTextMeasurer(fallback), "string");
  assert.ok(measured > 0);
  const narrowMetrics = prepared.layoutJsonWithTextMeasurer(measurementResult);
  const wideMetrics = prepared.layoutJsonWithTextMeasurer(request => {
    const result = measurementResult(request);
    for (const field of ["width", "length", "raw_width", "bbox_left", "bbox_right"]) {
      if (typeof result[field] === "number") result[field] *= 2;
    }
    return result;
  });
  assert.notEqual(narrowMetrics, wideMetrics, "each output uses its selected host font metrics");

  let reentered = false;
  prepared.renderSvgWithTextMeasurer(() => {
    if (!reentered) {
      reentered = true;
      assert.throws(() => prepared.renderSvg(), error => error.kind === "reentrant-call");
    }
    return undefined;
  });
  assert.equal(reentered, true);
  assert.equal(prepared.renderSvg(), first);
  prepared.dispose();
  prepared.dispose();
  assert.throws(() => prepared.renderSvg(), /disposed/);

  const disposable = api.prepareSource(source, options);
  let disposed = false;
  assert.match(disposable.renderSvgWithTextMeasurer(() => {
    if (!disposed) {
      disposed = true;
      disposable.dispose();
      assert.throws(() => disposable.renderSvg(), /disposed/);
    }
    return undefined;
  }), /<svg/);
  assert.equal(disposed, true);
  disposable.dispose();
  assert.throws(() => disposable.layoutJson(), /disposed/);
  assert.throws(() => api.prepareSource("flowchart LR\nA -->"), error => error.code_name === "MERMAN_PARSE_ERROR");
}
