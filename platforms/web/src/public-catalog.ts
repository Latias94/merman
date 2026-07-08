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
  analysis: boolean;
  ascii: boolean;
  core_full: boolean;
  core_host: boolean;
  elk_layout: boolean;
  ratex_math: boolean;
  editor_language: boolean;
  text_measurement: TextMeasurementCapabilities;
}

export interface TextMeasurementCapabilities {
  vendored: boolean;
  deterministic: boolean;
  host_callback: boolean;
  font_assets: boolean;
}

interface BindingCapabilityFlags {
  render: boolean;
  analysis: boolean;
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

export interface LintRuleCatalogResponse {
  version: number;
  rules: LintRuleCatalogEntry[];
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

export const CORE_BINDING_CAPABILITIES: BindingCapabilities = bindingCapabilities({
  render: false,
  analysis: true,
  ascii: false,
  core_full: false,
  core_host: false,
  elk_layout: false,
  ratex_math: false,
  editor_language: false,
});

export const RENDER_BINDING_CAPABILITIES: BindingCapabilities = bindingCapabilities({
  render: true,
  analysis: true,
  ascii: false,
  core_full: false,
  core_host: false,
  elk_layout: false,
  ratex_math: false,
  editor_language: false,
});

export const ASCII_BINDING_CAPABILITIES: BindingCapabilities = bindingCapabilities({
  render: false,
  analysis: false,
  ascii: true,
  core_full: false,
  core_host: false,
  elk_layout: false,
  ratex_math: false,
  editor_language: false,
});

export const FULL_BINDING_CAPABILITIES: BindingCapabilities = bindingCapabilities({
  render: true,
  analysis: true,
  ascii: true,
  core_full: true,
  core_host: true,
  elk_layout: true,
  ratex_math: false,
  editor_language: true,
});

export const DEFAULT_BINDING_CAPABILITIES: BindingCapabilities = FULL_BINDING_CAPABILITIES;

function bindingCapabilities(flags: BindingCapabilityFlags): BindingCapabilities {
  return {
    ...flags,
    text_measurement: {
      vendored: flags.render,
      deterministic: flags.render,
      host_callback: flags.render,
      font_assets: false,
    },
  };
}

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
