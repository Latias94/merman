export type MermanResourceProfile =
  | "interactive"
  | "constrained"
  | "trusted-native"
  | "unbounded-for-trusted-input";

export interface MermanBindingOptions {
  version?: 3;
  runtime_policy?: "deterministic";
  fixed_today?: string | null;
  /** Exact host-supplied Unix milliseconds; mutually exclusive with a non-null fixed_today. */
  fixed_unix_millis?: number | null;
  fixed_local_offset_minutes?: number | null;
  environment?: {
    theme_portability?: "best-effort" | "require-portable";
    text_measurement?: "deterministic" | null;
    [key: string]: unknown;
  };
  resources?: {
    profile?: MermanResourceProfile;
    limits?: Record<string, number>;
  };
  /** A supplied selection replaces the complete constructor theme; null clears it. */
  theme?: DiagramThemeSelection | null;
  [key: string]: unknown;
}

export type ThemeAppearance = "light" | "dark";
export type ThemeJsonValue =
  | string | number | boolean | null
  | ThemeJsonValue[]
  | { [key: string]: ThemeJsonValue };
export type ThemeJsonObject = { [key: string]: ThemeJsonValue };

/** Editable JSON sections are validated by the shared Rust theme compiler. */
export interface ThemeDesignV1 {
  tokens?: Record<string, ThemeJsonValue>;
  styles?: Record<string, ThemeJsonValue>;
  rules?: ThemeJsonObject[];
  palettes?: Record<string, ThemeJsonValue>;
  canvas?: ThemeJsonObject;
  typography?: ThemeJsonObject;
  mermaid?: ThemeJsonObject;
  assets?: ThemeJsonObject;
  requirements?: ThemeJsonObject;
  appearances?: Partial<Record<ThemeAppearance, ThemeAppearanceOverridesV1>>;
  default_appearance?: ThemeAppearance;
}

export type ThemeAppearanceOverridesV1 =
  Omit<ThemeDesignV1, "appearances" | "default_appearance" | "assets" | "requirements">;

export type ThemeRecipeV1 = {
  schema_version: 1;
  preset?: never;
  spec?: never;
  recipe?: never;
  appearance?: never;
} & (
  | { kind: "design"; design: ThemeDesignV1; complete_spec?: never }
  | { kind: "complete_spec"; complete_spec: ThemeJsonObject; design?: never }
);

export type DiagramThemeSelection =
  | ThemeRecipeV1
  | ({ kind?: never; schema_version?: never; design?: never; complete_spec?: never } & (
      | { preset: string; appearance?: ThemeAppearance; recipe?: never; spec?: never }
      | { recipe: ThemeRecipeV1; appearance?: ThemeAppearance; preset?: never; spec?: never }
      | { spec: ThemeJsonObject; preset?: never; recipe?: never; appearance?: never }
    ));

export type ThemeAuthoringRequestV1 =
  | { schema_version: 1; action: "inspect"; recipe: ThemeRecipeV1; appearance?: never }
  | { schema_version: 1; action: "materialize"; recipe: ThemeRecipeV1; appearance?: ThemeAppearance };
export type ThemeAuthoringInput = ThemeDesignV1 | ThemeAuthoringRequestV1 | string;

export interface ThemeDesignReferenceUse {
  token: string;
  path: string;
  domain: ThemeJsonObject;
}

export interface ThemeRecipeReferenceUseV1 extends ThemeDesignReferenceUse {
  token_path: string;
}

export interface MaterializedThemeWireV1 {
  schema_version: 1;
  authoring_schema_version: 1;
  expansion_version: 1;
  spec_schema_version: 1;
  spec: ThemeJsonObject;
  references?: ThemeDesignReferenceUse[];
}

export interface ThemeRecipeInspectionV1 {
  schema_version: 1;
  recipe_schema_version: 1;
  supported_appearances: ThemeAppearance[];
  default_appearance: ThemeAppearance | null;
  appearances: {
    appearance: ThemeAppearance | null;
    references: ThemeRecipeReferenceUseV1[];
  }[];
}

export interface MaterializedThemeRecipeV1 {
  schema_version: 1;
  recipe_schema_version: 1;
  supported_appearances: ThemeAppearance[];
  default_appearance: ThemeAppearance | null;
  effective_appearance: ThemeAppearance | null;
  spec: ThemeJsonObject;
  references: ThemeRecipeReferenceUseV1[];
}

export type ThemeAuthoringResult =
  | MaterializedThemeWireV1 | ThemeRecipeInspectionV1 | MaterializedThemeRecipeV1;
export type ThemeAuthoringResultFor<Input extends ThemeAuthoringInput> =
  Input extends { action: "inspect" } ? ThemeRecipeInspectionV1
    : Input extends { action: "materialize" } ? MaterializedThemeRecipeV1
      : Input extends string ? ThemeAuthoringResult : MaterializedThemeWireV1;

