import {
  BINDING_OPERATION_METADATA_CONTRACT,
  BINDING_PAYLOAD_SCHEMAS,
  RUNTIME_CATALOG_MAX_SAFE_INTEGER,
} from "./generated/binding-contract.mjs";
import { CAPABILITY_DESCRIPTOR_DIGEST } from "./generated/capability-surface.mjs";
import {
  NODE_TRANSPORT_FIELD_LIMITS,
  NODE_TRANSPORT_LIMITS,
  NODE_WIRE_CONTRACT,
} from "./transport-contract.mjs";

const BINDING_RESULT_SCHEMA = BINDING_PAYLOAD_SCHEMAS.find(
  (schema) => schema.id === "binding-result",
);
if (BINDING_RESULT_SCHEMA === undefined) {
  throw new Error("Generated binding contract is missing the binding-result schema.");
}
const BINDING_RESULT_PAYLOAD_VERSION = BINDING_RESULT_SCHEMA.version;
const BINDING_STATUS_NAME_BY_CODE = new Map([
  [1, "MERMAN_INVALID_ARGUMENT"],
  [2, "MERMAN_UTF8_ERROR"],
  [3, "MERMAN_OPTIONS_JSON_ERROR"],
  [4, "MERMAN_NO_DIAGRAM"],
  [5, "MERMAN_PARSE_ERROR"],
  [6, "MERMAN_RENDER_ERROR"],
  [7, "MERMAN_UNSUPPORTED_OPERATION"],
  [8, "MERMAN_PANIC"],
  [9, "MERMAN_INTERNAL_ERROR"],
  [10, "MERMAN_RESOURCE_LIMIT_EXCEEDED"],
  [11, "MERMAN_BUSY"],
  [12, "MERMAN_CANCELLED"],
]);
const CANCELLATION_REASONS = new Set(["requested", "deadline_exceeded"]);
const OPERATION_PHASE_IDENTIFIER = /^[a-z][a-z0-9_-]{0,63}$/;
const JSON_NUMBER_TOKEN = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
const JSON_NUMBER_PARTS = /^-?(0|[1-9]\d*)(?:\.(\d+))?(?:[eE]([+-]?)(\d+))?$/;
const MAX_SAFE_INTEGER_DECIMAL = String(RUNTIME_CATALOG_MAX_SAFE_INTEGER);
const U64_MAX_DECIMAL = "18446744073709551615";
const CANONICAL_WIDE_UNSIGNED_DECIMAL = /^[1-9]\d*$/;
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
const RESPONSE_EXACT_SAFE_INTEGER_PATHS = Object.freeze([
  ["version"],
  ["error", "code"],
  ["error", "details", "resource", "actual"],
  ["error", "details", "resource", "max"],
  ["error", "details", "diagnostic", "span", "start"],
  ["error", "details", "diagnostic", "span", "end"],
  ["error", "details", "diagnostic", "requested_max_width"],
  ["error", "details", "diagnostic", "actual_width"],
  ["error", "details", "icon_registry", "pack_index"],
]);
const OPERATION_METADATA_EXACT_SAFE_INTEGER_PATHS = Object.freeze([
  ...BINDING_OPERATION_METADATA_CONTRACT.fields
    .filter((field) => field.json_type === "unsigned-integer")
    .map((field) => [field.name]),
  ...BINDING_OPERATION_METADATA_CONTRACT.output_plans.flatMap((plan) =>
    plan.fields
      .filter((field) => field.json_type === "unsigned-integer")
      .map((field) => ["output_plan", field.name])
  ),
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
    this.diagnosticDetails = payload.details?.diagnostic ?? null;
    this.cancellationDetails = payload.details?.cancellation ?? null;
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

export class MermanNativeLoadError extends MermanError {
  constructor({ packageName, target, cause }) {
    super(
      `The installed Merman platform package ${packageName} could not load its native addon for ${target}. This is a native ABI or shared-library loader failure, not a missing npm package. Use a compatible glibc baseline or opt into @mermanjs/node-wasm.`,
      { code: "MERMAN_NATIVE_LOAD_ERROR", cause },
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

export { NODE_TRANSPORT_FIELD_LIMITS, NODE_TRANSPORT_LIMITS, NODE_WIRE_CONTRACT };

export function parseTransportJsonText(value, label, limits) {
  if (typeof value !== "string") {
    throw new MermanInvalidTransportError(`Merman transport ${label} must be JSON text.`);
  }
  if (!limits || typeof limits !== "object") {
    throw new TypeError(`Missing Node wire limits for ${label}.`);
  }
  const byteLength = strictUtf8ByteLength(value, label);
  if (byteLength > limits.max_utf8_bytes) {
    throw new MermanInvalidTransportError(
      `Merman transport ${label} exceeds the ${limits.max_utf8_bytes}-byte wire limit.`,
    );
  }
  scanBoundedJsonText(value, label, limits);
  try {
    return JSON.parse(value);
  } catch (cause) {
    throw new MermanInvalidTransportError(
      `Merman transport returned invalid ${label} JSON.`,
      cause,
    );
  }
}

export function encodeTransportJson(value, label, limits) {
  if (!limits || typeof limits !== "object") {
    throw new TypeError(`Missing Node wire limits for ${label}.`);
  }
  let boundedValue;
  try {
    boundedValue = cloneJsonWireValueWithinLimit(value, label, limits);
  } catch (cause) {
    if (cause instanceof MermanInvalidTransportError) throw cause;
    throw new MermanInvalidTransportError(
      `Merman could not encode ${label} as JSON text.`,
      cause,
    );
  }
  let text;
  try {
    text = JSON.stringify(boundedValue);
  } catch (cause) {
    throw new MermanInvalidTransportError(
      `Merman could not encode ${label} as JSON text.`,
      cause,
    );
  }
  parseTransportJsonText(text, label, limits);
  return text;
}

function cloneJsonWireValueWithinLimit(value, label, limits) {
  const budget = { label, max: limits.max_utf8_bytes, used: 0 };
  const active = new WeakSet();
  return cloneJsonWireValue(value, label, limits, budget, active, 1);
}

function cloneJsonWireValue(value, label, limits, budget, active, depth) {
  if (value === null) {
    addJsonWireBytes(budget, 4);
    return null;
  }
  if (typeof value === "boolean") {
    addJsonWireBytes(budget, value ? 4 : 5);
    return value;
  }
  if (typeof value === "string") {
    addJsonStringWireBytes(value, label, budget);
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${label} must be a finite JSON number.`);
    }
    addJsonWireBytes(budget, JSON.stringify(value).length);
    return value;
  }
  if (!value || typeof value !== "object") {
    throw new TypeError(`${label} is not a JSON wire value.`);
  }
  if (depth > limits.max_depth) {
    throw new RangeError(`${label} exceeds the structural depth limit ${limits.max_depth}.`);
  }
  if (active.has(value)) {
    throw new TypeError(`${label} must not contain cyclic references.`);
  }

  const isArray = Array.isArray(value);
  const prototype = Object.getPrototypeOf(value);
  if (!isArray && prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${label} must contain only arrays and plain objects.`);
  }

  active.add(value);
  try {
    if (isArray) {
      return cloneJsonWireArray(value, label, limits, budget, active, depth);
    }
    return cloneJsonWireObject(value, label, limits, budget, active, depth);
  } finally {
    active.delete(value);
  }
}

function cloneJsonWireArray(value, label, limits, budget, active, depth) {
  const keys = Reflect.ownKeys(value);
  if (keys.some((key) => typeof key === "symbol")) {
    throw new TypeError(`${label} must not contain symbol keys.`);
  }
  if (keys.length !== value.length + 1) {
    throw new TypeError(`${label} arrays must not contain custom properties.`);
  }

  const clone = [];
  Object.defineProperty(clone, "toJSON", { value: undefined });
  addJsonWireBytes(budget, 1);
  for (let index = 0; index < value.length; index += 1) {
    if (!Object.hasOwn(value, index)) {
      throw new TypeError(`${label} arrays must not contain holes.`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor?.enumerable || !("value" in descriptor)) {
      throw new TypeError(`${label}[${index}] must be an enumerable data property.`);
    }
    if (index > 0) addJsonWireBytes(budget, 1);
    clone[index] = cloneJsonWireValue(
      descriptor.value,
      `${label}[${index}]`,
      limits,
      budget,
      active,
      depth + 1,
    );
  }
  addJsonWireBytes(budget, 1);
  return clone;
}

function cloneJsonWireObject(value, label, limits, budget, active, depth) {
  const clone = Object.create(null);
  addJsonWireBytes(budget, 1);
  let member = 0;
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key === "symbol") {
      throw new TypeError(`${label} must not contain symbol keys.`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor?.enumerable || !("value" in descriptor)) {
      throw new TypeError(`${label}.${key} must be an enumerable data property.`);
    }
    if (member > 0) addJsonWireBytes(budget, 1);
    addJsonStringWireBytes(key, `${label} object key`, budget);
    addJsonWireBytes(budget, 1);
    const child = cloneJsonWireValue(
      descriptor.value,
      `${label}.${key}`,
      limits,
      budget,
      active,
      depth + 1,
    );
    Object.defineProperty(clone, key, {
      configurable: true,
      enumerable: true,
      value: child,
      writable: true,
    });
    member += 1;
  }
  addJsonWireBytes(budget, 1);
  return clone;
}

function addJsonStringWireBytes(value, label, budget) {
  addJsonWireBytes(budget, 1);
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit === 0x22 || codeUnit === 0x5c) {
      addJsonWireBytes(budget, 2);
    } else if (
      codeUnit === 0x08 ||
      codeUnit === 0x09 ||
      codeUnit === 0x0a ||
      codeUnit === 0x0c ||
      codeUnit === 0x0d
    ) {
      addJsonWireBytes(budget, 2);
    } else if (codeUnit <= 0x1f) {
      addJsonWireBytes(budget, 6);
    } else if (codeUnit <= 0x7f) {
      addJsonWireBytes(budget, 1);
    } else if (codeUnit <= 0x7ff) {
      addJsonWireBytes(budget, 2);
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const low = value.charCodeAt(index + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
        );
      }
      addJsonWireBytes(budget, 4);
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
      );
    } else {
      addJsonWireBytes(budget, 3);
    }
  }
  addJsonWireBytes(budget, 1);
}

