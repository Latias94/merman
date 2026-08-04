import {
  createMermanRuntimeState,
  currentMermanRuntimeState,
  type MermanRuntimeState,
} from "./runtime-state.js";
import {
  MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
} from "./generated/text-measurement-abi.js";
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
  if (!isRecord(value) || value.schema_version !== 1) {
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
    schema_version: 1,
    transport_api_version: catalogTransportApiVersion,
    package_version: value.package_version,
    options_schema_versions: optionsSchemaVersions,
    payload_schemas: normalizeRuntimePayloadSchemas(value.payload_schemas),
    metadata_ids: normalizeSortedIdentifierIds(value.metadata_ids, "runtime metadata IDs"),
    capabilities,
    output_contracts: normalizeRuntimeOutputContracts(
      value.output_contracts,
      capabilities.output_ids
    ),
    registry: { diagram_family_count: diagramFamilyCount },
    resources: normalizeRuntimeResourceContract(
      value.resources,
      new Set(capabilities.operation_ids)
    ),
  };
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
      id: assertRuntimeIdentifier(entry.id, "runtime payload schema IDs"),
      version: assertSafeIntegerField(entry.version, "runtime payload schema version", 1),
    };
  });
  for (let index = 1; index < schemas.length; index += 1) {
    if (schemas[index - 1].id >= schemas[index].id) {
      throw new Error("Merman WASM runtime payload schema IDs must be sorted and unique.");
    }
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
    limits,
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
  const operationSet = new Set(operationIds);
  const outputIds = normalizeSortedIdentifierIds(value.output_ids, "runtime output IDs");
  for (const outputId of outputIds) {
    if (!operationSet.has(outputId)) {
      throw new Error(
        `Merman WASM runtime output ${outputId} is absent from runtime binding operation IDs.`
      );
    }
  }

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

  const textMeasurement = normalizeTextMeasurementCapabilities(value.text_measurement);
  const svgAvailable = capabilitySet.has("svg");
  if (svgAvailable !== (textMeasurement !== null)) {
    throw new Error(
      "Merman WASM text measurement must be present exactly when SVG is available."
    );
  }

  return {
    capability_ids: capabilityIds,
    output_ids: outputIds,
    operation_ids: operationIds,
    system_adapter_ids: systemAdapterIds,
    text_measurement: textMeasurement,
  };
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
      typeof limit.phase !== "string" ||
      typeof limit.description !== "string" ||
      typeof limit.overridable !== "boolean" ||
      typeof limit.hard_cap !== "boolean" ||
      !Number.isSafeInteger(limit.minimum_value) ||
      (limit.minimum_value as number) < 0
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
    !Number.isSafeInteger(value.protocol_version) ||
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
    /^[a-z0-9][a-z0-9-]*$/.test(value)
  ) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertSafeIntegerField(value: unknown, label: string, minimum: number): number {
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= minimum) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
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
