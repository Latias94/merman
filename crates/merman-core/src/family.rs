//! Diagram family facts for the pinned Mermaid baseline.
//!
//! This module owns release-facing Mermaid family facts and projects them into detector,
//! parser, render-model, and metadata surfaces.

use crate::baseline::BaselineRegistryProfile;
use crate::detect::DetectorFn;
use crate::diagram::{DiagramSemanticParser, RenderSemanticModel, RenderSemanticParser};
use crate::{MermaidConfig, ParseMetadata, Result};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub(crate) struct DetectorFact {
    pub(crate) id: &'static str,
    pub(crate) detector: DetectorFn,
}

#[derive(Clone, Copy)]
pub(crate) struct FastDetectKeywordFact {
    pub(crate) keyword: &'static str,
    pub(crate) id: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: DiagramSemanticParser,
    pub(crate) header_policy: HeaderPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeaderPolicy {
    Required,
    Alias(&'static str),
    Internal,
}

#[derive(Clone, Copy)]
pub(crate) struct RenderParserFact {
    pub(crate) id: &'static str,
    pub(crate) metadata_id: Option<&'static str>,
    pub(crate) model_kind: &'static str,
    pub(crate) parser: RenderSemanticParser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SupportedDiagramFact {
    pub(crate) metadata_id: &'static str,
    pub(crate) render_parser_ids: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagramHeaderFact {
    /// Mermaid diagram type id used for profile gating.
    pub diagram_type: &'static str,
    /// Header text suggested to the user.
    pub label: &'static str,
    /// Short description shown in completion details.
    pub detail: &'static str,
    /// Whether this header should only appear in the full baseline profile.
    pub full_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramFamilyCapability {
    /// Mermaid diagram type id used by the pinned detector and parser registries.
    pub diagram_type: &'static str,
    /// Public supported-diagram metadata id, when this family contributes an admitted renderer.
    pub metadata_id: Option<&'static str>,
    /// Whether the selected registry profile has a semantic parser for this diagram type.
    pub has_semantic_parser: bool,
    /// Whether the selected registry profile has a typed render-model parser for this diagram type.
    pub has_render_parser: bool,
}

pub(crate) fn detector_facts(profile: BaselineRegistryProfile) -> &'static [DetectorFact] {
    match profile {
        BaselineRegistryProfile::Tiny => detector_facts_tiny(),
        BaselineRegistryProfile::Full => DETECTOR_FACTS_FULL,
    }
}

pub(crate) fn fast_detect_by_leading_keyword(
    text: &str,
    profile: BaselineRegistryProfile,
) -> Option<&'static str> {
    fn has_boundary(rest: &str) -> bool {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == ';')
    }

    let trimmed = text.trim_start();
    let keywords = match profile {
        BaselineRegistryProfile::Tiny => fast_detect_keyword_facts_tiny(),
        BaselineRegistryProfile::Full => FAST_DETECT_KEYWORDS_FULL,
    };

    keywords.iter().find_map(|fact| {
        trimmed
            .strip_prefix(fact.keyword)
            .and_then(|rest| has_boundary(rest).then_some(fact.id))
    })
}

pub(crate) fn selected_registry_profile() -> BaselineRegistryProfile {
    #[cfg(feature = "full-registry")]
    {
        BaselineRegistryProfile::Full
    }
    #[cfg(not(feature = "full-registry"))]
    {
        BaselineRegistryProfile::Tiny
    }
}

pub(crate) fn semantic_parser_facts(
    profile: BaselineRegistryProfile,
) -> &'static [SemanticParserFact] {
    match profile {
        BaselineRegistryProfile::Tiny => semantic_parser_facts_tiny(),
        BaselineRegistryProfile::Full => SEMANTIC_PARSER_FACTS,
    }
}

pub(crate) fn render_parser_facts(profile: BaselineRegistryProfile) -> &'static [RenderParserFact] {
    match profile {
        BaselineRegistryProfile::Tiny => render_parser_facts_tiny(),
        BaselineRegistryProfile::Full => RENDER_PARSER_FACTS,
    }
}

pub(crate) fn supported_diagram_facts(
    profile: BaselineRegistryProfile,
) -> &'static [SupportedDiagramFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<SupportedDiagramFact> {
        let render_facts = render_parser_facts(profile);
        SUPPORTED_DIAGRAM_METADATA_IDS
            .iter()
            .filter_map(|metadata_id| {
                let render_parser_ids: Vec<_> = render_facts
                    .iter()
                    .filter_map(|fact| (fact.metadata_id == Some(*metadata_id)).then_some(fact.id))
                    .collect();

                (!render_parser_ids.is_empty()).then_some(SupportedDiagramFact {
                    metadata_id,
                    render_parser_ids,
                })
            })
            .collect()
    }

    static TINY_FACTS: OnceLock<Vec<SupportedDiagramFact>> = OnceLock::new();
    static FULL_FACTS: OnceLock<Vec<SupportedDiagramFact>> = OnceLock::new();

    match profile {
        BaselineRegistryProfile::Tiny => TINY_FACTS
            .get_or_init(|| build(BaselineRegistryProfile::Tiny))
            .as_slice(),
        BaselineRegistryProfile::Full => FULL_FACTS
            .get_or_init(|| build(BaselineRegistryProfile::Full))
            .as_slice(),
    }
}

