import {
  currentRuntimeState,
  diagramFamilyCapabilities,
  encodeOptions,
  getMerman,
  UNAVAILABLE_DIAGRAM_DETECTION,
} from "./runtime-core.js";
import type {
  DiagramType,
  LintRuleCatalogEntry,
  LintRuleCatalogResponse,
} from "./public-catalog.js";
import type {
  AnalysisFactsResult,
  AnalysisResult,
  DiagramDetectionFacts,
  SvgBindingOptions,
  ValidationResult,
} from "./public-types.js";
import type { MermanRuntimeState } from "./runtime-state.js";

interface AnalysisRuntimeCache {
  diagramMetadataBySyntaxId: ReadonlyMap<string, DiagramType | null> | null;
  lintRuleCatalog: LintRuleCatalogEntry[] | null;
}

const analysisRuntimeCaches = new WeakMap<
  MermanRuntimeState,
  AnalysisRuntimeCache
>();

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
  const facts = getMerman().analysisFacts;
  if (!facts) {
    throw new Error("Merman analysisFacts() is not available in this artifact.");
  }
  return facts(source, encodeOptions(options));
}

export function detectDiagramFacts(
  source: string,
  options?: SvgBindingOptions | string
): DiagramDetectionFacts {
  try {
    const facts: unknown = analysisFacts(source, options);
    if (
      !isRecord(facts) ||
      facts.version !== 2 ||
      typeof facts.valid !== "boolean"
    ) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const diagrams = facts.diagrams;
    if (!Array.isArray(diagrams) || diagrams.length !== 1 || !isRecord(diagrams[0])) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const parseDisposition = diagrams[0].parse_disposition;
    if (parseDisposition === "unavailable") {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }
    if (parseDisposition !== "parsed" && parseDisposition !== "recovered") {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const syntax = diagrams[0].syntax;
    if (!isRecord(syntax)) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const syntaxId = syntax.diagram_type;
    const effectiveLayoutId = syntax.effective_layout;
    if (
      typeof syntaxId !== "string" ||
      syntaxId.trim().length === 0 ||
      typeof effectiveLayoutId !== "string" ||
      effectiveLayoutId.trim().length === 0
    ) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const diagramType = diagramMetadataBySyntaxId().get(syntaxId);
    if (diagramType == null) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    return Object.freeze({
      status: "available",
      validity: parseDisposition === "parsed" ? "valid" : "recoverable-invalid",
      diagramType,
      syntaxId,
      effectiveLayoutId,
    });
  } catch {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
}

export function analyzeDocument(
  source: string,
  uri: string,
  options?: SvgBindingOptions | string
): AnalysisResult {
  if (typeof uri !== "string" || uri.length === 0) {
    throw new TypeError("analyzeDocument() requires a non-empty document URI.");
  }
  const analyzeDocument = getMerman().analyzeDocument;
  if (!analyzeDocument) {
    throw new Error("Merman analyzeDocument() is not available in this artifact.");
  }
  return analyzeDocument(source, uri, encodeOptions(options));
}

export function analyzeDocumentFacts(
  source: string,
  uri: string,
  options?: SvgBindingOptions | string
): AnalysisFactsResult {
  if (typeof uri !== "string" || uri.length === 0) {
    throw new TypeError("analyzeDocumentFacts() requires a non-empty document URI.");
  }
  const analyzeDocumentFacts = getMerman().analyzeDocumentFacts;
  if (!analyzeDocumentFacts) {
    throw new Error("Merman analyzeDocumentFacts() is not available in this artifact.");
  }
  return analyzeDocumentFacts(source, uri, encodeOptions(options));
}

export function validate(source: string, options?: SvgBindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function lintRuleCatalog(): LintRuleCatalogEntry[] {
  const cache = currentAnalysisRuntimeCache();
  if (!cache.lintRuleCatalog) {
    const response = getMerman().lintRuleCatalog?.();
    if (!response) {
      throw new Error("Merman lintRuleCatalog() is not available in this artifact.");
    }
    cache.lintRuleCatalog = normalizeLintRuleCatalogResponse(response);
  }
  return cache.lintRuleCatalog.map((rule) => ({
    ...rule,
    evidence: [...rule.evidence],
    tags: [...(rule.tags ?? [])],
  }));
}

function diagramMetadataBySyntaxId(): ReadonlyMap<string, DiagramType | null> {
  const cache = currentAnalysisRuntimeCache();
  if (cache.diagramMetadataBySyntaxId) {
    return cache.diagramMetadataBySyntaxId;
  }

  const index = new Map<string, DiagramType | null>();
  for (const capability of diagramFamilyCapabilities()) {
    const syntaxId = capability.diagram_type;
    if (index.has(syntaxId)) {
      index.set(syntaxId, null);
    } else {
      index.set(syntaxId, capability.metadata_id);
    }
  }
  cache.diagramMetadataBySyntaxId = index;
  return index;
}

function currentAnalysisRuntimeCache(): AnalysisRuntimeCache {
  const state = currentRuntimeState();
  let cache = analysisRuntimeCaches.get(state);
  if (!cache) {
    cache = {
      diagramMetadataBySyntaxId: null,
      lintRuleCatalog: null,
    };
    analysisRuntimeCaches.set(state, cache);
  }
  return cache;
}

function normalizeLintRuleCatalogResponse(
  response: LintRuleCatalogResponse
): LintRuleCatalogEntry[] {
  if (!response || typeof response !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog response.");
  }
  if (response.version !== 1) {
    throw new Error(
      `Merman WASM returned unsupported lint rule catalog version: ${String(response.version)}.`
    );
  }
  if (!Array.isArray(response.rules)) {
    throw new Error("Merman WASM returned a lint rule catalog response without rules.");
  }
  return response.rules.map(normalizeLintRuleCatalogEntry);
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
    tags: normalizeLintRuleTags(rule.tags),
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

function normalizeLintRuleTags(
  tags: LintRuleCatalogEntry["tags"]
): NonNullable<LintRuleCatalogEntry["tags"]> {
  if (tags === undefined) {
    return [];
  }
  if (!Array.isArray(tags)) {
    throw new Error("Merman WASM returned invalid lint rule tags.");
  }
  return tags.map((tag) => assertCatalogValue(tag, ["deprecated"]));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
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
