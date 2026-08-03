import { SUPPORTED_DIAGRAMS } from "./generated/diagram-catalog.js";
import {
  SYSTEM_ADAPTER_IDS,
  WEB_CAPABILITIES,
  WEB_CAPABILITY_IDS,
  WEB_BINDING_OPERATION_IDS,
  WEB_BINDING_OPERATIONS,
  WEB_OUTPUT_IDS,
  WEB_OUTPUTS,
} from "./generated/capability-surface.js";
import type {
  SystemAdapterId,
  WebBindingOperationId,
} from "./generated/capability-surface.js";

export { SUPPORTED_DIAGRAMS };
export {
  SYSTEM_ADAPTER_IDS,
  WEB_CAPABILITIES,
  WEB_CAPABILITY_IDS,
  WEB_BINDING_OPERATION_IDS,
  WEB_BINDING_OPERATIONS,
  WEB_OUTPUT_IDS,
  WEB_OUTPUTS,
};
export type { SystemAdapterId, WebBindingOperationId };

// Runtime vocabulary is returned by the selected artifact. It may contain
// capabilities that are valid for another target, so it must stay open here.
export type RuntimeCapabilityId = string;
export type RuntimeOutputId = string;
export type RuntimeOperationId = string;

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
  "MERMAN_UNSUPPORTED_OPERATION",
  "MERMAN_PANIC",
  "MERMAN_INTERNAL_ERROR",
  "MERMAN_RESOURCE_LIMIT_EXCEEDED",
] as const;

export type BindingStatusCodeName = (typeof BINDING_STATUS_CODE_NAMES)[number];

export type BindingErrorKind =
  | "generic"
  | "unknown-operation"
  | "missing-capability";

export interface BindingResourceErrorDetails {
  limit_id: string;
  phase: string;
  actual: number;
  max: number;
  profile: string;
}

export interface BindingErrorPayload {
  version: number;
  ok: false;
  code: number;
  code_name: BindingStatusCodeName | string;
  kind: BindingErrorKind | string;
  capability_id: RuntimeCapabilityId | string | null;
  details?: {
    resource: BindingResourceErrorDetails;
  };
  message: string;
}

export const TEXT_MEASUREMENT_PROVIDER_IDS = [
  "host-callback",
  "vendored",
] as const;

export type TextMeasurementProviderId = string;

export interface RuntimeCapabilities {
  [key: string]: unknown;
  capability_ids: RuntimeCapabilityId[];
  output_ids: RuntimeOutputId[];
  operation_ids: RuntimeOperationId[];
  system_adapter_ids: string[];
  text_measurement: TextMeasurementCapabilities | null;
}

export interface TextMeasurementCapabilities {
  protocol_version: number;
  provider_ids: TextMeasurementProviderId[];
}

export interface DiagramFamilyCapability {
  diagram_type: string;
  logical_family_kind: string;
  metadata_id: DiagramType | null;
  render_model_kind: string | null;
  has_detector: boolean;
  has_semantic_parser: boolean;
  has_editor_parser: boolean;
  has_combined_parser: boolean;
  has_render_parser: boolean;
  has_header: boolean;
  config_namespace: string | null;
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
  const resource =
    payload.details && typeof payload.details === "object"
      ? (payload.details as Record<string, unknown>).resource
      : undefined;
  const hasValidDetails =
    payload.details === undefined ||
    (!!resource &&
      typeof resource === "object" &&
      typeof (resource as Record<string, unknown>).limit_id === "string" &&
      typeof (resource as Record<string, unknown>).phase === "string" &&
      typeof (resource as Record<string, unknown>).actual === "number" &&
      typeof (resource as Record<string, unknown>).max === "number" &&
      typeof (resource as Record<string, unknown>).profile === "string");
  return (
    payload.ok === false &&
    typeof payload.version === "number" &&
    typeof payload.code === "number" &&
    typeof payload.code_name === "string" &&
    typeof payload.kind === "string" &&
    (payload.capability_id === null ||
      typeof payload.capability_id === "string") &&
    hasValidDetails &&
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