pub(crate) fn supported_diagram_metadata_ids(
    profile: BaselineRegistryProfile,
) -> &'static [&'static str] {
    fn build(profile: BaselineRegistryProfile) -> Vec<&'static str> {
        supported_diagram_facts(profile)
            .iter()
            .inspect(|fact| debug_assert!(!fact.render_parser_ids.is_empty()))
            .map(|fact| fact.metadata_id)
            .collect()
    }

    static TINY_IDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    static FULL_IDS: OnceLock<Vec<&'static str>> = OnceLock::new();

    match profile {
        BaselineRegistryProfile::Tiny => TINY_IDS
            .get_or_init(|| build(BaselineRegistryProfile::Tiny))
            .as_slice(),
        BaselineRegistryProfile::Full => FULL_IDS
            .get_or_init(|| build(BaselineRegistryProfile::Full))
            .as_slice(),
    }
}

pub(crate) fn diagram_header_facts(
    profile: BaselineRegistryProfile,
) -> &'static [DiagramHeaderFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<DiagramHeaderFact> {
        let include_full_only = matches!(profile, BaselineRegistryProfile::Full);
        DIAGRAM_HEADER_FACTS
            .iter()
            .copied()
            .filter(|fact| {
                diagram_type_supported_in_profile(profile, fact.diagram_type)
                    && (!fact.full_only || include_full_only)
                    && semantic_parser_facts(profile).iter().any(|semantic| {
                        semantic.id == fact.diagram_type
                            && semantic.header_policy == HeaderPolicy::Required
                    })
            })
            .collect()
    }

    static TINY_FACTS: OnceLock<Vec<DiagramHeaderFact>> = OnceLock::new();
    static FULL_FACTS: OnceLock<Vec<DiagramHeaderFact>> = OnceLock::new();

    match profile {
        BaselineRegistryProfile::Tiny => TINY_FACTS
            .get_or_init(|| build(BaselineRegistryProfile::Tiny))
            .as_slice(),
        BaselineRegistryProfile::Full => FULL_FACTS
            .get_or_init(|| build(BaselineRegistryProfile::Full))
            .as_slice(),
    }
}

#[cfg(test)]
pub(crate) fn declared_diagram_header_facts() -> &'static [DiagramHeaderFact] {
    DIAGRAM_HEADER_FACTS
}

#[cfg(test)]
pub(crate) fn fast_detect_keyword_facts(
    profile: BaselineRegistryProfile,
) -> &'static [FastDetectKeywordFact] {
    match profile {
        BaselineRegistryProfile::Tiny => fast_detect_keyword_facts_tiny(),
        BaselineRegistryProfile::Full => FAST_DETECT_KEYWORDS_FULL,
    }
}

pub(crate) fn diagram_family_capabilities(
    profile: BaselineRegistryProfile,
) -> &'static [DiagramFamilyCapability] {
    fn build(profile: BaselineRegistryProfile) -> Vec<DiagramFamilyCapability> {
        let detector_facts = detector_facts(profile);
        let semantic_facts = semantic_parser_facts(profile);
        let render_facts = render_parser_facts(profile);

        let mut capabilities: Vec<_> = detector_facts
            .iter()
            .filter(|detector| detector.id != "---")
            .map(|detector| {
                let semantic = semantic_facts
                    .iter()
                    .find(|semantic| semantic.id == detector.id);
                let render = render_facts.iter().find(|render| render.id == detector.id);
                DiagramFamilyCapability {
                    diagram_type: detector.id,
                    metadata_id: render.and_then(|render| render.metadata_id),
                    has_semantic_parser: semantic.is_some(),
                    has_render_parser: render.is_some(),
                }
            })
            .collect();

        for semantic in semantic_facts {
            if capabilities
                .iter()
                .any(|capability| capability.diagram_type == semantic.id)
            {
                continue;
            }
            let render = render_facts.iter().find(|render| render.id == semantic.id);
            capabilities.push(DiagramFamilyCapability {
                diagram_type: semantic.id,
                metadata_id: render.and_then(|render| render.metadata_id),
                has_semantic_parser: true,
                has_render_parser: render.is_some(),
            });
        }

        for render in render_facts {
            if capabilities
                .iter()
                .any(|capability| capability.diagram_type == render.id)
            {
                continue;
            }
            capabilities.push(DiagramFamilyCapability {
                diagram_type: render.id,
                metadata_id: render.metadata_id,
                has_semantic_parser: false,
                has_render_parser: true,
            });
        }

        capabilities
    }

    static TINY_CAPABILITIES: OnceLock<Vec<DiagramFamilyCapability>> = OnceLock::new();
    static FULL_CAPABILITIES: OnceLock<Vec<DiagramFamilyCapability>> = OnceLock::new();

    match profile {
        BaselineRegistryProfile::Tiny => TINY_CAPABILITIES
            .get_or_init(|| build(BaselineRegistryProfile::Tiny))
            .as_slice(),
        BaselineRegistryProfile::Full => FULL_CAPABILITIES
            .get_or_init(|| build(BaselineRegistryProfile::Full))
            .as_slice(),
    }
}

