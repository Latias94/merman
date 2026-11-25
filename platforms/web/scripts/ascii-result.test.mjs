import assert from "node:assert/strict";
import test from "node:test";

import * as webApi from "../dist/index.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";
import { webPackages } from "./surface-manifest.mjs";

test("ASCII result facade preserves canonical metadata and encodes options once", async () => {
  const expected = asciiResult({
    primary_width: 5,
    primary_height: 1,
    emitted_width: 5,
    emitted_height: 1,
    outcome: "primary",
  }, "A → B");
  expected.metadata.future_metadata = { preserved: true };
  const { api, calls } = await asciiRuntime({ result: expected });
  const options = {
    version: 4,
    theme: { preset: "studio", appearance: "dark" },
    ascii: { color_mode: "truecolor", graph_presentation: "strict", max_width: 80 },
    timeout_ms: 125,
  };

  assert.equal(api.renderAsciiResult("source owned by WASM", options), expected);
  const rawOptions = '{ "ascii": { "graphPresentation": "adaptive" } }';
  assert.equal(api.renderAscii("source owned by WASM", rawOptions), expected.text);
  assert.deepEqual(calls, [
    { method: "result", source: "source owned by WASM", options: JSON.stringify(options) },
    { method: "text", source: "source owned by WASM", options: rawOptions },
  ]);
});

test("ASCII results retain null primary geometry and the canonical planning reason", async () => {
  for (const reason of ["routing_search_exhausted", "port_capacity_exceeded"]) {
    const expected = asciiResult({
      primary_width: null,
      primary_height: null,
      layout_profile: null,
      primary_overflowed: null,
      requested_graph_presentation: "adaptive",
      requested_layout_profile: "auto",
      requested_max_width: 80,
      compact_attempted: true,
      projection: "structured_text",
      outcome: "fallback",
      fallback_capability: "available",
      fallback_attempted: true,
      fallback_reason: reason,
      emitted_width: 12,
      emitted_height: 2,
      lossiness: "presentation_only",
    }, "N1: Alpha\nE1: N1 -> N1");
    const { api, calls } = await asciiRuntime({ result: expected });
    const result = api.renderAsciiResult("ignored by the facade");
    assert.equal(result, expected);
    assert.equal(result.metadata.version, 1);
    assert.equal(result.metadata.output_plan.schema_version, 4);
    assert.equal(result.metadata.output_plan.fallback_reason, reason);
    assert.equal(result.metadata.output_plan.primary_width, null);
    assert.equal(result.metadata.output_plan.primary_height, null);
    assert.equal(result.metadata.output_plan.layout_profile, null);
    assert.equal(result.metadata.output_plan.primary_overflowed, null);
    assert.equal(result.metadata.output_plan.emitted_overflowed, false);
    assert.equal("overflowed" in result.metadata.output_plan, false);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].options, undefined);
  }
});

test("a real empty primary keeps measured zero dimensions", async () => {
  const { api } = await asciiRuntime();
  const { text, metadata } = api.renderAsciiResult("flowchart LR");
  assert.equal(text, "");
  assert.equal(metadata.byte_length, 0);
  assert.equal(metadata.output_plan.primary_width, 0);
  assert.equal(metadata.output_plan.primary_height, 0);
  assert.equal(metadata.output_plan.primary_overflowed, false);
  assert.equal(metadata.output_plan.layout_profile, "canonical");
  assert.equal(metadata.output_plan.requested_graph_presentation, "strict");
  assert.equal(metadata.output_plan.outcome, "empty");
});

test("ASCII capability copies preserve presentation boundaries without exposing the cache", async () => {
  const capabilities = [
    capability("flowchart", ["strict", "adaptive"], ["canonical", "compact", "auto"]),
    capability("swimlane", ["strict", "adaptive"]),
    capability("state", ["strict", "adaptive"]),
    capability("class", ["strict"]),
    capability("zenuml", []),
  ];
  const first = await asciiRuntime({ capabilities });
  const copy = first.api.asciiCapabilities();
  assert.deepEqual(copy.map(({ graph_presentations }) => graph_presentations), [
    ["strict", "adaptive"], ["strict", "adaptive"], ["strict", "adaptive"], ["strict"], [],
  ]);
  assert.deepEqual(copy[2].layout_profiles, ["canonical"]);
  assert.deepEqual(first.api.asciiDiagrammaticDiagrams(), ["flowchart", "swimlane", "state", "class"]);

  copy[0].graph_presentations.length = 0;
  copy[2].layout_profiles.push("compact");
  copy[0].evidence[0].note = "caller mutation";
  capabilities[0].graph_presentations.length = 0;
  const fresh = first.api.asciiCapabilities();
  assert.deepEqual(fresh[0].graph_presentations, ["strict", "adaptive"]);
  assert.deepEqual(fresh[2].layout_profiles, ["canonical"]);
  assert.equal(fresh[0].evidence[0].note, "canonical evidence");
  assert.equal(first.calls.filter(({ method }) => method === "capabilities").length, 1);

  const second = await asciiRuntime({ capabilities: [capability("class", ["strict"])] });
  assert.deepEqual(second.api.asciiCapabilities().map(({ diagram_type }) => diagram_type), ["class"]);
  assert.deepEqual(first.api.asciiCapabilities()[0].graph_presentations, ["strict", "adaptive"]);
});

