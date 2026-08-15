import type {
  AsciiCapability,
  BindingStatusCodeName,
  DiagramFamilyCapability,
  DiagramType,
  LintBindingOptions,
  LintRuleCatalogEntry,
  LintRuleCatalogResponse,
  LintRuleCategory,
  LintRuleSeverity,
  LintRuleTag,
  RuntimeCapabilities,
  ThemeName,
} from "./public-catalog.js";
import type {
  HostTextDirection,
  HostTextMeasurementOperation,
  HostTextMeasurementPhase,
  HostTextWhiteSpace,
  HostTextWrapMode,
} from "./generated/text-measurement-abi.js";
import type { EditorRenamePolicy } from "./generated/token-descriptor.js";
import type {
  BINDING_OPTIONS_SCHEMA_VERSION,
  ResourceOverrideId,
  ResourceLimitId,
  ResourceOptions,
  ResourceProfile,
} from "./generated/resource-contract.js";

export type {
  HostTextDirection,
  HostTextMeasurementOperation,
  HostTextMeasurementPhase,
  HostTextMeasurementResultKind,
  HostTextWhiteSpace,
  HostTextWrapMode,
} from "./generated/text-measurement-abi.js";
export type { ResourceOptions } from "./generated/resource-contract.js";

export interface ParseOptions {
  suppress_errors?: boolean;
}

export interface LayoutOptions {
  container_width?: number;
  container_height?: number;
  screen_available_width?: number;
}

