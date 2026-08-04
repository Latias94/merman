import {
  isDiagramFont,
  type DiagramFont,
} from "../../lib/diagram-font.ts";
import {
  normalizeMermaidExternalRequirements,
  type MermaidExternalRequirements,
} from "../mermaid-requirements.ts";
import {
  exceedsUtf8ByteBudget,
  utf8ByteLength,
} from "../../lib/utf8.ts";

export { utf8ByteLength } from "../../lib/utf8.ts";

export const REALM_PROTOCOL_VERSION = 2 as const;

export const REALM_BUDGETS = Object.freeze({
  // Engine artifacts are generated, hash-bound program inputs. They are kept
  // separate from user-controlled protocol messages so a large Mermaid engine
  // cannot force the source/SVG budgets to grow with it. Mermaid 11.16 plus
  // the admitted external modules stays below this raw-source ceiling while
  // the encoded envelope remains independently bounded by realmInitBytes.
  engineArtifactBytes: 44 * 1024 * 1024,
  realmInitBytes: 48 * 1024 * 1024,
  sourceBytes: 2 * 1024 * 1024,
  configBytes: 1024 * 1024,
  svgBytes: 24 * 1024 * 1024,
  messageBytes: 25 * 1024 * 1024,
  errorBytes: 64 * 1024,
  stageTimeoutMs: 30_000,
  runTimeoutMs: 10 * 60_000,
  maxViewportDimension: 4096,
  maxViewportPixels: 4096 * 4096,
});

export const BENCHMARK_BUDGETS = Object.freeze({
  maxActiveRuns: 1,
  maxIterations: 1_000,
  maxLiveRealms: 2,
  maxResourceObservations: 128,
  maxRetainedSamples: 2_000,
  maxWarmups: 200,
});

export type RealmKind = "compare" | "benchmark";
export type RealmEngineArtifactId =
  | "mermaid"
  | "benchmark-merman";

export interface RealmEngineArtifactIdentity {
  readonly bytes: number;
  readonly id: RealmEngineArtifactId;
  readonly schemaVersion: 1;
  readonly sha256: string;
}

export interface RealmEngineArtifact extends RealmEngineArtifactIdentity {
  readonly resourceUrl: string | null;
  readonly source: string;
}

export interface RealmIdentity {
  readonly kind: RealmKind;
  readonly realmId: string;
  readonly realmToken: string;
}

export interface RealmBootIdentity {
  readonly bootNonce: string;
  readonly kind: RealmKind;
  readonly realmId: string;
}

export interface RealmHello extends RealmBootIdentity {
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly type: "realm-hello";
}

export interface RealmInit extends RealmBootIdentity, RealmIdentity {
  readonly engineArtifact: RealmEngineArtifact;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly type: "realm-init";
}

export interface RealmReady extends RealmIdentity {
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly sequence: 0;
  readonly type: "realm-ready";
  readonly viewport: RealmViewport;
}

export interface RealmFatal extends RealmIdentity {
  readonly message: string;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly sequence: number;
  readonly type: "realm-fatal";
}

export interface RealmViewport {
  readonly height: number;
  readonly width: number;
}

export interface CompareRenderPayload {
  readonly configJson: string;
  readonly diagramFont: DiagramFont;
  readonly externalRequirements: MermaidExternalRequirements;
  readonly source: string;
  readonly theme: string;
  readonly viewport: RealmViewport;
}

export interface CompareRenderRequest extends RealmIdentity {
  readonly payload: CompareRenderPayload;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly sequence: number;
  readonly type: "render";
}

export type CompareOperationStage =
  | "fonts"
  | "adapter-import"
  | "load"
  | "register"
  | "initialize"
  | "render"
  | "svg-budget"
  | "presentation";

export const COMPARE_OPERATION_STAGES: readonly CompareOperationStage[] = [
  "fonts",
  "adapter-import",
  "load",
  "register",
  "initialize",
  "render",
  "svg-budget",
  "presentation",
];

export interface CompareRenderProgress extends RealmIdentity {
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly sequence: number;
  readonly stage: CompareOperationStage;
  readonly type: "render-progress";
}

export interface CompareRenderSuccess extends RealmIdentity {
  readonly prepareTimeMs: number;
  readonly presentationTimeMs: number;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly renderTimeMs: number;
  readonly requestId: string;
  readonly sequence: number;
  readonly svg: string;
  readonly type: "render-success";
  readonly version: string;
}

