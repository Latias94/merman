import {
  createMermanRuntimeState,
  currentMermanRuntimeState,
  type MermanRuntimeState,
} from "./runtime-state.js";
import {
  MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
} from "./generated/text-measurement-abi.js";
import {
  BINDING_OPTION_GROUP_SPECS,
  BINDING_PAYLOAD_SCHEMAS,
  BINDING_TRANSPORT_EXPOSURE_SPECS,
  CAPABILITY_SPECS,
  CONSTRUCTOR_SERVICE_SPECS,
  METADATA_SPECS,
  RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN,
  RUNTIME_CATALOG_IDENTIFIER_PATTERN,
  RUNTIME_CATALOG_MAX_SAFE_INTEGER,
  RUNTIME_CATALOG_SCHEMA_VERSION,
  TEXT_MEASUREMENT_PROVIDER_SPECS,
} from "./generated/binding-contract.js";
import {
  WEB_BINDING_OPERATIONS,
} from "./generated/capability-surface.js";
import {
  BINDING_OPTIONS_SCHEMA_VERSION,
  tightenResourceOptions,
} from "./generated/resource-contract.js";

import {
  isDiagramType,
  isThemeName,
} from "./public-catalog.js";
import type {
  DiagramFamilyCapability,
  DiagramType,
  RuntimeCapabilities,
  TextMeasurementCapabilities,
  ThemeName,
} from "./public-catalog.js";
import type {
  CommonBindingOptions,
  MermanInitInput,
  MermanWasmModule,
  PresentationAspectCatalogEntry,
  PresentationCatalog,
  PresentationProfileCatalogEntry,
  PresentationThemePresetCatalogEntry,
  RuntimeCatalog,
  RuntimeEmbeddedImageLimits,
  ResourceOptions,
  UnavailableDiagramDetectionFacts,
} from "./public-types.js";

const defaultRuntimeState = createMermanRuntimeState(defaultLoader);
const WEB_TRANSPORT_API_VERSION = 5;
const METADATA_SPEC_BY_ID: ReadonlyMap<string, (typeof METADATA_SPECS)[number]> = new Map(
  METADATA_SPECS.map((spec) => [spec.id, spec])
);
const CAPABILITY_SPEC_BY_ID: ReadonlyMap<string, (typeof CAPABILITY_SPECS)[number]> = new Map(
  CAPABILITY_SPECS.map((spec) => [spec.id, spec])
);
const RUNTIME_CATALOG_IDENTIFIER = new RegExp(RUNTIME_CATALOG_IDENTIFIER_PATTERN);
const RUNTIME_CATALOG_FIELD_IDENTIFIER = new RegExp(
  RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN
);
const BINDING_OPTION_GROUP_SPEC_BY_ID: ReadonlyMap<
  string,
  (typeof BINDING_OPTION_GROUP_SPECS)[number]
> = new Map(
  BINDING_OPTION_GROUP_SPECS.map((spec) => [spec.id, spec])
);
const CONSTRUCTOR_SERVICE_SPEC_BY_ID: ReadonlyMap<
  string,
  (typeof CONSTRUCTOR_SERVICE_SPECS)[number]
> = new Map(
  CONSTRUCTOR_SERVICE_SPECS.map((spec) => [spec.id, spec])
);
const WEB_TRANSPORT_EXPOSURE = BINDING_TRANSPORT_EXPOSURE_SPECS.find(
  (spec) => spec.id === "web"
);
if (WEB_TRANSPORT_EXPOSURE === undefined) {
  throw new Error("Generated binding contract is missing the Web transport exposure.");
}
const WEB_BINDING_OPERATION_SPEC_BY_ID: ReadonlyMap<
  string,
  (typeof WEB_BINDING_OPERATIONS)[number]
> = new Map(WEB_BINDING_OPERATIONS.map((spec) => [spec.id, spec]));
const WEB_PAYLOAD_SCHEMA_VERSION_BY_ID: ReadonlyMap<string, number> = new Map(
  WEB_TRANSPORT_EXPOSURE.payload_schema_ids.map((id) => {
    const spec = BINDING_PAYLOAD_SCHEMAS.find((candidate) => candidate.id === id);
    if (spec === undefined) {
      throw new Error(`Generated Web payload schema ${id} has no typed schema contract.`);
    }
    return [id, spec.version] as const;
  })
);
const WEB_CONSTRUCTOR_SERVICE_CANDIDATE_IDS: ReadonlySet<string> = new Set(
  WEB_TRANSPORT_EXPOSURE.constructor_service_candidate_ids
);
const TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID: ReadonlyMap<
  string,
  (typeof TEXT_MEASUREMENT_PROVIDER_SPECS)[number]
> = new Map(
  TEXT_MEASUREMENT_PROVIDER_SPECS.map((spec) => [spec.id, spec])
);

export function currentRuntimeState(): MermanRuntimeState {
  return currentMermanRuntimeState(defaultRuntimeState);
}

export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {
  const state = currentRuntimeState();
  if (state.wasmModule) {
    return Promise.resolve(state.wasmModule);
  }
  if (state.initPromise) {
    return state.initPromise;
  }
  state.initPromise = doInit(state, init).catch((error) => {
    state.initPromise = null;
    throw error;
  });
  return state.initPromise;
}

async function doInit(
  state: MermanRuntimeState,
  init?: MermanInitInput
): Promise<MermanWasmModule> {
  const loader = typeof init === "function" ? init : init?.loader;
  const wasm = typeof init === "function" ? undefined : init?.wasm;
  const module = loader ? await loader() : await state.defaultLoader();
  if (wasm === undefined) {
    await module.default();
  } else {
    await module.default({ module_or_path: wasm });
  }
  if (module.transportApiVersion() !== WEB_TRANSPORT_API_VERSION) {
    throw new Error(
      `Merman WASM transport API is incompatible with Web transport API ${WEB_TRANSPORT_API_VERSION}.`,
    );
  }
  state.wasmModule = module;
  return module;
}

