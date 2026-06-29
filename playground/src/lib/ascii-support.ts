import { SUPPORTED_ASCII_DIAGRAMS } from "@mermanjs/web";

export const FALLBACK_ASCII_SUPPORTED_TYPES = SUPPORTED_ASCII_DIAGRAMS;

export type AsciiSupportedType =
  (typeof FALLBACK_ASCII_SUPPORTED_TYPES)[number];

export function normalizeAsciiDiagramType(diagramType: string): string {
  return diagramType === "gitGraph" ? "gitgraph" : diagramType;
}

export function isAsciiSupported(
  diagramType: string,
  supportedTypes: readonly string[] = FALLBACK_ASCII_SUPPORTED_TYPES
): boolean {
  return supportedTypes.includes(normalizeAsciiDiagramType(diagramType));
}
