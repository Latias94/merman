import {
  BENCHMARK_CORPUS_CATALOG_ID,
  BENCHMARK_CORPUS_FIXTURE_KIND,
  BENCHMARK_CORPUS_KIND,
  BENCHMARK_CORPUS_SCHEMA_VERSION,
} from "./corpus-schema.ts";
import type {
  BENCHMARK_REPORT_SCHEMA_VERSION,
  BenchmarkReport,
} from "./report.ts";
import type {
  BenchmarkEngine,
  BenchmarkSampleMode,
} from "./trace.ts";

export type BenchmarkCorpusTerminalStatus =
  | "success"
  | "complete-with-errors"
  | "cancelled"
  | "invalidated";

export interface BenchmarkCorpusCatalogIdentity {
  readonly availableFamilies: number;
  readonly id: typeof BENCHMARK_CORPUS_CATALOG_ID;
  readonly mermaidBaseline: string;
  readonly role: "family-baseline";
  readonly source: "playground/src/generated/examples.ts";
}

export interface BenchmarkCorpusSourceIdentity {
  readonly bytes: number;
  readonly sha256: string;
}

export interface BenchmarkCorpusFixtureDescriptor {
  readonly family: string;
  readonly fixture: string;
  readonly id: string;
  readonly order: number;
  readonly source: BenchmarkCorpusSourceIdentity;
}

export interface BenchmarkCorpusCatalog {
  readonly fixtures: readonly BenchmarkCorpusFixtureDescriptor[];
  readonly identity: BenchmarkCorpusCatalogIdentity;
}

export interface BenchmarkCorpusFailure {
  readonly detail: string | null;
  readonly fixtureId: string;
  readonly family: string;
  readonly message: string;
  readonly mode: BenchmarkSampleMode | null;
  readonly stage: string;
}

interface BenchmarkCorpusModeEvidenceBase {
  readonly mode: BenchmarkSampleMode;
  readonly seed: number;
}

export interface BenchmarkCorpusModeSuccess
  extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: null;
  readonly report: BenchmarkReport;
  readonly skipReason: null;
  readonly status: "success";
}

export interface BenchmarkCorpusModeFailure
  extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: BenchmarkCorpusFailure;
  readonly report: BenchmarkReport | null;
  readonly skipReason: null;
  readonly status: "failure";
}

export interface BenchmarkCorpusModeSkipped
  extends BenchmarkCorpusModeEvidenceBase {
  readonly failure: null;
  readonly report: BenchmarkReport | null;
  readonly skipReason: string;
  readonly status: "skipped";
}

export type BenchmarkCorpusModeEvidence =
  | BenchmarkCorpusModeSuccess
  | BenchmarkCorpusModeFailure
  | BenchmarkCorpusModeSkipped;

export interface BenchmarkCorpusFixtureEvidence
  extends BenchmarkCorpusFixtureDescriptor {
  readonly attempted: boolean;
  readonly cold: BenchmarkCorpusModeEvidence;
  readonly failure: BenchmarkCorpusFailure | null;
  readonly status: "success" | "failure";
  readonly warm: BenchmarkCorpusModeEvidence;
}

export interface BenchmarkCorpusFixtureRun {
  readonly coldSeed: number;
  readonly durationMs: number;
  readonly endedAt: string;
  readonly id: string;
  readonly iterations: number;
  readonly startedAt: string;
  readonly warmSeed: number;
  readonly warmups: number;
}

