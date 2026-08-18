import type { AsciiCapability } from "@mermanjs/web";
import { GENERATED_ASCII_CAPABILITIES } from "../generated/ascii-capabilities.ts";

export const FALLBACK_ASCII_CAPABILITIES = GENERATED_ASCII_CAPABILITIES;
export const FALLBACK_ASCII_SUPPORTED_TYPES = FALLBACK_ASCII_CAPABILITIES.filter(
  (capability) => capability.primary_projection !== "none"
).map((capability) => capability.diagram_type);

export type { AsciiCapability };

export function isAsciiSupported(
  diagramType: string,
  supportedTypes: readonly string[] = FALLBACK_ASCII_SUPPORTED_TYPES
): boolean {
  return supportedTypes.includes(diagramType);
}

export function asciiSupportLabelKey(
  capability: Pick<
    AsciiCapability,
    "semantic_coverage" | "primary_projection"
  > | null
): string {
  if (!capability || capability.primary_projection === "none") {
    return "asciiSupport.unsupported";
  }
  if (capability.primary_projection === "structured_text") {
    return "asciiSupport.structuredText";
  }
  return `asciiSupport.levels.${capability.semantic_coverage}`;
}

export function asciiSupportDescription(
  capability: Pick<AsciiCapability, "limits"> | null
): string {
  return capability?.limits?.find((limit) => limit.trim().length > 0) ?? "";
}
