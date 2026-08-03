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
  if (!envelope || typeof envelope !== "object" || envelope.version !== 1) {
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
      envelope.version === 1 &&
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