function addJsonWireBytes(budget, bytes) {
  if (bytes > budget.max - budget.used) {
    throw new MermanInvalidTransportError(
      `Merman transport ${budget.label} exceeds the ${budget.max}-byte wire limit.`,
    );
  }
  budget.used += bytes;
}

export function assertUtf8Field(value, label, maxUtf8Bytes) {
  const bytes = strictUtf8ByteLength(value, label);
  if (bytes > maxUtf8Bytes) {
    throw new MermanInvalidTransportError(
      `Merman transport ${label} exceeds the ${maxUtf8Bytes}-byte field limit.`,
    );
  }
  return bytes;
}

export function parseRuntimeCatalogJsonText(value, label = "runtime catalog") {
  const parsed = parseTransportJsonText(
    value,
    label,
    NODE_TRANSPORT_LIMITS.runtime_catalog,
  );
  assertExactSafeIntegerJsonTokens(
    value,
    label,
    RUNTIME_CATALOG_EXACT_SAFE_INTEGER_PATHS,
  );
  return parsed;
}

export function validateTransportIdentityJson(
  value,
  { expectedTransport, expectedPackageVersion },
) {
  const identity = parseTransportJsonText(
    value,
    "identity",
    NODE_TRANSPORT_LIMITS.identity,
  );
  if (
    !isPlainJsonObject(identity) ||
    identity.schema_version !== 1 ||
    identity.package_id !== NODE_WIRE_CONTRACT.package_id ||
    identity.artifact_id !== NODE_WIRE_CONTRACT.artifact_id ||
    identity.package_version !== expectedPackageVersion ||
    identity.transport_kind !== expectedTransport ||
    identity.transport_api_version !== NODE_WIRE_CONTRACT.transport_api_version ||
    identity.binding_result_payload_version !==
      NODE_WIRE_CONTRACT.binding_result_payload_version ||
    identity.capability_descriptor_digest !== CAPABILITY_DESCRIPTOR_DIGEST ||
    !sameJsonValue(identity.wire_contract, NODE_WIRE_CONTRACT)
  ) {
    throw new MermanInvalidTransportError(
      "The Merman candidate module identity is incompatible with this loader package.",
    );
  }
  assertUtf8Field(
    identity.package_version,
    "identity package_version",
    NODE_TRANSPORT_FIELD_LIMITS.package_version_utf8_bytes,
  );
  assertUtf8Field(
    identity.capability_descriptor_digest,
    "identity capability_descriptor_digest",
    NODE_TRANSPORT_FIELD_LIMITS.contract_digest_utf8_bytes,
  );
  return identity;
}

