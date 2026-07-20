import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_REPORT_SCHEMA_VERSION,
  buildBenchmarkReport,
  downloadBenchmarkReport,
  projectBenchmarkRealmSample,
  serializeBenchmarkReport,
  type BenchmarkRunEvidence,
} from "./report.ts";
import { BENCHMARK_PROTOCOL_VERSION } from "./protocol.ts";
import { BENCHMARK_TRACE_SCHEMA_VERSION } from "./trace.ts";
import { REALM_PROTOCOL_VERSION } from "../runtime/realm/channel-protocol.ts";
import { BENCHMARK_PUBLICATION_CLOCK_BOUNDARY } from "./publication.ts";

test("report derives intervals from raw traces and strips capability tokens", () => {
  const sample = projectBenchmarkRealmSample(
    {
      blockIndex: 0,
      orderIndex: 0,
      purpose: "measured",
    },
    realmSuccess("merman", "realm-cold", "sample-1", coldTrace())
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
  const evidence = baseEvidence([
    projectBenchmarkRealmSample(
      { blockIndex: 0, orderIndex: 0, purpose: "measured" },
      realmSuccess("merman", "realm-cold", "m-1", coldTrace())
    ),
    projectBenchmarkRealmSample(
      { blockIndex: 0, orderIndex: 1, purpose: "measured" },
      realmFailure("mermaid", "realm-cold", "j-1")
    ),
  ]);
  const report = buildBenchmarkReport(evidence, "complete-with-errors");

  assert.equal(report.samples.length, 2);
  assert.equal(report.samples[1].outcome, "failure");
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

test("cancelled and invalidated reports suppress all aggregates", () => {
  const evidence = baseEvidence([
    projectBenchmarkRealmSample(
      { blockIndex: 0, orderIndex: 0, purpose: "measured" },
      realmSuccess("merman", "realm-cold", "m-1", coldTrace())
    ),
  ]);

  assert.equal(buildBenchmarkReport(evidence, "cancelled").aggregates, null);
  assert.equal(buildBenchmarkReport(evidence, "invalidated").aggregates, null);
});

test("ratio requires the same measured block identities on both engines", () => {
  const merman = projectBenchmarkRealmSample(
    { blockIndex: 0, orderIndex: 0, purpose: "measured" },
    realmSuccess("merman", "realm-cold", "m-1", coldTrace())
  );
  const mermaid = projectBenchmarkRealmSample(
    { blockIndex: 1, orderIndex: 0, purpose: "measured" },
    realmSuccess("mermaid", "realm-cold", "j-1", coldTrace())
  );
  const report = buildBenchmarkReport(baseEvidence([merman, mermaid]), "success");

  assert(report.aggregates);
  assert.equal(report.aggregates.ratios.firstIsolatedPresentationMs, null);
});

test("JSON download exactly serializes the displayed report and revokes its URL", () => {
  const report = buildBenchmarkReport(baseEvidence([]), "success");
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
      seed: 7,
      mode: "realm-cold",
      iterations: 1,
      warmups: 0,
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
    schedule: {
      seed: 7,
      blocks: [{ index: 0, order: ["merman", "mermaid"] }],
    },
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
  engine: "merman" | "mermaid",
  mode: "realm-cold" | "warm",
  requestId: string,
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
    requestId,
    engine,
    mode,
    role: "measured" as const,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources: [],
    resourceError: null,
    version: `test-${engine}`,
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
  engine: "merman" | "mermaid",
  mode: "realm-cold" | "warm",
  requestId: string
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
    requestId,
    engine,
    mode,
    role: "measured" as const,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: { ...coldTrace(), budgeted_svg_ready: null, isolated_dom_inserted: null, isolated_layout_box_ready: null, isolated_presentation_ready: null },
    resources: [],
    resourceError: null,
    version: `test-${engine}`,
    stage: "render" as const,
    message: "render failed",
    detail: engine === "mermaid" ? '{"hash":{"token":"INVALID"}}' : null,
  };
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