fn semantic_parser_facts_tiny() -> &'static [SemanticParserFact] {
    static FACTS: OnceLock<Vec<SemanticParserFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            SEMANTIC_PARSER_FACTS
                .iter()
                .copied()
                .filter(|fact| {
                    diagram_type_supported_in_profile(BaselineRegistryProfile::Tiny, fact.id)
                })
                .collect()
        })
        .as_slice()
}

fn render_parser_facts_tiny() -> &'static [RenderParserFact] {
    static FACTS: OnceLock<Vec<RenderParserFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            RENDER_PARSER_FACTS
                .iter()
                .copied()
                .filter(|fact| {
                    diagram_type_supported_in_profile(BaselineRegistryProfile::Tiny, fact.id)
                })
                .collect()
        })
        .as_slice()
}

fn detector_facts_tiny() -> &'static [DetectorFact] {
    static FACTS: OnceLock<Vec<DetectorFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            DETECTOR_FACTS_FULL
                .iter()
                .copied()
                .filter(|fact| {
                    diagram_type_supported_in_profile(BaselineRegistryProfile::Tiny, fact.id)
                })
                .collect()
        })
        .as_slice()
}

fn fast_detect_keyword_facts_tiny() -> &'static [FastDetectKeywordFact] {
    static FACTS: OnceLock<Vec<FastDetectKeywordFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            FAST_DETECT_KEYWORDS_FULL
                .iter()
                .copied()
                .filter(|fact| {
                    diagram_type_supported_in_profile(BaselineRegistryProfile::Tiny, fact.id)
                })
                .collect()
        })
        .as_slice()
}

pub(crate) fn diagram_type_supported_in_profile(
    profile: BaselineRegistryProfile,
    diagram_type: &str,
) -> bool {
    match profile {
        BaselineRegistryProfile::Full => true,
        BaselineRegistryProfile::Tiny => {
            !matches!(diagram_type, "architecture" | "flowchart-elk" | "mindmap")
        }
    }
}

pub(crate) fn render_model_kind_supports_diagram_type(
    model_kind: &'static str,
    diagram_type: &str,
) -> bool {
    RENDER_PARSER_FACTS
        .iter()
        .any(|fact| fact.model_kind == model_kind && fact.id == diagram_type)
}

pub fn diagram_type_family_kind(diagram_type: &str) -> Option<&'static str> {
    RENDER_PARSER_FACTS
        .iter()
        .find_map(|fact| (fact.id == diagram_type).then_some(fact.model_kind))
}

pub(crate) fn permits_json_render_fallback(
    profile: BaselineRegistryProfile,
    diagram_type: &str,
) -> bool {
    diagram_type == "error"
        || !semantic_parser_facts(profile)
            .iter()
            .any(|fact| fact.id == diagram_type)
}

pub(crate) fn apply_known_type_detector_side_effects(
    diagram_type: &str,
    effective_config: &mut MermaidConfig,
) {
    if diagram_type == "flowchart-elk" {
        effective_config.set_value("layout", Value::String("elk".to_string()));
        return;
    }

    if matches!(diagram_type, "flowchart-v2" | "flowchart")
        && effective_config.get_str("flowchart.defaultRenderer") == Some("elk")
    {
        effective_config.set_value("layout", Value::String("elk".to_string()));
    }
}

pub(crate) fn apply_diagram_type_config_defaults(
    diagram_type: &str,
    user_config: &MermaidConfig,
    effective_config: &mut MermaidConfig,
) {
    if diagram_type == "swimlane" && user_config.get_str("layout").is_none() {
        effective_config.set_value("layout", Value::String("swimlane".to_string()));
    }
}

