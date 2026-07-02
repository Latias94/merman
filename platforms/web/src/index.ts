export interface ParseOptions {
  suppress_errors?: boolean;
}

export interface LayoutOptions {
  viewport_width?: number;
  viewport_height?: number;
  text_measurer?: "vendored" | "deterministic";
  math_renderer?: "none" | "ratex";
  flowchart_elk_backend?: "source-ported" | "source_ported" | "source" | "compat";
}

export interface ResourceOptions {
  profile?:
    | "interactive"
    | "typst-package"
    | "typst_package"
    | "typst"
    | "trusted-native"
    | "trusted_native"
    | "trusted"
    | "unbounded-for-trusted-input"
    | "unbounded_for_trusted_input"
    | "unbounded";
  max_source_bytes?: number;
  max_svg_bytes?: number;
  max_flowchart_nodes?: number;
  max_flowchart_edges?: number;
  max_flowchart_subgraphs?: number;
  max_label_bytes?: number;
}

export interface SvgOptions {
  diagram_id?: string;
  pipeline?: "parity" | "readable" | "resvg-safe" | "resvg_safe";
  scoped_css?: string;
  css_override_policy?: "preserve" | "strip-existing-important" | "strip_existing_important";
  root_background_color?: string;
  drop_native_duplicate_fallbacks?: boolean;
}

export type HostThemeAppearance = "light" | "dark";

export interface HostThemeRolesOptions {
  canvas?: string;
  surface?: string;
  surface_alt?: string;
  surface_muted?: string;
  text?: string;
  subtle_text?: string;
  border?: string;
  line?: string;
  edge_label_background?: string;
  cluster_background?: string;
  cluster_border?: string;
  note_background?: string;
  note_border?: string;
  note_text?: string;
  actor_background?: string;
  actor_border?: string;
  actor_text?: string;
  activation_background?: string;
  activation_border?: string;
  error?: string;
  warning?: string;
  success?: string;
}

export interface HostThemeOutputOptions {
  pipeline?: "parity" | "readable" | "resvg-safe" | "resvg_safe";
  css_override_policy?: "preserve" | "strip-existing-important" | "strip_existing_important";
  root_background?: "none" | "canvas" | string;
  drop_native_duplicate_fallbacks?: boolean;
  scoped_css?: string;
}

export interface HostThemeOptions {
  preset?: HostThemePresetName;
  appearance?: HostThemeAppearance;
  font_family?: string;
  font_size?: string;
  roles?: HostThemeRolesOptions;
  series_palette?: string[];
  output?: HostThemeOutputOptions;
  themeVariables?: Record<string, unknown>;
  theme_variables?: Record<string, unknown>;
  site_config?: MermaidSiteConfig;
}

export type MermaidSiteConfig = Record<string, unknown>;

export interface CommonBindingOptions {
  version?: number;
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
  site_config?: MermaidSiteConfig;
  parse?: ParseOptions;
  lint?: LintBindingOptions;
}

export type AsciiCharsetOption = "ascii" | "unicode";
export type AsciiDirectionOption =
  | "lr"
  | "left-right"
  | "left_right"
  | "td"
  | "tb"
  | "top-down"
  | "top_down";
export type AsciiColorModeOption =
  | "plain"
  | "none"
  | "auto"
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
  xychart_vertical_plot_height?: number;
  xychartVerticalPlotHeight?: number;
  xychart_category_band_width?: number;
  xychartCategoryBandWidth?: number;
  xychart_horizontal_plot_width?: number;
  xychartHorizontalPlotWidth?: number;
  max_grid_cells?: number;
  maxGridCells?: number;
}

export interface AsciiBindingOptions extends CommonBindingOptions {
  ascii?: AsciiRenderOptions;
}

export interface SvgBindingOptions extends CommonBindingOptions {
  host_theme?: HostThemeOptions;
  layout?: LayoutOptions;
  resources?: ResourceOptions;
  svg?: SvgOptions;
}

export type BindingOptions = SvgBindingOptions;

export type HostTextWrapMode =
  | "svg-like"
  | "svg-like-single-run"
  | "html-like";

export type HostTextWhiteSpace =
  | "normal"
  | "nowrap"
  | "break-spaces"
  | "pre-wrap";

export interface HostTextMeasureRequest {
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
  direction: "auto" | "ltr" | "rtl";
  white_space: HostTextWhiteSpace;
}

export interface HostTextMeasureResult {
  handled?: boolean;
  width: number;
  height: number;
  line_count?: number;
}

export type HostTextMeasurer = (
  request: HostTextMeasureRequest
) => HostTextMeasureResult | null | undefined;

export const SUPPORTED_THEMES = [
  "default",
  "base",
  "dark",
  "forest",
  "neutral",
  "neo",
  "neo-dark",
  "redux",
  "redux-dark",
  "redux-color",
  "redux-dark-color",
] as const;

export type ThemeName = (typeof SUPPORTED_THEMES)[number];

