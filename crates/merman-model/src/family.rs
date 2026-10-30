//! Pinned Mermaid identity and model availability, independent of source and output backends.
//!
//! Logical families, diagram-type aliases, metadata names, and model kinds are separate facts.
//! Model availability reports the payload compiled into this package; it does not admit a
//! source parser, editor provider, terminal handler, or graphical renderer.

use serde::Serialize;
use std::fmt;
use std::sync::OnceLock;

/// Closed, catalog-owned identity for one logical Mermaid diagram family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DiagramFamilyId(&'static str);

impl DiagramFamilyId {
    /// Returns the stable string identifier used by public metadata and serde output.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagramFamilyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Public Cargo selector for one logical family, regardless of this build's availability.
///
/// Aliases share the selector of their owning family. Infrastructure models have no selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct DiagramFamilySelector {
    pub logical_family_kind: &'static str,
    pub feature: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagramHeaderFact {
    pub diagram_type: &'static str,
    pub label: &'static str,
    pub detail: &'static str,
}

/// Pinned public metadata spelling and its enumeration order, independent of availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MetadataDefinition {
    pub id: &'static str,
    pub order: Option<u16>,
}

const fn metadata(id: &'static str, order: Option<u16>) -> MetadataDefinition {
    MetadataDefinition { id, order }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct HeaderDefinition {
    pub order: u16,
    pub label: &'static str,
    pub detail: &'static str,
}

const fn header(order: u16, label: &'static str, detail: &'static str) -> HeaderDefinition {
    HeaderDefinition {
        order,
        label,
        detail,
    }
}

/// Data-only identity for a pinned diagram type, including source aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FamilyVariantIdentity {
    pub key: DiagramTypeId,
    pub id: &'static str,
    pub catalog_order: u16,
    pub render_model_kind: Option<&'static str>,
    pub metadata: Option<MetadataDefinition>,
    pub headers: &'static [HeaderDefinition],
    pub frontmatter_alias_order: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FamilyConfigDefinition {
    pub namespace: &'static str,
    pub frontmatter_order: u16,
}

/// One logical family and the pinned type identities sharing its payload contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiagramFamilyDefinition {
    pub id: DiagramFamilyId,
    pub selector: Option<&'static str>,
    /// Payload availability, including payloads shared across logical families.
    pub model_available: bool,
    pub config: Option<FamilyConfigDefinition>,
    pub variants: &'static [FamilyVariantIdentity],
}

/// Portable identity and payload capability; source and output availability have their own owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ModelFamilyCapability {
    pub diagram_type: &'static str,
    pub logical_family_kind: &'static str,
    pub metadata_id: Option<&'static str>,
    pub render_model_kind: Option<&'static str>,
    pub has_model: bool,
}

/// Iterates all pinned identities, including aliases whose payload is unavailable in this build.
pub fn diagram_family_identities() -> impl Iterator<
    Item = (
        &'static DiagramFamilyDefinition,
        &'static FamilyVariantIdentity,
    ),
> {
    FAMILY_CATALOG
        .iter()
        .flat_map(|family| family.variants.iter().map(move |variant| (family, variant)))
}

/// Finds a pinned diagram-type spelling without requiring its model or any source provider.
pub fn find_variant(
    diagram_type: &str,
) -> Option<(
    &'static DiagramFamilyDefinition,
    &'static FamilyVariantIdentity,
)> {
    diagram_family_identities().find(|(_, variant)| variant.id == diagram_type)
}

/// Returns the public metadata spelling associated with a pinned diagram type.
pub fn diagram_type_metadata_id(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).and_then(|(_, variant)| variant.metadata.map(|metadata| metadata.id))
}

/// Returns logical family ownership, retaining aliases and shared-model distinctions.
pub fn diagram_type_family_id(diagram_type: &str) -> Option<DiagramFamilyId> {
    find_variant(diagram_type).map(|(family, _)| family.id)
}

/// Returns the stable logical family spelling independently of payload availability.
pub fn diagram_type_family_kind(diagram_type: &str) -> Option<&'static str> {
    diagram_type_family_id(diagram_type).map(DiagramFamilyId::as_str)
}

/// Returns the declared model kind independently of parser, handler, or payload availability.
pub fn diagram_type_render_model_kind(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).and_then(|(_, variant)| variant.render_model_kind)
}

