import {
  BINDING_PAYLOAD_SCHEMAS,
  RUNTIME_CATALOG_MAX_SAFE_INTEGER,
} from "./generated/binding-contract.mjs";

const BINDING_RESULT_SCHEMA = BINDING_PAYLOAD_SCHEMAS.find(
  (schema) => schema.id === "binding-result",
);
if (BINDING_RESULT_SCHEMA === undefined) {
  throw new Error("Generated binding contract is missing the binding-result schema.");
}
const BINDING_RESULT_PAYLOAD_VERSION = BINDING_RESULT_SCHEMA.version;
const JSON_NUMBER_TOKEN = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
const JSON_NUMBER_PARTS = /^-?(0|[1-9]\d*)(?:\.(\d+))?(?:[eE]([+-]?)(\d+))?$/;
const MAX_SAFE_INTEGER_DECIMAL = String(RUNTIME_CATALOG_MAX_SAFE_INTEGER);
const RUNTIME_CATALOG_EXACT_SAFE_INTEGER_PATHS = Object.freeze([
  ["schema_version"],
  ["transport_api_version"],
  ["options_schema_versions", "*"],
  ["payload_schemas", "*", "version"],
  ["capabilities", "text_measurement", "protocol_version"],
  ["constructor_service_contracts", "*", "resource_limits", "*", "value"],
  [
    "output_contracts",
    "*",
    "embedded_images",
    "limits",
    "max_bytes_per_image",
  ],
  ["output_contracts", "*", "embedded_images", "limits", "max_total_bytes"],
  [
    "output_contracts",
    "*",
    "embedded_images",
    "limits",
    "max_pixels_per_image",
  ],
  ["output_contracts", "*", "embedded_images", "limits", "max_total_pixels"],
  ["registry", "diagram_family_count"],
  ["resources", "limits", "*", "minimum_value"],
  ["resources", "profiles", "*", "limits", "*"],
]);

export class MermanError extends Error {
  constructor(message, { code = "MERMAN_NODE_ERROR", cause } = {}) {
    super(message, { cause });
    this.name = new.target.name;
    this.code = code;
  }
}

export class MermanOperationError extends MermanError {
  constructor(payload) {
    super(payload.message, { code: payload.code_name ?? "MERMAN_OPERATION_ERROR" });
    this.status = payload.code ?? null;
    this.codeName = payload.code_name ?? null;
    this.kind = payload.kind ?? "generic";
    this.capabilityId = payload.capability_id ?? null;
    this.resourceDetails = payload.details?.resource ?? null;
  }
}

export class MermanQueueSaturatedError extends MermanError {
  constructor(maxQueue) {
    super(`Merman queue is full (maxQueue=${maxQueue}).`, {
      code: "MERMAN_QUEUE_SATURATED",
    });
    this.maxQueue = maxQueue;
  }
}

export class MermanDisposedError extends MermanError {
  constructor() {
    super("Merman engine has been disposed.", { code: "MERMAN_ENGINE_DISPOSED" });
  }
}

export class MermanLifecycleError extends MermanError {
  constructor(message) {
    super(message, { code: "MERMAN_LIFECYCLE_ERROR" });
  }
}

export class MermanUnsupportedTargetError extends MermanError {
  constructor({ platform, arch, libc = null, reason = null }) {
    const target = [platform, arch, libc].filter(Boolean).join("-");
    super(reason ?? `Unsupported Merman Node target: ${target}.`, {
      code: "MERMAN_UNSUPPORTED_TARGET",
    });
    this.platform = platform;
    this.arch = arch;
    this.libc = libc;
  }
}

export class MermanMissingPlatformPackageError extends MermanError {
  constructor({ packageName, target, cause }) {
    super(
      `The required Merman platform package ${packageName} is not installed for ${target}.`,
      { code: "MERMAN_MISSING_PLATFORM_PACKAGE", cause },
    );
    this.packageName = packageName;
    this.target = target;
  }
}

export class MermanInvalidTransportError extends MermanError {
  constructor(message, cause) {
    super(message, { code: "MERMAN_INVALID_TRANSPORT", cause });
  }
}

export const NODE_TRANSPORT_LIMITS = Object.freeze({
  metadataBytes: 8 * 1024 * 1024,
  runtimeCatalogBytes: 1024 * 1024,
});

export function parseTransportJsonText(value, label, maxUtf8Bytes) {
  if (typeof value !== "string") {
    throw new MermanInvalidTransportError(`Merman transport ${label} must be JSON text.`);
  }
  const byteLength = Buffer.byteLength(value, "utf8");
  if (byteLength > maxUtf8Bytes) {
    throw new MermanInvalidTransportError(
      `Merman transport ${label} exceeds the ${maxUtf8Bytes}-byte wire limit.`,
    );
  }
  try {
    return JSON.parse(value);
  } catch (cause) {
    throw new MermanInvalidTransportError(
      `Merman transport returned invalid ${label} JSON.`,
      cause,
    );
  }
}

