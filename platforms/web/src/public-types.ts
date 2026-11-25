import type {
  AsciiCapability,
  AsciiGraphPresentation,
  AsciiLayoutProfile,
  AsciiOutputEncoding,
  AsciiWidthProfile,
  BindingStatusCodeName,
  DiagramFamilyCapability,
  DiagramType,
  LintBindingOptions,
  LintRuleCatalogResponse,
  LintRuleCategory,
  LintRuleSeverity,
  LintRuleTag,
  RuntimeCapabilities,
} from "./public-catalog.js";
import type {
  HostTextDirection,
  HostTextMeasurementOperation,
  HostTextMeasurementPhase,
  HostTextWhiteSpace,
  HostTextWrapMode,
} from "./generated/text-measurement-abi.js";
import type { EditorRenamePolicy } from "./generated/editor-rename-policy.js";
import type {
  BINDING_OPTIONS_SCHEMA_VERSION,
  ResourceOverrideId,
  ResourceLimitId,
  ResourceOptions,
  ResourceProfile,
} from "./generated/resource-contract.js";
import type {
  DiagramThemeSelection,
  ThemeCatalog,
  ThemeDocumentV2,
  ThemeOutcome,
  ThemeResolution,
} from "./theme-types.js";

export type {
  HostTextDirection,
  HostTextMeasurementOperation,
  HostTextMeasurementPhase,
  HostTextMeasurementResultKind,
  HostTextWhiteSpace,
  HostTextWrapMode,
} from "./generated/text-measurement-abi.js";
export type { ResourceOptions } from "./generated/resource-contract.js";
export type * from "./theme-types.js";

export interface ParseOptions {
  suppress_errors?: boolean;
}

/** Opaque layout hints from a successful Shoal SVG result. Keep them in the owning document session. */
export type ShoalLayoutFrame = Readonly<Record<string, unknown>>;

export interface LayoutOptions {
  /** Explicit width fitting for Kanban or a Flowchart/Agentflow using Shoal. */
  adaptation?: "none" | "fit-width";
  container_width?: number;
  container_height?: number;
  screen_available_width?: number;
  /** Allow Shoal fit-width to fold wide leaf ranks. Defaults to false. */
  fold_wide_ranks?: boolean;
  /** Include a frame in the result metadata when rendering with Shoal. */
  capture_shoal_frame?: boolean;
  /** Replay the previous successful frame for this document and capture a new one. */
  previous_shoal_frame?: ShoalLayoutFrame;
}

export interface RenderEnvironmentOptions {
  text_measurement?: "deterministic" | null;
  math_renderer?: "none" | "ratex";
}

/**
 * Current capabilities and policies exposed by the loaded browser artifact.
 *
 * Stable ID arrays are sorted and unique. Consumers must tolerate IDs introduced by newer
 * artifacts and should make decisions only from the relations present in this catalog.
 */
export interface RuntimeCatalog {
  [key: string]: unknown;
  schema_version: 1;
  transport_api_version: number;
  package_version: string;
  options_schema_versions: number[];
  payload_schemas: RuntimePayloadSchema[];
  metadata_ids: string[];
  option_group_ids: string[];
  constructor_service_ids: string[];
  constructor_service_contracts: RuntimeConstructorServiceContract[];
  capabilities: RuntimeCapabilities;
  output_contracts: RuntimeOutputContract[];
  registry: RuntimeRegistryContract;
  resources: RuntimeResourceContract;
}

export interface RuntimePayloadSchema {
  [key: string]: unknown;
  id: string;
  version: number;
}

export interface RuntimeRegistryContract {
  [key: string]: unknown;
  diagram_family_count: number;
  diagram_families: string[];
}

export interface RuntimeConstructorServiceContract {
  [key: string]: unknown;
  id: string;
  provided_text_measurement_provider_ids: string[];
  resource_limits: RuntimeConstructorResourceLimit[];
}

export interface RuntimeConstructorResourceLimit {
  [key: string]: unknown;
  id: string;
  phase: string;
  unit: string;
  description: string;
  value: number;
}

export interface RuntimeOutputContract {
  [key: string]: unknown;
  id: string;
  media_type: string;
  system_fonts: RuntimeSystemFontContract | null;
  embedded_images: RuntimeEmbeddedImageContract | null;
}

export interface RuntimeSystemFontContract {
  [key: string]: unknown;
  source_id: string;
  discovery: string;
  cache_scope: string;
  host_dependent: boolean;
  caller_configurable: boolean;
  resource_bounded: boolean;
}

export interface RuntimeEmbeddedImageContract {
  [key: string]: unknown;
  source_ids: string[];
  filesystem_access: boolean;
  network_access: boolean;
  caller_configurable: boolean;
  limits: RuntimeEmbeddedImageLimits;
}

export interface RuntimeEmbeddedImageLimits {
  [key: string]: unknown;
  max_bytes_per_image: number | null;
  max_total_bytes: number | null;
  max_pixels_per_image: number | null;
  max_total_pixels: number | null;
}

