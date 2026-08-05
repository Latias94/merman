import type { AvailableDiagramDetectionFacts } from "@mermanjs/web";

import {
  GENERATED_EXAMPLES,
  PLAYGROUND_EXAMPLE_BASELINE,
  PLAYGROUND_MERMAN_VERSION,
} from "../generated/examples.ts";
import { BENCHMARK_BUDGETS } from "../runtime/realm/channel-protocol.ts";
import { projectError } from "../runtime/error-projection.ts";
import type { BenchmarkController, BenchmarkRunRequest } from "./controller.ts";
import {
  createBenchmarkCorpusCatalogIdentity,
  createBenchmarkCorpusFailureEnvelope,
  createBenchmarkCorpusFixtureEnvelope,
  type BenchmarkCorpusCatalog,
  type BenchmarkCorpusFailure,
  type BenchmarkCorpusFixtureDescriptor,
  type BenchmarkCorpusFixtureEnvelope,
  type BenchmarkCorpusModeEvidence,
  type BenchmarkCorpusSourceIdentity,
  type BenchmarkCorpusTerminalStatus,
} from "./corpus-evidence.ts";
import {
  BENCHMARK_CORPUS_CATALOG_ID,
  BENCHMARK_CORPUS_FIXTURE_KIND,
  BENCHMARK_CORPUS_KIND,
  BENCHMARK_CORPUS_SCHEMA_VERSION,
} from "./corpus-schema.ts";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "./report-schema.ts";
import type { BenchmarkReport } from "./report.ts";
import {
  calculateBenchmarkSamplePlanBudget,
  createUint32Random,
  shuffleInPlace,
} from "./sample-plan.ts";
import type { BenchmarkEngine, BenchmarkSampleMode } from "./trace.ts";

export {
  BENCHMARK_CORPUS_CATALOG_ID,
  BENCHMARK_CORPUS_FIXTURE_KIND,
  BENCHMARK_CORPUS_KIND,
  BENCHMARK_CORPUS_SCHEMA_VERSION,
};
export type {
  BenchmarkCorpusAggregateEnvelope,
  BenchmarkCorpusCatalog,
  BenchmarkCorpusCatalogIdentity,
  BenchmarkCorpusCoverage,
  BenchmarkCorpusFailure,
  BenchmarkCorpusFixtureDescriptor,
  BenchmarkCorpusFixtureEnvelope,
  BenchmarkCorpusFixtureEvidence,
  BenchmarkCorpusModeEvidence,
  BenchmarkCorpusPlanEvidenceEntry,
  BenchmarkCorpusSourceIdentity,
  BenchmarkCorpusTerminalStatus,
} from "./corpus-evidence.ts";

export const BENCHMARK_CORPUS_BUDGETS = Object.freeze({
  maxRetainedSamples: 4_096,
});

export interface BenchmarkCorpusFixture {
  readonly detection: AvailableDiagramDetectionFacts;
  readonly family: string;
  readonly fixture: string;
  readonly id: string;
  readonly order: number;
  readonly source: string;
}

export interface BenchmarkCorpusPlanEntry {
  readonly coldSeed: number;
  readonly fixture: BenchmarkCorpusFixture;
  readonly warmSeed: number;
}

export interface BenchmarkCorpusPreparedFixture {
  readonly detection: BenchmarkRunRequest["detection"];
  readonly payload: BenchmarkRunRequest["payload"];
}

export interface BenchmarkCorpusFixtureRunRequest {
  readonly coldSeed: number;
  readonly fixtureId: string;
  readonly iterations: number;
  readonly signal?: AbortSignal;
  readonly warmSeed: number;
  readonly warmups: number;
}

