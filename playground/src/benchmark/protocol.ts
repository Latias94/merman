import {
  BENCHMARK_BUDGETS,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertEncodedMessageBudget,
  utf8ByteLength,
  validateCompareRenderPayload,
  type CompareRenderPayload,
  type RealmIdentity,
} from "../runtime/realm/channel-protocol.ts";
import {
  BENCHMARK_TRACE_EVENT_NAMES,
  BENCHMARK_TRACE_SCHEMA_VERSION,
  validateBenchmarkRawTrace,
  type BenchmarkEngine,
  type BenchmarkRawTrace,
  type BenchmarkSampleMode,
  type BenchmarkTraceMark,
} from "./trace.ts";

export const BENCHMARK_PROTOCOL_VERSION = 1 as const;

export type BenchmarkSampleRole = "measured" | "warmup";

export type BenchmarkFailureStage =
  | "environment"
  | "fonts"
  | "adapter-import"
  | "engine-import"
  | "resource-acquire"
  | "register"
  | "initialize"
  | "render"
  | "svg-validation"
  | "presentation"
  | "protocol"
  | "timeout"
  | "disposed";

export interface BenchmarkSampleRequest extends RealmIdentity {
  readonly benchmarkProtocol: typeof BENCHMARK_PROTOCOL_VERSION;
  readonly engine: BenchmarkEngine;
  readonly mode: BenchmarkSampleMode;
  readonly payload: CompareRenderPayload;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly role: BenchmarkSampleRole;
  readonly runId: string;
  readonly runToken: string;
  readonly sequence: number;
  readonly type: "benchmark-sample";
}

export interface BenchmarkSampleProgress extends RealmIdentity {
  readonly benchmarkProtocol: typeof BENCHMARK_PROTOCOL_VERSION;
  readonly engine: BenchmarkEngine;
  readonly event: BenchmarkTraceMark;
  readonly mode: BenchmarkSampleMode;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly role: BenchmarkSampleRole;
  readonly runId: string;
  readonly runToken: string;
  readonly sequence: number;
  readonly traceSchema: typeof BENCHMARK_TRACE_SCHEMA_VERSION;
  readonly type: "benchmark-progress";
}

export interface BenchmarkResourceObservation {
  readonly decodedBodySize: number | null;
  readonly deliveryType: string | null;
  readonly duration: number;
  readonly encodedBodySize: number | null;
  readonly initiatorType: string;
  readonly name: string;
  readonly responseStatus: number | null;
  readonly startOffset: number;
  readonly transferSize: number | null;
}

interface BenchmarkSampleResponseBase extends RealmIdentity {
  readonly benchmarkProtocol: typeof BENCHMARK_PROTOCOL_VERSION;
  readonly engine: BenchmarkEngine;
  readonly mode: BenchmarkSampleMode;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly resourceError: string | null;
  readonly resources: readonly BenchmarkResourceObservation[];
  readonly role: BenchmarkSampleRole;
  readonly runId: string;
  readonly runToken: string;
  readonly sequence: number;
  readonly traceSchema: typeof BENCHMARK_TRACE_SCHEMA_VERSION;
  readonly version: string | null;
}

export interface BenchmarkSampleSuccess extends BenchmarkSampleResponseBase {
  readonly svg: string;
  readonly trace: BenchmarkRawTrace;
  readonly type: "benchmark-sample-success";
  readonly version: string;
}

export interface BenchmarkSampleFailure extends BenchmarkSampleResponseBase {
  readonly message: string;
  readonly stage: BenchmarkFailureStage;
  readonly trace: BenchmarkRawTrace | null;
  readonly type: "benchmark-sample-failure";
}

export type BenchmarkSampleResponse =
  | BenchmarkSampleFailure
  | BenchmarkSampleSuccess;

export type BenchmarkExpectedSample = Pick<
  BenchmarkSampleRequest,
  "engine" | "mode" | "requestId" | "role" | "runId" | "runToken"
>;

