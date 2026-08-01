import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_CORPUS_SCHEMA_VERSION,
  BENCHMARK_CORPUS_BUDGETS,
  FAMILY_BASELINE_CORPUS,
  createBenchmarkCorpusOrchestrator,
  createBenchmarkCorpusPlan,
  validateBenchmarkCorpusRunBudget,
  type BenchmarkCorpusDependencies,
} from "./corpus.ts";
import type { BenchmarkRunRequest } from "./controller.ts";
import {
  BENCHMARK_REPORT_SCHEMA_VERSION,
  type BenchmarkReport,
} from "./report.ts";
import { BENCHMARK_PROTOCOL_VERSION } from "./protocol.ts";
import { BENCHMARK_TRACE_SCHEMA_VERSION } from "./trace.ts";
import { REALM_PROTOCOL_VERSION } from "../runtime/realm/channel-protocol.ts";

const FIXTURE_IDS = ["basic-flowchart", "sequence-interaction"] as const;

test("family corpus is the one-baseline-per-family generated projection", () => {
  assert.equal(FAMILY_BASELINE_CORPUS.length, 35);
  assert.equal(
    new Set(FAMILY_BASELINE_CORPUS.map((fixture) => fixture.family)).size,
    FAMILY_BASELINE_CORPUS.length
  );
  assert.equal(
    new Set(FAMILY_BASELINE_CORPUS.map((fixture) => fixture.id)).size,
    FAMILY_BASELINE_CORPUS.length
  );
  assert.ok(
    FAMILY_BASELINE_CORPUS.every(
      (fixture, index) =>
        index === 0 || FAMILY_BASELINE_CORPUS[index - 1]!.order < fixture.order
    )
  );
  assert.ok(
    FAMILY_BASELINE_CORPUS.every(
      (fixture) =>
        fixture.detection.status === "available" &&
        fixture.detection.validity === "valid" &&
        fixture.detection.diagramType === fixture.family &&
        fixture.detection.syntaxId !== null &&
        fixture.detection.effectiveLayoutId !== null
    )
  );
});

test("corpus plans are seed-deterministic and selection-order independent", () => {
  const first = createBenchmarkCorpusPlan({
    fixtureIds: [...FIXTURE_IDS],
    masterSeed: 0xdecafbad,
  });
  const second = createBenchmarkCorpusPlan({
    fixtureIds: [...FIXTURE_IDS].reverse(),
    masterSeed: 0xdecafbad,
  });
  const changed = createBenchmarkCorpusPlan({
    fixtureIds: [...FIXTURE_IDS],
    masterSeed: 0xdecafbae,
  });

  assert.deepEqual(first, second);
  assert.notDeepEqual(
    first.map(({ coldSeed, warmSeed }) => [coldSeed, warmSeed]),
    changed.map(({ coldSeed, warmSeed }) => [coldSeed, warmSeed])
  );
  assert.throws(
    () =>
      createBenchmarkCorpusPlan({
        fixtureIds: ["not-a-family-baseline"],
        masterSeed: 1,
      }),
    /unknown fixture/u
  );
});

test("whole-corpus retained evidence is bounded before execution", () => {
  const request = { iterations: 30, warmups: 0 };
  assert.doesNotThrow(() => validateBenchmarkCorpusRunBudget(request, 1));
  assert.throws(
    () => validateBenchmarkCorpusRunBudget(request, FAMILY_BASELINE_CORPUS.length),
    new RegExp(`whole-corpus budget is ${BENCHMARK_CORPUS_BUDGETS.maxRetainedSamples}`, "u")
  );
});

test("orchestrator runs serially and records schema-5 failures", async () => {
  const active: string[] = [];
  const calls: BenchmarkRunRequest[] = [];
  let maxActive = 0;
  const dependencies = fakeDependencies(async (request) => {
    calls.push(request);
    active.push(request.payload.source);
    maxActive = Math.max(maxActive, active.length);
    await Promise.resolve();
    active.pop();
    return calls.length === 4
      ? report(request, "complete-with-errors")
      : successReport(request);
  });
  const orchestrator = createBenchmarkCorpusOrchestrator(dependencies);
  const envelope = await orchestrator.run({
    fixtureIds: [...FIXTURE_IDS],
    iterations: 2,
    masterSeed: 0x12345678,
    warmups: 0,
  });

  assert.equal(envelope.schemaVersion, BENCHMARK_CORPUS_SCHEMA_VERSION);
  assert.equal(envelope.execution.fixtureIsolation, "single-page");
  assert.equal(envelope.terminalStatus, "complete-with-errors");
  assert.equal(envelope.coverage.availableFamilies, 35);
  assert.equal(envelope.coverage.selectedFamilies, 2);
  assert.equal(envelope.coverage.succeededFamilies, 1);
  assert.equal(envelope.coverage.failedFamilies, 1);
  assert.equal(envelope.coverage.skippedFamilies, 33);
  assert.equal(envelope.failures.length, 1);
  assert.equal(envelope.failures[0]!.mode, "warm");
  assert.equal(maxActive, 1);
  assert.deepEqual(
    calls.map((request) => request.mode),
    ["realm-cold", "warm", "realm-cold", "warm"]
  );
  for (let index = 0; index < calls.length; index += 2) {
    assert.equal(calls[index]!.payload.source, calls[index + 1]!.payload.source);
  }
  for (const result of envelope.fixtures.filter(
    (fixture) => fixture.cold.report !== null
  )) {
    assert.match(result.source.sha256, /^[0-9a-f]{64}$/u);
    assert.equal(
      result.source.bytes,
      new TextEncoder().encode(
        result.cold.report!.input.source
      ).byteLength
    );
  }
});

