import { BoundedExecutor } from "./bounded-executor.mjs";
import {
  MermanInvalidTransportError,
  NODE_TRANSPORT_FIELD_LIMITS,
  NODE_TRANSPORT_LIMITS,
  NODE_WIRE_CONTRACT,
  abortError,
  assertUtf8Field,
  decodeWireInvocationError,
  decodeWireResponse,
  encodeTransportJson,
  parseRuntimeCatalogJsonText,
  parseTransportJsonText,
  sameJsonValue,
} from "./errors.mjs";
import {
  BINDING_OPTION_GROUP_SPECS,
  BINDING_OPERATION_EXPECTATIONS,
  BINDING_OPTIONS_SCHEMA_VERSION,
  BINDING_PAYLOAD_SCHEMAS,
  BINDING_TRANSPORT_EXPOSURE_SPECS,
  CAPABILITY_SPECS,
  CONSTRUCTOR_SERVICE_SPECS,
  METADATA_SPECS,
  RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN,
  RUNTIME_CATALOG_IDENTIFIER_PATTERN,
  RUNTIME_CATALOG_MAX_SAFE_INTEGER,
  RUNTIME_CATALOG_SCHEMA_VERSION,
  TEXT_MEASUREMENT_PROTOCOL_VERSION,
  TEXT_MEASUREMENT_PROVIDER_IDS,
  TEXT_MEASUREMENT_PROVIDER_SPECS,
  VENDORED_TEXT_MEASUREMENT_PROVIDER_ID,
} from "./generated/binding-contract.mjs";

const RESOURCE_PROFILES = new Set([
  "interactive",
  "constrained",
  "trusted-native",
  "unbounded-for-trusted-input",
]);
const RUNTIME_CATALOG_IDENTIFIER = new RegExp(RUNTIME_CATALOG_IDENTIFIER_PATTERN);
const RUNTIME_CATALOG_FIELD_IDENTIFIER = new RegExp(
  RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN,
);
const NODE_TRANSPORT_EXPOSURE = BINDING_TRANSPORT_EXPOSURE_SPECS.find(
  (spec) => spec.id === "node",
);
if (NODE_TRANSPORT_EXPOSURE === undefined) {
  throw new Error("Generated binding contract is missing the Node transport exposure.");
}
const PAYLOAD_SCHEMA_SPEC_BY_ID = new Map(
  BINDING_PAYLOAD_SCHEMAS.map((spec) => [spec.id, spec]),
);
const CAPABILITY_SPEC_BY_ID = new Map(CAPABILITY_SPECS.map((spec) => [spec.id, spec]));
const REQUIRED_PAYLOAD_SCHEMAS = new Map(
  NODE_TRANSPORT_EXPOSURE.payload_schema_ids.map((id) => [
    id,
    PAYLOAD_SCHEMA_SPEC_BY_ID.get(id).version,
  ]),
);
const OPTION_GROUP_SPEC_BY_ID = new Map(
  BINDING_OPTION_GROUP_SPECS.map((spec) => [spec.id, spec]),
);
const METADATA_SPEC_BY_ID = new Map(METADATA_SPECS.map((spec) => [spec.id, spec]));
const OPERATION_EXPECTATION_BY_ID = new Map(
  BINDING_OPERATION_EXPECTATIONS.map((expectation) => [
    expectation.operation_id,
    expectation,
  ]),
);
const CONSTRUCTOR_SERVICE_SPEC_BY_ID = new Map(
  CONSTRUCTOR_SERVICE_SPECS.map((spec) => [spec.id, spec]),
);
const NODE_CONSTRUCTOR_SERVICE_CANDIDATE_IDS = new Set(
  NODE_TRANSPORT_EXPOSURE.constructor_service_candidate_ids,
);
const TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID = new Map(
  TEXT_MEASUREMENT_PROVIDER_SPECS.map((spec) => [spec.id, spec]),
);
const KNOWN_TEXT_MEASUREMENT_PROVIDER_IDS = new Set(TEXT_MEASUREMENT_PROVIDER_IDS);
const COMPILED_PREREQUISITES_BY_OPERATION_ID = new Map(
  BINDING_OPERATION_EXPECTATIONS.map(({ operation_id, compiled_prerequisite_ids }) => [
    operation_id,
    compiled_prerequisite_ids,
  ]),
);
const KNOWN_OUTPUT_CONTRACT_BY_ID = new Map(
  NODE_WIRE_CONTRACT.artifact.output_contracts.map((contract) => [contract.id, contract]),
);
const MAX_OPERATION_TIMEOUT_MS = 0xffff_ffff;
const MERMAN_ENGINE_CONSTRUCTION_TOKEN = Symbol("MermanEngine construction");

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
  const optionsJson = encodeTransportJson(
    normalizedOptions,
    "binding options",
    NODE_TRANSPORT_LIMITS.binding_options,
  );
  const transport = await loadTransport(optionsJson);
  try {
    assertTransport(transport);
    const runtimeCatalog = validateRuntimeCatalog(await transport.runtimeCatalogJson());
    return new MermanEngine(
      MERMAN_ENGINE_CONSTRUCTION_TOKEN,
      transport,
      runtimeCatalog,
      { concurrency, maxQueue },
    );
  } catch (error) {
    await disposeUnusableTransport(transport);
    throw error;
  }
}

