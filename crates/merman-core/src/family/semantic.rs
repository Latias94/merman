//! Canonical parser callbacks bound by the family catalog.

#[cfg(feature = "diagram-class")]
pub(super) fn canonical_class(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::class::parse_class_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Class),
    )
}

#[cfg(feature = "diagram-git-graph")]
pub(super) fn canonical_git_graph(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::git_graph::parse_git_graph_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::GitGraph),
    )
}

#[cfg(feature = "diagram-agentflow")]
pub(super) fn canonical_agentflow(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::agentflow::parse_agentflow_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Agentflow),
    )
}

#[cfg(feature = "diagram-kanban")]
pub(super) fn canonical_kanban(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::kanban::parse_kanban_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Kanban),
    )
}

#[cfg(feature = "diagram-mindmap")]
pub(super) fn canonical_mindmap(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::mindmap::parse_mindmap_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Mindmap),
    )
}

#[cfg(feature = "diagram-xychart")]
pub(super) fn canonical_xychart(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::xychart::parse_xychart_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::XyChart),
    )
}

#[cfg(feature = "diagram-block")]
pub(super) fn canonical_block(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::block::parse_block_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Block),
    )
}

#[cfg(feature = "diagram-ishikawa")]
pub(super) fn canonical_ishikawa(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::ishikawa::parse_ishikawa_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Ishikawa),
    )
}

#[cfg(feature = "diagram-treemap")]
pub(super) fn canonical_treemap(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::treemap::parse_treemap_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Treemap),
    )
}

#[cfg(feature = "diagram-c4")]
pub(super) fn canonical_c4(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::c4::parse_c4_semantic_model_controlled(
        code,
        metadata,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::C4))
}

#[cfg(feature = "diagram-tree-view")]
pub(super) fn canonical_tree_view(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::tree_view::parse_tree_view_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::TreeView),
    )
}

#[cfg(feature = "diagram-packet")]
pub(super) fn canonical_packet(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::packet::parse_packet_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Packet),
    )
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) fn canonical_flowchart(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::flowchart::parse_semantic_model_controlled(
        code,
        metadata,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::Flowchart))
}

#[cfg(feature = "diagram-sequence")]
pub(super) fn canonical_sequence(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::sequence::parse_sequence_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Sequence),
    )
}

#[cfg(feature = "diagram-cynefin")]
pub(super) fn canonical_cynefin(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::cynefin::parse_cynefin_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Cynefin),
    )
}

#[cfg(feature = "diagram-info")]
pub(super) fn canonical_info(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::info::parse_info_semantic_model_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::Info))
}

#[cfg(feature = "diagram-pie")]
pub(super) fn canonical_pie(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::pie::parse_pie_semantic_model_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::Pie))
}

#[cfg(feature = "diagram-journey")]
pub(super) fn canonical_journey(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::journey::parse_journey_semantic_model_controlled(
            code,
            &metadata.diagram_type,
            &metadata.semantic_config,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Journey),
    )
}

#[cfg(feature = "diagram-state")]
pub(super) fn canonical_state(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::state::parse_state_semantic_model_controlled(
            code,
            &metadata.diagram_type,
            &metadata.semantic_config,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::State),
    )
}

#[cfg(feature = "diagram-timeline")]
pub(super) fn canonical_timeline(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::timeline::parse_timeline_semantic_model_controlled(
            code,
            &metadata.diagram_type,
            &metadata.semantic_config,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Timeline),
    )
}

#[cfg(feature = "diagram-quadrant-chart")]
pub(super) fn canonical_quadrant_chart(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::quadrant_chart::parse_quadrant_chart_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::QuadrantChart),
    )
}

#[cfg(feature = "diagram-radar")]
pub(super) fn canonical_radar(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::radar::parse_radar_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Radar),
    )
}

#[cfg(feature = "diagram-sankey")]
pub(super) fn canonical_sankey(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::sankey::parse_sankey_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Sankey),
    )
}

#[cfg(feature = "diagram-venn")]
pub(super) fn canonical_venn(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::venn::parse_venn_semantic_model_controlled(
        code,
        metadata,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::Venn))
}

#[cfg(feature = "diagram-requirement")]
pub(super) fn canonical_requirement(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::requirement::parse_requirement_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Requirement),
    )
}

#[cfg(feature = "diagram-usecase")]
pub(super) fn canonical_usecase(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::usecase::parse_usecase_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Usecase),
    )
}

#[cfg(feature = "diagram-event-modeling")]
pub(super) fn canonical_eventmodeling(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::eventmodeling::parse_eventmodeling_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::EventModeling),
    )
}

#[cfg(feature = "diagram-wardley")]
pub(super) fn canonical_wardley(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::wardley::parse_wardley_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Wardley),
    )
}

#[cfg(feature = "diagram-railroad")]
pub(super) fn canonical_railroad(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::railroad::parse_railroad_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Railroad),
    )
}

#[cfg(feature = "diagram-railroad")]
pub(super) fn canonical_railroad_ebnf(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::railroad::parse_railroad_ebnf_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Railroad),
    )
}

#[cfg(feature = "diagram-railroad")]
pub(super) fn canonical_railroad_abnf(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::railroad::parse_railroad_abnf_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Railroad),
    )
}

#[cfg(feature = "diagram-railroad")]
pub(super) fn canonical_railroad_peg(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::railroad::parse_railroad_peg_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Railroad),
    )
}

#[cfg(feature = "diagram-zenuml")]
pub(super) fn canonical_zenuml(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::zenuml::parse_zenuml_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Zenuml),
    )
}

#[cfg(feature = "diagram-er")]
pub(super) fn canonical_er(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(crate::diagrams::er::parse_er_semantic_model_controlled(
        code,
        metadata,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(crate::SemanticDiagramModel::Er))
}

#[cfg(feature = "diagram-architecture")]
pub(super) fn canonical_architecture(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::architecture::parse_architecture_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Architecture),
    )
}

#[cfg(feature = "diagram-gantt")]
pub(super) fn canonical_gantt(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: super::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<super::SemanticConstruction<crate::SemanticDiagramModel>> {
    Ok(
        crate::diagrams::gantt::parse_gantt_semantic_model_controlled(
            code,
            metadata,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            control,
        )?
        .map_model(crate::SemanticDiagramModel::Gantt),
    )
}
