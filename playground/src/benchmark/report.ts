import {
  BENCHMARK_PROTOCOL_VERSION,
  type BenchmarkResourceObservation,
} from "./protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  deriveBenchmarkIntervals,
  type BenchmarkDerivedIntervals,
  type BenchmarkEngine,
  type BenchmarkRawTrace,
  type BenchmarkSampleMode,
} from "./trace.ts";
import type {
  BenchmarkRealmCreationEvidence,
  BenchmarkRealmSampleResult,
} from "./realm/controller.ts";
import type { BenchmarkParentPublicationEvidence } from "./publication.ts";
import type { BalancedBenchmarkSchedule } from "./schedule.ts";
import {
  calculateBenchmarkStatistics,
  calculateMedianRatio,
  type BenchmarkStatistics,
} from "./statistics.ts";
import {
  REALM_PROTOCOL_VERSION,
  type CompareRenderPayload,
} from "../runtime/realm/channel-protocol.ts";

export const BENCHMARK_REPORT_SCHEMA_VERSION = 4 as const;

export interface BenchmarkReportIntervals extends BenchmarkDerivedIntervals {
  readonly firstPublishableSvgMs: number | null;
  readonly isolatedPresentationReceiptMs: number | null;
  readonly responseEnvelopeValidationMs: number | null;
  readonly responseDeliveryMs: number | null;
  readonly strictSvgValidationMs: number | null;
  readonly warmPublishableSvgMs: number | null;
}

export const BENCHMARK_INTERVAL_NAMES = Object.freeze([
  "adapterImportMs",
  "engineImportMs",
  "resourceAcquisitionMs",
  "registrationMs",
  "initializationMs",
  "firstBudgetedSvgMs",
  "firstIsolatedPresentationMs",
  "warmBudgetedSvgMs",
  "warmIsolatedPresentationMs",
  "isolatedPresentationReceiptMs",
  "responseDeliveryMs",
  "responseEnvelopeValidationMs",
  "strictSvgValidationMs",
  "firstPublishableSvgMs",
  "warmPublishableSvgMs",
] as const satisfies readonly (keyof BenchmarkReportIntervals)[]);

export type BenchmarkIntervalName = (typeof BENCHMARK_INTERVAL_NAMES)[number];
export type BenchmarkTerminalStatus =
  | "success"
  | "complete-with-errors"
  | "cancelled"
  | "invalidated"
  | "failed";
export type BenchmarkSamplePurpose = "setup" | "warmup" | "measured";

export interface BenchmarkDetectionSnapshot {
  readonly diagramType: string | null;
  readonly effectiveLayoutId: string | null;
  readonly status: "available" | "unavailable";
  readonly syntaxId: string | null;
  readonly validity: "valid" | "recoverable-invalid" | "unknown";
}

export interface BenchmarkFrozenInput extends CompareRenderPayload {
  readonly detection: BenchmarkDetectionSnapshot;
}

export interface BenchmarkEnvironment {
  readonly crossOriginIsolated: boolean;
  readonly devicePixelRatio: number;
  readonly hardwareConcurrency: number | null;
  readonly language: string;
  readonly platform: string;
  readonly userAgent: string;
}

export interface BenchmarkEnvironmentTransition {
  readonly atMs: number;
  readonly kind: "start" | "visibility-hidden" | "pagehide" | "freeze";
  readonly persisted?: boolean;
  readonly visibilityState: string;
}

export interface BenchmarkSampleMetadata {
  readonly blockIndex: number | null;
  readonly orderIndex: number;
  readonly purpose: BenchmarkSamplePurpose;
}

interface BenchmarkRecordedSampleBase extends BenchmarkSampleMetadata {
  readonly engine: BenchmarkEngine;
  readonly failure: BenchmarkRecordedFailureDetail | null;
  readonly intervals: BenchmarkReportIntervals | null;
  readonly mode: BenchmarkSampleMode;
  readonly parentPublication: BenchmarkParentPublicationEvidence | null;
  readonly realmCreation: BenchmarkRealmCreationEvidence | null;
  readonly requestId: string;
  readonly resourceError: string | null;
  readonly resources: readonly BenchmarkResourceObservation[];
  readonly role: "measured" | "warmup";
  readonly runId: string;
  readonly sequence: number | null;
  readonly svgBytes: number | null;
  readonly trace: BenchmarkRawTrace | null;
  readonly version: string | null;
}