export function normalizeBindingOptions(value = {}) {
  if (!isPlainObject(value)) throw new TypeError("bindingOptions must be a plain object.");
  const normalized = cloneBoundedJsonValue(value, "bindingOptions");
  normalized.version ??= BINDING_OPTIONS_SCHEMA_VERSION;
  if (normalized.version !== BINDING_OPTIONS_SCHEMA_VERSION) {
    throw new RangeError(
      `Unsupported binding options schema version \`${normalized.version}\`; expected ${BINDING_OPTIONS_SCHEMA_VERSION}.`,
    );
  }
  normalized.runtime_policy ??= "deterministic";
  if (normalized.runtime_policy !== "deterministic") {
    throw new RangeError(
      "bindingOptions.runtime_policy must be `deterministic` for @mermanjs/node.",
    );
  }
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

export class MermanEngine {
  #disposePromise = null;
  #executor;
  #metadataIds;
  #operationIds;
  #runtimeCatalog;
  #transport;

  constructor(constructionToken, transport, runtimeCatalog, queueOptions) {
    if (constructionToken !== MERMAN_ENGINE_CONSTRUCTION_TOKEN) {
      throw new MermanInvalidTransportError(
        "MermanEngine instances must be created with createNodeEngine().",
      );
    }
    this.#transport = transport;
    this.#runtimeCatalog = runtimeCatalog;
    this.#metadataIds = new Set(
      runtimeCatalog.metadata_ids.filter((id) => METADATA_SPEC_BY_ID.has(id)),
    );
    this.#operationIds = new Set(runtimeCatalog.capabilities.operation_ids);
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
      { signal: options.signal, timeoutMs: options.timeoutMs },
    ).then((result) => result.data);
  }

  renderSvgSync(source, options = {}) {
    return this.executeOperationSync(
      {
        operationId: "svg",
        source,
        optionsJson: options.optionsJson,
      },
      { timeoutMs: options.timeoutMs },
    ).data;
  }

  svgPlanJson(source, options = {}) {
    return this.executeOperation(
      { operationId: "svg-plan-json", source, optionsJson: options.optionsJson },
      { signal: options.signal, timeoutMs: options.timeoutMs },
    ).then((result) => result.data);
  }

  svgPlanJsonSync(source, options = {}) {
    return this.executeOperationSync(
      {
        operationId: "svg-plan-json",
        source,
        optionsJson: options.optionsJson,
      },
      { timeoutMs: options.timeoutMs },
    ).data;
  }

  metadataJson(id) {
    this.#executor.assertOpen();
    if (typeof id !== "string" || id.length === 0) {
      throw new TypeError("metadata id must be a non-empty string.");
    }
    if (!this.#metadataIds.has(id)) {
      throw new RangeError(
        `metadata id \`${id}\` is not callable through this SDK and artifact contract.`,
      );
    }
    let value;
    try {
      value = this.#transport.metadataJson(id);
    } catch (cause) {
      throw decodeWireInvocationError(cause, `Merman metadata \`${id}\``);
    }
    parseTransportJsonText(
      value,
      `metadata \`${id}\``,
      NODE_TRANSPORT_LIMITS.metadata,
    );
    return value;
  }

  executeOperation(request, { signal, timeoutMs } = {}) {
    try {
      this.#executor.assertOpen();
    } catch (error) {
      return Promise.reject(error);
    }
    const prepared = prepareOperationRequest(request, { timeoutMs });
    if (signal?.aborted) return Promise.reject(abortError());
    const encoded = operationRequestJson(prepared);
    return this.#executor.submit(
      async () => {
        // A signal can flip after queue admission but before this start microtask runs. That work
        // has not crossed the transport boundary, so preserve the established AbortError result.
        if (signal?.aborted) throw abortError();
        let responseJson;
        try {
          responseJson = await executeTransportOperation(
            this.#transport,
            encoded.json,
            signal,
            timeoutMs,
          );
        } catch (cause) {
          throw decodeWireInvocationError(cause, "Merman operation");
        }
        return decodeWireResponse(responseJson, encoded.expectation, {
          allowedCancellationReasons: invocationCancellationReasons(signal, timeoutMs),
          requireUnavailable: !this.#operationIds.has(encoded.expectation.operation_id),
        });
      },
      { signal },
    );
  }

  executeOperationSync(request, { timeoutMs } = {}) {
    this.#executor.assertSyncAvailable();
    const encoded = operationRequestJson(prepareOperationRequest(request, { timeoutMs }));
    let responseJson;
    try {
      responseJson = this.#transport.executeSync(encoded.json, timeoutMs);
    } catch (cause) {
      throw decodeWireInvocationError(cause, "Merman operation");
    }
    return decodeWireResponse(responseJson, encoded.expectation, {
      allowedCancellationReasons: invocationCancellationReasons(undefined, timeoutMs),
      requireUnavailable: !this.#operationIds.has(encoded.expectation.operation_id),
    });
  }

  dispose() {
    if (this.#disposePromise) return this.#disposePromise;
    this.#disposePromise = this.#executor.dispose().then(async () => {
      await this.#transport.dispose?.();
    });
    return this.#disposePromise;
  }
}