const DETECTOR_FACTS_FULL: &[DetectorFact] = &[
    DetectorFact {
        id: "error",
        detector: crate::detect::detector_error,
    },
    DetectorFact {
        id: "---",
        detector: crate::detect::detector_frontmatter_unparsed,
    },
    DetectorFact {
        id: "flowchart-elk",
        detector: crate::detect::detector_flowchart_elk,
    },
    DetectorFact {
        id: "mindmap",
        detector: crate::detect::detector_mindmap,
    },
    DetectorFact {
        id: "architecture",
        detector: crate::detect::detector_architecture,
    },
    DetectorFact {
        id: "zenuml",
        detector: crate::detect::detector_zenuml,
    },
    DetectorFact {
        id: "c4",
        detector: crate::detect::detector_c4,
    },
    DetectorFact {
        id: "kanban",
        detector: crate::detect::detector_kanban,
    },
    DetectorFact {
        id: "classDiagram",
        detector: crate::detect::detector_class_v2,
    },
    DetectorFact {
        id: "class",
        detector: crate::detect::detector_class_dagre_d3,
    },
    DetectorFact {
        id: "er",
        detector: crate::detect::detector_er,
    },
    DetectorFact {
        id: "gantt",
        detector: crate::detect::detector_gantt,
    },
    DetectorFact {
        id: "info",
        detector: crate::detect::detector_info,
    },
    DetectorFact {
        id: "pie",
        detector: crate::detect::detector_pie,
    },
    DetectorFact {
        id: "requirement",
        detector: crate::detect::detector_requirement,
    },
    DetectorFact {
        id: "sequence",
        detector: crate::detect::detector_sequence,
    },
    DetectorFact {
        id: "swimlane",
        detector: crate::detect::detector_swimlane,
    },
    DetectorFact {
        id: "flowchart-v2",
        detector: crate::detect::detector_flowchart_v2,
    },
    DetectorFact {
        id: "flowchart",
        detector: crate::detect::detector_flowchart_dagre_d3_graph,
    },
    DetectorFact {
        id: "timeline",
        detector: crate::detect::detector_timeline,
    },
    DetectorFact {
        id: "gitGraph",
        detector: crate::detect::detector_git_graph,
    },
    DetectorFact {
        id: "stateDiagram",
        detector: crate::detect::detector_state_v2,
    },
    DetectorFact {
        id: "state",
        detector: crate::detect::detector_state_dagre_d3,
    },
    DetectorFact {
        id: "journey",
        detector: crate::detect::detector_journey,
    },
    DetectorFact {
        id: "quadrantChart",
        detector: crate::detect::detector_quadrant,
    },
    DetectorFact {
        id: "sankey",
        detector: crate::detect::detector_sankey,
    },
    DetectorFact {
        id: "packet",
        detector: crate::detect::detector_packet,
    },
    DetectorFact {
        id: "xychart",
        detector: crate::detect::detector_xychart,
    },
    DetectorFact {
        id: "block",
        detector: crate::detect::detector_block,
    },
    DetectorFact {
        id: "eventmodeling",
        detector: crate::detect::detector_eventmodeling,
    },
    DetectorFact {
        id: "treeView",
        detector: crate::detect::detector_tree_view,
    },
    DetectorFact {
        id: "radar",
        detector: crate::detect::detector_radar,
    },
    DetectorFact {
        id: "ishikawa",
        detector: crate::detect::detector_ishikawa,
    },
    DetectorFact {
        id: "treemap",
        detector: crate::detect::detector_treemap,
    },
    DetectorFact {
        id: "railroad",
        detector: crate::detect::detector_railroad,
    },
    DetectorFact {
        id: "railroadEbnf",
        detector: crate::detect::detector_railroad_ebnf,
    },
    DetectorFact {
        id: "railroadAbnf",
        detector: crate::detect::detector_railroad_abnf,
    },
    DetectorFact {
        id: "railroadPeg",
        detector: crate::detect::detector_railroad_peg,
    },
    DetectorFact {
        id: "venn",
        detector: crate::detect::detector_venn,
    },
    DetectorFact {
        id: "wardley",
        detector: crate::detect::detector_wardley,
    },
    DetectorFact {
        id: "cynefin",
        detector: crate::detect::detector_cynefin,
    },
];

const FAST_DETECT_KEYWORDS_FULL: &[FastDetectKeywordFact] = &[
    FastDetectKeywordFact {
        keyword: "sequenceDiagram",
        id: "sequence",
    },
    FastDetectKeywordFact {
        keyword: "mindmap",
        id: "mindmap",
    },
    FastDetectKeywordFact {
        keyword: "architecture",
        id: "architecture",
    },
    FastDetectKeywordFact {
        keyword: "erDiagram",
        id: "er",
    },
    FastDetectKeywordFact {
        keyword: "gantt",
        id: "gantt",
    },
    FastDetectKeywordFact {
        keyword: "timeline",
        id: "timeline",
    },
    FastDetectKeywordFact {
        keyword: "journey",
        id: "journey",
    },
    FastDetectKeywordFact {
        keyword: "gitGraph",
        id: "gitGraph",
    },
    FastDetectKeywordFact {
        keyword: "quadrantChart",
        id: "quadrantChart",
    },
    FastDetectKeywordFact {
        keyword: "packet-beta",
        id: "packet",
    },
    FastDetectKeywordFact {
        keyword: "xychart-beta",
        id: "xychart",
    },
    FastDetectKeywordFact {
        keyword: "treeView-beta",
        id: "treeView",
    },
    FastDetectKeywordFact {
        keyword: "ishikawa-beta",
        id: "ishikawa",
    },
    FastDetectKeywordFact {
        keyword: "ishikawa",
        id: "ishikawa",
    },
    FastDetectKeywordFact {
        keyword: "eventmodeling",
        id: "eventmodeling",
    },
];

