import { SUPPORTED_DIAGRAMS } from "./generated/diagram-catalog.js";
import { RUNTIME_CATALOG_MAX_SAFE_INTEGER } from "./generated/binding-contract.js";
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

export const BUNDLED_THEME_PRESETS = [
  "editor-light",
  "editor-dark",
  "one-dark",
  "gruvbox-light",
  "gruvbox-dark",
  "ayu-light",
  "ayu-dark",
] as const;

export type BundledThemePresetName = (typeof BUNDLED_THEME_PRESETS)[number];

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
] as const;

export type AsciiDiagramType = (typeof SUPPORTED_ASCII_DIAGRAMS)[number];

export const DIAGRAMMATIC_ASCII_DIAGRAMS = [
  "class",
  "er",
  "flowchart",
  "sequence",
  "state",
  "xychart",
] as const satisfies readonly AsciiDiagramType[];

export type DiagrammaticAsciiDiagramType =
  (typeof DIAGRAMMATIC_ASCII_DIAGRAMS)[number];

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
  "MERMAN_BUSY",
  "MERMAN_CANCELLED",
] as const;

export type BindingStatusCodeName = (typeof BINDING_STATUS_CODE_NAMES)[number];

export type BindingErrorKind =
  | "generic"
  | "unknown-operation"
  | "missing-capability"
  | "busy"
  | "reentrant-call";

/**
 * A lossless unsigned resource count. Safe integers use `number`; wider `u64` values use a
 * canonical decimal `string`.
 */
export type BindingResourceCount = number | string;

export interface BindingResourceErrorDetails {
  cause: string;
  limit_id: string;
  phase: string;
  actual: BindingResourceCount;
  max: BindingResourceCount;
  profile: string;
}

export interface BindingDiagnosticSpan {
  start: number;
  end: number;
  kind: "exact" | "insertion-point" | "fallback" | string;
}

export interface BindingDiagnosticErrorDetails {
  code: string;
  span: BindingDiagnosticSpan | null;
  field: string | null;
  diagram_type: string | null;
}

export interface BindingCancellationErrorDetails {
  reason: string;
  phase: string;
}

