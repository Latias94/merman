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
  const runtimeCatalog = validateRuntimeCatalog(await transport.runtimeCatalogJson());
  return new MermanNodeEngine(transport, runtimeCatalog, { concurrency, maxQueue });
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
  #runtimeCatalog;
  #transport;

  constructor(transport, runtimeCatalog, queueOptions) {
    this.#transport = transport;
    this.#runtimeCatalog = runtimeCatalog;
    this.#executor = new BoundedExecutor(queueOptions);
  }

  get queueState() {
    return this.#executor.snapshot;
  }

  get runtimeCatalog() {
    return structuredClone(this.#runtimeCatalog);
  }

  renderSvg(source, options = {}) {
    return this.executeOperation(
      { operationId: "svg", source, optionsJson: options.optionsJson },
      { signal: options.signal },
    ).then((result) => result.data);
  }

  renderSvgSync(source, options = {}) {
    return this.executeOperationSync({
      operationId: "svg",
      source,
      optionsJson: options.optionsJson,
    }).data;
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

function operationRequestJson(value) {
  if (!isPlainObject(value)) throw new TypeError("operation request must be a plain object.");
  const knownFields = new Set(["operationId", "source", "uri", "optionsJson"]);
  for (const field of Object.keys(value)) {
    if (!knownFields.has(field)) {
      throw new TypeError(`unknown operation request field \`${field}\`.`);
    }
  }
  const {
    operationId,
    source,
    uri = null,
    optionsJson = undefined,
  } = value;
  if (typeof operationId !== "string" || operationId.length === 0) {
    throw new TypeError("operationId must be a non-empty string.");
  }
  if (typeof source !== "string") throw new TypeError("source must be a string.");
  if (uri !== null && typeof uri !== "string") throw new TypeError("uri must be a string or null.");
  const request = {
    operation_id: operationId,
    source,
    uri,
  };
  if (optionsJson !== undefined) {
    if (typeof optionsJson !== "string") {
      throw new TypeError("optionsJson must be a JSON string when provided.");
    }
    request.options_json = optionsJson;
  }
  return JSON.stringify(request);
}

function assertTransport(transport) {
  if (
    !transport ||
    typeof transport.execute !== "function" ||
    typeof transport.executeSync !== "function" ||
    typeof transport.runtimeCatalogJson !== "function"
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport must provide runtimeCatalogJson(), execute(), and executeSync().",
    );
  }
}

export function validateRuntimeCatalog(value) {
  let catalog;
  try {
    catalog = typeof value === "string" ? JSON.parse(value) : value;
  } catch (cause) {
    throw new MermanInvalidTransportError("Merman transport returned invalid runtime catalog JSON.", cause);
  }
  if (!isPlainObject(catalog)) {
    throw new MermanInvalidTransportError("Merman transport returned an invalid runtime catalog.");
  }
  if (catalog.schema_version !== 1 || catalog.transport_api_version !== 1) {
    throw new MermanInvalidTransportError("Merman transport returned an unsupported runtime catalog version.");
  }
  if (typeof catalog.package_version !== "string" || catalog.package_version.length === 0) {
    throw new MermanInvalidTransportError("Merman runtime catalog has an invalid package version.");
  }

  const capabilities = catalog.capabilities;
  const registry = catalog.registry;
  const resources = catalog.resources;
  if (!isPlainObject(capabilities) || !isPlainObject(registry) || !isPlainObject(resources)) {
    throw new MermanInvalidTransportError("Merman runtime catalog has invalid required sections.");
  }
  const capabilityIds = sortedUniqueStrings(capabilities.capability_ids, "capability_ids");
  const outputIds = sortedUniqueStrings(capabilities.output_ids, "output_ids");
  const operationIds = sortedUniqueStrings(capabilities.operation_ids, "operation_ids");
  const systemAdapterIds = sortedUniqueStrings(
    capabilities.system_adapter_ids,
    "system_adapter_ids",
  );
  if (
    !outputIds.every((id) => operationIds.includes(id)) ||
    !systemAdapterIds.every((id) => capabilityIds.includes(id))
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog contains invalid capability relations.",
    );
  }
  validateTextMeasurement(capabilities.text_measurement, capabilityIds.includes("svg"));
  validateRegistry(registry);
  validateResources(resources);
  return structuredClone(catalog);
}