const ENGINES = new Set<BenchmarkEngine>(["merman", "mermaid"]);
const MODES = new Set<BenchmarkSampleMode>(["realm-cold", "warm"]);
const ROLES = new Set<BenchmarkSampleRole>(["measured", "warmup"]);
const TRACE_MARKS = new Set<BenchmarkTraceMark>(
  BENCHMARK_TRACE_EVENT_NAMES.filter(
    (event): event is BenchmarkTraceMark =>
      event !== "sample_start" && event !== "sample_end"
  )
);
const FAILURE_STAGES = new Set<BenchmarkFailureStage>([
  "environment",
  "fonts",
  "adapter-import",
  "engine-import",
  "resource-acquire",
  "register",
  "initialize",
  "render",
  "svg-validation",
  "presentation",
  "protocol",
  "timeout",
  "disposed",
]);
const PRE_CLOCK_FAILURE_STAGES = new Set<BenchmarkFailureStage>([
  "environment",
]);

export function validateBenchmarkSampleRequest(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number
): BenchmarkSampleRequest {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "benchmark sample request");
  assertExactKeys(message, [
    "type",
    "protocol",
    "benchmarkProtocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "runId",
    "runToken",
    "requestId",
    "engine",
    "mode",
    "role",
    "payload",
  ]);
  assertEnvelope(message, identity, expectedSequence, "benchmark-sample");
  assertBenchmarkProtocol(message.benchmarkProtocol);

  return {
    type: "benchmark-sample",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    runId: expectBoundedString(message.runId, "runId", 128),
    runToken: expectSecureToken(message.runToken, "runToken"),
    requestId: expectBoundedString(message.requestId, "requestId", 128),
    engine: expectSetValue(message.engine, ENGINES, "engine"),
    mode: expectSetValue(message.mode, MODES, "mode"),
    role: expectSetValue(message.role, ROLES, "role"),
    payload: validateCompareRenderPayload(message.payload),
  };
}

export function validateBenchmarkSampleProgress(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number,
  expected: BenchmarkExpectedSample
): BenchmarkSampleProgress {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "benchmark sample progress");
  assertExactKeys(message, [
    "type",
    "protocol",
    "benchmarkProtocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "runId",
    "runToken",
    "requestId",
    "engine",
    "mode",
    "role",
    "traceSchema",
    "event",
  ]);
  assertEnvelope(message, identity, expectedSequence, "benchmark-progress");
  assertBenchmarkProtocol(message.benchmarkProtocol);
  assertExpectedSample(message, expected);
  if (message.traceSchema !== BENCHMARK_TRACE_SCHEMA_VERSION) {
    throw new RealmProtocolError("Benchmark trace schema version is invalid.");
  }

  return Object.freeze({
    type: "benchmark-progress",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    ...sampleIdentity(expected),
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    event: expectSetValue(message.event, TRACE_MARKS, "progress event"),
  });
}

