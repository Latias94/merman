import {
  DIAGRAMMATIC_ASCII_DIAGRAMS,
  SUPPORTED_ASCII_DIAGRAMS,
  type AsciiCapability,
  type AsciiPrimaryProjection,
  type AsciiSemanticCoverage,
  type AsciiSupportLevel,
} from "@mermanjs/web";

export const FALLBACK_ASCII_SUPPORTED_TYPES = SUPPORTED_ASCII_DIAGRAMS;
export const FALLBACK_ASCII_DIAGRAMMATIC_TYPES = DIAGRAMMATIC_ASCII_DIAGRAMS;

export type AsciiSupportedType =
  (typeof FALLBACK_ASCII_SUPPORTED_TYPES)[number];
export type {
  AsciiCapability,
  AsciiPrimaryProjection,
  AsciiSemanticCoverage,
  AsciiSupportLevel,
};

type FallbackCapabilityInput = {
  displayName: string;
  semanticCoverage: Exclude<AsciiSemanticCoverage, null>;
  primaryProjection: Exclude<AsciiPrimaryProjection, "none">;
  structuredTextFallback?: boolean;
  supportedSemantics?: string[];
  limits: string[];
};

const BUILT_IN_TYPED_FAMILIES = [
  "architecture",
  "block",
  "c4",
  "class",
  "cynefin",
  "er",
  "eventmodeling",
  "flowchart",
  "gantt",
  "gitgraph",
  "info",
  "ishikawa",
  "journey",
  "kanban",
  "mindmap",
  "packet",
  "pie",
  "quadrantchart",
  "radar",
  "railroad",
  "requirement",
  "sankey",
  "sequence",
  "state",
  "timeline",
  "treeView",
  "treemap",
  "venn",
  "wardley",
  "xychart",
  "zenuml",
] as const;

const FALLBACK_ASCII_CAPABILITY_INPUTS: Partial<
  Record<(typeof BUILT_IN_TYPED_FAMILIES)[number], FallbackCapabilityInput>
> = {
  class: {
    displayName: "Class",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    structuredTextFallback: true,
    limits: [
      "namespace containers are not drawn as nested boxes",
      "dense or grid-budgeted relation scenes can summarize",
    ],
  },
  er: {
    displayName: "ER",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    structuredTextFallback: true,
    limits: [
      "complex cyclic topology can summarize",
      "unknown cardinality or relationship kinds are unsupported",
    ],
  },
  flowchart: {
    displayName: "Flowchart / graph",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    limits: [
      "icons, images, callbacks, and links are not terminal output",
      "some uncommon route shapes are approximate",
    ],
  },
  gantt: {
    displayName: "Gantt",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "titles",
      "sections",
      "tasks",
      "dates",
      "tags",
      "deterministic date formatting",
    ],
    limits: [
      "output is a readable task summary, not terminal timeline geometry",
      "dependency source expressions are not disclosed",
    ],
  },
  gitgraph: {
    displayName: "GitGraph",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: ["does not draw a full Git lane graph"],
  },
  journey: {
    displayName: "Journey",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: ["does not draw Mermaid journey chart geometry"],
  },
  kanban: {
    displayName: "Kanban",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: ["drag and board presentation metadata are not terminal output"],
  },
  mindmap: {
    displayName: "Mindmap",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: ["icons, images, and rich browser node shapes are omitted or approximated"],
  },
  packet: {
    displayName: "Packet",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: [
      "output is an ordered row report rather than a spatial bit-width grid",
      "visual styling beyond terminal borders is not represented",
    ],
  },
  sequence: {
    displayName: "Sequence",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    limits: ["actor presentation metadata and links are omitted"],
  },
  state: {
    displayName: "State",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    limits: ["some presentation metadata is omitted"],
  },
  timeline: {
    displayName: "Timeline",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    limits: ["does not draw Mermaid timeline geometry"],
  },
  treeView: {
    displayName: "TreeView",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "hierarchical outline order",
      "directory and file distinction",
      "ASCII and Unicode tree connectors",
      "icons classes and descriptions as text disclosure",
    ],
    limits: [
      "outline output does not claim two-dimensional diagram geometry",
      "browser icons and CSS classes are disclosed rather than styled",
    ],
  },
  xychart: {
    displayName: "XYChart",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    limits: [
      "browser hover tooltips and SVG-coordinate precision are not represented",
      "dense data uses terminal-compact layout",
    ],
  },
};

export const FALLBACK_ASCII_CAPABILITIES: readonly AsciiCapability[] =
  BUILT_IN_TYPED_FAMILIES.map((diagramType) => {
    const capability = FALLBACK_ASCII_CAPABILITY_INPUTS[diagramType];
    const semanticCoverage = capability?.semanticCoverage ?? null;
    const primaryProjection = capability?.primaryProjection ?? "none";
    return {
      diagram_type: diagramType,
      display_name: capability?.displayName ?? diagramType,
      semantic_coverage: semanticCoverage,
      primary_projection: primaryProjection,
      structured_text_fallback: capability?.structuredTextFallback ?? false,
      support_level: deriveSupportLevel(semanticCoverage, primaryProjection),
      supported_semantics: capability?.supportedSemantics ?? [],
      limits: capability?.limits ?? ["no terminal projection is available"],
      evidence: [
        {
          kind: "support_matrix",
          source: "docs/rendering/ASCII_SUPPORT_MATRIX.md",
          note: "playground fallback capability synthesized from tracked support matrix",
        },
      ],
    };
  });

function deriveSupportLevel(
  coverage: AsciiSemanticCoverage,
  projection: AsciiPrimaryProjection
): AsciiSupportLevel {
  if (coverage === null || projection === "none") return "unsupported";
  if (projection === "structured_text") return "summary";
  return coverage;
}

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
    return "asciiSupport.summary";
  }
  return `asciiSupport.levels.${capability.semantic_coverage}`;
}

export function asciiSupportDescription(
  capability: Pick<AsciiCapability, "limits"> | null
): string {
  return capability?.limits?.find((limit) => limit.trim().length > 0) ?? "";
}