export const SUPPORTED_HOST_THEME_PRESETS = [
  "editor-light",
  "editor-dark",
  "one-dark",
  "gruvbox-light",
  "gruvbox-dark",
  "ayu-light",
  "ayu-dark",
] as const;

export type HostThemePresetName = (typeof SUPPORTED_HOST_THEME_PRESETS)[number];

export const SUPPORTED_DIAGRAMS = [
  "architecture",
  "block",
  "c4",
  "class",
  "er",
  "flowchart",
  "gantt",
  "gitgraph",
  "info",
  "journey",
  "kanban",
  "mindmap",
  "packet",
  "pie",
  "quadrantchart",
  "radar",
  "requirement",
  "sankey",
  "sequence",
  "state",
  "timeline",
  "treemap",
  "venn",
  "xychart",
  "zenuml",
] as const;

export type DiagramType = (typeof SUPPORTED_DIAGRAMS)[number];

export const SUPPORTED_ASCII_DIAGRAMS = [
  "class",
  "er",
  "flowchart",
  "gantt",
  "gitgraph",
  "journey",
  "kanban",
  "mindmap",
  "packet",
  "sequence",
  "state",
  "timeline",
  "treeView",
  "xychart",
  "zenuml",
] as const;

export type AsciiDiagramType = (typeof SUPPORTED_ASCII_DIAGRAMS)[number];

export const BINDING_STATUS_CODE_NAMES = [
  "MERMAN_OK",
  "MERMAN_INVALID_ARGUMENT",
  "MERMAN_UTF8_ERROR",
  "MERMAN_OPTIONS_JSON_ERROR",
  "MERMAN_NO_DIAGRAM",
  "MERMAN_PARSE_ERROR",
  "MERMAN_RENDER_ERROR",
  "MERMAN_UNSUPPORTED_FORMAT",
  "MERMAN_PANIC",
  "MERMAN_INTERNAL_ERROR",
  "MERMAN_RESOURCE_LIMIT_EXCEEDED",
] as const;

export type BindingStatusCodeName = (typeof BINDING_STATUS_CODE_NAMES)[number];

export interface BindingErrorPayload {
  version: number;
  ok: false;
  code: number;
  code_name: BindingStatusCodeName | string;
  message: string;
}

export interface BindingCapabilities {
  render: boolean;
  ascii: boolean;
  core_full: boolean;
  core_host: boolean;
  elk_layout: boolean;
  ratex_math: boolean;
  editor_language: boolean;
}

export type RegistryProfile = "full" | "tiny";

export interface DiagramFamilyCapability {
  diagram_type: string;
  metadata_id: DiagramType | null;
  has_semantic_parser: boolean;
  has_render_parser: boolean;
}

export type LintRuleSeverity = "error" | "warning" | "info" | "hint";

export type LintRuleCategory =
  | "parse"
  | "semantic"
  | "config"
  | "resource"
  | "compatibility"
  | "layout"
  | "render"
  | "internal";

export type LintRuleProfile = "core" | "recommended" | "strict";

export type LintRuleOrigin =
  | "mermaid_syntax"
  | "mermaid_compatibility"
  | "merman_authoring"
  | "merman_resource_policy"
  | "merman_internal";

export interface LintRuleCatalogEntry {
  id: string;
  description: string;
  evidence: string[];
  default_severity: LintRuleSeverity;
  category: LintRuleCategory;
  default_enabled: boolean;
  default_profile: LintRuleProfile;
  origin: LintRuleOrigin;
  configurable: boolean;
  fixable: boolean;
}

export interface LintRuleSeverityOverrideOptions {
  rule_id: string;
  severity: LintRuleSeverity;
}

export interface LintBindingOptions {
  profile?: LintRuleProfile;
  enable_rules?: string[];
  disable_rules?: string[];
  rule_severities?: LintRuleSeverityOverrideOptions[];
}

export type AsciiSupportLevel = "full" | "partial" | "summary" | "unsupported";

export type AsciiEvidenceKind =
  | "mermaid_ascii_oracle"
  | "beautiful_mermaid_prior_art"
  | "local_semantic_probe"
  | "local_advantage"
  | "support_matrix"
  | "gap_registry";

export interface AsciiCapabilityEvidence {
  kind: AsciiEvidenceKind | string;
  source: string;
  note: string;
}

export interface AsciiCapability {
  diagram_type: AsciiDiagramType | string;
  display_name: string;
  support_level: AsciiSupportLevel;
  summary_fallback: boolean;
  supported_semantics: string[];
  limits: string[];
  evidence: AsciiCapabilityEvidence[];
}

export const DEFAULT_BINDING_CAPABILITIES: BindingCapabilities = {
  render: true,
  ascii: true,
  core_full: true,
  core_host: true,
  elk_layout: true,
  ratex_math: false,
  editor_language: true,
};