test("cancelling an active corpus retains the cancelled report and skips the tail", async () => {
  const pending = Promise.withResolvers<BenchmarkReport>();
  const dependencies = fakeDependencies(() => pending.promise);
  const orchestrator = createBenchmarkCorpusOrchestrator(dependencies);
  const running = orchestrator.run({
    fixtureIds: [...FIXTURE_IDS],
    iterations: 2,
    masterSeed: 7,
    warmups: 0,
  });
  await dependencies.controllerStarted();

  orchestrator.cancel("test-cancel");
  pending.resolve(cancelledReport(dependencies.lastRequest()!));
  const envelope = await running;

  assert.equal(dependencies.cancelReasons().at(-1), "test-cancel");
  assert.equal(envelope.terminalStatus, "cancelled");
  assert.equal(envelope.coverage.succeededFamilies, 0);
  assert.equal(envelope.coverage.failedFamilies, 0);
  assert.equal(envelope.coverage.skippedFamilies, 35);
  const attempted = envelope.fixtures.find(
    (fixture) => fixture.cold.report !== null
  );
  assert.equal(attempted?.cold.report?.terminalStatus, "cancelled");
  assert.ok(envelope.skips.some((skip) => skip.reason === "test-cancel"));
});

function fakeDependencies(
  run: (request: BenchmarkRunRequest) => Promise<BenchmarkReport>
): BenchmarkCorpusDependencies & {
  cancelReasons(): readonly string[];
  controllerStarted(): Promise<void>;
  controllerRunCount(): number;
  lastRequest(): BenchmarkRunRequest | null;
} {
  const requests: BenchmarkRunRequest[] = [];
  const reasons: string[] = [];
  const started = Promise.withResolvers<void>();
  let now = 100;
  return {
    controller: {
      cancel(reason) {
        reasons.push(reason ?? "user");
      },
      async run(request) {
        requests.push(request);
        started.resolve();
        return run(request);
      },
    },
    cancelReasons: () => reasons,
    controllerStarted: () => started.promise,
    controllerRunCount: () => requests.length,
    prepareFixture(fixture) {
      return {
        payload: {
          source: fixture.source,
          configJson: "{}",
          theme: "default",
          diagramFont: "trebuchet",
          externalRequirements: { externalDiagrams: [], layoutModules: [] },
          viewport: { width: 800, height: 600 },
        },
        detection: {
          status: "available",
          validity: "valid",
          diagramType: fixture.family,
          syntaxId: fixture.family,
          effectiveLayoutId: "dagre",
        },
      };
    },
    dateNow: () => 1_753_000_000_000 + now,
    digest: async (bytes) => bytesToFakeSha(bytes),
    lastRequest: () => requests.at(-1) ?? null,
    now: () => now++,
    versions: { merman: "test-merman", mermaid: "test-mermaid" },
  };
}

function successReport(request: BenchmarkRunRequest): BenchmarkReport {
  return report(request, "success");
}

function cancelledReport(request: BenchmarkRunRequest): BenchmarkReport {
  return report(request, "cancelled");
}

function report(
  request: BenchmarkRunRequest,
  terminalStatus: BenchmarkReport["terminalStatus"]
): BenchmarkReport {
  return {
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    protocols: {
      benchmark: BENCHMARK_PROTOCOL_VERSION,
      realm: REALM_PROTOCOL_VERSION,
      trace: BENCHMARK_TRACE_SCHEMA_VERSION,
    },
    run: {
      id: `${request.mode}-${request.seed}`,
      seed: request.seed!,
      mode: request.mode,
      iterations: request.iterations,
      warmups: request.warmups,
      startedAt: "2026-07-30T00:00:00.000Z",
      endedAt: "2026-07-30T00:00:00.001Z",
      durationMs: 1,
    },
    input: {
      ...request.payload,
      detection: request.detection,
    },
    schedule: { seed: request.seed!, blocks: [] },
    versions: {
      expected: request.versions,
      observed: { merman: ["test-merman"], mermaid: ["test-mermaid"] },
    },
    environment: {
      userAgent: "test",
      language: "en-US",
      platform: "test",
      hardwareConcurrency: 8,
      devicePixelRatio: 1,
      crossOriginIsolated: false,
    },
    transitions: [],
    samples: [],
    terminalError: null,
    terminalStatus,
    aggregates: terminalStatus === "success" ? { engines: {} as never, ratios: {} as never } : null,
  };
}

function bytesToFakeSha(bytes: Uint8Array): string {
  const checksum = bytes.reduce((value, byte) => (value + byte) % 256, 0);
  return checksum.toString(16).padStart(2, "0").repeat(32);
}