export interface BenchmarkCorpusFixtureEnvelope {
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: BenchmarkCorpusCatalogIdentity;
  readonly execution: {
    readonly fixtureIsolation: "single-page";
  };
  readonly fixture: BenchmarkCorpusFixtureEvidence;
  readonly fixtureId: string;
  readonly kind: typeof BENCHMARK_CORPUS_FIXTURE_KIND;
  readonly run: BenchmarkCorpusFixtureRun;
  readonly schemaVersion: typeof BENCHMARK_CORPUS_SCHEMA_VERSION;
  readonly terminalStatus: BenchmarkCorpusTerminalStatus;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export interface BenchmarkCorpusPlanEvidenceEntry {
  readonly coldSeed: number;
  readonly fixtureId: string;
  readonly warmSeed: number;
}

export interface BenchmarkCorpusCoverage {
  readonly attemptedFamilies: number;
  readonly availableFamilies: number;
  readonly failedFamilies: number;
  readonly selectedFamilies: number;
  readonly succeededFamilies: number;
}

export interface BenchmarkCorpusAggregateRun {
  readonly durationMs: number;
  readonly endedAt: string;
  readonly id: string;
  readonly iterations: number;
  readonly masterSeed: number;
  readonly order: readonly string[];
  readonly startedAt: string;
  readonly warmups: number;
}

export interface BenchmarkCorpusAggregateEnvelope {
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: BenchmarkCorpusCatalogIdentity;
  readonly coverage: BenchmarkCorpusCoverage;
  readonly execution: {
    readonly fixtureIsolation: "fresh-browser-process-per-fixture";
  };
  readonly failures: readonly BenchmarkCorpusFailure[];
  readonly fixtures: readonly BenchmarkCorpusFixtureEvidence[];
  readonly kind: typeof BENCHMARK_CORPUS_KIND;
  readonly run: BenchmarkCorpusAggregateRun;
  readonly schemaVersion: typeof BENCHMARK_CORPUS_SCHEMA_VERSION;
  readonly terminalStatus: BenchmarkCorpusTerminalStatus;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

interface BenchmarkCorpusFixtureEnvelopeInput {
  readonly attempted: boolean;
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: BenchmarkCorpusCatalogIdentity;
  readonly cold: BenchmarkCorpusModeEvidence;
  readonly failure: BenchmarkCorpusFailure | null;
  readonly fixture: BenchmarkCorpusFixtureDescriptor;
  readonly run: BenchmarkCorpusFixtureRun;
  readonly terminalStatus: BenchmarkCorpusTerminalStatus;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
  readonly warm: BenchmarkCorpusModeEvidence;
}

export interface BenchmarkCorpusFailureEnvelopeInput {
  readonly attempted: boolean;
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: BenchmarkCorpusCatalogIdentity;
  readonly failure: Readonly<Pick<BenchmarkCorpusFailure, "detail" | "message" | "stage">>;
  readonly fixture: BenchmarkCorpusFixtureDescriptor;
  readonly run: BenchmarkCorpusFixtureRun;
  readonly skipReason: string;
  readonly terminalStatus: Exclude<BenchmarkCorpusTerminalStatus, "success">;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export interface BenchmarkCorpusAggregateInput {
  readonly benchmarkReportSchemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly catalog: BenchmarkCorpusCatalog;
  readonly fixtureEnvelopes: readonly BenchmarkCorpusFixtureEnvelope[];
  readonly plan: readonly BenchmarkCorpusPlanEvidenceEntry[];
  readonly run: Omit<BenchmarkCorpusAggregateRun, "order">;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export function createBenchmarkCorpusCatalogIdentity(
  mermaidBaseline: string,
  availableFamilies: number
): BenchmarkCorpusCatalogIdentity {
  if (mermaidBaseline.length === 0) {
    throw new Error("Benchmark corpus Mermaid baseline cannot be empty.");
  }
  if (!Number.isSafeInteger(availableFamilies) || availableFamilies < 1) {
    throw new Error("Benchmark corpus available-family count is invalid.");
  }
  return Object.freeze({
    id: BENCHMARK_CORPUS_CATALOG_ID,
    mermaidBaseline,
    role: "family-baseline",
    source: "playground/src/generated/examples.ts",
    availableFamilies,
  });
}

export function createBenchmarkCorpusFixtureEnvelope(
  input: BenchmarkCorpusFixtureEnvelopeInput
): BenchmarkCorpusFixtureEnvelope {
  const cold = freezeModeEvidence(input.cold);
  const warm = freezeModeEvidence(input.warm);
  validateModeEvidence(cold, "realm-cold", input.run.coldSeed);
  validateModeEvidence(warm, "warm", input.run.warmSeed);
  const failure = input.failure === null ? null : freezeFailure(input.failure);
  const status =
    failure === null && cold.status === "success" && warm.status === "success"
      ? "success"
      : "failure";
  if (
    status === "failure" &&
    failure === null &&
    cold.failure === null &&
    warm.failure === null
  ) {
    throw new Error(
      `Benchmark fixture ${input.fixture.id} failed without structured evidence.`
    );
  }
  if (
    (status === "success") !== (input.terminalStatus === "success")
  ) {
    throw new Error(
      `Benchmark fixture ${input.fixture.id} terminal status does not match its evidence.`
    );
  }
  const fixture = Object.freeze({
    ...freezeFixtureDescriptor(input.fixture),
    attempted: input.attempted,
    status,
    failure,
    cold,
    warm,
  });
  const envelope = Object.freeze({
    schemaVersion: BENCHMARK_CORPUS_SCHEMA_VERSION,
    benchmarkReportSchemaVersion: input.benchmarkReportSchemaVersion,
    kind: BENCHMARK_CORPUS_FIXTURE_KIND,
    execution: Object.freeze({ fixtureIsolation: "single-page" as const }),
    catalog: freezeCatalogIdentity(input.catalog),
    fixtureId: fixture.id,
    run: freezeFixtureRun(input.run),
    versions: freezeVersions(input.versions),
    terminalStatus: input.terminalStatus,
    fixture,
  });
  return envelope;
}

export function createBenchmarkCorpusFailureEnvelope(
  input: BenchmarkCorpusFailureEnvelopeInput
): BenchmarkCorpusFixtureEnvelope {
  const failure = freezeFailure({
    fixtureId: input.fixture.id,
    family: input.fixture.family,
    mode: null,
    stage: input.failure.stage,
    message: input.failure.message,
    detail: input.failure.detail,
  });
  return createBenchmarkCorpusFixtureEnvelope({
    attempted: input.attempted,
    benchmarkReportSchemaVersion: input.benchmarkReportSchemaVersion,
    catalog: input.catalog,
    fixture: input.fixture,
    run: input.run,
    versions: input.versions,
    terminalStatus: input.terminalStatus,
    failure,
    cold: skippedMode("realm-cold", input.run.coldSeed, input.skipReason),
    warm: skippedMode("warm", input.run.warmSeed, input.skipReason),
  });
}

export function assembleBenchmarkCorpusAggregate(
  input: BenchmarkCorpusAggregateInput
): BenchmarkCorpusAggregateEnvelope {
  const catalogById = validateAndIndexCatalog(input.catalog);
  if (input.plan.length === 0) {
    throw new Error("Benchmark corpus aggregate plan cannot be empty.");
  }
  if (input.fixtureEnvelopes.length !== input.plan.length) {
    throw new Error(
      `Benchmark corpus aggregate expected ${input.plan.length} fixture envelopes, received ${input.fixtureEnvelopes.length}.`
    );
  }

  const selectedIds = new Set<string>();
  const fixtures: BenchmarkCorpusFixtureEvidence[] = [];
  const failures: BenchmarkCorpusFailure[] = [];
  let attemptedFamilies = 0;
  let failedFamilies = 0;
  let succeededFamilies = 0;
  let terminalStatus: BenchmarkCorpusTerminalStatus = "success";

  for (let index = 0; index < input.plan.length; index += 1) {
    const planned = input.plan[index]!;
    validateSeed(planned.coldSeed, "cold");
    validateSeed(planned.warmSeed, "warm");
    if (selectedIds.has(planned.fixtureId)) {
      throw new Error(
        `Benchmark corpus aggregate plan contains duplicate fixture ${planned.fixtureId}.`
      );
    }
    selectedIds.add(planned.fixtureId);
    const catalogFixture = catalogById.get(planned.fixtureId);
    if (!catalogFixture) {
      throw new Error(
        `Benchmark corpus aggregate plan contains unknown fixture ${planned.fixtureId}.`
      );
    }
    const envelope = input.fixtureEnvelopes[index]!;
    validateFixtureEnvelopeAgainstPlan(
      envelope,
      planned,
      catalogFixture,
      input
    );
    const fixture = freezeFixtureEvidence(envelope.fixture);
    fixtures.push(fixture);
    if (fixture.attempted) attemptedFamilies += 1;
    if (fixture.status === "success") succeededFamilies += 1;
    else failedFamilies += 1;
    if (fixture.failure) failures.push(fixture.failure);
    if (fixture.cold.failure) failures.push(fixture.cold.failure);
    if (fixture.warm.failure) failures.push(fixture.warm.failure);
    terminalStatus = mergeTerminalStatus(
      terminalStatus,
      envelope.terminalStatus
    );
  }

  const coverage = Object.freeze({
    availableFamilies: input.catalog.identity.availableFamilies,
    selectedFamilies: input.plan.length,
    attemptedFamilies,
    succeededFamilies,
    failedFamilies,
  });
  return Object.freeze({
    schemaVersion: BENCHMARK_CORPUS_SCHEMA_VERSION,
    benchmarkReportSchemaVersion: input.benchmarkReportSchemaVersion,
    kind: BENCHMARK_CORPUS_KIND,
    execution: Object.freeze({
      fixtureIsolation: "fresh-browser-process-per-fixture" as const,
    }),
    catalog: freezeCatalogIdentity(input.catalog.identity),
    run: Object.freeze({
      ...freezeAggregateRun(input.run),
      order: Object.freeze(input.plan.map((entry) => entry.fixtureId)),
    }),
    versions: freezeVersions(input.versions),
    terminalStatus,
    coverage,
    failures: Object.freeze(failures),
    fixtures: Object.freeze(fixtures),
  });
}

export function validateBenchmarkCorpusFixtureEnvelope(
  envelope: BenchmarkCorpusFixtureEnvelope
): void {
  if (
    envelope.schemaVersion !== BENCHMARK_CORPUS_SCHEMA_VERSION ||
    envelope.kind !== BENCHMARK_CORPUS_FIXTURE_KIND ||
    envelope.execution.fixtureIsolation !== "single-page" ||
    envelope.fixtureId !== envelope.fixture.id
  ) {
    throw new Error("Benchmark fixture envelope identity is invalid.");
  }
  validateCatalogIdentity(envelope.catalog);
  validateFixtureDescriptor(envelope.fixture);
  validateFixtureRun(envelope.run);
  validateVersions(envelope.versions);
  validateTerminalStatus(envelope.terminalStatus);
  if (typeof envelope.fixture.attempted !== "boolean") {
    throw new Error("Benchmark fixture attempted state is invalid.");
  }
  validateModeEvidence(envelope.fixture.cold, "realm-cold", envelope.run.coldSeed);
  validateModeEvidence(envelope.fixture.warm, "warm", envelope.run.warmSeed);
  for (const evidence of [envelope.fixture.cold, envelope.fixture.warm]) {
    if (
      evidence.report !== null &&
      evidence.report.schemaVersion !== envelope.benchmarkReportSchemaVersion
    ) {
      throw new Error("Benchmark fixture report schema version drifted.");
    }
  }
  if (envelope.fixture.failure) {
    validateFailure(envelope.fixture.failure, envelope.fixture, null);
  }
  if (envelope.fixture.cold.failure) {
    validateFailure(envelope.fixture.cold.failure, envelope.fixture, "realm-cold");
  }
  if (envelope.fixture.warm.failure) {
    validateFailure(envelope.fixture.warm.failure, envelope.fixture, "warm");
  }
  const isSuccess =
    envelope.fixture.failure === null &&
    envelope.fixture.cold.status === "success" &&
    envelope.fixture.warm.status === "success";
  if (
    isSuccess !== (envelope.fixture.status === "success") ||
    isSuccess !== (envelope.terminalStatus === "success")
  ) {
    throw new Error("Benchmark fixture envelope status is inconsistent.");
  }
  if (
    !isSuccess &&
    envelope.fixture.failure === null &&
    envelope.fixture.cold.failure === null &&
    envelope.fixture.warm.failure === null
  ) {
    throw new Error("Benchmark fixture envelope lacks structured failure evidence.");
  }
}

function validateFixtureEnvelopeAgainstPlan(
  envelope: BenchmarkCorpusFixtureEnvelope,
  planned: BenchmarkCorpusPlanEvidenceEntry,
  catalogFixture: BenchmarkCorpusFixtureDescriptor,
  input: BenchmarkCorpusAggregateInput
): void {
  validateBenchmarkCorpusFixtureEnvelope(envelope);
  if (
    envelope.fixtureId !== planned.fixtureId ||
    envelope.run.coldSeed !== planned.coldSeed ||
    envelope.run.warmSeed !== planned.warmSeed ||
    envelope.run.iterations !== input.run.iterations ||
    envelope.run.warmups !== input.run.warmups
  ) {
    throw new Error(
      `Benchmark fixture envelope contract drifted for ${planned.fixtureId}.`
    );
  }
  if (
    envelope.benchmarkReportSchemaVersion !==
      input.benchmarkReportSchemaVersion ||
    !sameCatalogIdentity(envelope.catalog, input.catalog.identity) ||
    !sameVersions(envelope.versions, input.versions) ||
    !sameFixtureDescriptor(envelope.fixture, catalogFixture)
  ) {
    throw new Error(
      `Benchmark fixture envelope provenance drifted for ${planned.fixtureId}.`
    );
  }
}

function validateAndIndexCatalog(
  catalog: BenchmarkCorpusCatalog
): ReadonlyMap<string, BenchmarkCorpusFixtureDescriptor> {
  validateCatalogIdentity(catalog.identity);
  if (catalog.fixtures.length !== catalog.identity.availableFamilies) {
    throw new Error("Benchmark corpus catalog size does not match its identity.");
  }
  const ids = new Set<string>();
  const families = new Set<string>();
  const fixturesById = new Map<string, BenchmarkCorpusFixtureDescriptor>();
  let previousOrder = Number.NEGATIVE_INFINITY;
  for (const fixture of catalog.fixtures) {
    validateFixtureDescriptor(fixture);
    if (
      ids.has(fixture.id) ||
      families.has(fixture.family) ||
      fixture.order <= previousOrder
    ) {
      throw new Error("Benchmark corpus catalog order or identity is invalid.");
    }
    ids.add(fixture.id);
    families.add(fixture.family);
    fixturesById.set(fixture.id, fixture);
    previousOrder = fixture.order;
  }
  return fixturesById;
}

function validateCatalogIdentity(identity: BenchmarkCorpusCatalogIdentity): void {
  if (
    identity.id !== BENCHMARK_CORPUS_CATALOG_ID ||
    identity.role !== "family-baseline" ||
    identity.source !== "playground/src/generated/examples.ts" ||
    identity.mermaidBaseline.length === 0 ||
    !Number.isSafeInteger(identity.availableFamilies) ||
    identity.availableFamilies < 1
  ) {
    throw new Error("Benchmark corpus catalog identity is invalid.");
  }
}

function validateFixtureDescriptor(
  fixture: BenchmarkCorpusFixtureDescriptor
): void {
  if (
    fixture.id.length === 0 ||
    fixture.family.length === 0 ||
    fixture.fixture.length === 0 ||
    !Number.isSafeInteger(fixture.order) ||
    fixture.order < 0 ||
    !Number.isSafeInteger(fixture.source.bytes) ||
    fixture.source.bytes < 0 ||
    !/^[0-9a-f]{64}$/u.test(fixture.source.sha256)
  ) {
    throw new Error(`Benchmark corpus fixture ${fixture.id} is invalid.`);
  }
}

function validateFixtureRun(run: BenchmarkCorpusFixtureRun): void {
  validateSeed(run.coldSeed, "cold");
  validateSeed(run.warmSeed, "warm");
  if (
    run.id.length === 0 ||
    !Number.isSafeInteger(run.iterations) ||
    run.iterations < 1 ||
    !Number.isSafeInteger(run.warmups) ||
    run.warmups < 0 ||
    !Number.isFinite(run.durationMs) ||
    run.durationMs < 0 ||
    run.startedAt.length === 0 ||
    run.endedAt.length === 0
  ) {
    throw new Error("Benchmark corpus fixture run metadata is invalid.");
  }
}

function validateModeEvidence(
  evidence: BenchmarkCorpusModeEvidence,
  mode: BenchmarkSampleMode,
  seed: number
): void {
  if (evidence.mode !== mode || evidence.seed !== seed) {
    throw new Error(`Benchmark corpus ${mode} evidence identity is invalid.`);
  }
  if (
    evidence.status !== "success" &&
    evidence.status !== "failure" &&
    evidence.status !== "skipped"
  ) {
    throw new Error(`Benchmark corpus ${mode} evidence status is invalid.`);
  }
  if (
    evidence.status === "success" &&
    (evidence.report === null ||
      evidence.failure !== null ||
      evidence.skipReason !== null)
  ) {
    throw new Error(`Benchmark corpus ${mode} success evidence is invalid.`);
  }
  if (
    evidence.status === "failure" &&
    (evidence.failure === null || evidence.skipReason !== null)
  ) {
    throw new Error(`Benchmark corpus ${mode} failure evidence is invalid.`);
  }
  if (
    evidence.status === "skipped" &&
    (evidence.failure !== null || evidence.skipReason.length === 0)
  ) {
    throw new Error(`Benchmark corpus ${mode} skipped evidence is invalid.`);
  }
}

function validateFailure(
  failure: BenchmarkCorpusFailure,
  fixture: Pick<BenchmarkCorpusFixtureDescriptor, "family" | "id">,
  mode: BenchmarkSampleMode | null
): void {
  if (
    failure.fixtureId !== fixture.id ||
    failure.family !== fixture.family ||
    failure.mode !== mode ||
    failure.stage.length === 0 ||
    failure.message.length === 0
  ) {
    throw new Error(`Benchmark corpus failure for ${fixture.id} is invalid.`);
  }
}

function validateVersions(
  versions: Readonly<Record<BenchmarkEngine, string>>
): void {
  if (versions.merman.length === 0 || versions.mermaid.length === 0) {
    throw new Error("Benchmark corpus version identity is invalid.");
  }
}

function validateTerminalStatus(status: BenchmarkCorpusTerminalStatus): void {
  if (
    status !== "success" &&
    status !== "complete-with-errors" &&
    status !== "cancelled" &&
    status !== "invalidated"
  ) {
    throw new Error("Benchmark corpus terminal status is invalid.");
  }
}

function validateSeed(seed: number, label: string): void {
  if (!Number.isSafeInteger(seed) || seed < 0 || seed > 0xffff_ffff) {
    throw new Error(`Benchmark corpus ${label} seed is invalid.`);
  }
}

function freezeCatalogIdentity(
  identity: BenchmarkCorpusCatalogIdentity
): BenchmarkCorpusCatalogIdentity {
  validateCatalogIdentity(identity);
  return Object.freeze({ ...identity });
}

function freezeFixtureDescriptor(
  fixture: BenchmarkCorpusFixtureDescriptor
): BenchmarkCorpusFixtureDescriptor {
  validateFixtureDescriptor(fixture);
  return projectFixtureDescriptor(fixture);
}

function projectFixtureDescriptor(
  fixture: BenchmarkCorpusFixtureDescriptor
): BenchmarkCorpusFixtureDescriptor {
  return Object.freeze({
    id: fixture.id,
    family: fixture.family,
    fixture: fixture.fixture,
    order: fixture.order,
    source: Object.freeze({ ...fixture.source }),
  });
}

function freezeFixtureEvidence(
  evidence: BenchmarkCorpusFixtureEvidence
): BenchmarkCorpusFixtureEvidence {
  return Object.freeze({
    ...projectFixtureDescriptor(evidence),
    attempted: evidence.attempted,
    status: evidence.status,
    failure: evidence.failure === null ? null : freezeFailure(evidence.failure),
    cold: freezeModeEvidence(evidence.cold),
    warm: freezeModeEvidence(evidence.warm),
  });
}

function freezeFixtureRun(
  run: BenchmarkCorpusFixtureRun
): BenchmarkCorpusFixtureRun {
  validateFixtureRun(run);
  return Object.freeze({ ...run });
}

function freezeAggregateRun(
  run: Omit<BenchmarkCorpusAggregateRun, "order">
): Omit<BenchmarkCorpusAggregateRun, "order"> {
  if (
    run.id.length === 0 ||
    !Number.isSafeInteger(run.masterSeed) ||
    run.masterSeed < 0 ||
    run.masterSeed > 0xffff_ffff ||
    !Number.isSafeInteger(run.iterations) ||
    run.iterations < 1 ||
    !Number.isSafeInteger(run.warmups) ||
    run.warmups < 0 ||
    !Number.isFinite(run.durationMs) ||
    run.durationMs < 0 ||
    run.startedAt.length === 0 ||
    run.endedAt.length === 0
  ) {
    throw new Error("Benchmark corpus aggregate run metadata is invalid.");
  }
  return Object.freeze({ ...run });
}

function freezeVersions(
  versions: Readonly<Record<BenchmarkEngine, string>>
): Readonly<Record<BenchmarkEngine, string>> {
  validateVersions(versions);
  return Object.freeze({ ...versions });
}

function freezeFailure(
  failure: BenchmarkCorpusFailure
): BenchmarkCorpusFailure {
  return Object.freeze({ ...failure });
}

function freezeModeEvidence(
  evidence: BenchmarkCorpusModeEvidence
): BenchmarkCorpusModeEvidence {
  if (evidence.status === "success") {
    return Object.freeze({ ...evidence });
  }
  if (evidence.status === "failure") {
    return Object.freeze({
      ...evidence,
      failure: freezeFailure(evidence.failure),
    });
  }
  return Object.freeze({ ...evidence });
}

function skippedMode(
  mode: BenchmarkSampleMode,
  seed: number,
  reason: string
): BenchmarkCorpusModeSkipped {
  if (reason.length === 0) {
    throw new Error("Benchmark corpus skip reason cannot be empty.");
  }
  return Object.freeze({
    mode,
    seed,
    status: "skipped",
    report: null,
    failure: null,
    skipReason: reason,
  });
}

function sameCatalogIdentity(
  left: BenchmarkCorpusCatalogIdentity,
  right: BenchmarkCorpusCatalogIdentity
): boolean {
  return (
    left.id === right.id &&
    left.mermaidBaseline === right.mermaidBaseline &&
    left.role === right.role &&
    left.source === right.source &&
    left.availableFamilies === right.availableFamilies
  );
}

function sameFixtureDescriptor(
  left: BenchmarkCorpusFixtureDescriptor,
  right: BenchmarkCorpusFixtureDescriptor
): boolean {
  return (
    left.id === right.id &&
    left.family === right.family &&
    left.fixture === right.fixture &&
    left.order === right.order &&
    left.source.bytes === right.source.bytes &&
    left.source.sha256 === right.source.sha256
  );
}

function sameVersions(
  left: Readonly<Record<BenchmarkEngine, string>>,
  right: Readonly<Record<BenchmarkEngine, string>>
): boolean {
  return left.merman === right.merman && left.mermaid === right.mermaid;
}

function mergeTerminalStatus(
  current: BenchmarkCorpusTerminalStatus,
  next: BenchmarkCorpusTerminalStatus
): BenchmarkCorpusTerminalStatus {
  if (current === "invalidated" || next === "invalidated") {
    return "invalidated";
  }
  if (current === "cancelled" || next === "cancelled") return "cancelled";
  if (current === "complete-with-errors" || next === "complete-with-errors") {
    return "complete-with-errors";
  }
  return "success";
}
