use crate::options::{AsciiGraphPresentation, AsciiLayoutProfile, TerminalWidthProfile};
use crate::output::AsciiOutputEncoding;
use std::sync::OnceLock;

const CANONICAL_LAYOUT_PROFILES: &[AsciiLayoutProfile] = &[AsciiLayoutProfile::Canonical];
const STRICT_GRAPH_PRESENTATIONS: &[AsciiGraphPresentation] = &[AsciiGraphPresentation::Strict];
const CORE_GRAPH_PRESENTATIONS: &[AsciiGraphPresentation] = &[
    AsciiGraphPresentation::Strict,
    AsciiGraphPresentation::Adaptive,
];
const FLOWCHART_SEQUENCE_LAYOUT_PROFILES: &[AsciiLayoutProfile] = &[
    AsciiLayoutProfile::Canonical,
    AsciiLayoutProfile::Compact,
    AsciiLayoutProfile::Auto,
];
const TERMINAL_WIDTH_PROFILES: &[TerminalWidthProfile] =
    &[TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk];
const OUTPUT_ENCODINGS: &[AsciiOutputEncoding] = &[
    AsciiOutputEncoding::Plain,
    AsciiOutputEncoding::Ansi16,
    AsciiOutputEncoding::Ansi256,
    AsciiOutputEncoding::TrueColor,
    AsciiOutputEncoding::Html,
];
const PLAIN_FALLBACK_ENCODINGS: &[AsciiOutputEncoding] = &[AsciiOutputEncoding::Plain];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsciiSupportLevel {
    Full,
    Partial,
    Summary,
    Unsupported,
}

impl AsciiSupportLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Summary => "summary",
            Self::Unsupported => "unsupported",
        }
    }

    pub const fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsciiSemanticCoverage {
    Full,
    Partial,
}

impl AsciiSemanticCoverage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsciiPrimaryProjection {
    Diagrammatic,
    StructuredText,
    None,
}

impl AsciiPrimaryProjection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diagrammatic => "diagrammatic",
            Self::StructuredText => "structured_text",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsciiEvidenceKind {
    MermaidAsciiOracle,
    BeautifulMermaidPriorArt,
    LocalSemanticProbe,
    LocalAdvantage,
    SupportMatrix,
    GapRegistry,
}

impl AsciiEvidenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MermaidAsciiOracle => "mermaid_ascii_oracle",
            Self::BeautifulMermaidPriorArt => "beautiful_mermaid_prior_art",
            Self::LocalSemanticProbe => "local_semantic_probe",
            Self::LocalAdvantage => "local_advantage",
            Self::SupportMatrix => "support_matrix",
            Self::GapRegistry => "gap_registry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiCapabilityEvidence {
    pub kind: AsciiEvidenceKind,
    pub source: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiCapability {
    pub diagram_type: &'static str,
    pub display_name: &'static str,
    pub semantic_coverage: Option<AsciiSemanticCoverage>,
    pub primary_projection: AsciiPrimaryProjection,
    pub structured_text_fallback: bool,
    /// Layout profiles admitted for this diagram family.
    pub layout_profiles: &'static [AsciiLayoutProfile],
    /// Graph presentation policies admitted for this diagram family.
    pub graph_presentations: &'static [AsciiGraphPresentation],
    /// Terminal display-width conventions admitted for this diagram family.
    pub width_profiles: &'static [TerminalWidthProfile],
    /// Encoded output representations admitted for primary output.
    pub encodings: &'static [AsciiOutputEncoding],
    /// Encoded representations that can carry a complete overflow fallback.
    pub fallback_encodings: &'static [AsciiOutputEncoding],
    /// Compatibility view derived from semantic coverage and the primary projection.
    pub support_level: AsciiSupportLevel,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: &'static [AsciiCapabilityEvidence],
}

impl AsciiCapability {
    fn from_definition(definition: AsciiCapabilityDefinition) -> Self {
        let support_level =
            derive_support_level(definition.semantic_coverage, definition.primary_projection);
        let supported = definition.semantic_coverage.is_some();
        let layout_profiles = if !supported {
            &[]
        } else {
            match definition.diagram_type {
                "flowchart" | "sequence" => FLOWCHART_SEQUENCE_LAYOUT_PROFILES,
                _ => CANONICAL_LAYOUT_PROFILES,
            }
        };
        let width_profiles = if supported {
            TERMINAL_WIDTH_PROFILES
        } else {
            &[]
        };
        let graph_presentations = match definition.diagram_type {
            _ if !supported => &[],
            "agentflow" | "flowchart" | "requirement" | "swimlane" | "state" if supported => {
                CORE_GRAPH_PRESENTATIONS
            }
            _ => STRICT_GRAPH_PRESENTATIONS,
        };
        let encodings = if supported { OUTPUT_ENCODINGS } else { &[] };
        let fallback_encodings = if supported && definition.structured_text_fallback {
            PLAIN_FALLBACK_ENCODINGS
        } else {
            &[]
        };
        Self {
            diagram_type: definition.diagram_type,
            display_name: definition.display_name,
            semantic_coverage: definition.semantic_coverage,
            primary_projection: definition.primary_projection,
            structured_text_fallback: definition.structured_text_fallback,
            layout_profiles,
            graph_presentations,
            width_profiles,
            encodings,
            fallback_encodings,
            support_level,
            supported_semantics: definition.supported_semantics,
            limits: definition.limits,
            evidence: definition.evidence,
        }
    }

