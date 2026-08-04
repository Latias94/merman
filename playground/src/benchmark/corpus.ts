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
  BENCHMARK_REPORT_SCHEMA_VERSION,
  type BenchmarkReport,
} from "./report.ts";
import { createUint32Random, shuffleInPlace } from "./schedule.ts";
import type { BenchmarkEngine, BenchmarkSampleMode } from "./trace.ts";
import {
  BENCHMARK_CORPUS_KIND,
  BENCHMARK_CORPUS_SCHEMA_VERSION,
} from "./corpus-schema.ts";

export { BENCHMARK_CORPUS_KIND, BENCHMARK_CORPUS_SCHEMA_VERSION };

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

export interface BenchmarkCorpusSourceIdentity {
  readonly bytes: number;
  readonly sha256: string;
}

export interface BenchmarkCorpusFailure {
  readonly detail: string | null;
  readonly fixtureId: string;
  readonly family: string;
  readonly message: string;
  readonly mode: BenchmarkSampleMode;
  readonly stage: string;
}

interface BenchmarkCorpusModeEvidenceBase {
  readonly mode: BenchmarkSampleMode;
}

interface BenchmarkCorpusModeSuccess extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: null;
  readonly report: BenchmarkReport;
  readonly seed: number;
  readonly skipReason: null;
  readonly status: "success";
}

interface BenchmarkCorpusModeFailure extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: BenchmarkCorpusFailure;
  readonly report: BenchmarkReport | null;
  readonly seed: number;
  readonly skipReason: null;
  readonly status: "failure";
}

interface BenchmarkCorpusModeSkipped extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: null;
  readonly report: BenchmarkReport | null;
  readonly seed: number | null;
  readonly skipReason: string;
  readonly status: "skipped";
}

export type BenchmarkCorpusModeEvidence =
  | BenchmarkCorpusModeSuccess
  | BenchmarkCorpusModeFailure
  | BenchmarkCorpusModeSkipped;

export interface BenchmarkCorpusPreparedFixture {
  readonly detection: BenchmarkRunRequest["detection"];
  readonly payload: BenchmarkRunRequest["payload"];
}

export interface BenchmarkCorpusFixtureEvidence {
  readonly cold: BenchmarkCorpusModeEvidence;
  readonly family: string;
  readonly fixture: string;
  readonly id: string;
  readonly order: number;
  readonly source: BenchmarkCorpusSourceIdentity;
  readonly status: "success" | "failure" | "skipped";
  readonly warm: BenchmarkCorpusModeEvidence;
}

export interface BenchmarkCorpusCoverage {
  readonly attemptedFamilies: number;
  readonly availableFamilies: number;
  readonly failedFamilies: number;
  readonly selectedFamilies: number;
  readonly skippedFamilies: number;
  readonly succeededFamilies: number;
}

export interface BenchmarkCorpusSkip {
  readonly family: string;
  readonly fixtureId: string;
  readonly reason: string;
}

export interface BenchmarkCorpusEnvelope {
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: {
    readonly mermaidBaseline: string;
    readonly role: "family-baseline";
    readonly source: "playground/src/generated/examples.ts";
  };
  readonly coverage: BenchmarkCorpusCoverage;
  readonly execution: {
    readonly fixtureIsolation:
      | "single-page"
      | "fresh-browser-process-per-fixture";
  };
  readonly failures: readonly BenchmarkCorpusFailure[];
  readonly fixtures: readonly BenchmarkCorpusFixtureEvidence[];
  readonly kind: typeof BENCHMARK_CORPUS_KIND;
  readonly run: {
    readonly durationMs: number;
    readonly endedAt: string;
    readonly id: string;
    readonly iterations: number;
    readonly masterSeed: number;
    readonly order: readonly string[];
    readonly startedAt: string;
    readonly warmups: number;
  };
  readonly schemaVersion: typeof BENCHMARK_CORPUS_SCHEMA_VERSION;
  readonly skips: readonly BenchmarkCorpusSkip[];
  readonly terminalStatus:
    | "success"
    | "complete-with-errors"
    | "cancelled"
    | "invalidated";
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export interface BenchmarkCorpusRunRequest {
  readonly fixtureIds?: readonly string[];
  readonly iterations: number;
  readonly masterSeed: number;
  readonly signal?: AbortSignal;
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
  run(request: BenchmarkCorpusRunRequest): Promise<BenchmarkCorpusEnvelope>;
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
      .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id))
  );

export const BENCHMARK_CORPUS_MERMAN_VERSION = PLAYGROUND_MERMAN_VERSION;

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