async function defaultLoader(): Promise<MermanWasmModule> {
  throw new Error(
    "The shared @mermanjs/web implementation has no WASM artifact. Import one browser package entry such as @mermanjs/web, @mermanjs/web-render, @mermanjs/web-analysis, @mermanjs/web-editor, or @mermanjs/web-ascii."
  );
}

export function getMerman(): MermanWasmModule {
  const state = currentRuntimeState();
  if (!state.wasmModule) {
    throw new Error("Merman WASM is not initialized. Call initMerman() first.");
  }
  return state.wasmModule;
}

export function isMermanInitialized(): boolean {
  return currentRuntimeState().wasmModule !== null;
}

export const UNAVAILABLE_DIAGRAM_DETECTION: UnavailableDiagramDetectionFacts = Object.freeze({
  status: "unavailable",
  validity: "unknown",
  diagramType: null,
  syntaxId: null,
  effectiveLayoutId: null,
});

export function runtimeCatalog(): RuntimeCatalog {
  const state = currentRuntimeState();
  state.runtimeCatalogCache ??= normalizeRuntimeCatalog(getMerman().runtimeCatalog());
  return structuredCloneValue(state.runtimeCatalogCache);
}

export function presentationCatalog(): PresentationCatalog {
  const state = currentRuntimeState();
  state.presentationCatalogCache ??= normalizePresentationCatalog(
    getMerman().presentationCatalog()
  );
  return structuredCloneValue(state.presentationCatalogCache);
}

export function supportedDiagrams(): DiagramType[] {
  const state = currentRuntimeState();
  state.supportedDiagramsCache ??= getMerman().supportedDiagrams().map(assertDiagramType);
  return [...state.supportedDiagramsCache];
}

export function diagramFamilyCapabilities(): DiagramFamilyCapability[] {
  return cachedDiagramFamilyCapabilities().map((capability) => ({ ...capability }));
}

function cachedDiagramFamilyCapabilities(): readonly DiagramFamilyCapability[] {
  const state = currentRuntimeState();
  state.diagramFamilyCapabilitiesCache ??= getMerman()
    .diagramFamilyCapabilities()
    .map(normalizeDiagramFamilyCapability);
  return state.diagramFamilyCapabilitiesCache;
}

export function supportedThemes(): ThemeName[] {
  const state = currentRuntimeState();
  state.supportedThemesCache ??= getMerman().supportedThemes().map(assertThemeName);
  return [...state.supportedThemesCache];
}

export function transportApiVersion(): number {
  return assertSafeIntegerField(
    getMerman().transportApiVersion(),
    "Web transport API version",
    1
  );
}

export function packageVersion(): string {
  return getMerman().packageVersion();
}

export function encodeOptions(
  options?: CommonBindingOptions | string
): string | undefined {
  if (options === undefined) {
    return undefined;
  }
  return typeof options === "string" ? options : JSON.stringify(options);
}

export function withResourceOptions<T extends CommonBindingOptions>(
  options: T,
  resources: ResourceOptions,
): T {
  const result: CommonBindingOptions = { ...options };
  const wrappers = (["analysis", "merman"] as const).filter((key) =>
    Object.prototype.hasOwnProperty.call(options, key)
  );
  if (wrappers.length > 1) {
    throw new TypeError(
      "Merman options must not contain both analysis and merman wrappers."
    );
  }
  if (
    wrappers.length !== 0 &&
    Object.prototype.hasOwnProperty.call(options, "resources")
  ) {
    throw new TypeError(
      "Merman options must not mix top-level resources with an analysis or merman wrapper."
    );
  }

  const wrapperKey = wrappers[0];
  if (wrapperKey !== undefined) {
    const wrapper = options[wrapperKey];
    if (!isRecord(wrapper)) {
      throw new TypeError(`Merman ${wrapperKey} wrapper must be an object.`);
    }
    result[wrapperKey] = {
      ...wrapper,
      resources: tightenResourceOptions(
        resources,
        wrapper.resources as ResourceOptions | undefined,
      ),
    };
  } else {
    result.resources = tightenResourceOptions(resources, options.resources);
  }
  return result as T;
}

