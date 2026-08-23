import { currentRuntimeState, encodeOptions, getMerman } from "./runtime-core.js";
import { isAsciiDiagramType } from "./public-catalog.js";
import type {
  AsciiCapability,
  AsciiCapabilityEvidence,
  AsciiDiagramType,
  AsciiPrimaryProjection,
  AsciiSemanticCoverage,
  AsciiSupportLevel,
} from "./public-catalog.js";
import type { AsciiBindingOptions } from "./public-types.js";
import type { MermanRuntimeState } from "./runtime-state.js";

interface AsciiRuntimeCache {
  capabilities: AsciiCapability[] | null;
  supportedDiagrams: AsciiDiagramType[] | null;
}

const asciiRuntimeCaches = new WeakMap<MermanRuntimeState, AsciiRuntimeCache>();

export function renderAscii(
  source: string,
  options?: AsciiBindingOptions | string
): string {
  return getMerman().renderAscii(source, encodeOptions(options));
}

export function asciiSupportedDiagrams(): AsciiDiagramType[] {
  const cache = currentAsciiRuntimeCache();
  cache.supportedDiagrams ??= getMerman()
    .asciiSupportedDiagrams()
    .map(assertAsciiDiagramType);
  return [...cache.supportedDiagrams];
}

export function asciiCapabilities(): AsciiCapability[] {
  const cache = currentAsciiRuntimeCache();
  cache.capabilities ??= getMerman()
    .asciiCapabilities()
    .map(normalizeAsciiCapability);
  return cache.capabilities.map((capability) => ({
    ...capability,
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  }));
}

export function asciiDiagrammaticDiagrams(): AsciiDiagramType[] {
  return asciiCapabilities()
    .filter((capability) => capability.primary_projection === "diagrammatic")
    .map((capability) => assertAsciiDiagramType(capability.diagram_type));
}

function currentAsciiRuntimeCache(): AsciiRuntimeCache {
  const state = currentRuntimeState();
  let cache = asciiRuntimeCaches.get(state);
  if (!cache) {
    cache = {
      capabilities: null,
      supportedDiagrams: null,
    };
    asciiRuntimeCaches.set(state, cache);
  }
  return cache;
}

function assertAsciiDiagramType(diagram: string): AsciiDiagramType {
  if (isAsciiDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown ASCII diagram type: ${diagram}`);
}

function normalizeAsciiCapability(capability: AsciiCapability): AsciiCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }

  const evidence = Array.isArray(capability.evidence)
    ? capability.evidence.map(normalizeAsciiCapabilityEvidence)
    : [];
  const semanticCoverage = normalizeAsciiSemanticCoverage(
    capability.semantic_coverage
  );
  const primaryProjection = normalizeAsciiPrimaryProjection(
    capability.primary_projection
  );
  const supportLevel = deriveAsciiSupportLevel(
    semanticCoverage,
    primaryProjection
  );
  if (capability.support_level !== supportLevel) {
    throw new Error(
      "Merman WASM returned an inconsistent ASCII compatibility support level."
    );
  }

  return {
    diagram_type: capability.diagram_type,
    display_name:
      typeof capability.display_name === "string"
        ? capability.display_name
        : capability.diagram_type,
    semantic_coverage: semanticCoverage,
    primary_projection: primaryProjection,
    structured_text_fallback: Boolean(capability.structured_text_fallback),
    support_level: supportLevel,
    supported_semantics: Array.isArray(capability.supported_semantics)
      ? capability.supported_semantics.map(String)
      : [],
    limits: Array.isArray(capability.limits) ? capability.limits.map(String) : [],
    evidence,
  };
}

function normalizeAsciiSemanticCoverage(level: unknown): AsciiSemanticCoverage {
  if (level === null || level === "full" || level === "partial") {
    return level;
  }
  throw new Error("Merman WASM returned an invalid ASCII semantic coverage.");
}

function normalizeAsciiPrimaryProjection(
  projection: unknown
): AsciiPrimaryProjection {
  if (
    projection === "diagrammatic" ||
    projection === "structured_text" ||
    projection === "none"
  ) {
    return projection;
  }
  throw new Error("Merman WASM returned an invalid ASCII primary projection.");
}

function deriveAsciiSupportLevel(
  coverage: AsciiSemanticCoverage,
  projection: AsciiPrimaryProjection
): AsciiSupportLevel {
  if (coverage === null || projection === "none") {
    if (coverage === null && projection === "none") return "unsupported";
    throw new Error("Merman WASM returned an invalid ASCII capability combination.");
  }
  if (projection === "structured_text") return "summary";
  return coverage;
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