export function isThemeName(theme: string): theme is ThemeName {
  return (SUPPORTED_THEMES as readonly string[]).includes(theme);
}

export function isHostThemePresetName(
  preset: string
): preset is HostThemePresetName {
  return (SUPPORTED_HOST_THEME_PRESETS as readonly string[]).includes(preset);
}

export function isDiagramType(diagram: string): diagram is DiagramType {
  return (SUPPORTED_DIAGRAMS as readonly string[]).includes(diagram);
}

export function isAsciiDiagramType(
  diagram: string
): diagram is AsciiDiagramType {
  return (SUPPORTED_ASCII_DIAGRAMS as readonly string[]).includes(diagram);
}

export function isBindingStatusCodeName(
  codeName: string
): codeName is BindingStatusCodeName {
  return (BINDING_STATUS_CODE_NAMES as readonly string[]).includes(codeName);
}

export function isBindingErrorPayload(error: unknown): error is BindingErrorPayload {
  if (!error || typeof error !== "object") {
    return false;
  }
  const payload = error as Record<string, unknown>;
  return (
    payload.ok === false &&
    typeof payload.version === "number" &&
    typeof payload.code === "number" &&
    typeof payload.code_name === "string" &&
    typeof payload.message === "string"
  );
}

export function normalizeThemeName(theme: string | null | undefined): ThemeName {
  return theme && isThemeName(theme) ? theme : "default";
}

export function normalizeHostThemePresetName(
  preset: string | null | undefined
): HostThemePresetName | null {
  return preset && isHostThemePresetName(preset) ? preset : null;
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
  message: string;
  code?: number | null;
  code_name?: string | null;
  diagram_type?: string | null;
  span?: AnalysisSpan | null;
  related: AnalysisDiagnosticRelated[];
  help?: string | null;
  fixes?: AnalysisDiagnosticFix[];
}

export interface AnalysisResult {
  version: number;
  valid: boolean;
  summary: AnalysisSummary;
  source: AnalysisSource;
  diagnostics: AnalysisDiagnostic[];
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

export type AnalysisSemanticRole = "entity" | "outline" | "payload" | string;

export type AnalysisExpectedSyntaxKind =
  | "id_list"
  | "node_identifier"
  | "shape"
  | "shape_trigger"
  | "direction"
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
}

export interface AnalysisExpectedSyntaxFacts {
  kind: AnalysisExpectedSyntaxKind;
  span: AnalysisFactSpan;
}

export interface AnalysisFlowchartEdgeDefaults {
  interpolate?: string | null;
  style: string[];
}

export interface AnalysisFlowchartNodeFacts {
  id: string;
  label?: string | null;
  labelType?: string | null;
  layoutShape?: string | null;
  icon?: string | null;
  form?: string | null;
  pos?: string | null;
  img?: string | null;
  constraint?: string | null;
  assetWidth?: number | null;
  assetHeight?: number | null;
  classes: string[];
  styles: string[];
  link?: string | null;
  linkTarget?: string | null;
  haveCallback: boolean;
}

export interface AnalysisFlowchartEdgeFacts {
  id: string;
  from: string;
  to: string;
  label?: string | null;
  labelType?: string | null;
  type?: string | null;
  stroke?: string | null;
  interpolate?: string | null;
  classes: string[];
  style: string[];
  animate?: boolean | null;
  animation?: string | null;
  length: number;
}

export interface AnalysisFlowchartSubgraphFacts {
  id: string;
  title: string;
  dir?: string | null;
  labelType?: string | null;
  classes: string[];
  styles: string[];
  nodes: string[];
}

export interface AnalysisFlowchartFacts {
  direction?: string | null;
  classDefs: Record<string, string[]>;
  edgeDefaults?: AnalysisFlowchartEdgeDefaults | null;
  vertexCalls: string[];
  nodes: AnalysisFlowchartNodeFacts[];
  edges: AnalysisFlowchartEdgeFacts[];
  subgraphs: AnalysisFlowchartSubgraphFacts[];
  tooltips: Record<string, string>;
}

export interface AnalysisDiagramSyntaxFacts {
  diagram_type?: string | null;
  fact_source: EditorSemanticFactSource;
  parser_backed: boolean;
  recovered: boolean;
  flowchart?: AnalysisFlowchartFacts | null;
  node_ids: string[];
  class_names: string[];
  directive_prefixes: string[];
  references: AnalysisReferenceFacts[];
  outline_items: AnalysisLineItemFacts[];
  semantic_items: AnalysisSemanticItemFacts[];
  expected_syntax: AnalysisExpectedSyntaxFacts[];
}

export interface AnalysisDiagramFacts {
  source_id: string;
  index: number;
  kind: AnalysisDiagramKind;
  source: AnalysisSource;
  span?: AnalysisSpan | null;
  body_span?: AnalysisSpan | null;
  text_len: number;
  fence_delimiter?: AnalysisFenceDelimiterFacts | null;
  syntax: AnalysisDiagramSyntaxFacts;
}