export interface BenchmarkRecordedSuccess extends BenchmarkRecordedSampleBase {
  readonly failure: null;
  readonly intervals: BenchmarkReportIntervals;
  readonly outcome: "success";
  readonly sequence: number;
  readonly svgBytes: number;
  readonly trace: BenchmarkRawTrace;
  readonly version: string;
}

export interface BenchmarkRecordedFailure extends BenchmarkRecordedSampleBase {
  readonly failure: BenchmarkRecordedFailureDetail;
  readonly outcome: "failure";
}

export interface BenchmarkRecordedFailureDetail {
  readonly kind: "realm" | "transport";
  readonly message: string;
  readonly stage: string;
}

export type BenchmarkRecordedSample =
  | BenchmarkRecordedFailure
  | BenchmarkRecordedSuccess;

export interface BenchmarkRunEvidence {
  readonly environment: BenchmarkEnvironment;
  readonly input: BenchmarkFrozenInput;
  readonly protocols: {
    readonly benchmark: typeof BENCHMARK_PROTOCOL_VERSION;
    readonly realm: typeof REALM_PROTOCOL_VERSION;
    readonly trace: typeof BENCHMARK_TRACE_SCHEMA_VERSION;
  };
  readonly run: {
    readonly durationMs: number;
    readonly endedAt: string;
    readonly id: string;
    readonly iterations: number;
    readonly mode: BenchmarkSampleMode;
    readonly seed: number;
    readonly startedAt: string;
    readonly warmups: number;
  };
  readonly samples: readonly BenchmarkRecordedSample[];
  readonly schedule: BalancedBenchmarkSchedule;
  readonly schemaVersion: typeof BENCHMARK_REPORT_SCHEMA_VERSION;
  readonly terminalError: BenchmarkRecordedFailureDetail | null;
  readonly transitions: readonly BenchmarkEnvironmentTransition[];
  readonly versions: {
    readonly expected: Readonly<Record<BenchmarkEngine, string>>;
    readonly observed: Readonly<Record<BenchmarkEngine, readonly string[]>>;
  };
}

export type BenchmarkEngineStatistics = Readonly<
  Record<BenchmarkIntervalName, BenchmarkStatistics | null>
>;

export interface BenchmarkAggregates {
  readonly engines: Readonly<Record<BenchmarkEngine, BenchmarkEngineStatistics>>;
  readonly ratios: Readonly<Record<BenchmarkIntervalName, number | null>>;
}

export interface BenchmarkReport extends BenchmarkRunEvidence {
  readonly aggregates: BenchmarkAggregates | null;
  readonly terminalStatus: BenchmarkTerminalStatus;
}

export interface BenchmarkReportDownloadDependencies {
  createObjectUrl(blob: Blob): string;
  clickDownload(url: string, filename: string): void;
  revokeObjectUrl(url: string): void;
}

export function projectBenchmarkRealmSample(
  metadata: BenchmarkSampleMetadata,
  result: BenchmarkRealmSampleResult,
  realmCreation: BenchmarkRealmCreationEvidence | null = null
): BenchmarkRecordedSample {
  const common = {
    ...metadata,
    engine: result.engine,
    mode: result.mode,
    realmCreation,
    requestId: result.requestId,
    resourceError: result.resourceError,
    resources: result.resources,
    role: result.role,
    runId: result.runId,
    sequence: result.sequence,
  } as const;
  if (result.type === "benchmark-sample-success") {
    return Object.freeze({
      ...common,
      outcome: "success",
      failure: null,
      trace: result.trace,
      intervals: deriveReportIntervals(
        result.trace,
        result.mode,
        result.parentPublication
      ),
      parentPublication: result.parentPublication,
      version: result.version,
      svgBytes: result.svgBytes,
    });
  }
  return Object.freeze({
    ...common,
    outcome: "failure",
    trace: result.trace,
    intervals:
      result.trace === null
        ? null
        : deriveReportIntervals(result.trace, result.mode, null),
    parentPublication: null,
    version: result.version,
    failure: Object.freeze({
      kind: "realm",
      message: result.message,
      stage: result.stage,
    }),
    svgBytes: null,
  });
}