/// Returns whether this package contains the declared payload, independently of source parsers.
///
/// Flowchart and Swimlane share one payload. Selecting Agentflow also makes that shared data
/// available, without selecting either language's source parser or output handler.
pub fn has_diagram_model(diagram_type: &str) -> bool {
    find_variant(diagram_type).is_some_and(|(family, _)| family.model_available)
}

/// Checks the pinned model-kind association without promising any executable provider.
pub fn render_model_kind_supports_diagram_type(model_kind: &str, diagram_type: &str) -> bool {
    diagram_type_render_model_kind(diagram_type) == Some(model_kind)
}

/// Returns each logical family's positive selector once, excluding infrastructure models.
pub fn diagram_family_selectors() -> &'static [DiagramFamilySelector] {
    static SELECTORS: OnceLock<Vec<DiagramFamilySelector>> = OnceLock::new();
    SELECTORS.get_or_init(|| {
        FAMILY_CATALOG
            .iter()
            .filter_map(|family| {
                family.selector.map(|feature| DiagramFamilySelector {
                    logical_family_kind: family.id.as_str(),
                    feature,
                })
            })
            .collect()
    })
}

/// Returns every known diagram type in pinned order, including unavailable model payloads.
pub fn model_family_capabilities() -> &'static [ModelFamilyCapability] {
    static CAPABILITIES: OnceLock<Vec<ModelFamilyCapability>> = OnceLock::new();
    CAPABILITIES.get_or_init(|| {
        let mut capabilities = diagram_family_identities()
            .map(|(family, variant)| {
                (
                    variant.catalog_order,
                    ModelFamilyCapability {
                        diagram_type: variant.id,
                        logical_family_kind: family.id.as_str(),
                        metadata_id: variant.metadata.map(|metadata| metadata.id),
                        render_model_kind: variant.render_model_kind,
                        has_model: family.model_available,
                    },
                )
            })
            .collect::<Vec<_>>();
        capabilities.sort_by_key(|(order, _)| *order);
        capabilities
            .into_iter()
            .map(|(_, capability)| capability)
            .collect()
    })
}

macro_rules! define_family_catalog {
    (
        $(
            $constant:ident => $family_id:literal {
                selector: $selector:expr,
                model_available: $model_available:expr,
                config: $config:expr,
                variants: [
                    $(
                        $key:ident => $id:literal {
                            catalog_order: $catalog_order:literal,
                            render_kind: $render_kind:expr,
                            metadata: $metadata:expr,
                            headers: $headers:expr,
                            config_alias_order: $config_alias_order:expr $(,)?
                        }
                    ),+ $(,)?
                ] $(,)?
            }
        ),+ $(,)?
    ) => {
        /// Closed pinned diagram-type IDs used by executable owners to bind their providers.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        pub enum DiagramTypeId {
            $($(
                #[serde(rename = $id)]
                $key,
            )+)+
        }

        impl DiagramTypeId {
            pub const fn all() -> &'static [Self] {
                &[$($(Self::$key,)+)+]
            }

            pub const fn as_str(self) -> &'static str {
                match self { $($(Self::$key => $id,)+)+ }
            }

            pub fn from_id(value: &str) -> Option<Self> {
                match value { $($($id => Some(Self::$key),)+)+ _ => None }
            }
        }

        impl DiagramFamilyId {
            $(
                #[doc = concat!("Built-in `", $family_id, "` diagram family identity.")]
                pub const $constant: Self = Self($family_id);
            )+

            pub const fn all() -> &'static [Self] {
                &[$(Self::$constant),+]
            }

            pub fn from_id(value: &str) -> Option<Self> {
                match value { $($family_id => Some(Self::$constant),)+ _ => None }
            }
        }

        /// The single pinned declaration of family identities, aliases, and data relationships.
        pub const FAMILY_CATALOG: &[DiagramFamilyDefinition] = &[
            $(
                DiagramFamilyDefinition {
                    id: DiagramFamilyId::$constant,
                    selector: $selector,
                    model_available: $model_available,
                    config: $config,
                    variants: &[
                        $(
                            FamilyVariantIdentity {
                                key: DiagramTypeId::$key,
                                id: $id,
                                catalog_order: $catalog_order,
                                render_model_kind: $render_kind,
                                metadata: $metadata,
                                headers: $headers,
                                frontmatter_alias_order: $config_alias_order,
                            },
                        )+
                    ],
                },
            )+
        ];
    };
}