    fn unsupported(diagram_type: &'static str) -> Self {
        Self::from_definition(AsciiCapabilityDefinition {
            diagram_type,
            display_name: diagram_type,
            semantic_coverage: None,
            primary_projection: AsciiPrimaryProjection::None,
            structured_text_fallback: false,
            supported_semantics: &[],
            limits: &["no terminal projection is available"],
            evidence: &[AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#unsupported-families",
                note: "the total capability matrix records this typed family as unsupported",
            }],
        })
    }

    pub const fn derived_support_level(self) -> AsciiSupportLevel {
        derive_support_level(self.semantic_coverage, self.primary_projection)
    }

    pub const fn is_supported(self) -> bool {
        self.semantic_coverage.is_some()
    }

    #[must_use]
    pub fn supports_layout_profile(self, profile: AsciiLayoutProfile) -> bool {
        self.layout_profiles.contains(&profile)
    }

    #[must_use]
    pub fn supports_graph_presentation(self, presentation: AsciiGraphPresentation) -> bool {
        self.graph_presentations.contains(&presentation)
    }

    #[must_use]
    pub fn supports_width_profile(self, profile: TerminalWidthProfile) -> bool {
        self.width_profiles.contains(&profile)
    }

    #[must_use]
    pub fn supports_encoding(self, encoding: AsciiOutputEncoding) -> bool {
        self.encodings.contains(&encoding)
    }

    #[must_use]
    pub fn supports_fallback_encoding(self, encoding: AsciiOutputEncoding) -> bool {
        self.fallback_encodings.contains(&encoding)
    }

    /// Returns whether the independent primary-output axes form an admitted request.
    #[must_use]
    pub fn supports_output(
        self,
        layout_profile: AsciiLayoutProfile,
        width_profile: TerminalWidthProfile,
        encoding: AsciiOutputEncoding,
    ) -> bool {
        self.supports_layout_profile(layout_profile)
            && self.supports_width_profile(width_profile)
            && self.supports_encoding(encoding)
    }
}

const fn derive_support_level(
    semantic_coverage: Option<AsciiSemanticCoverage>,
    primary_projection: AsciiPrimaryProjection,
) -> AsciiSupportLevel {
    match (semantic_coverage, primary_projection) {
        (Some(_), AsciiPrimaryProjection::StructuredText) => AsciiSupportLevel::Summary,
        (Some(AsciiSemanticCoverage::Full), AsciiPrimaryProjection::Diagrammatic) => {
            AsciiSupportLevel::Full
        }
        (Some(AsciiSemanticCoverage::Partial), AsciiPrimaryProjection::Diagrammatic) => {
            AsciiSupportLevel::Partial
        }
        _ => AsciiSupportLevel::Unsupported,
    }
}

#[derive(Debug, Clone, Copy)]
struct AsciiCapabilityDefinition {
    diagram_type: &'static str,
    display_name: &'static str,
    semantic_coverage: Option<AsciiSemanticCoverage>,
    primary_projection: AsciiPrimaryProjection,
    structured_text_fallback: bool,
    supported_semantics: &'static [&'static str],
    limits: &'static [&'static str],
    evidence: &'static [AsciiCapabilityEvidence],
}