const fn semantic_parser_fact(
    id: &'static str,
    parser: DiagramSemanticParser,
) -> SemanticParserFact {
    SemanticParserFact {
        id,
        parser,
        header_policy: HeaderPolicy::Required,
    }
}

const fn semantic_parser_alias_fact(
    id: &'static str,
    canonical_id: &'static str,
    parser: DiagramSemanticParser,
) -> SemanticParserFact {
    SemanticParserFact {
        id,
        parser,
        header_policy: HeaderPolicy::Alias(canonical_id),
    }
}

const fn internal_semantic_parser_fact(
    id: &'static str,
    parser: DiagramSemanticParser,
) -> SemanticParserFact {
    SemanticParserFact {
        id,
        parser,
        header_policy: HeaderPolicy::Internal,
    }
}

const SEMANTIC_PARSER_FACTS: &[SemanticParserFact] = &[
    internal_semantic_parser_fact("error", crate::diagrams::error_diagram::parse_error),
    semantic_parser_fact("flowchart-v2", crate::diagrams::flowchart::parse_flowchart),
    semantic_parser_alias_fact(
        "flowchart",
        "flowchart-v2",
        crate::diagrams::flowchart::parse_flowchart,
    ),
    semantic_parser_fact("flowchart-elk", crate::diagrams::flowchart::parse_flowchart),
    semantic_parser_fact("info", crate::diagrams::info::parse_info),
    semantic_parser_fact("pie", crate::diagrams::pie::parse_pie),
    semantic_parser_fact("c4", crate::diagrams::c4::parse_c4),
    semantic_parser_fact(
        "requirement",
        crate::diagrams::requirement::parse_requirement,
    ),
    semantic_parser_fact("sequence", crate::diagrams::sequence::parse_sequence),
    semantic_parser_fact("swimlane", crate::diagrams::flowchart::parse_flowchart),
    semantic_parser_fact("cynefin", crate::diagrams::cynefin::parse_cynefin),
    semantic_parser_fact("railroad", crate::diagrams::railroad::parse_railroad),
    semantic_parser_fact(
        "railroadEbnf",
        crate::diagrams::railroad::parse_railroad_ebnf,
    ),
    semantic_parser_fact(
        "railroadAbnf",
        crate::diagrams::railroad::parse_railroad_abnf,
    ),
    semantic_parser_fact("railroadPeg", crate::diagrams::railroad::parse_railroad_peg),
    semantic_parser_fact("zenuml", crate::diagrams::zenuml::parse_zenuml),
    semantic_parser_fact("classDiagram", crate::diagrams::class::parse_class),
    semantic_parser_alias_fact("class", "classDiagram", crate::diagrams::class::parse_class),
    semantic_parser_fact("er", crate::diagrams::er::parse_er),
    semantic_parser_alias_fact("erDiagram", "er", crate::diagrams::er::parse_er),
    semantic_parser_fact("stateDiagram", crate::diagrams::state::parse_state),
    semantic_parser_alias_fact("state", "stateDiagram", crate::diagrams::state::parse_state),
    semantic_parser_fact("mindmap", crate::diagrams::mindmap::parse_mindmap),
    semantic_parser_fact("gantt", crate::diagrams::gantt::parse_gantt),
    semantic_parser_fact("timeline", crate::diagrams::timeline::parse_timeline),
    semantic_parser_fact("journey", crate::diagrams::journey::parse_journey),
    semantic_parser_fact("kanban", crate::diagrams::kanban::parse_kanban),
    semantic_parser_fact(
        "architecture",
        crate::diagrams::architecture::parse_architecture,
    ),
    semantic_parser_fact("block", crate::diagrams::block::parse_block),
    semantic_parser_fact("gitGraph", crate::diagrams::git_graph::parse_git_graph),
    semantic_parser_fact(
        "quadrantChart",
        crate::diagrams::quadrant_chart::parse_quadrant_chart,
    ),
    semantic_parser_fact("packet", crate::diagrams::packet::parse_packet),
    semantic_parser_fact("radar", crate::diagrams::radar::parse_radar),
    semantic_parser_fact("treeView", crate::diagrams::tree_view::parse_tree_view),
    semantic_parser_fact("ishikawa", crate::diagrams::ishikawa::parse_ishikawa),
    semantic_parser_fact(
        "eventmodeling",
        crate::diagrams::eventmodeling::parse_eventmodeling,
    ),
    semantic_parser_fact("treemap", crate::diagrams::treemap::parse_treemap),
    semantic_parser_fact("venn", crate::diagrams::venn::parse_venn),
    semantic_parser_fact("sankey", crate::diagrams::sankey::parse_sankey),
    semantic_parser_fact("xychart", crate::diagrams::xychart::parse_xychart),
];

macro_rules! render_parser {
    ($fn_name:ident, $parser:path, $variant:path) => {
        fn $fn_name(code: &str, meta: &ParseMetadata) -> Result<RenderSemanticModel> {
            $parser(code, meta).map($variant)
        }
    };
}

