import type {
  AsciiCapability,
  BindingCapabilities,
  BindingStatusCodeName,
  DiagramFamilyCapability,
  HostThemePresetName,
  LintBindingOptions,
  LintRuleCatalogEntry,
  LintRuleCatalogResponse,
  LintRuleCategory,
  LintRuleSeverity,
} from "./public-catalog.js";

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
  max_class_nodes?: number;
  max_class_edges?: number;
  max_class_namespaces?: number;
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

export interface AnalysisBindingOptions {
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
  site_config?: MermaidSiteConfig;
  parse?: ParseOptions;
  resources?: ResourceOptions;
  lint?: LintBindingOptions;
}

export interface CommonBindingOptions extends AnalysisBindingOptions {
  version?: number;
  analysis?: AnalysisBindingOptions;
  merman?: AnalysisBindingOptions;
}

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
  relation_summary_diagnostics?: boolean;
  relationSummaryDiagnostics?: boolean;
}

export interface AsciiBindingOptions extends CommonBindingOptions {
  ascii?: AsciiRenderOptions;
}

export interface SvgBindingOptions extends CommonBindingOptions {
  host_theme?: HostThemeOptions;
  layout?: LayoutOptions;
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
  source_mapped_spans: boolean;
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

export type EditorSemanticFactSource =
  | "text_scan"
  | "parser_complete"
  | "parser_complete_degraded_spans"
  | "parser_recovered"
  | "parser_recovered_degraded_spans";

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
  analyze?: (source: string, optionsJson?: string | null) => AnalysisResult;
  analyzeJson?: (source: string, optionsJson?: string | null) => AnalysisResult;
  analysisFacts?: (source: string, optionsJson?: string | null) => AnalysisFactsResult;
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
  editorWorkspaceSymbols?: (
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
  editorSemanticTokenLegend?: () => EditorSemanticTokenLegend;
  editorSemanticTokens?: (
    source: string,
    uri?: string | null,
    optionsJson?: string | null
  ) => EditorSemanticToken[];
  asciiSupportedDiagrams: () => string[];
  asciiCapabilities: () => AsciiCapability[];
  bindingCapabilities: () => BindingCapabilities;
  selectedRegistryProfile: () => string;
  diagramFamilyCapabilities: () => DiagramFamilyCapability[];
  lintRuleCatalog?: () => LintRuleCatalogResponse;
  supportedDiagrams: () => string[];
  supportedHostThemePresets: () => string[];
  supportedThemes: () => string[];
}

export type MermanWasmLoader = () => Promise<MermanWasmModule>;

export interface MermanInitOptions {
  loader?: MermanWasmLoader;
  wasm?: unknown;
}

export type MermanInitInput = MermanWasmLoader | MermanInitOptions;
