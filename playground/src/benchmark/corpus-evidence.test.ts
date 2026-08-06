import assert from "node:assert/strict";
import test from "node:test";

import {
  assembleBenchmarkCorpusAggregate,
  createBenchmarkCorpusCatalogIdentity,
  createBenchmarkCorpusFailureEnvelope,
  createBenchmarkCorpusFixtureEnvelope,
  type BenchmarkCorpusCatalog,
  type BenchmarkCorpusFixtureDescriptor,
  type BenchmarkCorpusFixtureEnvelope,
  type BenchmarkCorpusPlanEvidenceEntry,
} from "./corpus-evidence.ts";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "./report-schema.ts";
import type { BenchmarkReport } from "./report.ts";

const VERSIONS = Object.freeze({ merman: "test-merman", mermaid: "test-mermaid" });

test("one fixture envelope becomes one aggregate row without placeholders", () => {
  const catalog = fixtureCatalog(1);
  const plan = fixturePlan(catalog);
  const envelope = successEnvelope(catalog, plan[0]!);
  const aggregate = aggregateEnvelope(catalog, plan, [envelope]);

  assert.equal(aggregate.fixtures.length, 1);
  assert.deepEqual(aggregate.run.order, ["fixture-1"]);
  assert.deepEqual(aggregate.coverage, {
    availableFamilies: 1,
    selectedFamilies: 1,
    attemptedFamilies: 1,
    succeededFamilies: 1,
    failedFamilies: 0,
  });
  assert.equal(aggregate.failures.length, 0);
  assert.equal(aggregate.terminalStatus, "success");
});

test("linear assembler emits exactly one row for each of 35 selected fixtures", () => {
  const catalog = fixtureCatalog(35);
  const plan = fixturePlan(catalog);
  const envelopes = plan.map((entry) =>
    failureEnvelope(catalog, entry, "browser-crash", true)
  );
  const aggregate = aggregateEnvelope(catalog, plan, envelopes);

  assert.equal(aggregate.fixtures.length, 35);
  assert.equal(aggregate.failures.length, 35);
  assert.equal(aggregate.coverage.availableFamilies, 35);
  assert.equal(aggregate.coverage.selectedFamilies, 35);
  assert.equal(aggregate.coverage.attemptedFamilies, 35);
  assert.equal(aggregate.coverage.failedFamilies, 35);
  assert.deepEqual(
    aggregate.fixtures.map(({ id }) => id),
    plan.map(({ fixtureId }) => fixtureId)
  );
  assert.ok(
    aggregate.fixtures.every(
      (fixture) =>
        fixture.failure?.stage === "browser-crash" &&
        fixture.cold.skipReason === "browser-crash" &&
        fixture.warm.skipReason === "browser-crash"
    )
  );
});

test("assembler rejects missing, duplicate, unknown, and out-of-order evidence", () => {
  const catalog = fixtureCatalog(2);
  const plan = fixturePlan(catalog);
  const envelopes = plan.map((entry) => successEnvelope(catalog, entry));

  assert.throws(
    () => aggregateEnvelope(catalog, plan, envelopes.slice(0, 1)),
    /expected 2 fixture envelopes/u
  );
  assert.throws(
    () =>
      aggregateEnvelope(
        catalog,
        [plan[0]!, { ...plan[1]!, fixtureId: plan[0]!.fixtureId }],
        envelopes
      ),
    /duplicate fixture/u
  );
  assert.throws(
    () =>
      aggregateEnvelope(
        catalog,
        [{ ...plan[0]!, fixtureId: "unknown-fixture" }, plan[1]!],
        envelopes
      ),
    /unknown fixture/u
  );
  assert.throws(
    () => aggregateEnvelope(catalog, plan, [...envelopes].reverse()),
    /contract drifted/u
  );
});

test("assembler rejects catalog and version drift at the process boundary", () => {
  const catalog = fixtureCatalog(1);
  const plan = fixturePlan(catalog);
  const envelope = successEnvelope(catalog, plan[0]!);
  const catalogDrift = {
    ...envelope,
    catalog: {
      ...envelope.catalog,
      mermaidBaseline: "mermaid@drifted",
    },
  } as BenchmarkCorpusFixtureEnvelope;
  const versionDrift = {
    ...envelope,
    versions: { ...envelope.versions, mermaid: "drifted" },
  } as BenchmarkCorpusFixtureEnvelope;

  assert.throws(
    () => aggregateEnvelope(catalog, plan, [catalogDrift]),
    /provenance drifted/u
  );
  assert.throws(
    () => aggregateEnvelope(catalog, plan, [versionDrift]),
    /provenance drifted/u
  );
});