export interface RuntimeResourceContract {
  [key: string]: unknown;
  general_binding_default_profile: string;
  cli_default_profile: string;
  limits: RuntimeResourceLimit[];
  profiles: RuntimeResourceProfile[];
}

export interface RuntimeResourceLimit {
  [key: string]: unknown;
  id: ResourceLimitId;
  phase: string;
  description: string;
  overridable: boolean;
  hard_cap: boolean;
  minimum_value: number;
  operation_ids: string[];
}

export interface RuntimeResourceProfile {
  [key: string]: unknown;
  id: string;
  purpose: string;
  trust_assumption: string;
  recommended_binding_default: boolean;
  limits: Record<ResourceLimitId, number | null>;
}

export interface SvgOptions {
  diagram_id?: string;
  pipeline?: "parity" | "static" | "readable" | "resvg-safe";
  root_background_color?: string;
  drop_native_duplicate_fallbacks?: boolean;
  viewbox_padding?: number;
  viewBoxPadding?: number;
}

export interface MermaidSiteConfig {
  [key: string]: unknown;
  themeCSS?: never;
  secure?: never;
}

export interface AnalysisBindingOptions {
  fixed_today?: string | null;
  /** Exact host-supplied Unix milliseconds; mutually exclusive with a non-null fixed_today. */
  fixed_unix_millis?: number | null;
  fixed_local_offset_minutes?: number | null;
  site_config?: MermaidSiteConfig;
  resources?: ResourceOptions;
  lint?: LintBindingOptions;
}

export type EditorResourceProfile = Extract<
  ResourceProfile,
  "interactive" | "constrained"
>;
export type EditorResourceLimitId = Extract<
  ResourceOverrideId,
  "max_source_bytes" | "max_document_diagrams"
>;
export type EditorResourceLimitOverrides = Partial<
  Record<EditorResourceLimitId, number>
>;

export interface EditorResourceOptions {
  profile?: EditorResourceProfile;
  limits?: EditorResourceLimitOverrides;
}

export type EditorAnalysisBindingOptions = Omit<
  AnalysisBindingOptions,
  "resources"
> & {
  resources?: EditorResourceOptions;
};

interface BindingVersionOptions {
  version?: typeof BINDING_OPTIONS_SCHEMA_VERSION;
}

type NoDirectAnalysisBindingOptions<Options extends Omit<AnalysisBindingOptions, "resources">> = {
  [Property in keyof Options]?: never;
};

type DirectAnalysisBindingRoot<Options extends Omit<AnalysisBindingOptions, "resources">> = Options & {
  analysis?: never;
  merman?: never;
};

type AnalysisWrappedBindingRoot<Options extends Omit<AnalysisBindingOptions, "resources">> =
  NoDirectAnalysisBindingOptions<Options> & {
  analysis: Options;
  merman?: never;
};

type MermanWrappedBindingRoot<Options extends Omit<AnalysisBindingOptions, "resources">> =
  NoDirectAnalysisBindingOptions<Options> & {
  analysis?: never;
  merman: Options;
};

type AnalysisBindingRoot<Options extends Omit<AnalysisBindingOptions, "resources">> =
  (
    | DirectAnalysisBindingRoot<Options>
    | AnalysisWrappedBindingRoot<Options>
    | MermanWrappedBindingRoot<Options>
  );

export type EditorBindingOptions = BindingVersionOptions &
  AnalysisBindingRoot<EditorAnalysisBindingOptions>;

interface CommonBindingFields {
  parse?: ParseOptions;
}

export type CommonBindingOptions = BindingVersionOptions &
  AnalysisBindingRoot<AnalysisBindingOptions> &
  CommonBindingFields;

export type AsciiCharsetOption = "ascii" | "unicode";
export type AsciiWidthProfileOption = "unicode" | "cjk";
export type AsciiLayoutProfileOption = "canonical" | "compact" | "auto";
export type AsciiGraphPresentationOption = AsciiGraphPresentation;
export type AsciiOverflowOption = "allow" | "fallback" | "error";
export type AsciiDirectionOption =
  | "lr"
  | "leftRight"
  | "left-right"
  | "left_right"
  | "td"
  | "tb"
  | "topDown"
  | "top-down"
  | "top_down";
export type AsciiColorModeOption =
  | "plain"
  | "none"
  | "ansi16"
  | "ansi-16"
  | "ansi_16"
  | "ansi256"
  | "ansi-256"
  | "ansi_256"
  | "truecolor"
  | "true-color"
  | "true_color"
  | "html";

export interface AsciiThemeOptions {
  foreground?: string;
  fg?: string;
  background?: string;
  bg?: string;
  line?: string;
  accent?: string;
  muted?: string;
  surface?: string;
  border?: string;
}