test("ASCII capability normalization rejects missing or unknown presentation policies", async () => {
  for (const presentations of [undefined, null, "strict", ["best_effort"]]) {
    const value = capability("flowchart", presentations);
    const { api } = await asciiRuntime({ capabilities: [value] });
    assert.throws(() => api.asciiCapabilities(), /ASCII graph presentation/);
  }
});

test("ASCII errors propagate from the canonical operation without publishing a result", async () => {
  const error = { code_name: "MERMAN_INVALID_ARGUMENT", kind: "generic", message: "invalid presentation" };
  const { api } = await asciiRuntime({ error });
  assert.throws(
    () => api.renderAsciiResult("stateDiagram-v2", { ascii: { layoutProfile: "compact" } }),
    (actual) => actual === error,
  );
});

test("only full and ASCII package profiles expose the ASCII result facade", () => {
  assert.equal(typeof webApi.renderAsciiResult, "function");
  for (const descriptor of webPackages) {
    const expected = descriptor.id === "full" || descriptor.id === "ascii";
    assert.equal(descriptor.runtimeExportNames.includes("renderAsciiResult"), expected, descriptor.name);
    assert.equal(descriptor.wasmExportNames.includes("renderAsciiResult"), expected, descriptor.name);
  }
});

async function asciiRuntime({ result = asciiResult(), capabilities = [], error } = {}) {
  const calls = [];
  const api = bindSurfaceRuntime(async () => ({
    default: async () => {},
    transportApiVersion: () => webApi.WEB_TRANSPORT_API_VERSION,
    renderAsciiResult(source, options) {
      calls.push({ method: "result", source, options });
      if (error) throw error;
      return result;
    },
    renderAscii(source, options) {
      calls.push({ method: "text", source, options });
      if (error) throw error;
      return result.text;
    },
    asciiCapabilities() {
      calls.push({ method: "capabilities" });
      return capabilities;
    },
  }), {
    initMerman: webApi.initMerman,
    renderAscii: webApi.renderAscii,
    renderAsciiResult: webApi.renderAsciiResult,
    asciiCapabilities: webApi.asciiCapabilities,
    asciiDiagrammaticDiagrams: webApi.asciiDiagrammaticDiagrams,
  });
  await api.initMerman();
  return { api, calls };
}

function asciiResult(plan = {}, text = "") {
  return {
    text,
    metadata: {
      version: 1,
      operation_id: "ascii",
      media_type: "text/plain; charset=utf-8",
      runtime_policy: "deterministic",
      byte_length: Buffer.byteLength(text),
      output_plan: {
        kind: "ascii",
        schema_version: 4,
        family: "flowchart",
        projection: "diagrammatic",
        encoding: "plain",
        primary_width: 0,
        primary_height: 0,
        emitted_width: 0,
        emitted_height: 0,
        width_profile: "unicode",
        layout_profile: "canonical",
        requested_layout_profile: "canonical",
        requested_graph_presentation: "strict",
        compact_attempted: false,
        requested_max_width: null,
        primary_overflowed: false,
        emitted_overflowed: false,
        outcome: "empty",
        fallback_capability: "unsupported",
        fallback_attempted: false,
        fallback_reason: null,
        trimmed: false,
        lossiness: "none",
        ...plan,
      },
    },
  };
}

function capability(diagram_type, graph_presentations, layout_profiles = ["canonical"]) {
  const supported = diagram_type !== "zenuml";
  return {
    diagram_type,
    display_name: diagram_type,
    semantic_coverage: supported ? "partial" : null,
    primary_projection: supported ? "diagrammatic" : "none",
    structured_text_fallback: supported,
    graph_presentations,
    layout_profiles: supported ? layout_profiles : [],
    width_profiles: supported ? ["unicode", "cjk"] : [],
    encodings: supported ? ["plain"] : [],
    fallback_encodings: supported ? ["plain"] : [],
    support_level: supported ? "partial" : "unsupported",
    supported_semantics: [],
    limits: [],
    evidence: [{ kind: "support_matrix", source: "matrix", note: "canonical evidence" }],
  };
}