const FLOWCHART_HEADERS: &[HeaderDefinition] = &[
    header(0, "flowchart TD", "flowchart header"),
    header(1, "graph TD", "flowchart alias"),
];
const SEQUENCE_HEADERS: &[HeaderDefinition] = &[header(2, "sequenceDiagram", "sequence header")];
const SWIMLANE_HEADERS: &[HeaderDefinition] = &[header(3, "swimlane-beta", "swimlane header")];
const CLASS_HEADERS: &[HeaderDefinition] = &[
    header(4, "classDiagram", "class header"),
    header(5, "classDiagram-v2", "class header"),
];
const STATE_HEADERS: &[HeaderDefinition] = &[
    header(6, "stateDiagram-v2", "state header"),
    header(7, "stateDiagram", "legacy state header"),
];
const ER_HEADERS: &[HeaderDefinition] = &[header(8, "erDiagram", "er header")];
const GANTT_HEADERS: &[HeaderDefinition] = &[header(9, "gantt", "gantt header")];
const MINDMAP_HEADERS: &[HeaderDefinition] = &[header(10, "mindmap", "mindmap header")];
const INFO_HEADERS: &[HeaderDefinition] = &[header(11, "info", "info header")];
const JOURNEY_HEADERS: &[HeaderDefinition] = &[header(12, "journey", "journey header")];
const TIMELINE_HEADERS: &[HeaderDefinition] = &[header(13, "timeline", "timeline header")];
const GIT_GRAPH_HEADERS: &[HeaderDefinition] = &[header(14, "gitGraph", "git graph header")];
const PIE_HEADERS: &[HeaderDefinition] = &[header(15, "pie", "pie header")];
const REQUIREMENT_HEADERS: &[HeaderDefinition] =
    &[header(16, "requirementDiagram", "requirement header")];
const SANKEY_HEADERS: &[HeaderDefinition] = &[header(17, "sankey", "sankey header")];
const PACKET_HEADERS: &[HeaderDefinition] = &[
    header(18, "packet", "packet header"),
    header(19, "packet-beta", "packet beta header"),
];
const XYCHART_HEADERS: &[HeaderDefinition] = &[
    header(20, "xychart", "xychart header"),
    header(21, "xychart-beta", "xychart beta header"),
];
const TREE_VIEW_HEADERS: &[HeaderDefinition] = &[header(22, "treeView-beta", "tree view header")];
const ISHIKAWA_HEADERS: &[HeaderDefinition] = &[header(23, "ishikawa-beta", "ishikawa header")];
const EVENTMODELING_HEADERS: &[HeaderDefinition] =
    &[header(24, "eventmodeling", "event modeling header")];
const QUADRANT_HEADERS: &[HeaderDefinition] =
    &[header(25, "quadrantChart", "quadrant chart header")];
const VENN_HEADERS: &[HeaderDefinition] = &[header(26, "venn-beta", "venn header")];
const ZENUML_HEADERS: &[HeaderDefinition] = &[header(27, "zenuml", "zenuml header")];
const C4_HEADERS: &[HeaderDefinition] = &[
    header(28, "C4Context", "c4 context header"),
    header(29, "C4Container", "c4 container header"),
    header(30, "C4Component", "c4 component header"),
    header(31, "C4Dynamic", "c4 dynamic header"),
    header(32, "C4Deployment", "c4 deployment header"),
];
const KANBAN_HEADERS: &[HeaderDefinition] = &[header(33, "kanban", "kanban header")];
const ARCHITECTURE_HEADERS: &[HeaderDefinition] =
    &[header(34, "architecture-beta", "architecture header")];
const BLOCK_HEADERS: &[HeaderDefinition] = &[header(35, "block-beta", "block header")];
const RADAR_HEADERS: &[HeaderDefinition] = &[header(36, "radar-beta", "radar header")];
const TREEMAP_HEADERS: &[HeaderDefinition] = &[header(37, "treemap-beta", "treemap header")];
const RAILROAD_HEADERS: &[HeaderDefinition] = &[header(38, "railroad-beta", "railroad header")];
const RAILROAD_EBNF_HEADERS: &[HeaderDefinition] =
    &[header(39, "railroad-ebnf-beta", "railroad ebnf header")];
const RAILROAD_ABNF_HEADERS: &[HeaderDefinition] =
    &[header(40, "railroad-abnf-beta", "railroad abnf header")];
const RAILROAD_PEG_HEADERS: &[HeaderDefinition] =
    &[header(41, "railroad-peg-beta", "railroad peg header")];
const WARDLEY_HEADERS: &[HeaderDefinition] = &[header(42, "wardley-beta", "wardley header")];
const CYNEFIN_HEADERS: &[HeaderDefinition] = &[header(43, "cynefin-beta", "cynefin header")];
const FLOWCHART_ELK_HEADERS: &[HeaderDefinition] =
    &[header(44, "flowchart-elk TD", "elk flowchart header")];
