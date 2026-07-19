import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_PROTOCOL_VERSION,
  validateBenchmarkSampleProgress,
  validateBenchmarkSampleRequest,
  validateBenchmarkSampleResponse,
} from "./protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  type BenchmarkRawTrace,
} from "./trace.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
} from "../runtime/realm/channel-protocol.ts";

const TOKEN = "t".repeat(43);
const RUN_TOKEN = "r".repeat(43);
const IDENTITY = {
  kind: "benchmark" as const,
  realmId: "benchmark-realm",
  realmToken: TOKEN,
};

test("benchmark request binds protocol, realm, run, request, and engine", () => {
  const request = sampleRequest();
  assert.deepEqual(validateBenchmarkSampleRequest(request, IDENTITY, 1), request);

  for (const invalid of [
    { ...request, protocol: REALM_PROTOCOL_VERSION + 1 },
    { ...request, benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION + 1 },
    { ...request, realmToken: "x".repeat(43) },
    { ...request, runToken: "short" },
    { ...request, requestId: "" },
    { ...request, engine: "other" },
    { ...request, mode: "cold" },
    { ...request, totalMs: 1 },
  ]) {
    assert.throws(
      () => validateBenchmarkSampleRequest(invalid, IDENTITY, 1),
      RealmProtocolError
    );
  }
});

test("benchmark request accepts source and config limits and rejects one byte more", () => {
  const exact = sampleRequest({
    payload: {
      ...sampleRequest().payload,
      source: "s".repeat(REALM_BUDGETS.sourceBytes),
      configJson: "c".repeat(REALM_BUDGETS.configBytes),
    },
  });
  assert.doesNotThrow(() => validateBenchmarkSampleRequest(exact, IDENTITY, 1));

  for (const payload of [
    {
      ...exact.payload,
      source: `${exact.payload.source}s`,
    },
    {
      ...exact.payload,
      source: "flowchart TD\nA-->B",
      configJson: `${exact.payload.configJson}c`,
    },
  ]) {
    assert.throws(
      () =>
        validateBenchmarkSampleRequest(
          sampleRequest({ payload }),
          IDENTITY,
          1
        ),
      RealmProtocolError
    );
  }
});

test("benchmark progress is exact, authenticated, and request-bound", () => {
  const request = sampleRequest();
  const progress = sampleProgress(request, "fonts_wait_start", 1);
  const validated = validateBenchmarkSampleProgress(
    progress,
    IDENTITY,
    1,
    request
  );

  assert.deepEqual(validated, progress);
  assert(Object.isFrozen(validated));
  for (const invalid of [
    { ...progress, sequence: 2 },
    { ...progress, realmToken: "x".repeat(43) },
    { ...progress, runToken: "x".repeat(43) },
    { ...progress, requestId: "other" },
    { ...progress, engine: "mermaid" },
    { ...progress, mode: "warm" },
    { ...progress, role: "warmup" },
    { ...progress, traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION + 1 },
    { ...progress, event: "sample_start" },
    { ...progress, elapsedMs: 1 },
    { ...progress, totalMs: 1 },
  ]) {
    assert.throws(
      () =>
        validateBenchmarkSampleProgress(invalid, IDENTITY, 1, request),
      RealmProtocolError
    );
  }
});

test("successful response validates exact trace, SVG, and nullable resources", () => {
  const request = sampleRequest();
  const response = sampleSuccess();
  const validated = validateBenchmarkSampleResponse(
    response,
    IDENTITY,
    1,
    request
  );

  assert.equal(validated.type, "benchmark-sample-success");
  assert(Object.isFrozen(validated));
  assert(Object.isFrozen(validated.trace));
  assert(Object.isFrozen(validated.resources));
  assert.deepEqual(validated.resources[0], response.resources[0]);
});

test("optional resource evidence can fail without discarding a successful sample", () => {
  const request = sampleRequest();
  const response = {
    ...sampleSuccess(),
    resources: [],
    resourceError: "Resource Timing is unavailable.",
  };
  const validated = validateBenchmarkSampleResponse(
    response,
    IDENTITY,
    1,
    request
  );

  assert.equal(validated.resourceError, response.resourceError);
  assert.deepEqual(validated.resources, []);
  assert.throws(() =>
    validateBenchmarkSampleResponse(
      { ...response, resources: sampleSuccess().resources },
      IDENTITY,
      1,
      request
    )
  );
});

test("response rejects identity drift, adapter totals, malformed trace, and SVG overflow", () => {
  const request = sampleRequest();
  const response = sampleSuccess();
  for (const invalid of [
    { ...response, requestId: "other" },
    { ...response, engine: "mermaid" },
    { ...response, totalMs: 12 },
    { ...response, trace: { ...response.trace, renderTimeMs: 2 } },
    { ...response, trace: { ...response.trace, sample_end: Number.NaN } },
    {
      ...response,
      resources: [{ ...response.resources[0], duration: 20 }],
    },
    {
      ...response,
      trace: {
        ...response.trace,
        budgeted_svg_ready: 30_010,
        isolated_dom_inserted: 30_010,
        isolated_layout_box_ready: 30_010,
        isolated_presentation_ready: 30_011,
        sample_end: 30_011,
      },
    },
    { ...response, svg: "s".repeat(REALM_BUDGETS.svgBytes + 1) },
  ]) {
    assert.throws(() =>
      validateBenchmarkSampleResponse(invalid, IDENTITY, 1, request)
    );
  }
});