export type CompareFailureStage =
  | "handshake"
  | "protocol"
  | "timeout"
  | "disposed"
  | "svg-validation"
  | CompareOperationStage;

export interface CompareRenderFailure extends RealmIdentity {
  readonly detail: string | null;
  readonly message: string;
  readonly protocol: typeof REALM_PROTOCOL_VERSION;
  readonly requestId: string;
  readonly sequence: number;
  readonly stage: CompareFailureStage;
  readonly type: "render-failure";
}

export type CompareRenderResponse =
  | CompareRenderSuccess
  | CompareRenderFailure;

const FAILURE_STAGES = new Set<CompareFailureStage>([
  "handshake",
  "protocol",
  "timeout",
  "disposed",
  ...COMPARE_OPERATION_STAGES,
]);
const OPERATION_STAGES = new Set(COMPARE_OPERATION_STAGES);
export class RealmProtocolError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RealmProtocolError";
  }
}

export class RealmBudgetError extends RealmProtocolError {
  readonly resource: string;

  constructor(resource: string, message: string) {
    super(message);
    this.name = "RealmBudgetError";
    this.resource = resource;
  }
}

export class RealmTimeoutError extends RealmProtocolError {
  constructor(message: string) {
    super(message);
    this.name = "RealmTimeoutError";
  }
}

export function assertEncodedMessageBudget(value: unknown): void {
  assertEncodedBudget(
    value,
    REALM_BUDGETS.messageBytes,
    "message",
    "Realm message exceeds the 25 MiB budget."
  );
}

export function assertRealmInitBudget(value: unknown): void {
  assertEncodedBudget(
    value,
    REALM_BUDGETS.realmInitBytes,
    "engineArtifact",
    "Realm initialization exceeds the 48 MiB engine-artifact budget."
  );
}

function assertEncodedBudget(
  value: unknown,
  maxBytes: number,
  resource: string,
  errorMessage: string
): void {
  let encoded: string;
  try {
    encoded = typeof value === "string" ? value : JSON.stringify(value);
  } catch {
    throw new RealmProtocolError("Realm message is not JSON-encodable.");
  }
  if (exceedsUtf8Budget(encoded, maxBytes)) {
    throw new RealmBudgetError(resource, errorMessage);
  }
}

export function createRealmToken(): string {
  const crypto = globalThis.crypto;
  if (!crypto?.getRandomValues) {
    throw new RealmProtocolError("Secure browser entropy is unavailable.");
  }
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return encodeBase64Url(bytes);
}

export function validateRealmHello(
  value: unknown,
  expected: RealmBootIdentity
): RealmHello {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "hello");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "bootNonce",
  ]);
  assertBootEnvelope(message, expected, "realm-hello");
  return {
    type: "realm-hello",
    protocol: REALM_PROTOCOL_VERSION,
    ...expected,
  };
}

export function validateRealmInit(
  value: unknown,
  expected: RealmBootIdentity,
  expectedArtifact: RealmEngineArtifactIdentity
): RealmInit {
  assertRealmInitBudget(value);
  const message = expectRecord(value, "init");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "bootNonce",
    "realmToken",
    "engineArtifact",
  ]);
  assertBootEnvelope(message, expected, "realm-init");
  const realmToken = expectSecureToken(message.realmToken, "realmToken");
  const engineArtifact = validateRealmEngineArtifact(
    message.engineArtifact,
    expectedArtifact
  );
  return {
    type: "realm-init",
    protocol: REALM_PROTOCOL_VERSION,
    ...expected,
    realmToken,
    engineArtifact,
  };
}

export function createOneTimeRealmInitGate(
  expected: RealmBootIdentity,
  expectedArtifact: RealmEngineArtifactIdentity
) {
  let consumed = false;
  return {
    consume(value: unknown, transferredPortCount: number): RealmInit {
      if (consumed) {
        throw new RealmProtocolError("Realm INIT was replayed.");
      }
      if (transferredPortCount !== 1) {
        throw new RealmProtocolError("Realm INIT must transfer one port.");
      }
      const init = validateRealmInit(value, expected, expectedArtifact);
      consumed = true;
      return init;
    },
  };
}