export function parseRuntimeCatalogJsonText(value, label = "runtime catalog") {
  const parsed = parseTransportJsonText(
    value,
    label,
    NODE_TRANSPORT_LIMITS.runtimeCatalogBytes,
  );
  assertExactSafeIntegerJsonTokens(
    value,
    label,
    RUNTIME_CATALOG_EXACT_SAFE_INTEGER_PATHS,
  );
  return parsed;
}

function assertExactSafeIntegerJsonTokens(value, label, paths) {
  // JSON.parse rounds before semantic validators can observe the original token. Follow only the
  // known integer paths so additive schema-1 subtrees retain ordinary JSON number semantics.
  const candidates = paths.map((_, index) => index);
  scanJsonValue(value, skipJsonWhitespace(value, 0), paths, candidates, 0, (token) => {
    if (!isExactSafeIntegerJsonToken(token, value.length)) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} must encode known integer fields as exact JSON-safe integers.`,
      );
    }
  });
}

function scanJsonValue(source, index, paths, candidates, depth, visitNumber) {
  index = skipJsonWhitespace(source, index);
  const character = source[index];
  const terminal = candidates.some((candidate) => paths[candidate].length === depth);
  if (terminal && isJsonNumberStart(character)) {
    return visitJsonNumber(source, index, visitNumber);
  }
  const continuing = candidates.filter((candidate) => paths[candidate].length > depth);
  // Once no known path can match, skip the complete subtree without materializing a token tree.
  if (continuing.length === 0) return skipJsonValue(source, index);
  if (character === "{") {
    return scanJsonObject(source, index, paths, continuing, depth, visitNumber);
  }
  if (character === "[") {
    return scanJsonArray(source, index, paths, continuing, depth, visitNumber);
  }
  return skipJsonValue(source, index);
}

function scanJsonObject(source, index, paths, candidates, depth, visitNumber) {
  index = skipJsonWhitespace(source, index + 1);
  if (source[index] === "}") return index + 1;
  while (index < source.length) {
    const keyEnd = skipJsonString(source, index + 1);
    const key = JSON.parse(source.slice(index, keyEnd));
    index = skipJsonWhitespace(source, keyEnd);
    index = skipJsonWhitespace(source, index + 1);
    index = scanJsonValue(
      source,
      index,
      paths,
      advancePathCandidates(paths, candidates, depth, key),
      depth + 1,
      visitNumber,
    );
    index = skipJsonWhitespace(source, index);
    if (source[index] === "}") return index + 1;
    index = skipJsonWhitespace(source, index + 1);
  }
  return index;
}

function scanJsonArray(source, index, paths, candidates, depth, visitNumber) {
  const elementCandidates = advancePathCandidates(paths, candidates, depth, "*");
  index = skipJsonWhitespace(source, index + 1);
  if (source[index] === "]") return index + 1;
  while (index < source.length) {
    index = scanJsonValue(
      source,
      index,
      paths,
      elementCandidates,
      depth + 1,
      visitNumber,
    );
    index = skipJsonWhitespace(source, index);
    if (source[index] === "]") return index + 1;
    index = skipJsonWhitespace(source, index + 1);
  }
  return index;
}

function advancePathCandidates(paths, candidates, depth, segment) {
  return candidates.filter((candidate) => {
    const expected = paths[candidate][depth];
    return expected === "*" || expected === segment;
  });
}

function skipJsonValue(source, index) {
  index = skipJsonWhitespace(source, index);
  const character = source[index];
  if (character === '"') return skipJsonString(source, index + 1);
  if (isJsonNumberStart(character)) return visitJsonNumber(source, index, () => {});
  if (character !== "{" && character !== "[") {
    if (source.startsWith("true", index)) return index + 4;
    if (source.startsWith("false", index)) return index + 5;
    return index + 4;
  }
  let nesting = 0;
  while (index < source.length) {
    if (source[index] === '"') {
      index = skipJsonString(source, index + 1);
      continue;
    }
    if (source[index] === "{" || source[index] === "[") {
      nesting += 1;
    } else if (source[index] === "}" || source[index] === "]") {
      nesting -= 1;
      if (nesting === 0) return index + 1;
    }
    index += 1;
  }
  return index;
}

function visitJsonNumber(source, index, visit) {
  JSON_NUMBER_TOKEN.lastIndex = index;
  const match = JSON_NUMBER_TOKEN.exec(source);
  if (match === null) return index + 1;
  visit(match[0]);
  return JSON_NUMBER_TOKEN.lastIndex;
}

function isJsonNumberStart(character) {
  return character === "-" || (character >= "0" && character <= "9");
}

function skipJsonWhitespace(source, index) {
  while (
    source[index] === " " ||
    source[index] === "\n" ||
    source[index] === "\r" ||
    source[index] === "\t"
  ) {
    index += 1;
  }
  return index;
}

function skipJsonString(value, index) {
  while (index < value.length) {
    if (value[index] === "\\") {
      index += 2;
    } else if (value[index] === '"') {
      return index + 1;
    } else {
      index += 1;
    }
  }
  return index;
}

function isExactSafeIntegerJsonToken(token, sourceLength) {
  const match = JSON_NUMBER_PARTS.exec(token);
  if (match === null) return false;
  const [, integerDigits, fractionDigits = "", exponentSign = "", exponentDigits = "0"] = match;
  const significantDigits = `${integerDigits}${fractionDigits}`.replace(/^0+/, "");
  if (significantDigits.length === 0) return true;

  const normalizedExponentDigits = exponentDigits.replace(/^0+/, "") || "0";
  const maximumRelevantExponent = sourceLength + MAX_SAFE_INTEGER_DECIMAL.length;
  const maximumRelevantExponentText = String(maximumRelevantExponent);
  if (
    normalizedExponentDigits.length > maximumRelevantExponentText.length ||
    (normalizedExponentDigits.length === maximumRelevantExponentText.length &&
      normalizedExponentDigits > maximumRelevantExponentText)
  ) {
    return false;
  }
  const exponentMagnitude = Number(normalizedExponentDigits);
  const exponent = exponentSign === "-" ? -exponentMagnitude : exponentMagnitude;
  const decimalShift = exponent - fractionDigits.length;

  let integerValueDigits;
  if (decimalShift >= 0) {
    if (significantDigits.length + decimalShift > MAX_SAFE_INTEGER_DECIMAL.length) {
      return false;
    }
    integerValueDigits = `${significantDigits}${"0".repeat(decimalShift)}`;
  } else {
    const fractionalDigits = -decimalShift;
    if (fractionalDigits >= significantDigits.length) return false;
    if (!/^0+$/.test(significantDigits.slice(-fractionalDigits))) return false;
    integerValueDigits = significantDigits.slice(0, -fractionalDigits);
  }

  return integerValueDigits.length < MAX_SAFE_INTEGER_DECIMAL.length ||
    (integerValueDigits.length === MAX_SAFE_INTEGER_DECIMAL.length &&
      integerValueDigits <= MAX_SAFE_INTEGER_DECIMAL);
}

export function abortError() {
  if (typeof DOMException === "function") {
    return new DOMException("The queued Merman operation was aborted.", "AbortError");
  }
  const error = new Error("The queued Merman operation was aborted.");
  error.name = "AbortError";
  return error;
}

export function decodeWireResponse(value) {
  if (typeof value !== "string") {
    throw new MermanInvalidTransportError("Merman transport response must be JSON text.");
  }
  let envelope;
  try {
    envelope = JSON.parse(value);
  } catch (cause) {
    throw new MermanInvalidTransportError("Merman transport returned invalid JSON.", cause);
  }
  if (
    !envelope ||
    typeof envelope !== "object" ||
    envelope.version !== BINDING_RESULT_PAYLOAD_VERSION
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport returned an unsupported response envelope.",
    );
  }
  if (envelope.ok === false && envelope.error && typeof envelope.error === "object") {
    throw new MermanOperationError(envelope.error);
  }
  if (
    envelope.ok !== true ||
    !envelope.result ||
    typeof envelope.result.operation_id !== "string" ||
    envelope.result.operation_id.length === 0 ||
    typeof envelope.result.media_type !== "string" ||
    envelope.result.media_type.length === 0 ||
    typeof envelope.result.data !== "string" ||
    typeof envelope.result.metadata_json !== "string"
  ) {
    throw new MermanInvalidTransportError("Merman transport returned an invalid result envelope.");
  }
  return envelope.result;
}

export function decodeWireCreationError(cause, label) {
  return decodeWireFailure(cause, `${label} failed to initialize.`);
}

export function decodeWireInvocationError(cause, label) {
  return decodeWireFailure(cause, `${label} failed.`);
}

function decodeWireFailure(cause, fallbackMessage) {
  if (cause instanceof MermanError) return cause;
  const value = cause instanceof Error ? cause.message : cause;
  try {
    const envelope = typeof value === "string" ? JSON.parse(value) : value;
    if (
      envelope &&
      typeof envelope === "object" &&
      envelope.version === BINDING_RESULT_PAYLOAD_VERSION &&
      envelope.ok === false &&
      envelope.error &&
      typeof envelope.error === "object"
    ) {
      return new MermanOperationError(envelope.error);
    }
  } catch {
    // Fall through to the transport-level error below.
  }
  return new MermanInvalidTransportError(fallbackMessage, cause);
}