render_parser!(
    render_mindmap,
    crate::diagrams::mindmap::parse_mindmap_model_for_render,
    RenderSemanticModel::Mindmap
);
render_parser!(
    render_state,
    crate::diagrams::state::parse_state_model_for_render,
    RenderSemanticModel::State
);
render_parser!(
    render_zenuml,
    crate::diagrams::zenuml::parse_zenuml_model_for_render,
    RenderSemanticModel::Sequence
);
render_parser!(
    render_sequence,
    crate::diagrams::sequence::parse_sequence_model_for_render,
    RenderSemanticModel::Sequence
);
render_parser!(
    render_flowchart,
    crate::diagrams::flowchart::parse_flowchart_model_for_render,
    RenderSemanticModel::Flowchart
);
render_parser!(
    render_class,
    crate::diagrams::class::parse_class_typed,
    RenderSemanticModel::Class
);
render_parser!(
    render_c4,
    crate::diagrams::c4::parse_c4_model_for_render,
    RenderSemanticModel::C4
);
render_parser!(
    render_cynefin,
    crate::diagrams::cynefin::parse_cynefin_model_for_render,
    RenderSemanticModel::Cynefin
);
render_parser!(
    render_railroad,
    crate::diagrams::railroad::parse_railroad_model_for_render,
    RenderSemanticModel::Railroad
);
render_parser!(
    render_railroad_ebnf,
    crate::diagrams::railroad::parse_railroad_ebnf_model_for_render,
    RenderSemanticModel::Railroad
);
render_parser!(
    render_railroad_abnf,
    crate::diagrams::railroad::parse_railroad_abnf_model_for_render,
    RenderSemanticModel::Railroad
);
render_parser!(
    render_railroad_peg,
    crate::diagrams::railroad::parse_railroad_peg_model_for_render,
    RenderSemanticModel::Railroad
);
render_parser!(
    render_architecture,
    crate::diagrams::architecture::parse_architecture_model_for_render,
    RenderSemanticModel::Architecture
);
render_parser!(
    render_kanban,
    crate::diagrams::kanban::parse_kanban_model_for_render,
    RenderSemanticModel::Kanban
);
render_parser!(
    render_gantt,
    crate::diagrams::gantt::parse_gantt_model_for_render,
    RenderSemanticModel::Gantt
);
render_parser!(
    render_pie,
    crate::diagrams::pie::parse_pie_model_for_render,
    RenderSemanticModel::Pie
);
render_parser!(
    render_packet,
    crate::diagrams::packet::parse_packet_model_for_render,
    RenderSemanticModel::Packet
);
render_parser!(
    render_timeline,
    crate::diagrams::timeline::parse_timeline_model_for_render,
    RenderSemanticModel::Timeline
);
render_parser!(
    render_journey,
    crate::diagrams::journey::parse_journey_model_for_render,
    RenderSemanticModel::Journey
);
render_parser!(
    render_requirement,
    crate::diagrams::requirement::parse_requirement_model_for_render,
    RenderSemanticModel::Requirement
);
render_parser!(
    render_sankey,
    crate::diagrams::sankey::parse_sankey_model_for_render,
    RenderSemanticModel::Sankey
);
render_parser!(
    render_radar,
    crate::diagrams::radar::parse_radar_model_for_render,
    RenderSemanticModel::Radar
);
render_parser!(
    render_info,
    crate::diagrams::info::parse_info_model_for_render,
    RenderSemanticModel::Info
);
render_parser!(
    render_treemap,
    crate::diagrams::treemap::parse_treemap_model_for_render,
    RenderSemanticModel::Treemap
);
render_parser!(
    render_block,
    crate::diagrams::block::parse_block_model_for_render,
    RenderSemanticModel::Block
);
render_parser!(
    render_er,
    crate::diagrams::er::parse_er_model_for_render,
    RenderSemanticModel::Er
);
render_parser!(
    render_quadrant_chart,
    crate::diagrams::quadrant_chart::parse_quadrant_chart_model_for_render,
    RenderSemanticModel::QuadrantChart
);
render_parser!(
    render_xychart,
    crate::diagrams::xychart::parse_xychart_model_for_render,
    RenderSemanticModel::XyChart
);
render_parser!(
    render_git_graph,
    crate::diagrams::git_graph::parse_git_graph_model_for_render,
    RenderSemanticModel::GitGraph
);
render_parser!(
    render_tree_view,
    crate::diagrams::tree_view::parse_tree_view_model_for_render,
    RenderSemanticModel::TreeView
);
render_parser!(
    render_ishikawa,
    crate::diagrams::ishikawa::parse_ishikawa_model_for_render,
    RenderSemanticModel::Ishikawa
);
render_parser!(
    render_eventmodeling,
    crate::diagrams::eventmodeling::parse_eventmodeling_model_for_render,
    RenderSemanticModel::EventModeling
);
render_parser!(
    render_venn,
    crate::diagrams::venn::parse_venn_model_for_render,
    RenderSemanticModel::Venn
);

