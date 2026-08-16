import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_CORPUS_BUDGETS,
  BENCHMARK_CORPUS_FIXTURE_KIND,
  BENCHMARK_CORPUS_SCHEMA_VERSION,
  FAMILY_BASELINE_CORPUS,
  createBenchmarkCorpusCatalog,
  createBenchmarkCorpusOrchestrator,
  createBenchmarkCorpusPlan,
  validateBenchmarkCorpusRunBudget,
  type BenchmarkCorpusDependencies,
} from "./corpus.ts";
import type { BenchmarkRunRequest } from "./controller.ts";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "./report-schema.ts";
import type { BenchmarkReport } from "./report.ts";

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
});

test("corpus plans retain exact deterministic cold and warm seeds", () => {
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
        fixtureIds: [FIXTURE_IDS[0], FIXTURE_IDS[0]],
        masterSeed: 1,
      }),
    /selection is invalid/u
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

test("catalog projection hashes every catalog row exactly once", async () => {
  let digestCalls = 0;
  const catalog = await createBenchmarkCorpusCatalog(async (bytes) => {
    digestCalls += 1;
    return bytesToFakeSha(bytes);
  });

  assert.equal(digestCalls, FAMILY_BASELINE_CORPUS.length);
  assert.equal(catalog.identity.availableFamilies, FAMILY_BASELINE_CORPUS.length);
  assert.equal(catalog.fixtures.length, FAMILY_BASELINE_CORPUS.length);
  assert.deepEqual(
    catalog.fixtures.map(({ id }) => id),
    FAMILY_BASELINE_CORPUS.map(({ id }) => id)
  );
});

test("whole-corpus retained evidence is bounded before execution", () => {
  const request = { iterations: 30, warmups: 0 };
  assert.doesNotThrow(() => validateBenchmarkCorpusRunBudget(request, 1));
  assert.throws(
    () =>
      validateBenchmarkCorpusRunBudget(
        request,
        FAMILY_BASELINE_CORPUS.length
      ),
    new RegExp(
      `whole-corpus budget is ${BENCHMARK_CORPUS_BUDGETS.maxRetainedSamples}`,
      "u"
    )
  );
});

test("one page executes exactly one fixture with the planned seeds", async () => {
  const calls: BenchmarkRunRequest[] = [];
  let digestCalls = 0;
  const dependencies = fakeDependencies(async (request) => {
    calls.push(request);
    return report(request, "success");
  });
  dependencies.digest = async (bytes) => {
    digestCalls += 1;
    return bytesToFakeSha(bytes);
  };
  const orchestrator = createBenchmarkCorpusOrchestrator(dependencies);
  const envelope = await orchestrator.run({
    fixtureId: FIXTURE_IDS[0],
    coldSeed: 101,
    warmSeed: 202,
    iterations: 2,
    warmups: 1,
  });

  assert.equal(envelope.schemaVersion, BENCHMARK_CORPUS_SCHEMA_VERSION);
  assert.equal(envelope.kind, BENCHMARK_CORPUS_FIXTURE_KIND);
  assert.equal(envelope.execution.fixtureIsolation, "single-page");
  assert.equal(envelope.fixtureId, FIXTURE_IDS[0]);
  assert.equal(envelope.fixture.status, "success");
  assert.equal(envelope.terminalStatus, "success");
  assert.equal(digestCalls, 1);
  assert.deepEqual(
    calls.map(({ mode, seed }) => [mode, seed]),
    [
      ["realm-cold", 101],
      ["warm", 202],
    ]
  );
  assert.equal(calls[0]!.payload.source, calls[1]!.payload.source);
  assert.equal(envelope.fixture.source.bytes, new TextEncoder().encode(
    calls[0]!.payload.source
  ).byteLength);
  assert.equal("fixtures" in envelope, false);
});

test("fixture page rejects missing, plural, and unknown selections", async () => {
  const orchestrator = createBenchmarkCorpusOrchestrator(
    fakeDependencies(async (request) => report(request, "success"))
  );
  const base = {
    coldSeed: 1,
    warmSeed: 2,
    iterations: 2,
    warmups: 0,
  };

  await assert.rejects(
    orchestrator.run({ ...base, fixtureId: "" }),
    /fixture id is required/u
  );
  await assert.rejects(
    orchestrator.run({ ...base, fixtureId: "not-a-family-baseline" }),
    /unknown fixture/u
  );
  await assert.rejects(
    orchestrator.run({
      ...base,
      fixtureId: [FIXTURE_IDS[0], FIXTURE_IDS[1]],
    } as unknown as Parameters<typeof orchestrator.run>[0]),
    /fixture id is required/u
  );
});

test("cancellation returns one structured fixture failure envelope", async () => {
  const pending = Promise.withResolvers<BenchmarkReport>();
  const dependencies = fakeDependencies(() => pending.promise);
  const orchestrator = createBenchmarkCorpusOrchestrator(dependencies);
  const running = orchestrator.run({
    fixtureId: FIXTURE_IDS[0],
    coldSeed: 7,
    warmSeed: 8,
    iterations: 2,
    warmups: 0,
  });
  await dependencies.controllerStarted();

  orchestrator.cancel("test-cancel");
  pending.resolve(report(dependencies.lastRequest()!, "cancelled"));
  const envelope = await running;

  assert.equal(dependencies.cancelReasons().at(-1), "test-cancel");
  assert.equal(envelope.terminalStatus, "cancelled");
  assert.equal(envelope.fixture.status, "failure");
  assert.equal(envelope.fixture.failure?.stage, "cancelled");
  assert.equal(envelope.fixture.cold.report?.terminalStatus, "cancelled");
  assert.equal(envelope.fixture.warm.status, "skipped");
});

function fakeDependencies(
  run: (request: BenchmarkRunRequest) => Promise<BenchmarkReport>
): BenchmarkCorpusDependencies & {
  cancelReasons(): readonly string[];
  controllerStarted(): Promise<void>;
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
      start(request) {
        requests.push(request);
        started.resolve();
        return {
          completion: run(request),
          runId: `fake-run-${requests.length}`,
        };
      },
    },
    cancelReasons: () => reasons,
    controllerStarted: () => started.promise,
    prepareFixture(fixture) {
      return {
        payload: {
          source: fixture.source,
          configJson: "{}",
          theme: "default",
          diagramFont: "trebuchet",
          externalRequirements: { externalDiagrams: [], layoutModules: [] },
          screenAvailableWidth: 1512,
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
    dateNow: () => 1_754_352_000_000 + now,
    digest: async (bytes) => bytesToFakeSha(bytes),
    lastRequest: () => requests.at(-1) ?? null,
    now: () => now++,
    versions: { merman: "test-merman", mermaid: "test-mermaid" },
  };
}

function report(
  request: BenchmarkRunRequest,
  terminalStatus: BenchmarkReport["terminalStatus"]
): BenchmarkReport {
  return {
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus,
    terminalError:
      terminalStatus === "success"
        ? null
        : {
            kind: "transport",
            stage: "test-report",
            message: `Test report ended with ${terminalStatus}.`,
            detail: null,
          },
    input: {
      ...request.payload,
      detection: request.detection,
    },
  } as BenchmarkReport;
}

function bytesToFakeSha(bytes: Uint8Array): string {
  const checksum = bytes.reduce((value, byte) => (value + byte) % 256, 0);
  return checksum.toString(16).padStart(2, "0").repeat(32);
}
