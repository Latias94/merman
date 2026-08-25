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
import {
  benchmarkIntentMode,
  benchmarkIntentPurpose,
  benchmarkIntentRole,
  isBenchmarkAggregationIntent,
  type BenchmarkSampleIntent,
  type BenchmarkSampleIntentKind,
  type BenchmarkSamplePlan,
  type BenchmarkSamplePurpose,
} from "./sample-plan.ts";
import {
  calculateBenchmarkStatistics,
  calculateMedianRatio,
  type BenchmarkStatistics,
} from "./statistics.ts";
import {
  REALM_PROTOCOL_VERSION,
  type CompareRenderPayload,
} from "../runtime/realm/channel-protocol.ts";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "./report-schema.ts";
import { projectError } from "../runtime/error-projection.ts";

export { BENCHMARK_REPORT_SCHEMA_VERSION } from "./report-schema.ts";

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

interface BenchmarkRecordedSampleBase {
  readonly aggregateKey: string | null;
  readonly blockIndex: number | null;
  readonly engine: BenchmarkEngine;
  readonly failure: BenchmarkRecordedFailureDetail | null;
  readonly intervals: BenchmarkReportIntervals | null;
  readonly intentKind: BenchmarkSampleIntentKind;
  readonly mode: BenchmarkSampleMode;
  readonly orderIndex: 0 | 1;
  readonly parentPublication: BenchmarkParentPublicationEvidence | null;
  readonly purpose: BenchmarkSamplePurpose;
  readonly realmCreation: BenchmarkRealmCreationEvidence | null;
  readonly requestId: string;
  readonly resourceError: string | null;
  readonly resources: readonly BenchmarkResourceObservation[];
  readonly role: "measured" | "warmup";
  readonly runId: string;
  readonly sampleId: string;
  readonly sequence: number | null;
  readonly sessionId: string;
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
  readonly detail: string | null;
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
    readonly startedAt: string;
  };
  readonly plan: BenchmarkSamplePlan;
  readonly samples: readonly BenchmarkRecordedSample[];
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
  intent: BenchmarkSampleIntent,
  result: BenchmarkRealmSampleResult,
  realmCreation: BenchmarkRealmCreationEvidence | null = null
): BenchmarkRecordedSample {
  const mode = benchmarkIntentMode(intent);
  const common = {
    aggregateKey: isBenchmarkAggregationIntent(intent)
      ? intent.aggregateKey
      : null,
    blockIndex: isBenchmarkAggregationIntent(intent) ? intent.blockIndex : null,
    engine: intent.engine,
    intentKind: intent.kind,
    mode,
    orderIndex: intent.orderIndex,
    purpose: benchmarkIntentPurpose(intent),
    realmCreation,
    requestId: result.requestId,
    resourceError: result.resourceError,
    resources: result.resources,
    role: benchmarkIntentRole(intent),
    runId: result.runId,
    sampleId: intent.sampleId,
    sequence: result.sequence,
    sessionId: intent.sessionId,
  } as const;
  if (result.type === "benchmark-sample-success") {
    return Object.freeze({
      ...common,
      outcome: "success",
      failure: null,
      trace: result.trace,
      intervals: deriveReportIntervals(
        result.trace,
        intent.engine,
        mode,
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
        : deriveReportIntervals(result.trace, intent.engine, mode, null),
    parentPublication: null,
    version: result.version,
    failure: Object.freeze({
      detail: result.detail,
      kind: "realm",
      message: result.message,
      stage: result.stage,
    }),
    svgBytes: null,
  });
}

export function projectBenchmarkTransportFailure(
  intent: BenchmarkSampleIntent,
  input: Readonly<{
    requestId: string;
    runId: string;
  }>,
  error: unknown,
  stage = "transport",
  realmCreation: BenchmarkRealmCreationEvidence | null = null
): BenchmarkRecordedFailure {
  const projection = projectError(error);
  return Object.freeze({
    aggregateKey: isBenchmarkAggregationIntent(intent)
      ? intent.aggregateKey
      : null,
    blockIndex: isBenchmarkAggregationIntent(intent) ? intent.blockIndex : null,
    outcome: "failure",
    engine: intent.engine,
    intentKind: intent.kind,
    mode: benchmarkIntentMode(intent),
    orderIndex: intent.orderIndex,
    purpose: benchmarkIntentPurpose(intent),
    realmCreation,
    requestId: input.requestId,
    role: benchmarkIntentRole(intent),
    runId: input.runId,
    sampleId: intent.sampleId,
    sequence: null,
    sessionId: intent.sessionId,
    trace: null,
    intervals: null,
    parentPublication: null,
    resources: Object.freeze([]),
    resourceError: null,
    version: null,
    svgBytes: null,
    failure: Object.freeze({
      detail: projection.detail,
      kind: "transport",
      message: projection.summary,
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
  validateEvidenceSamples(evidence.plan, evidence.samples, terminalStatus);
  const report: BenchmarkReport = {
    environment: Object.freeze({ ...evidence.environment }),
    input: projectFrozenInput(evidence.input),
    protocols: Object.freeze({ ...evidence.protocols }),
    run: Object.freeze({ ...evidence.run }),
    plan: evidence.plan,
    samples: Object.freeze([...evidence.samples]),
    schemaVersion: evidence.schemaVersion,
    terminalError:
      evidence.terminalError === null
        ? null
        : Object.freeze({ ...evidence.terminalError }),
    transitions: Object.freeze(
      evidence.transitions.map((transition) => Object.freeze({ ...transition }))
    ),
    versions: Object.freeze({
      expected: Object.freeze({ ...evidence.versions.expected }),
      observed: Object.freeze({
        merman: Object.freeze([...evidence.versions.observed.merman]),
        mermaid: Object.freeze([...evidence.versions.observed.mermaid]),
      }),
    }),
    terminalStatus,
    aggregates: shouldAggregate(terminalStatus)
      ? buildAggregates(evidence.samples, evidence.plan)
      : null,
  };
  return Object.freeze(report);
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
  engine: BenchmarkEngine,
  mode: BenchmarkSampleMode,
  parentPublication: BenchmarkParentPublicationEvidence | null
): BenchmarkReportIntervals {
  const local = deriveBenchmarkIntervals(trace, { engine, mode });
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
  plan: BenchmarkSamplePlan
): BenchmarkAggregates {
  const engines = {
    merman: buildEngineStatistics(samples, "merman"),
    mermaid: buildEngineStatistics(samples, "mermaid"),
  } as const;
  const expectedIterations = plan.iterations;
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

function validateEvidenceSamples(
  plan: BenchmarkSamplePlan,
  samples: readonly BenchmarkRecordedSample[],
  terminalStatus: BenchmarkTerminalStatus
): void {
  if (samples.length > plan.samples.length) {
    throw new Error("Benchmark report contains more samples than its plan.");
  }
  for (const [index, sample] of samples.entries()) {
    const intent = plan.samples[index];
    if (!intent || intent.sampleId !== sample.sampleId) {
      throw new Error("Benchmark report samples do not follow plan order.");
    }
    const aggregateKey = isBenchmarkAggregationIntent(intent)
      ? intent.aggregateKey
      : null;
    const blockIndex = isBenchmarkAggregationIntent(intent)
      ? intent.blockIndex
      : null;
    if (
      sample.engine !== intent.engine ||
      sample.intentKind !== intent.kind ||
      sample.mode !== benchmarkIntentMode(intent) ||
      sample.role !== benchmarkIntentRole(intent) ||
      sample.purpose !== benchmarkIntentPurpose(intent) ||
      sample.orderIndex !== intent.orderIndex ||
      sample.sessionId !== intent.sessionId ||
      sample.aggregateKey !== aggregateKey ||
      sample.blockIndex !== blockIndex
    ) {
      throw new Error(
        `Benchmark report sample ${sample.sampleId} does not match its plan intent.`
      );
    }
  }
  if (terminalStatus === "success") {
    if (
      samples.length !== plan.samples.length ||
      samples.some((sample) => sample.outcome !== "success")
    ) {
      throw new Error(
        "A successful benchmark report must contain every planned successful sample."
      );
    }
  }
  if (
    terminalStatus === "complete-with-errors" &&
    !samples.some((sample) => sample.outcome === "failure")
  ) {
    throw new Error(
      "A complete-with-errors benchmark report must contain failure evidence."
    );
  }
}

function projectFrozenInput(input: BenchmarkFrozenInput): BenchmarkFrozenInput {
  return Object.freeze({
    source: input.source,
    configJson: input.configJson,
    theme: input.theme,
    diagramFont: input.diagramFont,
    externalRequirements: Object.freeze({
      externalDiagrams: Object.freeze([
        ...input.externalRequirements.externalDiagrams,
      ]),
      layoutModules: Object.freeze([...input.externalRequirements.layoutModules]),
    }),
    screenAvailableWidth: input.screenAvailableWidth,
    viewport: Object.freeze({ ...input.viewport }),
    detection: Object.freeze({ ...input.detection }),
  });
}