export function validateTransportIdentityExport(
  readIdentity,
  { expectedTransport, expectedPackageVersion, label },
) {
  if (typeof readIdentity !== "function") {
    throw new MermanInvalidTransportError(
      `${label} does not export transportIdentityJson().`,
    );
  }
  let value;
  try {
    value = readIdentity();
  } catch (cause) {
    throw new MermanInvalidTransportError(
      `${label} transport identity preflight failed.`,
      cause,
    );
  }
  return validateTransportIdentityJson(value, {
    expectedPackageVersion,
    expectedTransport,
  });
}

export function strictUtf8ByteLength(value, label = "text") {
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit <= 0x7f) {
      bytes += 1;
    } else if (codeUnit <= 0x7ff) {
      bytes += 2;
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const low = value.charCodeAt(index + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
        );
      }
      bytes += 4;
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
      );
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

function scanBoundedJsonText(source, label, limits) {
  let depth = 0;
  let members = 0;
  let tokens = 0;
  const stack = [];
  for (let index = 0; index < source.length;) {
    const codeUnit = source.charCodeAt(index);
    if (isJsonWhitespaceCodeUnit(codeUnit)) {
      index += 1;
      continue;
    }
    const character = source[index];
    if (character === '"') {
      const scanned = scanJsonStringToken(source, index, label);
      if (scanned.utf8Bytes > limits.max_string_utf8_bytes) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} contains a string exceeding the ${limits.max_string_utf8_bytes}-byte field limit.`,
        );
      }
      tokens += 1;
      index = scanned.end;
    } else if (character === "{" || character === "[") {
      stack.push(character);
      depth += 1;
      tokens += 1;
      if (depth > limits.max_depth) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} exceeds the structural depth limit ${limits.max_depth}.`,
        );
      }
      index += 1;
    } else if (character === "}" || character === "]") {
      const expected = character === "}" ? "{" : "[";
      if (stack.at(-1) !== expected) break;
      stack.pop();
      depth -= 1;
      index += 1;
    } else if (character === ":") {
      members += 1;
      index += 1;
    } else if (character === "-" || (character >= "0" && character <= "9")) {
      JSON_NUMBER_TOKEN.lastIndex = index;
      const match = JSON_NUMBER_TOKEN.exec(source);
      if (match === null) break;
      if (!Number.isFinite(Number(match[0]))) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} contains a number outside the finite JSON range.`,
        );
      }
      tokens += 1;
      index = JSON_NUMBER_TOKEN.lastIndex;
    } else if (source.startsWith("true", index)) {
      tokens += 1;
      index += 4;
    } else if (source.startsWith("false", index)) {
      tokens += 1;
      index += 5;
    } else if (source.startsWith("null", index)) {
      tokens += 1;
      index += 4;
    } else {
      index += 1;
    }
    if (members > limits.max_members) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} exceeds the member-work limit ${limits.max_members}.`,
      );
    }
    if (tokens > limits.max_tokens) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} exceeds the token-work limit ${limits.max_tokens}.`,
      );
    }
  }
}

function scanJsonStringToken(source, start, label) {
  let utf8Bytes = 0;
  for (let index = start + 1; index < source.length; index += 1) {
    const codeUnit = source.charCodeAt(index);
    if (codeUnit === 0x22) return { end: index + 1, utf8Bytes };
    if (codeUnit === 0x5c) {
      const escaped = source[index + 1];
      if (escaped === "u") {
        const first = parseJsonUnicodeEscape(source, index, label);
        if (first >= 0xd800 && first <= 0xdbff) {
          if (source[index + 6] !== "\\" || source[index + 7] !== "u") {
            throw new MermanInvalidTransportError(
              `Merman transport ${label} contains an isolated JSON surrogate escape.`,
            );
          }
          const second = parseJsonUnicodeEscape(source, index + 6, label);
          if (!(second >= 0xdc00 && second <= 0xdfff)) {
            throw new MermanInvalidTransportError(
              `Merman transport ${label} contains an isolated JSON surrogate escape.`,
            );
          }
          utf8Bytes += 4;
          index += 11;
          continue;
        }
        if (first >= 0xdc00 && first <= 0xdfff) {
          throw new MermanInvalidTransportError(
            `Merman transport ${label} contains an isolated JSON surrogate escape.`,
          );
        }
        utf8Bytes += utf8LengthOfCodeUnit(first);
        index += 5;
        continue;
      }
      utf8Bytes += 1;
      index += 1;
      continue;
    }
    if (codeUnit <= 0x7f) {
      utf8Bytes += 1;
    } else if (codeUnit <= 0x7ff) {
      utf8Bytes += 2;
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const low = source.charCodeAt(index + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) {
        throw new MermanInvalidTransportError(
          `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
        );
      }
      utf8Bytes += 4;
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new MermanInvalidTransportError(
        `Merman transport ${label} contains an isolated UTF-16 surrogate.`,
      );
    } else {
      utf8Bytes += 3;
    }
  }
  return { end: source.length, utf8Bytes };
}