const ASCII_CAPABILITY_DEFINITIONS: &[AsciiCapabilityDefinition] = &[
    AsciiCapabilityDefinition {
        diagram_type: "agentflow",
        display_name: "Agentflow",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "distinct task, tool, input, reference-document, action, decision, and connector roles",
            "sequence arrows, headless dotted references, and failure cross markers",
            "expanded flow containers, nesting, local directions, and global node ownership",
            "complete keyed original entities and relationships for rich or collapsed reading records",
            "metadata types, authored styles, diagnostics, source configuration, and quoted identities in records",
        ],
        limits: &[
            "semantic metadata, collapsed views, unsupported shapes, and ambiguous containment use StructuredText primaries",
            "collapsed reading records disclose authored children and relationships rather than claiming browser-visible geometry",
            "terminal shapes approximate browser geometry and do not execute tools or resolve connectorRef metadata",
            "dense spatial routes remain bounded; Adaptive can select complete Plain records",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/agentflow_model.rs",
                note: "parser-backed roles, operators, scopes, metadata, viewport, resource and cancellation coverage",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ascii/AGENTFLOW_SUPPORT.md",
                note: "family-owned spatial admission and complete keyed reading records",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "block",
        display_name: "Block",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "family-owned terminal columns and authored column spans",
            "ordered rows and explicit spacer cells",
            "nested composite block frames",
            "independent source and target markers, strokes, and edge labels",
            "complete keyed hierarchy and relationship records for width fallback",
        ],
        limits: &[
            "terminal geometry approximates browser shapes",
            "non-spatial numeric columns and spans require explicit diagnostics",
            "bounded routing may reject dense or colliding fixed-grid relationships",
            "styling and accessibility metadata are retained by fallback rather than browser painting",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/block_model.rs",
                note: "parser-backed terminal column, span, nesting, edge, and resource contracts",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ascii/BLOCK_SUPPORT.md",
                note: "family-owned fixed-grid support and diagnostic boundaries",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "class",
        display_name: "Class",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "class boxes",
            "members and methods",
            "annotations and notes",
            "common relationship markers",
            "independent source and target relationship markers",
            "endpoint labels",
            "top-down, bottom-up, left-right, and right-left directions",
            "namespace containers",
            "namespace-qualified endpoint aliases",
            "namespace-internal class and note relationship routing",
            "readable sibling namespace, namespace-to-root, and nested namespace facade routing",
            "length-framed namespace facade member identity",
            "self-relation loops",
            "bounded iterative relation-layer sweeps",
            "directed component ranks and private channels for bounded cycles and DAG diamonds",
            "routed relation lanes",
            "independent relation components",
            "lossless crossing, port-fit, route, and overlay collision summaries",
        ],
        limits: &[
            "dense or colliding cross-namespace relationships render as lossless relation summaries",
            "parallel relationship lanes whose ports do not fit render as lossless relation summaries",
            "dense or collision-prone relation scenes can summarize",
            "bounded private-channel search does not guarantee a route for every topology",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "class compartments, annotations, and relationship coverage are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/class/",
                note: "local fixtures assert typed class semantics instead of copied reference spacing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/class_model.rs",
                note: "class_directed_cycles_share_topology_across_directions_and_declaration_order and class_diamond_uses_directed_ranks_in_every_direction cover directed rings and DAG ranks",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/src/relation_graph/tests/",
                note: "shared topology and private-channel tests retain declaration-order stability, disjoint routes, exact/N-1 work admission, and fallback ledger ownership",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "docs/rendering/ascii/ASCII_GAP_REGISTRY.md#a-classer-010",
                note: "shared relation_graph gap owner records routed and summary boundaries",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "er",
        display_name: "ER",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "entity boxes",
            "attributes, key tokens, and attribute comments",
            "relationship labels",
            "cardinality markers",
            "parent diamond cardinality markers",
            "nested ER group frames and typed group endpoints",
            "actual member identity disclosure for group facade routes",
            "identifying relationships",
            "top-down, bottom-up, left-right, and right-left directions",
            "self-relationship loops",
            "bounded iterative relation-layer sweeps",
            "directed component ranks and private channels for bounded cycles and DAG diamonds",
            "routed relation lanes",
            "independent relation components",
            "lossless crossing, port-fit, route, and overlay collision summaries",
        ],
        limits: &[
            "parallel relationship lanes whose ports do not fit render as lossless relation summaries",
            "complex cyclic or collision-prone topology can summarize",
            "bounded private-channel search does not guarantee a route for every topology",
            "unknown cardinality markers are unsupported",
            "unknown relationship identity kinds are unsupported",
            "ancestor/member relationships use typed summaries alongside group geometry",
            "duplicate groups, ambiguous ownership, and shadowed forward-reference endpoints are unsupported",
            "accessibility, Mermaid diagram source comments, and styling metadata are intentionally omitted from terminal output",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "ER attributes, relationships, and cardinalities are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/er/",
                note: "local fixtures assert entity, attribute, cardinality, and summary semantics",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/er_model.rs",
                note: "er_directed_cycles_share_topology_across_directions_and_declaration_order and er_diamond_uses_directed_ranks_and_private_relationship_labels cover directed rings and DAG ranks",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/src/relation_graph/tests/",
                note: "shared topology and private-channel tests retain declaration-order stability, disjoint routes, exact/N-1 work admission, and fallback ledger ownership",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "docs/rendering/ascii/ASCII_GAP_REGISTRY.md#a-classer-010",
                note: "shared relation_graph gap owner records routed and summary boundaries",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "flowchart",
        display_name: "Flowchart / graph",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "root directions",
            "boxed nodes and common shapes",
            "explicit type compartments for admitted uncommon shapes",
            "icon and image metadata as complete terminal text",
            "terminal-cell wrapped node labels",
            "edge labels",
            "open dotted and thick edges",
            "subgraphs and nested groups",
            "boundary-aware routes",
            "terminal color roles",
        ],
        limits: &[
            "icon glyphs and image pixels are not rendered; asset metadata is disclosed as text",
            "fixed control shapes reject asset metadata rather than hiding it",
            "callbacks and links are not terminal output",
            "some uncommon route shapes are approximate",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::MermaidAsciiOracle,
                source: "crates/merman-ascii/tests/testdata/mermaid-ascii/",
                note: "copied graph fixtures preserve an exact subset plus named renderable semantic differences",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "graph ASCII shape and disconnected-layout tests are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#intentional-differences",
                note: "true RL/BT handling is a local semantic target, not a beautiful-mermaid capability",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/flowchart/issue_53_long_node_labels.mmd",
                note: "Issue #53 verifies terminal-cell wrapping before node sizing while preserving labels and topology",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "gantt",
        display_name: "Gantt",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "titles",
            "sections",
            "tasks",
            "stable task ids",
            "typed start and end constraints with dependency ids",
            "resolved and adjusted end times",
            "dates",
            "tags",
            "time-of-day precision",
            "deterministic date formatting",
            "length-framed authored report fields",
        ],
        limits: &[
            "no terminal timeline geometry",
            "output is a readable task summary",
            "links and click callbacks are metadata-only",
            "duplicate or empty task ids are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve task identity and typed scheduling constraints with injective authored-field framing without claiming pseudo-graph geometry",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Gantt as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "gitgraph",
        display_name: "GitGraph",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "commits",
            "branches",
            "merges",
            "tags",
            "cherry-picks",
            "parent topology",
            "explicit merge id and type overrides",
            "ordering",
        ],
        limits: &[
            "does not draw a full Git lane graph",
            "terminal output normalizes implementation flags into semantic labels",
            "unknown direct-model commit types are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve graph history facts in terminal text",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies GitGraph as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "journey",
        display_name: "Journey",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "sections",
            "tasks",
            "actors",
            "scores",
            "length-framed authored report fields",
        ],
        limits: &["does not draw Mermaid journey chart geometry"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve actor and score data in stable, injectively framed rows",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Journey as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "kanban",
        display_name: "Kanban",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "columns",
            "cards",
            "stable card and group ids",
            "independent repeated card occurrences",
            "assignments",
            "metadata",
            "deterministic Unassigned grouping",
            "group parent ownership disclosure",
            "length-framed authored report fields",
        ],
        limits: &[
            "drag and board presentation metadata are not terminal output",
            "group parent ownership is disclosed without nested board geometry",
            "duplicate group ids and empty ids are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve column-first card order and metadata with injective authored-field framing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Kanban as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "mindmap",
        display_name: "Mindmap",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "hierarchical nodes",
            "stable authored node ids",
            "labels",
            "nesting",
            "wrapped text",
            "shape, icon, and section disclosure",
            "disconnected components and cycles",
            "validated edge endpoints",
            "ASCII and Unicode tree connectors",
            "length-framed authored report fields",
        ],
        limits: &[
            "icons and rich browser node shapes are disclosed as text rather than styled",
            "duplicate internal or authored ids, missing authored ids, parallel edges, and missing endpoints are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve hierarchy with charset-aware connectors and injective authored-field framing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Mindmap as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "packet",
        display_name: "Packet",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "fixed two-terminal-cell bit grid",
            "exact bit positions, gaps, and cross-row field splitting",
            "ascending and descending bit order",
            "optional bit-number display",
            "source packet.bitsPerRow, packet.bitOrder, and packet.showBits configuration",
            "length-framed authored labels and raw start/end/bits records after the plot",
        ],
        limits: &[
            "spatial admission requires integer packet.bitsPerRow in 1..=64; this is a terminal bound, not a Mermaid syntax limit",
            "borrowed render_model defaults to 32 bits per row, ascending order, and visible bit numbers",
            "row widths outside the spatial domain, coordinates outside exact nonnegative integers, fractional/non-finite/negative DB bits, overlaps, or out-of-order blocks retain complete structured records and report StructuredText",
            "narrow field labels may use '*' with an explicit legend and complete detail records; labels never widen fields",
            "stored DB bits can differ from inclusive range width after upstream row splitting and are preserved without correction",
            "visual styling beyond terminal borders is not represented",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/packet_grid.rs",
                note: "owns bounded spatial admission, bit positions, configuration, exact records, viewport, encoding, and resource checks",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model compatibility tests retain packet ranges, DB bit counts, and injective labels",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#packet-bit-grid",
                note: "support matrix bounds Packet's partial spatial projection and complete record-only cases",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "pie",
        display_name: "Pie",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "family-owned percent bars on one shared 0..100% scale with 50 terminal cells",
            "all categories in source order with stable P1, P2, and subsequent reading keys",
            "complete keyed labels and exact numeric values, including non-finite direct-model values",
            "showData controls raw values beside bars; exact values always remain in records",
            "title, accessibility metadata, source title, and source configuration in records",
            "upstream first-duplicate-label wins for parsed input; direct-model duplicates retain separate keys",
            "complete keyed reading records for non-spatial data and narrow width fallback",
        ],
        limits: &[
            "percent bars approximate Pie proportions rather than browser slice geometry",
            "diagrammatic admission requires nonempty finite nonnegative values with a positive total",
            "empty, all-zero, non-finite, or negative data use complete StructuredText primaries with an explicit reason",
            "positive values quantized below one bar cell are explicitly marked; no category is filtered",
            "narrow fallback reflows complete keyed records without truncation and requires Plain encoding",
            "only Canonical layout and Strict graph presentation are admitted",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/pie_model.rs",
                note: "parser-backed percent bars, exact records, direct-model admission, viewports, encodings, and resource coverage",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ascii/PIE_SUPPORT.md",
                note: "family-owned percent-bar admission and complete keyed Pie reading records",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "requirement",
        display_name: "Requirement",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "one shared endpoint graph with R requirement and N element reading keys",
            "all six requirement types with id, text, risk, and verifyMethod records",
            "element type and docRef records",
            "full contains, copies, derives, satisfies, verifies, refines, and traces labels",
            "solid contains relationships with a source circle and no target arrow",
            "dotted non-contains relationships with a target arrow",
            "top-down, bottom-up, left-right, and right-left directions",
            "complete keyed declarations and source/target relationship records following topology",
            "classes, styles, accessibility, source title, and source configuration in records",
        ],
        limits: &[
            "ambiguous or duplicate names, unresolved endpoints, and unknown relation kinds or directions use StructuredText primaries",
            "terminal geometry approximates browser nodes and does not paint CSS or resolve document references",
            "dense spatial planning failures remain diagnostics; Adaptive can select complete Plain records",
            "reading primaries use a natural width of 80; narrow fallback reflows complete keyed records without truncation",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/requirement_model.rs",
                note: "parser-backed shared endpoints, typed relations, complete fields, record primaries, viewports, and resource coverage",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ascii/REQUIREMENT_SUPPORT.md",
                note: "family-owned spatial admission and complete keyed Requirement reading records",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "sequence",
        display_name: "Sequence",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "participants",
            "Mermaid-valid spaced and Unicode participant identifiers",
            "headless, filled, cross, point, bidirectional, and half-arrow messages",
            "central endpoint decorations",
            "notes",
            "lifecycles",
            "actor boxes",
            "participant-bounded nested control frames",
            "diagram-wide empty boxes",
            "sequence box inner padding",
            "all-participant boxes around dynamic lifecycle content",
            "optional mirrored actors",
            "terminal color roles",
        ],
        limits: &["actor presentation metadata and links are accepted but intentionally omitted"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::MermaidAsciiOracle,
                source: "crates/merman-ascii/tests/testdata/mermaid-ascii/sequence/",
                note: "copied sequence fixtures keep the admitted byte-level oracle stable",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "sequence parser and block-layout cases are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/sequence/",
                note: "local fixtures assert message, frame, and note semantics",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "state",
        display_name: "State",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "states",
            "start and end nodes",
            "transitions",
            "notes",
            "choice fork and join-like nodes",
            "composite groups",
            "terminal color roles",
        ],
        limits: &[
            "some presentation metadata is omitted",
            "future state shape variants need explicit support rules",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "state-oriented ASCII shape ideas are prior art, not a byte oracle",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/state/",
                note: "local fixtures assert composite and wide-label state behavior",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "docs/rendering/ascii/ASCII_GAP_REGISTRY.md#a-state-010",
                note: "remaining state presentation metadata is explicitly tracked",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "timeline",
        display_name: "Timeline",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "sections",
            "events",
            "direction",
            "ordered grouped text",
            "length-framed authored report fields",
        ],
        limits: &[
            "does not draw Mermaid timeline geometry",
            "parser bookkeeping score is intentionally omitted",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests keep section and event order stable with injective authored-field framing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Timeline as structured-text output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "treeView",
        display_name: "TreeView",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: true,
        supported_semantics: &[
            "hierarchical outline order",
            "root and node identities with authored levels",
            "directory and file distinction",
            "ASCII and Unicode tree connectors",
            "icons classes and descriptions as text disclosure",
            "length-framed authored report fields",
        ],
        limits: &[
            "outline output does not claim two-dimensional diagram geometry",
            "browser icons and CSS classes are disclosed rather than styled",
            "duplicate node ids are rejected",
            "unknown node types are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve hierarchy and annotations with injective authored-field framing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies TreeView as structured outline output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "usecase",
        display_name: "Usecase",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "distinct actor, use-case, boundary, package, and note roles",
            "association, include, extend, and generalization relationship identity",
            "independent endpoint markers and dotted relationships",
            "top-down, bottom-up, left-right, and right-left directions",
            "boundary membership and note attachment",
            "ordered JSON tables with typed paths, literal values, and non-finite number metadata",
            "complete keyed entity and relationship records for width fallback",
        ],
        limits: &[
            "terminal role presentation approximates browser actor and ellipse geometry",
            "JSON tables retain indexed array paths and quoted strings rather than browser cell geometry",
            "bounded routing may reject dense or colliding compound relationships",
            "styling, animation, and accessibility metadata are retained by fallback rather than browser painting",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/usecase_model.rs",
                note: "parser-backed family roles, relationship identity, scopes, viewports, and resources",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ascii/USECASE_SUPPORT.md",
                note: "family-owned role projection and bounded route admission",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "xychart",
        display_name: "XYChart",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: true,
        supported_semantics: &[
            "model-owned x/y sample coordinates and point labels",
            "parser-produced x coordinates derived from typed axes, categories, and sample order",
            "band and linear axes with compact scale-aware ticks",
            "grouped bar, topology-resolved line, and mixed plots",
            "horizontal and vertical orientation",
            "titles and axes",
            "legends",
            "length-framed exact data labels and semantic disclosure",
            "record-oriented disclosure after the plot",
            "joint line topology with distinct series markers and shared sample cells",
            "length-framed empty-chart metadata disclosure",
            "configurable plot dimensions",
        ],
        limits: &[
            "browser hover tooltips are replaced by deterministic terminal disclosure",
            "typed chart coordinates are independently quantized by the terminal plan",
            "quantized shared line samples use X plus exact disclosure; line markers repeat after six series",
            "line paths overlay bars; quantized bar and line overlap still requires exact disclosure",
            "unknown direct-model orientations and band y-axes are rejected",
            "accessibility title and description metadata are intentionally omitted from terminal output",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "XYChart ASCII and legend behavior are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/xychart_model.rs",
                note: "semantic tests assert model-owned coordinates, parser-derived x positions, grouped lanes, missing-sample gaps, connected horizontal paths, precision, injective disclosure, empty-chart reports, clipping, collisions, direct-model validation, labels, and resource extents",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "docs/rendering/ascii/ASCII_GAP_REGISTRY.md#a-xy-010",
                note: "shared line topology retains sample ownership; terminal quantization, dense bars, and marker reuse remain explicit compact-layout limits",
            },
        ],
    },
];

