import type { DiagramDetectionFacts } from "@mermanjs/web";

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