function assertDiagramType(diagram: string): DiagramType {
  if (isDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown diagram type: ${diagram}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function normalizeDiagramFamilyCapability(
  capability: DiagramFamilyCapability
): DiagramFamilyCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  const metadataId =
    capability.metadata_id === undefined || capability.metadata_id === null
      ? null
      : assertDiagramType(String(capability.metadata_id));
  return {
    diagram_type: capability.diagram_type,
    logical_family_kind: assertStringField(
      capability.logical_family_kind,
      "diagram logical family kind"
    ),
    metadata_id: metadataId,
    render_model_kind: assertNullableStringField(
      capability.render_model_kind,
      "diagram render model kind"
    ),
    has_detector: assertBooleanField(capability.has_detector, "diagram detector capability"),
    has_semantic_parser: assertBooleanField(
      capability.has_semantic_parser,
      "diagram semantic parser capability"
    ),
    has_editor_parser: assertBooleanField(
      capability.has_editor_parser,
      "diagram editor parser capability"
    ),
    has_combined_parser: assertBooleanField(
      capability.has_combined_parser,
      "diagram combined parser capability"
    ),
    has_render_parser: assertBooleanField(
      capability.has_render_parser,
      "diagram render parser capability"
    ),
    has_header: assertBooleanField(capability.has_header, "diagram header capability"),
    config_namespace: assertNullableStringField(
      capability.config_namespace,
      "diagram config namespace"
    ),
  };
}

function assertStringField(value: unknown, label: string): string {
  if (typeof value === "string") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertNullableStringField(value: unknown, label: string): string | null {
  if (value === undefined || value === null) {
    return null;
  }
  return assertStringField(value, label);
}

function assertBooleanField(value: unknown, label: string): boolean {
  if (typeof value === "boolean") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertThemeName(theme: string): ThemeName {
  if (isThemeName(theme)) {
    return theme;
  }
  throw new Error(`Merman WASM returned unknown theme: ${theme}`);
}

function normalizeRuntimeCatalog(value: unknown): RuntimeCatalog {
  if (!isRecord(value) || value.schema_version !== RUNTIME_CATALOG_SCHEMA_VERSION) {
    throw new Error("Merman WASM returned an unsupported runtime catalog schema.");
  }
  assertRequiredRecordKeys(
    value,
    [
      "capabilities",
      "metadata_ids",
      "options_schema_versions",
      "output_contracts",
      "package_version",
      "payload_schemas",
      "registry",
      "resources",
      "schema_version",
      "transport_api_version",
    ],
    "Merman WASM runtime catalog"
  );
  const catalogTransportApiVersion = assertSafeIntegerField(
    value.transport_api_version,
    "runtime transport API version",
    1
  );
  if (catalogTransportApiVersion !== transportApiVersion()) {
    throw new Error(
      "Merman WASM runtime catalog transport API does not match the loaded module."
    );
  }
  if (
    typeof value.package_version !== "string" ||
    value.package_version.length === 0 ||
    value.package_version !== packageVersion()
  ) {
    throw new Error("Merman WASM runtime catalog package version does not match the loaded module.");
  }
  if (!isRecord(value.registry)) {
    throw new Error("Merman WASM returned an invalid runtime registry catalog.");
  }
  assertRequiredRecordKeys(
    value.registry,
    ["diagram_family_count"],
    "Merman WASM runtime registry catalog"
  );
  const diagramFamilyCount = assertSafeIntegerField(
    value.registry.diagram_family_count,
    "runtime registry diagram family count",
    0
  );
  if (!isRecord(value.resources)) {
    throw new Error("Merman WASM returned an invalid runtime resource contract.");
  }
  const capabilities = normalizeRuntimeCapabilities(value.capabilities);
  const usesSvgPipeline = runtimeUsesSvgPipeline(capabilities);
  const metadataIds = normalizeRuntimeMetadataIds(value.metadata_ids, capabilities);
  const optionGroupIds = Object.prototype.hasOwnProperty.call(value, "option_group_ids")
    ? normalizeRuntimeOptionGroupIds(value.option_group_ids, capabilities, usesSvgPipeline)
    : [];
  const constructorServices = normalizeRuntimeConstructorServices(value, capabilities);
  const optionsSchemaVersions = normalizeSortedPositiveIntegers(
    value.options_schema_versions,
    "runtime options schema versions"
  );
  if (!optionsSchemaVersions.includes(BINDING_OPTIONS_SCHEMA_VERSION)) {
    throw new Error(
      `Merman WASM runtime catalog does not advertise options schema v${BINDING_OPTIONS_SCHEMA_VERSION}.`
    );
  }
  return {
    ...structuredCloneValue(value),
    schema_version: RUNTIME_CATALOG_SCHEMA_VERSION,
    transport_api_version: catalogTransportApiVersion,
    package_version: value.package_version,
    options_schema_versions: optionsSchemaVersions,
    payload_schemas: normalizeRuntimePayloadSchemas(value.payload_schemas),
    metadata_ids: metadataIds,
    option_group_ids: optionGroupIds,
    constructor_service_ids: constructorServices.ids,
    constructor_service_contracts: constructorServices.contracts,
    capabilities,
    output_contracts: normalizeRuntimeOutputContracts(
      value.output_contracts,
      capabilities.output_ids
    ),
    registry: {
      ...structuredCloneValue(value.registry),
      diagram_family_count: diagramFamilyCount,
    },
    resources: normalizeRuntimeResourceContract(
      value.resources,
      new Set(capabilities.operation_ids)
    ),
  };
}

function runtimeUsesSvgPipeline(capabilities: RuntimeCapabilities): boolean {
  return capabilities.capability_ids.includes("svg") || capabilities.text_measurement !== null;
}

function normalizeRuntimeMetadataIds(
  value: unknown,
  capabilities: RuntimeCapabilities
): string[] {
  const ids = normalizeSortedIdentifierIds(value, "runtime metadata IDs");
  const capabilityIds = new Set(capabilities.capability_ids);
  for (const id of ids) {
    const spec = METADATA_SPEC_BY_ID.get(id);
    if (
      spec?.required_capability_id !== null &&
      spec?.required_capability_id !== undefined &&
      !capabilityIds.has(spec.required_capability_id)
    ) {
      throw new Error(
        `Merman WASM runtime metadata ${id} requires capability ${spec.required_capability_id}.`
      );
    }
  }
  return ids;
}

function normalizeRuntimeOptionGroupIds(
  value: unknown,
  capabilities: RuntimeCapabilities,
  usesSvgPipeline: boolean
): string[] {
  const ids = normalizeSortedOptionGroupIds(value);
  const capabilityIds = new Set(capabilities.capability_ids);
  const expectedKnownIds = BINDING_OPTION_GROUP_SPECS
    .filter(
      (spec) =>
        spec.always_available ||
        (spec.requires_svg_pipeline && usesSvgPipeline) ||
        spec.any_capability_ids.some((id) => capabilityIds.has(id))
    )
    .map((spec) => spec.id)
    .sort();
  const actualKnownIds = ids.filter((id) => BINDING_OPTION_GROUP_SPEC_BY_ID.has(id));
  if (!sameStringArrays(actualKnownIds, expectedKnownIds)) {
    throw new Error(
      "Merman WASM runtime option group IDs do not match the artifact capability closure."
    );
  }
  return ids;
}

function normalizeRuntimeConstructorServices(
  catalog: Record<string, unknown>,
  capabilities: RuntimeCapabilities
): {
  ids: string[];
  contracts: RuntimeCatalog["constructor_service_contracts"];
} {
  const hasIds = Object.prototype.hasOwnProperty.call(catalog, "constructor_service_ids");
  const hasContracts = Object.prototype.hasOwnProperty.call(
    catalog,
    "constructor_service_contracts"
  );
  if (hasIds !== hasContracts) {
    throw new Error(
      "Merman WASM runtime constructor service IDs and contracts must appear together."
    );
  }
  if (!hasIds) {
    return { ids: [], contracts: [] };
  }

  const ids = normalizeSortedIdentifierIds(
    catalog.constructor_service_ids,
    "runtime constructor service IDs"
  );
  const usesSvgPipeline = runtimeUsesSvgPipeline(capabilities);
  for (const id of ids) {
    const knownSpec = CONSTRUCTOR_SERVICE_SPEC_BY_ID.get(id);
    if (knownSpec === undefined) {
      continue;
    }
    if (!WEB_CONSTRUCTOR_SERVICE_CANDIDATE_IDS.has(id)) {
      throw new Error(
        `Merman WASM advertises constructor service ${id}, which this Web facade cannot provide.`
      );
    }
    if (knownSpec.requires_svg_pipeline && !usesSvgPipeline) {
      throw new Error(
        `Merman WASM advertises constructor service ${id} without an SVG pipeline.`
      );
    }
  }
  const contracts = normalizeRuntimeConstructorServiceContracts(
    catalog.constructor_service_contracts
  );
  const contractIds = contracts.map((contract) => contract.id);
  if (!sameStringArrays(ids, contractIds)) {
    throw new Error(
      "Merman WASM runtime constructor service IDs and contracts must be in one-to-one correspondence."
    );
  }
  validateRuntimeConstructorServiceProviders(contracts, capabilities);
  return { ids, contracts };
}

function normalizeRuntimeConstructorServiceContracts(
  value: unknown
): RuntimeCatalog["constructor_service_contracts"] {
  if (!Array.isArray(value)) {
    throw new Error("Merman WASM returned invalid runtime constructor service contracts.");
  }
  const contracts = value.map((contract) => {
    if (!isRecord(contract)) {
      throw new Error("Merman WASM returned an invalid runtime constructor service contract.");
    }
    assertRequiredRecordKeys(
      contract,
      ["id", "provided_text_measurement_provider_ids", "resource_limits"],
      "Merman WASM runtime constructor service contract"
    );
    const id = assertRuntimeIdentifier(contract.id, "runtime constructor service contract ID");
    const providerIds = normalizeSortedIdentifierIds(
      contract.provided_text_measurement_provider_ids,
      `runtime constructor service ${id} text measurement provider IDs`
    );
    return {
      ...structuredCloneValue(contract),
      id,
      provided_text_measurement_provider_ids: providerIds,
      resource_limits: normalizeRuntimeConstructorResourceLimits(contract.resource_limits, id),
    };
  });
  const ids = contracts.map((contract) => contract.id);
  for (let index = 1; index < ids.length; index += 1) {
    if (ids[index - 1] >= ids[index]) {
      throw new Error(
        "Merman WASM runtime constructor service contracts must be sorted and unique by ID."
      );
    }
  }
  return contracts;
}

function normalizeRuntimeConstructorResourceLimits(
  value: unknown,
  serviceId: string
): RuntimeCatalog["constructor_service_contracts"][number]["resource_limits"] {
  if (!Array.isArray(value)) {
    throw new Error(
      `Merman WASM returned invalid runtime constructor service ${serviceId} resource limits.`
    );
  }
  const limits = value.map((limit) => {
    if (!isRecord(limit)) {
      throw new Error(
        `Merman WASM returned an invalid runtime constructor service ${serviceId} resource limit.`
      );
    }
    assertRequiredRecordKeys(
      limit,
      ["id", "phase", "unit", "description", "value"],
      `Merman WASM runtime constructor service ${serviceId} resource limit`
    );
    const id = assertRuntimeFieldIdentifier(
      limit.id,
      `runtime constructor service ${serviceId} resource limit ID`
    );
    if (
      typeof limit.phase !== "string" ||
      limit.phase.length === 0 ||
      typeof limit.unit !== "string" ||
      limit.unit.length === 0 ||
      typeof limit.description !== "string" ||
      limit.description.length === 0
    ) {
      throw new Error(
        `Merman WASM returned an invalid runtime constructor service ${serviceId} resource limit.`
      );
    }
    return {
      ...structuredCloneValue(limit),
      id,
      phase: limit.phase,
      unit: limit.unit,
      description: limit.description,
      value: assertSafeIntegerField(
        limit.value,
        `runtime constructor service ${serviceId} resource limit value`,
        0
      ),
    };
  });
  const ids = limits.map((limit) => limit.id);
  for (let index = 1; index < ids.length; index += 1) {
    if (ids[index - 1] >= ids[index]) {
      throw new Error(
        `Merman WASM runtime constructor service ${serviceId} resource limits must be sorted and unique by ID.`
      );
    }
  }
  return limits;
}

function validateRuntimeConstructorServiceProviders(
  contracts: RuntimeCatalog["constructor_service_contracts"],
  capabilities: RuntimeCapabilities
): void {
  const runtimeProviderIds = new Set(capabilities.text_measurement?.provider_ids ?? []);
  const contractById = new Map(contracts.map((contract) => [contract.id, contract]));
  const providerOwnerById = new Map<string, string>();

  for (const contract of contracts) {
    const serviceSpec = CONSTRUCTOR_SERVICE_SPEC_BY_ID.get(contract.id);
    if (serviceSpec !== undefined) {
      const actualKnownProviderIds = contract.provided_text_measurement_provider_ids.filter((id) =>
        TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.has(id)
      );
      if (
        !sameStringArrays(
          actualKnownProviderIds,
          [...serviceSpec.provided_text_measurement_provider_ids]
        )
      ) {
        throw new Error(
          `Merman WASM runtime constructor service ${contract.id} does not match its known provider contract.`
        );
      }
    }

    for (const providerId of contract.provided_text_measurement_provider_ids) {
      if (!runtimeProviderIds.has(providerId)) {
        throw new Error(
          `Merman WASM runtime constructor service ${contract.id} provides an unavailable text measurement provider.`
        );
      }
      const previousOwner = providerOwnerById.get(providerId);
      if (previousOwner !== undefined) {
        throw new Error(
          `Merman WASM runtime text measurement provider ${providerId} has multiple constructor service owners.`
        );
      }
      providerOwnerById.set(providerId, contract.id);

      const providerSpec = TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.get(providerId);
      if (
        providerSpec !== undefined &&
        (providerSpec.source !== "constructor-service" ||
          providerSpec.constructor_service_id !== contract.id)
      ) {
        throw new Error(
          `Merman WASM runtime text measurement provider ${providerId} has the wrong constructor service owner.`
        );
      }
    }
  }

  for (const providerId of runtimeProviderIds) {
    const providerSpec = TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.get(providerId);
    if (providerSpec?.source !== "constructor-service") {
      continue;
    }
    const ownerId = providerSpec.constructor_service_id;
    const ownerContract = ownerId === null ? undefined : contractById.get(ownerId);
    if (
      ownerContract === undefined ||
      !ownerContract.provided_text_measurement_provider_ids.includes(providerId)
    ) {
      throw new Error(
        `Merman WASM runtime text measurement provider ${providerId} is missing its constructor service owner.`
      );
    }
  }
}

function normalizePresentationCatalog(value: unknown): PresentationCatalog {
  if (!isRecord(value) || value.schema_version !== 1) {
    throw new Error("Merman WASM returned an unsupported presentation catalog schema.");
  }
  assertRequiredRecordKeys(
    value,
    ["profiles", "schema_version", "theme_presets"],
    "Merman WASM presentation catalog"
  );
  if (!Array.isArray(value.theme_presets) || !Array.isArray(value.profiles)) {
    throw new Error("Merman WASM returned an invalid presentation catalog.");
  }
  return {
    schema_version: 1,
    theme_presets: normalizePresentationThemePresets(value.theme_presets),
    profiles: normalizePresentationProfiles(value.profiles),
  };
}

function normalizePresentationThemePresets(
  value: unknown[]
): PresentationThemePresetCatalogEntry[] {
  const seen = new Set<string>();
  return value.map((entry) => {
    if (!isRecord(entry)) {
      throw new Error("Merman WASM returned an invalid presentation theme preset.");
    }
    assertRequiredRecordKeys(
      entry,
      ["appearance", "fully_available", "id", "missing_capability_ids"],
      "Merman WASM presentation theme preset"
    );
    const id = assertUniquePresentationId(entry.id, seen, "theme preset");
    return {
      id,
      appearance: assertRuntimeIdentifier(
        entry.appearance,
        `presentation theme preset ${id} appearance`
      ),
      fully_available: assertBooleanField(
        entry.fully_available,
        `presentation theme preset ${id} availability`
      ),
      missing_capability_ids: normalizeSortedIdentifierIds(
        entry.missing_capability_ids,
        `presentation theme preset ${id} missing capability IDs`
      ),
    };
  });
}

function normalizePresentationProfiles(value: unknown[]): PresentationProfileCatalogEntry[] {
  const seen = new Set<string>();
  return value.map((entry) => {
    if (!isRecord(entry)) {
      throw new Error("Merman WASM returned an invalid presentation profile.");
    }
    assertRequiredRecordKeys(
      entry,
      ["aspects", "fully_available", "id", "missing_capability_ids"],
      "Merman WASM presentation profile"
    );
    if (!Array.isArray(entry.aspects)) {
      throw new Error("Merman WASM returned invalid presentation profile aspects.");
    }
    const id = assertUniquePresentationId(entry.id, seen, "profile");
    return {
      id,
      fully_available: assertBooleanField(
        entry.fully_available,
        `presentation profile ${id} availability`
      ),
      missing_capability_ids: normalizeSortedIdentifierIds(
        entry.missing_capability_ids,
        `presentation profile ${id} missing capability IDs`
      ),
      aspects: normalizePresentationAspects(id, entry.aspects),
    };
  });
}

function normalizePresentationAspects(
  profileId: string,
  value: unknown[]
): PresentationAspectCatalogEntry[] {
  const seen = new Set<string>();
  return value.map((entry) => {
    if (!isRecord(entry) || !isRecord(entry.applicability)) {
      throw new Error("Merman WASM returned an invalid presentation profile aspect.");
    }
    assertRequiredRecordKeys(
      entry,
      [
        "applicability",
        "available",
        "id",
        "missing_capability_ids",
        "required_capability_id",
      ],
      "Merman WASM presentation profile aspect"
    );
    assertRequiredRecordKeys(
      entry.applicability,
      ["family_id", "kind"],
      "Merman WASM presentation aspect applicability"
    );
    const id = assertUniquePresentationId(
      entry.id,
      seen,
      `profile ${profileId} aspect`
    );
    const familyId = entry.applicability.family_id;
    return {
      id,
      applicability: {
        kind: assertRuntimeIdentifier(
          entry.applicability.kind,
          `presentation aspect ${id} applicability kind`
        ),
        family_id:
          familyId === null
            ? null
            : assertRuntimeIdentifier(
                familyId,
                `presentation aspect ${id} family ID`
              ),
      },
      required_capability_id:
        entry.required_capability_id === null
          ? null
          : assertRuntimeIdentifier(
              entry.required_capability_id,
              `presentation aspect ${id} required capability ID`
            ),
      available: assertBooleanField(
        entry.available,
        `presentation aspect ${id} availability`
      ),
      missing_capability_ids: normalizeSortedIdentifierIds(
        entry.missing_capability_ids,
        `presentation aspect ${id} missing capability IDs`
      ),
    };
  });
}

function assertUniquePresentationId(
  value: unknown,
  seen: Set<string>,
  label: string
): string {
  const id = assertRuntimeIdentifier(value, `presentation ${label} ID`);
  if (seen.has(id)) {
    throw new Error(`Merman WASM presentation ${label} IDs must be unique.`);
  }
  seen.add(id);
  return id;
}

function normalizeRuntimePayloadSchemas(value: unknown): RuntimeCatalog["payload_schemas"] {
  if (!Array.isArray(value)) {
    throw new Error("Merman WASM returned invalid runtime payload schemas.");
  }
  const schemas = value.map((entry) => {
    if (!isRecord(entry)) {
      throw new Error("Merman WASM returned an invalid runtime payload schema.");
    }
    assertRequiredRecordKeys(entry, ["id", "version"], "Merman WASM runtime payload schema");
    return {
      ...structuredCloneValue(entry),
      id: assertRuntimeIdentifier(entry.id, "runtime payload schema IDs"),
      version: assertSafeIntegerField(entry.version, "runtime payload schema version", 1),
    };
  });
  for (let index = 1; index < schemas.length; index += 1) {
    if (schemas[index - 1].id >= schemas[index].id) {
      throw new Error("Merman WASM runtime payload schema IDs must be sorted and unique.");
    }
  }
  const knownSchemas = schemas.filter((schema) =>
    BINDING_PAYLOAD_SCHEMAS.some((known) => known.id === schema.id)
  );
  if (
    knownSchemas.length !== WEB_PAYLOAD_SCHEMA_VERSION_BY_ID.size ||
    knownSchemas.some(
      (schema) => WEB_PAYLOAD_SCHEMA_VERSION_BY_ID.get(schema.id) !== schema.version
    )
  ) {
    throw new Error(
      "Merman WASM runtime payload schemas do not match the Web transport contract."
    );
  }
  return schemas;
}

function normalizeRuntimeOutputContracts(
  value: unknown,
  outputIds: string[]
): RuntimeCatalog["output_contracts"] {
  if (!Array.isArray(value)) {
    throw new Error("Merman WASM returned invalid runtime output contracts.");
  }
  const contracts = value.map((entry) => {
    if (!isRecord(entry)) {
      throw new Error("Merman WASM returned an invalid runtime output contract.");
    }
    assertRequiredRecordKeys(
      entry,
      ["id", "media_type", "system_fonts", "embedded_images"],
      "Merman WASM runtime output contract"
    );
    const id = assertRuntimeIdentifier(entry.id, "runtime output contract IDs");
    const mediaType = assertStringField(entry.media_type, "runtime output media type");
    if (mediaType.length === 0) {
      throw new Error("Merman WASM returned an empty runtime output media type.");
    }
    return {
      ...structuredCloneValue(entry),
      id,
      media_type: mediaType,
      system_fonts: normalizeRuntimeSystemFonts(entry.system_fonts),
      embedded_images: normalizeRuntimeEmbeddedImages(entry.embedded_images),
    };
  });
  const contractIds = contracts.map((contract) => contract.id);
  for (let index = 1; index < contractIds.length; index += 1) {
    if (contractIds[index - 1] >= contractIds[index]) {
      throw new Error("Merman WASM runtime output contract IDs must be sorted and unique.");
    }
  }
  if (
    contractIds.length !== outputIds.length ||
    contractIds.some((id, index) => id !== outputIds[index])
  ) {
    throw new Error("Merman WASM runtime output contracts do not match runtime output IDs.");
  }
  return contracts;
}

function normalizeRuntimeSystemFonts(
  value: unknown
): RuntimeCatalog["output_contracts"][number]["system_fonts"] {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned an invalid runtime system-font contract.");
  }
  assertRequiredRecordKeys(
    value,
    [
      "source_id",
      "discovery",
      "cache_scope",
      "host_dependent",
      "caller_configurable",
      "resource_bounded",
    ],
    "Merman WASM runtime system-font contract"
  );
  return {
    ...structuredCloneValue(value),
    source_id: assertRuntimeIdentifier(value.source_id, "runtime font source ID"),
    discovery: assertRuntimeIdentifier(value.discovery, "runtime font discovery ID"),
    cache_scope: assertRuntimeIdentifier(value.cache_scope, "runtime font cache-scope ID"),
    host_dependent: assertBooleanField(value.host_dependent, "runtime font host dependence"),
    caller_configurable: assertBooleanField(
      value.caller_configurable,
      "runtime font configurability"
    ),
    resource_bounded: assertBooleanField(value.resource_bounded, "runtime font resource bound"),
  };
}

function normalizeRuntimeEmbeddedImages(
  value: unknown
): RuntimeCatalog["output_contracts"][number]["embedded_images"] {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned an invalid runtime embedded-image contract.");
  }
  assertRequiredRecordKeys(
    value,
    ["source_ids", "filesystem_access", "network_access", "caller_configurable", "limits"],
    "Merman WASM runtime embedded-image contract"
  );
  if (!isRecord(value.limits)) {
    throw new Error("Merman WASM returned invalid runtime embedded-image limits.");
  }
  const limitValues = value.limits;
  const limitNames = [
    "max_bytes_per_image",
    "max_total_bytes",
    "max_pixels_per_image",
    "max_total_pixels",
  ] as const;
  assertRequiredRecordKeys(
    limitValues,
    [...limitNames],
    "Merman WASM runtime embedded-image limits"
  );
  const readLimit = (name: (typeof limitNames)[number]): number | null => {
    const limit = limitValues[name];
    return limit === null
      ? null
      : assertSafeIntegerField(limit, `runtime embedded-image ${name}`, 1);
  };
  const limits: RuntimeEmbeddedImageLimits = {
    max_bytes_per_image: readLimit("max_bytes_per_image"),
    max_total_bytes: readLimit("max_total_bytes"),
    max_pixels_per_image: readLimit("max_pixels_per_image"),
    max_total_pixels: readLimit("max_total_pixels"),
  };
  return {
    ...structuredCloneValue(value),
    source_ids: normalizeSortedIdentifierIds(value.source_ids, "runtime embedded-image source IDs"),
    filesystem_access: assertBooleanField(
      value.filesystem_access,
      "runtime embedded-image filesystem access"
    ),
    network_access: assertBooleanField(
      value.network_access,
      "runtime embedded-image network access"
    ),
    caller_configurable: assertBooleanField(
      value.caller_configurable,
      "runtime embedded-image configurability"
    ),
    limits: {
      ...structuredCloneValue(limitValues),
      ...limits,
    },
  };
}

function normalizeRuntimeCapabilities(value: unknown): RuntimeCapabilities {
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned an invalid runtime capability report.");
  }
  assertRequiredRecordKeys(value, [
    "capability_ids",
    "output_ids",
    "operation_ids",
    "system_adapter_ids",
    "text_measurement",
  ], "Merman WASM runtime capability report");

  const capabilityIds = normalizeSortedIdentifierIds(value.capability_ids, "runtime capability IDs");
  const capabilitySet = new Set(capabilityIds);
  const operationIds = normalizeSortedIdentifierIds(
    value.operation_ids,
    "runtime binding operation IDs"
  );
  const outputIds = normalizeSortedIdentifierIds(value.output_ids, "runtime output IDs");

  const systemAdapterIds = normalizeSortedIdentifierIds(
    value.system_adapter_ids,
    "system adapter IDs"
  );
  for (const adapterId of systemAdapterIds) {
    if (!capabilitySet.has(adapterId)) {
      throw new Error(
        `Merman WASM system adapter ${adapterId} is absent from runtime capability IDs.`
      );
    }
  }
  if (systemAdapterIds.length !== 0) {
    throw new Error("Merman browser WASM must not expose native system adapters.");
  }

  validateCapabilityImplications(capabilityIds);
  const textMeasurement = normalizeTextMeasurementCapabilities(value.text_measurement);
  const svgAvailable = capabilitySet.has("svg");
  if (svgAvailable && textMeasurement === null) {
    throw new Error(
      "Merman WASM text measurement must be present when SVG is available."
    );
  }

  validateKnownRuntimeOperationRelations(operationIds, capabilitySet, new Set(outputIds));

  return {
    ...structuredCloneValue(value),
    capability_ids: capabilityIds,
    output_ids: outputIds,
    operation_ids: operationIds,
    system_adapter_ids: systemAdapterIds,
    text_measurement: textMeasurement,
  };
}

