import {
  BENCHMARK_BUDGETS,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertEncodedMessageBudget,
  utf8ByteLength,
  validateCompareRenderPayload,
  type CompareRenderPayload,
  type RealmIdentity
} from "../runtime/realm/channel-protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  validateBenchmarkRawTrace,
  type BenchmarkEngine,
  type BenchmarkRawTrace,
  type BenchmarkTraceMark
} from "./trace.ts";
import {
  benchmarkPhasePath,
  isBenchmarkFailureStage,
  type BenchmarkFailureStage,
  type FrozenBenchmarkPhasePath
} from "./phase-contract.ts";
import {
  benchmarkIntentModeFromKind,
  type BenchmarkSampleIntentKind
} from "./sample-plan.ts";

export const BENCHMARK_PROTOCOL_VERSION = 4 as const;

export type { BenchmarkFailureStage } from "./phase-contract.ts";

interface BenchmarkSampleRequestBase extends RealmIdentity {
  readonly benchmarkProtocol: typeof BENCHMARK_PROTOCOL_VERSION;
  readonly engine: BenchmarkEngine;
  readonly inputId: string;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly runId: string;
  readonly runToken: string;
  readonly sampleId: string;
  readonly sequence: number;
  readonly type: "benchmark-sample";
}

export interface BenchmarkInputSampleRequest
  extends BenchmarkSampleRequestBase {
  readonly intentKind: "cold-measured" | "warm-setup";
  readonly payload: CompareRenderPayload;
}

export interface BenchmarkReuseSampleRequest
  extends BenchmarkSampleRequestBase {
  readonly intentKind: "warm-measured" | "warmup";
}

export type BenchmarkSampleRequest =
  | BenchmarkInputSampleRequest
  | BenchmarkReuseSampleRequest;

export interface BenchmarkSampleProgress extends RealmIdentity {
  readonly benchmarkProtocol: typeof BENCHMARK_PROTOCOL_VERSION;
  readonly engine: BenchmarkEngine;
  readonly event: BenchmarkTraceMark;
  readonly intentKind: BenchmarkSampleIntentKind;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly runId: string;
  readonly runToken: string;
  readonly sampleId: string;
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
  readonly intentKind: BenchmarkSampleIntentKind;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly resourceError: string | null;
  readonly resources: readonly BenchmarkResourceObservation[];
  readonly runId: string;
  readonly runToken: string;
  readonly sampleId: string;
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
  readonly detail: string | null;
  readonly message: string;
  readonly stage: BenchmarkFailureStage;
  readonly trace: BenchmarkRawTrace | null;
  readonly type: "benchmark-sample-failure";
}

export type BenchmarkSampleResponse =
  BenchmarkSampleFailure | BenchmarkSampleSuccess;

export type BenchmarkExpectedSample = Pick<
  BenchmarkSampleRequest,
  "engine" | "intentKind" | "requestId" | "runId" | "runToken" | "sampleId"
>;

const ENGINES = new Set<BenchmarkEngine>(["merman", "mermaid"]);
const INTENT_KINDS = new Set<BenchmarkSampleIntentKind>([
  "cold-measured",
  "warm-setup",
  "warmup",
  "warm-measured"
]);
const INPUT_INTENT_KINDS = new Set<BenchmarkSampleIntentKind>([
  "cold-measured",
  "warm-setup"
]);

const REQUEST_COMMON_KEYS = [
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
  "sampleId",
  "engine",
  "intentKind",
  "inputId"
] as const;
const REQUEST_INPUT_SCHEMA = exactKeySchema([
  ...REQUEST_COMMON_KEYS,
  "payload"
]);
const REQUEST_REUSE_SCHEMA = exactKeySchema(REQUEST_COMMON_KEYS);
const PROGRESS_SCHEMA = exactKeySchema([
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
  "sampleId",
  "engine",
  "intentKind",
  "traceSchema",
  "event"
]);
const RESPONSE_COMMON_KEYS = [
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
  "sampleId",
  "engine",
  "intentKind",
  "traceSchema",
  "trace",
  "resources",
  "resourceError",
  "version"
] as const;
const SUCCESS_RESPONSE_SCHEMA = exactKeySchema([
  ...RESPONSE_COMMON_KEYS,
  "svg"
]);
const FAILURE_RESPONSE_SCHEMA = exactKeySchema([
  ...RESPONSE_COMMON_KEYS,
  "stage",
  "message",
  "detail"
]);
const RESOURCE_OBSERVATION_SCHEMA = exactKeySchema([
  "name",
  "initiatorType",
  "startOffset",
  "duration",
  "transferSize",
  "encodedBodySize",
  "decodedBodySize",
  "responseStatus",
  "deliveryType"
]);

