//! Full family data and explicitly declared custom JSON values.

use crate::ManagedJson as ManagedSemanticJson;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Declared producer path for a named custom JSON value.
///
/// This declaration does not certify source parsing or allow a custom value to use a built-in handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomJsonProvenance {
    /// Produced by a custom semantic JSON parser because no custom render parser was registered.
    #[cfg(feature = "compat-json")]
    SemanticRegistryOverlay,
    /// Produced by an explicitly registered custom render-model parser.
    RenderRegistryOverlay,
}

/// Explicit non-built-in JSON model boundary for custom parser adapters.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomJsonRenderModel {
    model_name: String,
    value: ManagedSemanticJson,
    provenance: CustomJsonProvenance,
}

impl CustomJsonRenderModel {
    /// Creates named JSON data with a declared render-overlay producer path.
    pub fn new(model_name: impl Into<String>, value: impl Into<ManagedSemanticJson>) -> Self {
        Self {
            model_name: model_name.into(),
            value: value.into(),
            provenance: CustomJsonProvenance::RenderRegistryOverlay,
        }
    }

    /// Constructs named JSON with an explicit producer declaration, without source authority.
    pub fn with_provenance(
        model_name: impl Into<String>,
        value: impl Into<ManagedSemanticJson>,
        provenance: CustomJsonProvenance,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            value: value.into(),
            provenance,
        }
    }

    /// Returns the adapter-defined model name.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Returns the custom JSON payload.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Borrows the managed payload for iterative cloning and export.
    pub fn json(&self) -> &ManagedSemanticJson {
        &self.value
    }

    /// Borrows mutable custom data while preserving its controlled JSON lifecycle owner.
    pub fn json_mut(&mut self) -> &mut ManagedSemanticJson {
        &mut self.value
    }

    /// Consumes the adapter while retaining managed ownership.
    pub fn into_json(self) -> ManagedSemanticJson {
        self.value
    }

    /// Extracts raw JSON; subsequent deep lifecycle operations are the caller's responsibility.
    pub fn into_unmanaged_value(self) -> Value {
        self.value.into_unmanaged_value()
    }

    /// Returns which custom registry path produced the model.
    pub fn provenance(&self) -> CustomJsonProvenance {
        self.provenance
    }
}