export function validateRealmEngineArtifact(
  value: unknown,
  expected: RealmEngineArtifactIdentity
): RealmEngineArtifact {
  const artifact = expectRecord(value, "engine artifact");
  assertExactKeys(artifact, [
    "schemaVersion",
    "id",
    "bytes",
    "sha256",
    "resourceUrl",
    "source",
  ]);
  if (
    expected.schemaVersion !== 1 ||
    artifact.schemaVersion !== 1 ||
    artifact.id !== expected.id ||
    artifact.bytes !== expected.bytes ||
    artifact.sha256 !== expected.sha256 ||
    typeof artifact.bytes !== "number" ||
    !Number.isSafeInteger(artifact.bytes) ||
    artifact.bytes <= 0 ||
    artifact.bytes > REALM_BUDGETS.engineArtifactBytes ||
    typeof artifact.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/u.test(artifact.sha256)
  ) {
    throw new RealmProtocolError("Realm engine artifact identity is invalid.");
  }
  const source = expectString(artifact.source, "engine artifact source");
  if (utf8ByteLength(source) !== artifact.bytes) {
    throw new RealmProtocolError("Realm engine artifact byte length is invalid.");
  }
  const resourceUrl = artifact.resourceUrl;
  if (
    resourceUrl !== null &&
    (typeof resourceUrl !== "string" ||
      resourceUrl.length === 0 ||
      utf8ByteLength(resourceUrl) > 4096)
  ) {
    throw new RealmProtocolError("Realm engine resource URL is invalid.");
  }
  if (artifact.id !== "benchmark-merman" && resourceUrl !== null) {
    throw new RealmProtocolError(
      "Only the Merman benchmark may receive an engine resource URL."
    );
  }
  return Object.freeze({
    schemaVersion: 1,
    id: artifact.id as RealmEngineArtifactId,
    bytes: artifact.bytes,
    sha256: artifact.sha256,
    resourceUrl: resourceUrl as string | null,
    source,
  });
}

export function validateRealmReady(
  value: unknown,
  identity: RealmIdentity
): RealmReady {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "ready");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "viewport",
  ]);
  assertEnvelope(message, identity, 0, "realm-ready");
  return {
    type: "realm-ready",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: 0,
    viewport: validateRealmViewport(message.viewport),
  };
}

export function validateRealmFatal(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number
): RealmFatal {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "fatal message");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "message",
  ]);
  assertEnvelope(message, identity, expectedSequence, "realm-fatal");
  const fatalMessage = expectString(message.message, "message");
  assertTextBudget(fatalMessage, REALM_BUDGETS.errorBytes, "message");
  return {
    type: "realm-fatal",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    message: fatalMessage,
  };
}

export function validateCompareRenderRequest(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number
): CompareRenderRequest {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "render request");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "requestId",
    "payload",
  ]);
  assertEnvelope(message, identity, expectedSequence, "render");
  const requestId = expectBoundedString(message.requestId, "requestId", 128);
  const payload = validateCompareRenderPayload(message.payload);
  return {
    type: "render",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    requestId,
    payload,
  };
}

export function validateCompareRenderPayload(
  value: unknown
): CompareRenderPayload {
  assertEncodedMessageBudget(value);
  const payload = expectRecord(value, "render payload");
  assertExactKeys(payload, [
    "source",
    "configJson",
    "theme",
    "diagramFont",
    "externalRequirements",
    "viewport",
  ]);

  const source = expectString(payload.source, "source");
  assertTextBudget(source, REALM_BUDGETS.sourceBytes, "source");
  const configJson = expectString(payload.configJson, "configJson");
  assertTextBudget(configJson, REALM_BUDGETS.configBytes, "configJson");
  const theme = expectBoundedString(payload.theme, "theme", 128);
  const diagramFont = payload.diagramFont;
  if (typeof diagramFont !== "string" || !isDiagramFont(diagramFont)) {
    throw new RealmProtocolError("Realm diagramFont is invalid.");
  }

  const requirements = expectRecord(
    payload.externalRequirements,
    "externalRequirements"
  );
  assertExactKeys(requirements, ["externalDiagrams", "layoutModules"]);
  const externalDiagrams = expectStringArray(
    requirements.externalDiagrams,
    "externalDiagrams"
  );
  const layoutModules = expectStringArray(
    requirements.layoutModules,
    "layoutModules"
  );
  let normalizedRequirements: MermaidExternalRequirements;
  try {
    normalizedRequirements = normalizeMermaidExternalRequirements({
      externalDiagrams,
      layoutModules,
    });
  } catch {
    throw new RealmProtocolError("Realm external requirements are invalid.");
  }
  if (
    !sameStrings(externalDiagrams, normalizedRequirements.externalDiagrams) ||
    !sameStrings(layoutModules, normalizedRequirements.layoutModules)
  ) {
    throw new RealmProtocolError(
      "Realm external requirements must be sorted and deduplicated."
    );
  }

  return {
    source,
    configJson,
    theme,
    diagramFont: diagramFont as DiagramFont,
    externalRequirements: normalizedRequirements,
    viewport: validateRealmViewport(payload.viewport),
  };
}

