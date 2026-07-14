//! Diagram family facts for the pinned Mermaid baseline.
//!
//! This module owns release-facing Mermaid family facts and projects them into detector,
//! parser, render-model, and metadata surfaces.

use crate::baseline::BaselineRegistryProfile;
use crate::detect::DetectorFn;
use crate::diagram::{DiagramSemanticParser, RenderSemanticModel, RenderSemanticParser};
use crate::{EditorSemanticFacts, MermaidConfig, ParseMetadata, Result};
use serde_json::Value;
use std::sync::OnceLock;

pub(crate) type EditorSemanticParser =
    fn(code: &str, meta: &ParseMetadata) -> Result<EditorSemanticFacts>;
pub(crate) type CombinedSemanticParser =
    fn(code: &str, meta: &ParseMetadata) -> Result<(Value, EditorSemanticFacts)>;

#[derive(Clone, Copy)]
pub(crate) struct DetectorFact {
    pub(crate) id: &'static str,
    pub(crate) detector: DetectorFn,
}

#[derive(Clone, Copy)]
pub(crate) struct FastDetectKeywordFact {
    keyword: &'static str,
    id: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: DiagramSemanticParser,
}

#[derive(Clone, Copy)]
pub(crate) struct RenderParserFact {
    pub(crate) id: &'static str,
    pub(crate) metadata_id: Option<&'static str>,
    pub(crate) model_kind: &'static str,
    pub(crate) parser: RenderSemanticParser,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: EditorSemanticParser,
}

#[derive(Clone, Copy)]
pub(crate) struct CombinedParserFact {
    pub(crate) id: &'static str,
    pub(crate) parser: CombinedSemanticParser,
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
    /// Logical diagram family. This does not change when a family reuses another render model.
    pub logical_family_kind: &'static str,
    /// Public supported-diagram metadata id, when this family contributes an admitted renderer.
    pub metadata_id: Option<&'static str>,
    /// Typed render-model kind, when this id owns a typed render projection.
    pub render_model_kind: Option<&'static str>,
    /// Whether this id participates in automatic detection.
    pub has_detector: bool,
    /// Whether the selected registry profile has a semantic parser for this diagram type.
    pub has_semantic_parser: bool,
    /// Whether the selected registry profile has parser-backed editor facts.
    pub has_editor_parser: bool,
    /// Whether JSON and editor facts share one combined semantic construction.
    pub has_combined_parser: bool,
    /// Whether the selected registry profile has a typed render-model parser for this diagram type.
    pub has_render_parser: bool,
    /// Whether this id contributes at least one authoring header.
    pub has_header: bool,
    /// Mermaid configuration namespace associated with this id.
    pub config_namespace: Option<&'static str>,
}

