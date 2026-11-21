//! Canonical semantic family data and declared metadata.

use crate::MermaidConfig;

/// Configuration and declared source metadata used to interpret a diagram's meaning.
///
/// This value can be assembled by callers; possessing it does not certify source admission.
#[derive(Debug, Clone)]
pub struct SemanticMetadata {
    pub diagram_type: String,
    /// Source-authored config projection used by Mermaid's public parse metadata. It follows
    /// upstream secure keys unless the host explicitly supplies its own `secure` policy.
    pub config: MermaidConfig,
    /// Effective semantic config after the site's secure-key and CSS-value admission, before
    /// theme materialization.
    pub semantic_config: MermaidConfig,
    pub title: Option<String>,
}

/// A typed semantic result for the selected built-in diagram families.
#[non_exhaustive]
#[derive(Debug)]
pub enum SemanticDiagramModel {
    /// Canonical infrastructure model for an explicit or suppressed parser error.
    Error(crate::families::error::ErrorDiagramModel),
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    Flowchart(crate::families::flowchart::FlowchartModel),
    #[cfg(feature = "diagram-sequence")]
    Sequence(crate::families::sequence::SequenceDiagramModel),
    #[cfg(feature = "diagram-state")]
    State(crate::families::state::StateDiagramModel),
    #[cfg(feature = "diagram-timeline")]
    Timeline(crate::families::timeline::TimelineDiagramModel),
    #[cfg(feature = "diagram-cynefin")]
    Cynefin(crate::families::cynefin::CynefinDiagramModel),
    #[cfg(feature = "diagram-info")]
    Info(crate::families::info::InfoDiagramModel),
    #[cfg(feature = "diagram-journey")]
    Journey(crate::families::journey::JourneyDiagramModel),
    #[cfg(feature = "diagram-pie")]
    Pie(crate::families::pie::PieDiagramModel),
    #[cfg(feature = "diagram-packet")]
    Packet(crate::families::packet::PacketDiagramModel),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(crate::families::tree_view::TreeViewDiagramModel),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChart(crate::families::quadrant_chart::QuadrantChartDiagramModel),
    #[cfg(feature = "diagram-radar")]
    Radar(crate::families::radar::RadarDiagramModel),
    #[cfg(feature = "diagram-sankey")]
    Sankey(crate::families::sankey::SankeyDiagramModel),
    #[cfg(feature = "diagram-venn")]
    Venn(crate::families::venn::VennDiagramModel),
    #[cfg(feature = "diagram-requirement")]
    Requirement(crate::families::requirement::RequirementDiagramModel),
    #[cfg(feature = "diagram-usecase")]
    Usecase(crate::families::usecase::UsecaseDiagramModel),
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling(crate::families::event_modeling::EventModelingDiagramModel),
    #[cfg(feature = "diagram-wardley")]
    Wardley(crate::families::wardley::WardleyDiagramModel),
    #[cfg(feature = "diagram-railroad")]
    Railroad(crate::families::railroad::RailroadDiagramModel),
    #[cfg(feature = "diagram-zenuml")]
    Zenuml(crate::families::zenuml::ZenumlDiagramModel),
    #[cfg(feature = "diagram-er")]
    Er(crate::families::er::ErDiagramModel),
    #[cfg(feature = "diagram-architecture")]
    Architecture(crate::families::architecture::ArchitectureDiagramModel),
    #[cfg(feature = "diagram-gantt")]
    Gantt(crate::families::gantt::GanttDiagramModel),
    #[cfg(feature = "diagram-c4")]
    C4(crate::families::c4::C4DiagramModel),
    #[cfg(feature = "diagram-ishikawa")]
    Ishikawa(crate::families::ishikawa::IshikawaDiagramModel),
    #[cfg(feature = "diagram-treemap")]
    Treemap(crate::families::treemap::TreemapDiagramModel),
    #[cfg(feature = "diagram-block")]
    Block(crate::families::block::BlockDiagramModel),
    #[cfg(feature = "diagram-kanban")]
    Kanban(crate::families::kanban::KanbanDiagramModel),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(crate::families::mindmap::MindmapDiagramModel),
    #[cfg(feature = "diagram-xychart")]
    XyChart(crate::families::xy_chart::XyChartDiagramModel),
    #[cfg(feature = "diagram-agentflow")]
    Agentflow(crate::families::agentflow::AgentflowDiagramModel),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(crate::families::git_graph::GitGraphDiagramModel),
    #[cfg(feature = "diagram-class")]
    Class(crate::families::class::ClassDiagramModel),
}

impl SemanticDiagramModel {
    /// Consumes the model and returns its source-mapped semantic warning facts.
    pub fn into_warning_facts(self) -> Vec<crate::warnings::DiagramWarningFact> {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(model) => model.warning_facts,
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::Flowchart(model) => model.warning_facts,
            #[cfg(feature = "diagram-block")]
            Self::Block(model) => model.warning_facts,
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => model.warning_facts,
            _ => Vec::new(),
        }
    }
}

/// Metadata and canonical family data returned by semantic construction.
#[derive(Debug)]
pub struct ParsedSemanticDiagram {
    pub metadata: SemanticMetadata,
    pub model: SemanticDiagramModel,
}

#[cfg(all(test, feature = "diagram-agentflow"))]
mod tests {
    #[test]
    fn shared_flowchart_payload_is_usable_without_selecting_a_source_language() {
        let flowchart = serde_json::from_value::<crate::families::flowchart::FlowchartModel>(
            serde_json::json!({"nodes": [], "edges": []}),
        )
        .unwrap();
        let expected =
            crate::resources::FlowchartComplexity::from_model(&flowchart).as_model_complexity();
        let model = super::SemanticDiagramModel::Flowchart(flowchart);
        assert_eq!(
            crate::resources::model_complexity_from_semantic_model(&model),
            expected
        );
        assert!(model.into_warning_facts().is_empty());
    }
}