export interface BenchmarkCorpusDependencies {
  readonly controller: Pick<BenchmarkController, "cancel" | "start">;
  dateNow(): number;
  digest(bytes: Uint8Array): Promise<string>;
  now(): number;
  prepareFixture(
    fixture: BenchmarkCorpusFixture
  ): BenchmarkCorpusPreparedFixture;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export interface BenchmarkCorpusOrchestrator {
  cancel(reason?: string): void;
  run(
    request: BenchmarkCorpusFixtureRunRequest
  ): Promise<BenchmarkCorpusFixtureEnvelope>;
}

export const FAMILY_BASELINE_CORPUS: readonly BenchmarkCorpusFixture[] =
  Object.freeze(
    GENERATED_EXAMPLES.filter(
      (example) => example.evidence.role === "family-baseline"
    )
      .map((example) =>
        Object.freeze({
          detection: Object.freeze({
            status: "available",
            validity: "valid",
            diagramType: example.diagramType,
            syntaxId: example.syntaxId,
            effectiveLayoutId: example.effectiveLayoutId,
          }),
          family: example.diagramType,
          fixture: example.fixture,
          id: example.id,
          order: example.order,
          source: example.source,
        })
      )
      .sort(
        (left, right) =>
          left.order - right.order || left.id.localeCompare(right.id)
      )
  );

export const BENCHMARK_CORPUS_MERMAN_VERSION = PLAYGROUND_MERMAN_VERSION;
export const BENCHMARK_CORPUS_CATALOG = createBenchmarkCorpusCatalogIdentity(
  PLAYGROUND_EXAMPLE_BASELINE,
  FAMILY_BASELINE_CORPUS.length
);

validateFamilyCorpus(FAMILY_BASELINE_CORPUS);

export function createBenchmarkCorpusPlan(
  input: Readonly<{
    fixtureIds?: readonly string[];
    masterSeed: number;
  }>,
  catalog: readonly BenchmarkCorpusFixture[] = FAMILY_BASELINE_CORPUS
): readonly BenchmarkCorpusPlanEntry[] {
  validateSeed(input.masterSeed);
  const selected = selectFixtures(input.fixtureIds, catalog);
  const random = createUint32Random(input.masterSeed);
  const ordered = [...selected];
  shuffleInPlace(ordered, random);
  return Object.freeze(
    ordered.map((fixture) =>
      Object.freeze({
        fixture,
        coldSeed: nextSeed(random),
        warmSeed: nextSeed(random),
      })
    )
  );
}

export async function createBenchmarkCorpusCatalog(
  digest: BenchmarkCorpusDependencies["digest"],
  fixtures: readonly BenchmarkCorpusFixture[] = FAMILY_BASELINE_CORPUS
): Promise<BenchmarkCorpusCatalog> {
  const descriptors = await Promise.all(
    fixtures.map((fixture) => describeBenchmarkCorpusFixture(fixture, digest))
  );
  return Object.freeze({
    identity: createBenchmarkCorpusCatalogIdentity(
      PLAYGROUND_EXAMPLE_BASELINE,
      fixtures.length
    ),
    fixtures: Object.freeze(descriptors),
  });
}

export async function describeBenchmarkCorpusFixture(
  fixture: BenchmarkCorpusFixture,
  digest: BenchmarkCorpusDependencies["digest"]
): Promise<BenchmarkCorpusFixtureDescriptor> {
  return Object.freeze({
    id: fixture.id,
    family: fixture.family,
    fixture: fixture.fixture,
    order: fixture.order,
    source: await identifyBenchmarkCorpusSource(fixture.source, digest),
  });
}

export async function identifyBenchmarkCorpusSource(
  source: string,
  digest: BenchmarkCorpusDependencies["digest"]
): Promise<BenchmarkCorpusSourceIdentity> {
  const bytes = new TextEncoder().encode(source);
  const sha256 = await digest(bytes);
  if (!/^[0-9a-f]{64}$/u.test(sha256)) {
    throw new Error(
      "Benchmark corpus digest must be a lowercase SHA-256 hex string."
    );
  }
  return Object.freeze({ bytes: bytes.byteLength, sha256 });
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0")
  ).join("");
}

export function createBenchmarkCorpusOrchestrator(
  dependencies: BenchmarkCorpusDependencies
): BenchmarkCorpusOrchestrator {
  let active: ActiveCorpusRun | null = null;

  const cancel = (reason = "user") => {
    if (!active || active.abort.signal.aborted) return;
    active.abort.abort(reason);
    dependencies.controller.cancel(reason);
  };

  return {
    cancel,
    async run(request) {
      if (active) throw new Error("A benchmark corpus run is already active.");
      const fixture = resolveFixtureRunRequest(request);
      const current: ActiveCorpusRun = {
        abort: new AbortController(),
        stopStatus: null,
      };
      active = current;
      const onAbort = () => cancel(abortReason(request.signal));
      try {
        request.signal?.addEventListener("abort", onAbort, { once: true });
        if (request.signal?.aborted) onAbort();
        return await executeFixture(dependencies, request, fixture, current);
      } finally {
        request.signal?.removeEventListener("abort", onAbort);
        if (active === current) active = null;
      }
    },
  };
}