function invocationCancellationReasons(signal, timeoutMs) {
  const reasons = [];
  if (signal?.aborted) reasons.push("requested");
  if (timeoutMs !== undefined) reasons.push("deadline_exceeded");
  return reasons;
}

function prepareOperationRequest(value, { timeoutMs } = {}) {
  if (!isPlainObject(value)) throw new TypeError("operation request must be a plain object.");
  const knownFields = new Set(["operationId", "source", "uri", "optionsJson"]);
  for (const field of Object.keys(value)) {
    if (!knownFields.has(field)) {
      throw new TypeError(`unknown operation request field \`${field}\`.`);
    }
  }
  const operationId = value.operationId;
  const source = value.source;
  const uri = value.uri === undefined ? null : value.uri;
  if (typeof operationId !== "string" || operationId.length === 0) {
    throw new TypeError("operationId must be a non-empty string.");
  }
  if (!OPERATION_EXPECTATION_BY_ID.has(operationId)) {
    throw new RangeError(
      `operation id \`${operationId}\` is not callable through this SDK version.`,
    );
  }
  const expectation = OPERATION_EXPECTATION_BY_ID.get(operationId);
  if (typeof source !== "string") throw new TypeError("source must be a string.");
  if (uri !== null && typeof uri !== "string") throw new TypeError("uri must be a string or null.");
  const normalizedUri = uri === "" ? null : uri;
  if (expectation.requires_uri && normalizedUri === null) {
    throw new TypeError(`operation \`${operationId}\` requires a non-empty uri.`);
  }
  if (!expectation.requires_uri && normalizedUri !== null) {
    throw new TypeError(`operation \`${operationId}\` does not accept a uri.`);
  }
  assertUtf8Field(
    operationId,
    "operation request operationId",
    NODE_TRANSPORT_FIELD_LIMITS.operation_id_utf8_bytes,
  );
  assertUtf8Field(
    source,
    "operation request source",
    NODE_TRANSPORT_FIELD_LIMITS.source_utf8_bytes,
  );
  if (normalizedUri !== null) {
    assertUtf8Field(
      normalizedUri,
      "operation request uri",
      NODE_TRANSPORT_FIELD_LIMITS.uri_utf8_bytes,
    );
  }
  const request = {
    operation_id: operationId,
    source,
    uri: normalizedUri,
  };
  if (timeoutMs !== undefined) {
    if (
      !Number.isSafeInteger(timeoutMs) ||
      timeoutMs < 0 ||
      timeoutMs > MAX_OPERATION_TIMEOUT_MS
    ) {
      throw new RangeError(
        `operation timeoutMs must be an integer from 0 through ${MAX_OPERATION_TIMEOUT_MS}.`,
      );
    }
    request.operation_control = { timeout_ms: timeoutMs };
  }
  return { expectation, request, value };
}

function operationRequestJson({ expectation, request, value }) {
  const optionsJson = value.optionsJson;
  if (optionsJson !== undefined) {
    if (typeof optionsJson !== "string") {
      throw new TypeError("optionsJson must be a JSON string when provided.");
    }
    assertUtf8Field(
      optionsJson,
      "operation request optionsJson",
      NODE_TRANSPORT_FIELD_LIMITS.options_json_utf8_bytes,
    );
    parseTransportJsonText(
      optionsJson,
      "operation request optionsJson",
      NODE_TRANSPORT_LIMITS.binding_options,
    );
    request.options_json = optionsJson;
  }
  return {
    expectation,
    json: encodeTransportJson(request, "operation request", NODE_TRANSPORT_LIMITS.request),
  };
}

async function executeTransportOperation(transport, requestJson, signal, timeoutMs) {
  if (!signal) return transport.execute(requestJson, undefined, timeoutMs);

  // Forward into a private signal so the N-API bridge cannot replace a caller-owned `onabort`
  // handler while it installs its cooperative cancellation callback.
  const transportAbort = new AbortController();
  let transportReady = false;
  let pendingAbort = false;
  const forwardAbort = () => {
    if (transportReady) {
      transportAbort.abort(signal.reason);
    } else {
      pendingAbort = true;
    }
  };
  signal.addEventListener("abort", forwardAbort, { once: true });
  try {
    const operation = transport.execute(requestJson, transportAbort.signal, timeoutMs);
    transportReady = true;
    if (pendingAbort || signal.aborted) transportAbort.abort(signal.reason);
    return await operation;
  } finally {
    signal.removeEventListener("abort", forwardAbort);
  }
}

function assertTransport(transport) {
  if (
    !transport ||
    typeof transport.execute !== "function" ||
    typeof transport.executeSync !== "function" ||
    typeof transport.runtimeCatalogJson !== "function" ||
    typeof transport.metadataJson !== "function"
  ) {
    throw new MermanInvalidTransportError(
      "Merman transport must provide runtimeCatalogJson(), metadataJson(), execute(), and executeSync().",
    );
  }
}

async function disposeUnusableTransport(transport) {
  try {
    await transport?.dispose?.();
  } catch {
    // Preserve the construction failure that made the transport unusable.
  }
}