function validateTextMeasurement(value, hasSvg) {
  if (!hasSvg) {
    if (value !== null) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog reports text measurement without the SVG capability.",
      );
    }
    return;
  }
  if (!isPlainObject(value) || !positiveSafeInteger(value.protocol_version)) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid text measurement metadata.",
    );
  }
  const providerIds = sortedUniqueStrings(
    value.provider_ids,
    "text_measurement.provider_ids",
  );
  if (providerIds.length !== 1 || providerIds[0] !== "vendored") {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog must expose only the callable vendored text measurement provider.",
    );
  }
}

function validateRegistry(value) {
  if (
    !Number.isSafeInteger(value.diagram_family_count) ||
    value.diagram_family_count < 0
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid registry metadata.",
    );
  }
}

function validateResources(value) {
  if (
    typeof value.general_binding_default_profile !== "string" ||
    value.general_binding_default_profile.length === 0 ||
    typeof value.cli_default_profile !== "string" ||
    value.cli_default_profile.length === 0 ||
    !Array.isArray(value.limits) ||
    value.limits.length === 0 ||
    !Array.isArray(value.profiles) ||
    value.profiles.length === 0
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid resource metadata.",
    );
  }
  const limitIds = new Set();
  for (const limit of value.limits) {
    if (
      !isPlainObject(limit) ||
      typeof limit.id !== "string" ||
      limit.id.length === 0 ||
      limitIds.has(limit.id) ||
      typeof limit.phase !== "string" ||
      limit.phase.length === 0 ||
      typeof limit.description !== "string" ||
      limit.description.length === 0 ||
      typeof limit.overridable !== "boolean" ||
      typeof limit.hard_cap !== "boolean"
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid resource limit.",
      );
    }
    limitIds.add(limit.id);
  }
  const profileIds = new Set();
  for (const profile of value.profiles) {
    if (
      !isPlainObject(profile) ||
      typeof profile.id !== "string" ||
      profile.id.length === 0 ||
      profileIds.has(profile.id) ||
      typeof profile.purpose !== "string" ||
      profile.purpose.length === 0 ||
      typeof profile.trust_assumption !== "string" ||
      profile.trust_assumption.length === 0 ||
      typeof profile.recommended_binding_default !== "boolean" ||
      !isPlainObject(profile.limits)
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid resource profile.",
      );
    }
    const profileLimitIds = Object.keys(profile.limits);
    if (!sameStringSet(profileLimitIds, limitIds)) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog resource profile does not cover the declared limits.",
      );
    }
    for (const limit of Object.values(profile.limits)) {
      if (limit !== null && (!Number.isSafeInteger(limit) || limit < 0)) {
        throw new MermanInvalidTransportError(
          "Merman runtime catalog resource profile has an invalid limit.",
        );
      }
    }
    profileIds.add(profile.id);
  }
  if (
    !profileIds.has(value.general_binding_default_profile) ||
    !profileIds.has(value.cli_default_profile)
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog resource defaults do not name declared profiles.",
    );
  }
  const generalDefault = value.profiles.find(
    (profile) => profile.id === value.general_binding_default_profile,
  );
  if (generalDefault.recommended_binding_default !== true) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog general binding default is not recommended for bindings.",
    );
  }
}

function sameStringSet(values, expected) {
  return values.length === expected.size && values.every((value) => expected.has(value));
}

function positiveSafeInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function sortedUniqueStrings(value, field) {
  if (
    !Array.isArray(value) ||
    value.some((item) => typeof item !== "string" || item.length === 0) ||
    value.some((item, index) => index > 0 && value[index - 1] >= item)
  ) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog ${field} must contain sorted unique non-empty strings.`,
    );
  }
  return value;
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