/// Reports local terminal adapters for the selected canonical model families.
///
/// Shared model types do not merge language availability: Flowchart and Swimlane
/// have separate rows even though they use the same terminal projection.
pub fn ascii_capabilities() -> &'static [AsciiCapability] {
    static CAPABILITIES: OnceLock<Vec<AsciiCapability>> = OnceLock::new();
    CAPABILITIES
        .get_or_init(|| {
            let mut capabilities = Vec::new();
            for family in merman_model::family::model_family_capabilities() {
                let diagram_type = capability_diagram_type(
                    merman_model::family::diagram_type_metadata_id(family.logical_family_kind)
                        .unwrap_or(family.logical_family_kind),
                );
                if !family.has_model
                    || diagram_type == "error"
                    || capabilities
                        .iter()
                        .any(|entry: &AsciiCapability| entry.diagram_type == diagram_type)
                {
                    continue;
                }
                let capability = ASCII_CAPABILITY_DEFINITIONS
                    .iter()
                    .find(|definition| {
                        family
                            .render_model_kind
                            .and_then(merman_model::family::diagram_type_metadata_id)
                            .map(capability_diagram_type)
                            == Some(definition.diagram_type)
                    })
                    .filter(|_| local_handler_available(family.logical_family_kind))
                    .copied()
                    .map(|definition| {
                        let mut capability = AsciiCapability::from_definition(definition);
                        capability.diagram_type = diagram_type;
                        if diagram_type == "swimlane" {
                            capability.display_name = "Swimlane";
                        }
                        capability
                    })
                    .unwrap_or_else(|| AsciiCapability::unsupported(diagram_type));
                capabilities.push(capability);
            }
            capabilities.sort_by_key(|capability| capability.diagram_type);
            capabilities
        })
        .as_slice()
}