test("unstarted cancellation remains an explicit structured fixture row", () => {
  const catalog = fixtureCatalog(1);
  const plan = fixturePlan(catalog);
  const envelope = failureEnvelope(
    catalog,
    plan[0]!,
    "batch-cancelled",
    false,
    "cancelled"
  );
  const aggregate = aggregateEnvelope(catalog, plan, [envelope]);

  assert.equal(aggregate.fixtures.length, 1);
  assert.equal(aggregate.fixtures[0]!.attempted, false);
  assert.equal(aggregate.fixtures[0]!.status, "failure");
  assert.equal(aggregate.failures[0]!.stage, "batch-cancelled");
  assert.equal(aggregate.coverage.attemptedFamilies, 0);
  assert.equal(aggregate.terminalStatus, "cancelled");
});

function aggregateEnvelope(
  catalog: BenchmarkCorpusCatalog,
  plan: readonly BenchmarkCorpusPlanEvidenceEntry[],
  fixtureEnvelopes: readonly BenchmarkCorpusFixtureEnvelope[]
) {
  return assembleBenchmarkCorpusAggregate({
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog,
    fixtureEnvelopes,
    plan,
    run: {
      id: "aggregate-test",
      masterSeed: 0x5eed1234,
      iterations: 2,
      warmups: 0,
      startedAt: "2026-08-05T00:00:00.000Z",
      endedAt: "2026-08-05T00:00:00.010Z",
      durationMs: 10,
    },
    versions: VERSIONS,
  });
}

function successEnvelope(
  catalog: BenchmarkCorpusCatalog,
  planned: BenchmarkCorpusPlanEvidenceEntry
): BenchmarkCorpusFixtureEnvelope {
  const fixture = catalog.fixtures.find(
    (candidate) => candidate.id === planned.fixtureId
  )!;
  return createBenchmarkCorpusFixtureEnvelope({
    attempted: true,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: catalog.identity,
    fixture,
    run: fixtureRun(planned),
    versions: VERSIONS,
    terminalStatus: "success",
    failure: null,
    cold: {
      mode: "realm-cold",
      seed: planned.coldSeed,
      status: "success",
      report: report("success"),
      failure: null,
      skipReason: null,
    },
    warm: {
      mode: "warm",
      seed: planned.warmSeed,
      status: "success",
      report: report("success"),
      failure: null,
      skipReason: null,
    },
  });
}

function failureEnvelope(
  catalog: BenchmarkCorpusCatalog,
  planned: BenchmarkCorpusPlanEvidenceEntry,
  stage: string,
  attempted: boolean,
  terminalStatus: "complete-with-errors" | "cancelled" | "invalidated" =
    "complete-with-errors"
): BenchmarkCorpusFixtureEnvelope {
  const fixture = catalog.fixtures.find(
    (candidate) => candidate.id === planned.fixtureId
  )!;
  return createBenchmarkCorpusFailureEnvelope({
    attempted,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: catalog.identity,
    fixture,
    run: fixtureRun(planned),
    versions: VERSIONS,
    terminalStatus,
    skipReason: stage,
    failure: {
      stage,
      message: `Fixture failed at ${stage}.`,
      detail: null,
    },
  });
}

function fixtureCatalog(size: number): BenchmarkCorpusCatalog {
  const fixtures = Object.freeze(
    Array.from({ length: size }, (_, index) => fixtureDescriptor(index + 1))
  );
  return Object.freeze({
    identity: createBenchmarkCorpusCatalogIdentity(
      "mermaid@test",
      fixtures.length
    ),
    fixtures,
  });
}

function fixtureDescriptor(index: number): BenchmarkCorpusFixtureDescriptor {
  return Object.freeze({
    id: `fixture-${index}`,
    family: `family-${index}`,
    fixture: `fixtures/family-${index}.mmd`,
    order: index,
    source: Object.freeze({
      bytes: index,
      sha256: index.toString(16).padStart(64, "0"),
    }),
  });
}

function fixturePlan(
  catalog: BenchmarkCorpusCatalog
): readonly BenchmarkCorpusPlanEvidenceEntry[] {
  return Object.freeze(
    catalog.fixtures.map((fixture, index) =>
      Object.freeze({
        fixtureId: fixture.id,
        coldSeed: index * 2 + 1,
        warmSeed: index * 2 + 2,
      })
    )
  );
}

function fixtureRun(planned: BenchmarkCorpusPlanEvidenceEntry) {
  return Object.freeze({
    id: `run-${planned.fixtureId}`,
    coldSeed: planned.coldSeed,
    warmSeed: planned.warmSeed,
    iterations: 2,
    warmups: 0,
    startedAt: "2026-08-05T00:00:00.000Z",
    endedAt: "2026-08-05T00:00:00.001Z",
    durationMs: 1,
  });
}

function report(terminalStatus: BenchmarkReport["terminalStatus"]): BenchmarkReport {
  return {
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus,
  } as BenchmarkReport;
}