function parseJsonUnicodeEscape(source, slashIndex, label) {
  const digits = source.slice(slashIndex + 2, slashIndex + 6);
  if (!/^[0-9a-fA-F]{4}$/.test(digits)) {
    throw new MermanInvalidTransportError(
      `Merman transport ${label} contains an invalid JSON Unicode escape.`,
    );
  }
  return Number.parseInt(digits, 16);
}

function utf8LengthOfCodeUnit(codeUnit) {
  if (codeUnit <= 0x7f) return 1;
  if (codeUnit <= 0x7ff) return 2;
  return 3;
}

function isJsonWhitespaceCodeUnit(codeUnit) {
  return codeUnit === 0x20 || codeUnit === 0x09 || codeUnit === 0x0a || codeUnit === 0x0d;
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

export function decodeWireResponse(
  value,
  expectation,
  { allowedCancellationReasons = [], requireUnavailable = false } = {},
) {
  const cancellationReasons = validateAllowedCancellationReasons(allowedCancellationReasons);
  const envelope = parseTransportJsonText(
    value,
    "response",
    NODE_TRANSPORT_LIMITS.response,
  );
  assertExactSafeIntegerJsonTokens(
    value,
    "response",
    RESPONSE_EXACT_SAFE_INTEGER_PATHS,
  );
  validateEnvelopeHeader(envelope, "response");
  if (envelope.ok === false) {
    parseTransportJsonText(value, "error response", NODE_TRANSPORT_LIMITS.error);
    if (Object.hasOwn(envelope, "result") || !Object.hasOwn(envelope, "error")) {
      throw new MermanInvalidTransportError(
        "Merman transport error envelopes must contain error only.",
      );
    }
    const error = validateErrorPayload(
      envelope.error,
      expectation,
      requireUnavailable,
      cancellationReasons,
    );
    throw new MermanOperationError(error);
  }
  if (envelope.ok !== true || Object.hasOwn(envelope, "error")) {
    throw new MermanInvalidTransportError(
      "Merman transport success envelopes must contain result only.",
    );
  }
  if (requireUnavailable) {
    throw new MermanInvalidTransportError(
      "Merman transport executed an operation that its runtime catalog does not advertise.",
    );
  }
  return validateSuccessResult(envelope.result, expectation);
}

function validateAllowedCancellationReasons(reasons) {
  if (
    !Array.isArray(reasons) ||
    reasons.some((reason) => !CANCELLATION_REASONS.has(reason))
  ) {
    throw new TypeError("allowedCancellationReasons must contain known cancellation reasons.");
  }
  return new Set(reasons);
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
    const envelope = parseTransportJsonText(
      value,
      "thrown error",
      NODE_TRANSPORT_LIMITS.error,
    );
    validateEnvelopeHeader(envelope, "thrown error");
    if (
      envelope.ok === false &&
      !Object.hasOwn(envelope, "result") &&
      Object.hasOwn(envelope, "error")
    ) {
      return new MermanOperationError(validateErrorPayload(envelope.error));
    }
  } catch (causeError) {
    if (causeError instanceof MermanOperationError) return causeError;
    // Fall through to the transport-level error below.
  }
  return new MermanInvalidTransportError(fallbackMessage, cause);
}