function validateCapabilityImplications(capabilityIds: readonly string[]): void {
  const capabilityIdSet = new Set(capabilityIds);
  for (const capabilityId of capabilityIds) {
    const spec = CAPABILITY_SPEC_BY_ID.get(capabilityId);
    if (spec === undefined) {
      continue;
    }
    for (const implicationId of spec.implication_ids) {
      if (!capabilityIdSet.has(implicationId)) {
        throw new Error(
          `Merman WASM runtime capability ${capabilityId} is missing implied capability ${implicationId}.`
        );
      }
    }
  }
}

function validateKnownRuntimeOperationRelations(
  operationIds: readonly string[],
  capabilityIds: ReadonlySet<string>,
  outputIds: ReadonlySet<string>
): void {
  for (const operationId of operationIds) {
    const operation = WEB_BINDING_OPERATION_SPEC_BY_ID.get(operationId);
    if (operation === undefined) {
      continue;
    }
    if (operation.capability !== null && !capabilityIds.has(operation.capability)) {
      throw new Error(
        `Merman WASM runtime operation ${operationId} is missing capability ${operation.capability}.`
      );
    }
    if (operation.output !== null && !outputIds.has(operation.output)) {
      throw new Error(
        `Merman WASM runtime operation ${operationId} is missing output ${operation.output}.`
      );
    }
  }
}

