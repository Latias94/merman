import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_REPORT_SCHEMA_VERSION,
  buildBenchmarkReport,
  downloadBenchmarkReport,
  projectBenchmarkRealmSample,
  projectBenchmarkTransportFailure,
  serializeBenchmarkReport,
  type BenchmarkRunEvidence,
} from "./report.ts";
import { BENCHMARK_PROTOCOL_VERSION } from "./protocol.ts";
import { BENCHMARK_TRACE_SCHEMA_VERSION } from "./trace.ts";
import { REALM_PROTOCOL_VERSION } from "../runtime/realm/channel-protocol.ts";
import { BENCHMARK_PUBLICATION_CLOCK_BOUNDARY } from "./publication.ts";
import {
  createBenchmarkSamplePlan,
  type BenchmarkSampleIntent,
  type BenchmarkSamplePlan,
} from "./sample-plan.ts";

test("report derives intervals from raw traces and strips capability tokens", () => {
  const plan = coldPlan();
  const intent = plan.samples[1];
  const sample = projectBenchmarkRealmSample(
    intent,
    realmSuccess(intent, coldTrace())
  );

  assert.equal(sample.outcome, "success");
  assert.equal(sample.intervals.firstIsolatedPresentationMs, 12);
  assert.equal(sample.intervals.firstPublishableSvgMs, 26);
  assert.equal(sample.intervals.strictSvgValidationMs, 3);
  assert.equal(sample.intervals.resourceAcquisitionMs, 3.5);
  assert.deepEqual(sample.parentPublication, parentPublication());
  assert.equal("runToken" in sample, false);
  assert.equal("realmToken" in sample, false);
});

test("failed samples remain in evidence but cannot produce a ratio", () => {
  const plan = coldPlan();
  const evidence = baseEvidence(plan, [
    projectBenchmarkRealmSample(
      plan.samples[0],
      realmFailure(plan.samples[0])
    ),
    projectBenchmarkRealmSample(
      plan.samples[1],
      realmSuccess(plan.samples[1], coldTrace())
    ),
  ]);
  const report = buildBenchmarkReport(evidence, "complete-with-errors");

  assert.equal(report.samples.length, 2);
  assert.equal(report.samples[0].outcome, "failure");
  assert(report.aggregates);
  assert.equal(report.aggregates.ratios.firstIsolatedPresentationMs, null);
  assert.equal(
    report.aggregates.engines.merman.firstIsolatedPresentationMs?.count,
    1
  );
  assert.equal(
    report.aggregates.engines.mermaid.firstIsolatedPresentationMs,
    null
  );
});

test("transport failures preserve the engine source and structured error detail", () => {
  const intent = coldPlan().samples[0];
  const sample = projectBenchmarkTransportFailure(
    intent,
    {
      requestId: "mermaid-reset",
      runId: "run-1",
    },
    {
      message: "Mermaid realm reset while rendering.",
      reason: { code: "REALM_RESET" },
    },
    "realm-reset"
  );

  assert.equal(sample.engine, "mermaid");
  assert.equal(sample.outcome, "failure");
  if (sample.outcome !== "failure") return;
  assert.equal(sample.failure.kind, "transport");
  assert.equal(sample.failure.stage, "realm-reset");
  assert.equal(sample.failure.message, "Mermaid realm reset while rendering.");
  assert.match(sample.failure.detail ?? "", /REALM_RESET/);
  assert.doesNotMatch(sample.failure.message, /\[object Object\]/);
  assert.doesNotMatch(sample.failure.detail ?? "", /\[object Object\]/);
});

test("cancelled and invalidated reports suppress all aggregates", () => {
  const plan = coldPlan();
  const evidence = baseEvidence(plan, [
    projectBenchmarkRealmSample(
      plan.samples[0],
      realmSuccess(plan.samples[0], coldTrace())
    ),
  ]);

  assert.equal(buildBenchmarkReport(evidence, "cancelled").aggregates, null);
  assert.equal(buildBenchmarkReport(evidence, "invalidated").aggregates, null);
});

test("report rejects samples whose metadata drifts from the sample plan", () => {
  const plan = coldPlan();
  const sample = projectBenchmarkRealmSample(
    plan.samples[0],
    realmSuccess(plan.samples[0], coldTrace())
  );
  assert.throws(
    () =>
      buildBenchmarkReport(
        baseEvidence(plan, [
          Object.freeze({ ...sample, sessionId: "wrong-session" }),
        ]),
        "cancelled"
      ),
    /does not match its plan intent/
  );
});