function validateEnvelopeHeader(envelope, label) {
  if (!isPlainJsonObject(envelope) || envelope.version !== BINDING_RESULT_PAYLOAD_VERSION) {
    throw new MermanInvalidTransportError(
      `Merman transport returned an unsupported ${label} envelope.`,
    );
  }
  if (typeof envelope.ok !== "boolean") {
    throw new MermanInvalidTransportError(`Merman transport ${label} envelope has invalid ok.`);
  }
}

function validateSuccessResult(result, expectation) {
  if (
    !isPlainJsonObject(result) ||
    typeof result.operation_id !== "string" ||
    result.operation_id.length === 0 ||
    typeof result.media_type !== "string" ||
    result.media_type.length === 0 ||
    typeof result.data !== "string" ||
    typeof result.metadata_json !== "string"
  ) {
    throw new MermanInvalidTransportError("Merman transport returned an invalid result envelope.");
  }
  assertUtf8Field(
    result.operation_id,
    "response operation_id",
    NODE_TRANSPORT_FIELD_LIMITS.operation_id_utf8_bytes,
  );
  assertUtf8Field(
    result.media_type,
    "response media_type",
    NODE_TRANSPORT_FIELD_LIMITS.media_type_utf8_bytes,
  );
  const dataBytes = assertUtf8Field(
    result.data,
    "response data",
    NODE_TRANSPORT_FIELD_LIMITS.data_utf8_bytes,
  );
  assertUtf8Field(
    result.metadata_json,
    "response metadata_json",
    NODE_TRANSPORT_FIELD_LIMITS.metadata_json_utf8_bytes,
  );
  if (
    !expectation ||
    result.operation_id !== expectation.operation_id ||
    result.media_type !== expectation.media_type
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport result identity does not match the requested operation contract.",
    );
  }
  const metadata = parseTransportJsonText(
    result.metadata_json,
    "nested operation metadata",
    NODE_TRANSPORT_LIMITS.metadata,
  );
  assertExactSafeIntegerJsonTokens(
    result.metadata_json,
    "nested operation metadata",
    OPERATION_METADATA_EXACT_SAFE_INTEGER_PATHS,
  );
  if (
    !isPlainJsonObject(metadata) ||
    metadata.version !== expectation.metadata_schema_version ||
    metadata.operation_id !== expectation.operation_id ||
    metadata.media_type !== expectation.media_type ||
    metadata.runtime_policy !== "deterministic" ||
    !Number.isSafeInteger(metadata.byte_length) ||
    metadata.byte_length < 0 ||
    metadata.byte_length !== dataBytes
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport operation metadata does not match the result envelope.",
    );
  }
  return result;
}

