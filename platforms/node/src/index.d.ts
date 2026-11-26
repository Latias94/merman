export type MermanResourceProfile =
  | "interactive"
  | "constrained"
  | "trusted-native"
  | "unbounded-for-trusted-input";

export type MermanAsciiGraphPresentation = "strict" | "adaptive";

/** Options for the canonical `ascii` operation, also accepted through optionsJson. */
export interface MermanAsciiRenderOptions {
  /** Defaults to strict. Adaptive is Plain-only for Flowchart, Swimlane, and State. */
  graph_presentation?: MermanAsciiGraphPresentation;
  graphPresentation?: MermanAsciiGraphPresentation;
  /** Fixed routing space requires a flat Flowchart and strict presentation. */
  graph_routing_space?: "expand" | "fixed";
  graphRoutingSpace?: MermanAsciiRenderOptions["graph_routing_space"];
  /** State supports Canonical only; Auto requires a maximum width. */
  layout_profile?: "canonical" | "compact" | "auto";
  layoutProfile?: "canonical" | "compact" | "auto";
  charset?: "ascii" | "unicode";
  max_width?: number;
  maxWidth?: number;
  overflow?: "allow" | "fallback" | "error";
  color_mode?:
    | "plain" | "none"
    | "ansi16" | "ansi-16" | "ansi_16"
    | "ansi256" | "ansi-256" | "ansi_256"
    | "truecolor" | "true-color" | "true_color" | "html";
  colorMode?: MermanAsciiRenderOptions["color_mode"];
  [key: string]: unknown;
}

export interface MermanBindingOptions {
  version?: 4;
  runtime_policy?: "deterministic";
  fixed_today?: string | null;
  /** Exact host-supplied Unix milliseconds; mutually exclusive with a non-null fixed_today. */
  fixed_unix_millis?: number | null;
  fixed_local_offset_minutes?: number | null;
  environment?: {
    text_measurement?: "deterministic" | null;
    [key: string]: unknown;
  };
  resources?: {
    profile?: MermanResourceProfile;
    limits?: Record<string, number>;
  };
  /** A supplied selection replaces the constructor theme. Use `null` or `"none"` to clear it. */
  theme?: DiagramThemeSelection | null;
  /** Overrides the SVG canvas base with a hex color or `"transparent"`. */
  background?: string;
  ascii?: MermanAsciiRenderOptions;
  [key: string]: unknown;
}

/** A concrete appearance selected by the host. */
export type ThemeAppearance = "light" | "dark";
export type ThemeCategory = "reading" | "editor" | "art" | "custom";
export type ThemePrecedence = "host" | "source";

export interface ThemeCatalog {
  readonly schema_version: 2;
  readonly presets: readonly ThemePresetCatalogEntry[];
  readonly mermaid_themes: readonly string[];
  readonly families: readonly { id: string; capabilities: number }[];
}

export interface ThemePresetCatalogEntry {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly category: ThemeCategory;
  readonly appearances: readonly ThemeAppearance[];
  readonly default_appearance: ThemeAppearance;
  /** Semantic colors for the preset's default appearance. */
  readonly swatches: readonly [
    canvas: string, node: string, container: string,
    title: string, primaryText: string, firstDataFill: string,
  ];
}

export interface ThemeDocumentV2 {
  readonly $schema?: string;
  readonly schema_version: 2;
  readonly id?: string;
  readonly name?: string;
  readonly description?: string;
  readonly category?: ThemeCategory;
  readonly extends?: string;
  readonly default_appearance?: ThemeAppearance;
  readonly seed?: ThemeSeed;
  readonly roles?: Partial<ThemeRoles>;
  readonly typography?: Partial<ThemeTypography>;
  readonly form?: ThemeFormPatch;
  readonly appearances?: Readonly<Partial<Record<ThemeAppearance, ThemeAppearanceLayer>>>;
  readonly overrides?: readonly ThemeFamilyOverride[];
}

/** A preset ID, `none` to clear the selection, or an editable v2 document. */
export type DiagramThemeSelection =
  | string
  | (ThemeDocumentV2 & {
      readonly preset?: never;
      readonly document?: never;
      readonly appearance?: never;
      readonly precedence?: never;
    })
  | ({
      readonly schema_version?: never;
      readonly appearance?: ThemeAppearance;
      readonly precedence?: ThemePrecedence;
    } & (
      | { readonly preset: string; readonly document?: never }
      | { readonly document: ThemeDocumentV2; readonly preset?: never }
    ));