export async function identifyBenchmarkCorpusSource(
  source: string,
  digest: BenchmarkCorpusDependencies["digest"]
): Promise<BenchmarkCorpusSourceIdentity> {
  const bytes = new TextEncoder().encode(source);
  const sha256 = await digest(bytes);
  if (!/^[0-9a-f]{64}$/u.test(sha256)) {
    throw new Error("Benchmark corpus digest must be a lowercase SHA-256 hex string.");
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
      validateRunRequest(request);
      const plan = createBenchmarkCorpusPlan(request);
      validateBenchmarkCorpusRunBudget(request, plan.length);
      const current: ActiveCorpusRun = {
        abort: new AbortController(),
        stopStatus: null,
      };
      active = current;
      const onAbort = () => cancel(abortReason(request.signal));
      try {
        request.signal?.addEventListener("abort", onAbort, { once: true });
        if (request.signal?.aborted) onAbort();
        return await executeCorpus(dependencies, request, plan, current);
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

interface MutableFixtureEvidence {
  cold: BenchmarkCorpusModeEvidence;
  readonly fixture: BenchmarkCorpusFixture;
  readonly source: BenchmarkCorpusSourceIdentity;
  warm: BenchmarkCorpusModeEvidence;
}

async function executeCorpus(
  dependencies: BenchmarkCorpusDependencies,
  request: BenchmarkCorpusRunRequest,
  plan: readonly BenchmarkCorpusPlanEntry[],
  active: ActiveCorpusRun
): Promise<BenchmarkCorpusEnvelope> {
  const startedAtMs = dependencies.now();
  const startedAtWallMs = dependencies.dateNow();
  const planById = new Map(plan.map((entry) => [entry.fixture.id, entry]));
  const evidence: MutableFixtureEvidence[] = [];
  for (const fixture of FAMILY_BASELINE_CORPUS) {
    const source = await identifyBenchmarkCorpusSource(
      fixture.source,
      dependencies.digest
    );
    const planned = planById.get(fixture.id);
    evidence.push({
      fixture,
      source,
      cold: planned
        ? skippedMode("realm-cold", planned.coldSeed, "pending")
        : skippedMode("realm-cold", null, "not-selected"),
      warm: planned
        ? skippedMode("warm", planned.warmSeed, "pending")
        : skippedMode("warm", null, "not-selected"),
    });
  }
  const evidenceById = new Map(
    evidence.map((candidate) => [candidate.fixture.id, candidate])
  );

  for (const planned of plan) {
    const current = evidenceById.get(planned.fixture.id)!;
    if (active.abort.signal.aborted) break;
    let prepared: BenchmarkCorpusPreparedFixture;
    try {
      prepared = dependencies.prepareFixture(planned.fixture);
    } catch (error) {
      current.cold = failedMode(
        planned.fixture,
        "realm-cold",
        planned.coldSeed,
        "request",
        error,
        null
      );
      current.warm = failedMode(
        planned.fixture,
        "warm",
        planned.warmSeed,
        "request",
        error,
        null
      );
      continue;
    }
    current.cold = await executeMode(
      dependencies,
      planned.fixture,
      prepared,
      "realm-cold",
      planned.coldSeed,
      request,
      active
    );
    if (active.stopStatus || active.abort.signal.aborted) break;
    current.warm = await executeMode(
      dependencies,
      planned.fixture,
      prepared,
      "warm",
      planned.warmSeed,
      request,
      active
    );
    if (active.stopStatus || active.abort.signal.aborted) break;
  }

  const unfinishedReason = active.abort.signal.aborted
    ? abortReason(active.abort.signal)
    : active.stopStatus ?? "not-run";
  for (const candidate of evidence) {
    if (candidate.cold.skipReason === "pending") {
      candidate.cold = skippedMode(
        "realm-cold",
        candidate.cold.seed,
        unfinishedReason
      );
    }
    if (candidate.warm.skipReason === "pending") {
      candidate.warm = skippedMode("warm", candidate.warm.seed, unfinishedReason);
    }
  }

  const fixtures = Object.freeze(evidence.map(freezeFixtureEvidence));
  const failures = Object.freeze(
    fixtures.flatMap((fixture) =>
      [fixture.cold.failure, fixture.warm.failure].filter(
        (failure): failure is BenchmarkCorpusFailure => failure !== null
      )
    )
  );
  const skips = Object.freeze(
    fixtures
      .filter((fixture) => fixture.status === "skipped")
      .map((fixture) =>
        Object.freeze({
          family: fixture.family,
          fixtureId: fixture.id,
          reason:
            fixture.cold.skipReason ?? fixture.warm.skipReason ?? "not-run",
        })
      )
  );
  const coverage = buildCoverage(fixtures, plan.length);
  const endedAtMs = dependencies.now();
  const endedAtWallMs = dependencies.dateNow();
  const terminalStatus =
    active.stopStatus ??
    (active.abort.signal.aborted
      ? "cancelled"
      : failures.length > 0
        ? "complete-with-errors"
        : "success");

  return Object.freeze({
    schemaVersion: BENCHMARK_CORPUS_SCHEMA_VERSION,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    kind: BENCHMARK_CORPUS_KIND,
    execution: Object.freeze({ fixtureIsolation: "single-page" }),
    catalog: Object.freeze({
      mermaidBaseline: PLAYGROUND_EXAMPLE_BASELINE,
      role: "family-baseline",
      source: "playground/src/generated/examples.ts",
    }),
    run: Object.freeze({
      id: `corpus-${startedAtWallMs}-${request.masterSeed.toString(16)}`,
      masterSeed: request.masterSeed,
      order: Object.freeze(plan.map((entry) => entry.fixture.id)),
      iterations: request.iterations,
      warmups: request.warmups,
      startedAt: new Date(startedAtWallMs).toISOString(),
      endedAt: new Date(endedAtWallMs).toISOString(),
      durationMs: Math.max(0, endedAtMs - startedAtMs),
    }),
    versions: Object.freeze({ ...dependencies.versions }),
    terminalStatus,
    coverage,
    failures,
    skips,
    fixtures,
  });
}

async function executeMode(
  dependencies: BenchmarkCorpusDependencies,
  fixture: BenchmarkCorpusFixture,
  prepared: BenchmarkCorpusPreparedFixture,
  mode: BenchmarkSampleMode,
  seed: number,
  options: Pick<BenchmarkCorpusRunRequest, "iterations" | "warmups">,
  active: ActiveCorpusRun
): Promise<BenchmarkCorpusModeEvidence> {
  if (active.abort.signal.aborted) {
    return skippedMode(mode, seed, abortReason(active.abort.signal));
  }
  const request: BenchmarkRunRequest = {
    ...prepared,
    mode,
    iterations: options.iterations,
    warmups: mode === "warm" ? options.warmups : 0,
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
    report.terminalError?.stage ?? "schema-5-report",
    report.terminalError?.message ??
      `Benchmark report ended with ${report.terminalStatus}.`,
    report,
    report.terminalError?.detail ?? null
  );
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
  const failure = Object.freeze({
    fixtureId: fixture.id,
    family: fixture.family,
    mode,
    stage,
    message: projection.summary,
    detail: explicitDetail ?? projection.detail,
  });
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
  seed: number | null,
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

function freezeFixtureEvidence(
  candidate: MutableFixtureEvidence
): BenchmarkCorpusFixtureEvidence {
  const status =
    candidate.cold.status === "success" && candidate.warm.status === "success"
      ? "success"
      : candidate.cold.status === "failure" || candidate.warm.status === "failure"
        ? "failure"
        : "skipped";
  return Object.freeze({
    id: candidate.fixture.id,
    family: candidate.fixture.family,
    fixture: candidate.fixture.fixture,
    order: candidate.fixture.order,
    source: candidate.source,
    status,
    cold: candidate.cold,
    warm: candidate.warm,
  });
}

function buildCoverage(
  fixtures: readonly BenchmarkCorpusFixtureEvidence[],
  selectedFamilies: number
): BenchmarkCorpusCoverage {
  return Object.freeze({
    availableFamilies: fixtures.length,
    selectedFamilies,
    attemptedFamilies: fixtures.filter(
      (fixture) =>
        fixture.cold.report !== null ||
        fixture.warm.report !== null ||
        fixture.cold.failure !== null ||
        fixture.warm.failure !== null
    ).length,
    succeededFamilies: fixtures.filter((fixture) => fixture.status === "success")
      .length,
    failedFamilies: fixtures.filter((fixture) => fixture.status === "failure")
      .length,
    skippedFamilies: fixtures.filter((fixture) => fixture.status === "skipped")
      .length,
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
      throw new Error(`Benchmark corpus fixture selection is invalid: ${String(id)}.`);
    }
    requested.add(id);
  }
  const known = new Set(catalog.map((fixture) => fixture.id));
  for (const id of requested) {
    if (!known.has(id)) throw new Error(`Benchmark corpus has unknown fixture: ${id}.`);
  }
  return catalog.filter((fixture) => requested.has(fixture.id));
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

function validateRunRequest(request: BenchmarkCorpusRunRequest): void {
  validateSeed(request.masterSeed);
  if (
    !Number.isSafeInteger(request.iterations) ||
    request.iterations < 2 ||
    request.iterations > BENCHMARK_BUDGETS.maxIterations ||
    request.iterations % 2 !== 0
  ) {
    throw new Error(
      `Benchmark corpus iterations must be an even integer from 2 to ${BENCHMARK_BUDGETS.maxIterations}.`
    );
  }
  if (
    !Number.isSafeInteger(request.warmups) ||
    request.warmups < 0 ||
    request.warmups + 1 > BENCHMARK_BUDGETS.maxWarmups ||
    (request.iterations + request.warmups + 1) * 2 >
      BENCHMARK_BUDGETS.maxRetainedSamples
  ) {
    throw new Error("Benchmark corpus warmup budget is invalid.");
  }
}

export function validateBenchmarkCorpusRunBudget(
  request: Pick<BenchmarkCorpusRunRequest, "iterations" | "warmups">,
  selectedFamilies: number
): void {
  if (!Number.isSafeInteger(selectedFamilies) || selectedFamilies < 1) {
    throw new Error("Benchmark corpus selected-family count is invalid.");
  }
  const coldSamplesPerFixture = request.iterations * 2;
  const warmSamplesPerFixture =
    (request.iterations + request.warmups + 1) * 2;
  const retainedSamples =
    selectedFamilies * (coldSamplesPerFixture + warmSamplesPerFixture);
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
