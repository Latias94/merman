import { useMemo } from "react";
import { useMerman } from "@/src/hooks/useMerman";
import {
  FALLBACK_ASCII_CAPABILITIES,
  type AsciiCapability,
  normalizeAsciiDiagramType,
} from "@/src/lib/ascii-support";

export function useAsciiSupport() {
  const { ready, getAsciiCapabilities, getAsciiSupportedDiagrams } = useMerman();
  const capabilities = useMemo(
    () =>
      ready
        ? getAsciiCapabilities().map(normalizeCapability)
        : FALLBACK_ASCII_CAPABILITIES.map(normalizeCapability),
    [getAsciiCapabilities, ready]
  );

  const capabilityMap = useMemo(() => {
    const map = new Map<string, AsciiCapability>();
    for (const capability of capabilities) {
      map.set(normalizeAsciiDiagramType(capability.diagram_type), capability);
    }
    return map;
  }, [capabilities]);

  const supportedTypes = useMemo(() => {
    const fromCapabilities = capabilities
      .filter((capability) => capability.support_level !== "unsupported")
      .map((capability) => normalizeAsciiDiagramType(capability.diagram_type));
    return fromCapabilities.length > 0
      ? fromCapabilities
      : getAsciiSupportedDiagrams().map(normalizeAsciiDiagramType);
  }, [capabilities, getAsciiSupportedDiagrams]);

  const supportedTypeSet = useMemo(() => new Set(supportedTypes), [supportedTypes]);

  return useMemo(
    () => ({
      capabilities,
      capabilityMap,
      supportedTypes,
      capabilityFor: (diagramType: string) =>
        capabilityMap.get(normalizeAsciiDiagramType(diagramType)) ?? null,
      isSupported: (diagramType: string) =>
        supportedTypeSet.has(normalizeAsciiDiagramType(diagramType)),
    }),
    [capabilities, capabilityMap, supportedTypeSet, supportedTypes]
  );
}

function normalizeCapability(capability: AsciiCapability): AsciiCapability {
  return {
    ...capability,
    diagram_type: normalizeAsciiDiagramType(capability.diagram_type),
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  };
}