export interface BindingErrorPayload {
  version: number;
  ok: false;
  code: number;
  code_name: BindingStatusCodeName | string;
  kind: BindingErrorKind | string;
  capability_id: RuntimeCapabilityId | string | null;
  details?: {
    resource?: BindingResourceErrorDetails;
    diagnostic?: BindingDiagnosticErrorDetails;
    icon_registry?: Record<string, unknown>;
    cancellation?: BindingCancellationErrorDetails;
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
  [key: string]: unknown;
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

export type LintRuleTag = "deprecated";

export interface LintRuleCatalogEntry {
  id: string;
  description: string;
  evidence: string[];
  default_severity: LintRuleSeverity;
  category: LintRuleCategory;
  tags?: LintRuleTag[];
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
export type AsciiSemanticCoverage = "full" | "partial" | null;
export type AsciiPrimaryProjection =
  | "diagrammatic"
  | "structured_text"
  | "none";
export type AsciiLayoutProfile = "canonical" | "compact";
export type AsciiWidthProfile = "unicode" | "cjk";
export type AsciiOutputEncoding =
  | "plain"
  | "ansi16"
  | "ansi256"
  | "truecolor"
  | "html";

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
  semantic_coverage: AsciiSemanticCoverage;
  primary_projection: AsciiPrimaryProjection;
  structured_text_fallback: boolean;
  layout_profiles: AsciiLayoutProfile[];
  width_profiles: AsciiWidthProfile[];
  encodings: AsciiOutputEncoding[];
  fallback_encodings: AsciiOutputEncoding[];
  /** Compatibility view derived from semantic coverage and the primary projection. */
  support_level: AsciiSupportLevel;
  supported_semantics: string[];
  limits: string[];
  evidence: AsciiCapabilityEvidence[];
}

export function isThemeName(theme: string): theme is ThemeName {
  return (SUPPORTED_THEMES as readonly string[]).includes(theme);
}

export function isBundledThemePresetName(
  preset: string
): preset is BundledThemePresetName {
  return (BUNDLED_THEME_PRESETS as readonly string[]).includes(preset);
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

const MAX_SAFE_RESOURCE_COUNT_DECIMAL = String(RUNTIME_CATALOG_MAX_SAFE_INTEGER);
const U64_MAX_DECIMAL = "18446744073709551615";
const CANONICAL_WIDE_UNSIGNED_DECIMAL = /^[1-9]\d*$/;
const BINDING_CANCELLATION_REASONS = new Set([
  "requested",
  "deadline_exceeded",
]);
const OPERATION_PHASE_IDENTIFIER = /^[a-z][a-z0-9_]*$/;

function isBindingResourceCount(value: unknown): value is BindingResourceCount {
  if (typeof value === "number") {
    return Number.isSafeInteger(value) && value >= 0;
  }
  if (typeof value !== "string" || !CANONICAL_WIDE_UNSIGNED_DECIMAL.test(value)) {
    return false;
  }
  return compareCanonicalUnsignedDecimals(value, MAX_SAFE_RESOURCE_COUNT_DECIMAL) > 0 &&
    compareCanonicalUnsignedDecimals(value, U64_MAX_DECIMAL) <= 0;
}

function compareCanonicalUnsignedDecimals(left: string, right: string): number {
  if (left.length !== right.length) return left.length < right.length ? -1 : 1;
  if (left === right) return 0;
  return left < right ? -1 : 1;
}

export function isBindingErrorPayload(error: unknown): error is BindingErrorPayload {
  if (!error || typeof error !== "object") {
    return false;
  }
  const payload = error as Record<string, unknown>;
  const details =
    payload.details && typeof payload.details === "object"
      ? (payload.details as Record<string, unknown>)
      : undefined;
  const resource = details?.resource;
  const diagnostic = details?.diagnostic;
  const iconRegistry = details?.icon_registry;
  const cancellation = details?.cancellation;
  const resourceRecord =
    resource && typeof resource === "object"
      ? (resource as Record<string, unknown>)
      : undefined;
  const validResource =
    resource === undefined ||
    (!!resourceRecord &&
      typeof resourceRecord.cause === "string" &&
      resourceRecord.cause.length > 0 &&
      typeof resourceRecord.limit_id === "string" &&
      resourceRecord.limit_id.length > 0 &&
      typeof resourceRecord.phase === "string" &&
      resourceRecord.phase.length > 0 &&
      isBindingResourceCount(resourceRecord.actual) &&
      isBindingResourceCount(resourceRecord.max) &&
      typeof resourceRecord.profile === "string" &&
      resourceRecord.profile.length > 0);
  const diagnosticRecord =
    diagnostic && typeof diagnostic === "object"
      ? (diagnostic as Record<string, unknown>)
      : undefined;
  const diagnosticSpan = diagnosticRecord?.span;
  const diagnosticSpanRecord =
    diagnosticSpan && typeof diagnosticSpan === "object"
      ? (diagnosticSpan as Record<string, unknown>)
      : undefined;
  const validDiagnosticSpan =
    diagnosticSpan === null ||
    (!!diagnosticSpanRecord &&
      Number.isSafeInteger(diagnosticSpanRecord.start) &&
      Number.isSafeInteger(diagnosticSpanRecord.end) &&
      Number(diagnosticSpanRecord.start) >= 0 &&
      Number(diagnosticSpanRecord.end) >= Number(diagnosticSpanRecord.start) &&
      ["exact", "insertion-point", "fallback"].includes(
        diagnosticSpanRecord.kind as string
      ) &&
      (diagnosticSpanRecord.kind !== "insertion-point" ||
        Number(diagnosticSpanRecord.start) === Number(diagnosticSpanRecord.end)));
  const validDiagnostic =
    diagnostic === undefined ||
    (!!diagnosticRecord &&
      typeof diagnosticRecord.code === "string" &&
      diagnosticRecord.code.length > 0 &&
      validDiagnosticSpan &&
      (diagnosticRecord.field === null ||
        typeof diagnosticRecord.field === "string") &&
      (diagnosticRecord.diagram_type === null ||
        typeof diagnosticRecord.diagram_type === "string"));
  const validCancellation =
    cancellation === undefined ||
    (() => {
      if (!cancellation || typeof cancellation !== "object") return false;
      const cancellationRecord = cancellation as Record<string, unknown>;
      return (
        typeof cancellationRecord.reason === "string" &&
        BINDING_CANCELLATION_REASONS.has(cancellationRecord.reason) &&
        typeof cancellationRecord.phase === "string" &&
        OPERATION_PHASE_IDENTIFIER.test(cancellationRecord.phase)
      );
    })();
  const hasConsistentCancellationTerminal =
    cancellation === undefined ||
    (payload.kind === "generic" &&
      payload.capability_id === null &&
      resource === undefined &&
      diagnostic === undefined &&
      iconRegistry === undefined);
  const hasValidDetails =
    payload.details === undefined ||
    (!!details &&
      validResource &&
      validDiagnostic &&
      validCancellation &&
      (iconRegistry === undefined ||
        (!!iconRegistry && typeof iconRegistry === "object")));
  return (
    payload.ok === false &&
    typeof payload.version === "number" &&
    Number.isSafeInteger(payload.version) &&
    payload.version >= 1 &&
    typeof payload.code === "number" &&
    Number.isSafeInteger(payload.code) &&
    payload.code > 0 &&
    payload.code < BINDING_STATUS_CODE_NAMES.length &&
    typeof payload.code_name === "string" &&
    BINDING_STATUS_CODE_NAMES[payload.code] === payload.code_name &&
    typeof payload.kind === "string" &&
    (payload.capability_id === null ||
      typeof payload.capability_id === "string") &&
    (payload.code_name !== "MERMAN_CANCELLED" || cancellation !== undefined) &&
    (cancellation === undefined || payload.code_name === "MERMAN_CANCELLED") &&
    hasConsistentCancellationTerminal &&
    hasValidDetails &&
    typeof payload.message === "string"
  );
}

export function normalizeThemeName(theme: string | null | undefined): ThemeName {
  return theme && isThemeName(theme) ? theme : "default";
}

export function normalizeBundledThemePresetName(
  preset: string | null | undefined
): BundledThemePresetName | null {
  return preset && isBundledThemePresetName(preset) ? preset : null;
}
