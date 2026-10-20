import assert from "node:assert/strict";
import test from "node:test";
import * as api from "../dist/index.js";

test("the public render surface exposes persistent mounting", () => {
  assert.equal(typeof api.mountDiagram, "function");
});
import * as core from "../dist/runtime-core.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";
import { mountDiagram } from "../dist/runtime-mount.js";

function documentFixture() {
  let nextFrame = 0;
  const frames = new Map();
  const observers = [];
  const listeners = new Map();
  let ready;
  const readyPromise = new Promise(resolve => { ready = resolve; });
  const document = {
    URL: "https://example.com/diagram", baseURI: "https://example.com/diagram",
    fonts: {
      ready: readyPromise,
      addEventListener(name, callback) { (listeners.get(name) ?? listeners.set(name, new Set()).get(name)).add(callback); },
      removeEventListener(name, callback) { listeners.get(name)?.delete(callback); },
    },
    importNode(root) { return { ...root, ownerDocument: document, querySelectorAll: () => [] }; },
  };
  document.defaultView = {
    crypto: globalThis.crypto,
    getComputedStyle: () => ({ paddingLeft: "0", paddingRight: "0" }),
    DOMParser: class {
      parseFromString(svg) { return { querySelector: () => null, documentElement: { localName: "svg", outerHTML: svg } }; }
    },
    XMLSerializer: class { serializeToString(root) { return root.outerHTML; } },
    ResizeObserver: class {
      constructor(callback) { this.callback = callback; observers.push(this); }
      observe(target) { this.target = target; }
      disconnect() { this.disconnected = true; }
    },
    requestAnimationFrame(callback) { frames.set(++nextFrame, callback); return nextFrame; },
    cancelAnimationFrame(id) { frames.delete(id); },
  };
  return {
    document, observers, listeners, ready,
    flush() { const pending = [...frames.values()]; frames.clear(); for (const callback of pending) callback(); },
    resize(target, width) { target.width = width; for (const observer of observers) observer.callback([{ target, contentRect: { width } }]); },
    fonts() { for (const callback of [...listeners.get("loadingdone") ?? []]) callback(); },
    target(width = 640) { return { ownerDocument: document, width, isConnected: true, children: [], get clientWidth() { return this.width; }, getBoundingClientRect() { return { width: this.width }; }, replaceChildren(...children) { this.children = children; } }; },
  };
}

async function runtime(id = "test") {
  const instances = [];
  class PreparedSource {
    constructor(source, options) {
      if (source === "invalid") throw { kind: "parse-error" };
      this.source = source; this.options = options; this.freeCalls = 0; this.renders = 0; instances.push(this);
    }
    renderSvgResult(options) {
      assert.equal(core.packageVersion(), id);
      this.renders++;
      if (this.source === "layout-error") throw { kind: "layout-error" };
      if (JSON.parse(options).resources?.limits?.max_svg_bytes === 1) throw { kind: "resource-limit" };
      return { svg: this.source === "unsafe" ? '<svg><script/></svg>' : `<svg><text>${this.source}</text></svg>`, metadata: { options: JSON.parse(options) } };
    }
    renderSvgResultWithTextMeasurer(options, measure) {
      this.active = true;
      try { measure({ text: this.source }); return this.renderSvgResult(options); }
      finally { this.active = false; }
    }
    free() { assert.equal(this.active ?? false, false); assert.equal(core.packageVersion(), id); this.freeCalls++; }
  }
  const module = { default: async () => {}, transportApiVersion: () => core.WEB_TRANSPORT_API_VERSION, packageVersion: () => id, PreparedSource };
  const api = bindSurfaceRuntime(async () => module, { initMerman: core.initMerman, mountDiagram });
  await api.initMerman();
  return { api, instances };
}

test("unchanged parse inputs reuse the snapshot; failures retain the last published drawing", async () => {
  const { api, instances } = await runtime();
  const realm = documentFixture(); const target = realm.target();
  const mounted = api.mountDiagram(target, "first", { outputOptions: { layout: { adaptation: "fit-width" } } });
  const first = mounted.result; const root = target.children[0];
  assert.equal(mounted.status, "ready");
  mounted.refresh(); realm.resize(target, 320); realm.flush();
  assert.equal(instances.length, 1);
  assert.equal(mounted.result.metadata.options.layout.container_width, 320);
  for (const source of ["invalid", "layout-error", "unsafe"]) {
    mounted.update(source);
    assert.equal(mounted.source, source); assert.equal(mounted.status, "failed");
    assert.equal(target.children[0].outerHTML, root.outerHTML);
    assert.ok(mounted.error);
  }
  mounted.update("first");
  assert.equal(instances.filter(value => value.source === "first").length, 1);
  mounted.update("first", { prepareOptions: { site_config: { fontFamily: "serif" } } });
  assert.equal(instances.filter(value => value.source === "first").length, 2);
  assert.notEqual(mounted.result, first);
  mounted.destroy(); mounted.destroy();
  assert.equal(mounted.status, "destroyed"); assert.ok(target.children.length);
  assert.throws(() => mounted.refresh(), /destroyed/);
  assert.ok(instances.every(value => value.freeCalls === 1));
});