export function validateBenchmarkSampleResponse(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number,
  expected: BenchmarkExpectedSample
): BenchmarkSampleResponse {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "benchmark sample response");
  const type = message.type;
  const commonKeys = [
    "type",
    "protocol",
    "benchmarkProtocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "runId",
    "runToken",
    "requestId",
    "engine",
    "mode",
    "role",
    "traceSchema",
    "trace",
    "resources",
    "resourceError",
    "version",
  ];

  if (type === "benchmark-sample-success") {
    assertExactKeys(message, [...commonKeys, "svg"]);
  } else if (type === "benchmark-sample-failure") {
    assertExactKeys(message, [...commonKeys, "stage", "message"]);
  } else {
    throw new RealmProtocolError("Benchmark response type is invalid.");
  }

  assertEnvelope(message, identity, expectedSequence, type);
  assertBenchmarkProtocol(message.benchmarkProtocol);
  assertExpectedSample(message, expected);
  if (message.traceSchema !== BENCHMARK_TRACE_SCHEMA_VERSION) {
    throw new RealmProtocolError("Benchmark trace schema version is invalid.");
  }
  const resourceValue = message.resources;
  const resourceError = expectNullableBoundedString(
    message.resourceError,
    "resourceError",
    REALM_BUDGETS.errorBytes
  );
  const version = expectNullableBoundedString(message.version, "version", 256);

  if (type === "benchmark-sample-success") {
    const svg = expectString(message.svg, "svg");
    assertByteBudget(svg, REALM_BUDGETS.svgBytes, "svg");
    const trace = validateBenchmarkRawTrace(message.trace, {
      engine: expected.engine,
      mode: expected.mode,
      outcome: "success",
    });
    assertTraceTimeBudget(trace, false);
    const resources = validateResourceObservations(
      resourceValue,
      trace.sample_end
    );
    if (version === null) {
      throw new RealmProtocolError("Successful benchmark response has no version.");
    }
    if (resourceError !== null && resources.length > 0) {
      throw new RealmProtocolError(
        "Benchmark resource observation error conflicts with retained evidence."
      );
    }
    return Object.freeze({
      type,
      protocol: REALM_PROTOCOL_VERSION,
      benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
      ...identity,
      sequence: expectedSequence,
      ...sampleIdentity(expected),
      traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
      trace,
      resources,
      resourceError,
      svg,
      version,
    });
  }

  if (
    typeof message.stage !== "string" ||
    !FAILURE_STAGES.has(message.stage as BenchmarkFailureStage)
  ) {
    throw new RealmProtocolError("Benchmark failure stage is invalid.");
  }
  const failureMessage = expectString(message.message, "message");
  assertByteBudget(failureMessage, REALM_BUDGETS.errorBytes, "message");
  const failureStage = message.stage as BenchmarkFailureStage;
  const trace =
    message.trace === null
      ? null
      : validateBenchmarkRawTrace(message.trace, {
          engine: expected.engine,
          mode: expected.mode,
          outcome: "failure",
        });
  if (trace === null && !PRE_CLOCK_FAILURE_STAGES.has(failureStage)) {
    throw new RealmProtocolError(
      "Benchmark post-clock failure must retain its raw trace."
    );
  }
  if (trace !== null) {
    assertFailureStageMatchesTrace(failureStage, trace, expected);
    assertTraceTimeBudget(trace, failureStage === "timeout");
  }
  const resources = validateResourceObservations(
    resourceValue,
    trace?.sample_end ?? null
  );
  if (
    (trace === null && resourceError !== null) ||
    (resourceError !== null && resources.length > 0)
  ) {
    throw new RealmProtocolError(
      "Benchmark resource observation error conflicts with retained evidence."
    );
  }
  return Object.freeze({
    type,
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    ...sampleIdentity(expected),
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources,
    resourceError,
    stage: failureStage,
    message: failureMessage,
    version,
  });
}

function sampleIdentity(
  expected: BenchmarkExpectedSample
): BenchmarkExpectedSample {
  return {
    engine: expected.engine,
    mode: expected.mode,
    requestId: expected.requestId,
    role: expected.role,
    runId: expected.runId,
    runToken: expected.runToken,
  };
}

function assertFailureStageMatchesTrace(
  stage: BenchmarkFailureStage,
  trace: BenchmarkRawTrace,
  expected: BenchmarkExpectedSample
): void {
  const coldOnlyStages = new Set<BenchmarkFailureStage>([
    "adapter-import",
    "engine-import",
    "resource-acquire",
    "register",
    "initialize",
  ]);
  if (expected.mode === "warm" && coldOnlyStages.has(stage)) {
    throw new RealmProtocolError(
      "Warm benchmark failure declared a cold-only stage."
    );
  }
  if (
    (expected.engine === "merman" && stage === "register") ||
    (expected.engine === "mermaid" && stage === "resource-acquire")
  ) {
    throw new RealmProtocolError(
      "Benchmark failure stage does not apply to its engine."
    );
  }

  const requirePair = (start: keyof BenchmarkRawTrace, label: string) => {
    if (trace[start] === null) {
      throw new RealmProtocolError(
        `Benchmark ${stage} failure has no ${label} evidence.`
      );
    }
  };
  const forbidAfter = (...events: (keyof BenchmarkRawTrace)[]) => {
    if (events.some((event) => trace[event] !== null)) {
      throw new RealmProtocolError(
        `Benchmark ${stage} failure contains later phase evidence.`
      );
    }
  };

  switch (stage) {
    case "environment":
      throw new RealmProtocolError(
        "Post-clock environment failure must not use a sample response."
      );
    case "fonts":
      requirePair("fonts_wait_start", "font wait");
      forbidAfter(
        "engine_import_start",
        "resource_acquire_start",
        "register_start",
        "initialize_start",
        "render_start"
      );
      return;
    case "adapter-import":
      requirePair("adapter_import_start", "adapter import");
      forbidAfter(
        "engine_import_start",
        "resource_acquire_start",
        "register_start",
        "initialize_start",
        "render_start"
      );
      return;
    case "engine-import":
      requirePair("engine_import_start", "engine import");
      forbidAfter("register_start", "initialize_start", "render_start");
      return;
    case "resource-acquire":
      requirePair("resource_acquire_start", "resource acquisition");
      forbidAfter("initialize_start", "render_start");
      return;
    case "register":
      requirePair("register_start", "registration");
      forbidAfter("initialize_start", "render_start");
      return;
    case "initialize":
      requirePair("initialize_start", "initialization");
      forbidAfter("render_start");
      return;
    case "render":
    case "svg-validation":
      if (trace.render_start === null || trace.safe_svg_ready !== null) {
        throw new RealmProtocolError(
          `Benchmark ${stage} failure has an invalid render prefix.`
        );
      }
      return;
    case "presentation":
      if (
        trace.safe_svg_ready === null ||
        trace.presentation_ready !== null
      ) {
        throw new RealmProtocolError(
          "Benchmark presentation failure has an invalid presentation prefix."
        );
      }
      return;
    case "timeout":
    case "protocol":
    case "disposed":
      return;
  }
}

