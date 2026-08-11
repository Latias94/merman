use std::sync::OnceLock;

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
    /// Compatibility view derived from semantic coverage and the primary projection.
    pub support_level: AsciiSupportLevel,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: &'static [AsciiCapabilityEvidence],
}

impl AsciiCapability {
    const fn from_definition(definition: AsciiCapabilityDefinition) -> Self {
        let support_level =
            derive_support_level(definition.semantic_coverage, definition.primary_projection);
        Self {
            diagram_type: definition.diagram_type,
            display_name: definition.display_name,
            semantic_coverage: definition.semantic_coverage,
            primary_projection: definition.primary_projection,
            structured_text_fallback: definition.structured_text_fallback,
            support_level,
            supported_semantics: definition.supported_semantics,
            limits: definition.limits,
            evidence: definition.evidence,
        }
    }

    const fn unsupported(diagram_type: &'static str) -> Self {
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
            "self-relation loops",
            "bounded iterative relation-layer sweeps",
            "routed relation lanes",
            "independent relation components",
            "lossless crossing, port-fit, route, and overlay collision summaries",
        ],
        limits: &[
            "cross-namespace or cross-container relationships render as relation summaries",
            "parallel relationship lanes whose ports do not fit render as lossless relation summaries",
            "dense or collision-prone relation scenes can summarize",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "class compartments, annotations, and relationship coverage are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/class/",
                note: "local fixtures assert typed class semantics instead of copied reference spacing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#a-classer-010",
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
            "attributes and key tokens",
            "relationship labels",
            "cardinality markers",
            "parent diamond cardinality markers",
            "identifying relationships",
            "top-down, bottom-up, left-right, and right-left directions",
            "self-relationship loops",
            "bounded iterative relation-layer sweeps",
            "routed relation lanes",
            "independent relation components",
            "lossless crossing, port-fit, route, and overlay collision summaries",
        ],
        limits: &[
            "parallel relationship lanes whose ports do not fit render as lossless relation summaries",
            "complex cyclic or collision-prone topology can summarize",
            "unknown cardinality markers are unsupported",
            "unknown relationship identity kinds are unsupported",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "ER attributes, relationships, and cardinalities are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/er/",
                note: "local fixtures assert entity, attribute, cardinality, and summary semantics",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#a-classer-010",
                note: "shared relation_graph gap owner records routed and summary boundaries",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "flowchart",
        display_name: "Flowchart / graph",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: false,
        supported_semantics: &[
            "root directions",
            "boxed nodes and common shapes",
            "edge labels",
            "open dotted and thick edges",
            "subgraphs and nested groups",
            "boundary-aware routes",
            "terminal color roles",
        ],
        limits: &[
            "icons and images are omitted",
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
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "graph ASCII shape and disconnected-layout tests are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#intentional-differences",
                note: "true RL/BT handling is a local semantic target, not a beautiful-mermaid capability",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "gantt",
        display_name: "Gantt",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: false,
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
                note: "typed-model tests preserve task identity and typed scheduling constraints without claiming pseudo-graph geometry",
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
        structured_text_fallback: false,
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
        structured_text_fallback: false,
        supported_semantics: &["sections", "tasks", "actors", "scores"],
        limits: &["does not draw Mermaid journey chart geometry"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve actor and score data in stable rows",
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
        structured_text_fallback: false,
        supported_semantics: &[
            "columns",
            "cards",
            "stable card and group ids",
            "assignments",
            "metadata",
            "deterministic Unassigned grouping",
        ],
        limits: &[
            "drag and board presentation metadata are not terminal output",
            "duplicate or empty ids are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve column-first card order and metadata",
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
        structured_text_fallback: false,
        supported_semantics: &[
            "hierarchical nodes",
            "stable authored node ids",
            "labels",
            "nesting",
            "wrapped text",
            "shape, icon, and section disclosure",
            "disconnected components and cycles",
            "validated edge endpoints",
        ],
        limits: &[
            "icons and rich browser node shapes are disclosed as text rather than styled",
            "duplicate internal or authored ids, missing authored ids, parallel edges, and missing endpoints are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve hierarchy instead of imitating browser geometry",
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
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: false,
        supported_semantics: &["bit ranges", "labels", "row splitting", "multi-row packets"],
        limits: &[
            "output is an ordered row report rather than a spatial bit-width grid",
            "visual styling beyond terminal borders is not represented",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests render packet ranges as terminal-native rows",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies Packet as partial structured text",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "sequence",
        display_name: "Sequence",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: false,
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
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
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
        structured_text_fallback: false,
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
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "state-oriented ASCII shape ideas are prior art, not a byte oracle",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/state/",
                note: "local fixtures assert composite and wide-label state behavior",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#a-state-010",
                note: "remaining state presentation metadata is explicitly tracked",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "timeline",
        display_name: "Timeline",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::StructuredText,
        structured_text_fallback: false,
        supported_semantics: &["sections", "events", "direction", "ordered grouped text"],
        limits: &[
            "does not draw Mermaid timeline geometry",
            "parser bookkeeping score is intentionally omitted",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests keep section and event order stable",
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
        structured_text_fallback: false,
        supported_semantics: &[
            "hierarchical outline order",
            "root and node identities with authored levels",
            "directory and file distinction",
            "ASCII and Unicode tree connectors",
            "icons classes and descriptions as text disclosure",
        ],
        limits: &[
            "outline output does not claim two-dimensional diagram geometry",
            "browser icons and CSS classes are disclosed rather than styled",
            "duplicate node ids are rejected",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/new_family_models.rs",
                note: "typed-model tests preserve hierarchy and annotations as terminal text",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#structured-text-outputs",
                note: "support matrix classifies TreeView as structured outline output",
            },
        ],
    },
    AsciiCapabilityDefinition {
        diagram_type: "xychart",
        display_name: "XYChart",
        semantic_coverage: Some(AsciiSemanticCoverage::Partial),
        primary_projection: AsciiPrimaryProjection::Diagrammatic,
        structured_text_fallback: false,
        supported_semantics: &[
            "model-owned x/y sample coordinates and point labels",
            "parser-produced x coordinates derived from typed axes, categories, and sample order",
            "band and linear axes with compact scale-aware ticks",
            "grouped bar, topology-resolved line, and mixed plots",
            "horizontal and vertical orientation",
            "titles and axes",
            "legends",
            "exact data labels and semantic disclosure",
            "configurable plot dimensions",
        ],
        limits: &[
            "browser hover tooltips are replaced by deterministic terminal disclosure",
            "typed chart coordinates are independently quantized by the terminal plan",
            "cross-series same-cell collisions use deterministic paint order plus exact disclosure",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison",
                note: "XYChart ASCII and legend behavior are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/xychart_model.rs",
                note: "semantic tests assert model-owned coordinates, parser-derived x positions, grouped lanes, missing-sample gaps, connected horizontal paths, precision, clipping, collisions, labels, and resource extents",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#a-xy-010",
                note: "cross-series same-cell ownership remains an explicit compact-layout residual",
            },
        ],
    },
];

pub fn ascii_capabilities() -> &'static [AsciiCapability] {
    static CAPABILITIES: OnceLock<Vec<AsciiCapability>> = OnceLock::new();
    CAPABILITIES
        .get_or_init(|| {
            merman_core::built_in_typed_render_families()
                .iter()
                .map(|family| {
                    ASCII_CAPABILITY_DEFINITIONS
                        .iter()
                        .find(|definition| definition.diagram_type == family.diagram_type)
                        .copied()
                        .map(AsciiCapability::from_definition)
                        .unwrap_or_else(|| AsciiCapability::unsupported(family.diagram_type))
                })
                .collect()
        })
        .as_slice()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::Path};

    const ALLOWED_EVIDENCE_ANCHORS: &[(&str, &str, &str)] = &[
        (
            "crates/merman-ascii/ASCII_GAP_REGISTRY.md",
            "a-classer-010",
            r#"id="a-classer-010""#,
        ),
        (
            "crates/merman-ascii/ASCII_GAP_REGISTRY.md",
            "a-state-010",
            r#"id="a-state-010""#,
        ),
        (
            "crates/merman-ascii/ASCII_GAP_REGISTRY.md",
            "a-xy-010",
            r#"id="a-xy-010""#,
        ),
        (
            "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md",
            "family-comparison",
            "## Family Comparison",
        ),
        (
            "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md",
            "intentional-differences",
            "## Intentional Differences",
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
        let core_families = merman_core::built_in_typed_render_families();
        let capability_types = capabilities
            .iter()
            .map(|capability| capability.diagram_type)
            .collect::<BTreeSet<_>>();
        let core_types = core_families
            .iter()
            .map(|family| family.diagram_type)
            .collect::<BTreeSet<_>>();

        assert_eq!(capabilities.len(), 31);
        assert_eq!(capability_types.len(), capabilities.len());
        assert_eq!(capability_types, core_types);
        assert!(!capability_types.contains("error"));
        assert!(!capability_types.contains("custom-json"));
    }

    #[test]
    fn output_available_and_diagrammatic_lists_are_distinct_projections() {
        assert_eq!(
            ascii_supported_diagram_types(),
            &[
                "class",
                "er",
                "flowchart",
                "gantt",
                "gitgraph",
                "journey",
                "kanban",
                "mindmap",
                "packet",
                "sequence",
                "state",
                "timeline",
                "treeView",
                "xychart",
            ]
        );
        assert_eq!(
            ascii_diagrammatic_diagram_types(),
            &["class", "er", "flowchart", "sequence", "state", "xychart",]
        );
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
        assert_eq!(find("packet").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("treeView").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("gantt").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("xychart").support_level, AsciiSupportLevel::Partial);

        for diagram_type in [
            "gantt", "gitgraph", "journey", "kanban", "mindmap", "packet", "timeline", "treeView",
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
    fn sequence_capability_claims_participant_bounded_control_frames() {
        let sequence = find("sequence");

        assert!(
            sequence
                .supported_semantics
                .contains(&"participant-bounded nested control frames")
        );
    }

    fn find(diagram_type: &str) -> AsciiCapability {
        ascii_capabilities()
            .iter()
            .copied()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap_or_else(|| panic!("missing ASCII capability for {diagram_type}"))
    }
}