export type ThemeSeed = Readonly<Partial<Record<
  "canvas" | "ink" | "accent" | "surface" | "callout" | "info" | "success" | "warn" | "danger",
  string
>>>;

export type ThemePaint = string | {
  readonly kind: "linear_gradient";
  readonly angle_deg?: number;
  readonly stops: readonly { offset: number; color: string; opacity?: number }[];
};

export interface ThemeRoles {
  readonly canvas: ThemePaint;
  readonly "surface.node": ThemePaint;
  readonly "surface.container": ThemePaint;
  readonly "surface.callout": ThemePaint;
  readonly "surface.emphasis": ThemePaint;
  readonly "stroke.border": ThemePaint;
  readonly "stroke.connector": ThemePaint;
  readonly "stroke.guide": ThemePaint;
  readonly "text.primary": string;
  readonly "text.muted": string;
  readonly "text.title": string;
  readonly "text.callout": string;
  readonly "text.on_emphasis": string;
  readonly "status.info": string;
  readonly "status.success": string;
  readonly "status.warn": string;
  readonly "status.danger": string;
}

type ThemeTextureColor = { readonly color?: string; readonly opacity?: number };

export type ThemeTexture =
  | { readonly kind: "none" }
  | (ThemeTextureColor & {
      readonly kind: "grid";
      readonly step_px: number;
      readonly major_every?: number;
      readonly major_opacity?: number;
    })
  | (ThemeTextureColor & { readonly kind: "dots"; readonly step_px: number; readonly radius_px?: number })
  | (ThemeTextureColor & {
      readonly kind: "ruled";
      readonly line_height_px: number;
      readonly margin_rule_color?: string;
      readonly margin_rule_x_px?: number;
    })
  | (ThemeTextureColor & { readonly kind: "fibers"; readonly angle_deg?: number; readonly cross?: boolean })
  | (ThemeTextureColor & { readonly kind: "scanlines"; readonly step_px: number })
  | {
      readonly kind: "radial_wash";
      readonly color: string;
      readonly opacity?: number;
      readonly cx?: number;
      readonly cy?: number;
      readonly radius?: number;
    };

interface ThemeShadowLayer {
  readonly dx: number;
  readonly dy: number;
  readonly blur?: number;
  readonly color: string;
  readonly opacity?: number;
}

interface ThemeGlow {
  readonly kind: "glow";
  readonly color: string;
  readonly blur: number;
  readonly opacity?: number;
}

export type ThemeElevationSpec =
  | "flat" | "soft" | "crisp"
  | { readonly kind: "layered_stack"; readonly layers: readonly ThemeShadowLayer[] }
  | ThemeGlow;

export type ThemeElevation =
  | { readonly kind: "flat" }
  | { readonly kind: "shadows"; readonly layers: readonly Required<ThemeShadowLayer>[] }
  | Required<ThemeGlow>;

type ThemeFontRole = "sans" | "serif" | "mono";

export interface ThemeTypography {
  readonly sans: readonly string[];
  readonly mono: readonly string[];
  readonly serif: readonly string[];
  readonly body_family: ThemeFontRole;
  readonly title_family: ThemeFontRole;
  readonly code_family: ThemeFontRole;
  readonly base_size_px: number;
  readonly title_weight: number;
  readonly label_weight: number;
}

export interface ThemeForm {
  readonly density: "compact" | "balanced" | "airy";
  readonly radius: number;
  readonly container_radius: number;
  readonly stroke_width: number;
  readonly connector_width: number;
  readonly edge_style: "curved" | "orthogonal" | "straight" | "sketch";
  readonly marker_style: "stealth" | "filled" | "open" | "classic";
  readonly edge_label_bg: "auto" | "canvas" | "pill" | "transparent";
  readonly chart: {
    readonly gridlines: boolean;
    readonly bar_radius: number;
    readonly bar_wireframe: boolean;
    readonly line_markers: boolean;
    readonly pie_gap_px: number;
    readonly pie_opacity: number;
  };
}

export interface ThemeFormPatch extends Partial<Omit<ThemeForm, "chart">> {
  readonly elevation?: ThemeElevationSpec;
  readonly chart?: Partial<ThemeForm["chart"]>;
}

interface ThemeCanvas {
  readonly textures: readonly ThemeTexture[];
  readonly padding_px: number;
}

interface ThemeDataPalette {
  readonly ink: readonly string[];
  readonly fill: readonly string[];
  readonly fill_ink: readonly string[];
  readonly pattern_encoding: boolean;
}