function normalizeRuntimeResourceContract(
  value: Record<string, unknown>,
  operationIds: ReadonlySet<string>
): RuntimeCatalog["resources"] {
  assertRequiredRecordKeys(
    value,
    [
      "general_binding_default_profile",
      "cli_default_profile",
      "limits",
      "profiles",
    ],
    "Merman WASM runtime resource contract"
  );
  if (
    typeof value.general_binding_default_profile !== "string" ||
    value.general_binding_default_profile.length === 0 ||
    typeof value.cli_default_profile !== "string" ||
    value.cli_default_profile.length === 0 ||
    !Array.isArray(value.limits) ||
    !Array.isArray(value.profiles)
  ) {
    throw new Error("Merman WASM returned an invalid runtime resource contract.");
  }
  const limits = value.limits.map((limit) => {
    if (!isRecord(limit)) {
      throw new Error("Merman WASM returned an invalid runtime resource limit.");
    }
    assertRequiredRecordKeys(
      limit,
      [
        "id",
        "phase",
        "description",
        "overridable",
        "hard_cap",
        "minimum_value",
        "operation_ids",
      ],
      "Merman WASM runtime resource limit"
    );
    if (
      typeof limit.id !== "string" ||
      !RUNTIME_CATALOG_FIELD_IDENTIFIER.test(limit.id) ||
      typeof limit.phase !== "string" ||
      typeof limit.description !== "string" ||
      typeof limit.overridable !== "boolean" ||
      typeof limit.hard_cap !== "boolean" ||
      typeof limit.minimum_value !== "number" ||
      !isSafeCatalogInteger(limit.minimum_value) ||
      limit.minimum_value < 0
    ) {
      throw new Error("Merman WASM returned an invalid runtime resource limit.");
    }
    if (limit.hard_cap && limit.overridable) {
      throw new Error("Merman WASM runtime hard resource limits cannot be overridable.");
    }
    const limitOperationIds = normalizeSortedIdentifierIds(
      limit.operation_ids,
      `runtime resource limit ${limit.id} operation IDs`
    );
    if (limitOperationIds.some((id) => !operationIds.has(id))) {
      throw new Error("Merman WASM runtime resource limit names an unavailable operation.");
    }
    return {
      ...structuredCloneValue(limit),
      id: limit.id,
      phase: limit.phase,
      description: limit.description,
      overridable: limit.overridable,
      hard_cap: limit.hard_cap,
      minimum_value: limit.minimum_value as number,
      operation_ids: limitOperationIds,
    };
  });
  const profiles = value.profiles.map((profile) => {
    if (!isRecord(profile)) {
      throw new Error("Merman WASM returned an invalid runtime resource profile.");
    }
    assertRequiredRecordKeys(
      profile,
      ["id", "purpose", "trust_assumption", "recommended_binding_default", "limits"],
      "Merman WASM runtime resource profile"
    );
    if (
      typeof profile.id !== "string" ||
      !RUNTIME_CATALOG_IDENTIFIER.test(profile.id) ||
      typeof profile.purpose !== "string" ||
      typeof profile.trust_assumption !== "string" ||
      typeof profile.recommended_binding_default !== "boolean" ||
      !(isRecord(profile.limits) || profile.limits instanceof Map)
    ) {
      throw new Error("Merman WASM returned an invalid runtime resource profile.");
    }
    const rawProfileLimits = new Map(normalizeStringMapEntries(
      profile.limits,
      `resource profile ${profile.id} limits`
    ));
    if (
      rawProfileLimits.size !== limits.length ||
      limits.some((limit) => !rawProfileLimits.has(limit.id))
    ) {
      throw new Error("Merman WASM runtime resource profile does not cover the declared limits.");
    }
    const profileLimits: Record<string, number | null> = {};
    for (const descriptor of limits) {
      const limit = rawProfileLimits.get(descriptor.id);
      profileLimits[descriptor.id] =
        limit === null || limit === undefined
          ? null
          : assertSafeIntegerField(
              limit,
              `resource profile limit ${descriptor.id}`,
              descriptor.minimum_value
            );
      if (descriptor.hard_cap && profileLimits[descriptor.id] === null) {
        throw new Error("Merman WASM runtime resource profile removed a finite hard cap.");
      }
    }
    return {
      ...structuredCloneValue(profile),
      id: profile.id,
      purpose: profile.purpose,
      trust_assumption: profile.trust_assumption,
      recommended_binding_default: profile.recommended_binding_default,
      limits: profileLimits,
    };
  });
  const profileById = new Map<string, (typeof profiles)[number]>();
  for (const profile of profiles) {
    if (profileById.has(profile.id)) {
      throw new Error("Merman WASM runtime resource profile IDs must be unique.");
    }
    profileById.set(profile.id, profile);
  }
  const generalDefault = profileById.get(value.general_binding_default_profile);
  if (!generalDefault || !profileById.has(value.cli_default_profile)) {
    throw new Error("Merman WASM runtime resource defaults name unknown profiles.");
  }
  const recommendedProfiles = profiles.filter(
    (profile) => profile.recommended_binding_default
  );
  if (
    recommendedProfiles.length !== 1 ||
    recommendedProfiles[0]?.id !== generalDefault.id
  ) {
    throw new Error(
      "Merman WASM runtime resources must recommend exactly the binding default profile."
    );
  }
  return {
    ...structuredCloneValue(value),
    general_binding_default_profile: value.general_binding_default_profile,
    cli_default_profile: value.cli_default_profile,
    limits,
    profiles,
  };
}

function structuredCloneValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function normalizeTextMeasurementCapabilities(value: unknown): TextMeasurementCapabilities | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned invalid text measurement capabilities.");
  }
  assertRequiredRecordKeys(
    value,
    ["protocol_version", "provider_ids"],
    "Merman WASM text measurement capabilities"
  );
  if (
    typeof value.protocol_version !== "number" ||
    !isSafeCatalogInteger(value.protocol_version) ||
    value.protocol_version !== MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION
  ) {
    throw new Error("Merman WASM returned an unsupported text measurement protocol.");
  }
  const providerIds = normalizeSortedIdentifierIds(
    value.provider_ids,
    "text measurement provider IDs"
  );
  if (!providerIds.includes("vendored")) {
    throw new Error("Merman WASM text measurement must include vendored support.");
  }
  return {
    ...structuredCloneValue(value),
    protocol_version: value.protocol_version,
    provider_ids: providerIds,
  };
}

function normalizeStringMapEntries(
  value: unknown,
  label: string
): [string, unknown][] {
  const entries = value instanceof Map
    ? [...value.entries()]
    : isRecord(value)
      ? Object.entries(value)
      : null;
  if (entries === null || entries.some(([key]) => typeof key !== "string" || key.length === 0)) {
    throw new Error(`Merman WASM returned invalid ${label}.`);
  }
  return entries as [string, unknown][];
}