export interface RenderEnvironmentOptions {
  text_measurement?: "vendored" | "parity" | "deterministic";
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

export interface ThemeCatalog {
  schema_version: 2;
  structured_spec_available: boolean;
  supported_output_ids: string[];
  presets: ThemePresetCatalogEntry[];
  known_capability_ids: string[];
  known_text_capability_ids: string[];
  known_font_container_ids: string[];
  known_font_source_ids: string[];
  known_semantic_target_ids: string[];
  known_variant_ids: string[];
  resource_limits: ThemeResourceLimitCatalogEntry[];
}

export interface ThemePresetCatalogEntry {
  id: string;
  appearance: string;
  maturity: string;
  required_capability_ids: string[];
  required_text_capability_ids: string[];
}

export interface ThemeResourceLimitCatalogEntry {
  id: string;
  phase: string;
  description: string;
  effective_value: number | null;
  hard_cap: boolean;
}

export interface RuntimePayloadSchema {
  [key: string]: unknown;
  id: string;
  version: number;
}

export interface RuntimeRegistryContract {
  [key: string]: unknown;
  diagram_family_count: number;
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
  pipeline?: "parity" | "readable" | "resvg-safe";
  root_background_color?: string;
  drop_native_duplicate_fallbacks?: boolean;
  viewbox_padding?: number;
  viewBoxPadding?: number;
}

export type DiagramThemeSelection =
  | { preset: string; spec?: never }
  | { preset?: never; spec: DiagramThemeSpec };

export interface DiagramThemeSpec {
  mermaid?: MermaidThemeCompatibility;
  typography?: ThemeTypographySpec;
  styles?: ThemeStyleEntry[];
  canvas?: ThemeCanvasSpec;
  effects?: ThemeEffectEntry[];
  requirements?: ThemeRequirementsSpec;
  assets?: ThemeAssetsSpec;
}

export interface MermaidThemeCompatibility {
  theme?: ThemeName;
  dark_mode?: boolean;
  variables?: Record<string, string | number | boolean>;
}

export interface ThemeTypographySpec {
  default?: ThemeTextStyle;
  families?: Record<string, ThemeTextStyle>;
}

export interface ThemeTextStyle {
  font_stack?: string[];
  font_size_px?: number;
  font_weight?: number;
  font_style?: "normal" | "italic" | "oblique";
  line_height?: ThemeLineHeight;
  letter_spacing_px?: number;
  word_spacing_px?: number;
  transform?: "none" | "uppercase" | "lowercase" | "capitalize";
  decoration?: "none" | "underline" | "overline" | "line-through";
  text_align?: "start" | "center" | "end";
  white_space?: "normal" | "pre" | "no-wrap" | "pre-wrap" | "pre-line";
  wrap?: "normal" | "break-word" | "anywhere";
}

export type ThemeLineHeight = "normal" | number | { px: number };
export type ThemePatch<T> = T | null;
export type ThemeLength = number | { px: number } | { percent: number };
export type ThemeInsets =
  | number
  | { top: number; right: number; bottom: number; left: number };

export type ThemePaint =
  | string
  | { kind: "transparent" }
  | { kind: "solid"; color: string }
  | {
      kind: "linear-gradient";
      angle_degrees: number;
      stops: ThemeGradientStop[];
    }
  | {
      kind: "radial-gradient";
      center_x: ThemeLength;
      center_y: ThemeLength;
      radius: ThemeLength;
      stops: ThemeGradientStop[];
    }
  | {
      kind: "pattern";
      pattern: "dots" | "grid" | "stripes";
      cell_width: number;
      cell_height: number;
      foreground: string;
      background?: string;
      angle_degrees?: number;
    };

export interface ThemeGradientStop {
  offset: number;
  color: string;
}

export type ThemeStyleEntry = ThemeRuleEntry | ThemeOrdinalPaletteEntry;

export interface ThemeRuleEntry {
  kind: "rule";
  target: string;
  family?: string;
  variant?: string;
  ordinal?: { exact: number } | { cycle: { period: number; offset: number } };
  style: ThemeStylePatch;
}

export interface ThemeOrdinalPaletteEntry {
  kind: "ordinal-palette";
  target: string;
  colors: string[];
}

export interface ThemeStylePatch {
  fill?: ThemePatch<ThemePaint>;
  opacity?: ThemePatch<number>;
  fill_opacity?: ThemePatch<number>;
  stroke?: ThemePatch<ThemeStrokePatch>;
  radius?: ThemePatch<number>;
  padding?: ThemePatch<ThemeInsets>;
  typography?: ThemePatch<ThemeTextStylePatch>;
  effect?: ThemePatch<string>;
}

export interface ThemeStrokePatch {
  paint?: ThemePatch<ThemePaint>;
  width?: ThemePatch<number>;
  dasharray?: ThemePatch<number[]>;
  linecap?: ThemePatch<"butt" | "round" | "square">;
  linejoin?: ThemePatch<"miter" | "round" | "bevel">;
  opacity?: ThemePatch<number>;
}

export interface ThemeTextStylePatch {
  font_stack?: ThemePatch<string[]>;
  font_size_px?: ThemePatch<number>;
  font_weight?: ThemePatch<number>;
  font_style?: ThemePatch<"normal" | "italic" | "oblique">;
  line_height?: ThemePatch<ThemeLineHeight>;
  letter_spacing_px?: ThemePatch<number>;
  word_spacing_px?: ThemePatch<number>;
  transform?: ThemePatch<"none" | "uppercase" | "lowercase" | "capitalize">;
  decoration?: ThemePatch<"none" | "underline" | "overline" | "line-through">;
  text_align?: ThemePatch<"start" | "center" | "end">;
  white_space?: ThemePatch<"normal" | "pre" | "no-wrap" | "pre-wrap" | "pre-line">;
  wrap?: ThemePatch<"normal" | "break-word" | "anywhere">;
}

export interface ThemeCanvasSpec {
  base?: ThemePaint;
  layers?: ThemeCanvasLayer[];
  bleed?: ThemeInsets;
}

export interface ThemeCanvasLayer {
  paint: ThemePaint;
  opacity?: number;
  blend_mode?: "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "difference";
  offset_x?: number;
  offset_y?: number;
}

export type ThemeEffectEntry = ThemeEffectGraph | ThemeEffectBinding;

export interface ThemeEffectGraph {
  kind: "graph";
  id: string;
  primitives: ThemeEffectPrimitive[];
}

export interface ThemeEffectBinding {
  kind: "binding";
  target: string;
  effect_id: string;
}

export type ThemeEffectInput = "source-graphic" | "previous";
export type ThemeEffectPrimitive =
  | {
      kind: "drop-shadow";
      input?: ThemeEffectInput;
      offset_x: number;
      offset_y: number;
      blur_radius: number;
      spread: number;
      color: string;
    }
  | { kind: "gaussian-blur"; input?: ThemeEffectInput; std_deviation: number }
  | { kind: "color-matrix"; input?: ThemeEffectInput; values: number[] }
  | {
      kind: "turbulence";
      input?: ThemeEffectInput;
      base_frequency_x: number;
      base_frequency_y: number;
      octaves: number;
      seed: number;
    }
  | {
      kind: "displacement";
      input?: ThemeEffectInput;
      map_input: ThemeEffectInput;
      scale: number;
    };

export interface ThemeRequirementsSpec {
  capabilities?: string[];
  text_capabilities?: string[];
}

export interface ThemeAssetsSpec {
  fonts?: ThemeFontAsset[];
  aliases?: Array<{ alias: string; target: string }>;
  generic_families?: Array<{ generic: string; target: string }>;
  available_sources?: Array<"embedded" | "system">;
  embedding?: "none" | "full-font" | "subset";
}

export interface ThemeFontAsset {
  id: string;
  format: "truetype" | "opentype" | "collection" | "woff2";
  data_base64: string;
}

export interface MermaidSiteConfig {
  [key: string]: unknown;
  themeCSS?: never;
  secure?: never;
}

export interface AnalysisBindingOptions {
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
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

type NoDirectAnalysisBindingOptions<Options extends AnalysisBindingOptions> = {
  [Property in keyof Options]?: never;
};

type DirectAnalysisBindingRoot<Options extends AnalysisBindingOptions> = Options & {
  analysis?: never;
  merman?: never;
};

type AnalysisWrappedBindingRoot<Options extends AnalysisBindingOptions> =
  NoDirectAnalysisBindingOptions<Options> & {
  analysis: Options;
  merman?: never;
};

type MermanWrappedBindingRoot<Options extends AnalysisBindingOptions> =
  NoDirectAnalysisBindingOptions<Options> & {
  analysis?: never;
  merman: Options;
};

type AnalysisBindingRoot<Options extends AnalysisBindingOptions> =
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
  default_direction?: AsciiDirectionOption;
  defaultDirection?: AsciiDirectionOption;
  color_mode?: AsciiColorModeOption;
  colorMode?: AsciiColorModeOption;
  theme?: AsciiThemeOptions;
  sequence_mirror_actors?: boolean;
  sequenceMirrorActors?: boolean;
  box_border_padding?: number;
  boxBorderPadding?: number;
  graph_padding_x?: number;
  graphPaddingX?: number;
  graph_padding_y?: number;
  graphPaddingY?: number;
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
  ascii?: AsciiRenderOptions;
}

export type AsciiBindingOptions = CommonBindingOptions & AsciiBindingFields;

interface SvgBindingFields {
  theme?: DiagramThemeSelection | null;
  environment?: RenderEnvironmentOptions;
  layout?: LayoutOptions;
  svg?: SvgOptions;
}

export type SvgBindingOptions = CommonBindingOptions & SvgBindingFields;

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
}

