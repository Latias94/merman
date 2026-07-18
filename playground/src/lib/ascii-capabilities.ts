import { useMemo } from "react";
import {
  FALLBACK_ASCII_CAPABILITIES,
  FALLBACK_ASCII_SUPPORTED_TYPES,
  type AsciiCapability,
  normalizeAsciiDiagramType,
} from "@/src/lib/ascii-support";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";

export function useAsciiSupport() {
  const facade = useMermanRuntime(selectMermanFacade);
  const capabilities = useMemo(
    () =>
      facade
        ? facade.getAsciiCapabilities().map(normalizeCapability)
        : FALLBACK_ASCII_CAPABILITIES.map(normalizeCapability),
    [facade]
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
      : (facade?.getAsciiSupportedDiagrams() ?? FALLBACK_ASCII_SUPPORTED_TYPES).map(
          normalizeAsciiDiagramType
        );
  }, [capabilities, facade]);

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