pub(crate) fn detector_facts(profile: BaselineRegistryProfile) -> &'static [DetectorFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<DetectorFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .filter_map(|(_, variant)| {
                variant.detector.map(|ordered| {
                    (
                        ordered.order,
                        DetectorFact {
                            id: variant.id,
                            detector: ordered.value,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<DetectorFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<DetectorFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
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
    let keywords = fast_detect_keyword_facts(profile);

    keywords.iter().find_map(|fact| {
        trimmed
            .strip_prefix(fact.keyword)
            .and_then(|rest| has_boundary(rest).then_some(fact.id))
    })
}

pub(crate) fn selected_registry_profile() -> BaselineRegistryProfile {
    #[cfg(feature = "full")]
    {
        BaselineRegistryProfile::Full
    }
    #[cfg(not(feature = "full"))]
    {
        BaselineRegistryProfile::Tiny
    }
}

pub(crate) fn semantic_parser_facts(
    profile: BaselineRegistryProfile,
) -> &'static [SemanticParserFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<SemanticParserFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .filter_map(|(_, variant)| {
                variant.semantic.map(|ordered| {
                    (
                        ordered.order,
                        SemanticParserFact {
                            id: variant.id,
                            parser: ordered.value,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<SemanticParserFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<SemanticParserFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
    }
}

pub(crate) fn render_parser_facts(profile: BaselineRegistryProfile) -> &'static [RenderParserFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<RenderParserFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .filter_map(|(_, variant)| {
                variant.typed_render.map(|ordered| {
                    (
                        ordered.order,
                        RenderParserFact {
                            id: variant.id,
                            metadata_id: variant.metadata.map(|metadata| metadata.id),
                            model_kind: variant
                                .render_model_kind
                                .expect("typed render variants declare their model kind"),
                            parser: ordered.value,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<RenderParserFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<RenderParserFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
    }
}

pub(crate) fn editor_parser_facts(profile: BaselineRegistryProfile) -> &'static [EditorParserFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<EditorParserFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .filter_map(|(_, variant)| {
                variant.editor.map(|ordered| {
                    (
                        ordered.order,
                        EditorParserFact {
                            id: variant.id,
                            parser: ordered.value,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<EditorParserFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<EditorParserFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
    }
}

pub(crate) fn combined_parser_facts(
    profile: BaselineRegistryProfile,
) -> &'static [CombinedParserFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<CombinedParserFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .filter_map(|(_, variant)| {
                variant.combined.map(|ordered| {
                    (
                        ordered.order,
                        CombinedParserFact {
                            id: variant.id,
                            parser: ordered.value,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<CombinedParserFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<CombinedParserFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
    }
}

pub(crate) fn editor_parser(
    profile: BaselineRegistryProfile,
    diagram_type: &str,
) -> Option<EditorSemanticParser> {
    editor_parser_facts(profile)
        .iter()
        .find_map(|fact| (fact.id == diagram_type).then_some(fact.parser))
}

pub(crate) fn combined_parser(
    profile: BaselineRegistryProfile,
    diagram_type: &str,
) -> Option<CombinedSemanticParser> {
    combined_parser_facts(profile)
        .iter()
        .find_map(|fact| (fact.id == diagram_type).then_some(fact.parser))
}

pub(crate) fn supported_diagram_facts(
    profile: BaselineRegistryProfile,
) -> &'static [SupportedDiagramFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<SupportedDiagramFact> {
        let variants: Vec<_> = variants_for_profile(profile).collect();
        let render_facts = render_parser_facts(profile);
        let mut metadata: Vec<_> = variants
            .iter()
            .filter_map(|(_, variant)| {
                variant
                    .metadata
                    .and_then(|metadata| metadata.order.map(|order| (order, metadata.id)))
            })
            .collect();
        metadata.sort_by_key(|(order, _)| *order);
        metadata
            .into_iter()
            .filter_map(|(_, metadata_id)| {
                let render_parser_ids: Vec<_> = render_facts
                    .iter()
                    .filter_map(|fact| (fact.metadata_id == Some(metadata_id)).then_some(fact.id))
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
        let mut facts: Vec<_> = variants_for_profile(profile)
            .flat_map(|(_, variant)| {
                variant.headers.iter().map(move |header| {
                    (
                        header.order,
                        DiagramHeaderFact {
                            diagram_type: variant.id,
                            label: header.label,
                            detail: header.detail,
                            full_only: variant.profile == VariantProfile::FullOnly,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
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

pub(crate) fn diagram_family_capabilities(
    profile: BaselineRegistryProfile,
) -> &'static [DiagramFamilyCapability] {
    fn build(profile: BaselineRegistryProfile) -> Vec<DiagramFamilyCapability> {
        let mut capabilities: Vec<_> = variants_for_profile(profile)
            .map(|(family, variant)| {
                (
                    variant.catalog_order,
                    DiagramFamilyCapability {
                        diagram_type: variant.id,
                        logical_family_kind: family.logical_kind,
                        metadata_id: variant.metadata.map(|metadata| metadata.id),
                        render_model_kind: variant.render_model_kind,
                        has_detector: variant.detector.is_some(),
                        has_semantic_parser: variant.semantic.is_some(),
                        has_editor_parser: variant.editor.is_some(),
                        has_combined_parser: variant.combined.is_some(),
                        has_render_parser: variant.typed_render.is_some(),
                        has_header: !variant.headers.is_empty(),
                        config_namespace: family.config.map(|config| config.namespace),
                    },
                )
            })
            .collect();
        capabilities.sort_by_key(|(order, _)| *order);
        capabilities.into_iter().map(|(_, fact)| fact).collect()
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

fn fast_detect_keyword_facts(profile: BaselineRegistryProfile) -> &'static [FastDetectKeywordFact] {
    fn build(profile: BaselineRegistryProfile) -> Vec<FastDetectKeywordFact> {
        let mut facts: Vec<_> = variants_for_profile(profile)
            .flat_map(|(_, variant)| {
                variant.fast_keywords.iter().map(move |keyword| {
                    (
                        keyword.order,
                        FastDetectKeywordFact {
                            keyword: keyword.keyword,
                            id: variant.id,
                        },
                    )
                })
            })
            .collect();
        facts.sort_by_key(|(order, _)| *order);
        facts.into_iter().map(|(_, fact)| fact).collect()
    }

    static TINY: OnceLock<Vec<FastDetectKeywordFact>> = OnceLock::new();
    static FULL: OnceLock<Vec<FastDetectKeywordFact>> = OnceLock::new();
    match profile {
        BaselineRegistryProfile::Tiny => TINY.get_or_init(|| build(profile)).as_slice(),
        BaselineRegistryProfile::Full => FULL.get_or_init(|| build(profile)).as_slice(),
    }
}

pub(crate) fn diagram_type_supported_in_profile(
    profile: BaselineRegistryProfile,
    diagram_type: &str,
) -> bool {
    find_variant(diagram_type).is_none_or(|(_, variant)| variant.profile.includes(profile))
}

pub(crate) fn is_builtin_diagram_type(diagram_type: &str) -> bool {
    find_variant(diagram_type).is_some()
}

pub(crate) fn render_model_kind_supports_diagram_type(
    model_kind: &'static str,
    diagram_type: &str,
) -> bool {
    render_parser_facts(BaselineRegistryProfile::Full)
        .iter()
        .any(|fact| fact.id == diagram_type && fact.model_kind == model_kind)
}

pub fn diagram_type_family_kind(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).map(|(family, _)| family.logical_kind)
}

pub fn diagram_type_render_model_kind(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).and_then(|(_, variant)| variant.render_model_kind)
}

pub(crate) fn apply_known_type_detector_side_effects(
    diagram_type: &str,
    effective_config: &mut MermaidConfig,
) {
    let effect = find_variant(diagram_type)
        .map(|(_, variant)| variant.known_type_effect)
        .unwrap_or(KnownTypeEffect::None);
    match effect {
        KnownTypeEffect::None => {}
        KnownTypeEffect::ForceElk => {
            effective_config.set_value("layout", Value::String("elk".to_string()));
        }
        KnownTypeEffect::FlowchartConfiguredRenderer => {
            if effective_config.get_str("flowchart.defaultRenderer") == Some("elk") {
                effective_config.set_value("layout", Value::String("elk".to_string()));
            }
        }
    }
}

pub(crate) fn apply_diagram_type_config_defaults(
    diagram_type: &str,
    user_config: &MermaidConfig,
    effective_config: &mut MermaidConfig,
) {
    let effect = find_variant(diagram_type)
        .map(|(_, variant)| variant.default_effect)
        .unwrap_or(DefaultEffect::None);
    match effect {
        DefaultEffect::None => {}
        DefaultEffect::SwimlaneLayout if user_config.get_str("layout").is_none() => {
            effective_config.set_value("layout", Value::String("swimlane".to_string()));
        }
        DefaultEffect::SwimlaneLayout => {}
    }
}

macro_rules! infallible_editor_adapter {
    ($name:ident, $parser:path) => {
        fn $name(code: &str, meta: &ParseMetadata) -> Result<EditorSemanticFacts> {
            Ok($parser(code, meta))
        }
    };
}

infallible_editor_adapter!(
    editor_sequence,
    crate::diagrams::sequence::parse_sequence_editor_facts
);
infallible_editor_adapter!(
    editor_state,
    crate::diagrams::state::parse_state_editor_facts
);
infallible_editor_adapter!(
    editor_class,
    crate::diagrams::class::parse_class_editor_facts
);
infallible_editor_adapter!(editor_er, crate::diagrams::er::parse_er_editor_facts);
infallible_editor_adapter!(
    editor_mindmap,
    crate::diagrams::mindmap::parse_mindmap_editor_facts
);
infallible_editor_adapter!(
    editor_gantt,
    crate::diagrams::gantt::parse_gantt_editor_facts
);
infallible_editor_adapter!(
    editor_architecture,
    crate::diagrams::architecture::parse_architecture_editor_facts
);
infallible_editor_adapter!(
    editor_block,
    crate::diagrams::block::parse_block_editor_facts
);
infallible_editor_adapter!(editor_c4, crate::diagrams::c4::parse_c4_editor_facts);
infallible_editor_adapter!(
    editor_cynefin,
    crate::diagrams::cynefin::parse_cynefin_editor_facts
);
infallible_editor_adapter!(
    editor_git_graph,
    crate::diagrams::git_graph::parse_git_graph_editor_facts
);
infallible_editor_adapter!(
    editor_kanban,
    crate::diagrams::kanban::parse_kanban_editor_facts
);
infallible_editor_adapter!(
    editor_ishikawa,
    crate::diagrams::ishikawa::parse_ishikawa_editor_facts
);
infallible_editor_adapter!(
    editor_journey,
    crate::diagrams::journey::parse_journey_editor_facts
);
infallible_editor_adapter!(editor_info, crate::diagrams::info::parse_info_editor_facts);
infallible_editor_adapter!(
    editor_timeline,
    crate::diagrams::timeline::parse_timeline_editor_facts
);
infallible_editor_adapter!(editor_pie, crate::diagrams::pie::parse_pie_editor_facts);
infallible_editor_adapter!(
    editor_packet,
    crate::diagrams::packet::parse_packet_editor_facts
);
infallible_editor_adapter!(
    editor_sankey,
    crate::diagrams::sankey::parse_sankey_editor_facts
);
infallible_editor_adapter!(
    editor_tree_view,
    crate::diagrams::tree_view::parse_tree_view_editor_facts
);
infallible_editor_adapter!(
    editor_eventmodeling,
    crate::diagrams::eventmodeling::parse_eventmodeling_editor_facts
);
infallible_editor_adapter!(
    editor_quadrant_chart,
    crate::diagrams::quadrant_chart::parse_quadrant_chart_editor_facts
);
infallible_editor_adapter!(
    editor_railroad,
    crate::diagrams::railroad::parse_railroad_editor_facts
);
infallible_editor_adapter!(
    editor_railroad_ebnf,
    crate::diagrams::railroad::parse_railroad_ebnf_editor_facts
);
infallible_editor_adapter!(
    editor_railroad_abnf,
    crate::diagrams::railroad::parse_railroad_abnf_editor_facts
);
infallible_editor_adapter!(
    editor_railroad_peg,
    crate::diagrams::railroad::parse_railroad_peg_editor_facts
);
infallible_editor_adapter!(
    editor_radar,
    crate::diagrams::radar::parse_radar_editor_facts
);
infallible_editor_adapter!(
    editor_treemap,
    crate::diagrams::treemap::parse_treemap_editor_facts
);
infallible_editor_adapter!(
    editor_requirement,
    crate::diagrams::requirement::parse_requirement_editor_facts
);
infallible_editor_adapter!(editor_venn, crate::diagrams::venn::parse_venn_editor_facts);
infallible_editor_adapter!(
    editor_xychart,
    crate::diagrams::xychart::parse_xychart_editor_facts
);
infallible_editor_adapter!(
    editor_zenuml,
    crate::diagrams::zenuml::parse_zenuml_editor_facts
);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantProfile {
    All,
    FullOnly,
}

impl VariantProfile {
    fn includes(self, profile: BaselineRegistryProfile) -> bool {
        self == Self::All || profile == BaselineRegistryProfile::Full
    }
}

#[derive(Clone, Copy)]
struct Ordered<T> {
    order: u16,
    value: T,
}

const fn ordered<T>(order: u16, value: T) -> Ordered<T> {
    Ordered { order, value }
}

#[derive(Clone, Copy)]
struct FastKeywordDefinition {
    order: u16,
    keyword: &'static str,
}

const fn fast_keyword(order: u16, keyword: &'static str) -> FastKeywordDefinition {
    FastKeywordDefinition { order, keyword }
}

#[derive(Clone, Copy)]
struct MetadataDefinition {
    id: &'static str,
    order: Option<u16>,
}

const fn metadata(id: &'static str, order: Option<u16>) -> MetadataDefinition {
    MetadataDefinition { id, order }
}

#[derive(Clone, Copy)]
struct HeaderDefinition {
    order: u16,
    label: &'static str,
    detail: &'static str,
}

const fn header(order: u16, label: &'static str, detail: &'static str) -> HeaderDefinition {
    HeaderDefinition {
        order,
        label,
        detail,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownTypeEffect {
    None,
    ForceElk,
    FlowchartConfiguredRenderer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultEffect {
    None,
    SwimlaneLayout,
}

#[derive(Clone, Copy)]
struct FamilyVariantDefinition {
    id: &'static str,
    profile: VariantProfile,
    catalog_order: u16,
    detector: Option<Ordered<DetectorFn>>,
    fast_keywords: &'static [FastKeywordDefinition],
    semantic: Option<Ordered<DiagramSemanticParser>>,
    editor: Option<Ordered<EditorSemanticParser>>,
    combined: Option<Ordered<CombinedSemanticParser>>,
    typed_render: Option<Ordered<RenderSemanticParser>>,
    render_model_kind: Option<&'static str>,
    metadata: Option<MetadataDefinition>,
    headers: &'static [HeaderDefinition],
    #[cfg_attr(not(feature = "full-config"), allow(dead_code))]
    frontmatter_alias_order: Option<u16>,
    known_type_effect: KnownTypeEffect,
    default_effect: DefaultEffect,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "full-config"), allow(dead_code))]
struct FamilyConfigDefinition {
    namespace: &'static str,
    frontmatter_order: u16,
}

#[derive(Clone, Copy)]
struct DiagramFamilyDefinition {
    logical_kind: &'static str,
    #[cfg_attr(not(feature = "full-config"), allow(dead_code))]
    config: Option<FamilyConfigDefinition>,
    variants: &'static [FamilyVariantDefinition],
}

macro_rules! variant {
    (
        id: $id:literal,
        profile: $profile:expr,
        catalog_order: $catalog_order:literal,
        detector: $detector:expr,
        fast: $fast:expr,
        semantic: $semantic:expr,
        editor: $editor:expr,
        combined: $combined:expr,
        typed: $typed:expr,
        render_kind: $render_kind:expr,
        metadata: $metadata:expr,
        headers: $headers:expr,
        config_alias_order: $config_alias_order:expr,
        known_effect: $known_effect:expr,
        default_effect: $default_effect:expr $(,)?
    ) => {
        FamilyVariantDefinition {
            id: $id,
            profile: $profile,
            catalog_order: $catalog_order,
            detector: $detector,
            fast_keywords: $fast,
            semantic: $semantic,
            editor: $editor,
            combined: $combined,
            typed_render: $typed,
            render_model_kind: $render_kind,
            metadata: $metadata,
            headers: $headers,
            frontmatter_alias_order: $config_alias_order,
            known_type_effect: $known_effect,
            default_effect: $default_effect,
        }
    };
}

fn variants_for_profile(
    profile: BaselineRegistryProfile,
) -> impl Iterator<
    Item = (
        &'static DiagramFamilyDefinition,
        &'static FamilyVariantDefinition,
    ),
> {
    FAMILY_CATALOG.iter().flat_map(move |family| {
        family
            .variants
            .iter()
            .filter(move |variant| variant.profile.includes(profile))
            .map(move |variant| (family, variant))
    })
}

fn find_variant(
    diagram_type: &str,
) -> Option<(
    &'static DiagramFamilyDefinition,
    &'static FamilyVariantDefinition,
)> {
    FAMILY_CATALOG.iter().find_map(|family| {
        family
            .variants
            .iter()
            .find(|variant| variant.id == diagram_type)
            .map(|variant| (family, variant))
    })
}

#[derive(Clone, Copy)]
#[cfg(feature = "full-config")]
pub(crate) struct FrontmatterConfigAliasFact {
    pub(crate) source: &'static str,
    pub(crate) namespace: &'static str,
}

#[cfg(feature = "full-config")]
pub(crate) fn frontmatter_config_aliases() -> &'static [FrontmatterConfigAliasFact] {
    static FACTS: OnceLock<Vec<FrontmatterConfigAliasFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            let mut facts: Vec<_> = FAMILY_CATALOG
                .iter()
                .flat_map(|family| {
                    family.variants.iter().filter_map(move |variant| {
                        variant.frontmatter_alias_order.map(|order| {
                            let namespace = family
                                .config
                                .expect("config aliases require a family namespace")
                                .namespace;
                            (
                                order,
                                FrontmatterConfigAliasFact {
                                    source: variant.id,
                                    namespace,
                                },
                            )
                        })
                    })
                })
                .collect();
            facts.sort_by_key(|(order, _)| *order);
            facts.into_iter().map(|(_, fact)| fact).collect()
        })
        .as_slice()
}

#[cfg(feature = "full-config")]
pub(crate) fn frontmatter_config_namespaces() -> &'static [&'static str] {
    static NAMESPACES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMESPACES
        .get_or_init(|| {
            let mut namespaces: Vec<_> = FAMILY_CATALOG
                .iter()
                .filter_map(|family| {
                    family
                        .config
                        .map(|config| (config.frontmatter_order, config.namespace))
                })
                .collect();
            namespaces.sort_by_key(|(order, _)| *order);
            namespaces
                .into_iter()
                .map(|(_, namespace)| namespace)
                .collect()
        })
        .as_slice()
}

pub(crate) fn config_namespace_for_diagram_type(diagram_type: &str) -> Option<&'static str> {
    find_variant(diagram_type).and_then(|(family, _)| family.config.map(|config| config.namespace))
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
const PIE_HEADERS: &[HeaderDefinition] = &[header(14, "pie", "pie header")];
const REQUIREMENT_HEADERS: &[HeaderDefinition] =
    &[header(15, "requirementDiagram", "requirement header")];
const SANKEY_HEADERS: &[HeaderDefinition] = &[header(16, "sankey", "sankey header")];
const PACKET_HEADERS: &[HeaderDefinition] = &[
    header(17, "packet", "packet header"),
    header(18, "packet-beta", "packet beta header"),
];
const XYCHART_HEADERS: &[HeaderDefinition] = &[
    header(19, "xychart", "xychart header"),
    header(20, "xychart-beta", "xychart beta header"),
];
const TREE_VIEW_HEADERS: &[HeaderDefinition] = &[header(21, "treeView-beta", "tree view header")];
const ISHIKAWA_HEADERS: &[HeaderDefinition] = &[header(22, "ishikawa-beta", "ishikawa header")];
const EVENTMODELING_HEADERS: &[HeaderDefinition] =
    &[header(23, "eventmodeling", "event modeling header")];
const QUADRANT_HEADERS: &[HeaderDefinition] =
    &[header(24, "quadrantChart", "quadrant chart header")];
const VENN_HEADERS: &[HeaderDefinition] = &[header(25, "venn-beta", "venn header")];
const ZENUML_HEADERS: &[HeaderDefinition] = &[header(26, "zenuml", "zenuml header")];
const C4_HEADERS: &[HeaderDefinition] = &[
    header(27, "C4Context", "c4 context header"),
    header(28, "C4Container", "c4 container header"),
    header(29, "C4Component", "c4 component header"),
    header(30, "C4Dynamic", "c4 dynamic header"),
    header(31, "C4Deployment", "c4 deployment header"),
];
const KANBAN_HEADERS: &[HeaderDefinition] = &[header(32, "kanban", "kanban header")];
const ARCHITECTURE_HEADERS: &[HeaderDefinition] =
    &[header(33, "architecture-beta", "architecture header")];
const BLOCK_HEADERS: &[HeaderDefinition] = &[header(34, "block-beta", "block header")];
const RADAR_HEADERS: &[HeaderDefinition] = &[header(35, "radar-beta", "radar header")];
const TREEMAP_HEADERS: &[HeaderDefinition] = &[header(36, "treemap-beta", "treemap header")];
const RAILROAD_HEADERS: &[HeaderDefinition] = &[header(37, "railroad-beta", "railroad header")];
const RAILROAD_EBNF_HEADERS: &[HeaderDefinition] =
    &[header(38, "railroad-ebnf-beta", "railroad ebnf header")];
const RAILROAD_ABNF_HEADERS: &[HeaderDefinition] =
    &[header(39, "railroad-abnf-beta", "railroad abnf header")];
const RAILROAD_PEG_HEADERS: &[HeaderDefinition] =
    &[header(40, "railroad-peg-beta", "railroad peg header")];
const WARDLEY_HEADERS: &[HeaderDefinition] = &[header(41, "wardley-beta", "wardley header")];
const CYNEFIN_HEADERS: &[HeaderDefinition] = &[header(42, "cynefin-beta", "cynefin header")];
const FLOWCHART_ELK_HEADERS: &[HeaderDefinition] =
    &[header(43, "flowchart-elk TD", "elk flowchart header")];

const FAST_SEQUENCE: &[FastKeywordDefinition] = &[fast_keyword(0, "sequenceDiagram")];
const FAST_MINDMAP: &[FastKeywordDefinition] = &[fast_keyword(1, "mindmap")];
const FAST_ARCHITECTURE: &[FastKeywordDefinition] = &[fast_keyword(2, "architecture")];
const FAST_ER: &[FastKeywordDefinition] = &[fast_keyword(3, "erDiagram")];
const FAST_GANTT: &[FastKeywordDefinition] = &[fast_keyword(4, "gantt")];
const FAST_TIMELINE: &[FastKeywordDefinition] = &[fast_keyword(5, "timeline")];
const FAST_JOURNEY: &[FastKeywordDefinition] = &[fast_keyword(6, "journey")];
const FAST_GIT_GRAPH: &[FastKeywordDefinition] = &[fast_keyword(7, "gitGraph")];
const FAST_QUADRANT: &[FastKeywordDefinition] = &[fast_keyword(8, "quadrantChart")];
const FAST_PACKET: &[FastKeywordDefinition] = &[fast_keyword(9, "packet-beta")];
const FAST_XYCHART: &[FastKeywordDefinition] = &[fast_keyword(10, "xychart-beta")];
const FAST_TREE_VIEW: &[FastKeywordDefinition] = &[fast_keyword(11, "treeView-beta")];
const FAST_ISHIKAWA: &[FastKeywordDefinition] = &[
    fast_keyword(12, "ishikawa-beta"),
    fast_keyword(13, "ishikawa"),
];
const FAST_EVENTMODELING: &[FastKeywordDefinition] = &[fast_keyword(14, "eventmodeling")];

const ERROR_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "error",
    profile: VariantProfile::All,
    catalog_order: 0,
    detector: Some(ordered(0, crate::detect::detector_error)),
    fast: &[],
    semantic: Some(ordered(0, crate::diagrams::error_diagram::parse_error)),
    editor: None,
    combined: None,
    typed: None,
    render_kind: None,
    metadata: None,
    headers: &[],
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const FLOWCHART_VARIANTS: &[FamilyVariantDefinition] = &[
    variant! {
        id: "flowchart-elk",
        profile: VariantProfile::FullOnly,
        catalog_order: 2,
        detector: Some(ordered(2, crate::detect::detector_flowchart_elk)),
        fast: &[],
        semantic: Some(ordered(3, crate::diagrams::flowchart::parse_flowchart)),
        editor: Some(ordered(2, crate::diagrams::flowchart::parse_flowchart_editor_facts)),
        combined: Some(ordered(2, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
        typed: Some(ordered(7, render_flowchart)),
        render_kind: Some("flowchart"),
        metadata: Some(metadata("flowchart", None)),
        headers: FLOWCHART_ELK_HEADERS,
        config_alias_order: Some(3),
        known_effect: KnownTypeEffect::ForceElk,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "flowchart-v2",
        profile: VariantProfile::All,
        catalog_order: 17,
        detector: Some(ordered(17, crate::detect::detector_flowchart_v2)),
        fast: &[],
        semantic: Some(ordered(1, crate::diagrams::flowchart::parse_flowchart)),
        editor: Some(ordered(0, crate::diagrams::flowchart::parse_flowchart_editor_facts)),
        combined: Some(ordered(0, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
        typed: Some(ordered(5, render_flowchart)),
        render_kind: Some("flowchart"),
        metadata: Some(metadata("flowchart", Some(6))),
        headers: FLOWCHART_HEADERS,
        config_alias_order: Some(2),
        known_effect: KnownTypeEffect::FlowchartConfiguredRenderer,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "flowchart",
        profile: VariantProfile::All,
        catalog_order: 18,
        detector: Some(ordered(18, crate::detect::detector_flowchart_dagre_d3_graph)),
        fast: &[],
        semantic: Some(ordered(2, crate::diagrams::flowchart::parse_flowchart)),
        editor: Some(ordered(1, crate::diagrams::flowchart::parse_flowchart_editor_facts)),
        combined: Some(ordered(1, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
        typed: Some(ordered(6, render_flowchart)),
        render_kind: Some("flowchart"),
        metadata: Some(metadata("flowchart", None)),
        headers: &[],
        config_alias_order: None,
        known_effect: KnownTypeEffect::FlowchartConfiguredRenderer,
        default_effect: DefaultEffect::None,
    },
];

const SWIMLANE_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "swimlane",
    profile: VariantProfile::All,
    catalog_order: 16,
    detector: Some(ordered(16, crate::detect::detector_swimlane)),
    fast: &[],
    semantic: Some(ordered(9, crate::diagrams::flowchart::parse_flowchart)),
    editor: Some(ordered(3, crate::diagrams::flowchart::parse_flowchart_editor_facts)),
    combined: Some(ordered(3, crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts)),
    typed: None,
    render_kind: None,
    metadata: None,
    headers: SWIMLANE_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::SwimlaneLayout,
}];

const MINDMAP_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "mindmap",
    profile: VariantProfile::FullOnly,
    catalog_order: 3,
    detector: Some(ordered(3, crate::detect::detector_mindmap)),
    fast: FAST_MINDMAP,
    semantic: Some(ordered(22, crate::diagrams::mindmap::parse_mindmap)),
    editor: Some(ordered(11, editor_mindmap)),
    combined: Some(ordered(5, crate::diagrams::mindmap::parse_mindmap_json_and_editor_facts)),
    typed: Some(ordered(0, render_mindmap)),
    render_kind: Some("mindmap"),
    metadata: Some(metadata("mindmap", Some(12))),
    headers: MINDMAP_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const ARCHITECTURE_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "architecture",
    profile: VariantProfile::FullOnly,
    catalog_order: 4,
    detector: Some(ordered(4, crate::detect::detector_architecture)),
    fast: FAST_ARCHITECTURE,
    semantic: Some(ordered(27, crate::diagrams::architecture::parse_architecture)),
    editor: Some(ordered(13, editor_architecture)),
    combined: Some(ordered(4, crate::diagrams::architecture::parse_architecture_json_and_editor_facts)),
    typed: Some(ordered(16, render_architecture)),
    render_kind: Some("architecture"),
    metadata: Some(metadata("architecture", Some(0))),
    headers: ARCHITECTURE_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const ZENUML_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "zenuml",
    profile: VariantProfile::All,
    catalog_order: 5,
    detector: Some(ordered(5, crate::detect::detector_zenuml)),
    fast: &[],
    semantic: Some(ordered(15, crate::diagrams::zenuml::parse_zenuml)),
    editor: Some(ordered(38, editor_zenuml)),
    combined: None,
    typed: Some(ordered(3, render_zenuml)),
    render_kind: Some("sequence"),
    metadata: Some(metadata("zenuml", Some(29))),
    headers: ZENUML_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const SEQUENCE_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "sequence",
    profile: VariantProfile::All,
    catalog_order: 15,
    detector: Some(ordered(15, crate::detect::detector_sequence)),
    fast: FAST_SEQUENCE,
    semantic: Some(ordered(8, crate::diagrams::sequence::parse_sequence)),
    editor: Some(ordered(4, editor_sequence)),
    combined: None,
    typed: Some(ordered(4, render_sequence)),
    render_kind: Some("sequence"),
    metadata: Some(metadata("sequence", Some(23))),
    headers: SEQUENCE_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const C4_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "c4",
    profile: VariantProfile::All,
    catalog_order: 6,
    detector: Some(ordered(6, crate::detect::detector_c4)),
    fast: &[],
    semantic: Some(ordered(6, crate::diagrams::c4::parse_c4)),
    editor: Some(ordered(15, editor_c4)),
    combined: None,
    typed: Some(ordered(10, render_c4)),
    render_kind: Some("c4"),
    metadata: Some(metadata("c4", Some(2))),
    headers: C4_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const KANBAN_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "kanban",
    profile: VariantProfile::All,
    catalog_order: 7,
    detector: Some(ordered(7, crate::detect::detector_kanban)),
    fast: &[],
    semantic: Some(ordered(26, crate::diagrams::kanban::parse_kanban)),
    editor: Some(ordered(18, editor_kanban)),
    combined: None,
    typed: Some(ordered(17, render_kanban)),
    render_kind: Some("kanban"),
    metadata: Some(metadata("kanban", Some(11))),
    headers: KANBAN_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const CLASS_VARIANTS: &[FamilyVariantDefinition] = &[
    variant! {
        id: "classDiagram",
        profile: VariantProfile::All,
        catalog_order: 8,
        detector: Some(ordered(8, crate::detect::detector_class_v2)),
        fast: &[],
        semantic: Some(ordered(16, crate::diagrams::class::parse_class)),
        editor: Some(ordered(7, editor_class)),
        combined: None,
        typed: Some(ordered(8, render_class)),
        render_kind: Some("class"),
        metadata: Some(metadata("class", Some(3))),
        headers: CLASS_HEADERS,
        config_alias_order: Some(0),
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "class",
        profile: VariantProfile::All,
        catalog_order: 9,
        detector: Some(ordered(9, crate::detect::detector_class_dagre_d3)),
        fast: &[],
        semantic: Some(ordered(17, crate::diagrams::class::parse_class)),
        editor: Some(ordered(8, editor_class)),
        combined: None,
        typed: Some(ordered(9, render_class)),
        render_kind: Some("class"),
        metadata: Some(metadata("class", None)),
        headers: &[],
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
];

const ER_VARIANTS: &[FamilyVariantDefinition] = &[
    variant! {
        id: "er",
        profile: VariantProfile::All,
        catalog_order: 10,
        detector: Some(ordered(10, crate::detect::detector_er)),
        fast: FAST_ER,
        semantic: Some(ordered(18, crate::diagrams::er::parse_er)),
        editor: Some(ordered(9, editor_er)),
        combined: Some(ordered(17, crate::diagrams::er::parse_er_json_and_editor_facts)),
        typed: Some(ordered(29, render_er)),
        render_kind: Some("er"),
        metadata: Some(metadata("er", Some(5))),
        headers: ER_HEADERS,
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "erDiagram",
        profile: VariantProfile::All,
        catalog_order: 41,
        detector: None,
        fast: &[],
        semantic: Some(ordered(19, crate::diagrams::er::parse_er)),
        editor: Some(ordered(10, editor_er)),
        combined: Some(ordered(18, crate::diagrams::er::parse_er_json_and_editor_facts)),
        typed: Some(ordered(30, render_er)),
        render_kind: Some("er"),
        metadata: Some(metadata("er", None)),
        headers: &[],
        config_alias_order: Some(1),
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
];

const GANTT_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "gantt",
    profile: VariantProfile::All,
    catalog_order: 11,
    detector: Some(ordered(11, crate::detect::detector_gantt)),
    fast: FAST_GANTT,
    semantic: Some(ordered(23, crate::diagrams::gantt::parse_gantt)),
    editor: Some(ordered(12, editor_gantt)),
    combined: None,
    typed: Some(ordered(18, render_gantt)),
    render_kind: Some("gantt"),
    metadata: Some(metadata("gantt", Some(7))),
    headers: GANTT_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const INFO_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "info",
    profile: VariantProfile::All,
    catalog_order: 12,
    detector: Some(ordered(12, crate::detect::detector_info)),
    fast: &[],
    semantic: Some(ordered(4, crate::diagrams::info::parse_info)),
    editor: Some(ordered(21, editor_info)),
    combined: Some(ordered(10, crate::diagrams::info::parse_info_json_and_editor_facts)),
    typed: Some(ordered(26, render_info)),
    render_kind: Some("info"),
    metadata: Some(metadata("info", Some(9))),
    headers: INFO_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const PIE_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "pie",
    profile: VariantProfile::All,
    catalog_order: 13,
    detector: Some(ordered(13, crate::detect::detector_pie)),
    fast: &[],
    semantic: Some(ordered(5, crate::diagrams::pie::parse_pie)),
    editor: Some(ordered(23, editor_pie)),
    combined: Some(ordered(11, crate::diagrams::pie::parse_pie_json_and_editor_facts)),
    typed: Some(ordered(19, render_pie)),
    render_kind: Some("pie"),
    metadata: Some(metadata("pie", Some(14))),
    headers: PIE_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const REQUIREMENT_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "requirement",
    profile: VariantProfile::All,
    catalog_order: 14,
    detector: Some(ordered(14, crate::detect::detector_requirement)),
    fast: &[],
    semantic: Some(ordered(7, crate::diagrams::requirement::parse_requirement)),
    editor: Some(ordered(35, editor_requirement)),
    combined: None,
    typed: Some(ordered(23, render_requirement)),
    render_kind: Some("requirement"),
    metadata: Some(metadata("requirement", Some(21))),
    headers: REQUIREMENT_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const TIMELINE_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "timeline",
    profile: VariantProfile::All,
    catalog_order: 19,
    detector: Some(ordered(19, crate::detect::detector_timeline)),
    fast: FAST_TIMELINE,
    semantic: Some(ordered(24, crate::diagrams::timeline::parse_timeline)),
    editor: Some(ordered(22, editor_timeline)),
    combined: None,
    typed: Some(ordered(21, render_timeline)),
    render_kind: Some("timeline"),
    metadata: Some(metadata("timeline", Some(25))),
    headers: TIMELINE_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const GIT_GRAPH_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "gitGraph",
    profile: VariantProfile::All,
    catalog_order: 20,
    detector: Some(ordered(20, crate::detect::detector_git_graph)),
    fast: FAST_GIT_GRAPH,
    semantic: Some(ordered(29, crate::diagrams::git_graph::parse_git_graph)),
    editor: Some(ordered(17, editor_git_graph)),
    combined: Some(ordered(15, crate::diagrams::git_graph::parse_git_graph_json_and_editor_facts)),
    typed: Some(ordered(33, render_git_graph)),
    render_kind: Some("gitGraph"),
    metadata: Some(metadata("gitgraph", Some(8))),
    headers: &[],
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const STATE_VARIANTS: &[FamilyVariantDefinition] = &[
    variant! {
        id: "stateDiagram",
        profile: VariantProfile::All,
        catalog_order: 21,
        detector: Some(ordered(21, crate::detect::detector_state_v2)),
        fast: &[],
        semantic: Some(ordered(20, crate::diagrams::state::parse_state)),
        editor: Some(ordered(5, editor_state)),
        combined: None,
        typed: Some(ordered(1, render_state)),
        render_kind: Some("state"),
        metadata: Some(metadata("state", Some(24))),
        headers: STATE_HEADERS,
        config_alias_order: Some(4),
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "state",
        profile: VariantProfile::All,
        catalog_order: 22,
        detector: Some(ordered(22, crate::detect::detector_state_dagre_d3)),
        fast: &[],
        semantic: Some(ordered(21, crate::diagrams::state::parse_state)),
        editor: Some(ordered(6, editor_state)),
        combined: None,
        typed: Some(ordered(2, render_state)),
        render_kind: Some("state"),
        metadata: Some(metadata("state", None)),
        headers: &[],
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
];

const JOURNEY_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "journey",
    profile: VariantProfile::All,
    catalog_order: 23,
    detector: Some(ordered(23, crate::detect::detector_journey)),
    fast: FAST_JOURNEY,
    semantic: Some(ordered(25, crate::diagrams::journey::parse_journey)),
    editor: Some(ordered(20, editor_journey)),
    combined: None,
    typed: Some(ordered(22, render_journey)),
    render_kind: Some("journey"),
    metadata: Some(metadata("journey", Some(10))),
    headers: JOURNEY_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const QUADRANT_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "quadrantChart",
    profile: VariantProfile::All,
    catalog_order: 24,
    detector: Some(ordered(24, crate::detect::detector_quadrant)),
    fast: FAST_QUADRANT,
    semantic: Some(ordered(30, crate::diagrams::quadrant_chart::parse_quadrant_chart)),
    editor: Some(ordered(28, editor_quadrant_chart)),
    combined: None,
    typed: Some(ordered(31, render_quadrant_chart)),
    render_kind: Some("quadrantChart"),
    metadata: Some(metadata("quadrantchart", Some(15))),
    headers: QUADRANT_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const SANKEY_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "sankey",
    profile: VariantProfile::All,
    catalog_order: 25,
    detector: Some(ordered(25, crate::detect::detector_sankey)),
    fast: &[],
    semantic: Some(ordered(38, crate::diagrams::sankey::parse_sankey)),
    editor: Some(ordered(25, editor_sankey)),
    combined: Some(ordered(16, crate::diagrams::sankey::parse_sankey_json_and_editor_facts)),
    typed: Some(ordered(24, render_sankey)),
    render_kind: Some("sankey"),
    metadata: Some(metadata("sankey", Some(22))),
    headers: SANKEY_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const PACKET_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "packet",
    profile: VariantProfile::All,
    catalog_order: 26,
    detector: Some(ordered(26, crate::detect::detector_packet)),
    fast: FAST_PACKET,
    semantic: Some(ordered(31, crate::diagrams::packet::parse_packet)),
    editor: Some(ordered(24, editor_packet)),
    combined: Some(ordered(12, crate::diagrams::packet::parse_packet_json_and_editor_facts)),
    typed: Some(ordered(20, render_packet)),
    render_kind: Some("packet"),
    metadata: Some(metadata("packet", Some(13))),
    headers: PACKET_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const XYCHART_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "xychart",
    profile: VariantProfile::All,
    catalog_order: 27,
    detector: Some(ordered(27, crate::detect::detector_xychart)),
    fast: FAST_XYCHART,
    semantic: Some(ordered(39, crate::diagrams::xychart::parse_xychart)),
    editor: Some(ordered(37, editor_xychart)),
    combined: None,
    typed: Some(ordered(32, render_xychart)),
    render_kind: Some("xychart"),
    metadata: Some(metadata("xychart", Some(28))),
    headers: XYCHART_HEADERS,
    config_alias_order: Some(5),
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const BLOCK_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "block",
    profile: VariantProfile::All,
    catalog_order: 28,
    detector: Some(ordered(28, crate::detect::detector_block)),
    fast: &[],
    semantic: Some(ordered(28, crate::diagrams::block::parse_block)),
    editor: Some(ordered(14, editor_block)),
    combined: None,
    typed: Some(ordered(28, render_block)),
    render_kind: Some("block"),
    metadata: Some(metadata("block", Some(1))),
    headers: BLOCK_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const EVENTMODELING_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "eventmodeling",
    profile: VariantProfile::All,
    catalog_order: 29,
    detector: Some(ordered(29, crate::detect::detector_eventmodeling)),
    fast: FAST_EVENTMODELING,
    semantic: Some(ordered(35, crate::diagrams::eventmodeling::parse_eventmodeling)),
    editor: Some(ordered(27, editor_eventmodeling)),
    combined: None,
    typed: Some(ordered(36, render_eventmodeling)),
    render_kind: Some("eventmodeling"),
    metadata: None,
    headers: EVENTMODELING_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const TREE_VIEW_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "treeView",
    profile: VariantProfile::All,
    catalog_order: 30,
    detector: Some(ordered(30, crate::detect::detector_tree_view)),
    fast: FAST_TREE_VIEW,
    semantic: Some(ordered(33, crate::diagrams::tree_view::parse_tree_view)),
    editor: Some(ordered(26, editor_tree_view)),
    combined: None,
    typed: Some(ordered(34, render_tree_view)),
    render_kind: Some("treeView"),
    metadata: None,
    headers: TREE_VIEW_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const RADAR_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "radar",
    profile: VariantProfile::All,
    catalog_order: 31,
    detector: Some(ordered(31, crate::detect::detector_radar)),
    fast: &[],
    semantic: Some(ordered(32, crate::diagrams::radar::parse_radar)),
    editor: Some(ordered(33, editor_radar)),
    combined: Some(ordered(14, crate::diagrams::radar::parse_radar_json_and_editor_facts)),
    typed: Some(ordered(25, render_radar)),
    render_kind: Some("radar"),
    metadata: Some(metadata("radar", Some(16))),
    headers: RADAR_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const ISHIKAWA_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "ishikawa",
    profile: VariantProfile::All,
    catalog_order: 32,
    detector: Some(ordered(32, crate::detect::detector_ishikawa)),
    fast: FAST_ISHIKAWA,
    semantic: Some(ordered(34, crate::diagrams::ishikawa::parse_ishikawa)),
    editor: Some(ordered(19, editor_ishikawa)),
    combined: None,
    typed: Some(ordered(35, render_ishikawa)),
    render_kind: Some("ishikawa"),
    metadata: None,
    headers: ISHIKAWA_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const TREEMAP_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "treemap",
    profile: VariantProfile::All,
    catalog_order: 33,
    detector: Some(ordered(33, crate::detect::detector_treemap)),
    fast: &[],
    semantic: Some(ordered(36, crate::diagrams::treemap::parse_treemap)),
    editor: Some(ordered(34, editor_treemap)),
    combined: None,
    typed: Some(ordered(27, render_treemap)),
    render_kind: Some("treemap"),
    metadata: Some(metadata("treemap", Some(26))),
    headers: TREEMAP_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const RAILROAD_VARIANTS: &[FamilyVariantDefinition] = &[
    variant! {
        id: "railroad",
        profile: VariantProfile::All,
        catalog_order: 34,
        detector: Some(ordered(34, crate::detect::detector_railroad)),
        fast: &[],
        semantic: Some(ordered(11, crate::diagrams::railroad::parse_railroad)),
        editor: Some(ordered(29, editor_railroad)),
        combined: Some(ordered(6, crate::diagrams::railroad::parse_railroad_json_and_editor_facts)),
        typed: Some(ordered(12, render_railroad)),
        render_kind: Some("railroad"),
        metadata: Some(metadata("railroad", Some(17))),
        headers: RAILROAD_HEADERS,
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "railroadEbnf",
        profile: VariantProfile::All,
        catalog_order: 35,
        detector: Some(ordered(35, crate::detect::detector_railroad_ebnf)),
        fast: &[],
        semantic: Some(ordered(12, crate::diagrams::railroad::parse_railroad_ebnf)),
        editor: Some(ordered(30, editor_railroad_ebnf)),
        combined: Some(ordered(7, crate::diagrams::railroad::parse_railroad_ebnf_json_and_editor_facts)),
        typed: Some(ordered(13, render_railroad_ebnf)),
        render_kind: Some("railroad"),
        metadata: Some(metadata("railroadEbnf", Some(19))),
        headers: RAILROAD_EBNF_HEADERS,
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "railroadAbnf",
        profile: VariantProfile::All,
        catalog_order: 36,
        detector: Some(ordered(36, crate::detect::detector_railroad_abnf)),
        fast: &[],
        semantic: Some(ordered(13, crate::diagrams::railroad::parse_railroad_abnf)),
        editor: Some(ordered(31, editor_railroad_abnf)),
        combined: Some(ordered(8, crate::diagrams::railroad::parse_railroad_abnf_json_and_editor_facts)),
        typed: Some(ordered(14, render_railroad_abnf)),
        render_kind: Some("railroad"),
        metadata: Some(metadata("railroadAbnf", Some(18))),
        headers: RAILROAD_ABNF_HEADERS,
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
    variant! {
        id: "railroadPeg",
        profile: VariantProfile::All,
        catalog_order: 37,
        detector: Some(ordered(37, crate::detect::detector_railroad_peg)),
        fast: &[],
        semantic: Some(ordered(14, crate::diagrams::railroad::parse_railroad_peg)),
        editor: Some(ordered(32, editor_railroad_peg)),
        combined: Some(ordered(9, crate::diagrams::railroad::parse_railroad_peg_json_and_editor_facts)),
        typed: Some(ordered(15, render_railroad_peg)),
        render_kind: Some("railroad"),
        metadata: Some(metadata("railroadPeg", Some(20))),
        headers: RAILROAD_PEG_HEADERS,
        config_alias_order: None,
        known_effect: KnownTypeEffect::None,
        default_effect: DefaultEffect::None,
    },
];

const VENN_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "venn",
    profile: VariantProfile::All,
    catalog_order: 38,
    detector: Some(ordered(38, crate::detect::detector_venn)),
    fast: &[],
    semantic: Some(ordered(37, crate::diagrams::venn::parse_venn)),
    editor: Some(ordered(36, editor_venn)),
    combined: None,
    typed: Some(ordered(37, render_venn)),
    render_kind: Some("venn"),
    metadata: Some(metadata("venn", Some(27))),
    headers: VENN_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const WARDLEY_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "wardley",
    profile: VariantProfile::All,
    catalog_order: 39,
    detector: Some(ordered(39, crate::detect::detector_wardley)),
    fast: &[],
    semantic: None,
    editor: None,
    combined: None,
    typed: None,
    render_kind: None,
    metadata: None,
    headers: WARDLEY_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const CYNEFIN_VARIANTS: &[FamilyVariantDefinition] = &[variant! {
    id: "cynefin",
    profile: VariantProfile::All,
    catalog_order: 40,
    detector: Some(ordered(40, crate::detect::detector_cynefin)),
    fast: &[],
    semantic: Some(ordered(10, crate::diagrams::cynefin::parse_cynefin)),
    editor: Some(ordered(16, editor_cynefin)),
    combined: Some(ordered(13, crate::diagrams::cynefin::parse_cynefin_json_and_editor_facts)),
    typed: Some(ordered(11, render_cynefin)),
    render_kind: Some("cynefin"),
    metadata: Some(metadata("cynefin", Some(4))),
    headers: CYNEFIN_HEADERS,
    config_alias_order: None,
    known_effect: KnownTypeEffect::None,
    default_effect: DefaultEffect::None,
}];

const FAMILY_CATALOG: &[DiagramFamilyDefinition] = &[
    DiagramFamilyDefinition {
        logical_kind: "error",
        config: None,
        variants: ERROR_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "flowchart",
        config: Some(FamilyConfigDefinition {
            namespace: "flowchart",
            frontmatter_order: 7,
        }),
        variants: FLOWCHART_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "swimlane",
        config: Some(FamilyConfigDefinition {
            namespace: "swimlane",
            frontmatter_order: 23,
        }),
        variants: SWIMLANE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "mindmap",
        config: Some(FamilyConfigDefinition {
            namespace: "mindmap",
            frontmatter_order: 13,
        }),
        variants: MINDMAP_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "architecture",
        config: Some(FamilyConfigDefinition {
            namespace: "architecture",
            frontmatter_order: 0,
        }),
        variants: ARCHITECTURE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "zenuml",
        config: Some(FamilyConfigDefinition {
            namespace: "zenuml",
            frontmatter_order: 29,
        }),
        variants: ZENUML_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "sequence",
        config: Some(FamilyConfigDefinition {
            namespace: "sequence",
            frontmatter_order: 21,
        }),
        variants: SEQUENCE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "c4",
        config: Some(FamilyConfigDefinition {
            namespace: "c4",
            frontmatter_order: 2,
        }),
        variants: C4_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "kanban",
        config: Some(FamilyConfigDefinition {
            namespace: "kanban",
            frontmatter_order: 12,
        }),
        variants: KANBAN_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "class",
        config: Some(FamilyConfigDefinition {
            namespace: "class",
            frontmatter_order: 3,
        }),
        variants: CLASS_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "er",
        config: Some(FamilyConfigDefinition {
            namespace: "er",
            frontmatter_order: 5,
        }),
        variants: ER_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "gantt",
        config: Some(FamilyConfigDefinition {
            namespace: "gantt",
            frontmatter_order: 8,
        }),
        variants: GANTT_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "info",
        config: None,
        variants: INFO_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "pie",
        config: Some(FamilyConfigDefinition {
            namespace: "pie",
            frontmatter_order: 15,
        }),
        variants: PIE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "requirement",
        config: Some(FamilyConfigDefinition {
            namespace: "requirement",
            frontmatter_order: 19,
        }),
        variants: REQUIREMENT_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "timeline",
        config: Some(FamilyConfigDefinition {
            namespace: "timeline",
            frontmatter_order: 24,
        }),
        variants: TIMELINE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "gitGraph",
        config: Some(FamilyConfigDefinition {
            namespace: "gitGraph",
            frontmatter_order: 9,
        }),
        variants: GIT_GRAPH_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "state",
        config: Some(FamilyConfigDefinition {
            namespace: "state",
            frontmatter_order: 22,
        }),
        variants: STATE_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "journey",
        config: Some(FamilyConfigDefinition {
            namespace: "journey",
            frontmatter_order: 11,
        }),
        variants: JOURNEY_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "quadrantChart",
        config: Some(FamilyConfigDefinition {
            namespace: "quadrantChart",
            frontmatter_order: 16,
        }),
        variants: QUADRANT_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "sankey",
        config: Some(FamilyConfigDefinition {
            namespace: "sankey",
            frontmatter_order: 20,
        }),
        variants: SANKEY_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "packet",
        config: Some(FamilyConfigDefinition {
            namespace: "packet",
            frontmatter_order: 14,
        }),
        variants: PACKET_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "xychart",
        config: Some(FamilyConfigDefinition {
            namespace: "xyChart",
            frontmatter_order: 28,
        }),
        variants: XYCHART_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "block",
        config: Some(FamilyConfigDefinition {
            namespace: "block",
            frontmatter_order: 1,
        }),
        variants: BLOCK_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "eventmodeling",
        config: Some(FamilyConfigDefinition {
            namespace: "eventmodeling",
            frontmatter_order: 6,
        }),
        variants: EVENTMODELING_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "treeView",
        config: Some(FamilyConfigDefinition {
            namespace: "treeView",
            frontmatter_order: 25,
        }),
        variants: TREE_VIEW_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "radar",
        config: Some(FamilyConfigDefinition {
            namespace: "radar",
            frontmatter_order: 17,
        }),
        variants: RADAR_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "ishikawa",
        config: Some(FamilyConfigDefinition {
            namespace: "ishikawa",
            frontmatter_order: 10,
        }),
        variants: ISHIKAWA_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "treemap",
        config: Some(FamilyConfigDefinition {
            namespace: "treemap",
            frontmatter_order: 26,
        }),
        variants: TREEMAP_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "railroad",
        config: Some(FamilyConfigDefinition {
            namespace: "railroad",
            frontmatter_order: 18,
        }),
        variants: RAILROAD_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "venn",
        config: Some(FamilyConfigDefinition {
            namespace: "venn",
            frontmatter_order: 27,
        }),
        variants: VENN_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "wardley",
        config: None,
        variants: WARDLEY_VARIANTS,
    },
    DiagramFamilyDefinition {
        logical_kind: "cynefin",
        config: Some(FamilyConfigDefinition {
            namespace: "cynefin",
            frontmatter_order: 4,
        }),
        variants: CYNEFIN_VARIANTS,
    },
];

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_ids_orders_and_family_policy_are_internally_consistent() {
        let mut ids = BTreeSet::new();
        let mut catalog_orders = BTreeSet::new();
        let mut detector_orders = BTreeSet::new();
        let mut fast_orders = BTreeSet::new();
        let mut semantic_orders = BTreeSet::new();
        let mut editor_orders = BTreeSet::new();
        let mut combined_orders = BTreeSet::new();
        let mut render_orders = BTreeSet::new();
        let mut metadata_orders = BTreeSet::new();
        let mut header_orders = BTreeSet::new();

        for family in FAMILY_CATALOG {
            for variant in family.variants {
                assert_ne!(variant.id, "---", "frontmatter guard is not a family");
                assert!(
                    ids.insert(variant.id),
                    "duplicate catalog id {}",
                    variant.id
                );
                assert!(
                    catalog_orders.insert(variant.catalog_order),
                    "duplicate catalog order {}",
                    variant.catalog_order
                );
                assert_eq!(
                    variant.typed_render.is_some(),
                    variant.render_model_kind.is_some(),
                    "{} typed parser and render kind must be declared together",
                    variant.id
                );
                assert!(
                    variant.metadata.is_none() || variant.typed_render.is_some(),
                    "{} metadata requires a typed render parser",
                    variant.id
                );
                assert!(
                    variant.editor.is_none() || variant.semantic.is_some(),
                    "{} editor facts require family semantics",
                    variant.id
                );
                assert!(
                    variant.combined.is_none()
                        || (variant.semantic.is_some() && variant.editor.is_some()),
                    "{} combined parsing requires semantic and editor adapters",
                    variant.id
                );

                if let Some(fact) = variant.detector {
                    assert!(detector_orders.insert(fact.order));
                }
                for fact in variant.fast_keywords {
                    assert!(fast_orders.insert(fact.order));
                }
                if let Some(fact) = variant.semantic {
                    assert!(semantic_orders.insert(fact.order));
                }
                if let Some(fact) = variant.editor {
                    assert!(editor_orders.insert(fact.order));
                }
                if let Some(fact) = variant.combined {
                    assert!(combined_orders.insert(fact.order));
                }
                if let Some(fact) = variant.typed_render {
                    assert!(render_orders.insert(fact.order));
                }
                if let Some(order) = variant.metadata.and_then(|metadata| metadata.order) {
                    assert!(metadata_orders.insert(order));
                }
                for fact in variant.headers {
                    assert!(header_orders.insert(fact.order));
                }
            }
        }

        let config_orders = FAMILY_CATALOG
            .iter()
            .filter_map(|family| family.config.map(|config| config.frontmatter_order))
            .collect::<BTreeSet<_>>();
        let config_count = FAMILY_CATALOG
            .iter()
            .filter(|family| family.config.is_some())
            .count();
        assert_eq!(config_orders.len(), config_count);
        assert_eq!(
            ids.len(),
            diagram_family_capabilities(BaselineRegistryProfile::Full).len()
        );
    }
}