export function projectBenchmarkTransportFailure(
  metadata: BenchmarkSampleMetadata,
  input: Readonly<{
    engine: BenchmarkEngine;
    mode: BenchmarkSampleMode;
    requestId: string;
    role: "measured" | "warmup";
    runId: string;
  }>,
  error: unknown,
  stage = "transport",
  realmCreation: BenchmarkRealmCreationEvidence | null = null
): BenchmarkRecordedFailure {
  return Object.freeze({
    ...metadata,
    outcome: "failure",
    engine: input.engine,
    mode: input.mode,
    realmCreation,
    requestId: input.requestId,
    role: input.role,
    runId: input.runId,
    sequence: null,
    trace: null,
    intervals: null,
    parentPublication: null,
    resources: Object.freeze([]),
    resourceError: null,
    version: null,
    svgBytes: null,
    failure: Object.freeze({
      kind: "transport",
      message: boundedErrorMessage(error),
      stage,
    }),
  });
}

export function rejectBenchmarkRecordedSample(
  sample: BenchmarkRecordedSuccess,
  failure: BenchmarkRecordedFailureDetail
): BenchmarkRecordedFailure {
  return Object.freeze({
    ...sample,
    outcome: "failure",
    failure: Object.freeze({ ...failure }),
  });
}

export function buildBenchmarkReport(
  evidence: BenchmarkRunEvidence,
  terminalStatus: BenchmarkTerminalStatus
): BenchmarkReport {
  const report: BenchmarkReport = {
    ...evidence,
    terminalStatus,
    aggregates: shouldAggregate(terminalStatus)
      ? buildAggregates(evidence.samples, evidence.run.iterations)
      : null,
  };
  return deepFreeze(report);
}

export function serializeBenchmarkReport(report: BenchmarkReport): string {
  return `${JSON.stringify(report, null, 2)}\n`;
}

export function downloadBenchmarkReport(
  report: BenchmarkReport,
  dependencies: BenchmarkReportDownloadDependencies = browserDownloadDependencies()
): void {
  const blob = new Blob([serializeBenchmarkReport(report)], {
    type: "application/json;charset=utf-8",
  });
  const url = dependencies.createObjectUrl(blob);
  try {
    dependencies.clickDownload(
      url,
      `merman-benchmark-${report.run.id}.json`
    );
  } finally {
    dependencies.revokeObjectUrl(url);
  }
}

function shouldAggregate(status: BenchmarkTerminalStatus): boolean {
  return status === "success" || status === "complete-with-errors";
}

function deriveReportIntervals(
  trace: BenchmarkRawTrace,
  mode: BenchmarkSampleMode,
  parentPublication: BenchmarkParentPublicationEvidence | null
): BenchmarkReportIntervals {
  const local = deriveBenchmarkIntervals(trace, { mode });
  return Object.freeze({
    ...local,
    isolatedPresentationReceiptMs:
      parentPublication?.isolatedPresentationReceiptMs ?? null,
    responseDeliveryMs: parentPublication?.responseDeliveryMs ?? null,
    responseEnvelopeValidationMs:
      parentPublication?.responseEnvelopeValidationMs ?? null,
    strictSvgValidationMs: parentPublication?.strictSvgValidationMs ?? null,
    firstPublishableSvgMs:
      mode === "realm-cold" ? (parentPublication?.totalMs ?? null) : null,
    warmPublishableSvgMs:
      mode === "warm" ? (parentPublication?.totalMs ?? null) : null,
  });
}