const USECASE_HEADERS: &[HeaderDefinition] =
    &[header(45, "usecase-beta", "usecase diagram header")];
const AGENTFLOW_HEADERS: &[HeaderDefinition] =
    &[header(46, "agentflow-beta", "agentflow diagram header")];

define_family_catalog! {
    ERROR => "error" {
        selector: None,
        model_available: true,
        config: None,
        variants: [
            Error => "error" {
                catalog_order: 0,
                render_kind: Some("error"),
                metadata: None,
                headers: &[],
                config_alias_order: None,
            },
        ],
    },
    FLOWCHART => "flowchart" {
        selector: Some("diagram-flowchart"),
        model_available: cfg!(any(feature = "diagram-flowchart", feature = "diagram-swimlane", feature = "diagram-agentflow")),
        config: Some(FamilyConfigDefinition {
            namespace: "flowchart",
            frontmatter_order: 7,
        }),
        variants: [
            FlowchartElk => "flowchart-elk" {
                catalog_order: 2,
                render_kind: Some("flowchart"),
                metadata: Some(metadata("flowchart", None)),
                headers: FLOWCHART_ELK_HEADERS,
                config_alias_order: Some(3),
            },
            FlowchartV2 => "flowchart-v2" {
                catalog_order: 17,
                render_kind: Some("flowchart"),
                metadata: Some(metadata("flowchart", Some(7))),
                headers: FLOWCHART_HEADERS,
                config_alias_order: Some(2),
            },
            Flowchart => "flowchart" {
                catalog_order: 18,
                render_kind: Some("flowchart"),
                metadata: Some(metadata("flowchart", None)),
                headers: &[],
                config_alias_order: None,
            },
        ],
    },
    SWIMLANE => "swimlane" {
        selector: Some("diagram-swimlane"),
        model_available: cfg!(any(feature = "diagram-flowchart", feature = "diagram-swimlane", feature = "diagram-agentflow")),
        config: Some(FamilyConfigDefinition {
            namespace: "swimlane",
            frontmatter_order: 23,
        }),
        variants: [
            Swimlane => "swimlane" {
                catalog_order: 16,
                render_kind: Some("flowchart"),
                metadata: Some(metadata("swimlane", Some(27))),
                headers: SWIMLANE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    MINDMAP => "mindmap" {
        selector: Some("diagram-mindmap"),
        model_available: cfg!(feature = "diagram-mindmap"),
        config: Some(FamilyConfigDefinition {
            namespace: "mindmap",
            frontmatter_order: 13,
        }),
        variants: [
            Mindmap => "mindmap" {
                catalog_order: 3,
                render_kind: Some("mindmap"),
                metadata: Some(metadata("mindmap", Some(14))),
                headers: MINDMAP_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    ARCHITECTURE => "architecture" {
        selector: Some("diagram-architecture"),
        model_available: cfg!(feature = "diagram-architecture"),
        config: Some(FamilyConfigDefinition {
            namespace: "architecture",
            frontmatter_order: 0,
        }),
        variants: [
            Architecture => "architecture" {
                catalog_order: 4,
                render_kind: Some("architecture"),
                metadata: Some(metadata("architecture", Some(0))),
                headers: ARCHITECTURE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    ZENUML => "zenuml" {
        selector: Some("diagram-zenuml"),
        model_available: cfg!(feature = "diagram-zenuml"),
        config: Some(FamilyConfigDefinition {
            namespace: "zenuml",
            frontmatter_order: 30,
        }),
        variants: [
            Zenuml => "zenuml" {
                catalog_order: 5,
                render_kind: Some("zenuml"),
                metadata: Some(metadata("zenuml", Some(34))),
                headers: ZENUML_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    SEQUENCE => "sequence" {
        selector: Some("diagram-sequence"),
        model_available: cfg!(feature = "diagram-sequence"),
        config: Some(FamilyConfigDefinition {
            namespace: "sequence",
            frontmatter_order: 21,
        }),
        variants: [
            Sequence => "sequence" {
                catalog_order: 15,
                render_kind: Some("sequence"),
                metadata: Some(metadata("sequence", Some(25))),
                headers: SEQUENCE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    C4 => "c4" {
        selector: Some("diagram-c4"),
        model_available: cfg!(feature = "diagram-c4"),
        config: Some(FamilyConfigDefinition {
            namespace: "c4",
            frontmatter_order: 2,
        }),
        variants: [
            C4 => "c4" {
                catalog_order: 6,
                render_kind: Some("c4"),
                metadata: Some(metadata("c4", Some(2))),
                headers: C4_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    KANBAN => "kanban" {
        selector: Some("diagram-kanban"),
        model_available: cfg!(feature = "diagram-kanban"),
        config: Some(FamilyConfigDefinition {
            namespace: "kanban",
            frontmatter_order: 12,
        }),
        variants: [
            Kanban => "kanban" {
                catalog_order: 7,
                render_kind: Some("kanban"),
                metadata: Some(metadata("kanban", Some(13))),
                headers: KANBAN_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    CLASS => "class" {
        selector: Some("diagram-class"),
        model_available: cfg!(feature = "diagram-class"),
        config: Some(FamilyConfigDefinition {
            namespace: "class",
            frontmatter_order: 3,
        }),
        variants: [
            ClassDiagram => "classDiagram" {
                catalog_order: 8,
                render_kind: Some("class"),
                metadata: Some(metadata("class", Some(3))),
                headers: CLASS_HEADERS,
                config_alias_order: Some(0),
            },
            Class => "class" {
                catalog_order: 9,
                render_kind: Some("class"),
                metadata: Some(metadata("class", None)),
                headers: &[],
                config_alias_order: None,
            },
        ],
    },
    ER => "er" {
        selector: Some("diagram-er"),
        model_available: cfg!(feature = "diagram-er"),
        config: Some(FamilyConfigDefinition {
            namespace: "er",
            frontmatter_order: 5,
        }),
        variants: [
            Er => "er" {
                catalog_order: 10,
                render_kind: Some("er"),
                metadata: Some(metadata("er", Some(5))),
                headers: ER_HEADERS,
                config_alias_order: None,
            },
            ErDiagram => "erDiagram" {
                catalog_order: 41,
                render_kind: Some("er"),
                metadata: Some(metadata("er", None)),
                headers: &[],
                config_alias_order: Some(1),
            },
        ],
    },
    GANTT => "gantt" {
        selector: Some("diagram-gantt"),
        model_available: cfg!(feature = "diagram-gantt"),
        config: Some(FamilyConfigDefinition {
            namespace: "gantt",
            frontmatter_order: 8,
        }),
        variants: [
            Gantt => "gantt" {
                catalog_order: 11,
                render_kind: Some("gantt"),
                metadata: Some(metadata("gantt", Some(8))),
                headers: GANTT_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    INFO => "info" {
        selector: Some("diagram-info"),
        model_available: cfg!(feature = "diagram-info"),
        config: None,
        variants: [
            Info => "info" {
                catalog_order: 12,
                render_kind: Some("info"),
                metadata: Some(metadata("info", Some(10))),
                headers: INFO_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    PIE => "pie" {
        selector: Some("diagram-pie"),
        model_available: cfg!(feature = "diagram-pie"),
        config: Some(FamilyConfigDefinition {
            namespace: "pie",
            frontmatter_order: 15,
        }),
        variants: [
            Pie => "pie" {
                catalog_order: 13,
                render_kind: Some("pie"),
                metadata: Some(metadata("pie", Some(16))),
                headers: PIE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    REQUIREMENT => "requirement" {
        selector: Some("diagram-requirement"),
        model_available: cfg!(feature = "diagram-requirement"),
        config: Some(FamilyConfigDefinition {
            namespace: "requirement",
            frontmatter_order: 19,
        }),
        variants: [
            Requirement => "requirement" {
                catalog_order: 14,
                render_kind: Some("requirement"),
                metadata: Some(metadata("requirement", Some(23))),
                headers: REQUIREMENT_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    TIMELINE => "timeline" {
        selector: Some("diagram-timeline"),
        model_available: cfg!(feature = "diagram-timeline"),
        config: Some(FamilyConfigDefinition {
            namespace: "timeline",
            frontmatter_order: 24,
        }),
        variants: [
            Timeline => "timeline" {
                catalog_order: 19,
                render_kind: Some("timeline"),
                metadata: Some(metadata("timeline", Some(28))),
                headers: TIMELINE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    GIT_GRAPH => "gitGraph" {
        selector: Some("diagram-git-graph"),
        model_available: cfg!(feature = "diagram-git-graph"),
        config: Some(FamilyConfigDefinition {
            namespace: "gitGraph",
            frontmatter_order: 9,
        }),
        variants: [
            GitGraph => "gitGraph" {
                catalog_order: 20,
                render_kind: Some("gitGraph"),
                metadata: Some(metadata("gitgraph", Some(9))),
                headers: GIT_GRAPH_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    STATE => "state" {
        selector: Some("diagram-state"),
        model_available: cfg!(feature = "diagram-state"),
        config: Some(FamilyConfigDefinition {
            namespace: "state",
            frontmatter_order: 22,
        }),
        variants: [
            StateDiagram => "stateDiagram" {
                catalog_order: 21,
                render_kind: Some("state"),
                metadata: Some(metadata("state", Some(26))),
                headers: STATE_HEADERS,
                config_alias_order: Some(4),
            },
            State => "state" {
                catalog_order: 22,
                render_kind: Some("state"),
                metadata: Some(metadata("state", None)),
                headers: &[],
                config_alias_order: None,
            },
        ],
    },
    JOURNEY => "journey" {
        selector: Some("diagram-journey"),
        model_available: cfg!(feature = "diagram-journey"),
        config: Some(FamilyConfigDefinition {
            namespace: "journey",
            frontmatter_order: 11,
        }),
        variants: [
            Journey => "journey" {
                catalog_order: 23,
                render_kind: Some("journey"),
                metadata: Some(metadata("journey", Some(12))),
                headers: JOURNEY_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    QUADRANT_CHART => "quadrantChart" {
        selector: Some("diagram-quadrant-chart"),
        model_available: cfg!(feature = "diagram-quadrant-chart"),
        config: Some(FamilyConfigDefinition {
            namespace: "quadrantChart",
            frontmatter_order: 16,
        }),
        variants: [
            QuadrantChart => "quadrantChart" {
                catalog_order: 24,
                render_kind: Some("quadrantChart"),
                metadata: Some(metadata("quadrantchart", Some(17))),
                headers: QUADRANT_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    SANKEY => "sankey" {
        selector: Some("diagram-sankey"),
        model_available: cfg!(feature = "diagram-sankey"),
        config: Some(FamilyConfigDefinition {
            namespace: "sankey",
            frontmatter_order: 20,
        }),
        variants: [
            Sankey => "sankey" {
                catalog_order: 25,
                render_kind: Some("sankey"),
                metadata: Some(metadata("sankey", Some(24))),
                headers: SANKEY_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    PACKET => "packet" {
        selector: Some("diagram-packet"),
        model_available: cfg!(feature = "diagram-packet"),
        config: Some(FamilyConfigDefinition {
            namespace: "packet",
            frontmatter_order: 14,
        }),
        variants: [
            Packet => "packet" {
                catalog_order: 26,
                render_kind: Some("packet"),
                metadata: Some(metadata("packet", Some(15))),
                headers: PACKET_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    XY_CHART => "xychart" {
        selector: Some("diagram-xychart"),
        model_available: cfg!(feature = "diagram-xychart"),
        config: Some(FamilyConfigDefinition {
            namespace: "xyChart",
            frontmatter_order: 29,
        }),
        variants: [
            XyChart => "xychart" {
                catalog_order: 27,
                render_kind: Some("xychart"),
                metadata: Some(metadata("xychart", Some(33))),
                headers: XYCHART_HEADERS,
                config_alias_order: Some(5),
            },
        ],
    },
    BLOCK => "block" {
        selector: Some("diagram-block"),
        model_available: cfg!(feature = "diagram-block"),
        config: Some(FamilyConfigDefinition {
            namespace: "block",
            frontmatter_order: 1,
        }),
        variants: [
            Block => "block" {
                catalog_order: 28,
                render_kind: Some("block"),
                metadata: Some(metadata("block", Some(1))),
                headers: BLOCK_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    EVENT_MODELING => "eventmodeling" {
        selector: Some("diagram-event-modeling"),
        model_available: cfg!(feature = "diagram-event-modeling"),
        config: Some(FamilyConfigDefinition {
            namespace: "eventmodeling",
            frontmatter_order: 6,
        }),
        variants: [
            EventModeling => "eventmodeling" {
                catalog_order: 29,
                render_kind: Some("eventmodeling"),
                metadata: Some(metadata("eventmodeling", Some(6))),
                headers: EVENTMODELING_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    TREE_VIEW => "treeView" {
        selector: Some("diagram-tree-view"),
        model_available: cfg!(feature = "diagram-tree-view"),
        config: Some(FamilyConfigDefinition {
            namespace: "treeView",
            frontmatter_order: 25,
        }),
        variants: [
            TreeView => "treeView" {
                catalog_order: 30,
                render_kind: Some("treeView"),
                metadata: Some(metadata("treeView", Some(29))),
                headers: TREE_VIEW_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    RADAR => "radar" {
        selector: Some("diagram-radar"),
        model_available: cfg!(feature = "diagram-radar"),
        config: Some(FamilyConfigDefinition {
            namespace: "radar",
            frontmatter_order: 17,
        }),
        variants: [
            Radar => "radar" {
                catalog_order: 31,
                render_kind: Some("radar"),
                metadata: Some(metadata("radar", Some(18))),
                headers: RADAR_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    ISHIKAWA => "ishikawa" {
        selector: Some("diagram-ishikawa"),
        model_available: cfg!(feature = "diagram-ishikawa"),
        config: Some(FamilyConfigDefinition {
            namespace: "ishikawa",
            frontmatter_order: 10,
        }),
        variants: [
            Ishikawa => "ishikawa" {
                catalog_order: 32,
                render_kind: Some("ishikawa"),
                metadata: Some(metadata("ishikawa", Some(11))),
                headers: ISHIKAWA_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    TREEMAP => "treemap" {
        selector: Some("diagram-treemap"),
        model_available: cfg!(feature = "diagram-treemap"),
        config: Some(FamilyConfigDefinition {
            namespace: "treemap",
            frontmatter_order: 26,
        }),
        variants: [
            Treemap => "treemap" {
                catalog_order: 33,
                render_kind: Some("treemap"),
                metadata: Some(metadata("treemap", Some(30))),
                headers: TREEMAP_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    RAILROAD => "railroad" {
        selector: Some("diagram-railroad"),
        model_available: cfg!(feature = "diagram-railroad"),
        config: Some(FamilyConfigDefinition {
            namespace: "railroad",
            frontmatter_order: 18,
        }),
        variants: [
            Railroad => "railroad" {
                catalog_order: 34,
                render_kind: Some("railroad"),
                metadata: Some(metadata("railroad", Some(19))),
                headers: RAILROAD_HEADERS,
                config_alias_order: None,
            },
            RailroadEbnf => "railroadEbnf" {
                catalog_order: 35,
                render_kind: Some("railroad"),
                metadata: Some(metadata("railroadEbnf", Some(21))),
                headers: RAILROAD_EBNF_HEADERS,
                config_alias_order: None,
            },
            RailroadAbnf => "railroadAbnf" {
                catalog_order: 36,
                render_kind: Some("railroad"),
                metadata: Some(metadata("railroadAbnf", Some(20))),
                headers: RAILROAD_ABNF_HEADERS,
                config_alias_order: None,
            },
            RailroadPeg => "railroadPeg" {
                catalog_order: 37,
                render_kind: Some("railroad"),
                metadata: Some(metadata("railroadPeg", Some(22))),
                headers: RAILROAD_PEG_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    VENN => "venn" {
        selector: Some("diagram-venn"),
        model_available: cfg!(feature = "diagram-venn"),
        config: Some(FamilyConfigDefinition {
            namespace: "venn",
            frontmatter_order: 27,
        }),
        variants: [
            Venn => "venn" {
                catalog_order: 38,
                render_kind: Some("venn"),
                metadata: Some(metadata("venn", Some(31))),
                headers: VENN_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    WARDLEY => "wardley" {
        selector: Some("diagram-wardley"),
        model_available: cfg!(feature = "diagram-wardley"),
        config: Some(FamilyConfigDefinition {
            namespace: "wardley-beta",
            frontmatter_order: 28,
        }),
        variants: [
            Wardley => "wardley" {
                catalog_order: 39,
                render_kind: Some("wardley"),
                metadata: Some(metadata("wardley", Some(32))),
                headers: WARDLEY_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    CYNEFIN => "cynefin" {
        selector: Some("diagram-cynefin"),
        model_available: cfg!(feature = "diagram-cynefin"),
        config: Some(FamilyConfigDefinition {
            namespace: "cynefin",
            frontmatter_order: 4,
        }),
        variants: [
            Cynefin => "cynefin" {
                catalog_order: 40,
                render_kind: Some("cynefin"),
                metadata: Some(metadata("cynefin", Some(4))),
                headers: CYNEFIN_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    USECASE => "usecase" {
        selector: Some("diagram-usecase"),
        model_available: cfg!(feature = "diagram-usecase"),
        config: Some(FamilyConfigDefinition {
            namespace: "usecase",
            frontmatter_order: 35,
        }),
        variants: [
            Usecase => "usecase" {
                catalog_order: 42,
                render_kind: Some("usecase"),
                metadata: Some(metadata("usecase", Some(36))),
                headers: USECASE_HEADERS,
                config_alias_order: None,
            },
        ],
    },
    AGENTFLOW => "agentflow" {
        selector: Some("diagram-agentflow"),
        model_available: cfg!(feature = "diagram-agentflow"),
        config: Some(FamilyConfigDefinition {
            namespace: "agentflow",
            frontmatter_order: 36,
        }),
        variants: [
            Agentflow => "agentflow" {
                catalog_order: 43,
                render_kind: Some("agentflow"),
                metadata: Some(metadata("agentflow", Some(35))),
                headers: AGENTFLOW_HEADERS,
                config_alias_order: Some(36),
            },
        ],
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn pinned_keys_cover_identities_once_and_preserve_wire_spellings() {
        let mut keys = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut orders = BTreeSet::new();
        let families = FAMILY_CATALOG
            .iter()
            .map(|family| family.id)
            .collect::<Vec<_>>();
        assert_eq!(families.as_slice(), DiagramFamilyId::all());
        for family in DiagramFamilyId::all() {
            assert_eq!(DiagramFamilyId::from_id(family.as_str()), Some(*family));
            assert_eq!(serde_json::to_value(family).unwrap(), family.as_str());
        }
        for (family, identity) in diagram_family_identities() {
            assert!(keys.insert(identity.key));
            assert!(names.insert(identity.id));
            assert!(orders.insert(identity.catalog_order));
            assert_eq!(identity.key.as_str(), identity.id);
            assert_eq!(DiagramTypeId::from_id(identity.id), Some(identity.key));
            assert_eq!(serde_json::to_value(identity.key).unwrap(), identity.id);
            assert_eq!(diagram_type_family_id(identity.id), Some(family.id));
        }
        assert_eq!(
            keys,
            DiagramTypeId::all()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(families.len(), 35);
        assert_eq!(keys.len(), 43);
        assert_eq!(diagram_family_selectors().len(), 34);
        assert_eq!(DiagramFamilyId::from_id("future-family"), None);
        assert_eq!(DiagramTypeId::from_id("---"), None);
    }

    #[test]
    fn portable_capabilities_expose_payload_availability_without_source_or_output_claims() {
        for capability in model_family_capabilities() {
            assert_eq!(
                capability.has_model,
                has_diagram_model(capability.diagram_type)
            );
            assert_eq!(
                capability.metadata_id,
                diagram_type_metadata_id(capability.diagram_type)
            );
            assert_eq!(
                capability.render_model_kind,
                diagram_type_render_model_kind(capability.diagram_type)
            );
            let wire = serde_json::to_value(capability).unwrap();
            assert!(wire.get("has_semantic_parser").is_none());
            assert!(wire.get("has_editor_parser").is_none());
            assert!(wire.get("has_render_parser").is_none());
            assert!(wire.get("has_detector").is_none());
        }
        assert!(has_diagram_model("error"));
        assert!(!has_diagram_model("future-family"));
        assert_eq!(diagram_type_metadata_id("gitGraph"), Some("gitgraph"));
        assert_eq!(
            diagram_type_metadata_id("quadrantChart"),
            Some("quadrantchart")
        );
        assert_eq!(
            diagram_type_metadata_id("railroadEbnf"),
            Some("railroadEbnf")
        );
    }

    #[test]
    fn shared_flowchart_data_keeps_logical_identity_and_selector_ownership() {
        assert_eq!(
            diagram_type_family_id("flowchart-v2"),
            Some(DiagramFamilyId::FLOWCHART)
        );
        assert_eq!(
            diagram_type_family_id("swimlane"),
            Some(DiagramFamilyId::SWIMLANE)
        );
        let available = cfg!(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ));
        for diagram_type in ["flowchart", "flowchart-v2", "flowchart-elk", "swimlane"] {
            assert_eq!(has_diagram_model(diagram_type), available);
            assert_eq!(
                diagram_type_render_model_kind(diagram_type),
                Some("flowchart")
            );
        }
        assert_eq!(
            find_variant("flowchart").unwrap().0.selector,
            Some("diagram-flowchart")
        );
        assert_eq!(
            find_variant("swimlane").unwrap().0.selector,
            Some("diagram-swimlane")
        );
        assert_eq!(
            diagram_type_render_model_kind("agentflow"),
            Some("agentflow")
        );
    }
}