interface ActiveCorpusRun {
  readonly abort: AbortController;
  stopStatus: "cancelled" | "invalidated" | null;
}

async function executeFixture(
  dependencies: BenchmarkCorpusDependencies,
  request: BenchmarkCorpusFixtureRunRequest,
  fixture: BenchmarkCorpusFixture,
  active: ActiveCorpusRun
): Promise<BenchmarkCorpusFixtureEnvelope> {
  const startedAtMs = dependencies.now();
  const startedAtWallMs = dependencies.dateNow();
  const descriptor = await describeBenchmarkCorpusFixture(
    fixture,
    dependencies.digest
  );
  let attempted = false;
  let prepared: BenchmarkCorpusPreparedFixture;
  try {
    attempted = true;
    prepared = dependencies.prepareFixture(fixture);
  } catch (error) {
    return createFailureEnvelope(
      dependencies,
      request,
      descriptor,
      startedAtMs,
      startedAtWallMs,
      attempted,
      "request",
      error,
      "complete-with-errors"
    );
  }

  const cold = await executeMode(
    dependencies,
    fixture,
    prepared,
    "realm-cold",
    request.coldSeed,
    request,
    active
  );
  const warm =
    active.stopStatus || active.abort.signal.aborted
      ? skippedMode(
          "warm",
          request.warmSeed,
          active.stopStatus ?? abortReason(active.abort.signal)
        )
      : await executeMode(
          dependencies,
          fixture,
          prepared,
          "warm",
          request.warmSeed,
          request,
          active
        );

  const stopStatus = active.stopStatus ??
    (active.abort.signal.aborted ? "cancelled" : null);
  const fixtureFailure = stopStatus
    ? corpusFailure(
        descriptor,
        null,
        stopStatus === "invalidated" ? "document-invalidated" : "cancelled",
        stopStatus === "invalidated"
          ? "Benchmark fixture document was invalidated."
          : `Benchmark fixture was cancelled: ${abortReason(active.abort.signal)}.`,
        null
      )
    : null;
  const terminalStatus: BenchmarkCorpusTerminalStatus = stopStatus ??
    (cold.status === "success" && warm.status === "success"
      ? "success"
      : "complete-with-errors");
  const endedAtMs = dependencies.now();
  const endedAtWallMs = dependencies.dateNow();

  return createBenchmarkCorpusFixtureEnvelope({
    attempted,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: BENCHMARK_CORPUS_CATALOG,
    fixture: descriptor,
    run: fixtureRun(
      request,
      startedAtMs,
      startedAtWallMs,
      endedAtMs,
      endedAtWallMs
    ),
    versions: dependencies.versions,
    terminalStatus,
    failure: fixtureFailure,
    cold,
    warm,
  });
}

async function executeMode(
  dependencies: BenchmarkCorpusDependencies,
  fixture: BenchmarkCorpusFixture,
  prepared: BenchmarkCorpusPreparedFixture,
  mode: BenchmarkSampleMode,
  seed: number,
  options: Pick<BenchmarkCorpusFixtureRunRequest, "iterations" | "warmups">,
  active: ActiveCorpusRun
): Promise<BenchmarkCorpusModeEvidence> {
  if (active.abort.signal.aborted) {
    return skippedMode(mode, seed, abortReason(active.abort.signal));
  }
  const request: BenchmarkRunRequest =
    mode === "warm"
      ? {
          ...prepared,
          mode,
          iterations: options.iterations,
          warmups: options.warmups,
          seed,
          versions: dependencies.versions,
        }
      : {
          ...prepared,
          mode,
          iterations: options.iterations,
          warmups: 0,
          seed,
          versions: dependencies.versions,
        };

  let report: BenchmarkReport;
  try {
    report = await dependencies.controller.start(request).completion;
  } catch (error) {
    return active.abort.signal.aborted
      ? skippedMode(mode, seed, abortReason(active.abort.signal))
      : failedMode(fixture, mode, seed, "controller", error, null);
  }
  if (report.terminalStatus === "success") {
    return Object.freeze({
      mode,
      seed,
      status: "success",
      report,
      failure: null,
      skipReason: null,
    });
  }
  if (report.terminalStatus === "cancelled") {
    active.stopStatus = "cancelled";
    if (!active.abort.signal.aborted) {
      active.abort.abort("controller-cancelled");
    }
    return skippedMode(
      mode,
      seed,
      abortReason(active.abort.signal),
      report
    );
  }
  if (report.terminalStatus === "invalidated") {
    active.stopStatus = "invalidated";
  }
  return failedMode(
    fixture,
    mode,
    seed,
    report.terminalError?.stage ??
      `schema-${BENCHMARK_REPORT_SCHEMA_VERSION}-report`,
    report.terminalError?.message ??
      `Benchmark report ended with ${report.terminalStatus}.`,
    report,
    report.terminalError?.detail ?? null
  );
}