export function isRealmMessageType(value: unknown, type: string): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    (value as Record<string, unknown>).type === type
  );
}

export function validateCompareRenderResponse(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number,
  expectedRequestId: string
): CompareRenderResponse {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "render response");
  const type = message.type;
  if (type === "render-success") {
    assertExactKeys(message, [
      "type",
      "protocol",
      "kind",
      "realmId",
      "realmToken",
      "sequence",
      "requestId",
      "svg",
      "prepareTimeMs",
      "renderTimeMs",
      "presentationTimeMs",
      "version",
    ]);
    assertEnvelope(message, identity, expectedSequence, type);
    assertRequestId(message.requestId, expectedRequestId);
    const svg = expectString(message.svg, "svg");
    assertTextBudget(svg, REALM_BUDGETS.svgBytes, "svg");
    return {
      type,
      protocol: REALM_PROTOCOL_VERSION,
      ...identity,
      sequence: expectedSequence,
      requestId: expectedRequestId,
      svg,
      prepareTimeMs: expectDuration(message.prepareTimeMs, "prepareTimeMs"),
      renderTimeMs: expectDuration(message.renderTimeMs, "renderTimeMs"),
      presentationTimeMs: expectDuration(
        message.presentationTimeMs,
        "presentationTimeMs"
      ),
      version: expectBoundedString(message.version, "version", 256),
    };
  }

  if (type === "render-failure") {
    assertExactKeys(message, [
      "type",
      "protocol",
      "kind",
      "realmId",
      "realmToken",
      "sequence",
      "requestId",
      "stage",
      "message",
      "detail",
    ]);
    assertEnvelope(message, identity, expectedSequence, type);
    assertRequestId(message.requestId, expectedRequestId);
    if (
      typeof message.stage !== "string" ||
      !FAILURE_STAGES.has(message.stage as CompareFailureStage)
    ) {
      throw new RealmProtocolError("Realm failure stage is invalid.");
    }
    const failureMessage = expectString(message.message, "message");
    assertTextBudget(failureMessage, REALM_BUDGETS.errorBytes, "message");
    const failureDetail = expectNullableString(message.detail, "detail");
    if (failureDetail !== null) {
      assertTextBudget(failureDetail, REALM_BUDGETS.errorBytes, "detail");
    }
    return {
      type,
      protocol: REALM_PROTOCOL_VERSION,
      ...identity,
      sequence: expectedSequence,
      requestId: expectedRequestId,
      stage: message.stage as CompareFailureStage,
      message: failureMessage,
      detail: failureDetail,
    };
  }

  throw new RealmProtocolError("Realm response type is invalid.");
}

export function validateCompareRenderProgress(
  value: unknown,
  identity: RealmIdentity,
  expectedSequence: number,
  expectedRequestId: string
): CompareRenderProgress {
  assertEncodedMessageBudget(value);
  const message = expectRecord(value, "render progress");
  assertExactKeys(message, [
    "type",
    "protocol",
    "kind",
    "realmId",
    "realmToken",
    "sequence",
    "requestId",
    "stage",
  ]);
  assertEnvelope(message, identity, expectedSequence, "render-progress");
  assertRequestId(message.requestId, expectedRequestId);
  if (
    typeof message.stage !== "string" ||
    !OPERATION_STAGES.has(message.stage as CompareOperationStage)
  ) {
    throw new RealmProtocolError("Realm progress stage is invalid.");
  }
  return {
    type: "render-progress",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: expectedSequence,
    requestId: expectedRequestId,
    stage: message.stage as CompareOperationStage,
  };
}

export function advanceCompareOperationStage(
  currentStageIndex: number,
  nextStage: CompareOperationStage
): number {
  const nextStageIndex = COMPARE_OPERATION_STAGES.indexOf(nextStage);
  if (
    !Number.isInteger(currentStageIndex) ||
    currentStageIndex < -1 ||
    currentStageIndex >= COMPARE_OPERATION_STAGES.length ||
    nextStageIndex !== currentStageIndex + 1
  ) {
    throw new RealmProtocolError(
      "Mermaid realm progress must advance exactly one stage."
    );
  }
  return nextStageIndex;
}

export function assertRealmSourceBudget(source: string): void {
  assertTextBudget(source, REALM_BUDGETS.sourceBytes, "effective source");
}