const RENDER_PARSER_FACTS: &[RenderParserFact] = &[
    RenderParserFact {
        id: "mindmap",
        metadata_id: Some("mindmap"),
        model_kind: "mindmap",
        parser: render_mindmap,
    },
    RenderParserFact {
        id: "stateDiagram",
        metadata_id: Some("state"),
        model_kind: "state",
        parser: render_state,
    },
    RenderParserFact {
        id: "state",
        metadata_id: Some("state"),
        model_kind: "state",
        parser: render_state,
    },
    RenderParserFact {
        id: "zenuml",
        metadata_id: Some("zenuml"),
        model_kind: "sequence",
        parser: render_zenuml,
    },
    RenderParserFact {
        id: "sequence",
        metadata_id: Some("sequence"),
        model_kind: "sequence",
        parser: render_sequence,
    },
    RenderParserFact {
        id: "flowchart-v2",
        metadata_id: Some("flowchart"),
        model_kind: "flowchart",
        parser: render_flowchart,
    },
    RenderParserFact {
        id: "flowchart",
        metadata_id: Some("flowchart"),
        model_kind: "flowchart",
        parser: render_flowchart,
    },
    RenderParserFact {
        id: "flowchart-elk",
        metadata_id: Some("flowchart"),
        model_kind: "flowchart",
        parser: render_flowchart,
    },
    RenderParserFact {
        id: "classDiagram",
        metadata_id: Some("class"),
        model_kind: "class",
        parser: render_class,
    },
    RenderParserFact {
        id: "class",
        metadata_id: Some("class"),
        model_kind: "class",
        parser: render_class,
    },
    RenderParserFact {
        id: "c4",
        metadata_id: Some("c4"),
        model_kind: "c4",
        parser: render_c4,
    },
    RenderParserFact {
        id: "cynefin",
        metadata_id: Some("cynefin"),
        model_kind: "cynefin",
        parser: render_cynefin,
    },
    RenderParserFact {
        id: "railroad",
        metadata_id: Some("railroad"),
        model_kind: "railroad",
        parser: render_railroad,
    },
    RenderParserFact {
        id: "railroadEbnf",
        metadata_id: Some("railroadEbnf"),
        model_kind: "railroad",
        parser: render_railroad_ebnf,
    },
    RenderParserFact {
        id: "railroadAbnf",
        metadata_id: Some("railroadAbnf"),
        model_kind: "railroad",
        parser: render_railroad_abnf,
    },
    RenderParserFact {
        id: "railroadPeg",
        metadata_id: Some("railroadPeg"),
        model_kind: "railroad",
        parser: render_railroad_peg,
    },
    RenderParserFact {
        id: "architecture",
        metadata_id: Some("architecture"),
        model_kind: "architecture",
        parser: render_architecture,
    },
    RenderParserFact {
        id: "kanban",
        metadata_id: Some("kanban"),
        model_kind: "kanban",
        parser: render_kanban,
    },
    RenderParserFact {
        id: "gantt",
        metadata_id: Some("gantt"),
        model_kind: "gantt",
        parser: render_gantt,
    },
    RenderParserFact {
        id: "pie",
        metadata_id: Some("pie"),
        model_kind: "pie",
        parser: render_pie,
    },
    RenderParserFact {
        id: "packet",
        metadata_id: Some("packet"),
        model_kind: "packet",
        parser: render_packet,
    },
    RenderParserFact {
        id: "timeline",
        metadata_id: Some("timeline"),
        model_kind: "timeline",
        parser: render_timeline,
    },
    RenderParserFact {
        id: "journey",
        metadata_id: Some("journey"),
        model_kind: "journey",
        parser: render_journey,
    },
    RenderParserFact {
        id: "requirement",
        metadata_id: Some("requirement"),
        model_kind: "requirement",
        parser: render_requirement,
    },
    RenderParserFact {
        id: "sankey",
        metadata_id: Some("sankey"),
        model_kind: "sankey",
        parser: render_sankey,
    },
    RenderParserFact {
        id: "radar",
        metadata_id: Some("radar"),
        model_kind: "radar",
        parser: render_radar,
    },
    RenderParserFact {
        id: "info",
        metadata_id: Some("info"),
        model_kind: "info",
        parser: render_info,
    },
    RenderParserFact {
        id: "treemap",
        metadata_id: Some("treemap"),
        model_kind: "treemap",
        parser: render_treemap,
    },
    RenderParserFact {
        id: "block",
        metadata_id: Some("block"),
        model_kind: "block",
        parser: render_block,
    },
    RenderParserFact {
        id: "er",
        metadata_id: Some("er"),
        model_kind: "er",
        parser: render_er,
    },
    RenderParserFact {
        id: "erDiagram",
        metadata_id: Some("er"),
        model_kind: "er",
        parser: render_er,
    },
    RenderParserFact {
        id: "quadrantChart",
        metadata_id: Some("quadrantchart"),
        model_kind: "quadrantChart",
        parser: render_quadrant_chart,
    },
    RenderParserFact {
        id: "xychart",
        metadata_id: Some("xychart"),
        model_kind: "xychart",
        parser: render_xychart,
    },
    RenderParserFact {
        id: "gitGraph",
        metadata_id: Some("gitgraph"),
        model_kind: "gitGraph",
        parser: render_git_graph,
    },
    RenderParserFact {
        id: "treeView",
        metadata_id: None,
        model_kind: "treeView",
        parser: render_tree_view,
    },
    RenderParserFact {
        id: "ishikawa",
        metadata_id: None,
        model_kind: "ishikawa",
        parser: render_ishikawa,
    },
    RenderParserFact {
        id: "eventmodeling",
        metadata_id: None,
        model_kind: "eventmodeling",
        parser: render_eventmodeling,
    },
    RenderParserFact {
        id: "venn",
        metadata_id: Some("venn"),
        model_kind: "venn",
        parser: render_venn,
    },
];