function assertTraceTimeBudget(
  trace: BenchmarkRawTrace,
  allowStageTimeout: boolean
): void {
  if (trace.sample_end > REALM_BUDGETS.runTimeoutMs) {
    throw new RealmProtocolError("Benchmark trace exceeds the run time budget.");
  }
  if (allowStageTimeout) return;
  const spans: readonly (readonly [number | null, number | null])[] = [
    [trace.fonts_wait_start, trace.fonts_wait_end],
    [trace.adapter_import_start, trace.adapter_import_end],
    [trace.engine_import_start, trace.engine_import_end],
    [trace.resource_acquire_start, trace.resource_acquire_end],
    [trace.register_start, trace.register_end],
    [trace.initialize_start, trace.initialize_end],
    [trace.render_start, trace.safe_svg_ready],
    [trace.safe_svg_ready, trace.presentation_ready],
  ];
  if (
    spans.some(
      ([start, end]) =>
        start !== null &&
        end !== null &&
        end - start > REALM_BUDGETS.stageTimeoutMs
    )
  ) {
    throw new RealmProtocolError("Benchmark trace exceeds a stage time budget.");
  }

  const activeSpans: readonly (readonly [number | null, number | null])[] = [
    [trace.render_start, trace.safe_svg_ready],
    [trace.safe_svg_ready, trace.presentation_ready],
  ];
  if (
    activeSpans.some(
      ([start, end]) =>
        start !== null &&
        end === null &&
        trace.sample_end - start > REALM_BUDGETS.stageTimeoutMs
    )
  ) {
    throw new RealmProtocolError("Benchmark trace exceeds a stage time budget.");
  }
}

function isAtOrBefore(left: number, right: number): boolean {
  const tolerance =
    Number.EPSILON * Math.max(1, Math.abs(left), Math.abs(right)) * 8;
  return left <= right + tolerance;
}

