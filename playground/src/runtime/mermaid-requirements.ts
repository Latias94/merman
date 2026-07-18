import type { DiagramDetectionFacts } from "@mermanjs/web";

export const MERMAID_JS_VERSION = "11.16.0";
export const MERMAID_ZENUML_VERSION = "0.2.2";
export const MERMAID_LAYOUT_ELK_VERSION = "0.2.1";

export interface MermaidExternalRequirements {
  readonly elkLayouts: boolean;
  readonly zenuml: boolean;
}

export const NO_MERMAID_EXTERNAL_REQUIREMENTS: MermaidExternalRequirements =
  Object.freeze({
    elkLayouts: false,
    zenuml: false,
  });

// Pinned to @mermaid-js/layout-elk 0.2.1 layouts.ts (Mermaid 11.16).
const ELK_LAYOUT_IDS = new Set([
  "elk",
  "elk.stress",
  "elk.force",
  "elk.mrtree",
  "elk.sporeOverlap",
]);

export function mermaidExternalRequirementsFor(
  facts: DiagramDetectionFacts
): MermaidExternalRequirements {
  if (facts.status === "unavailable") {
    return NO_MERMAID_EXTERNAL_REQUIREMENTS;
  }

  return {
    elkLayouts: ELK_LAYOUT_IDS.has(facts.effectiveLayoutId),
    zenuml: facts.diagramType === "zenuml",
  };
}