function validateErrorPayload(
  error,
  expectation = null,
  requireUnavailable = false,
  allowedCancellationReasons = new Set(),
) {
  if (
    !isPlainJsonObject(error) ||
    !Number.isSafeInteger(error.code) ||
    error.code <= 0 ||
    typeof error.code_name !== "string" ||
    !/^MERMAN_[A-Z0-9_]+$/.test(error.code_name) ||
    typeof error.kind !== "string" ||
    error.kind.length === 0 ||
    !Object.hasOwn(error, "capability_id") ||
    (error.capability_id !== null &&
      (typeof error.capability_id !== "string" || error.capability_id.length === 0)) ||
    typeof error.message !== "string"
  ) {
    throw new MermanInvalidTransportError("Merman transport returned an invalid error payload.");
  }
  assertUtf8Field(
    error.code_name,
    "error code_name",
    NODE_TRANSPORT_FIELD_LIMITS.error_code_name_utf8_bytes,
  );
  assertUtf8Field(
    error.kind,
    "error kind",
    NODE_TRANSPORT_FIELD_LIMITS.error_kind_utf8_bytes,
  );
  assertUtf8Field(
    error.message,
    "error message",
    NODE_TRANSPORT_FIELD_LIMITS.error_message_utf8_bytes,
  );
  if (error.capability_id !== null) {
    assertUtf8Field(
      error.capability_id,
      "error capability_id",
      NODE_TRANSPORT_FIELD_LIMITS.capability_id_utf8_bytes,
    );
  }
  validateKnownErrorRelations(error);
  validateErrorDetails(error.details);

  if (
    error.code === 12 &&
    !allowedCancellationReasons.has(error.details.cancellation.reason)
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport returned cancellation without a matching invocation control.",
    );
  }

  if (requireUnavailable && expectation?.unavailable && error.code !== 12) {
    const unavailable = expectation.unavailable;
    if (
      error.code !== unavailable.status_code ||
      error.code_name !== unavailable.status_name ||
      error.kind !== unavailable.error_kind ||
      error.capability_id !== unavailable.capability_id
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned the wrong capability-gated operation error.",
      );
    }
  } else if (
    expectation &&
    (error.kind === "missing-capability" || error.kind === "unknown-operation")
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport contradicted its advertised operation catalog.",
    );
  }
  return error;
}

