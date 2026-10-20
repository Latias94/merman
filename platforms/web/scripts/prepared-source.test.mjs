import assert from "node:assert/strict";
import test from "node:test";
import * as core from "../dist/runtime-core.js";
import { prepareSource } from "../dist/runtime-render.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

function runtime(id) {
  const instances = [];
  class PreparedSource {
    constructor(source, options) {
      if (source === "invalid") throw new Error("parse rejected");
      this.source = source;
      this.options = options;
      this.freeCalls = 0;
      this.active = false;
      instances.push(this);
    }
    renderSvg(options) {
      assert.equal(core.packageVersion(), id);
      if (this.active) throw { kind: "reentrant-call" };
      return `${id}:${this.source}:${options}`;
    }
    renderSvgResult(options) { return { svg: this.renderSvg(options), metadata: {} }; }
    layoutJson(options) { return this.renderSvg(options); }
    edgeGeometryJson(options) { return this.renderSvg(options); }
    renderSvgWithTextMeasurer(options, measure) {
      this.active = true;
      try { measure({ text: "hello" }); return `${id}:${options}`; }
      finally { this.active = false; }
    }
    renderSvgResultWithTextMeasurer(options, measure) { return { svg: this.renderSvgWithTextMeasurer(options, measure), metadata: {} }; }
    layoutJsonWithTextMeasurer(options, measure) { return this.renderSvgWithTextMeasurer(options, measure); }
    edgeGeometryJsonWithTextMeasurer(options, measure) { return this.renderSvgWithTextMeasurer(options, measure); }
    free() {
      assert.equal(core.packageVersion(), id);
      assert.equal(this.active, false, "free waits until the native borrow ends");
      this.freeCalls++;
    }
  }
  const module = { default: async () => {}, transportApiVersion: () => core.WEB_TRANSPORT_API_VERSION, packageVersion: () => id, PreparedSource };
  const api = bindSurfaceRuntime(async () => module, { initMerman: core.initMerman, packageVersion: core.packageVersion, prepareSource });
  return { api, instances };
}

test("prepared outputs remain bound to their creating package runtime", async () => {
  const first = runtime("first");
  const second = runtime("second");
  await first.api.initMerman();
  await second.api.initMerman();
  const prepared = first.api.prepareSource("flowchart LR");
  const other = second.api.prepareSource("sequenceDiagram");
  assert.match(prepared.renderSvg({ layout: { container_width: 320 } }), /^first:/);
  assert.match(other.layoutJson(), /^second:/);
  assert.match(prepared.edgeGeometryJson(), /^first:/);
  prepared.dispose();
  prepared.dispose();
  assert.equal(first.instances[0].freeCalls, 1);
  assert.throws(() => prepared.renderSvg(), /prepared source is disposed/);
  other.dispose();
});

test("dispose in a synchronous measurement callback invalidates immediately and frees after output", async () => {
  const { api, instances } = runtime("dispose");
  await api.initMerman();
  const prepared = api.prepareSource("flowchart LR");
  prepared.renderSvgWithTextMeasurer(() => {
    prepared.dispose();
    prepared.dispose();
    assert.equal(instances[0].freeCalls, 0);
    assert.throws(() => prepared.layoutJson(), /disposed/);
  });
  assert.equal(instances[0].freeCalls, 1);
  assert.throws(() => prepared.edgeGeometryJson(), /disposed/);
});

test("callback reentry keeps its classification and does not poison later output", async () => {
  const { api } = runtime("reentry");
  await api.initMerman();
  const prepared = api.prepareSource("flowchart LR");
  prepared.renderSvgWithTextMeasurer(() => {
    assert.throws(() => prepared.renderSvg(), error => error.kind === "reentrant-call");
  });
  assert.match(prepared.renderSvgResult().svg, /^reentry:/);
  prepared.dispose();
});

test("failed preparation publishes no retained instance", async () => {
  const { api, instances } = runtime("failed");
  await api.initMerman();
  assert.throws(() => api.prepareSource("invalid"), /parse rejected/);
  assert.equal(instances.length, 0);
});

test("initialization rejects a stale artifact missing its required snapshot export", async () => {
  const module = { default: async () => {}, transportApiVersion: () => core.WEB_TRANSPORT_API_VERSION };
  const api = bindSurfaceRuntime(async () => module, { initMerman: core.initMerman, isMermanInitialized: core.isMermanInitialized }, undefined, ["PreparedSource"]);
  await assert.rejects(api.initMerman(), /missing required exports: PreparedSource/);
  assert.equal(api.isMermanInitialized(), false);
});