export function validateRuntimeCatalog(value) {
  if (typeof value !== "string") {
    throw new MermanInvalidTransportError(
      "Merman transport runtime catalog must be JSON text.",
    );
  }
  const catalog = parseRuntimeCatalogJsonText(value);
  if (!isPlainObject(catalog)) {
    throw new MermanInvalidTransportError("Merman transport returned an invalid runtime catalog.");
  }
  if (
    catalog.schema_version !== RUNTIME_CATALOG_SCHEMA_VERSION ||
    catalog.transport_api_version !== 1
  ) {
    throw new MermanInvalidTransportError("Merman transport returned an unsupported runtime catalog version.");
  }
  if (typeof catalog.package_version !== "string" || catalog.package_version.length === 0) {
    throw new MermanInvalidTransportError("Merman runtime catalog has an invalid package version.");
  }
  assertUtf8Field(
    catalog.package_version,
    "runtime catalog package_version",
    NODE_TRANSPORT_FIELD_LIMITS.package_version_utf8_bytes,
  );
  const optionsSchemaVersions = sortedUniquePositiveIntegers(
    catalog.options_schema_versions,
    "options_schema_versions",
  );
  if (!optionsSchemaVersions.includes(BINDING_OPTIONS_SCHEMA_VERSION)) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog does not advertise options schema ${BINDING_OPTIONS_SCHEMA_VERSION}.`,
    );
  }
  validatePayloadSchemas(catalog.payload_schemas);

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
  if (!systemAdapterIds.every((id) => capabilityIds.includes(id))) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog contains invalid capability relations.",
    );
  }
  validateCapabilityImplications(capabilityIds);
  validateOperationRelations({ capabilityIds, operationIds, outputIds });
  const metadataIds = validateMetadataIds(catalog.metadata_ids, new Set(capabilityIds));
  validateKnownArtifactIdentity({
    capabilityIds,
    metadataIds,
    operationIds,
    outputIds,
    systemAdapterIds,
  });
  const requiresSvgPipeline = capabilityIds.includes("svg") || operationIds.some((id) =>
    COMPILED_PREREQUISITES_BY_OPERATION_ID.get(id)?.includes("svg")
  );
  const textMeasurementProviderIds = validateTextMeasurement(capabilities.text_measurement, {
    requiresSvgPipeline,
  });
  validateKnownIds(
    textMeasurementProviderIds,
    NODE_WIRE_CONTRACT.artifact.text_measurement_provider_ids,
    (id) => TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.has(id),
    "text measurement providers",
  );
  // A valid provider record may describe a future operation whose prerequisite is unknown to this
  // SDK. Preserve that additive discovery while requiring the known SVG closure when recognized.
  const usesSvgPipeline = requiresSvgPipeline || capabilities.text_measurement !== null;
  const optionGroupIds = validateOptionGroupIds(catalog, {
    capabilityIds: new Set(capabilityIds),
    usesSvgPipeline,
  });
  const constructorServices = validateConstructorServices(catalog, {
    textMeasurementProviderIds: new Set(textMeasurementProviderIds),
    usesSvgPipeline,
  });
  validateOutputContracts(catalog.output_contracts, outputIds);
  validateRegistry(registry);
  validateResources(resources, new Set(operationIds));
  const normalized = structuredClone(catalog);
  normalized.option_group_ids = optionGroupIds;
  normalized.constructor_service_ids = constructorServices.ids;
  normalized.constructor_service_contracts = constructorServices.contracts;
  return normalized;
}

function validateKnownArtifactIdentity({
  capabilityIds,
  metadataIds,
  operationIds,
  outputIds,
  systemAdapterIds,
}) {
  const expected = NODE_WIRE_CONTRACT.artifact;
  validateKnownIds(
    capabilityIds,
    expected.capability_ids,
    (id) => CAPABILITY_SPEC_BY_ID.has(id),
    "capabilities",
  );
  validateKnownIds(
    operationIds,
    expected.operation_ids,
    (id) => OPERATION_EXPECTATION_BY_ID.has(id),
    "operations",
  );
  validateKnownIds(
    metadataIds,
    expected.metadata_ids,
    (id) => METADATA_SPEC_BY_ID.has(id),
    "metadata IDs",
  );
  const knownOutputIds = new Set(
    BINDING_OPERATION_EXPECTATIONS
      .map((expectation) => expectation.output_id)
      .filter((id) => id !== null),
  );
  validateKnownIds(
    outputIds,
    expected.output_ids,
    (id) => knownOutputIds.has(id),
    "outputs",
  );
  validateKnownIds(
    systemAdapterIds,
    expected.system_adapter_ids,
    (id) => CAPABILITY_SPEC_BY_ID.has(id),
    "system adapters",
  );
}

function validateKnownIds(actual, expected, isKnown, label) {
  const actualKnown = actual.filter(isKnown);
  if (!sameOrderedStrings(actualKnown, expected)) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog known ${label} do not match the Node artifact contract.`,
    );
  }
}