function validateResourceObservations(
  value: unknown,
  sampleEnd: number | null
): readonly BenchmarkResourceObservation[] {
  if (
    !Array.isArray(value) ||
    value.length > BENCHMARK_BUDGETS.maxResourceObservations ||
    (sampleEnd === null && value.length > 0)
  ) {
    throw new RealmProtocolError("Benchmark resource observations are invalid.");
  }
  return Object.freeze(
    value.map((candidate) => {
      const observation = expectRecord(candidate, "resource observation");
      assertExactKeys(observation, [
        "name",
        "initiatorType",
        "startOffset",
        "duration",
        "transferSize",
        "encodedBodySize",
        "decodedBodySize",
        "responseStatus",
        "deliveryType",
      ]);
      const startOffset = expectDuration(
        observation.startOffset,
        "resource startOffset"
      );
      const duration = expectDuration(
        observation.duration,
        "resource duration"
      );
      if (
        sampleEnd === null ||
        !isAtOrBefore(startOffset + duration, sampleEnd)
      ) {
        throw new RealmProtocolError(
          "Benchmark resource observation exceeds sample_end."
        );
      }
      return Object.freeze({
        name: expectBoundedString(observation.name, "resource name", 4_096),
        initiatorType: expectBoundedString(
          observation.initiatorType,
          "resource initiatorType",
          128
        ),
        startOffset,
        duration,
        transferSize: expectNullableDuration(
          observation.transferSize,
          "resource transferSize"
        ),
        encodedBodySize: expectNullableDuration(
          observation.encodedBodySize,
          "resource encodedBodySize"
        ),
        decodedBodySize: expectNullableDuration(
          observation.decodedBodySize,
          "resource decodedBodySize"
        ),
        responseStatus: expectNullableDuration(
          observation.responseStatus,
          "resource responseStatus"
        ),
        deliveryType: expectNullableBoundedString(
          observation.deliveryType,
          "resource deliveryType",
          128
        ),
      });
    })
  );
}

function assertExpectedSample(
  message: Record<string, unknown>,
  expected: BenchmarkExpectedSample
): void {
  for (const key of [
    "runId",
    "runToken",
    "requestId",
    "engine",
    "mode",
    "role",
  ] as const) {
    if (message[key] !== expected[key]) {
      throw new RealmProtocolError(`Benchmark ${key} is invalid.`);
    }
  }
}

function assertBenchmarkProtocol(value: unknown): void {
  if (value !== BENCHMARK_PROTOCOL_VERSION) {
    throw new RealmProtocolError("Benchmark protocol version is invalid.");
  }
}

function assertEnvelope(
  message: Record<string, unknown>,
  identity: RealmIdentity,
  expectedSequence: number,
  expectedType: string
): void {
  if (
    message.type !== expectedType ||
    message.protocol !== REALM_PROTOCOL_VERSION ||
    message.kind !== identity.kind ||
    message.realmId !== identity.realmId ||
    message.realmToken !== identity.realmToken ||
    message.sequence !== expectedSequence
  ) {
    throw new RealmProtocolError("Benchmark message envelope is invalid.");
  }
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new RealmProtocolError(`Benchmark ${label} must be an object.`);
  }
  return value as Record<string, unknown>;
}

function assertExactKeys(
  value: Record<string, unknown>,
  expectedKeys: readonly string[]
): void {
  const actual = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    throw new RealmProtocolError("Benchmark message contains unexpected fields.");
  }
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new RealmProtocolError(`Benchmark ${label} must be a string.`);
  }
  return value;
}

function expectBoundedString(
  value: unknown,
  label: string,
  maxBytes: number
): string {
  const text = expectString(value, label);
  if (text.length === 0) {
    throw new RealmProtocolError(`Benchmark ${label} is invalid.`);
  }
  assertByteBudget(text, maxBytes, label);
  return text;
}

function expectNullableBoundedString(
  value: unknown,
  label: string,
  maxBytes: number
): string | null {
  return value === null ? null : expectBoundedString(value, label, maxBytes);
}

function expectSecureToken(value: unknown, label: string): string {
  const token = expectString(value, label);
  if (!/^[A-Za-z0-9_-]{43}$/.test(token)) {
    throw new RealmProtocolError(`Benchmark ${label} is invalid.`);
  }
  return token;
}

function expectSetValue<T extends string>(
  value: unknown,
  allowed: ReadonlySet<T>,
  label: string
): T {
  if (typeof value !== "string" || !allowed.has(value as T)) {
    throw new RealmProtocolError(`Benchmark ${label} is invalid.`);
  }
  return value as T;
}

function expectDuration(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new RealmProtocolError(`Benchmark ${label} must be finite and non-negative.`);
  }
  return value;
}

function expectNullableDuration(value: unknown, label: string): number | null {
  return value === null ? null : expectDuration(value, label);
}

function assertByteBudget(value: string, limit: number, label: string): void {
  if (utf8ByteLength(value) > limit) {
    throw new RealmProtocolError(`Benchmark ${label} exceeds its byte budget.`);
  }
}