test("pre-clock environment failure may return null trace without a version", () => {
  const request = sampleRequest();
  const response = {
    type: "benchmark-sample-failure",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: null,
    resources: [],
    resourceError: null,
    stage: "environment",
    message: "hidden",
    version: null,
  };
  assert.deepEqual(
    validateBenchmarkSampleResponse(response, IDENTITY, 1, request),
    response
  );
  assert.throws(
    () =>
      validateBenchmarkSampleResponse(
        { ...response, stage: "render" },
        IDENTITY,
        1,
        request
      ),
    /must retain its raw trace/
  );
  assert.throws(
    () =>
      validateBenchmarkSampleResponse(
        { ...response, stage: "timeout" },
        IDENTITY,
        1,
        request
      ),
    /must retain its raw trace/
  );
});

test("failure response retains a validated completed trace prefix", () => {
  const request = sampleRequest();
  const response = {
    ...sampleSuccess(),
    type: "benchmark-sample-failure",
    stage: "render",
    message: "render failed",
    svg: undefined,
    version: "0.8.0-alpha.3",
    trace: {
      ...coldMermanTrace(),
      budgeted_svg_ready: null,
      isolated_dom_inserted: null,
      isolated_layout_box_ready: null,
      isolated_presentation_ready: null,
      sample_end: 9,
    },
  } as Record<string, unknown>;
  delete response.svg;

  const validated = validateBenchmarkSampleResponse(
    response,
    IDENTITY,
    1,
    request
  );
  assert.equal(validated.type, "benchmark-sample-failure");
  assert(Object.isFrozen(validated));
  assert.equal(validated.trace?.render_start, 8);
  assert.throws(
    () =>
      validateBenchmarkSampleResponse(
        { ...response, stage: "adapter-import" },
        IDENTITY,
        1,
        request
      ),
    /later phase evidence/
  );
});

test("non-timeout failures cannot hide an over-budget active stage", () => {
  const request = sampleRequest();
  const response = failureFromSuccess("render", {
    ...coldMermanTrace(),
    budgeted_svg_ready: null,
    isolated_dom_inserted: null,
    isolated_layout_box_ready: null,
    isolated_presentation_ready: null,
    sample_end: 8 + REALM_BUDGETS.stageTimeoutMs + 1,
  });
  assert.throws(
    () => validateBenchmarkSampleResponse(response, IDENTITY, 1, request),
    /stage time budget/
  );

  const presentation = failureFromSuccess("presentation", {
    ...coldMermanTrace(),
    isolated_presentation_ready: null,
    sample_end: 10 + REALM_BUDGETS.stageTimeoutMs + 1,
  });
  assert.throws(
    () => validateBenchmarkSampleResponse(presentation, IDENTITY, 1, request),
    /stage time budget/
  );
});

test("timeout failures still obey the whole-run budget", () => {
  const request = sampleRequest();
  const response = failureFromSuccess("timeout", {
    ...coldMermanTrace(),
    budgeted_svg_ready: null,
    isolated_dom_inserted: null,
    isolated_layout_box_ready: null,
    isolated_presentation_ready: null,
    sample_end: REALM_BUDGETS.runTimeoutMs + 1,
  });
  assert.throws(
    () => validateBenchmarkSampleResponse(response, IDENTITY, 1, request),
    /run time budget/
  );
});

function failureFromSuccess(
  stage: "presentation" | "render" | "timeout",
  trace: BenchmarkRawTrace
) {
  const response = {
    ...sampleSuccess(),
    type: "benchmark-sample-failure",
    stage,
    message: `${stage} failed`,
    trace,
  } as Record<string, unknown>;
  delete response.svg;
  return response;
}

function sampleRequest(
  overrides: Partial<ReturnType<typeof baseSampleRequest>> = {}
) {
  return { ...baseSampleRequest(), ...overrides };
}

function baseSampleRequest() {
  return {
    type: "benchmark-sample" as const,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    runId: "run-1",
    runToken: RUN_TOKEN,
    requestId: "request-1",
    engine: "merman" as const,
    mode: "realm-cold" as const,
    role: "measured" as const,
    payload: {
      source: "flowchart TD\nA-->B",
      configJson: "{}",
      theme: "default",
      diagramFont: "trebuchet" as const,
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
      viewport: { width: 800, height: 600 },
    },
  };
}

function sampleSuccess() {
  const request = sampleRequest();
  return {
    type: "benchmark-sample-success" as const,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: coldMermanTrace(),
    resources: [
      {
        name: "https://example.test/merman.wasm",
        initiatorType: "fetch",
        startOffset: 3,
        duration: 2,
        transferSize: null,
        encodedBodySize: 10,
        decodedBodySize: 10,
        responseStatus: 200,
        deliveryType: null,
      },
    ],
    resourceError: null,
    svg: '<svg xmlns="http://www.w3.org/2000/svg" />',
    version: "0.8.0-alpha.3",
  };
}

function sampleProgress(
  request: ReturnType<typeof sampleRequest>,
  event: string,
  sequence: number
) {
  return {
    type: "benchmark-progress" as const,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    event,
  };
}

function coldMermanTrace() {
  return {
    sample_start: 0 as const,
    fonts_wait_start: 0,
    fonts_wait_end: 7,
    adapter_import_start: 0,
    adapter_import_end: 2,
    engine_import_start: 2,
    engine_import_end: 5,
    resource_acquire_start: 2.5,
    resource_acquire_end: 6,
    register_start: null,
    register_end: null,
    initialize_start: 6,
    initialize_end: 7,
    render_start: 8,
    budgeted_svg_ready: 10,
    isolated_dom_inserted: 10.5,
    isolated_layout_box_ready: 11,
    isolated_presentation_ready: 12,
    sample_end: 12.5,
  };
}