export interface ThemeAppearanceLayer {
  readonly seed?: ThemeSeed;
  readonly roles?: Partial<ThemeRoles>;
  readonly data?: Partial<Omit<ThemeDataPalette, "fill_ink">> & { readonly fill_ink?: string };
  readonly canvas?: Partial<ThemeCanvas>;
  readonly elevation?: ThemeElevationSpec;
}

export interface ThemeFamilyOverride {
  readonly families: readonly string[];
  readonly appearance?: ThemeAppearance;
  readonly roles?: Partial<ThemeRoles>;
  readonly form?: ThemeFormPatch;
  readonly elevation?: ThemeElevationSpec;
  readonly typography?: Partial<ThemeTypography>;
  readonly node_ordinal_palette?: boolean;
}

interface ResolvedFamilyTheme {
  readonly roles: ThemeRoles;
  readonly data: ThemeDataPalette;
  readonly typography: ThemeTypography;
  readonly form: ThemeForm;
  readonly elevation: ThemeElevation;
  readonly node_ordinal_palette: boolean;
}

export interface ResolvedThemeV2 extends ResolvedFamilyTheme {
  readonly id: string | null;
  readonly appearance: ThemeAppearance;
  readonly canvas: ThemeCanvas;
  readonly family_overrides?: Readonly<Record<string, ResolvedFamilyTheme>>;
}

export type ThemeNotice =
  | {
      readonly kind: "low_contrast";
      readonly foreground: string;
      readonly background: string;
      readonly ratio: number;
      readonly minimum: number;
    }
  | { readonly kind: "contrast_adjusted"; readonly foreground: string; readonly color: string };

export interface ThemeOutcome {
  readonly mode: "mermaid_parity" | "merman_native";
  readonly preset_id?: string;
  readonly appearance?: ThemeAppearance;
  readonly notices: readonly (ThemeNotice | string)[];
}

export interface ThemeResolveOptions extends Pick<RenderSvgOptions, "signal" | "timeoutMs"> {
  readonly appearance?: ThemeAppearance;
  readonly auto_fix_contrast?: boolean;
}

export interface ThemeResolveError {
  readonly ok: false;
  readonly line: number | null;
  readonly column: number | null;
  readonly path: string | null;
  readonly message: string;
}

export type ThemeResolution =
  | { readonly ok: true; readonly resolved: ResolvedThemeV2; readonly notices: readonly ThemeNotice[] }
  | ThemeResolveError;

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
  /** Request-local SVG ID. Use optionsJson when supplying other request overrides. */
  diagramId?: string;
  /** Raw per-request options JSON merged by the shared binding contract. */
  optionsJson?: string;
}

type SvgPlanOptions = Omit<RenderSvgOptions, "diagramId"> & { diagramId?: never };

export interface MermanOperationRequest {
  operationId: string;
  source: string;
  uri?: string | null;
  /**
   * Raw per-request options JSON merged over the reusable engine baseline.
   * For ASCII, use operationId "ascii" and JSON.stringify({ ascii: { graph_presentation: "adaptive" } }
   * satisfies MermanBindingOptions). Admission is advertised by metadataJson("ascii-capabilities").
   */
  optionsJson?: string;
}

export interface MermanOperationResult {
  operation_id: string;
  media_type: string;
  data: string;
  /** Canonical JSON for `MermanOperationMetadata`. */
  metadata_json: string;
}

export interface MermanOperationMetadata {
  readonly version: 1;
  readonly operation_id: string;
  readonly media_type: string;
  readonly runtime_policy: "deterministic";
  readonly byte_length: number;
  readonly theme_outcome?: ThemeOutcome;
  readonly [key: string]: unknown;
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
  transport_api_version: 3;
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
  svgPlanJson(source: string, options?: SvgPlanOptions): Promise<string>;
  svgPlanJsonSync(
    source: string,
    options?: Omit<SvgPlanOptions, "signal">,
  ): string;
  themeCatalog(): ThemeCatalog;
  /** Exports the editable v2 document, including every declared appearance. */
  exportThemePreset(
    presetId: string,
    options?: Pick<RenderSvgOptions, "signal" | "timeoutMs">,
  ): Promise<ThemeDocumentV2>;
  /** Pass raw JSON text to preserve document-relative syntax and duplicate-key diagnostics. */
  resolveTheme(
    document: ThemeDocumentV2 | string,
    options?: ThemeResolveOptions,
  ): Promise<ThemeResolution>;
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
  /** Complete core-owned details. */
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