pub(crate) fn capability_for_diagram_type(diagram_type: &str) -> Option<AsciiCapability> {
    let metadata_id =
        merman_model::family::diagram_type_metadata_id(diagram_type).unwrap_or(diagram_type);
    let metadata_id = if metadata_id == "gitGraph" {
        "gitgraph"
    } else {
        metadata_id
    };
    ascii_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == metadata_id)
        .copied()
}

fn capability_diagram_type(logical_family: &'static str) -> &'static str {
    match logical_family {
        "gitGraph" | "GitGraph" | "gitgraph" => "gitgraph",
        other => other,
    }
}

#[allow(
    clippy::match_like_matches_macro,
    reason = "Each arm has a distinct feature condition that only coincides in all-family builds."
)]
pub(crate) fn local_handler_available(diagram_type: &str) -> bool {
    let family = match diagram_type {
        "gitgraph" => "gitGraph",
        other => merman_model::family::diagram_type_family_kind(other).unwrap_or(other),
    };
    match family {
        "agentflow" => cfg!(feature = "diagram-agentflow"),
        "block" => cfg!(feature = "diagram-block"),
        "class" => cfg!(feature = "diagram-class"),
        "er" => cfg!(feature = "diagram-er"),
        "flowchart" => cfg!(feature = "diagram-flowchart"),
        "gantt" => cfg!(feature = "diagram-gantt"),
        "gitGraph" => cfg!(feature = "diagram-git-graph"),
        "journey" => cfg!(feature = "diagram-journey"),
        "kanban" => cfg!(feature = "diagram-kanban"),
        "mindmap" => cfg!(feature = "diagram-mindmap"),
        "packet" => cfg!(feature = "diagram-packet"),
        "pie" => cfg!(feature = "diagram-pie"),
        "requirement" => cfg!(feature = "diagram-requirement"),
        "sequence" => cfg!(feature = "diagram-sequence"),
        "state" => cfg!(feature = "diagram-state"),
        "swimlane" => cfg!(feature = "diagram-swimlane"),
        "timeline" => cfg!(feature = "diagram-timeline"),
        "treeView" => cfg!(feature = "diagram-tree-view"),
        "usecase" => cfg!(feature = "diagram-usecase"),
        "xychart" => cfg!(feature = "diagram-xychart"),
        _ => false,
    }
}