export interface AsciiRenderOptions {
  charset?: AsciiCharsetOption;
  max_width?: number;
  maxWidth?: number;
  overflow?: AsciiOverflowOption;
  trim_trailing_spaces?: boolean;
  trimTrailingSpaces?: boolean;
  /** Auto requires max_width; State supports Canonical only. */
  layout_profile?: AsciiLayoutProfileOption;
  layoutProfile?: AsciiLayoutProfileOption;
  /** Defaults to strict. Adaptive recovery is Plain-only for Flowchart, Swimlane, and State. */
  graph_presentation?: AsciiGraphPresentationOption;
  graphPresentation?: AsciiGraphPresentationOption;
  /** Fixed routing space requires a flat Flowchart and strict presentation. */
  graph_routing_space?: "expand" | "fixed";
  graphRoutingSpace?: AsciiRenderOptions["graph_routing_space"];
  width_profile?: AsciiWidthProfileOption;
  widthProfile?: AsciiWidthProfileOption;
  default_direction?: AsciiDirectionOption;
  defaultDirection?: AsciiDirectionOption;
  color_mode?: AsciiColorModeOption;
  colorMode?: AsciiColorModeOption;
  theme?: AsciiThemeOptions;
  sequence_mirror_actors?: boolean;
  sequenceMirrorActors?: boolean;
  /** Fallback node padding for both axes; defaults to 1. Zero is allowed. */
  box_border_padding?: number;
  boxBorderPadding?: number;
  /** Horizontal Flowchart/Swimlane/State node padding; overrides box_border_padding. */
  node_padding_x?: number;
  nodePaddingX?: number;
  /** Vertical Flowchart/Swimlane/State node padding; overrides box_border_padding. */
  node_padding_y?: number;
  nodePaddingY?: number;
  /** Preferred graph column gap; required geometry can increase it. */
  graph_padding_x?: number;
  graphPaddingX?: number;
  /** Preferred graph row gap; required geometry can increase it. */
  graph_padding_y?: number;
  graphPaddingY?: number;
  flowchart_node_label_wrap_width?: number;
  flowchartNodeLabelWrapWidth?: number;
  sequence_participant_spacing?: number;
  sequenceParticipantSpacing?: number;
  sequence_message_spacing?: number;
  sequenceMessageSpacing?: number;
  sequence_self_message_width?: number;
  sequenceSelfMessageWidth?: number;
  xychart_vertical_plot_height?: number;
  xychartVerticalPlotHeight?: number;
  xychart_category_band_width?: number;
  xychartCategoryBandWidth?: number;
  xychart_horizontal_plot_width?: number;
  xychartHorizontalPlotWidth?: number;
  relation_summary_diagnostics?: boolean;
  relationSummaryDiagnostics?: boolean;
}

interface AsciiBindingFields {
  /** Native theme colors for truecolor and ANSI256 output. */
  theme?: DiagramThemeSelection | null;
  ascii?: AsciiRenderOptions;
  /** Relative deadline for this Web/WASM operation, in milliseconds. */
  timeout_ms?: number;
}

export type AsciiBindingOptions = CommonBindingOptions & AsciiBindingFields;

interface SvgBindingFields {
  /** Relative deadline for this Web/WASM operation, in milliseconds. */
  timeout_ms?: number;
  theme?: DiagramThemeSelection | null;
  /** Override the diagram canvas with a hex color or `transparent`. */
  background?: string;
  environment?: RenderEnvironmentOptions;
  layout?: LayoutOptions;
  svg?: SvgOptions;
}

export type SvgBindingOptions = CommonBindingOptions & SvgBindingFields;

/** Already loaded Iconify JSON; acquisition and credentials belong to the host. */
export interface LoadedIconPack {
  json: string;
  registrationName?: string;
}

export interface RenderSessionOptions {
  iconPacks?: readonly LoadedIconPack[];
}

/** Immutable constructor services, scoped to this session and its creating package runtime. */
export interface BrowserRenderSession {
  renderSvg(source: string, options?: SvgBindingOptions | string): string;
  renderSvgResult(source: string, options?: SvgBindingOptions | string): SvgRenderResult;
  layoutJson(source: string, options?: SvgBindingOptions | string): string;
  edgeGeometryJson(source: string, options?: SvgBindingOptions | string): string;
  renderSvgWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string;
  renderSvgResultWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): SvgRenderResult;
  layoutJsonWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string;
  edgeGeometryJsonWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string;
  dispose(): void;
}