function validateKnownErrorRelations(error) {
  if (BINDING_STATUS_NAME_BY_CODE.get(error.code) !== error.code_name) {
    throw new MermanInvalidTransportError(
      "Merman transport returned an inconsistent error status name.",
    );
  }
  if (error.kind === "unknown-operation") {
    if (
      error.code !== 7 ||
      error.code_name !== "MERMAN_UNSUPPORTED_OPERATION" ||
      error.capability_id !== null
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent unknown-operation error.",
      );
    }
  } else if (error.kind === "missing-capability") {
    if (
      error.code !== 7 ||
      error.code_name !== "MERMAN_UNSUPPORTED_OPERATION" ||
      error.capability_id === null
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent missing-capability error.",
      );
    }
  } else if (error.kind === "busy") {
    if (error.code !== 11 || error.code_name !== "MERMAN_BUSY") {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent busy error.",
      );
    }
  } else if (error.kind === "reentrant-call") {
    if (
      error.code !== 1 ||
      error.code_name !== "MERMAN_INVALID_ARGUMENT" ||
      error.capability_id !== null
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent reentrant-call error.",
      );
    }
  } else if (error.kind === "generic") {
    if (error.capability_id !== null) {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent generic error.",
      );
    }
  } else {
    throw new MermanInvalidTransportError("Merman transport returned an unknown error kind.");
  }
  const cancellation = error.details?.cancellation;
  if (error.code === 12) {
    if (
      error.kind !== "generic" ||
      error.capability_id !== null ||
      !isPlainJsonObject(error.details) ||
      cancellation === undefined ||
      error.details.resource !== undefined ||
      error.details.diagnostic !== undefined ||
      error.details.icon_registry !== undefined
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned an inconsistent cancellation error.",
      );
    }
  } else if (cancellation !== undefined) {
    throw new MermanInvalidTransportError(
      "Merman transport attached cancellation details to a non-cancellation error.",
    );
  }
}