function validatePayloadSchemas(value) {
  if (!Array.isArray(value)) {
    throw new MermanInvalidTransportError("Merman runtime catalog has invalid payload schemas.");
  }
  const ids = [];
  for (const schema of value) {
    if (
      !isPlainObject(schema) ||
      typeof schema.id !== "string" ||
      schema.id.length === 0 ||
      !positiveSafeInteger(schema.version)
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid payload schema.",
      );
    }
    ids.push(schema.id);
  }
  sortedUniqueStrings(ids, "payload_schemas.id");
  const actualKnownIds = ids.filter((id) => PAYLOAD_SCHEMA_SPEC_BY_ID.has(id));
  if (!sameStringSet(actualKnownIds, new Set(REQUIRED_PAYLOAD_SCHEMAS.keys()))) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog payload schemas do not match the Node transport exposure.",
    );
  }
  for (const [id, version] of REQUIRED_PAYLOAD_SCHEMAS) {
    const schema = value.find((candidate) => candidate.id === id);
    if (!schema || schema.version !== version) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog must advertise payload schema ${id} version ${version}.`,
      );
    }
  }
}

function validateOperationRelations({ capabilityIds, operationIds, outputIds }) {
  const capabilityIdSet = new Set(capabilityIds);
  const outputIdSet = new Set(outputIds);
  for (const id of operationIds) {
    const expectation = OPERATION_EXPECTATION_BY_ID.get(id);
    if (!expectation) continue;
    if (
      expectation.availability_capability_id !== null &&
      !capabilityIdSet.has(expectation.availability_capability_id)
    ) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog operation ${id} is missing its advertised capability.`,
      );
    }
    if (expectation.output_id !== null && !outputIdSet.has(expectation.output_id)) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog operation ${id} is missing its output contract.`,
      );
    }
  }
}

function validateCapabilityImplications(capabilityIds) {
  const capabilityIdSet = new Set(capabilityIds);
  for (const capabilityId of capabilityIds) {
    const spec = CAPABILITY_SPEC_BY_ID.get(capabilityId);
    if (spec === undefined) continue;
    for (const implicationId of spec.implication_ids) {
      if (!capabilityIdSet.has(implicationId)) {
        throw new MermanInvalidTransportError(
          `Merman runtime capability ${capabilityId} is missing implied capability ${implicationId}.`,
        );
      }
    }
  }
}

function validateMetadataIds(value, capabilityIds) {
  const ids = sortedUniqueStrings(value, "metadata_ids");
  for (const id of ids) {
    const spec = METADATA_SPEC_BY_ID.get(id);
    if (
      spec?.required_capability_id !== null &&
      spec?.required_capability_id !== undefined &&
      !capabilityIds.has(spec.required_capability_id)
    ) {
      throw new MermanInvalidTransportError(
        `Merman runtime metadata ${id} requires capability ${spec.required_capability_id}.`,
      );
    }
  }
  return ids;
}

function validateOptionGroupIds(catalog, { capabilityIds, usesSvgPipeline }) {
  if (!Object.hasOwn(catalog, "option_group_ids")) return [];
  const ids = sortedUniqueFieldIdentifiers(catalog.option_group_ids, "option_group_ids");
  const applicable = new Set();
  for (const spec of BINDING_OPTION_GROUP_SPECS) {
    if (
      spec.always_available ||
      (spec.requires_svg_pipeline && usesSvgPipeline) ||
      spec.any_capability_ids.some((id) => capabilityIds.has(id))
    ) {
      applicable.add(spec.id);
    }
  }

  const knownIds = ids.filter((id) => OPTION_GROUP_SPEC_BY_ID.has(id));
  if (
    !knownIds.every((id) => applicable.has(id)) ||
    !sameOrderedStrings(knownIds, NODE_WIRE_CONTRACT.artifact.option_group_ids)
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog option groups do not match the artifact capability closure.",
    );
  }
  return ids;
}

function validateConstructorServices(catalog, {
  textMeasurementProviderIds,
  usesSvgPipeline,
}) {
  const hasIds = Object.hasOwn(catalog, "constructor_service_ids");
  const hasContracts = Object.hasOwn(catalog, "constructor_service_contracts");
  if (!hasIds && !hasContracts) return { ids: [], contracts: [] };
  if (!hasIds || !hasContracts) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog must advertise constructor service IDs and contracts together.",
    );
  }
  const ids = sortedUniqueStrings(
    catalog.constructor_service_ids,
    "constructor_service_ids",
  );
  validateKnownIds(
    ids,
    NODE_WIRE_CONTRACT.artifact.constructor_service_ids,
    (id) => CONSTRUCTOR_SERVICE_SPEC_BY_ID.has(id),
    "constructor services",
  );
  const contracts = catalog.constructor_service_contracts;
  if (!Array.isArray(contracts)) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid constructor service contracts.",
    );
  }
  const contractIds = [];
  const providerOwners = new Map();
  for (const contract of contracts) {
    if (
      !isPlainObject(contract) ||
      typeof contract.id !== "string" ||
      contract.id.length === 0 ||
      !Array.isArray(contract.resource_limits)
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid constructor service contract.",
      );
    }
    const providerIds = sortedUniqueStrings(
      contract.provided_text_measurement_provider_ids,
      `constructor_service_contracts.${contract.id}.provided_text_measurement_provider_ids`,
    );
    validateConstructorResourceLimits(contract.resource_limits, contract.id);
    for (const providerId of providerIds) {
      if (providerOwners.has(providerId)) {
        throw new MermanInvalidTransportError(
          "Merman runtime catalog text measurement providers must have one constructor service owner.",
        );
      }
      providerOwners.set(providerId, contract.id);
      if (!textMeasurementProviderIds.has(providerId)) {
        throw new MermanInvalidTransportError(
          "Merman runtime catalog constructor services name an unavailable text measurement provider.",
        );
      }
      const providerSpec = TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.get(providerId);
      if (
        providerSpec !== undefined &&
        (providerSpec.source !== "constructor-service" ||
          providerSpec.constructor_service_id !== contract.id)
      ) {
        throw new MermanInvalidTransportError(
          "Merman runtime catalog constructor service provider ownership is inconsistent.",
        );
      }
    }
    const knownSpec = CONSTRUCTOR_SERVICE_SPEC_BY_ID.get(contract.id);
    if (knownSpec?.requires_svg_pipeline && !usesSvgPipeline) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog advertises a constructor service without an SVG pipeline.",
      );
    }
    if (
      knownSpec !== undefined &&
      !sameStringSet(
        providerIds,
        new Set(knownSpec.provided_text_measurement_provider_ids),
      )
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog constructor service providers do not match the generated contract.",
      );
    }
    contractIds.push(contract.id);
  }
  sortedUniqueStrings(contractIds, "constructor_service_contracts.id");
  if (!sameOrderedStrings(contractIds, ids)) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog constructor service contracts do not match constructor_service_ids.",
    );
  }
  for (const id of ids) {
    if (
      CONSTRUCTOR_SERVICE_SPEC_BY_ID.has(id) &&
      !NODE_CONSTRUCTOR_SERVICE_CANDIDATE_IDS.has(id)
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog advertises a constructor service unavailable through the Node facade.",
      );
    }
  }
  return { ids, contracts };
}

function validateConstructorResourceLimits(limits, serviceId) {
  const ids = [];
  for (const limit of limits) {
    if (
      !isPlainObject(limit) ||
      typeof limit.id !== "string" ||
      limit.id.length === 0 ||
      typeof limit.phase !== "string" ||
      limit.phase.length === 0 ||
      typeof limit.unit !== "string" ||
      limit.unit.length === 0 ||
      typeof limit.description !== "string" ||
      limit.description.length === 0 ||
      !safeInteger(limit.value) ||
      limit.value < 0
    ) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog constructor service ${serviceId} has an invalid resource limit.`,
      );
    }
    ids.push(limit.id);
  }
  sortedUniqueFieldIdentifiers(
    ids,
    `constructor_service_contracts.${serviceId}.resource_limits.id`,
  );
}