export interface WasmRenderSessionBinding {
  renderSvg(source: string, optionsJson?: string | null): string;
  renderSvgResult(source: string, optionsJson?: string | null): SvgRenderResult;
  layoutJson(source: string, optionsJson?: string | null): string;
  edgeGeometryJson(source: string, optionsJson?: string | null): string;
  renderSvgWithTextMeasurer(source: string, optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  renderSvgResultWithTextMeasurer(source: string, optionsJson: string | null | undefined, measurer: HostTextMeasurer): SvgRenderResult;
  layoutJsonWithTextMeasurer(source: string, optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  edgeGeometryJsonWithTextMeasurer(source: string, optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  free(): void;
}

export interface WasmRenderSessionConstructor {
  new (iconPacksJson?: string | null): WasmRenderSessionBinding;
}

/** Output settings for a prepared source with frozen parse configuration. */
export type PreparedSourceOutputOptions = BindingVersionOptions & {
  /** Relative deadline for this output, in milliseconds. */
  timeout_ms?: number;
  resources?: ResourceOptions;
  /** Omit to inherit the prepared theme. Use `none` or null to clear it for this output. */
  theme?: DiagramThemeSelection | null;
  background?: string;
  environment?: RenderEnvironmentOptions;
  layout?: LayoutOptions;
  svg?: SvgOptions;
};

export type HostTextMeasurerPreparedSourceOutputOptions = Omit<PreparedSourceOutputOptions, "environment"> & {
  environment?: Omit<RenderEnvironmentOptions, "text_measurement">;
};

export interface BrowserPreparedSource {
  renderSvg(options?: PreparedSourceOutputOptions | string): string;
  renderSvgResult(options?: PreparedSourceOutputOptions | string): SvgRenderResult;
  layoutJson(options?: PreparedSourceOutputOptions | string): string;
  edgeGeometryJson(options?: PreparedSourceOutputOptions | string): string;
  renderSvgWithTextMeasurer(measurer: HostTextMeasurer, options?: HostTextMeasurerPreparedSourceOutputOptions | string): string;
  renderSvgResultWithTextMeasurer(measurer: HostTextMeasurer, options?: HostTextMeasurerPreparedSourceOutputOptions | string): SvgRenderResult;
  layoutJsonWithTextMeasurer(measurer: HostTextMeasurer, options?: HostTextMeasurerPreparedSourceOutputOptions | string): string;
  edgeGeometryJsonWithTextMeasurer(measurer: HostTextMeasurer, options?: HostTextMeasurerPreparedSourceOutputOptions | string): string;
  dispose(): void;
}

export interface WasmPreparedSourceBinding {
  renderSvg(optionsJson?: string | null): string;
  renderSvgResult(optionsJson?: string | null): SvgRenderResult;
  layoutJson(optionsJson?: string | null): string;
  edgeGeometryJson(optionsJson?: string | null): string;
  renderSvgWithTextMeasurer(optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  renderSvgResultWithTextMeasurer(optionsJson: string | null | undefined, measurer: HostTextMeasurer): SvgRenderResult;
  layoutJsonWithTextMeasurer(optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  edgeGeometryJsonWithTextMeasurer(optionsJson: string | null | undefined, measurer: HostTextMeasurer): string;
  free(): void;
}

export interface WasmPreparedSourceConstructor {
  new (source: string, optionsJson?: string | null): WasmPreparedSourceBinding;
}


type HostTextMeasurerEnvironmentOptions = Omit<
  RenderEnvironmentOptions,
  "text_measurement"
> & {
  text_measurement?: never;
};

export type HostTextMeasurerSvgBindingOptions =
  CommonBindingOptions &
  Omit<SvgBindingFields, "environment"> & {
    environment?: HostTextMeasurerEnvironmentOptions;
  };

export type BindingOptions = SvgBindingOptions;

export interface HostTextMeasureRequest {
  operation: HostTextMeasurementOperation;
  phase: HostTextMeasurementPhase;
  text: string;
  font_family?: string | null;
  font_size: number;
  font_weight?: string | null;
  font_style: string;
  max_width?: number | null;
  has_max_width: boolean;
  line_height: number;
  letter_spacing: number;
  word_spacing: number;
  wrap_mode: HostTextWrapMode;
  direction: HostTextDirection;
  white_space: HostTextWhiteSpace;
}

export interface HostTextMetricsResult {
  handled?: true;
  kind: "metrics";
  width: number;
  height: number;
  line_count: number;
}

export interface HostTextLengthResult {
  handled?: true;
  kind: "length";
  length: number;
}

export interface HostTextHorizontalExtentsResult {
  handled?: true;
  kind: "horizontal-extents";
  bbox_left: number;
  bbox_right: number;
}

export interface HostTextWrappedWithRawWidthResult {
  handled?: true;
  kind: "wrapped-with-raw-width";
  width: number;
  height: number;
  line_count: number;
  raw_width?: number | null;
}

export interface HostTextUnhandledResult {
  handled: false;
}

export type HostTextMeasureResult =
  | HostTextMetricsResult
  | HostTextLengthResult
  | HostTextHorizontalExtentsResult
  | HostTextWrappedWithRawWidthResult
  | HostTextUnhandledResult;

export type HostTextMeasurer = (
  request: HostTextMeasureRequest
) => HostTextMeasureResult | null | undefined;

export interface BrowserTextMeasurementSession {
  readonly measure: HostTextMeasurer;
  dispose(): void;
}


export interface ValidationResult {
  valid: boolean;
  error?: string;
  code: number;
  code_name: BindingStatusCodeName;
}

export type AnalysisSourceKind = "diagram" | "markdown" | "mdx";

export interface AnalysisSource {
  kind: AnalysisSourceKind;
  path?: string | null;
  diagram_index?: number | null;
  language: string;
}

export interface AnalysisSummary {
  errors: number;
  warnings: number;
  infos: number;
  hints: number;
}

export interface AnalysisUtf16Position {
  line: number;
  character: number;
}

export interface AnalysisLspRange {
  start: AnalysisUtf16Position;
  end: AnalysisUtf16Position;
}

export interface AnalysisSpan {
  byte_start: number;
  byte_end: number;
  line: number;
  column: number;
  end_line: number;
  end_column: number;
  lsp_range: AnalysisLspRange;
}

export interface AnalysisDiagnosticRelated {
  message: string;
  span?: AnalysisSpan | null;
}

export interface AnalysisDiagnosticFixEdit {
  span: AnalysisSpan;
  replacement: string;
}

export interface AnalysisDiagnosticFix {
  title: string;
  edits: AnalysisDiagnosticFixEdit[];
  is_preferred?: boolean;
}

export interface AnalysisDiagnostic {
  id: string;
  severity: LintRuleSeverity;
  category: LintRuleCategory | string;
  tags?: LintRuleTag[];
  message: string;
  code?: number | null;
  code_name?: string | null;
  diagram_type?: string | null;
  span?: AnalysisSpan | null;
  related: AnalysisDiagnosticRelated[];
  help?: string | null;
  fixes?: AnalysisDiagnosticFix[];
}

interface AnalysisPayloadFields {
  valid: boolean;
  summary: AnalysisSummary;
  source: AnalysisSource;
  diagnostics: AnalysisDiagnostic[];
}

export interface AnalysisResult extends AnalysisPayloadFields {
  version: 1;
}

export interface AnalysisByteSpan {
  start: number;
  end: number;
}

export interface AnalysisFactSpan {
  local: AnalysisByteSpan;
  document?: AnalysisSpan | null;
}

export type AnalysisDiagramKind = "whole_document" | "mermaid_fence" | string;

export type AnalysisFenceMarker = "backtick" | "tilde" | "colon" | string;

export interface AnalysisFenceDelimiterFacts {
  marker: AnalysisFenceMarker;
  len: number;
}

export type AnalysisEditorSymbolKind =
  | "class"
  | "event"
  | "function"
  | "module"
  | "namespace"
  | "object"
  | "package"
  | "property"
  | "string"
  | "struct"
  | "variable"
  | string;

export type AnalysisSemanticRole =
  | "entity"
  | "class_definition"
  | "reference"
  | "outline"
  | "payload"
  | string;

export type AnalysisRenamePolicy = EditorRenamePolicy;

export type AnalysisExpectedSyntaxKind =
  | "directive"
  | "frontmatter"
  | "id_list"
  | "node_identifier"
  | "class_name"
  | "operator"
  | "shape"
  | "shape_trigger"
  | "direction"
  | "style_value"
  | "interaction_action"
  | "payload"
  | string;

export interface AnalysisReferenceFacts {
  name: string;
  kind: AnalysisEditorSymbolKind;
  spans: AnalysisFactSpan[];
}

export interface AnalysisLineItemFacts {
  name: string;
  detail?: string | null;
  kind: AnalysisEditorSymbolKind;
  span: AnalysisFactSpan;
  selection: AnalysisFactSpan;
}

export interface AnalysisSemanticItemFacts extends AnalysisLineItemFacts {
  role: AnalysisSemanticRole;
  rename_policy: AnalysisRenamePolicy;
}

export interface AnalysisExpectedSyntaxFacts {
  kind: AnalysisExpectedSyntaxKind;
  span: AnalysisFactSpan;
}

export interface AnalysisDiagramSyntaxFacts {
  diagram_type?: string | null;
  effective_layout?: string | null;
  fact_source: EditorSemanticFactSource;
  parser_backed: boolean;
  recovered: boolean;
  source_mapped_spans: boolean;
  node_ids: string[];
  class_names: string[];
  directive_prefixes: string[];
  references: AnalysisReferenceFacts[];
  outline_items: AnalysisLineItemFacts[];
  semantic_items: AnalysisSemanticItemFacts[];
  expected_syntax: AnalysisExpectedSyntaxFacts[];
}

export type DiagramParseDisposition = "parsed" | "recovered" | "unavailable";

export interface SourceAppearanceDeclaration {
  readonly value: string;
  readonly origin: { readonly kind: "frontmatter" } | { readonly kind: "directive"; readonly directive_index: number };
  readonly construct_span: Readonly<AnalysisByteSpan>;
}

export type SourceAppearanceSummary =
  | { readonly state: "unknown" }
  | {
      readonly state: "known";
      readonly theme: SourceAppearanceDeclaration | null;
      readonly font: SourceAppearanceDeclaration | null;
      readonly look: SourceAppearanceDeclaration | null;
    };

export interface AnalysisDiagramFacts {
  source_id: string;
  index: number;
  kind: AnalysisDiagramKind;
  source: AnalysisSource;
  span?: AnalysisSpan | null;
  body_span?: AnalysisSpan | null;
  text_len: number;
  fence_delimiter?: AnalysisFenceDelimiterFacts | null;
  parse_disposition: DiagramParseDisposition;
  syntax: AnalysisDiagramSyntaxFacts;
  appearance: SourceAppearanceSummary;
}

export interface AnalysisFactsResult extends AnalysisPayloadFields {
  version: 3;
  diagrams: AnalysisDiagramFacts[];
}

/** Actual family placement width, before SVG viewport scaling. */
export interface LayoutAdaptationObservation {
  mode: "fit-width";
  requested_width: number;
  actual_width: number;
  overflowed: boolean;
}

/** Canonical binding metadata for the output returned by the same execution. */
export interface BindingOperationMetadata {
  version: 1;
  operation_id: string;
  media_type: string;
  runtime_policy: string;
  byte_length: number;
  [key: string]: unknown;
}

/** Existing binding metadata for the exact SVG returned by the same execution. */
export interface SvgRenderMetadata extends BindingOperationMetadata {
  operation_id: "svg";
  media_type: "image/svg+xml";
  theme_outcome: ThemeOutcome;
  layout_adaptation?: LayoutAdaptationObservation;
  shoal_layout_frame?: ShoalLayoutFrame;
}

export interface SvgRenderResult {
  svg: string;
  metadata: SvgRenderMetadata;
}

/** Schema 4 report from the canonical ASCII operation; dimensions use terminal display cells. */
export interface AsciiOutputPlan {
  kind: "ascii";
  schema_version: 4;
  family: string;
  projection: "diagrammatic" | "structured_text";
  encoding: AsciiOutputEncoding;
  /** The four primary fields are all null when unmeasured; a valid empty primary retains zeroes. */
  primary_width: number | null;
  primary_height: number | null;
  layout_profile: Exclude<AsciiLayoutProfile, "auto"> | null;
  primary_overflowed: boolean | null;
  emitted_width: number;
  emitted_height: number;
  emitted_overflowed: boolean;
  width_profile: AsciiWidthProfile;
  requested_layout_profile: AsciiLayoutProfile;
  requested_graph_presentation: AsciiGraphPresentation;
  compact_attempted: boolean;
  requested_max_width: number | null;
  outcome: "primary" | "wide_allowed" | "fallback" | "empty";
  fallback_capability: "available" | "unsupported";
  fallback_attempted: boolean;
  fallback_reason: "primary_overflow" | "routing_search_exhausted" | "port_capacity_exceeded" | null;
  trimmed: boolean;
  lossiness: "none" | "presentation_only";
}

export interface AsciiRenderMetadata extends BindingOperationMetadata {
  operation_id: "ascii";
  media_type: "text/plain; charset=utf-8";
  output_plan: AsciiOutputPlan;
}

export interface AsciiRenderResult {
  text: string;
  metadata: AsciiRenderMetadata;
}

/**
 * Capability plan for one SVG render request.
 *
 * The plan is emitted by the compiled SVG owner before drawing. `ready` is
 * false when the selected artifact lacks one or more required capabilities.
 */
export interface SvgPlanResult {
  schema_version: 1;
  planned_operation_id: "svg";
  diagram_type: string;
  required_capability_ids: string[];
  missing_capability_ids: string[];
  supported_layout_adaptation_ids: ("none" | "fit-width")[];
  ready: boolean;
}

export interface AvailableDiagramDetectionFacts {
  readonly status: "available";
  readonly validity: "valid" | "recoverable-invalid";
  readonly diagramType: DiagramType;
  readonly syntaxId: string;
  readonly effectiveLayoutId: string;
  readonly appearance?: SourceAppearanceSummary;
}

export interface UnavailableDiagramDetectionFacts {
  readonly status: "unavailable";
  readonly validity: "unknown";
  readonly diagramType: null;
  readonly syntaxId: null;
  readonly effectiveLayoutId: null;
  readonly appearance?: SourceAppearanceSummary;
}

export type DiagramDetectionFacts =
  | AvailableDiagramDetectionFacts
  | UnavailableDiagramDetectionFacts;

export interface EditorPosition {
  line: number;
  character: number;
}

export interface EditorRange {
  start: EditorPosition;
  end: EditorPosition;
}

export interface EditorTextEdit {
  factSource?: EditorSemanticFactSource | null;
  range: EditorRange;
  newText: string;
}

export type EditorSemanticFactSource =
  | "unavailable"
  | "parser_complete"
  | "parser_recovered";

export type EditorCompletionItemKind = "keyword" | "variable" | "class" | "snippet";

export interface EditorCompletionResolveData {
  kind:
    | "diagram_header"
    | "operator"
    | "direction"
    | "directive"
    | "shape"
    | "class_name"
    | "node_identifier"
    | "style"
    | "interaction"
    | "frontmatter"
    | "template";
  label: string;
}

export interface EditorCompletionTextEdit {
  range: EditorRange;
  new_text: string;
}

export interface EditorCompletionItem {
  label: string;
  kind: EditorCompletionItemKind;
  detail?: string | null;
  data?: EditorCompletionResolveData | null;
  insert_text?: string | null;
  insert_text_format?: "plain_text" | "snippet";
  text_edit?: EditorCompletionTextEdit | null;
  label_details?: {
    description?: string | null;
    detail?: string | null;
  } | null;
}

export interface EditorCompletionList {
  is_incomplete: boolean;
  fact_source?: EditorSemanticFactSource | null;
  items: EditorCompletionItem[];
}

export type EditorDiagnosticSeverity = "error" | "warning" | "info" | "hint";

export interface EditorDiagnosticRelated {
  message: string;
  range: EditorRange;
}

export interface EditorDiagnosticData {
  id: string;
  code?: number | null;
  codeName?: string | null;
  category: LintRuleCategory | string;
  diagramType?: string | null;
  help?: string | null;
  fixes?: AnalysisDiagnosticFix[];
}

export interface EditorDiagnostic {
  range: EditorRange;
  severity: EditorDiagnosticSeverity;
  code: number | string;
  source: string;
  tags?: LintRuleTag[];
  message: string;
  related: EditorDiagnosticRelated[];
  data?: EditorDiagnosticData | null;
}

export interface EditorDiagnosticsResult {
  version: number;
  valid: boolean;
  summary: AnalysisResult["summary"];
  source: AnalysisResult["source"];
  diagnostics: EditorDiagnostic[];
}

export interface EditorCodeAction {
  title: string;
  kind: "quickfix";
  diagnostics: EditorDiagnostic[];
  edit: EditorWorkspaceEdit;
  isPreferred: boolean;
}

export interface EditorMarkupContent {
  kind: "markdown";
  value: string;
}

export interface EditorHover {
  contents: EditorMarkupContent;
  factSource: EditorSemanticFactSource;
  range?: EditorRange | null;
}

export type EditorSymbolKind =
  | "class"
  | "event"
  | "function"
  | "module"
  | "namespace"
  | "object"
  | "package"
  | "property"
  | "string"
  | "struct"
  | "variable";

export interface EditorDocumentSymbol {
  name: string;
  detail?: string | null;
  kind: EditorSymbolKind;
  factSource: EditorSemanticFactSource;
  range: EditorRange;
  selectionRange: EditorRange;
  children: EditorDocumentSymbol[];
}

export interface EditorLocation {
  uri: string;
  factSource: EditorSemanticFactSource;
  range: EditorRange;
}

export interface EditorSymbolInformation {
  name: string;
  kind: EditorSymbolKind;
  factSource: EditorSemanticFactSource;
  location: EditorLocation;
  containerName?: string | null;
}

export interface EditorPrepareRename {
  factSource: EditorSemanticFactSource;
  range: EditorRange;
  placeholder: string;
}

export interface EditorWorkspaceEdit {
  factSource?: EditorSemanticFactSource | null;
  changes: Record<string, EditorTextEdit[]>;
}

export interface BrowserEditorSession {
  readonly version: number;
  readonly uri: string;
  update(source: string, version: number): void;
  diagnostics(): EditorDiagnosticsResult;
  diagramDetection(): DiagramDetectionFacts;
  codeActions(): EditorCodeAction[];
  completions(position: EditorPosition): EditorCompletionList;
  hover(position: EditorPosition): EditorHover | null;
  documentSymbols(): EditorDocumentSymbol[];
  searchDocumentSymbols(query: string): EditorSymbolInformation[];
  definition(position: EditorPosition): EditorLocation | null;
  references(position: EditorPosition, includeDeclaration?: boolean): EditorLocation[];
  prepareRename(position: EditorPosition): EditorPrepareRename | null;
  rename(position: EditorPosition, newName: string): EditorWorkspaceEdit | null;
  dispose(): void;
}

export interface WasmEditorSessionBinding {
  readonly version: number;
  readonly uri: string;
  update(source: string, version: number): void;
  diagnostics(): EditorDiagnosticsResult;
  diagramDetection(): DiagramDetectionFacts;
  codeActions(): EditorCodeAction[];
  completions(line: number, character: number): EditorCompletionList;
  hover(line: number, character: number): EditorHover | null;
  documentSymbols(): EditorDocumentSymbol[];
  searchDocumentSymbols(query: string): EditorSymbolInformation[];
  definition(line: number, character: number): EditorLocation | null;
  references(
    line: number,
    character: number,
    includeDeclaration: boolean
  ): EditorLocation[];
  prepareRename(line: number, character: number): EditorPrepareRename | null;
  rename(
    line: number,
    character: number,
    newName: string
  ): EditorWorkspaceEdit | null;
  free(): void;
}

export interface WasmEditorSessionConstructor {
  new (
    source: string,
    version: number,
    uri?: string | null,
    optionsJson?: string | null
  ): WasmEditorSessionBinding;
}

export type MermanWasmSource =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

interface WasmBindgenInitEnvelope {
  module_or_path: MermanWasmSource | Promise<MermanWasmSource>;
}

export interface MermanWasmModuleBase {
  default: (input?: WasmBindgenInitEnvelope) => Promise<unknown>;
}

export interface MermanWasmModule extends MermanWasmModuleBase {
  EditorSession?: WasmEditorSessionConstructor;
  PreparedSource?: WasmPreparedSourceConstructor;
  RenderSession?: WasmRenderSessionConstructor;
  transportApiVersion: () => number;
  packageVersion: () => string;
  renderSvg: (source: string, optionsJson?: string | null) => string;
  renderSvgResult: (source: string, optionsJson?: string | null) => SvgRenderResult;
  renderSvgResultWithTextMeasurer?: (
    source: string,
    optionsJson: string | null | undefined,
    measurer: HostTextMeasurer
  ) => SvgRenderResult;
  edgeGeometryJson: (source: string, optionsJson?: string | null) => string;
  edgeGeometryJsonWithTextMeasurer?: (
    source: string,
    optionsJson: string | null | undefined,
    measurer: HostTextMeasurer
  ) => string;
  svgPlanJson: (source: string, optionsJson?: string | null) => SvgPlanResult;
  renderSvgWithTextMeasurer?: (
    source: string,
    optionsJson: string | null | undefined,
    measurer: HostTextMeasurer
  ) => string;
  renderAscii: (source: string, optionsJson?: string | null) => string;
  renderAsciiResult: (source: string, optionsJson?: string | null) => AsciiRenderResult;
  parseJson: (source: string, optionsJson?: string | null) => string;
  layoutJson: (source: string, optionsJson?: string | null) => string;
  layoutJsonWithTextMeasurer?: (
    source: string,
    optionsJson: string | null | undefined,
    measurer: HostTextMeasurer
  ) => string;
  analyze?: (source: string, optionsJson?: string | null) => AnalysisResult;
  analyzeJson?: (source: string, optionsJson?: string | null) => AnalysisResult;
  analysisFacts?: (source: string, optionsJson?: string | null) => AnalysisFactsResult;
  analyzeDocument?: (
    source: string,
    uri: string,
    optionsJson?: string | null
  ) => AnalysisResult;
  analyzeDocumentFacts?: (
    source: string,
    uri: string,
    optionsJson?: string | null
  ) => AnalysisFactsResult;
  validate: (source: string, optionsJson?: string | null) => ValidationResult;
  editorDiagnostics?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => EditorDiagnosticsResult;
  editorDiagramDetection?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => DiagramDetectionFacts;
  editorCodeActions?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => EditorCodeAction[];
  editorCompletions?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorCompletionList;
  editorHover?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorHover | null;
  editorDocumentSymbols?: (
    source: string,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorDocumentSymbol[];
  editorSearchDocumentSymbols?: (
    source: string,
    query: string,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorSymbolInformation[];
  editorDefinition?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorLocation | null;
  editorReferences?: (
    source: string,
    line: number,
    character: number,
    includeDeclaration: boolean,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorLocation[];
  editorPrepareRename?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorPrepareRename | null;
  editorRename?: (
    source: string,
    line: number,
    character: number,
    newName: string,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorWorkspaceEdit | null;
  editorCompletionTriggerCharacters?: () => string[];
  asciiSupportedDiagrams: () => string[];
  asciiCapabilities: () => AsciiCapability[];
  runtimeCatalog: () => RuntimeCatalog;
  themeCatalog: () => ThemeCatalog;
  exportThemePreset?: (presetId: string) => ThemeDocumentV2;
  resolveTheme?: (requestJson: string) => ThemeResolution;
  diagramFamilyCapabilities: () => DiagramFamilyCapability[];
  lintRuleCatalog?: () => LintRuleCatalogResponse;
  supportedDiagrams: () => string[];
  supportedThemes: () => string[];
}

export type MermanWasmLoader<Module extends MermanWasmModuleBase = MermanWasmModule> =
  () => Promise<Module>;

export interface MermanInitOptions<
  Module extends MermanWasmModuleBase = MermanWasmModule,
> {
  loader?: MermanWasmLoader<Module>;
  wasm?: MermanWasmSource | Promise<MermanWasmSource>;
}

export type MermanInitInput<Module extends MermanWasmModuleBase = MermanWasmModule> =
  | MermanWasmLoader<Module>
  | MermanInitOptions<Module>;
/** A persistent mount keeps its last successful drawing when a newer request fails. */
export type MountedDiagramStatus = "pending" | "rendering" | "ready" | "failed" | "destroyed";

export interface MountDiagramOptions {
  /** Frozen parse inputs and output defaults. Changing these options prepares a new snapshot. */
  prepareOptions?: SvgBindingOptions & { svg?: Omit<SvgOptions, "diagram_id"> & { diagram_id?: never } };
  /** Fresh redraw settings. The mount owns svg.diagram_id and fit-width container_width. */
  outputOptions?: Omit<PreparedSourceOutputOptions, "svg"> & { svg?: Omit<SvgOptions, "diagram_id"> & { diagram_id?: never } };
  /** Browser measurement is opt-in; a supplied callback uses the existing host provider. */
  textMeasurement?: "deterministic" | "browser" | HostTextMeasurer;
  onRender?: (result: SvgRenderResult, mount: MountedDiagram) => void;
  onError?: (error: unknown, mount: MountedDiagram) => void;
}

export interface MountedDiagram {
  readonly source: string;
  readonly result: SvgRenderResult | null;
  readonly error: unknown;
  readonly status: MountedDiagramStatus;
  /** Omitted options retain the current options; supplied options replace them completely. */
  update(source: string, options?: MountDiagramOptions): void;
  /** Refresh after external typography changes or adoption into another Document. */
  refresh(): void;
  /** Releases resources and leaves the last published DOM in place. Idempotent. */
  destroy(): void;
}