export function validateBenchmarkSampleRequest(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number
): BenchmarkSampleRequest {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "benchmark sample request");
  const intentKind = expectSetValue(
    message.intentKind,
    INTENT_KINDS,
    "intentKind"
  );
  const carriesInput = INPUT_INTENT_KINDS.has(intentKind);
  assertExactKeys(
    message,
    carriesInput ? REQUEST_INPUT_SCHEMA : REQUEST_REUSE_SCHEMA
  );
  assertEnvelope(message, identity, expectedSequence, "benchmark-sample");
  assertBenchmarkProtocol(message.benchmarkProtocol);

  const request = {
    type: "benchmark-sample",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    runId: expectBoundedString(message.runId, "runId", 128),
    runToken: expectSecureToken(message.runToken, "runToken"),
    requestId: expectBoundedString(message.requestId, "requestId", 128),
    sampleId: expectBoundedString(message.sampleId, "sampleId", 128),
    engine: expectSetValue(message.engine, ENGINES, "engine"),
    intentKind,
    inputId: expectBoundedString(message.inputId, "inputId", 128)
  } as const;
  return carriesInput
    ? Object.freeze({
        ...request,
        intentKind: intentKind as BenchmarkInputSampleRequest["intentKind"],
        payload: validateCompareRenderPayload(message.payload)
      })
    : Object.freeze({
        ...request,
        intentKind: intentKind as BenchmarkReuseSampleRequest["intentKind"]
      });
}

export function validateBenchmarkSampleProgress(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number,
  expected: BenchmarkExpectedSample
): BenchmarkSampleProgress {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "benchmark sample progress");
  assertExactKeys(message, PROGRESS_SCHEMA);
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
    event: expectProgressEvent(message.event, expected)
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
  if (type === "benchmark-sample-success") {
    assertExactKeys(message, SUCCESS_RESPONSE_SCHEMA);
  } else if (type === "benchmark-sample-failure") {
    assertExactKeys(message, FAILURE_RESPONSE_SCHEMA);
  } else {
    throw new RealmProtocolError("Benchmark response type is invalid.");
  }

  assertEnvelope(message, identity, expectedSequence, type);
  assertBenchmarkProtocol(message.benchmarkProtocol);
  assertExpectedSample(message, expected);
  if (message.traceSchema !== BENCHMARK_TRACE_SCHEMA_VERSION) {
    throw new RealmProtocolError("Benchmark trace schema version is invalid.");
  }
  const mode = benchmarkIntentModeFromKind(expected.intentKind);
  const phasePath = benchmarkPhasePath(expected.engine, mode);
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
      mode,
      outcome: "success"
    });
    assertTraceTimeBudget(trace, phasePath, false);
    const resources = validateResourceObservations(
      resourceValue,
      trace.sample_end
    );
    if (version === null) {
      throw new RealmProtocolError(
        "Successful benchmark response has no version."
      );
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
      version
    });
  }

  if (!isBenchmarkFailureStage(message.stage)) {
    throw new RealmProtocolError("Benchmark failure stage is invalid.");
  }
  const failureMessage = expectString(message.message, "message");
  assertByteBudget(failureMessage, REALM_BUDGETS.errorBytes, "message");
  const failureDetail = expectNullableBoundedString(
    message.detail,
    "detail",
    REALM_BUDGETS.errorBytes
  );
  const failureStage = message.stage;
  const trace =
    message.trace === null
      ? null
      : validateBenchmarkRawTrace(message.trace, {
          engine: expected.engine,
          mode,
          outcome: "failure"
        });
  if (trace === null && failureStage !== "environment") {
    throw new RealmProtocolError(
      "Benchmark post-clock failure must retain its raw trace."
    );
  }
  if (trace !== null) {
    assertFailureStageMatchesTrace(failureStage, trace, phasePath);
    assertTraceTimeBudget(trace, phasePath, failureStage === "timeout");
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
    detail: failureDetail,
    version
  });
}