test("hidden requests defer preparation and capture caller options before scheduled redraw", async () => {
  const { api, instances } = await runtime(); const realm = documentFixture(); const target = realm.target(0);
  const options = { prepareOptions: { site_config: { fontFamily: "serif" } }, outputOptions: { svg: { viewbox_padding: 7 }, layout: { adaptation: "fit-width" } } };
  const mounted = api.mountDiagram(target, "old", options);
  options.prepareOptions.site_config.fontFamily = "mutated"; options.outputOptions.svg.viewbox_padding = 99;
  mounted.update("new");
  assert.equal(mounted.status, "pending"); assert.equal(mounted.source, "new"); assert.equal(instances.length, 0);
  realm.resize(target, 320); realm.flush();
  assert.equal(instances.length, 1);
  assert.equal(JSON.parse(instances[0].options).site_config.fontFamily, "serif");
  assert.equal(mounted.result.metadata.options.svg.viewbox_padding, 7);
  mounted.destroy();
});

test("measurement reentry invalidates old output and remount/destroy suppress stale work", async () => {
  const { api, instances } = await runtime(); const realm = documentFixture(); const target = realm.target();
  const callbacks = [];
  const mounted = api.mountDiagram(target, "first");
  mounted.update("interrupted", { textMeasurement: () => mounted.update("new", { onRender: value => callbacks.push(value.svg) }), onRender: () => assert.fail("stale callback") });
  assert.equal(target.children[0].outerHTML, '<svg><text>first</text></svg>');
  realm.flush();
  assert.equal(mounted.source, "new"); assert.equal(callbacks.length, 1);
  mounted.update("destroyed", { textMeasurement: () => mounted.destroy(), onRender: () => assert.fail("destroyed callback") });
  assert.equal(mounted.status, "destroyed"); realm.flush();
  assert.match(target.children[0].outerHTML, /new/);
  assert.ok(instances.every(value => value.freeCalls === 1));
  const next = api.mountDiagram(target, "next", { textMeasurement: () => undefined });
  realm.fonts();
  const replacement = api.mountDiagram(target, "replacement");
  realm.flush(); realm.fonts(); realm.flush();
  assert.equal(next.status, "destroyed"); assert.equal(replacement.source, "replacement");
  assert.match(target.children[0].outerHTML, /replacement/); replacement.destroy();
});

test("callbacks cannot roll back newer publications and callback failures retain successful output", async () => {
  const { api } = await runtime(); const realm = documentFixture(); const target = realm.target();
  const mounted = api.mountDiagram(target, "first");
  mounted.update("second", { onRender: (_result, controller) => { controller.update("third", {}); throw new Error("obsolete callback"); } });
  assert.equal(mounted.status, "ready"); assert.equal(mounted.error, null); assert.match(mounted.result.svg, /third/);
  mounted.update("fourth", { onRender: () => { throw new Error("callback failed"); } });
  assert.match(mounted.result.svg, /fourth/); assert.match(target.children[0].outerHTML, /fourth/); assert.match(mounted.error.message, /callback failed/);
  mounted.update("invalid", { onError: (_error, controller) => controller.update("recovered", {}) });
  realm.flush();
  assert.match(mounted.result.svg, /recovered/); assert.equal(mounted.status, "ready"); mounted.destroy();
});

test("adoption refresh rebinds document listeners and captured package runtime stays isolated", async () => {
  const first = await runtime("first"); const second = await runtime("second");
  const oldRealm = documentFixture(); const newRealm = documentFixture(); const target = oldRealm.target();
  const mounted = first.api.mountDiagram(target, "source", { textMeasurement: () => undefined });
  target.ownerDocument = newRealm.document;
  mounted.refresh();
  const count = first.instances[0].renders;
  oldRealm.fonts(); oldRealm.ready(); await Promise.resolve(); oldRealm.flush(); assert.equal(first.instances[0].renders, count);
  newRealm.fonts(); newRealm.flush(); assert.equal(first.instances.length, 1); assert.equal(first.instances[0].renders, count + 1);
  assert.equal(oldRealm.observers[0].disconnected, true); assert.equal(oldRealm.listeners.get("loadingdone").size, 0);
  const other = second.api.mountDiagram(newRealm.target(), "other");
  assert.notEqual(mounted.result.metadata.options.svg.diagram_id, other.result.metadata.options.svg.diagram_id);
  mounted.destroy(); other.destroy();
  newRealm.ready(); await Promise.resolve(); newRealm.flush();
  assert.equal(mounted.status, "destroyed");
});


test("rejected option capture cannot later render with superseded settings", async () => {
  const { api, instances } = await runtime(); const realm = documentFixture(); const target = realm.target();
  const mounted = api.mountDiagram(target, "good", { textMeasurement: () => undefined });
  mounted.update("rejected", { outputOptions: { layout: { container_width: NaN } } });
  assert.equal(mounted.status, "failed"); assert.ok(mounted.error instanceof TypeError);
  realm.fonts(); realm.flush(); mounted.refresh();
  assert.equal(instances.length, 1); assert.match(target.children[0].outerHTML, /good/);
  mounted.update("recovered", {}); assert.match(mounted.result.svg, /recovered/); mounted.destroy();
});
