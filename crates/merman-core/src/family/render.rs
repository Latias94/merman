use super::*;

macro_rules! render_parser {
    ($fn_name:ident, $parser:path, $variant:path) => {
        pub(super) fn $fn_name(
            code: &str,
            meta: &PresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<RenderSemanticParseOutput>> {
            control.checkpoint()?;
            let result = $parser(code, meta)
                .map($variant)
                .map(RenderSemanticParseOutput::new);
            control.checkpoint()?;
            Ok(result)
        }
    };
}

#[allow(
    unused_macros,
    reason = "No controlled family adapter is emitted in infrastructure-only builds."
)]
macro_rules! render_parser_controlled {
    ($fn_name:ident, $parser:path, $variant:path) => {
        pub(super) fn $fn_name(
            code: &str,
            meta: &PresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<RenderSemanticParseOutput>> {
            control.checkpoint()?;
            let result = $parser(code, meta, control)?
                .map($variant)
                .map(RenderSemanticParseOutput::new);
            control.checkpoint()?;
            Ok(result)
        }
    };
}

render_parser!(
    render_error,
    crate::diagrams::error_diagram::parse_error_model_for_render,
    RenderSemanticModel::Error
);
#[cfg(feature = "diagram-mindmap")]
render_parser_controlled!(
    render_mindmap,
    crate::diagrams::mindmap::parse_mindmap_model_for_render_controlled,
    RenderSemanticModel::Mindmap
);
#[cfg(feature = "diagram-state")]
render_parser_controlled!(
    render_state,
    crate::diagrams::state::parse_state_model_for_render_controlled,
    RenderSemanticModel::State
);
#[cfg(feature = "diagram-zenuml")]
render_parser_controlled!(
    render_zenuml,
    crate::diagrams::zenuml::parse_zenuml_model_for_render_controlled,
    RenderSemanticModel::Zenuml
);
#[cfg(feature = "diagram-sequence")]
render_parser_controlled!(
    render_sequence,
    crate::diagrams::sequence::parse_sequence_model_for_render_controlled,
    RenderSemanticModel::Sequence
);
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) fn render_flowchart(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RenderSemanticParseOutput>> {
    let result = crate::diagrams::flowchart::parse_flowchart_model_with_render_context_controlled(
        code, meta, control,
    )?;
    Ok(result
        .map(|(model, label_sources)| RenderSemanticParseOutput::flowchart(model, label_sources)))
}
#[cfg(feature = "diagram-class")]
pub(super) fn render_class(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RenderSemanticParseOutput>> {
    control.checkpoint()?;
    let result = crate::diagrams::class::parse_class_typed_with_render_context_controlled(
        code, meta, control,
    )?
    .map(|(model, style_precedence_facts)| {
        RenderSemanticParseOutput::class(model, style_precedence_facts)
    });
    control.checkpoint()?;
    Ok(result)
}
#[cfg(feature = "diagram-c4")]
render_parser_controlled!(
    render_c4,
    crate::diagrams::c4::parse_c4_model_for_render_controlled,
    RenderSemanticModel::C4
);
#[cfg(feature = "diagram-cynefin")]
render_parser_controlled!(
    render_cynefin,
    crate::diagrams::cynefin::parse_cynefin_model_for_render_controlled,
    RenderSemanticModel::Cynefin
);
#[cfg(feature = "diagram-railroad")]
render_parser_controlled!(
    render_railroad,
    crate::diagrams::railroad::parse_railroad_model_for_render_controlled,
    RenderSemanticModel::Railroad
);
#[cfg(feature = "diagram-railroad")]
render_parser_controlled!(
    render_railroad_ebnf,
    crate::diagrams::railroad::parse_railroad_ebnf_model_for_render_controlled,
    RenderSemanticModel::Railroad
);
#[cfg(feature = "diagram-railroad")]
render_parser_controlled!(
    render_railroad_abnf,
    crate::diagrams::railroad::parse_railroad_abnf_model_for_render_controlled,
    RenderSemanticModel::Railroad
);
#[cfg(feature = "diagram-railroad")]
render_parser_controlled!(
    render_railroad_peg,
    crate::diagrams::railroad::parse_railroad_peg_model_for_render_controlled,
    RenderSemanticModel::Railroad
);
#[cfg(feature = "diagram-architecture")]
render_parser_controlled!(
    render_architecture,
    crate::diagrams::architecture::parse_architecture_model_for_render_controlled,
    RenderSemanticModel::Architecture
);
#[cfg(feature = "diagram-kanban")]
render_parser_controlled!(
    render_kanban,
    crate::diagrams::kanban::parse_kanban_model_for_render_controlled,
    RenderSemanticModel::Kanban
);
#[cfg(feature = "diagram-gantt")]
render_parser_controlled!(
    render_gantt,
    crate::diagrams::gantt::parse_gantt_model_for_render_controlled,
    RenderSemanticModel::Gantt
);
#[cfg(feature = "diagram-pie")]
render_parser_controlled!(
    render_pie,
    crate::diagrams::pie::parse_pie_model_for_render_controlled,
    RenderSemanticModel::Pie
);
#[cfg(feature = "diagram-packet")]
render_parser_controlled!(
    render_packet,
    crate::diagrams::packet::parse_packet_model_for_render_controlled,
    RenderSemanticModel::Packet
);
#[cfg(feature = "diagram-timeline")]
render_parser_controlled!(
    render_timeline,
    crate::diagrams::timeline::parse_timeline_model_for_render_controlled,
    RenderSemanticModel::Timeline
);
#[cfg(feature = "diagram-journey")]
render_parser_controlled!(
    render_journey,
    crate::diagrams::journey::parse_journey_model_for_render_controlled,
    RenderSemanticModel::Journey
);
#[cfg(feature = "diagram-requirement")]
render_parser_controlled!(
    render_requirement,
    crate::diagrams::requirement::parse_requirement_model_for_render_controlled,
    RenderSemanticModel::Requirement
);
#[cfg(feature = "diagram-sankey")]
render_parser_controlled!(
    render_sankey,
    crate::diagrams::sankey::parse_sankey_model_for_render_controlled,
    RenderSemanticModel::Sankey
);
#[cfg(feature = "diagram-radar")]
render_parser_controlled!(
    render_radar,
    crate::diagrams::radar::parse_radar_model_for_render_controlled,
    RenderSemanticModel::Radar
);
#[cfg(feature = "diagram-info")]
render_parser_controlled!(
    render_info,
    crate::diagrams::info::parse_info_model_for_render_controlled,
    RenderSemanticModel::Info
);
#[cfg(feature = "diagram-treemap")]
render_parser_controlled!(
    render_treemap,
    crate::diagrams::treemap::parse_treemap_model_for_render_controlled,
    RenderSemanticModel::Treemap
);
#[cfg(feature = "diagram-block")]
render_parser_controlled!(
    render_block,
    crate::diagrams::block::parse_block_model_for_render_controlled,
    RenderSemanticModel::Block
);
#[cfg(feature = "diagram-er")]
render_parser_controlled!(
    render_er,
    crate::diagrams::er::parse_er_model_for_render_controlled,
    RenderSemanticModel::Er
);
#[cfg(feature = "diagram-quadrant-chart")]
render_parser_controlled!(
    render_quadrant_chart,
    crate::diagrams::quadrant_chart::parse_quadrant_chart_model_for_render_controlled,
    RenderSemanticModel::QuadrantChart
);
#[cfg(feature = "diagram-xychart")]
render_parser_controlled!(
    render_xychart,
    crate::diagrams::xychart::parse_xychart_model_for_render_controlled,
    RenderSemanticModel::XyChart
);
#[cfg(feature = "diagram-git-graph")]
render_parser_controlled!(
    render_git_graph,
    crate::diagrams::git_graph::parse_git_graph_model_for_render_controlled,
    RenderSemanticModel::GitGraph
);
#[cfg(feature = "diagram-tree-view")]
render_parser_controlled!(
    render_tree_view,
    crate::diagrams::tree_view::parse_tree_view_model_for_render_controlled,
    RenderSemanticModel::TreeView
);
#[cfg(feature = "diagram-ishikawa")]
render_parser_controlled!(
    render_ishikawa,
    crate::diagrams::ishikawa::parse_ishikawa_model_for_render_controlled,
    RenderSemanticModel::Ishikawa
);
#[cfg(feature = "diagram-event-modeling")]
render_parser_controlled!(
    render_eventmodeling,
    crate::diagrams::eventmodeling::parse_eventmodeling_model_for_render_controlled,
    RenderSemanticModel::EventModeling
);
#[cfg(feature = "diagram-venn")]
render_parser_controlled!(
    render_venn,
    crate::diagrams::venn::parse_venn_model_for_render_controlled,
    RenderSemanticModel::Venn
);
#[cfg(feature = "diagram-wardley")]
render_parser_controlled!(
    render_wardley,
    crate::diagrams::wardley::parse_wardley_model_for_render_controlled,
    RenderSemanticModel::Wardley
);
#[cfg(feature = "diagram-usecase")]
render_parser_controlled!(
    render_usecase,
    crate::diagrams::usecase::parse_usecase_model_for_render_controlled,
    RenderSemanticModel::Usecase
);
#[cfg(feature = "diagram-agentflow")]
render_parser_controlled!(
    render_agentflow,
    crate::diagrams::agentflow::parse_agentflow_model_for_render_controlled,
    RenderSemanticModel::Agentflow
);
