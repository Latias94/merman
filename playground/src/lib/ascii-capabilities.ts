import { useMemo } from "react";
import { useMerman } from "@/src/hooks/useMerman";
import {
  FALLBACK_ASCII_SUPPORTED_TYPES,
  normalizeAsciiDiagramType,
} from "@/src/lib/ascii-support";

export function useAsciiSupport() {
  const { ready, getAsciiSupportedDiagrams } = useMerman();
  const supportedTypes = useMemo(
    () =>
      ready
        ? getAsciiSupportedDiagrams().map(normalizeAsciiDiagramType)
        : [...FALLBACK_ASCII_SUPPORTED_TYPES],
    [getAsciiSupportedDiagrams, ready]
  );

  const supportedTypeSet = useMemo(() => new Set(supportedTypes), [supportedTypes]);

  return useMemo(
    () => ({
      supportedTypes,
      isSupported: (diagramType: string) =>
        supportedTypeSet.has(normalizeAsciiDiagramType(diagramType)),
    }),
    [supportedTypeSet, supportedTypes]
  );
}