function normalizeSortedIdentifierIds(value: unknown, label: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Merman WASM returned invalid ${label}.`);
  }
  const identifiers = value.map((entry) => assertRuntimeIdentifier(entry, label));
  for (let index = 1; index < identifiers.length; index += 1) {
    if (identifiers[index - 1] >= identifiers[index]) {
      throw new Error(`Merman WASM ${label} must be sorted and unique.`);
    }
  }
  return identifiers;
}

function normalizeSortedOptionGroupIds(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw new Error("Merman WASM returned invalid runtime option group IDs.");
  }
  const identifiers = value.map((entry) =>
    assertRuntimeFieldIdentifier(entry, "runtime option group ID")
  );
  for (let index = 1; index < identifiers.length; index += 1) {
    if (identifiers[index - 1] >= identifiers[index]) {
      throw new Error("Merman WASM runtime option group IDs must be sorted and unique.");
    }
  }
  return identifiers;
}

function normalizeSortedPositiveIntegers(value: unknown, label: string): number[] {
  if (!Array.isArray(value)) {
    throw new Error(`Merman WASM returned invalid ${label}.`);
  }
  const integers = value.map((entry) => assertSafeIntegerField(entry, label, 1));
  for (let index = 1; index < integers.length; index += 1) {
    if (integers[index - 1] >= integers[index]) {
      throw new Error(`Merman WASM ${label} must be sorted and unique.`);
    }
  }
  return integers;
}

function assertRuntimeIdentifier(value: unknown, label: string): string {
  if (
    typeof value === "string" &&
    RUNTIME_CATALOG_IDENTIFIER.test(value)
  ) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertRuntimeFieldIdentifier(value: unknown, label: string): string {
  if (
    typeof value === "string" &&
    RUNTIME_CATALOG_FIELD_IDENTIFIER.test(value)
  ) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function sameStringArrays(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function assertSafeIntegerField(value: unknown, label: string, minimum: number): number {
  if (typeof value === "number" && isSafeCatalogInteger(value) && value >= minimum) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function isSafeCatalogInteger(value: number): boolean {
  return Number.isInteger(value) && Math.abs(value) <= RUNTIME_CATALOG_MAX_SAFE_INTEGER;
}

function assertRequiredRecordKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  label: string
): void {
  const missing = required.filter(
    (key) => !Object.prototype.hasOwnProperty.call(value, key)
  );
  if (missing.length !== 0) {
    throw new Error(`${label} is missing required fields: ${missing.join(", ")}.`);
  }
}