export function assertRealmSvgBudget(svg: string): void {
  assertTextBudget(svg, REALM_BUDGETS.svgBytes, "svg");
}

export function validateRealmViewport(value: unknown): RealmViewport {
  const viewport = expectRecord(value, "viewport");
  assertExactKeys(viewport, ["width", "height"]);
  const width = expectPositiveFinite(viewport.width, "viewport width");
  const height = expectPositiveFinite(viewport.height, "viewport height");
  if (
    width > REALM_BUDGETS.maxViewportDimension ||
    height > REALM_BUDGETS.maxViewportDimension ||
    width * height > REALM_BUDGETS.maxViewportPixels
  ) {
    throw new RealmProtocolError("Realm viewport exceeds its layout budget.");
  }
  return { width, height };
}

function assertEnvelope(
  message: Record<string, unknown>,
  identity: RealmIdentity,
  expectedSequence: number,
  expectedType: string
): void {
  if (message.type !== expectedType) {
    throw new RealmProtocolError("Realm message type is invalid.");
  }
  if (message.protocol !== REALM_PROTOCOL_VERSION) {
    throw new RealmProtocolError("Realm protocol version is invalid.");
  }
  if (
    message.kind !== identity.kind ||
    message.realmId !== identity.realmId ||
    message.realmToken !== identity.realmToken
  ) {
    throw new RealmProtocolError("Realm message identity is invalid.");
  }
  if (message.sequence !== expectedSequence) {
    throw new RealmProtocolError("Realm message sequence is invalid.");
  }
}

function assertBootEnvelope(
  message: Record<string, unknown>,
  expected: RealmBootIdentity,
  expectedType: string
): void {
  if (
    message.type !== expectedType ||
    message.protocol !== REALM_PROTOCOL_VERSION ||
    message.kind !== expected.kind ||
    message.realmId !== expected.realmId ||
    message.bootNonce !== expected.bootNonce
  ) {
    throw new RealmProtocolError("Realm boot identity is invalid.");
  }
  expectSecureToken(message.bootNonce, "bootNonce");
  expectBoundedString(message.realmId, "realmId", 128);
}

function assertRequestId(value: unknown, expected: string): void {
  if (value !== expected) {
    throw new RealmProtocolError("Realm request id is invalid.");
  }
}

function assertTextBudget(value: string, limit: number, label: string): void {
  if (exceedsUtf8Budget(value, limit)) {
    throw new RealmBudgetError(
      label,
      `Realm ${label} exceeds its byte budget.`
    );
  }
}

function exceedsUtf8Budget(value: string, limit: number): boolean {
  return exceedsUtf8ByteBudget(value, limit);
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new RealmProtocolError(`Realm ${label} must be an object.`);
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
    throw new RealmProtocolError("Realm message contains unexpected fields.");
  }
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new RealmProtocolError(`Realm ${label} must be a string.`);
  }
  return value;
}

function expectNullableString(value: unknown, label: string): string | null {
  if (value === null) return null;
  return expectString(value, label);
}

function expectStringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new RealmProtocolError(`Realm ${label} must be a string array.`);
  }
  return [...value] as string[];
}

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

function expectBoundedString(
  value: unknown,
  label: string,
  maxBytes: number
): string {
  const text = expectString(value, label);
  if (text.length === 0 || utf8ByteLength(text) > maxBytes) {
    throw new RealmProtocolError(`Realm ${label} is invalid.`);
  }
  return text;
}

function expectSecureToken(value: unknown, label: string): string {
  const token = expectString(value, label);
  if (!/^[A-Za-z0-9_-]{43}$/.test(token)) {
    throw new RealmProtocolError(`Realm ${label} is invalid.`);
  }
  return token;
}

function expectDuration(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new RealmProtocolError(`Realm ${label} must be finite and non-negative.`);
  }
  return value;
}

function expectPositiveFinite(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    throw new RealmProtocolError(`Realm ${label} must be finite and positive.`);
  }
  return value;
}

function encodeBase64Url(bytes: Uint8Array): string {
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let output = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0;
    const second = bytes[index + 1] ?? 0;
    const third = bytes[index + 2] ?? 0;
    const value = (first << 16) | (second << 8) | third;
    output += alphabet[(value >>> 18) & 63];
    output += alphabet[(value >>> 12) & 63];
    if (index + 1 < bytes.length) output += alphabet[(value >>> 6) & 63];
    if (index + 2 < bytes.length) output += alphabet[value & 63];
  }
  return output;
}