export interface AnalysisFactsResult extends AnalysisResult {
  diagrams: AnalysisDiagramFacts[];
}

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

export type EditorSemanticFactSource = "text_scan" | "parser_complete" | "parser_recovered";

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

export interface EditorDiagnostic {
  range: EditorRange;
  severity: EditorDiagnosticSeverity;
  code: number | string;
  source: string;
  message: string;
  related: EditorDiagnosticRelated[];
  data?: {
    id: string;
    fixes: Array<{
      title: string;
      edits: Array<{
        span: unknown;
        replacement: string;
      }>;
      is_preferred?: boolean;
    }>;
  } | null;
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

export interface EditorSemanticToken {
  line: number;
  start: number;
  length: number;
  tokenType: string;
  tokenModifier: string;
  factSource: EditorSemanticFactSource;
}

export interface MermanWasmModule {
  default: (input?: unknown) => Promise<unknown>;
  abiVersion: () => number;
  packageVersion: () => string;
  renderSvg: (source: string, optionsJson?: string | null) => string;
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
  analyze: (source: string, optionsJson?: string | null) => AnalysisResult;
  analyzeJson?: (source: string, optionsJson?: string | null) => AnalysisResult;
  analysisFacts?: (source: string, optionsJson?: string | null) => AnalysisFactsResult;
  analyzeFacts?: (source: string, optionsJson?: string | null) => AnalysisFactsResult;
  analyzeDocument?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => AnalysisResult;
  analyzeDocumentFacts?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => AnalysisFactsResult;
  validate: (source: string, optionsJson?: string | null) => ValidationResult;
  editorDiagnostics?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => EditorDiagnosticsResult;
  editorCodeActions?: (
    source: string,
    optionsJson?: string | null,
    uri?: string | null
  ) => EditorCodeAction[];
  editorCompletions?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null
  ) => EditorCompletionList;
  editorHover?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null
  ) => EditorHover | null;
  editorDocumentSymbols?: (
    source: string,
    uri?: string | null
  ) => EditorDocumentSymbol[];
  editorWorkspaceSymbols?: (
    source: string,
    query: string,
    uri?: string | null
  ) => EditorSymbolInformation[];
  editorDefinition?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null
  ) => EditorLocation | null;
  editorReferences?: (
    source: string,
    line: number,
    character: number,
    includeDeclaration: boolean,
    uri?: string | null
  ) => EditorLocation[];
  editorPrepareRename?: (
    source: string,
    line: number,
    character: number,
    uri?: string | null
  ) => EditorPrepareRename | null;
  editorRename?: (
    source: string,
    line: number,
    character: number,
    newName: string,
    uri?: string | null
  ) => EditorWorkspaceEdit | null;
  editorSemanticTokenLegend?: () => EditorSemanticTokenLegend;
  editorSemanticTokens?: (
    source: string,
    uri?: string | null
  ) => EditorSemanticToken[];
  asciiSupportedDiagrams: () => string[];
  asciiCapabilities?: () => AsciiCapability[];
  bindingCapabilities?: () => BindingCapabilities;
  selectedRegistryProfile?: () => string;
  diagramFamilyCapabilities?: () => DiagramFamilyCapability[];
  lintRuleCatalog?: () => LintRuleCatalogEntry[];
  supportedDiagrams: () => string[];
  supportedHostThemePresets?: () => string[];
  supportedThemes?: () => string[];
  themes?: () => string[];
}

export type MermanWasmLoader = () => Promise<MermanWasmModule>;

export interface MermanInitOptions {
  loader?: MermanWasmLoader;
  wasm?: unknown;
}

export type MermanInitInput = MermanWasmLoader | MermanInitOptions;

let wasmModule: MermanWasmModule | null = null;
let initPromise: Promise<MermanWasmModule> | null = null;
let supportedDiagramsCache: DiagramType[] | null = null;
let asciiSupportedDiagramsCache: AsciiDiagramType[] | null = null;
let asciiCapabilitiesCache: AsciiCapability[] | null = null;
let diagramFamilyCapabilitiesCache: DiagramFamilyCapability[] | null = null;
let lintRuleCatalogCache: LintRuleCatalogEntry[] | null = null;
let supportedHostThemePresetsCache: HostThemePresetName[] | null = null;
let supportedThemesCache: ThemeName[] | null = null;

export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {
  if (wasmModule) {
    return Promise.resolve(wasmModule);
  }
  if (initPromise) {
    return initPromise;
  }
  initPromise = doInit(init).catch((error) => {
    initPromise = null;
    throw error;
  });
  return initPromise;
}

async function doInit(init?: MermanInitInput): Promise<MermanWasmModule> {
  const loader = typeof init === "function" ? init : init?.loader;
  const wasm = typeof init === "function" ? undefined : init?.wasm;
  const module = loader ? await loader() : await defaultLoader();
  await module.default(wasm);
  wasmModule = module;
  return module;
}

