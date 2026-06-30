import {
  SUPPORTED_ASCII_DIAGRAMS,
  type AsciiCapability,
  type AsciiSupportLevel,
} from "@mermanjs/web";

export const FALLBACK_ASCII_SUPPORTED_TYPES = SUPPORTED_ASCII_DIAGRAMS;

export type AsciiSupportedType =
  (typeof FALLBACK_ASCII_SUPPORTED_TYPES)[number];
export type { AsciiCapability, AsciiSupportLevel };

type FallbackCapabilityInput = {
  diagramType: AsciiSupportedType;
  displayName: string;
  supportLevel: AsciiSupportLevel;
  summaryFallback?: boolean;
  limits: string[];
};

const FALLBACK_ASCII_CAPABILITY_INPUTS: readonly FallbackCapabilityInput[] = [
  {
    diagramType: "class",
    displayName: "Class",
    supportLevel: "partial",
    summaryFallback: true,
    limits: [
      "namespace containers are not drawn as nested boxes",
      "dense or grid-budgeted relation scenes can summarize",
    ],
  },
  {
    diagramType: "er",
    displayName: "ER",
    supportLevel: "partial",
    summaryFallback: true,
    limits: [
      "complex cyclic topology can summarize",
      "unknown cardinality or relationship kinds are unsupported",
    ],
  },
  {
    diagramType: "flowchart",
    displayName: "Flowchart / graph",
    supportLevel: "full",
    limits: [
      "icons, images, callbacks, and links are not terminal output",
      "some uncommon route shapes are approximate",
    ],
  },
  {
    diagramType: "gantt",
    displayName: "Gantt",
    supportLevel: "summary",
    limits: ["output is a readable task summary, not terminal timeline geometry"],
  },
  {
    diagramType: "gitgraph",
    displayName: "GitGraph",
    supportLevel: "summary",
    limits: ["does not draw a full Git lane graph"],
  },
  {
    diagramType: "journey",
    displayName: "Journey",
    supportLevel: "summary",
    limits: ["does not draw Mermaid journey chart geometry"],
  },
  {
    diagramType: "kanban",
    displayName: "Kanban",
    supportLevel: "summary",
    limits: ["drag and board presentation metadata are not terminal output"],
  },
  {
    diagramType: "mindmap",
    displayName: "Mindmap",
    supportLevel: "summary",
    limits: ["icons, images, and rich browser node shapes are omitted or approximated"],
  },
  {
    diagramType: "packet",
    displayName: "Packet",
    supportLevel: "full",
    limits: ["visual styling beyond terminal borders is not represented"],
  },
  {
    diagramType: "sequence",
    displayName: "Sequence",
    supportLevel: "full",
    limits: ["actor presentation metadata and links are omitted"],
  },
  {
    diagramType: "state",
    displayName: "State",
    supportLevel: "partial",
    limits: ["some presentation metadata is omitted"],
  },
  {
    diagramType: "timeline",
    displayName: "Timeline",
    supportLevel: "summary",
    limits: ["does not draw Mermaid timeline geometry"],
  },
  {
    diagramType: "treeView",
    displayName: "TreeView",
    supportLevel: "full",
    limits: ["browser tree styling is not represented"],
  },
  {
    diagramType: "xychart",
    displayName: "XYChart",
    supportLevel: "partial",
    limits: [
      "browser hover tooltips and SVG-coordinate precision are not represented",
      "dense data uses terminal-compact layout",
    ],
  },
  {
    diagramType: "zenuml",
    displayName: "ZenUML",
    supportLevel: "partial",
    limits: ["external ZenUML compatibility is a subset"],
  },
] as const;

export const FALLBACK_ASCII_CAPABILITIES: readonly AsciiCapability[] =
  FALLBACK_ASCII_CAPABILITY_INPUTS.map((capability) => ({
    diagram_type: capability.diagramType,
    display_name: capability.displayName,
    support_level: capability.supportLevel,
    summary_fallback: capability.summaryFallback ?? false,
    supported_semantics: [],
    limits: capability.limits,
    evidence: [
      {
        kind: "support_matrix",
        source: "docs/rendering/ASCII_SUPPORT_MATRIX.md",
        note: "playground fallback capability synthesized from tracked support matrix",
      },
    ],
  }));

export function normalizeAsciiDiagramType(diagramType: string): string {
  return diagramType === "gitGraph" ? "gitgraph" : diagramType;
}

export function isAsciiSupported(
  diagramType: string,
  supportedTypes: readonly string[] = FALLBACK_ASCII_SUPPORTED_TYPES
): boolean {
  return supportedTypes.includes(normalizeAsciiDiagramType(diagramType));
}

export function asciiSupportLabelKey(
  capability: Pick<AsciiCapability, "support_level" | "summary_fallback"> | null
): string {
  if (!capability) {
    return "asciiSupport.unsupported";
  }
  if (capability.support_level === "summary" || capability.summary_fallback) {
    return "asciiSupport.summary";
  }
  return `asciiSupport.levels.${capability.support_level}`;
}

export function asciiSupportDescription(
  capability: Pick<AsciiCapability, "limits"> | null
): string {
  return capability?.limits?.find((limit) => limit.trim().length > 0) ?? "";
}