function createFailureEnvelope(
  dependencies: BenchmarkCorpusDependencies,
  request: BenchmarkCorpusFixtureRunRequest,
  fixture: BenchmarkCorpusFixtureDescriptor,
  startedAtMs: number,
  startedAtWallMs: number,
  attempted: boolean,
  stage: string,
  error: unknown,
  terminalStatus: Exclude<BenchmarkCorpusTerminalStatus, "success">
): BenchmarkCorpusFixtureEnvelope {
  const endedAtMs = dependencies.now();
  const endedAtWallMs = dependencies.dateNow();
  const projection = projectError(error);
  return createBenchmarkCorpusFailureEnvelope({
    attempted,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: BENCHMARK_CORPUS_CATALOG,
    fixture,
    run: fixtureRun(
      request,
      startedAtMs,
      startedAtWallMs,
      endedAtMs,
      endedAtWallMs
    ),
    versions: dependencies.versions,
    terminalStatus,
    skipReason: stage,
    failure: {
      stage,
      message: projection.summary,
      detail: projection.detail,
    },
  });
}

function failedMode(
  fixture: BenchmarkCorpusFixture,
  mode: BenchmarkSampleMode,
  seed: number,
  stage: string,
  error: unknown,
  report: BenchmarkReport | null,
  explicitDetail?: string | null
): BenchmarkCorpusModeEvidence {
  const projection = projectError(error);
  const failure = corpusFailure(
    fixture,
    mode,
    stage,
    projection.summary,
    explicitDetail ?? projection.detail
  );
  return Object.freeze({
    mode,
    seed,
    status: "failure",
    report,
    failure,
    skipReason: null,
  });
}

function skippedMode(
  mode: BenchmarkSampleMode,
  seed: number,
  reason: string,
  report: BenchmarkReport | null = null
): BenchmarkCorpusModeEvidence {
  return Object.freeze({
    mode,
    seed,
    status: "skipped",
    report,
    failure: null,
    skipReason: reason,
  });
}

function corpusFailure(
  fixture: Readonly<Pick<BenchmarkCorpusFixture, "family" | "id">>,
  mode: BenchmarkSampleMode | null,
  stage: string,
  message: string,
  detail: string | null
): BenchmarkCorpusFailure {
  return Object.freeze({
    fixtureId: fixture.id,
    family: fixture.family,
    mode,
    stage,
    message,
    detail,
  });
}

function fixtureRun(
  request: BenchmarkCorpusFixtureRunRequest,
  startedAtMs: number,
  startedAtWallMs: number,
  endedAtMs: number,
  endedAtWallMs: number
) {
  return Object.freeze({
    id: `corpus-fixture-${request.fixtureId}-${startedAtWallMs}-${request.coldSeed.toString(16)}`,
    coldSeed: request.coldSeed,
    warmSeed: request.warmSeed,
    iterations: request.iterations,
    warmups: request.warmups,
    startedAt: new Date(startedAtWallMs).toISOString(),
    endedAt: new Date(endedAtWallMs).toISOString(),
    durationMs: Math.max(0, endedAtMs - startedAtMs),
  });
}