function validateOutputContracts(value, outputIds) {
  if (!Array.isArray(value)) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid output contracts.",
    );
  }
  const contractIds = [];
  for (const contract of value) {
    if (
      !isPlainObject(contract) ||
      typeof contract.id !== "string" ||
      contract.id.length === 0 ||
      typeof contract.media_type !== "string" ||
      contract.media_type.length === 0
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid output contract.",
      );
    }
    contractIds.push(contract.id);
    assertUtf8Field(
      contract.media_type,
      `runtime catalog output_contracts.${contract.id}.media_type`,
      NODE_TRANSPORT_FIELD_LIMITS.media_type_utf8_bytes,
    );
    validateSystemFonts(contract.system_fonts);
    validateEmbeddedImages(contract.embedded_images);
    const known = KNOWN_OUTPUT_CONTRACT_BY_ID.get(contract.id);
    if (
      known !== undefined &&
      (contract.media_type !== known.media_type ||
        !sameJsonValue(contract.system_fonts, known.system_fonts) ||
        !sameJsonValue(contract.embedded_images, known.embedded_images))
    ) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog output contract ${contract.id} disagrees with the generated artifact profile.`,
      );
    }
  }
  if (
    contractIds.some((id, index) => index > 0 && contractIds[index - 1] >= id) ||
    contractIds.length !== outputIds.length ||
    contractIds.some((id, index) => id !== outputIds[index])
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog output contracts do not match output_ids.",
    );
  }
}

function validateSystemFonts(value) {
  if (value === null) return;
  if (
    !isPlainObject(value) ||
    typeof value.source_id !== "string" ||
    !RUNTIME_CATALOG_IDENTIFIER.test(value.source_id) ||
    typeof value.discovery !== "string" ||
    !RUNTIME_CATALOG_IDENTIFIER.test(value.discovery) ||
    typeof value.cache_scope !== "string" ||
    !RUNTIME_CATALOG_IDENTIFIER.test(value.cache_scope) ||
    typeof value.host_dependent !== "boolean" ||
    typeof value.caller_configurable !== "boolean" ||
    typeof value.resource_bounded !== "boolean"
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has an invalid system-font output contract.",
    );
  }
}

function validateEmbeddedImages(value) {
  if (value === null) return;
  if (
    !isPlainObject(value) ||
    typeof value.filesystem_access !== "boolean" ||
    typeof value.network_access !== "boolean" ||
    typeof value.caller_configurable !== "boolean" ||
    !isPlainObject(value.limits)
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has an invalid embedded-image output contract.",
    );
  }
  sortedUniqueStrings(value.source_ids, "embedded_images.source_ids");
  for (const field of [
    "max_bytes_per_image",
    "max_total_bytes",
    "max_pixels_per_image",
    "max_total_pixels",
  ]) {
    const limit = value.limits[field];
    if (limit !== null && !positiveSafeInteger(limit)) {
      throw new MermanInvalidTransportError(
        `Merman runtime catalog embedded-image ${field} must be null or a positive safe integer.`,
      );
    }
  }
}

function validateTextMeasurement(value, { requiresSvgPipeline }) {
  if (value === null && !requiresSvgPipeline) return [];
  if (
    !isPlainObject(value) ||
    value.protocol_version !== TEXT_MEASUREMENT_PROTOCOL_VERSION
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid text measurement metadata.",
    );
  }
  const providerIds = sortedUniqueStrings(
    value.provider_ids,
    "text_measurement.provider_ids",
  );
  if (providerIds.length === 0) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog text measurement must advertise at least one provider.",
    );
  }
  if (
    providerIds.some(
      (id) =>
        KNOWN_TEXT_MEASUREMENT_PROVIDER_IDS.has(id) &&
        id !== VENDORED_TEXT_MEASUREMENT_PROVIDER_ID,
    )
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog advertises a known text measurement provider unavailable through the Node facade.",
    );
  }
  if (
    requiresSvgPipeline &&
    !providerIds.includes(VENDORED_TEXT_MEASUREMENT_PROVIDER_ID)
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog must expose the callable vendored text measurement provider for the SVG pipeline.",
    );
  }
  return providerIds;
}

function validateRegistry(value) {
  if (
    !safeInteger(value.diagram_family_count) ||
    value.diagram_family_count < 0
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog has invalid registry metadata.",
    );
  }
}

function validateResources(value, operationIds) {
  if (
    typeof value.general_binding_default_profile !== "string" ||
    !RUNTIME_CATALOG_IDENTIFIER.test(value.general_binding_default_profile) ||
    typeof value.cli_default_profile !== "string" ||
    !RUNTIME_CATALOG_IDENTIFIER.test(value.cli_default_profile) ||
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
  const limitMinimums = new Map();
  const hardCapIds = new Set();
  for (const limit of value.limits) {
    if (
      !isPlainObject(limit) ||
      typeof limit.id !== "string" ||
      !RUNTIME_CATALOG_FIELD_IDENTIFIER.test(limit.id) ||
      limitIds.has(limit.id) ||
      typeof limit.phase !== "string" ||
      limit.phase.length === 0 ||
      typeof limit.description !== "string" ||
      limit.description.length === 0 ||
      typeof limit.overridable !== "boolean" ||
      typeof limit.hard_cap !== "boolean" ||
      !safeInteger(limit.minimum_value) ||
      limit.minimum_value < 0 ||
      !Array.isArray(limit.operation_ids)
    ) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog has an invalid resource limit.",
      );
    }
    const limitOperationIds = sortedUniqueStrings(
      limit.operation_ids,
      `resources.limits.${limit.id}.operation_ids`,
    );
    if (!limitOperationIds.every((id) => operationIds.has(id))) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog resource limit names an unavailable operation.",
      );
    }
    if (limit.hard_cap && limit.overridable) {
      throw new MermanInvalidTransportError(
        "Merman runtime catalog hard resource limits cannot be overridable.",
      );
    }
    limitIds.add(limit.id);
    limitMinimums.set(limit.id, limit.minimum_value);
    if (limit.hard_cap) hardCapIds.add(limit.id);
  }
  const profileIds = new Set();
  for (const profile of value.profiles) {
    if (
      !isPlainObject(profile) ||
      typeof profile.id !== "string" ||
      !RUNTIME_CATALOG_IDENTIFIER.test(profile.id) ||
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
    for (const [id, limit] of Object.entries(profile.limits)) {
      if (
        (limit === null && hardCapIds.has(id)) ||
        (limit !== null &&
          (!safeInteger(limit) || limit < limitMinimums.get(id)))
      ) {
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
  const recommendedProfiles = value.profiles.filter(
    (profile) => profile.recommended_binding_default === true,
  );
  if (
    recommendedProfiles.length !== 1 ||
    recommendedProfiles[0].id !== generalDefault.id
  ) {
    throw new MermanInvalidTransportError(
      "Merman runtime catalog must recommend exactly the general binding default profile.",
    );
  }
}

function sameStringSet(values, expected) {
  return values.length === expected.size && values.every((value) => expected.has(value));
}

function sameOrderedStrings(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function positiveSafeInteger(value) {
  return safeInteger(value) && value > 0;
}

function safeInteger(value) {
  return Number.isInteger(value) && Math.abs(value) <= RUNTIME_CATALOG_MAX_SAFE_INTEGER;
}

function sortedUniquePositiveIntegers(value, field) {
  if (
    !Array.isArray(value) ||
    value.some((item) => !positiveSafeInteger(item)) ||
    value.some((item, index) => index > 0 && value[index - 1] >= item)
  ) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog field ${field} must be a sorted unique array of positive integers.`,
    );
  }
  return value;
}