test("JSON download exactly serializes the displayed report and revokes its URL", () => {
  const report = buildBenchmarkReport(baseEvidence(coldPlan(), []), "cancelled");
  const calls: string[] = [];
  const blobs: Blob[] = [];

  downloadBenchmarkReport(report, {
    createObjectUrl(blob) {
      blobs.push(blob);
      calls.push("create");
      return "blob:test";
    },
    clickDownload(url, filename) {
      calls.push(`click:${url}:${filename}`);
    },
    revokeObjectUrl(url) {
      calls.push(`revoke:${url}`);
    },
  });

  assert.deepEqual(calls, [
    "create",
    `click:blob:test:merman-benchmark-${report.run.id}.json`,
    "revoke:blob:test",
  ]);
  assert.equal(blobs.length, 1);
  return blobs[0].text().then((text) => {
    assert.equal(text, serializeBenchmarkReport(report));
    assert.deepEqual(JSON.parse(text), report);
  });
});

function baseEvidence(
  plan: BenchmarkSamplePlan,
  samples: BenchmarkRunEvidence["samples"]
): BenchmarkRunEvidence {
  return {
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    protocols: {
      benchmark: BENCHMARK_PROTOCOL_VERSION,
      realm: REALM_PROTOCOL_VERSION,
      trace: BENCHMARK_TRACE_SCHEMA_VERSION,
    },
    run: {
      id: "run-1",
      startedAt: "2026-07-19T00:00:00.000Z",
      endedAt: "2026-07-19T00:00:01.000Z",
      durationMs: 1000,
    },
    input: {
      source: "flowchart TD\nA-->B",
      configJson: "{}",
      theme: "default",
      diagramFont: "trebuchet",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
      viewport: { width: 800, height: 600 },
      detection: {
        status: "available",
        validity: "valid",
        diagramType: "flowchart",
        syntaxId: "flowchart",
        effectiveLayoutId: "dagre",
      },
    },
    plan,
    versions: {
      expected: { merman: "0.8.0-alpha.1", mermaid: "11.16.0" },
      observed: { merman: ["test-merman"], mermaid: [] },
    },
    environment: {
      userAgent: "test",
      language: "en-US",
      platform: "test",
      hardwareConcurrency: 8,
      devicePixelRatio: 1,
      crossOriginIsolated: false,
    },
    transitions: [{ atMs: 0, kind: "start", visibilityState: "visible" }],
    samples,
    terminalError: null,
  };
}

function realmSuccess(
  intent: BenchmarkSampleIntent,
  trace: ReturnType<typeof coldTrace>
) {
  return {
    type: "benchmark-sample-success" as const,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    kind: "benchmark" as const,
    realmId: "realm",
    realmToken: "t".repeat(43),
    sequence: 1,
    runId: "run-1",
    runToken: "r".repeat(43),
    requestId: intent.sampleId,
    sampleId: intent.sampleId,
    engine: intent.engine,
    intentKind: intent.kind,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources: [],
    resourceError: null,
    version: `test-${intent.engine}`,
    parentPublication: parentPublication(),
    svgBytes: 100,
  };
}

function parentPublication() {
  return Object.freeze({
    clockBoundary: BENCHMARK_PUBLICATION_CLOCK_BOUNDARY,
    isolatedPresentationReceiptMs: 20,
    responseDeliveryMs: 1,
    responseEnvelopeValidationMs: 2,
    strictSvgValidationMs: 3,
    totalMs: 26,
  });
}

function realmFailure(
  intent: BenchmarkSampleIntent
) {
  return {
    type: "benchmark-sample-failure" as const,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    kind: "benchmark" as const,
    realmId: "realm",
    realmToken: "t".repeat(43),
    sequence: 1,
    runId: "run-1",
    runToken: "r".repeat(43),
    requestId: intent.sampleId,
    sampleId: intent.sampleId,
    engine: intent.engine,
    intentKind: intent.kind,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: { ...coldTrace(), budgeted_svg_ready: null, isolated_dom_inserted: null, isolated_layout_box_ready: null, isolated_presentation_ready: null },
    resources: [],
    resourceError: null,
    version: `test-${intent.engine}`,
    stage: "render" as const,
    message: "render failed",
    detail:
      intent.engine === "mermaid" ? '{"hash":{"token":"INVALID"}}' : null,
  };
}

function coldPlan(): BenchmarkSamplePlan {
  return createBenchmarkSamplePlan({
    iterations: 2,
    mode: "realm-cold",
    seed: 7,
  });
}

function coldTrace() {
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
