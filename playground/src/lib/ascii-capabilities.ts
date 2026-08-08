import { useMemo } from "react";
import {
  FALLBACK_ASCII_CAPABILITIES,
  FALLBACK_ASCII_SUPPORTED_TYPES,
  type AsciiCapability,
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
      map.set(capability.diagram_type, capability);
    }
    return map;
  }, [capabilities]);

  const supportedTypes = useMemo(() => {
    const fromCapabilities = capabilities
      .filter((capability) => capability.primary_projection !== "none")
      .map((capability) => capability.diagram_type);
    return fromCapabilities.length > 0
      ? fromCapabilities
      : (facade?.getAsciiSupportedDiagrams() ?? FALLBACK_ASCII_SUPPORTED_TYPES);
  }, [capabilities, facade]);

  const supportedTypeSet = useMemo(() => new Set(supportedTypes), [supportedTypes]);

  return useMemo(
    () => ({
      capabilities,
      capabilityMap,
      supportedTypes,
      capabilityFor: (diagramType: string) =>
        capabilityMap.get(diagramType) ?? null,
      isSupported: (diagramType: string) =>
        supportedTypeSet.has(diagramType),
    }),
    [capabilities, capabilityMap, supportedTypeSet, supportedTypes]
  );
}

function normalizeCapability(capability: AsciiCapability): AsciiCapability {
  return {
    ...capability,
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  };
}
