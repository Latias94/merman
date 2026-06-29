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
pub struct AsciiCapability {
    pub diagram_type: &'static str,
    pub display_name: &'static str,
    pub support_level: AsciiSupportLevel,
    pub summary_fallback: bool,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: &'static [AsciiCapabilityEvidence],
}

impl AsciiCapability {
    pub const fn is_supported(self) -> bool {
        self.support_level.is_supported()
    }
}

pub const ASCII_CAPABILITIES: &[AsciiCapability] = &[
    AsciiCapability {
        diagram_type: "class",
        display_name: "Class",
        support_level: AsciiSupportLevel::Partial,
        summary_fallback: true,
        supported_semantics: &[
            "class boxes",
            "members and methods",
            "annotations and notes",
            "common relationship markers",
            "endpoint labels",
            "routed relation lanes",
            "dense relation summaries",
        ],
        limits: &[
            "namespace containers are not drawn as nested boxes",
            "multiple relation markers on one relation are unsupported",
            "dense or grid-budgeted relation scenes can summarize",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "repo-ref/beautiful-mermaid/src/__tests__/class-integration.test.ts",
                note: "class compartments, annotations, and relationship coverage are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/class/",
                note: "local fixtures assert typed class semantics instead of copied reference spacing",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#A-CLASSER-010",
                note: "shared relation_graph gap owner records routed and summary boundaries",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "er",
        display_name: "ER",
        support_level: AsciiSupportLevel::Partial,
        summary_fallback: true,
        supported_semantics: &[
            "entity boxes",
            "attributes and key tokens",
            "relationship labels",
            "cardinality markers",
            "identifying relationships",
            "routed relation lanes",
            "dense relation summaries",
        ],
        limits: &[
            "complex cyclic topology can summarize",
            "unknown cardinality markers are unsupported",
            "unknown relationship identity kinds are unsupported",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "repo-ref/beautiful-mermaid/src/__tests__/er-integration.test.ts",
                note: "ER attributes, relationships, and cardinalities are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/er/",
                note: "local fixtures assert entity, attribute, cardinality, and summary semantics",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#A-CLASSER-010",
                note: "shared relation_graph gap owner records routed and summary boundaries",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "flowchart",
        display_name: "Flowchart / graph",
        support_level: AsciiSupportLevel::Full,
        summary_fallback: false,
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
                note: "copied graph fixtures keep the admitted byte-level oracle stable",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "repo-ref/beautiful-mermaid/src/__tests__/ascii.test.ts",
                note: "graph ASCII shape and disconnected-layout tests are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#intentional-differences",
                note: "true RL/BT handling is a local semantic target, not a beautiful-mermaid capability",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "gantt",
        display_name: "Gantt",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &[
            "titles",
            "sections",
            "tasks",
            "dates",
            "tags",
            "dependencies",
            "deterministic date formatting",
        ],
        limits: &[
            "no terminal timeline geometry",
            "output is a readable task summary",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "summary output preserves typed task data without pseudo-graph geometry",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Gantt as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "gitgraph",
        display_name: "GitGraph",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &[
            "commits",
            "branches",
            "merges",
            "tags",
            "cherry-picks",
            "ordering",
        ],
        limits: &["does not draw a full Git lane graph"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "summary output preserves graph history facts in terminal text",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies GitGraph as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "journey",
        display_name: "Journey",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &["sections", "tasks", "actors", "scores"],
        limits: &["does not draw Mermaid journey chart geometry"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "summary output preserves actor and score data in stable rows",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Journey as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "kanban",
        display_name: "Kanban",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &["columns", "cards", "assignments", "metadata"],
        limits: &["drag and board presentation metadata are not terminal output"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "summary output preserves column-first card order and metadata",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Kanban as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "mindmap",
        display_name: "Mindmap",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &["hierarchical nodes", "labels", "nesting", "wrapped text"],
        limits: &["icons images and rich browser node shapes are omitted or approximated"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "outline output preserves hierarchy instead of imitating browser geometry",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Mindmap as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "packet",
        display_name: "Packet",
        support_level: AsciiSupportLevel::Full,
        summary_fallback: false,
        supported_semantics: &["bit ranges", "labels", "row splitting", "multi-row packets"],
        limits: &["visual styling beyond terminal borders is not represented"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "typed packet ranges render as terminal-native rows",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Packet as full output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "sequence",
        display_name: "Sequence",
        support_level: AsciiSupportLevel::Full,
        summary_fallback: false,
        supported_semantics: &[
            "participants",
            "messages",
            "notes",
            "lifecycles",
            "actor boxes",
            "control blocks",
            "diagram-wide empty boxes",
            "optional mirrored actors",
            "terminal color roles",
        ],
        limits: &["actor presentation metadata and links are omitted"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::MermaidAsciiOracle,
                source: "crates/merman-ascii/tests/testdata/mermaid-ascii/sequence/",
                note: "copied sequence fixtures keep the admitted byte-level oracle stable",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "repo-ref/beautiful-mermaid/src/__tests__/sequence-integration.test.ts",
                note: "sequence parser and block-layout cases are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/sequence/",
                note: "local fixtures assert message, frame, and note semantics",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "state",
        display_name: "State",
        support_level: AsciiSupportLevel::Partial,
        summary_fallback: false,
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
                source: "repo-ref/beautiful-mermaid/src/ascii/shapes/state.ts",
                note: "state-oriented ASCII shape ideas are prior art, not a byte oracle",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/state/",
                note: "local fixtures assert composite and wide-label state behavior",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#A-STATE-010",
                note: "remaining state presentation metadata is explicitly tracked",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "timeline",
        display_name: "Timeline",
        support_level: AsciiSupportLevel::Summary,
        summary_fallback: false,
        supported_semantics: &["sections", "events", "ordered grouped text"],
        limits: &["does not draw Mermaid timeline geometry"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "summary output keeps section and event order stable",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies Timeline as summary output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "treeView",
        display_name: "TreeView",
        support_level: AsciiSupportLevel::Full,
        summary_fallback: false,
        supported_semantics: &[
            "tree nodes",
            "folders and leaves",
            "indentation",
            "tree connectors",
        ],
        limits: &["browser tree styling is not represented"],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "crates/merman-ascii/README.md#shipped-diagram-matrix",
                note: "tree output is typed terminal structure and is not tied to metadata ids",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies TreeView as full output",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "xychart",
        display_name: "XYChart",
        support_level: AsciiSupportLevel::Partial,
        summary_fallback: false,
        supported_semantics: &[
            "compact bar and line plots",
            "mixed plots",
            "horizontal mode",
            "titles and axes",
            "legends",
            "data labels",
            "configurable plot dimensions",
        ],
        limits: &[
            "tooltips are not represented",
            "SVG coordinate precision is not represented",
            "dense data uses terminal-compact layout",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::BeautifulMermaidPriorArt,
                source: "repo-ref/beautiful-mermaid/src/__tests__/xychart-ascii.test.ts",
                note: "XYChart ASCII and legend behavior are capability prior art",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalSemanticProbe,
                source: "crates/merman-ascii/tests/testdata/local-semantic/xychart/",
                note: "local fixtures assert typed chart labels values and wide-cell behavior",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::GapRegistry,
                source: "crates/merman-ascii/ASCII_GAP_REGISTRY.md#A-XY-010",
                note: "richer terminal chart disclosure remains an explicit gap",
            },
        ],
    },
    AsciiCapability {
        diagram_type: "zenuml",
        display_name: "ZenUML",
        support_level: AsciiSupportLevel::Partial,
        summary_fallback: false,
        supported_semantics: &[
            "participants",
            "messages",
            "basic conditional frames",
            "sequence-like output",
        ],
        limits: &[
            "external ZenUML compatibility is a subset",
            "unsupported syntax is rejected before terminal output",
        ],
        evidence: &[
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::LocalAdvantage,
                source: "docs/alignment/ZENUML_UPSTREAM_TEST_COVERAGE.md",
                note: "headless ZenUML subset translates through typed sequence rendering",
            },
            AsciiCapabilityEvidence {
                kind: AsciiEvidenceKind::SupportMatrix,
                source: "docs/rendering/ASCII_SUPPORT_MATRIX.md#supported-families",
                note: "support matrix classifies ZenUML as partial output",
            },
        ],
    },
];

pub fn ascii_capabilities() -> &'static [AsciiCapability] {
    ASCII_CAPABILITIES
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_diagram_types_are_derived_from_capabilities() {
        let derived: Vec<_> = ascii_capabilities()
            .iter()
            .filter(|capability| capability.is_supported())
            .map(|capability| capability.diagram_type)
            .collect();

        assert_eq!(ascii_supported_diagram_types(), derived.as_slice());
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
                "zenuml",
            ]
        );
    }

    #[test]
    fn support_levels_match_public_matrix_boundary() {
        let class = find("class");
        assert_eq!(class.support_level, AsciiSupportLevel::Partial);
        assert!(class.summary_fallback);

        let er = find("er");
        assert_eq!(er.support_level, AsciiSupportLevel::Partial);
        assert!(er.summary_fallback);

        assert_eq!(find("flowchart").support_level, AsciiSupportLevel::Full);
        assert_eq!(find("sequence").support_level, AsciiSupportLevel::Full);
        assert_eq!(find("packet").support_level, AsciiSupportLevel::Full);
        assert_eq!(find("treeView").support_level, AsciiSupportLevel::Full);
        assert_eq!(find("gantt").support_level, AsciiSupportLevel::Summary);
        assert_eq!(find("xychart").support_level, AsciiSupportLevel::Partial);
        assert_eq!(find("zenuml").support_level, AsciiSupportLevel::Partial);
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
            assert!(
                !capability.supported_semantics.is_empty(),
                "{} should document supported semantics",
                capability.diagram_type
            );
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

    fn find(diagram_type: &str) -> AsciiCapability {
        ascii_capabilities()
            .iter()
            .copied()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap_or_else(|| panic!("missing ASCII capability for {diagram_type}"))
    }
}