/// Typed semantic model used by the headless renderer.
///
/// Retains family-owned typed data for headless rendering without a JSON round trip.
///
/// Family variants and their payload modules are compiled only when the owning `diagram-*`
/// feature is enabled. `Flowchart` is shared by `diagram-flowchart` and `diagram-swimlane`;
/// sharing that payload does not enable the other language. `Error` and `CustomJson` remain
/// available in infrastructure-only builds.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "Family selection changes variant size ratios; keep the public typed payloads unboxed."
)]
pub enum RenderSemanticModel {
    Error(crate::families::error::ErrorDiagramModel),
    CustomJson(CustomJsonRenderModel),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(crate::families::mindmap::MindmapDiagramRenderModel),
    #[cfg(feature = "diagram-state")]
    State(crate::families::state::StateDiagramRenderModel),
    #[cfg(feature = "diagram-sequence")]
    Sequence(crate::families::sequence::SequenceDiagramModel),
    #[cfg(feature = "diagram-zenuml")]
    Zenuml(crate::families::zenuml::ZenumlDiagramModel),
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    Flowchart(crate::families::flowchart::FlowchartModel),
    #[cfg(feature = "diagram-architecture")]
    Architecture(crate::families::architecture::ArchitectureDiagramModel),
    #[cfg(feature = "diagram-class")]
    Class(crate::families::class::ClassDiagram),
    #[cfg(feature = "diagram-c4")]
    C4(crate::families::c4::C4DiagramModel),
    #[cfg(feature = "diagram-cynefin")]
    Cynefin(crate::families::cynefin::CynefinDiagramModel),
    #[cfg(feature = "diagram-railroad")]
    Railroad(crate::families::railroad::RailroadDiagramModel),
    #[cfg(feature = "diagram-kanban")]
    Kanban(crate::families::kanban::KanbanDiagramRenderModel),
    #[cfg(feature = "diagram-gantt")]
    Gantt(crate::families::gantt::GanttDiagramRenderModel),
    #[cfg(feature = "diagram-pie")]
    Pie(crate::families::pie::PieDiagramModel),
    #[cfg(feature = "diagram-packet")]
    Packet(crate::families::packet::PacketDiagramModel),
    #[cfg(feature = "diagram-timeline")]
    Timeline(crate::families::timeline::TimelineDiagramModel),
    #[cfg(feature = "diagram-journey")]
    Journey(crate::families::journey::JourneyDiagramModel),
    #[cfg(feature = "diagram-requirement")]
    Requirement(crate::families::requirement::RequirementDiagramModel),
    #[cfg(feature = "diagram-sankey")]
    Sankey(crate::families::sankey::SankeyDiagramModel),
    #[cfg(feature = "diagram-radar")]
    Radar(crate::families::radar::RadarDiagramModel),
    #[cfg(feature = "diagram-info")]
    Info(crate::families::info::InfoDiagramModel),
    #[cfg(feature = "diagram-treemap")]
    Treemap(crate::families::treemap::TreemapDiagramRenderModel),
    #[cfg(feature = "diagram-block")]
    Block(crate::families::block::BlockDiagramRenderModel),
    #[cfg(feature = "diagram-er")]
    Er(crate::families::er::ErDiagramModel),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChart(crate::families::quadrant_chart::QuadrantChartDiagramModel),
    #[cfg(feature = "diagram-xychart")]
    XyChart(crate::families::xy_chart::XyChartDiagramRenderModel),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(crate::families::git_graph::GitGraphDiagramModel),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(crate::families::tree_view::TreeViewDiagramModel),
    #[cfg(feature = "diagram-ishikawa")]
    Ishikawa(crate::families::ishikawa::IshikawaDiagramModel),
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling(crate::families::event_modeling::EventModelingDiagramModel),
    #[cfg(feature = "diagram-venn")]
    Venn(crate::families::venn::VennDiagramModel),
    #[cfg(feature = "diagram-wardley")]
    Wardley(crate::families::wardley::WardleyDiagramModel),
    #[cfg(feature = "diagram-usecase")]
    Usecase(crate::families::usecase::UsecaseDiagramModel),
    #[cfg(feature = "diagram-agentflow")]
    Agentflow(crate::families::agentflow::AgentflowDiagramRenderModel),
}

impl RenderSemanticModel {
    /// Returns a stable family label for diagnostics and timing output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Error(_) => "error",
            Self::CustomJson(_) => "custom-json",
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(_) => "mindmap",
            #[cfg(feature = "diagram-state")]
            Self::State(_) => "state",
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(_) => "sequence",
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(_) => "zenuml",
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::Flowchart(_) => "flowchart",
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(_) => "architecture",
            #[cfg(feature = "diagram-class")]
            Self::Class(_) => "class",
            #[cfg(feature = "diagram-c4")]
            Self::C4(_) => "c4",
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(_) => "cynefin",
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(_) => "railroad",
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(_) => "kanban",
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(_) => "gantt",
            #[cfg(feature = "diagram-pie")]
            Self::Pie(_) => "pie",
            #[cfg(feature = "diagram-packet")]
            Self::Packet(_) => "packet",
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(_) => "timeline",
            #[cfg(feature = "diagram-journey")]
            Self::Journey(_) => "journey",
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(_) => "requirement",
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(_) => "sankey",
            #[cfg(feature = "diagram-radar")]
            Self::Radar(_) => "radar",
            #[cfg(feature = "diagram-info")]
            Self::Info(_) => "info",
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(_) => "treemap",
            #[cfg(feature = "diagram-block")]
            Self::Block(_) => "block",
            #[cfg(feature = "diagram-er")]
            Self::Er(_) => "er",
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(_) => "quadrantChart",
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(_) => "xychart",
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(_) => "gitGraph",
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(_) => "treeView",
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(_) => "ishikawa",
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(_) => "eventmodeling",
            #[cfg(feature = "diagram-venn")]
            Self::Venn(_) => "venn",
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(_) => "wardley",
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(_) => "usecase",
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(_) => "agentflow",
        }
    }

    /// Checks the pinned data association without querying parser or output availability.
    pub fn matches_diagram_type(&self, diagram_type: &str) -> bool {
        match self {
            Self::CustomJson(model) => {
                crate::family::find_variant(diagram_type).is_none()
                    && model.model_name() == diagram_type
            }
            model => {
                crate::family::render_model_kind_supports_diagram_type(model.kind(), diagram_type)
            }
        }
    }
}
