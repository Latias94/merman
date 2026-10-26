import assert from "node:assert/strict";
import test from "node:test";
import * as core from "../dist/runtime-core.js";
import { createRenderSession } from "../dist/runtime-render-session.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

function runtime(id) {
  const instances = [];
  class RenderSession {
    constructor(packs) {
      this.packs = JSON.parse(packs);
      this.active = false;
      this.freeCalls = 0;
      instances.push(this);
    }
    renderSvg(source, options) {
      assert.equal(core.packageVersion(), id);
      if (this.active) throw { kind: "reentrant-call" };
      return `${id}:${this.packs[0]?.registrationName ?? "empty"}:${source}:${options}`;
    }
    renderSvgResult(source, options) { return { svg: this.renderSvg(source, options), metadata: {} }; }
    layoutJson(source, options) { return this.renderSvg(source, options); }
    edgeGeometryJson(source, options) { return this.renderSvg(source, options); }
    renderSvgWithTextMeasurer(source, options, measure) {
      const svg = this.renderSvg(source, options);
      this.active = true;
      try { measure({ text: source }); return svg; }
      finally { this.active = false; }
    }
    renderSvgResultWithTextMeasurer(source, options, measure) { return { svg: this.renderSvgWithTextMeasurer(source, options, measure), metadata: {} }; }
    layoutJsonWithTextMeasurer(source, options, measure) { return this.renderSvgWithTextMeasurer(source, options, measure); }
    edgeGeometryJsonWithTextMeasurer(source, options, measure) { return this.renderSvgWithTextMeasurer(source, options, measure); }
    free() {
      assert.equal(core.packageVersion(), id);
      assert.equal(this.active, false, "native borrows end before free");
      this.freeCalls++;
    }
  }
  const module = { default: async () => {}, transportApiVersion: () => core.WEB_TRANSPORT_API_VERSION, packageVersion: () => id, RenderSession };
  const api = bindSurfaceRuntime(async () => module, {
    initMerman: core.initMerman, createRenderSession,
  });
  return { api, instances };
}

test("scoped packs, operation options and package runtime stay with the session", async () => {
  const first = runtime("first");
  const second = runtime("second");
  await first.api.initMerman();
  await second.api.initMerman();
  const packs = [{ json: '{"prefix":"test","icons":{}}', registrationName: "one" }];
  const a = first.api.createRenderSession({ iconPacks: packs });
  const b = second.api.createRenderSession();
  packs[0].registrationName = "mutated";
  assert.match(a.renderSvg("flowchart LR", { svg: { diagram_id: "block-1" } }), /first:one:.*block-1/);
  assert.match(b.layoutJson("flowchart TD"), /^second:empty:/);
  assert.match(a.edgeGeometryJson("flowchart LR"), /^first:one:/);
  if (a.renderSvgResult) assert.match(a.renderSvgResult("flowchart LR").svg, /^first:one:/);
  a.dispose();
  a.dispose();
  b.dispose();
  assert.equal(first.instances[0].freeCalls, 1);
  assert.equal(second.instances[0].freeCalls, 1);
  assert.throws(() => a.renderSvg("flowchart LR"), /render session is disposed/);
});

test("dispose during measurement retires immediately and frees after the active borrow", async () => {
  const { api, instances } = runtime("dispose");
  await api.initMerman();
  const session = api.createRenderSession();
  session.renderSvgWithTextMeasurer("Vehicle", () => {
    session.dispose();
    session.dispose();
    assert.equal(instances[0].freeCalls, 0);
    assert.throws(() => session.layoutJson("classDiagram"), /disposed/);
  });
  assert.equal(instances[0].freeCalls, 1);
});

test("callback reentry rejects without poisoning later operations", async () => {
  const { api } = runtime("reentry");
  await api.initMerman();
  const session = api.createRenderSession();
  session.layoutJsonWithTextMeasurer("Vehicle", () => {
    assert.throws(() => session.renderSvg("classDiagram"), error => error.kind === "reentrant-call");
  });
  assert.match(session.edgeGeometryJson("classDiagram"), /^reentry:/);
  session.dispose();
});