function validateErrorDetails(details) {
  if (details === undefined) return;
  if (!isPlainJsonObject(details)) {
    throw new MermanInvalidTransportError("Merman transport error details must be an object.");
  }
  if (details.resource !== undefined) {
    const resource = details.resource;
    if (
      !isPlainJsonObject(resource) ||
      typeof resource.cause !== "string" ||
      resource.cause.length === 0 ||
      typeof resource.limit_id !== "string" ||
      resource.limit_id.length === 0 ||
      typeof resource.phase !== "string" ||
      resource.phase.length === 0 ||
      typeof resource.profile !== "string" ||
      resource.profile.length === 0 ||
      !isBindingResourceCount(resource.actual) ||
      !isBindingResourceCount(resource.max)
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned invalid resource error details.",
      );
    }
  }
  if (details.diagnostic !== undefined) {
    const diagnostic = details.diagnostic;
    const span = diagnostic?.span;
    if (
      !isPlainJsonObject(diagnostic) ||
      typeof diagnostic.code !== "string" ||
      diagnostic.code.length === 0 ||
      (diagnostic.field !== null && typeof diagnostic.field !== "string") ||
      (diagnostic.diagram_type !== null && typeof diagnostic.diagram_type !== "string") ||
      !isOptionalDiagnosticUnsignedInteger(diagnostic.requested_max_width) ||
      !isOptionalDiagnosticUnsignedInteger(diagnostic.actual_width) ||
      (diagnostic.width_profile !== undefined &&
        diagnostic.width_profile !== null &&
        typeof diagnostic.width_profile !== "string") ||
      (diagnostic.fallback_reason !== undefined &&
        diagnostic.fallback_reason !== null &&
        typeof diagnostic.fallback_reason !== "string") ||
      (span !== null &&
        (!isPlainJsonObject(span) ||
          !Number.isSafeInteger(span.start) ||
          span.start < 0 ||
          !Number.isSafeInteger(span.end) ||
          span.end < span.start ||
          !["exact", "insertion-point", "fallback"].includes(span.kind)))
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned invalid diagnostic error details.",
      );
    }
  }
  if (details.cancellation !== undefined) {
    const cancellation = details.cancellation;
    if (
      !isPlainJsonObject(cancellation) ||
      !CANCELLATION_REASONS.has(cancellation.reason) ||
      typeof cancellation.phase !== "string" ||
      !OPERATION_PHASE_IDENTIFIER.test(cancellation.phase)
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned invalid cancellation error details.",
      );
    }
  }
  if (details.icon_registry !== undefined) {
    const iconRegistry = details.icon_registry;
    if (
      !isPlainJsonObject(iconRegistry) ||
      typeof iconRegistry.kind_id !== "string" ||
      iconRegistry.kind_id.length === 0 ||
      (iconRegistry.pack_index !== null &&
        (!Number.isSafeInteger(iconRegistry.pack_index) || iconRegistry.pack_index < 0)) ||
      (iconRegistry.registration_name !== null &&
        typeof iconRegistry.registration_name !== "string")
    ) {
      throw new MermanInvalidTransportError(
        "Merman transport returned invalid icon-registry error details.",
      );
    }
  }
}

function isBindingResourceCount(value) {
  if (typeof value === "number") {
    return Number.isSafeInteger(value) && value >= 0;
  }
  if (typeof value !== "string" || !CANONICAL_WIDE_UNSIGNED_DECIMAL.test(value)) {
    return false;
  }
  return compareCanonicalUnsignedDecimals(value, MAX_SAFE_INTEGER_DECIMAL) > 0 &&
    compareCanonicalUnsignedDecimals(value, U64_MAX_DECIMAL) <= 0;
}

function isOptionalDiagnosticUnsignedInteger(value) {
  return value === undefined ||
    value === null ||
    (Number.isSafeInteger(value) && value >= 0);
}

function compareCanonicalUnsignedDecimals(left, right) {
  if (left.length !== right.length) return left.length < right.length ? -1 : 1;
  if (left === right) return 0;
  return left < right ? -1 : 1;
}

function isPlainJsonObject(value) {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

export function sameJsonValue(left, right) {
  const stack = [[left, right]];
  while (stack.length > 0) {
    const [leftValue, rightValue] = stack.pop();
    if (Object.is(leftValue, rightValue)) continue;
    if (
      !leftValue ||
      !rightValue ||
      typeof leftValue !== "object" ||
      typeof rightValue !== "object" ||
      Array.isArray(leftValue) !== Array.isArray(rightValue)
    ) {
      return false;
    }
    const leftKeys = Object.keys(leftValue);
    const rightKeys = Object.keys(rightValue);
    if (
      leftKeys.length !== rightKeys.length ||
      leftKeys.some((key) => !Object.hasOwn(rightValue, key))
    ) {
      return false;
    }
    for (const key of leftKeys) stack.push([leftValue[key], rightValue[key]]);
  }
  return true;
}