function sortedUniqueStrings(value, field) {
  sortedUniqueNonEmptyStrings(value, field);
  if (value.some((item) => !RUNTIME_CATALOG_IDENTIFIER.test(item))) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog ${field} must contain valid identifiers.`,
    );
  }
  for (const item of value) {
    assertUtf8Field(
      item,
      `runtime catalog ${field}`,
      NODE_TRANSPORT_FIELD_LIMITS.capability_id_utf8_bytes,
    );
  }
  return value;
}

function sortedUniqueNonEmptyStrings(value, field) {
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

function sortedUniqueFieldIdentifiers(value, field) {
  sortedUniqueNonEmptyStrings(value, field);
  if (value.some((item) => !RUNTIME_CATALOG_FIELD_IDENTIFIER.test(item))) {
    throw new MermanInvalidTransportError(
      `Merman runtime catalog ${field} must contain valid field identifiers.`,
    );
  }
  for (const item of value) {
    assertUtf8Field(
      item,
      `runtime catalog ${field}`,
      NODE_TRANSPORT_FIELD_LIMITS.capability_id_utf8_bytes,
    );
  }
  return value;
}

function cloneBoundedJsonValue(root, label) {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;
  const active = new WeakSet();
  const clone = jsonContainerClone(root, label);
  const stack = [{ depth: 1, source: root, target: clone, exiting: false }];
  let members = 0;
  let tokens = 1;

  while (stack.length > 0) {
    const frame = stack.pop();
    if (frame.exiting) {
      active.delete(frame.source);
      continue;
    }
    if (active.has(frame.source)) {
      throw new TypeError(`${label} must not contain cyclic references.`);
    }
    if (frame.depth > limits.max_depth) {
      throw new RangeError(`${label} exceeds the structural depth limit ${limits.max_depth}.`);
    }
    active.add(frame.source);
    stack.push({ ...frame, exiting: true });

    const entries = jsonContainerEntries(
      frame.source,
      label,
      limits.max_members - members,
      limits.max_members,
    );
    members += entries.length;
    if (members > limits.max_members) {
      throw new RangeError(`${label} exceeds the member-work limit ${limits.max_members}.`);
    }

    for (let index = entries.length - 1; index >= 0; index -= 1) {
      const [key, child] = entries[index];
      assertUtf8Field(key, `${label} object key`, limits.max_string_utf8_bytes);
      tokens += 1;
      if (tokens > limits.max_tokens) {
        throw new RangeError(`${label} exceeds the token-work limit ${limits.max_tokens}.`);
      }

      if (child === null || typeof child === "boolean") {
        defineJsonValue(frame.target, key, child);
      } else if (typeof child === "string") {
        assertUtf8Field(child, `${label}.${key}`, limits.max_string_utf8_bytes);
        defineJsonValue(frame.target, key, child);
      } else if (typeof child === "number") {
        if (!Number.isFinite(child)) {
          throw new TypeError(`${label}.${key} must be a finite JSON number.`);
        }
        defineJsonValue(frame.target, key, child);
      } else if (typeof child === "object") {
        const childClone = jsonContainerClone(child, `${label}.${key}`);
        defineJsonValue(frame.target, key, childClone);
        stack.push({
          depth: frame.depth + 1,
          source: child,
          target: childClone,
          exiting: false,
        });
      } else {
        throw new TypeError(`${label}.${key} is not a JSON wire value.`);
      }
    }
  }
  return clone;
}

function jsonContainerClone(value, label) {
  if (Array.isArray(value)) return [];
  if (!isPlainObject(value)) {
    throw new TypeError(`${label} must contain only arrays and plain objects.`);
  }
  return Object.getPrototypeOf(value) === null ? Object.create(null) : {};
}

function jsonContainerEntries(value, label, remainingMembers, maxMembers) {
  if (Array.isArray(value)) {
    if (value.length > remainingMembers) {
      throw new RangeError(`${label} exceeds the member-work limit ${maxMembers}.`);
    }
    const keys = Reflect.ownKeys(value);
    if (keys.some((key) => typeof key === "symbol")) {
      throw new TypeError(`${label} must not contain symbol keys.`);
    }
    for (let index = 0; index < value.length; index += 1) {
      if (!Object.hasOwn(value, index)) {
        throw new TypeError(`${label} arrays must not contain holes.`);
      }
    }
    if (keys.length !== value.length + 1) {
      throw new TypeError(`${label} arrays must not contain custom properties.`);
    }
    const entries = new Array(value.length);
    for (let index = 0; index < value.length; index += 1) {
      const key = String(index);
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor?.enumerable || !("value" in descriptor)) {
        throw new TypeError(`${label}.${key} must be an enumerable data property.`);
      }
      entries[index] = [key, descriptor.value];
    }
    return entries;
  }

  const entries = [];
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key === "symbol") {
      throw new TypeError(`${label} must not contain symbol keys.`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor?.enumerable || !("value" in descriptor)) {
      throw new TypeError(`${label}.${key} must be an enumerable data property.`);
    }
    entries.push([key, descriptor.value]);
  }
  return entries;
}

function defineJsonValue(target, key, value) {
  if (Array.isArray(target)) {
    target[Number(key)] = value;
    return;
  }
  Object.defineProperty(target, key, {
    configurable: true,
    enumerable: true,
    value,
    writable: true,
  });
}

function isPlainObject(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