function selectFixtures(
  fixtureIds: readonly string[] | undefined,
  catalog: readonly BenchmarkCorpusFixture[]
): readonly BenchmarkCorpusFixture[] {
  if (fixtureIds === undefined) return catalog;
  if (fixtureIds.length === 0) {
    throw new Error("Benchmark corpus fixture selection cannot be empty.");
  }
  const requested = new Set<string>();
  for (const id of fixtureIds) {
    if (typeof id !== "string" || id.length === 0 || requested.has(id)) {
      throw new Error(
        `Benchmark corpus fixture selection is invalid: ${String(id)}.`
      );
    }
    requested.add(id);
  }
  const known = new Set(catalog.map((fixture) => fixture.id));
  for (const id of requested) {
    if (!known.has(id)) {
      throw new Error(`Benchmark corpus has unknown fixture: ${id}.`);
    }
  }
  return catalog.filter((fixture) => requested.has(fixture.id));
}

function selectFixture(fixtureId: string): BenchmarkCorpusFixture {
  if (typeof fixtureId !== "string" || fixtureId.length === 0) {
    throw new Error("Benchmark corpus fixture id is required.");
  }
  const fixture = FAMILY_BASELINE_CORPUS.find(
    (candidate) => candidate.id === fixtureId
  );
  if (!fixture) {
    throw new Error(`Benchmark corpus has unknown fixture: ${fixtureId}.`);
  }
  return fixture;
}

function validateFamilyCorpus(catalog: readonly BenchmarkCorpusFixture[]): void {
  if (catalog.length === 0) throw new Error("Benchmark family corpus is empty.");
  const ids = new Set<string>();
  const families = new Set<string>();
  let previousOrder = 0;
  for (const fixture of catalog) {
    if (
      ids.has(fixture.id) ||
      families.has(fixture.family) ||
      fixture.order <= previousOrder ||
      fixture.source.length === 0
    ) {
      throw new Error("Generated benchmark family corpus is invalid.");
    }
    ids.add(fixture.id);
    families.add(fixture.family);
    previousOrder = fixture.order;
  }
}

function resolveFixtureRunRequest(
  request: BenchmarkCorpusFixtureRunRequest
): BenchmarkCorpusFixture {
  const fixture = selectFixture(request.fixtureId);
  validateSeed(request.coldSeed);
  validateSeed(request.warmSeed);
  validateBenchmarkCorpusRunBudget(request, 1);
  return fixture;
}

function validateBenchmarkCorpusOptions(
  request: Readonly<{ iterations: number; warmups: number }>
): Readonly<{ coldSamples: number; warmSamples: number }> {
  let coldSamples: number;
  try {
    coldSamples = calculateBenchmarkSamplePlanBudget({
      iterations: request.iterations,
      mode: "realm-cold",
    }).totalSamples;
  } catch {
    throw new Error(
      `Benchmark corpus iterations must be an even integer from 2 to ${BENCHMARK_BUDGETS.maxIterations}.`
    );
  }
  let warmSamples: number;
  try {
    warmSamples = calculateBenchmarkSamplePlanBudget({
      iterations: request.iterations,
      mode: "warm",
      warmups: request.warmups,
    }).totalSamples;
  } catch {
    throw new Error("Benchmark corpus warmup budget is invalid.");
  }
  return Object.freeze({ coldSamples, warmSamples });
}

export function validateBenchmarkCorpusRunBudget(
  request: Readonly<{ iterations: number; warmups: number }>,
  selectedFamilies: number
): void {
  const { coldSamples, warmSamples } = validateBenchmarkCorpusOptions(request);
  if (!Number.isSafeInteger(selectedFamilies) || selectedFamilies < 1) {
    throw new Error("Benchmark corpus selected-family count is invalid.");
  }
  const retainedSamples =
    selectedFamilies * (coldSamples + warmSamples);
  if (retainedSamples > BENCHMARK_CORPUS_BUDGETS.maxRetainedSamples) {
    throw new Error(
      `Benchmark corpus would retain ${retainedSamples} samples; the whole-corpus budget is ${BENCHMARK_CORPUS_BUDGETS.maxRetainedSamples}.`
    );
  }
}

function validateSeed(seed: number): void {
  if (!Number.isSafeInteger(seed) || seed < 0 || seed > 0xffff_ffff) {
    throw new Error("Benchmark corpus seed must be an unsigned 32-bit integer.");
  }
}

function nextSeed(random: () => number): number {
  return Math.floor(random() * 0x1_0000_0000) >>> 0;
}

function abortReason(signal: AbortSignal | undefined): string {
  return typeof signal?.reason === "string" && signal.reason.length > 0
    ? signal.reason
    : "abort-signal";
}