function buildAggregates(
  samples: readonly BenchmarkRecordedSample[],
  expectedIterations: number
): BenchmarkAggregates {
  const engines = {
    merman: buildEngineStatistics(samples, "merman"),
    mermaid: buildEngineStatistics(samples, "mermaid"),
  } as const;
  const measuredSuccessCounts = {
    merman: countMeasuredSuccesses(samples, "merman"),
    mermaid: countMeasuredSuccesses(samples, "mermaid"),
  };
  const ratios = emptyIntervalRecord<number>();
  const hasFailure = samples.some((sample) => sample.outcome === "failure");

  for (const name of BENCHMARK_INTERVAL_NAMES) {
    const left = engines.merman[name];
    const right = engines.mermaid[name];
    ratios[name] =
      !hasFailure &&
      measuredSuccessCounts.merman === expectedIterations &&
      measuredSuccessCounts.mermaid === expectedIterations &&
      left?.count === expectedIterations &&
      right?.count === expectedIterations &&
      hasCorrespondingSamples(samples, name, expectedIterations)
        ? calculateMedianRatio(left, right)
        : null;
  }

  return Object.freeze({
    engines: Object.freeze(engines),
    ratios: Object.freeze(ratios),
  });
}

function hasCorrespondingSamples(
  samples: readonly BenchmarkRecordedSample[],
  interval: BenchmarkIntervalName,
  expectedIterations: number
): boolean {
  const keys = (engine: BenchmarkEngine) =>
    samples
      .filter(
        (sample): sample is BenchmarkRecordedSuccess =>
          sample.outcome === "success" &&
          sample.purpose === "measured" &&
          sample.engine === engine &&
          sample.blockIndex !== null &&
          sample.intervals[interval] !== null
      )
      .map((sample) => `${sample.mode}:${sample.blockIndex}`)
      .sort();
  const merman = keys("merman");
  const mermaid = keys("mermaid");
  return (
    merman.length === expectedIterations &&
    mermaid.length === expectedIterations &&
    new Set(merman).size === expectedIterations &&
    new Set(mermaid).size === expectedIterations &&
    merman.every((key, index) => key === mermaid[index])
  );
}

function buildEngineStatistics(
  samples: readonly BenchmarkRecordedSample[],
  engine: BenchmarkEngine
): BenchmarkEngineStatistics {
  const result = emptyIntervalRecord<BenchmarkStatistics>();
  for (const name of BENCHMARK_INTERVAL_NAMES) {
    const values = samples
      .filter(
        (sample): sample is BenchmarkRecordedSuccess =>
          sample.outcome === "success" &&
          sample.purpose === "measured" &&
          sample.engine === engine
      )
      .map((sample) => sample.intervals[name])
      .filter((value): value is number => value !== null);
    result[name] =
      values.length === 0 ? null : calculateBenchmarkStatistics(values);
  }
  return Object.freeze(result);
}

function countMeasuredSuccesses(
  samples: readonly BenchmarkRecordedSample[],
  engine: BenchmarkEngine
): number {
  return samples.filter(
    (sample) =>
      sample.outcome === "success" &&
      sample.purpose === "measured" &&
      sample.engine === engine
  ).length;
}

function emptyIntervalRecord<T>(): Record<BenchmarkIntervalName, T | null> {
  return Object.fromEntries(
    BENCHMARK_INTERVAL_NAMES.map((name) => [name, null])
  ) as Record<BenchmarkIntervalName, T | null>;
}

function boundedErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.slice(0, 8_192);
}

function browserDownloadDependencies(): BenchmarkReportDownloadDependencies {
  return {
    createObjectUrl: (blob) => URL.createObjectURL(blob),
    revokeObjectUrl: (url) => URL.revokeObjectURL(url),
    clickDownload(url, filename) {
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      anchor.hidden = true;
      document.body.appendChild(anchor);
      try {
        anchor.click();
      } finally {
        anchor.remove();
      }
    },
  };
}

function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const nested of Object.values(value as Record<string, unknown>)) {
      deepFreeze(nested);
    }
  }
  return value;
}