async function defaultLoader(): Promise<MermanWasmModule> {
  return (await import("../pkg/merman_wasm.js")) as unknown as MermanWasmModule;
}

export function getMerman(): MermanWasmModule {
  if (!wasmModule) {
    throw new Error("Merman WASM is not initialized. Call initMerman() first.");
  }
  return wasmModule;
}

export function isMermanInitialized(): boolean {
  return wasmModule !== null;
}

export function renderSvg(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
}

export function renderSvgWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const renderWithMeasurer = getMerman().renderSvgWithTextMeasurer;
  if (!renderWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose renderSvgWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return renderWithMeasurer(source, encodeOptions(options), measurer);
}

export function layoutJsonWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const layoutWithMeasurer = getMerman().layoutJsonWithTextMeasurer;
  if (!layoutWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose layoutJsonWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return layoutWithMeasurer(source, encodeOptions(options), measurer);
}

export function createBrowserTextMeasurer(): HostTextMeasurer {
  let probe: HTMLDivElement | null = null;

  return (request) => {
    probe ??= createTextMeasureProbe();
    if (!probe) {
      return undefined;
    }

    if (!request.text) {
      return {
        width: 0,
        height: request.line_height || request.font_size,
        line_count: 1,
      };
    }

    applyTextMeasureStyle(probe, request);
    const maxWidth = normalizeMeasureMaxWidth(request);
    if (request.wrap_mode === "html-like" && maxWidth !== null) {
      const natural = measureProbeText(probe, request.text, {
        display: "inline-block",
        width: "auto",
        maxWidth: "none",
        whiteSpace: "nowrap",
      });
      if (natural.width <= maxWidth) {
        return natural;
      }

      return measureProbeText(probe, request.text, {
        display: "table",
        width: `${maxWidth}px`,
        maxWidth: `${maxWidth}px`,
        whiteSpace: "break-spaces",
      });
    }

    return measureProbeText(probe, request.text, {
      display: "inline-block",
      width: "auto",
      maxWidth: maxWidth === null ? "none" : `${maxWidth}px`,
      whiteSpace: request.white_space,
    });
  };
}

function applyTextMeasureStyle(
  probe: HTMLDivElement,
  request: HostTextMeasureRequest
) {
    const style = probe.style;
    style.fontFamily = request.font_family || "sans-serif";
    style.fontSize = `${Math.max(1, request.font_size)}px`;
    style.fontWeight = request.font_weight || "normal";
    style.fontStyle = request.font_style || "normal";
    style.lineHeight = `${Math.max(1, request.line_height || request.font_size)}px`;
    style.letterSpacing = `${request.letter_spacing || 0}px`;
    style.wordSpacing = `${request.word_spacing || 0}px`;
    style.direction = request.direction === "rtl" ? "rtl" : "ltr";
}

function measureProbeText(
  probe: HTMLDivElement,
  text: string,
  styleOverride: Pick<
    CSSStyleDeclaration,
    "display" | "width" | "maxWidth" | "whiteSpace"
  >
): HostTextMeasureResult {
    probe.style.display = styleOverride.display;
    probe.style.width = styleOverride.width;
    probe.style.maxWidth = styleOverride.maxWidth;
    probe.style.whiteSpace = styleOverride.whiteSpace;
    probe.textContent = text;
    const rect = probe.getBoundingClientRect();
    const lineHeight = Math.max(1, parseFloat(probe.style.lineHeight) || 1);
    const height = Math.max(lineHeight, rect.height);
    return {
      width: Math.max(0, rect.width),
      height,
      line_count: Math.max(1, Math.round(height / lineHeight)),
    };
}

function normalizeMeasureMaxWidth(
  request: HostTextMeasureRequest
): number | null {
  if (
    !request.has_max_width ||
    typeof request.max_width !== "number" ||
    !Number.isFinite(request.max_width) ||
    request.max_width <= 0
  ) {
    return null;
  }
  return request.max_width;
}

function createTextMeasureProbe(): HTMLDivElement | null {
  if (typeof document === "undefined" || !document.body) {
    return null;
  }

  const probe = document.createElement("div");
  probe.setAttribute("aria-hidden", "true");
  Object.assign(probe.style, {
    position: "fixed",
    left: "-10000px",
    top: "-10000px",
    visibility: "hidden",
    contain: "layout style paint",
    boxSizing: "border-box",
    padding: "0",
    margin: "0",
    border: "0",
    display: "block",
  });
  document.body.appendChild(probe);
  return probe;
}

