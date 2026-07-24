import { BoundedExecutor } from "./bounded-executor.mjs";
import {
  MermanInvalidTransportError,
  decodeWireResponse,
} from "./errors.mjs";

const RESOURCE_PROFILES = new Set([
  "interactive",
  "constrained",
  "trusted-native",
  "unbounded-for-trusted-input",
]);

export async function createNodeEngine(
  {
    bindingOptions = {},
    concurrency = 1,
    maxQueue = 64,
  } = {},
  { loadTransport } = {},
) {
  if (typeof loadTransport !== "function") {
    throw new MermanInvalidTransportError("A concrete Node candidate transport loader is required.");
  }
  const normalizedOptions = normalizeBindingOptions(bindingOptions);
  const transport = await loadTransport(JSON.stringify(normalizedOptions));
  assertTransport(transport);
  return new MermanNodeEngine(transport, { concurrency, maxQueue });
}

export function normalizeBindingOptions(value = {}) {
  if (!isPlainObject(value)) throw new TypeError("bindingOptions must be a plain object.");
  rejectNonWireValues(value);
  const normalized = structuredClone(value);
  normalized.version ??= 1;
  normalized.runtime_policy ??= "deterministic";
  normalized.resources ??= {};
  if (!isPlainObject(normalized.resources)) {
    throw new TypeError("bindingOptions.resources must be a plain object.");
  }
  normalized.resources.profile ??= "interactive";
  if (!RESOURCE_PROFILES.has(normalized.resources.profile)) {
    throw new RangeError(`Unknown resource profile \`${normalized.resources.profile}\`.`);
  }
  return normalized;
}

export class MermanNodeEngine {
  #disposePromise = null;
  #executor;
  #transport;

  constructor(transport, queueOptions) {
    this.#transport = transport;
    this.#executor = new BoundedExecutor(queueOptions);
  }

  get queueState() {
    return this.#executor.snapshot;
  }

  renderSvg(source, options = {}) {
    return this.executeOperation(
      { operationId: "svg", source },
      { signal: options.signal },
    ).then((result) => result.data);
  }

  renderSvgSync(source) {
    return this.executeOperationSync({ operationId: "svg", source }).data;
  }

  executeOperation(request, { signal } = {}) {
    const requestJson = operationRequestJson(request);
    return this.#executor.submit(
      async () => decodeWireResponse(await this.#transport.execute(requestJson)),
      { signal },
    );
  }

  executeOperationSync(request) {
    this.#executor.assertSyncAvailable();
    return decodeWireResponse(this.#transport.executeSync(operationRequestJson(request)));
  }

  dispose() {
    if (this.#disposePromise) return this.#disposePromise;
    this.#disposePromise = this.#executor.dispose().then(async () => {
      await this.#transport.dispose?.();
    });
    return this.#disposePromise;
  }
}

function operationRequestJson({ operationId, source, uri = null }) {
  if (typeof operationId !== "string" || operationId.length === 0) {
    throw new TypeError("operationId must be a non-empty string.");
  }
  if (typeof source !== "string") throw new TypeError("source must be a string.");
  if (uri !== null && typeof uri !== "string") throw new TypeError("uri must be a string or null.");
  return JSON.stringify({
    operation_id: operationId,
    source,
    uri,
  });
}

function assertTransport(transport) {
  if (
    !transport ||
    typeof transport.execute !== "function" ||
    typeof transport.executeSync !== "function"
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport must provide execute() and executeSync().",
    );
  }
}

function rejectNonWireValues(value) {
  if (typeof value === "function") {
    throw new TypeError("JavaScript text measurement callbacks are not supported by @mermanjs/node.");
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (/text.*measur|measur.*text|font.*callback/i.test(key)) {
      throw new TypeError("JavaScript text measurement callbacks are not supported by @mermanjs/node.");
    }
    rejectNonWireValues(child);
  }
}

function isPlainObject(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