export interface ThemeSupportQueryV1 {
  schema_version: 1;
  family: string;
  output: string;
  subject:
    | { kind: "rule"; target: string; facet: string }
    | { kind: "ordinal-palette"; target: string }
    | { kind: "base-typography"; property: string };
}

export interface ThemeCapabilityDescriptorV1 {
  schema_version: 1;
  claim_revision: number;
  query: ThemeSupportQueryV1;
  state: "unconditional" | "conditional" | "not-applicable" | "unsupported" | "unverified" | (string & {});
  reason_ids: string[];
}

/** One host-loaded Iconify pack, admitted once into an immutable engine-local registry. */
export interface MermanIconPack {
  json: string;
  registrationName?: string;
}

export interface CreateNodeEngineOptions {
  bindingOptions?: MermanBindingOptions;
  iconPacks?: readonly MermanIconPack[];
  concurrency?: number;
  maxQueue?: number;
}

export interface RenderSvgOptions {
  /**
   * Cancels queued work immediately and requests cooperative cancellation after Rust work starts.
   * A queued request rejects with AbortError; a started request rejects with MermanOperationError
   * carrying canonical cancellation details once transport admission or a renderer checkpoint
   * observes the request.
   */
  signal?: AbortSignal;
  /**
   * Relative monotonic deadline in milliseconds, starting when the Rust transport admits the
   * operation. Values must be integers from 0 through 4,294,967,295.
   */
  timeoutMs?: number;
  /** Raw per-request options JSON merged by the shared binding contract. */
  optionsJson?: string;
}

export interface MermanOperationRequest {
  operationId: string;
  source: string;
  uri?: string | null;
  /** Raw per-request options JSON merged over the reusable engine baseline. */
  optionsJson?: string;
}

export interface MermanOperationResult {
  operation_id: string;
  media_type: string;
  data: string;
  metadata_json: string;
}

export interface MermanTextMeasurementCatalog {
  protocol_version: number;
  provider_ids: string[];
  [key: string]: unknown;
}

export interface MermanRuntimeResourceLimit {
  id: string;
  phase: string;
  description: string;
  overridable: boolean;
  hard_cap: boolean;
  minimum_value: number;
  operation_ids: string[];
  [key: string]: unknown;
}

export interface MermanRuntimeResourceProfile {
  id: string;
  purpose: string;
  trust_assumption: string;
  recommended_binding_default: boolean;
  limits: Record<string, number | null>;
  [key: string]: unknown;
}

export interface MermanRuntimeSystemFontContract {
  source_id: string;
  discovery: string;
  cache_scope: string;
  host_dependent: boolean;
  caller_configurable: boolean;
  resource_bounded: boolean;
  [key: string]: unknown;
}

export interface MermanRuntimeEmbeddedImageLimits {
  max_bytes_per_image: number | null;
  max_total_bytes: number | null;
  max_pixels_per_image: number | null;
  max_total_pixels: number | null;
  [key: string]: unknown;
}

export interface MermanRuntimeEmbeddedImageContract {
  source_ids: string[];
  filesystem_access: boolean;
  network_access: boolean;
  caller_configurable: boolean;
  limits: MermanRuntimeEmbeddedImageLimits;
  [key: string]: unknown;
}

export interface MermanRuntimeOutputContract {
  id: string;
  media_type: string;
  system_fonts: MermanRuntimeSystemFontContract | null;
  embedded_images: MermanRuntimeEmbeddedImageContract | null;
  [key: string]: unknown;
}

export interface MermanRuntimePayloadSchema {
  id: string;
  version: number;
  [key: string]: unknown;
}

export interface MermanRuntimeConstructorResourceLimit {
  id: string;
  phase: string;
  unit: string;
  description: string;
  value: number;
  [key: string]: unknown;
}

export interface MermanRuntimeConstructorServiceContract {
  id: string;
  provided_text_measurement_provider_ids: string[];
  resource_limits: MermanRuntimeConstructorResourceLimit[];
  [key: string]: unknown;
}