export function renderSvgElement(
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  if (typeof DOMParser === "undefined" || typeof document === "undefined") {
    throw new Error("renderSvgElement() requires a browser DOM.");
  }

  const svgText = renderSvg(source, options);
  const parsed = new DOMParser().parseFromString(svgText, "image/svg+xml");
  const parseError = parsed.querySelector("parsererror");
  if (parseError) {
    throw new Error(parseError.textContent || "Merman rendered invalid SVG.");
  }

  const svg = parsed.documentElement;
  if (svg.localName !== "svg") {
    throw new Error("Merman render output did not contain an SVG root element.");
  }
  return document.importNode(svg, true) as unknown as SVGSVGElement;
}

export function renderSvgToElement(
  target: Element,
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  const svg = renderSvgElement(source, options);
  target.replaceChildren(svg);
  return svg;
}

export function renderAscii(source: string, options?: AsciiBindingOptions | string): string {
  return getMerman().renderAscii(source, encodeOptions(options));
}

export function parseJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().parseJson(source, encodeOptions(options));
}

export function parseObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(parseJson(source, options)) as T;
}

export function layoutJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().layoutJson(source, encodeOptions(options));
}

export function layoutObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(layoutJson(source, options)) as T;
}

export function analyze(source: string, options?: SvgBindingOptions | string): AnalysisResult {
  const merman = getMerman();
  const encodedOptions = encodeOptions(options);
  const analysis =
    merman.analyze?.(source, encodedOptions) ?? merman.analyzeJson?.(source, encodedOptions);
  if (!analysis) {
    throw new Error("Merman analyze() is not available in this artifact.");
  }
  return analysis;
}