function sampleIdentity(
  expected: BenchmarkExpectedSample
): BenchmarkExpectedSample {
  return {
    engine: expected.engine,
    intentKind: expected.intentKind,
    requestId: expected.requestId,
    runId: expected.runId,
    runToken: expected.runToken,
    sampleId: expected.sampleId
  };
}

function assertFailureStageMatchesTrace(
  stage: BenchmarkFailureStage,
  trace: BenchmarkRawTrace,
  path: FrozenBenchmarkPhasePath
): void {
  if (stage === "environment") {
    throw new RealmProtocolError(
      "Post-clock environment failure must not use a sample response."
    );
  }
  if (stage === "timeout" || stage === "protocol" || stage === "disposed") {
    return;
  }

  const phase = stage === "svg-budget" ? "render" : stage;
  const boundary = path.boundary(phase);
  if (boundary === null) {
    throw new RealmProtocolError(
      "Benchmark failure stage does not apply to its phase path."
    );
  }
  if (trace[boundary.start] === null) {
    throw new RealmProtocolError(
      `Benchmark ${stage} failure has no ${phase} evidence.`
    );
  }
  if (
    (stage === "render" ||
      stage === "svg-budget" ||
      stage === "presentation") &&
    trace[boundary.end] !== null
  ) {
    throw new RealmProtocolError(
      `Benchmark ${stage} failure has an invalid ${phase} prefix.`
    );
  }
  if (
    path.applicableEvents.some(
      (event) =>
        trace[event] !== null &&
        event !== boundary.end &&
        path.dependsOn(event, boundary.end)
    )
  ) {
    throw new RealmProtocolError(
      `Benchmark ${stage} failure contains later phase evidence.`
    );
  }
}

function assertTraceTimeBudget(
  trace: BenchmarkRawTrace,
  path: FrozenBenchmarkPhasePath,
  allowStageTimeout: boolean
): void {
  if (trace.sample_end > REALM_BUDGETS.runTimeoutMs) {
    throw new RealmProtocolError(
      "Benchmark trace exceeds the run time budget."
    );
  }
  if (allowStageTimeout) return;
  for (const phase of path.timedPhases) {
    const boundary = path.boundary(phase);
    if (boundary === null) continue;
    const start = trace[boundary.start];
    const end = trace[boundary.end];
    const elapsed = start === null ? null : (end ?? trace.sample_end) - start;
    if (elapsed !== null && elapsed > REALM_BUDGETS.stageTimeoutMs) {
      throw new RealmProtocolError(
        "Benchmark trace exceeds a stage time budget."
      );
    }
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
    throw new RealmProtocolError(
      "Benchmark resource observations are invalid."
    );
  }
  return Object.freeze(
    value.map((candidate) => {
      const observation = expectRecord(candidate, "resource observation");
      assertExactKeys(observation, RESOURCE_OBSERVATION_SCHEMA);
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
        )
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
    "sampleId",
    "engine",
    "intentKind"
  ] as const) {
    if (message[key] !== expected[key]) {
      throw new RealmProtocolError(`Benchmark ${key} is invalid.`);
    }
  }
}

function expectProgressEvent(
  value: unknown,
  expected: BenchmarkExpectedSample
): BenchmarkTraceMark {
  if (typeof value !== "string") {
    throw new RealmProtocolError("Benchmark progress event is invalid.");
  }
  const mode = benchmarkIntentModeFromKind(expected.intentKind);
  const path = benchmarkPhasePath(expected.engine, mode);
  if (!path.rule(value as BenchmarkTraceMark)) {
    throw new RealmProtocolError("Benchmark progress event is invalid.");
  }
  return value as BenchmarkTraceMark;
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

interface ExactKeySchema {
  readonly keys: ReadonlySet<string>;
  readonly size: number;
}

function exactKeySchema(keys: readonly string[]): ExactKeySchema {
  return Object.freeze({ keys: new Set(keys), size: keys.length });
}

function assertExactKeys(
  value: Record<string, unknown>,
  schema: ExactKeySchema
): void {
  const actual = Object.keys(value);
  if (
    actual.length !== schema.size ||
    actual.some((key) => !schema.keys.has(key))
  ) {
    throw new RealmProtocolError(
      "Benchmark message contains unexpected fields."
    );
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
    throw new RealmProtocolError(
      `Benchmark ${label} must be finite and non-negative.`
    );
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