const SUPPORTED_DIAGRAM_METADATA_IDS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "cynefin",
    "er",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "railroad",
    "railroadAbnf",
    "railroadEbnf",
    "railroadPeg",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treemap",
    "venn",
    "xychart",
    "zenuml",
];

const DIAGRAM_HEADER_FACTS: &[DiagramHeaderFact] = &[
    DiagramHeaderFact {
        diagram_type: "flowchart-v2",
        label: "flowchart TD",
        detail: "flowchart header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "flowchart-v2",
        label: "graph TD",
        detail: "flowchart alias",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "sequence",
        label: "sequenceDiagram",
        detail: "sequence header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "swimlane",
        label: "swimlane-beta",
        detail: "swimlane header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "classDiagram",
        label: "classDiagram",
        detail: "class header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "classDiagram",
        label: "classDiagram-v2",
        detail: "class header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "stateDiagram",
        label: "stateDiagram-v2",
        detail: "state header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "stateDiagram",
        label: "stateDiagram",
        detail: "legacy state header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "er",
        label: "erDiagram",
        detail: "er header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "gantt",
        label: "gantt",
        detail: "gantt header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "mindmap",
        label: "mindmap",
        detail: "mindmap header",
        full_only: true,
    },
    DiagramHeaderFact {
        diagram_type: "info",
        label: "info",
        detail: "info header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "journey",
        label: "journey",
        detail: "journey header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "timeline",
        label: "timeline",
        detail: "timeline header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "gitGraph",
        label: "gitGraph",
        detail: "git graph header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "pie",
        label: "pie",
        detail: "pie header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "requirement",
        label: "requirementDiagram",
        detail: "requirement header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "sankey",
        label: "sankey",
        detail: "sankey header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "packet",
        label: "packet",
        detail: "packet header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "packet",
        label: "packet-beta",
        detail: "packet beta header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "xychart",
        label: "xychart",
        detail: "xychart header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "xychart",
        label: "xychart-beta",
        detail: "xychart beta header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "treeView",
        label: "treeView-beta",
        detail: "tree view header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "ishikawa",
        label: "ishikawa-beta",
        detail: "ishikawa header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "eventmodeling",
        label: "eventmodeling",
        detail: "event modeling header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "quadrantChart",
        label: "quadrantChart",
        detail: "quadrant chart header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "venn",
        label: "venn-beta",
        detail: "venn header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "zenuml",
        label: "zenuml",
        detail: "zenuml header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "c4",
        label: "C4Context",
        detail: "c4 context header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "c4",
        label: "C4Container",
        detail: "c4 container header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "c4",
        label: "C4Component",
        detail: "c4 component header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "c4",
        label: "C4Dynamic",
        detail: "c4 dynamic header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "c4",
        label: "C4Deployment",
        detail: "c4 deployment header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "kanban",
        label: "kanban",
        detail: "kanban header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "architecture",
        label: "architecture-beta",
        detail: "architecture header",
        full_only: true,
    },
    DiagramHeaderFact {
        diagram_type: "block",
        label: "block-beta",
        detail: "block header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "radar",
        label: "radar-beta",
        detail: "radar header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "treemap",
        label: "treemap-beta",
        detail: "treemap header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "railroad",
        label: "railroad-beta",
        detail: "railroad header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "railroadEbnf",
        label: "railroad-ebnf-beta",
        detail: "railroad ebnf header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "railroadAbnf",
        label: "railroad-abnf-beta",
        detail: "railroad abnf header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "railroadPeg",
        label: "railroad-peg-beta",
        detail: "railroad peg header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "cynefin",
        label: "cynefin-beta",
        detail: "cynefin header",
        full_only: false,
    },
    DiagramHeaderFact {
        diagram_type: "flowchart-elk",
        label: "flowchart-elk TD",
        detail: "elk flowchart header",
        full_only: true,
    },
];
