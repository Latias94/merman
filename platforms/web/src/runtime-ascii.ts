import { currentRuntimeState, encodeOptions, getMerman } from "./runtime-core.js";
import { isAsciiDiagramType } from "./public-catalog.js";
import type {
  AsciiCapability,
  AsciiCapabilityEvidence,
  AsciiDiagramType,
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

  return {
    diagram_type: capability.diagram_type,
    display_name:
      typeof capability.display_name === "string"
        ? capability.display_name
        : capability.diagram_type,
    support_level: normalizeAsciiSupportLevel(capability.support_level),
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