export interface MermanRuntimeCatalog {
  schema_version: 1;
  transport_api_version: 1;
  package_version: string;
  options_schema_versions: number[];
  payload_schemas: MermanRuntimePayloadSchema[];
  metadata_ids: string[];
  option_group_ids: string[];
  constructor_service_ids: string[];
  constructor_service_contracts: MermanRuntimeConstructorServiceContract[];
  capabilities: {
    capability_ids: string[];
    output_ids: string[];
    operation_ids: string[];
    system_adapter_ids: string[];
    text_measurement: MermanTextMeasurementCatalog | null;
    [key: string]: unknown;
  };
  output_contracts: MermanRuntimeOutputContract[];
  registry: {
    diagram_family_count: number;
    diagram_families: string[];
    [key: string]: unknown;
  };
  resources: {
    general_binding_default_profile: string;
    cli_default_profile: string;
    limits: MermanRuntimeResourceLimit[];
    profiles: MermanRuntimeResourceProfile[];
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

export interface MermanQueueState {
  readonly active: number;
  readonly pending: number;
  readonly concurrency: number;
  readonly maxQueue: number;
  readonly state: "open" | "disposing" | "disposed";
}

export declare class MermanEngine {
  private constructor();
  readonly queueState: MermanQueueState;
  readonly runtimeCatalog: MermanRuntimeCatalog;
  renderSvg(source: string, options?: RenderSvgOptions): Promise<string>;
  /** Synchronous rendering is intended only for explicit SSG build paths. */
  renderSvgSync(source: string, options?: Omit<RenderSvgOptions, "signal">): string;
  svgPlanJson(source: string, options?: RenderSvgOptions): Promise<string>;
  svgPlanJsonSync(
    source: string,
    options?: Omit<RenderSvgOptions, "signal">,
  ): string;
  /** Raw strings preserve duplicate-field admission and source paths in Rust. */
  materializeTheme<Input extends ThemeAuthoringInput>(
    input: Input,
    options?: RenderSvgOptions,
  ): Promise<ThemeAuthoringResultFor<Input>>;
  describeThemeSupport(
    query: ThemeSupportQueryV1 | string,
    options?: RenderSvgOptions,
  ): Promise<ThemeCapabilityDescriptorV1>;
  /** Exports the complete editable design, including every declared appearance. */
  exportThemePreset(presetId: string, options?: RenderSvgOptions): Promise<ThemeRecipeV1>;
  metadataJson(id: string): string;
  executeOperation(
    request: MermanOperationRequest,
    options?: Pick<RenderSvgOptions, "signal" | "timeoutMs">,
  ): Promise<MermanOperationResult>;
  executeOperationSync(
    request: MermanOperationRequest,
    options?: Pick<RenderSvgOptions, "timeoutMs">,
  ): MermanOperationResult;
  dispose(): Promise<void>;
}

export declare function createNodeEngine(
  options?: CreateNodeEngineOptions,
): Promise<MermanEngine>;

export declare class MermanError extends Error {
  readonly code: string;
}

/**
 * A lossless unsigned resource count. Safe integers use `number`; wider `u64` values use a
 * canonical decimal `string`.
 */
export type MermanResourceCount = number | string;

export interface MermanResourceErrorDetails {
  readonly cause: string;
  readonly limit_id: string;
  readonly phase: string;
  readonly actual: MermanResourceCount;
  readonly max: MermanResourceCount;
  readonly profile: string;
}

export interface MermanDiagnosticSpan {
  readonly start: number;
  readonly end: number;
  readonly kind: "exact" | "insertion-point" | "fallback" | string;
}

export interface MermanDiagnosticErrorDetails {
  readonly code: string;
  readonly span: MermanDiagnosticSpan | null;
  readonly field: string | null;
  readonly diagram_type: string | null;
  readonly requested_max_width?: number | null;
  readonly actual_width?: number | null;
  readonly width_profile?: string | null;
  readonly fallback_reason?: string | null;
}

export interface MermanCancellationErrorDetails {
  readonly reason: "requested" | "deadline_exceeded";
  /** Renderer phase identifier; new Merman versions may add phases. */
  readonly phase: string;
  readonly [key: string]: unknown;
}

export interface MermanIconRegistryErrorDetails {
  readonly kind_id: string;
  readonly pack_index: number | null;
  readonly registration_name: string | null;
}

export declare class MermanOperationError extends MermanError {
  readonly status: number | null;
  readonly codeName: string | null;
  readonly kind: "generic" | "unknown-operation" | "missing-capability" | string;
  readonly capabilityId: string | null;
  /** Complete core-owned details, including versioned theme_authoring diagnostics. */
  readonly details: Readonly<Record<string, unknown>> | null;
  readonly resourceDetails: MermanResourceErrorDetails | null;
  readonly iconRegistryDetails: MermanIconRegistryErrorDetails | null;
  readonly diagnosticDetails: MermanDiagnosticErrorDetails | null;
  readonly cancellationDetails: MermanCancellationErrorDetails | null;
}

export declare class MermanQueueSaturatedError extends MermanError {
  readonly maxQueue: number;
}

export declare class MermanDisposedError extends MermanError {}
export declare class MermanInvalidTransportError extends MermanError {}
export declare class MermanLifecycleError extends MermanError {}
export declare class MermanMissingPlatformPackageError extends MermanError {
  readonly packageName: string;
  readonly target: string;
}
export declare class MermanNativeLoadError extends MermanError {
  readonly packageName: string;
  readonly target: string;
}
export declare class MermanUnsupportedTargetError extends MermanError {
  readonly platform: string;
  readonly arch: string;
  readonly libc: string | null;
}