export function analyzeJson(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisResult {
  return analyze(source, options);
}

export function analysisFacts(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisFactsResult {
  const merman = getMerman();
  const facts = merman.analysisFacts ?? merman.analyzeFacts;
  if (!facts) {
    throw new Error("Merman analysisFacts() is not available in this artifact.");
  }
  return facts(source, encodeOptions(options));
}

export function analyzeFacts(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisFactsResult {
  return analysisFacts(source, options);
}

export function analyzeDocument(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisResult {
  const analyzeDocument = getMerman().analyzeDocument;
  if (!analyzeDocument) {
    throw new Error("Merman analyzeDocument() is not available in this artifact.");
  }
  return analyzeDocument(source, encodeOptions(options), uri);
}

export function analyzeDocumentFacts(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisFactsResult {
  const analyzeDocumentFacts = getMerman().analyzeDocumentFacts;
  if (!analyzeDocumentFacts) {
    throw new Error("Merman analyzeDocumentFacts() is not available in this artifact.");
  }
  return analyzeDocumentFacts(source, encodeOptions(options), uri);
}

export function validate(source: string, options?: SvgBindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function editorDiagnostics(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorDiagnosticsResult {
  const diagnostics = requireEditorLanguage("editorDiagnostics", getMerman().editorDiagnostics);
  return diagnostics(source, encodeOptions(options), uri);
}

export function editorCodeActions(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorCodeAction[] {
  const codeActions = requireEditorLanguage("editorCodeActions", getMerman().editorCodeActions);
  return codeActions(source, encodeOptions(options), uri);
}

export function editorCompletions(
  source: string,
  position: EditorPosition,
  uri?: string
): EditorCompletionList {
  const completions = requireEditorLanguage("editorCompletions", getMerman().editorCompletions);
  return completions(source, position.line, position.character, uri);
}

export function editorHover(
  source: string,
  position: EditorPosition,
  uri?: string
): EditorHover | null {
  const hover = requireEditorLanguage("editorHover", getMerman().editorHover);
  return hover(source, position.line, position.character, uri);
}

export function editorDocumentSymbols(
  source: string,
  uri?: string
): EditorDocumentSymbol[] {
  const documentSymbols = requireEditorLanguage(
    "editorDocumentSymbols",
    getMerman().editorDocumentSymbols
  );
  return documentSymbols(source, uri);
}

export function editorWorkspaceSymbols(
  source: string,
  query: string,
  uri?: string
): EditorSymbolInformation[] {
  const workspaceSymbols = requireEditorLanguage(
    "editorWorkspaceSymbols",
    getMerman().editorWorkspaceSymbols
  );
  return workspaceSymbols(source, query, uri);
}

export function editorDefinition(
  source: string,
  position: EditorPosition,
  uri?: string
): EditorLocation | null {
  const definition = requireEditorLanguage("editorDefinition", getMerman().editorDefinition);
  return definition(source, position.line, position.character, uri);
}

export function editorReferences(
  source: string,
  position: EditorPosition,
  includeDeclaration = true,
  uri?: string
): EditorLocation[] {
  const refs = requireEditorLanguage("editorReferences", getMerman().editorReferences);
  return refs(source, position.line, position.character, includeDeclaration, uri);
}

export function editorPrepareRename(
  source: string,
  position: EditorPosition,
  uri?: string
): EditorPrepareRename | null {
  const prepare = requireEditorLanguage("editorPrepareRename", getMerman().editorPrepareRename);
  return prepare(source, position.line, position.character, uri);
}

export function editorRename(
  source: string,
  position: EditorPosition,
  newName: string,
  uri?: string
): EditorWorkspaceEdit | null {
  const rename = requireEditorLanguage("editorRename", getMerman().editorRename);
  return rename(source, position.line, position.character, newName, uri);
}

export function editorSemanticTokenLegend(): EditorSemanticTokenLegend {
  const legend = requireEditorLanguage(
    "editorSemanticTokenLegend",
    getMerman().editorSemanticTokenLegend
  );
  return legend();
}

export function editorSemanticTokens(
  source: string,
  uri?: string
): EditorSemanticToken[] {
  const tokens = requireEditorLanguage("editorSemanticTokens", getMerman().editorSemanticTokens);
  return tokens(source, uri);
}

export function bindingCapabilities(): BindingCapabilities {
  const merman = getMerman();
  const capabilities = merman.bindingCapabilities?.();
  return capabilities
    ? normalizeBindingCapabilities(capabilities, merman)
    : {
        ...DEFAULT_BINDING_CAPABILITIES,
        editor_language: hasEditorLanguageBindings(merman),
      };
}

export function selectedRegistryProfile(): RegistryProfile {
  const profile = getMerman().selectedRegistryProfile?.();
  if (profile === "full" || profile === "tiny") {
    return profile;
  }
  return bindingCapabilities().core_full ? "full" : "tiny";
}

export function supportedDiagrams(): DiagramType[] {
  supportedDiagramsCache ??= getMerman().supportedDiagrams().map(assertDiagramType);
  return [...supportedDiagramsCache];
}

export function diagramFamilyCapabilities(): DiagramFamilyCapability[] {
  diagramFamilyCapabilitiesCache ??= (
    getMerman().diagramFamilyCapabilities?.() ?? []
  ).map(normalizeDiagramFamilyCapability);
  return diagramFamilyCapabilitiesCache.map((capability) => ({ ...capability }));
}

export function lintRuleCatalog(): LintRuleCatalogEntry[] {
  const rules = getMerman().lintRuleCatalog?.();
  if (!rules) {
    throw new Error("Merman lintRuleCatalog() is not available in this artifact.");
  }
  lintRuleCatalogCache ??= rules.map(normalizeLintRuleCatalogEntry);
  return lintRuleCatalogCache.map((rule) => ({
    ...rule,
    evidence: [...rule.evidence],
  }));
}

export function asciiSupportedDiagrams(): AsciiDiagramType[] {
  asciiSupportedDiagramsCache ??= getMerman()
    .asciiSupportedDiagrams()
    .map(assertAsciiDiagramType);
  return [...asciiSupportedDiagramsCache];
}

export function asciiCapabilities(): AsciiCapability[] {
  asciiCapabilitiesCache ??= (
    getMerman().asciiCapabilities?.() ?? fallbackAsciiCapabilities()
  ).map(normalizeAsciiCapability);
  return asciiCapabilitiesCache.map((capability) => ({
    ...capability,
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  }));
}

export function supportedThemes(): ThemeName[] {
  const merman = getMerman();
  supportedThemesCache ??= (
    merman.supportedThemes?.() ??
    merman.themes?.() ??
    SUPPORTED_THEMES
  ).map(assertThemeName);
  return [...supportedThemesCache];
}

export function supportedHostThemePresets(): HostThemePresetName[] {
  supportedHostThemePresetsCache ??= (
    getMerman().supportedHostThemePresets?.() ?? SUPPORTED_HOST_THEME_PRESETS
  ).map(assertHostThemePresetName);
  return [...supportedHostThemePresetsCache];
}

export function abiVersion(): number {
  return getMerman().abiVersion();
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

function assertDiagramType(diagram: string): DiagramType {
  if (isDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown diagram type: ${diagram}`);
}

function assertAsciiDiagramType(diagram: string): AsciiDiagramType {
  if (isAsciiDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown ASCII diagram type: ${diagram}`);
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
    metadata_id: metadataId,
    has_semantic_parser: Boolean(capability.has_semantic_parser),
    has_render_parser: Boolean(capability.has_render_parser),
  };
}

function normalizeLintRuleCatalogEntry(
  rule: LintRuleCatalogEntry
): LintRuleCatalogEntry {
  if (!rule || typeof rule !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog entry.");
  }
  return {
    id: assertStringField(rule.id, "lint rule id"),
    description: assertStringField(rule.description, "lint rule description"),
    evidence: assertStringArray(rule.evidence, "lint rule evidence"),
    default_severity: assertCatalogValue(rule.default_severity, [
      "error",
      "warning",
      "info",
      "hint",
    ]),
    category: assertCatalogValue(rule.category, [
      "parse",
      "semantic",
      "config",
      "resource",
      "compatibility",
      "layout",
      "render",
      "internal",
    ]),
    default_enabled: Boolean(rule.default_enabled),
    default_profile: assertCatalogValue(rule.default_profile, [
      "core",
      "recommended",
      "strict",
    ]),
    origin: assertCatalogValue(rule.origin, [
      "mermaid_syntax",
      "mermaid_compatibility",
      "merman_authoring",
      "merman_resource_policy",
      "merman_internal",
    ]),
    configurable: Boolean(rule.configurable),
    fixable: Boolean(rule.fixable),
  };
}

function assertStringField(value: unknown, label: string): string {
  if (typeof value === "string") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertStringArray(value: unknown, label: string): string[] {
  if (Array.isArray(value) && value.every((item) => typeof item === "string")) {
    return [...value];
  }
  throw new Error(`Merman WASM returned invalid ${label}.`);
}

function assertCatalogValue<const T extends string>(
  value: unknown,
  allowed: readonly T[]
): T {
  if (typeof value === "string" && (allowed as readonly string[]).includes(value)) {
    return value as T;
  }
  throw new Error(`Merman WASM returned an invalid lint rule catalog value: ${String(value)}`);
}

function normalizeAsciiCapability(capability: AsciiCapability): AsciiCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }

  const supportLevel = normalizeAsciiSupportLevel(capability.support_level);
  const evidence = Array.isArray(capability.evidence)
    ? capability.evidence.map(normalizeAsciiCapabilityEvidence)
    : [];

  return {
    diagram_type: capability.diagram_type,
    display_name:
      typeof capability.display_name === "string"
        ? capability.display_name
        : capability.diagram_type,
    support_level: supportLevel,
    summary_fallback: Boolean(capability.summary_fallback),
    supported_semantics: Array.isArray(capability.supported_semantics)
      ? capability.supported_semantics.map(String)
      : [],
    limits: Array.isArray(capability.limits) ? capability.limits.map(String) : [],
    evidence,
  };
}

function normalizeAsciiCapabilityEvidence(
  evidence: AsciiCapabilityEvidence
): AsciiCapabilityEvidence {
  return {
    kind: typeof evidence.kind === "string" ? evidence.kind : "support_matrix",
    source: typeof evidence.source === "string" ? evidence.source : "",
    note: typeof evidence.note === "string" ? evidence.note : "",
  };
}

function normalizeAsciiSupportLevel(level: unknown): AsciiSupportLevel {
  return level === "full" ||
    level === "partial" ||
    level === "summary" ||
    level === "unsupported"
    ? level
    : "unsupported";
}

function fallbackAsciiCapabilities(): AsciiCapability[] {
  return asciiSupportedDiagrams().map((diagramType) => ({
    diagram_type: diagramType,
    display_name: diagramType,
    support_level: "partial",
    summary_fallback: false,
    supported_semantics: [],
    limits: [],
    evidence: [
      {
        kind: "support_matrix",
        source: "SUPPORTED_ASCII_DIAGRAMS",
        note: "fallback capability synthesized from the legacy supported diagram list",
      },
    ],
  }));
}

function assertThemeName(theme: string): ThemeName {
  if (isThemeName(theme)) {
    return theme;
  }
  throw new Error(`Merman WASM returned unknown theme: ${theme}`);
}

function assertHostThemePresetName(preset: string): HostThemePresetName {
  if (isHostThemePresetName(preset)) {
    return preset;
  }
  throw new Error(`Merman WASM returned unknown host theme preset: ${preset}`);
}

function normalizeBindingCapabilities(
  capabilities: Partial<BindingCapabilities>,
  merman: MermanWasmModule
): BindingCapabilities {
  return {
    render: Boolean(capabilities.render),
    ascii: Boolean(capabilities.ascii),
    core_full: Boolean(capabilities.core_full),
    core_host: Boolean(capabilities.core_host),
    elk_layout: Boolean(capabilities.elk_layout),
    ratex_math: Boolean(capabilities.ratex_math),
    editor_language:
      capabilities.editor_language === undefined
        ? hasEditorLanguageBindings(merman)
        : Boolean(capabilities.editor_language),
  };
}

function requireEditorLanguage<T>(
  apiName: string,
  binding: T | undefined
): T {
  if (!bindingCapabilities().editor_language || binding === undefined) {
    throw new Error(`Merman ${apiName}() is not available in this artifact.`);
  }
  return binding;
}

function hasEditorLanguageBindings(merman: MermanWasmModule): boolean {
  return (
    typeof merman.editorDiagnostics === "function" &&
    typeof merman.editorCodeActions === "function" &&
    typeof merman.editorCompletions === "function" &&
    typeof merman.editorHover === "function" &&
    typeof merman.editorDocumentSymbols === "function" &&
    typeof merman.editorWorkspaceSymbols === "function" &&
    typeof merman.editorDefinition === "function" &&
    typeof merman.editorReferences === "function" &&
    typeof merman.editorPrepareRename === "function" &&
    typeof merman.editorRename === "function" &&
    typeof merman.editorSemanticTokenLegend === "function" &&
    typeof merman.editorSemanticTokens === "function"
  );
}
