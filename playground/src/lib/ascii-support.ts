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
    supportedSemantics: [
      "class boxes, members, methods, annotations, and notes",
      "independent relationship markers and endpoint labels",
      "four directions and namespace containers",
      "self-relations, routed relation components, and lossless summaries",
    ],
    limits: [
      "cross-namespace or cross-container relationships render as relation summaries",
      "parallel relation lanes whose ports do not fit use a lossless relation summary",
      "dense or collision-prone relation scenes can summarize",
    ],
  },
  er: {
    displayName: "ER",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    structuredTextFallback: true,
    supportedSemantics: [
      "entity boxes, attributes, and key tokens",
      "relationship labels, cardinalities, and identifying relationships",
      "four directions, self-relations, and routed relation components",
      "lossless crossing, port-fit, route, and collision summaries",
    ],
    limits: [
      "complex cyclic topology can summarize",
      "unknown cardinality or relationship kinds are unsupported",
    ],
  },
  flowchart: {
    displayName: "Flowchart / graph",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    supportedSemantics: [
      "root directions and common node shapes",
      "terminal-cell wrapped node labels",
      "edge labels and open, dotted, thick, and invisible edges",
      "subgraphs, nested groups, and boundary-aware routes",
      "terminal color roles",
    ],
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
      "stable task ids",
      "start and end constraint expressions",
      "resolved and adjusted end times",
      "dates",
      "tags",
      "time-of-day precision",
      "deterministic date formatting",
    ],
    limits: [
      "output is a readable task summary, not terminal timeline geometry",
      "links and click callbacks are metadata-only",
      "duplicate or empty task ids are rejected",
    ],
  },
  gitgraph: {
    displayName: "GitGraph",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "commits, branches, merges, tags, and cherry-picks",
      "parent topology and ordering",
      "explicit merge id and type overrides",
    ],
    limits: [
      "does not draw a full Git lane graph",
      "implementation flags are normalized into semantic labels",
    ],
  },
  journey: {
    displayName: "Journey",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: ["sections", "tasks", "actors", "scores"],
    limits: ["does not draw Mermaid journey chart geometry"],
  },
  kanban: {
    displayName: "Kanban",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "columns and cards",
      "stable card and group ids",
      "assignments and metadata",
      "deterministic Unassigned grouping",
      "group parent ownership disclosure",
    ],
    limits: [
      "drag and board presentation metadata are not terminal output",
      "group parent ownership is disclosed without nested board geometry",
      "duplicate or empty ids are rejected",
    ],
  },
  mindmap: {
    displayName: "Mindmap",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "hierarchical nodes, stable ids, labels, and nesting",
      "wrapped text and shape, icon, and section disclosure",
      "disconnected components, cycles, and validated edge endpoints",
    ],
    limits: [
      "icons and rich browser node shapes are disclosed as text rather than styled",
      "duplicate ids, parallel edges, and missing endpoints are rejected",
    ],
  },
  packet: {
    displayName: "Packet",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "bit ranges",
      "labels",
      "row splitting",
      "multi-row packets",
    ],
    limits: [
      "output is an ordered row report rather than a spatial bit-width grid",
      "visual styling beyond terminal borders is not represented",
    ],
  },
  sequence: {
    displayName: "Sequence",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    supportedSemantics: [
      "participants with Mermaid-valid spaced and Unicode identifiers",
      "typed headless, filled, cross, point, bidirectional, and half-arrow messages",
      "central decorations, notes, lifecycles, and boxes",
      "participant-bounded nested control frames",
      "optional mirrored actors and terminal color roles",
    ],
    limits: [
      "actor presentation metadata and links are accepted but intentionally omitted",
    ],
  },
  state: {
    displayName: "State",
    semanticCoverage: "partial",
    primaryProjection: "diagrammatic",
    supportedSemantics: [
      "states, start and end nodes, and transitions",
      "independently anchored notes",
      "choice, fork, join-like nodes, and composite groups",
      "terminal color roles",
    ],
    limits: [
      "some presentation metadata is omitted",
      "future state shape variants need explicit support rules",
    ],
  },
  timeline: {
    displayName: "Timeline",
    semanticCoverage: "partial",
    primaryProjection: "structured_text",
    supportedSemantics: [
      "sections",
      "events",
      "direction",
      "ordered grouped text",
    ],
    limits: [
      "does not draw Mermaid timeline geometry",
      "parser bookkeeping score is intentionally omitted",
    ],
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
    structuredTextFallback: true,
    supportedSemantics: [
      "model-owned x/y coordinates and point labels",
      "band and linear axes with compact scale-aware ticks",
      "grouped bars, topology-resolved lines, and mixed plots",
      "horizontal and vertical orientation, titles, legends, and exact disclosure",
      "length-framed empty-chart metadata disclosure",
    ],
    limits: [
      "browser hover tooltips are replaced by deterministic terminal disclosure",
      "typed chart coordinates are independently quantized by the terminal plan",
      "cross-series same-cell collisions use deterministic paint order plus exact disclosure",
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
    return "asciiSupport.structuredText";
  }
  return `asciiSupport.levels.${capability.semantic_coverage}`;
}

export function asciiSupportDescription(
  capability: Pick<AsciiCapability, "limits"> | null
): string {
  return capability?.limits?.find((limit) => limit.trim().length > 0) ?? "";
}