pub fn ascii_supported_diagram_types() -> &'static [&'static str] {
    static SUPPORTED: OnceLock<Vec<&'static str>> = OnceLock::new();
    SUPPORTED
        .get_or_init(|| {
            ascii_capabilities()
                .iter()
                .filter(|capability| capability.is_supported())
                .map(|capability| capability.diagram_type)
                .collect()
        })
        .as_slice()
}

pub fn ascii_diagrammatic_diagram_types() -> &'static [&'static str] {
    static DIAGRAMMATIC: OnceLock<Vec<&'static str>> = OnceLock::new();
    DIAGRAMMATIC
        .get_or_init(|| {
            ascii_capabilities()
                .iter()
                .filter(|capability| {
                    matches!(
                        capability.primary_projection,
                        AsciiPrimaryProjection::Diagrammatic
                    )
                })
                .map(|capability| capability.diagram_type)
                .collect()
        })
        .as_slice()
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::Path};

    const ALLOWED_EVIDENCE_ANCHORS: &[(&str, &str, &str)] = &[
        (
            "docs/rendering/ascii/ASCII_GAP_REGISTRY.md",
            "a-classer-010",
            r#"id="a-classer-010""#,
        ),
        (
            "docs/rendering/ascii/ASCII_GAP_REGISTRY.md",
            "a-state-010",
            r#"id="a-state-010""#,
        ),
        (
            "docs/rendering/ascii/ASCII_GAP_REGISTRY.md",
            "a-xy-010",
            r#"id="a-xy-010""#,
        ),
        (
            "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md",
            "family-comparison",
            "## Family Comparison",
        ),
        (
            "docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md",
            "intentional-differences",
            "## Intentional Differences",
        ),
        (
            "docs/rendering/ASCII_SUPPORT_MATRIX.md",
            "packet-bit-grid",
            "### Packet bit grid",
        ),
        (
            "docs/rendering/ASCII_SUPPORT_MATRIX.md",
            "structured-text-outputs",
            "## Structured-Text Outputs",
        ),
        (
            "docs/rendering/ASCII_SUPPORT_MATRIX.md",
            "unsupported-families",
            "## Unsupported Families",
        ),
    ];

    #[test]
    fn capabilities_cover_each_concrete_built_in_typed_family_once() {
        let capabilities = ascii_capabilities();
        let model_families = merman_model::family::model_family_capabilities();
        let capability_types = capabilities
            .iter()
            .map(|capability| capability.diagram_type)
            .collect::<BTreeSet<_>>();
        let model_types = model_families
            .iter()
            .filter(|family| family.has_model && family.logical_family_kind != "error")
            .map(|family| {
                merman_model::family::diagram_type_metadata_id(family.logical_family_kind).unwrap()
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(capabilities.len(), 34);
        assert_eq!(capability_types.len(), capabilities.len());
        assert_eq!(capability_types, model_types);
        assert!(!capability_types.contains("error"));
        assert!(!capability_types.contains("custom-json"));
    }

    #[test]
    fn output_available_and_diagrammatic_lists_are_distinct_projections() {
        assert_eq!(
            ascii_supported_diagram_types(),
            &[
                "agentflow",
                "block",
                "class",
                "er",
                "flowchart",
                "gantt",
                "gitgraph",
                "journey",
                "kanban",
                "mindmap",
                "packet",
                "pie",
                "requirement",
                "sequence",
                "state",
                "swimlane",
                "timeline",
                "treeView",
                "usecase",
                "xychart",
            ]
        );
        assert_eq!(
            ascii_diagrammatic_diagram_types(),
            &[
                "agentflow",
                "block",
                "class",
                "er",
                "flowchart",
                "packet",
                "pie",
                "requirement",
                "sequence",
                "state",
                "swimlane",
                "usecase",
                "xychart",
            ]
        );
    }

    #[test]
    fn capability_axes_admit_only_proven_layout_and_fallback_combinations() {
        for diagram_type in ["flowchart", "sequence"] {
            let capability = find(diagram_type);
            assert_eq!(
                capability.layout_profiles,
                &[
                    AsciiLayoutProfile::Canonical,
                    AsciiLayoutProfile::Compact,
                    AsciiLayoutProfile::Auto
                ]
            );
            assert_eq!(capability.width_profiles, TERMINAL_WIDTH_PROFILES);
            assert_eq!(capability.encodings, OUTPUT_ENCODINGS);
            assert_eq!(capability.fallback_encodings, &[AsciiOutputEncoding::Plain]);
            assert!(capability.supports_output(
                AsciiLayoutProfile::Compact,
                TerminalWidthProfile::Cjk,
                AsciiOutputEncoding::Ansi16,
            ));
            assert!(!capability.supports_fallback_encoding(AsciiOutputEncoding::Ansi16));
        }

        for diagram_type in ["state", "packet", "pie", "block", "usecase", "requirement"] {
            let capability = find(diagram_type);
            assert_eq!(capability.layout_profiles, &[AsciiLayoutProfile::Canonical]);
            assert!(!capability.supports_layout_profile(AsciiLayoutProfile::Compact));
            assert!(!capability.supports_layout_profile(AsciiLayoutProfile::Auto));
            assert_eq!(capability.width_profiles, TERMINAL_WIDTH_PROFILES);
            assert_eq!(capability.encodings, OUTPUT_ENCODINGS);
            assert_eq!(capability.fallback_encodings, &[AsciiOutputEncoding::Plain]);
        }

        let unsupported = find("zenuml");
        assert!(unsupported.layout_profiles.is_empty());
        assert!(unsupported.width_profiles.is_empty());
        assert!(unsupported.encodings.is_empty());
        assert!(unsupported.fallback_encodings.is_empty());
        assert!(unsupported.graph_presentations.is_empty());
    }

    #[test]
    fn adaptive_graph_admission_is_limited_to_admitted_families() {
        for capability in ascii_capabilities() {
            if !capability.is_supported() {
                assert!(capability.graph_presentations.is_empty());
            } else if matches!(
                capability.diagram_type,
                "agentflow" | "flowchart" | "requirement" | "swimlane" | "state"
            ) {
                assert_eq!(capability.graph_presentations, CORE_GRAPH_PRESENTATIONS);
                assert!(capability.supports_graph_presentation(AsciiGraphPresentation::Adaptive));
            } else {
                assert_eq!(capability.graph_presentations, STRICT_GRAPH_PRESENTATIONS);
                assert!(!capability.supports_graph_presentation(AsciiGraphPresentation::Adaptive));
            }
        }
    }

    #[test]
    fn semantic_coverage_projection_and_fallback_are_independent() {
        let class = find("class");
        assert_eq!(
            class.semantic_coverage,
            Some(AsciiSemanticCoverage::Partial)
        );
        assert_eq!(
            class.primary_projection,
            AsciiPrimaryProjection::Diagrammatic
        );
        assert_eq!(class.support_level, AsciiSupportLevel::Partial);
        assert!(class.structured_text_fallback);

        let er = find("er");
        assert_eq!(er.semantic_coverage, Some(AsciiSemanticCoverage::Partial));
        assert_eq!(er.primary_projection, AsciiPrimaryProjection::Diagrammatic);
        assert_eq!(er.support_level, AsciiSupportLevel::Partial);
        assert!(er.structured_text_fallback);

        assert_eq!(find("flowchart").support_level, AsciiSupportLevel::Partial);
        assert_eq!(find("sequence").support_level, AsciiSupportLevel::Partial);
        assert_eq!(find("packet").support_level, AsciiSupportLevel::Partial);
        assert_eq!(
            find("packet").primary_projection,
            AsciiPrimaryProjection::Diagrammatic
        );
        let pie = find("pie");
        assert_eq!(pie.semantic_coverage, Some(AsciiSemanticCoverage::Partial));
        assert_eq!(pie.primary_projection, AsciiPrimaryProjection::Diagrammatic);
        assert_eq!(pie.support_level, AsciiSupportLevel::Partial);
        assert!(pie.structured_text_fallback);
        assert_eq!(pie.graph_presentations, STRICT_GRAPH_PRESENTATIONS);
        assert_eq!(find("treeView").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("gantt").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("xychart").support_level, AsciiSupportLevel::Partial);

        for diagram_type in [
            "gantt", "gitgraph", "journey", "kanban", "mindmap", "timeline", "treeView",
        ] {
            let capability = find(diagram_type);
            assert_eq!(
                capability.primary_projection,
                AsciiPrimaryProjection::StructuredText
            );
            assert_eq!(
                capability.semantic_coverage,
                Some(AsciiSemanticCoverage::Partial)
            );
        }

        let zenuml = find("zenuml");
        assert_eq!(zenuml.semantic_coverage, None);
        assert_eq!(zenuml.primary_projection, AsciiPrimaryProjection::None);
        assert_eq!(zenuml.support_level, AsciiSupportLevel::Unsupported);

        for capability in ascii_capabilities() {
            assert_eq!(capability.support_level, capability.derived_support_level());
            assert_eq!(
                capability.semantic_coverage.is_some(),
                !matches!(capability.primary_projection, AsciiPrimaryProjection::None),
                "{} has an invalid availability/coverage combination",
                capability.diagram_type
            );
        }
    }

    #[test]
    fn class_capability_discloses_scoped_namespace_facade_routing() {
        let class = find("class");

        assert!(class.supported_semantics.contains(
            &"readable sibling namespace, namespace-to-root, and nested namespace facade routing"
        ));
        assert!(
            class
                .supported_semantics
                .contains(&"length-framed namespace facade member identity")
        );
        assert!(
            class
                .limits
                .contains(&"dense or colliding cross-namespace relationships render as lossless relation summaries")
        );
    }

    #[test]
    fn class_and_er_capabilities_disclose_bounded_directed_channel_planning() {
        for (diagram_type, parser_test) in [
            (
                "class",
                "class_directed_cycles_share_topology_across_directions_and_declaration_order",
            ),
            (
                "er",
                "er_directed_cycles_share_topology_across_directions_and_declaration_order",
            ),
        ] {
            let capability = find(diagram_type);

            assert!(capability.supported_semantics.contains(
                &"directed component ranks and private channels for bounded cycles and DAG diamonds"
            ));
            assert!(capability.limits.contains(
                &"bounded private-channel search does not guarantee a route for every topology"
            ));
            assert!(
                capability
                    .evidence
                    .iter()
                    .any(|evidence| evidence.note.contains(parser_test)),
                "{diagram_type} should cite its parser-backed cycle and direction tests",
            );
            assert!(capability.evidence.iter().any(|evidence| {
                evidence.source.ends_with("relation_graph/tests/")
                    && evidence.note.contains("disjoint routes")
                    && evidence.note.contains("exact/N-1")
            }));
        }
    }

    #[test]
    fn beautiful_mermaid_prior_art_is_explicitly_classified() {
        for diagram_type in ["flowchart", "sequence", "class", "er", "state", "xychart"] {
            let capability = find(diagram_type);
            assert!(
                capability.evidence.iter().any(|evidence| matches!(
                    evidence.kind,
                    AsciiEvidenceKind::BeautifulMermaidPriorArt
                )),
                "{diagram_type} should keep beautiful-mermaid evidence classified"
            );
        }

        let flowchart = find("flowchart");
        assert!(flowchart.evidence.iter().any(|evidence| {
            matches!(evidence.kind, AsciiEvidenceKind::LocalAdvantage)
                && evidence.note.contains("true RL/BT")
        }));
        assert!(
            flowchart
                .supported_semantics
                .contains(&"terminal-cell wrapped node labels")
        );
        assert!(flowchart.evidence.iter().any(|evidence| {
            matches!(evidence.kind, AsciiEvidenceKind::LocalSemanticProbe)
                && evidence.source.ends_with("issue_53_long_node_labels.mmd")
        }));
    }

    #[test]
    fn every_capability_has_limits_and_evidence() {
        for capability in ascii_capabilities() {
            if capability.is_supported() {
                assert!(
                    !capability.supported_semantics.is_empty(),
                    "{} should document supported semantics",
                    capability.diagram_type
                );
            }
            assert!(
                !capability.limits.is_empty(),
                "{} should document important limits",
                capability.diagram_type
            );
            assert!(
                !capability.evidence.is_empty(),
                "{} should document evidence",
                capability.diagram_type
            );
        }
    }

    #[test]
    fn capability_evidence_uses_durable_repository_sources() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

        for capability in ascii_capabilities() {
            for evidence in capability.evidence {
                assert!(
                    !evidence.source.starts_with("repo-ref/"),
                    "{} exposes ignored evidence source {}",
                    capability.diagram_type,
                    evidence.source
                );

                let (path, anchor) = evidence
                    .source
                    .split_once('#')
                    .map_or((evidence.source, None), |(path, anchor)| {
                        (path, Some(anchor))
                    });
                let target = workspace_root.join(path);
                assert!(
                    target.exists(),
                    "{} evidence source does not exist: {}",
                    capability.diagram_type,
                    evidence.source
                );

                let Some(anchor) = anchor else {
                    continue;
                };
                let marker = ALLOWED_EVIDENCE_ANCHORS
                    .iter()
                    .find_map(|(allowed_path, allowed_anchor, marker)| {
                        (*allowed_path == path && *allowed_anchor == anchor).then_some(*marker)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{} uses an unreviewed evidence anchor: {}",
                            capability.diagram_type, evidence.source
                        )
                    });
                let document = fs::read_to_string(&target).unwrap_or_else(|error| {
                    panic!(
                        "failed to read evidence source {}: {error}",
                        target.display()
                    )
                });
                assert!(
                    document.contains(marker),
                    "{} evidence anchor is missing from {}",
                    capability.diagram_type,
                    evidence.source
                );
            }
        }
    }

    #[test]
    fn gantt_capability_claims_only_disclosed_constraint_semantics() {
        let gantt = find("gantt");

        assert!(gantt.supported_semantics.iter().any(|semantic| {
            semantic.contains("typed start and end constraints")
                && semantic.contains("dependency ids")
        }));
        assert!(
            !gantt
                .limits
                .iter()
                .any(|limit| limit.contains("dependency source expressions"))
        );
    }

    #[test]
    fn sectioned_structured_text_capabilities_disclose_injective_field_framing() {
        for diagram_type in ["gantt", "journey", "timeline"] {
            assert!(
                find(diagram_type)
                    .supported_semantics
                    .contains(&"length-framed authored report fields"),
                "{diagram_type} should disclose its injective StructuredText field ownership",
            );
        }
    }

    #[test]
    fn hierarchy_structured_text_capabilities_disclose_injective_field_framing() {
        for diagram_type in ["kanban", "mindmap", "treeView"] {
            assert!(
                find(diagram_type)
                    .supported_semantics
                    .contains(&"length-framed authored report fields"),
                "{diagram_type} should disclose its injective StructuredText field ownership",
            );
        }

        assert!(
            find("mindmap")
                .supported_semantics
                .contains(&"ASCII and Unicode tree connectors")
        );
        assert!(
            find("kanban")
                .supported_semantics
                .contains(&"group parent ownership disclosure")
        );
    }

    #[test]
    fn gitgraph_capability_discloses_direct_model_commit_type_validation() {
        assert!(
            find("gitgraph")
                .limits
                .contains(&"unknown direct-model commit types are rejected")
        );
    }

    #[test]
    fn sequence_capability_claims_participant_bounded_control_frames() {
        let sequence = find("sequence");

        assert!(
            sequence
                .supported_semantics
                .contains(&"participant-bounded nested control frames")
        );
    }

    #[test]
    fn er_capability_discloses_presentation_metadata_omission() {
        let er = find("er");

        assert!(er.limits.iter().any(|limit| {
            limit.contains("accessibility")
                && limit.contains("Mermaid diagram source comments")
                && limit.contains("styling")
                && limit.contains("intentionally omitted")
        }));
        assert!(
            !er.limits
                .iter()
                .any(|limit| limit.contains("attribute comments"))
        );
        assert!(
            er.supported_semantics
                .iter()
                .any(|semantic| semantic.contains("attribute comments"))
        );
    }

    #[test]
    fn xychart_capability_discloses_injective_fields_and_direct_model_boundaries() {
        let xychart = find("xychart");

        assert!(xychart.structured_text_fallback);
        assert!(
            xychart
                .supported_semantics
                .contains(&"length-framed exact data labels and semantic disclosure")
        );
        assert!(
            xychart
                .supported_semantics
                .contains(&"length-framed empty-chart metadata disclosure")
        );
        assert!(
            xychart
                .limits
                .contains(&"unknown direct-model orientations and band y-axes are rejected")
        );
        assert!(xychart.limits.iter().any(|limit| {
            limit.contains("accessibility title and description metadata")
                && limit.contains("intentionally omitted")
        }));
    }

    fn find(diagram_type: &str) -> AsciiCapability {
        ascii_capabilities()
            .iter()
            .copied()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap_or_else(|| panic!("missing ASCII capability for {diagram_type}"))
    }
}