export interface AnalysisFactsResult extends AnalysisPayloadFields {
  version: 2;
  diagrams: AnalysisDiagramFacts[];
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
  ready: boolean;
}

export interface AvailableDiagramDetectionFacts {
  readonly status: "available";
  readonly validity: "valid" | "recoverable-invalid";
  readonly diagramType: DiagramType;
  readonly syntaxId: string;
  readonly effectiveLayoutId: string;
}

export interface UnavailableDiagramDetectionFacts {
  readonly status: "unavailable";
  readonly validity: "unknown";
  readonly diagramType: null;
  readonly syntaxId: null;
  readonly effectiveLayoutId: null;
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

export interface EditorSemanticTokenLegend {
  tokenTypes: string[];
  tokenModifiers: string[];
}

export type EditorSemanticTokenDescriptor =
  typeof import("./generated/token-descriptor.js").SEMANTIC_TOKEN_DESCRIPTOR;

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
  semanticTokens(): Uint32Array;
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
  semanticTokens(): Uint32Array;
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

export interface WasmSemanticTokenDescriptor {
  schemaVersion: number;
  digest: string;
  tokenTypes: Array<{
    id: string;
    code: number;
    lspName: string;
    lspIndex: number;
  }>;
  modifiers: Array<{
    id: string;
    index: number;
    bit: number;
    lspName: string;
    lspIndex: number;
  }>;
  packed: {
    encoding: string;
    wordWidthBits: number;
    recordWidth: number;
    fieldOrder: string[];
  };
  validTypeCodeMax: number;
  validModifierMask: number;
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
  transportApiVersion: () => number;
  packageVersion: () => string;
  renderSvg: (source: string, optionsJson?: string | null) => string;
  svgPlanJson: (source: string, optionsJson?: string | null) => SvgPlanResult;
  renderSvgWithTextMeasurer?: (
    source: string,
    optionsJson: string | null | undefined,
    measurer: HostTextMeasurer
  ) => string;
  renderAscii: (source: string, optionsJson?: string | null) => string;
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
  editorSemanticTokenDescriptor?: () => WasmSemanticTokenDescriptor;
  editorCompletionTriggerCharacters?: () => string[];
  editorSemanticTokens?: (
    source: string,
    uri?: string | null,
    optionsJson?: string | null
  ) => Uint32Array;
  asciiSupportedDiagrams: () => string[];
  asciiCapabilities: () => AsciiCapability[];
  runtimeCatalog: () => RuntimeCatalog;
  themeCatalog: () => ThemeCatalog;
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
