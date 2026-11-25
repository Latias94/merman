mod capability;
pub(crate) mod native_typography;
mod preparation;

pub use capability::{RenderCapabilityPlan, plan_render, supports_diagram_type};
#[doc(hidden)]
pub use capability::{has_edge_geometry, plan_render_with_options, validate_layout_options};

use crate::environment::{RenderSession, RenderSessionReport};
use crate::layout_backend::GraphLayoutBackend;
use crate::math::PreparedMathEvidenceLease;
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    FlowchartEdgeTraceCollector, ResvgCompatibleSvg, StandaloneSvgArtifact, SvgDebugOptions,
    SvgPipeline, SvgPipelinePreset, SvgPostprocessExecution, SvgPostprocessMetadata,
    SvgRenderOptions,
};
use crate::theme::ThemePortabilityRequirement;
#[cfg(feature = "diagram-wardley")]
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, RenderCapability, Result};
use merman_model::ManagedJson;
use merman_model::OperationPhase;
use merman_model::families as diagrams;
#[cfg(feature = "diagram-class")]
use merman_model::families::class::ClassDiagram;
use merman_model::family::DiagramFamilyId;
use merman_model::presentation::PresentationContext;
use merman_model::presentation::{
    BuiltinRenderSemantic, PresentationDiagram, PresentationMetadata, RenderSemanticModel,
};
use std::sync::Arc;

/// Stable identity for a built-in typed render family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderFamilyKind {
    Error,
    Mindmap,
    State,
    Sequence,
    Zenuml,
    Flowchart,
    Swimlane,
    Architecture,
    Class,
    C4,
    Cynefin,
    Wardley,
    Railroad,
    Kanban,
    Gantt,
    Pie,
    Packet,
    Timeline,
    Journey,
    Requirement,
    Sankey,
    Radar,
    Info,
    Treemap,
    Block,
    Er,
    QuadrantChart,
    XyChart,
    GitGraph,
    TreeView,
    Ishikawa,
    EventModeling,
    Venn,
    Usecase,
    Agentflow,
}

impl RenderFamilyKind {
    fn from_family_id(family_id: DiagramFamilyId) -> Self {
        match family_id {
            DiagramFamilyId::MINDMAP => RenderFamilyKind::Mindmap,
            DiagramFamilyId::STATE => RenderFamilyKind::State,
            DiagramFamilyId::SEQUENCE => RenderFamilyKind::Sequence,
            DiagramFamilyId::ZENUML => RenderFamilyKind::Zenuml,
            DiagramFamilyId::FLOWCHART => RenderFamilyKind::Flowchart,
            DiagramFamilyId::SWIMLANE => RenderFamilyKind::Swimlane,
            DiagramFamilyId::ARCHITECTURE => RenderFamilyKind::Architecture,
            DiagramFamilyId::CLASS => RenderFamilyKind::Class,
            DiagramFamilyId::C4 => RenderFamilyKind::C4,
            DiagramFamilyId::CYNEFIN => RenderFamilyKind::Cynefin,
            DiagramFamilyId::WARDLEY => RenderFamilyKind::Wardley,
            DiagramFamilyId::RAILROAD => RenderFamilyKind::Railroad,
            DiagramFamilyId::KANBAN => RenderFamilyKind::Kanban,
            DiagramFamilyId::GANTT => RenderFamilyKind::Gantt,
            DiagramFamilyId::PIE => RenderFamilyKind::Pie,
            DiagramFamilyId::PACKET => RenderFamilyKind::Packet,
            DiagramFamilyId::TIMELINE => RenderFamilyKind::Timeline,
            DiagramFamilyId::JOURNEY => RenderFamilyKind::Journey,
            DiagramFamilyId::REQUIREMENT => RenderFamilyKind::Requirement,
            DiagramFamilyId::SANKEY => RenderFamilyKind::Sankey,
            DiagramFamilyId::RADAR => RenderFamilyKind::Radar,
            DiagramFamilyId::INFO => RenderFamilyKind::Info,
            DiagramFamilyId::TREEMAP => RenderFamilyKind::Treemap,
            DiagramFamilyId::BLOCK => RenderFamilyKind::Block,
            DiagramFamilyId::ER => RenderFamilyKind::Er,
            DiagramFamilyId::QUADRANT_CHART => RenderFamilyKind::QuadrantChart,
            DiagramFamilyId::XY_CHART => RenderFamilyKind::XyChart,
            DiagramFamilyId::GIT_GRAPH => RenderFamilyKind::GitGraph,
            DiagramFamilyId::TREE_VIEW => RenderFamilyKind::TreeView,
            DiagramFamilyId::ISHIKAWA => RenderFamilyKind::Ishikawa,
            DiagramFamilyId::EVENT_MODELING => RenderFamilyKind::EventModeling,
            DiagramFamilyId::VENN => RenderFamilyKind::Venn,
            DiagramFamilyId::USECASE => RenderFamilyKind::Usecase,
            DiagramFamilyId::AGENTFLOW => RenderFamilyKind::Agentflow,
            DiagramFamilyId::ERROR => RenderFamilyKind::Error,
            _ => unreachable!("built-in artifact has a built-in family identity"),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Mindmap => "mindmap",
            Self::State => "state",
            Self::Sequence => "sequence",
            Self::Zenuml => "zenuml",
            Self::Flowchart => "flowchart",
            Self::Swimlane => "swimlane",
            Self::Architecture => "architecture",
            Self::Class => "class",
            Self::C4 => "c4",
            Self::Cynefin => "cynefin",
            Self::Wardley => "wardley",
            Self::Railroad => "railroad",
            Self::Kanban => "kanban",
            Self::Gantt => "gantt",
            Self::Pie => "pie",
            Self::Packet => "packet",
            Self::Timeline => "timeline",
            Self::Journey => "journey",
            Self::Requirement => "requirement",
            Self::Sankey => "sankey",
            Self::Radar => "radar",
            Self::Info => "info",
            Self::Treemap => "treemap",
            Self::Block => "block",
            Self::Er => "er",
            Self::QuadrantChart => "quadrantChart",
            Self::XyChart => "xychart",
            Self::GitGraph => "gitGraph",
            Self::TreeView => "treeView",
            Self::Ishikawa => "ishikawa",
            Self::EventModeling => "eventmodeling",
            Self::Venn => "venn",
            Self::Usecase => "usecase",
            Self::Agentflow => "agentflow",
        }
    }
}

impl std::fmt::Display for RenderFamilyKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Family identity and operation report frozen after SVG work completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyRenderReport {
    family_id: DiagramFamilyId,
    session: RenderSessionReport,
    output_mutated: bool,
}

impl FamilyRenderReport {
    fn freeze(family_id: DiagramFamilyId, session: RenderSession, output_mutated: bool) -> Self {
        Self {
            family_id,
            session: session.report(),
            output_mutated,
        }
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        RenderFamilyKind::from_family_id(self.family_id)
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    /// Whether document CSS or a custom postprocessor changed the generated output.
    pub const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    pub const fn session_report(&self) -> &RenderSessionReport {
        &self.session
    }

    pub fn into_session_report(self) -> RenderSessionReport {
        self.session
    }
}

struct FamilyRenderContext {
    layout_backend: GraphLayoutBackend,
    family_id: DiagramFamilyId,
    session: RenderSession,
}

#[derive(Clone, Copy)]
pub(crate) struct FamilyExecutionView<'a> {
    layout_backend: GraphLayoutBackend,
    family_id: DiagramFamilyId,
    session: &'a RenderSession,
}

impl<'a> FamilyExecutionView<'a> {
    pub(crate) const fn family_id(self) -> DiagramFamilyId {
        self.family_id
    }

    pub(crate) const fn layout_backend(self) -> GraphLayoutBackend {
        self.layout_backend
    }

    pub(crate) const fn session(self) -> &'a RenderSession {
        self.session
    }

    pub(crate) fn native_theme(self) -> Option<&'a crate::theme::FamilyTheme> {
        self.session
            .native_theme()
            .map(|theme| theme.resolved().family(self.family_id.as_str()))
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        session: &'a RenderSession,
        family_id: DiagramFamilyId,
        layout_backend: GraphLayoutBackend,
    ) -> Self {
        Self {
            layout_backend,
            family_id,
            session,
        }
    }
}

impl FamilyRenderContext {
    const fn resolve(
        session: RenderSession,
        family_id: DiagramFamilyId,
        layout_backend: GraphLayoutBackend,
    ) -> Self {
        Self {
            layout_backend,
            family_id,
            session,
        }
    }

    const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    const fn session(&self) -> &RenderSession {
        &self.session
    }

    const fn execution(&self) -> FamilyExecutionView<'_> {
        FamilyExecutionView {
            layout_backend: self.layout_backend,
            family_id: self.family_id,
            session: &self.session,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FamilyPair<S, L> {
    semantic: S,
    layout: L,
}

impl<S: std::ops::Deref, L> FamilyPair<S, L> {
    fn new(semantic: S, layout: L) -> Self {
        Self { semantic, layout }
    }

    pub(crate) fn semantic(&self) -> &S::Target {
        &self.semantic
    }

    pub(crate) fn layout(&self) -> &L {
        &self.layout
    }
}

impl<S: std::ops::Deref, L> FamilyPair<S, L>
where
    S::Target: BuiltinRenderSemantic,
{
    fn compatibility_json(
        &self,
        metadata: &PresentationMetadata,
        output_context: &PresentationContext,
        control: &merman_model::OperationControl,
    ) -> Result<ManagedJson> {
        self.semantic
            .compatibility_json_with_context_controlled(metadata, output_context, control)?
            .map_err(Error::SemanticProjection)
    }
}

#[derive(Debug)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) struct FlowchartFamilyArtifact<'parsed, L> {
    pair: FamilyPair<std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartModel>, L>,
    render_context: std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartRenderContext>,
    edge_style_plan: crate::svg::FlowchartEdgeStylePlan,
    prepared_theme: crate::flowchart::FlowchartPreparedTheme,
    prepared_nodes: Option<crate::svg::FlowchartPreparedNodes>,
    render_config: crate::svg::FlowchartRenderConfig,
    svg_label_sidecar: crate::flowchart::FlowchartSvgLabelSidecar,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
impl<'parsed, L> FlowchartFamilyArtifact<'parsed, L> {
    pub(crate) const fn render_config(&self) -> &crate::svg::FlowchartRenderConfig {
        &self.render_config
    }

    pub(crate) fn prepared_nodes(&self) -> Result<&crate::svg::FlowchartPreparedNodes> {
        self.prepared_nodes
            .as_ref()
            .ok_or_else(|| Error::InvalidModel {
                message: "Flowchart node paint was not prepared for SVG".into(),
            })
    }

    pub(crate) fn prepared_theme(&self) -> &crate::flowchart::FlowchartPreparedTheme {
        &self.prepared_theme
    }

    pub(crate) fn pair(
        &self,
    ) -> &FamilyPair<std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartModel>, L> {
        &self.pair
    }

    pub(crate) fn render_context(&self) -> &diagrams::flowchart::FlowchartRenderContext {
        &self.render_context
    }

    pub(crate) const fn edge_style_plan(&self) -> &crate::svg::FlowchartEdgeStylePlan {
        &self.edge_style_plan
    }

    pub(crate) fn svg_label_sidecar(&self) -> &crate::flowchart::FlowchartSvgLabelSidecar {
        &self.svg_label_sidecar
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-class")]
pub(crate) struct ClassFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed ClassDiagram, ClassDiagramLayout>,
    prepared_labels: crate::class::ClassPreparedLabels,
    route_contract: crate::graph_layout::RouteContract,
    relation_theme: crate::class::ClassRelationThemePlan,
    typography_theme: crate::class::ClassTextThemePlan,
    render_config: crate::class::ClassRenderConfig,
    node_visual_plan: crate::class::ClassNodeVisualPlan,
}

#[derive(Debug)]
#[cfg(feature = "diagram-c4")]
pub(crate) struct C4FamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::c4::C4DiagramModel, C4DiagramLayout>,
    cluster_theme: crate::c4::C4ClusterThemePlan,
    typography_theme: crate::c4::C4TypographyThemePlan,
    text_paint: crate::c4::C4TextPaintPlan,
    #[cfg(feature = "layout-shoal")]
    shoal: Option<crate::c4::C4ShoalGeometry>,
}

#[cfg(feature = "diagram-c4")]
impl<'parsed> C4FamilyArtifact<'parsed> {
    pub(crate) const fn text_paint(&self) -> &crate::c4::C4TextPaintPlan {
        &self.text_paint
    }
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::c4::C4DiagramModel, C4DiagramLayout> {
        &self.pair
    }

    pub(crate) const fn cluster_theme(&self) -> &crate::c4::C4ClusterThemePlan {
        &self.cluster_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::c4::C4TypographyThemePlan {
        &self.typography_theme
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn shoal(&self) -> Option<&crate::c4::C4ShoalGeometry> {
        self.shoal.as_ref()
    }
}

#[cfg(feature = "diagram-class")]
impl<'parsed> ClassFamilyArtifact<'parsed> {
    pub(crate) const fn prepared_labels(&self) -> &crate::class::ClassPreparedLabels {
        &self.prepared_labels
    }

    pub(crate) const fn route_contract(&self) -> &crate::graph_layout::RouteContract {
        &self.route_contract
    }

    pub(crate) const fn render_config(&self) -> &crate::class::ClassRenderConfig {
        &self.render_config
    }

    pub(crate) const fn node_visual_plan(&self) -> &crate::class::ClassNodeVisualPlan {
        &self.node_visual_plan
    }

    pub(crate) const fn pair(&self) -> &FamilyPair<&'parsed ClassDiagram, ClassDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn relation_theme(&self) -> &crate::class::ClassRelationThemePlan {
        &self.relation_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::class::ClassTextThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-gantt")]
pub(crate) struct GanttFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout>,
    style: crate::gantt::GanttStyle,
    task_source_indices: Vec<usize>,
}

#[derive(Debug)]
#[cfg(feature = "diagram-pie")]
pub(crate) struct PieFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::pie::PieDiagramModel, PieDiagramLayout>,
    appearance: crate::pie::PieAppearance,
}

#[derive(Debug)]
#[cfg(feature = "diagram-timeline")]
pub(crate) struct TimelineFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::timeline::TimelineDiagramModel, TimelineDiagramLayout>,
    style: crate::timeline::TimelineStyle,
}

#[derive(Debug)]
#[cfg(feature = "diagram-journey")]
pub(crate) struct JourneyFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::journey::JourneyDiagramModel, JourneyDiagramLayout>,
    style: crate::journey::JourneyStyle,
}

#[cfg(feature = "diagram-journey")]
impl<'parsed> JourneyFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::journey::JourneyDiagramModel, JourneyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::journey::JourneyStyle {
        &self.style
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-quadrant-chart")]
pub(crate) struct QuadrantChartFamilyArtifact<'parsed> {
    pair: FamilyPair<
        &'parsed diagrams::quadrant_chart::QuadrantChartDiagramModel,
        QuadrantChartDiagramLayout,
    >,
    point_theme: crate::quadrantchart::QuadrantChartPointThemePlan,
}

#[cfg(feature = "diagram-quadrant-chart")]
impl<'parsed> QuadrantChartFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        &'parsed diagrams::quadrant_chart::QuadrantChartDiagramModel,
        QuadrantChartDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn point_theme(&self) -> &crate::quadrantchart::QuadrantChartPointThemePlan {
        &self.point_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-xychart")]
pub(crate) struct XyChartFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::xy_chart::XyChartDiagramRenderModel, XyChartDiagramLayout>,
    typography_theme: crate::xychart::XyChartTypographyThemePlan,
    bar_legend_plots: Vec<usize>,
    data_label_color: String,
}

#[derive(Debug)]
#[cfg(feature = "diagram-radar")]
pub(crate) struct RadarFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::radar::RadarDiagramModel, RadarDiagramLayout>,
    series_paint: crate::radar::RadarSeriesPaintPlan,
    typography_theme: crate::radar::RadarTypographyThemePlan,
}

#[derive(Debug)]
#[cfg(feature = "diagram-sankey")]
pub(crate) struct SankeyFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::sankey::SankeyDiagramModel, SankeyDiagramLayout>,
    style: crate::sankey::SankeyStyle,
}

#[derive(Debug)]
#[cfg(feature = "diagram-venn")]
pub(crate) struct VennFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::venn::VennDiagramModel, VennDiagramLayout>,
    typography_theme: crate::venn::VennTypographyThemePlan,
}

#[derive(Debug)]
#[cfg(feature = "diagram-zenuml")]
pub(crate) struct ZenumlFamilyArtifact<'parsed> {
    pair: FamilyPair<
        &'parsed diagrams::zenuml::ZenumlDiagramModel,
        crate::zenuml::ZenumlDiagramLayout,
    >,
    style: crate::zenuml::ZenumlSourceStyle,
}

#[cfg(feature = "diagram-zenuml")]
impl<'parsed> ZenumlFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        &'parsed diagrams::zenuml::ZenumlDiagramModel,
        crate::zenuml::ZenumlDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::zenuml::ZenumlSourceStyle {
        &self.style
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-event-modeling")]
pub(crate) struct EventModelingFamilyArtifact<'parsed> {
    pair: FamilyPair<
        &'parsed diagrams::event_modeling::EventModelingDiagramModel,
        EventModelingDiagramLayout,
    >,
    style: crate::eventmodeling::EventModelingStyle,
    prepared_labels: crate::eventmodeling::EventModelingPreparedLabels,
}

#[derive(Debug)]
#[cfg(feature = "diagram-ishikawa")]
pub(crate) struct IshikawaFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::ishikawa::IshikawaDiagramModel, IshikawaDiagramLayout>,
    text_theme: crate::ishikawa::IshikawaTextThemePlan,
}

#[cfg(feature = "diagram-ishikawa")]
impl<'parsed> IshikawaFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::ishikawa::IshikawaDiagramModel, IshikawaDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn text_theme(&self) -> &crate::ishikawa::IshikawaTextThemePlan {
        &self.text_theme
    }
}

#[cfg(feature = "diagram-event-modeling")]
impl<'parsed> EventModelingFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        &'parsed diagrams::event_modeling::EventModelingDiagramModel,
        EventModelingDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn prepared_labels(
        &self,
    ) -> &crate::eventmodeling::EventModelingPreparedLabels {
        &self.prepared_labels
    }

    pub(crate) const fn style(&self) -> &crate::eventmodeling::EventModelingStyle {
        &self.style
    }
}

#[cfg(feature = "diagram-venn")]
impl<'parsed> VennFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::venn::VennDiagramModel, VennDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::venn::VennTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-block")]
pub(crate) struct BlockFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout>,
    prepared_labels: crate::block::BlockPreparedLabels,
    node_bindings: crate::block::BlockNodeBindings,
    edge_routes: crate::block::BlockEdgeRoutes,
    typography_theme: crate::block::BlockTypographyThemePlan,
}

#[cfg(feature = "diagram-block")]
impl<'parsed> BlockFamilyArtifact<'parsed> {
    pub(crate) const fn prepared_labels(&self) -> &crate::block::BlockPreparedLabels {
        &self.prepared_labels
    }

    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::block::BlockTypographyThemePlan {
        &self.typography_theme
    }

    pub(crate) const fn node_bindings(&self) -> &crate::block::BlockNodeBindings {
        &self.node_bindings
    }

    pub(crate) const fn edge_routes(&self) -> &crate::block::BlockEdgeRoutes {
        &self.edge_routes
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-railroad")]
pub(crate) struct RailroadFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::railroad::RailroadDiagramModel, RailroadDiagramLayout>,
    style: crate::railroad::RailroadStyle,
}

#[cfg(feature = "diagram-railroad")]
impl<'parsed> RailroadFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::railroad::RailroadDiagramModel, RailroadDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::railroad::RailroadStyle {
        &self.style
    }
}

#[derive(Debug)]
pub(crate) struct ErrorFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::error::ErrorDiagramModel, ErrorDiagramLayout>,
    typography_theme: crate::error::ErrorTypographyThemePlan,
}

impl<'parsed> ErrorFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::error::ErrorDiagramModel, ErrorDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::error::ErrorTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-info")]
pub(crate) struct InfoFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::info::InfoDiagramModel, InfoDiagramLayout>,
    typography_theme: crate::info::InfoAppearance,
}

#[cfg(feature = "diagram-info")]
impl<'parsed> InfoFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::info::InfoDiagramModel, InfoDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::info::InfoAppearance {
        &self.typography_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-cynefin")]
pub(crate) struct CynefinFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::cynefin::CynefinDiagramModel, CynefinDiagramLayout>,
    style: crate::cynefin::CynefinStyle,
}

#[cfg(feature = "diagram-cynefin")]
impl<'parsed> CynefinFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::cynefin::CynefinDiagramModel, CynefinDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::cynefin::CynefinStyle {
        &self.style
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-wardley")]
pub(crate) struct WardleyFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::wardley::WardleyDiagramModel, WardleyDiagramLayout>,
    style: crate::wardley::WardleyStyle,
}

#[cfg(feature = "diagram-wardley")]
impl<'parsed> WardleyFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::wardley::WardleyDiagramModel, WardleyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::wardley::WardleyStyle {
        &self.style
    }
}

#[cfg(feature = "diagram-sankey")]
impl<'parsed> SankeyFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::sankey::SankeyDiagramModel, SankeyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::sankey::SankeyStyle {
        &self.style
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-git-graph")]
pub(crate) struct GitGraphFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::git_graph::GitGraphDiagramModel, GitGraphDiagramLayout>,
    typography_theme: crate::gitgraph::GitGraphTypographyThemePlan,
}

#[cfg(feature = "diagram-git-graph")]
impl<'parsed> GitGraphFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::git_graph::GitGraphDiagramModel, GitGraphDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::gitgraph::GitGraphTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-treemap")]
pub(crate) struct TreemapFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout>,
    style: crate::treemap::TreemapStyle,
}

#[derive(Debug)]
#[cfg(feature = "diagram-requirement")]
pub(crate) struct RequirementFamilyArtifact<'parsed> {
    pair: FamilyPair<
        &'parsed diagrams::requirement::RequirementDiagramModel,
        crate::requirement::RequirementPreparedArtifact,
    >,
    paint_theme: crate::requirement::RequirementPaintThemePlan,
}

#[derive(Debug)]
#[cfg(feature = "diagram-packet")]
pub(crate) struct PacketFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::packet::PacketDiagramModel, PacketDiagramLayout>,
    typography_theme: crate::packet::PacketTypographyThemePlan,
}

#[cfg(feature = "diagram-packet")]
impl<'parsed> PacketFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::packet::PacketDiagramModel, PacketDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::packet::PacketTypographyThemePlan {
        &self.typography_theme
    }
}

#[cfg(feature = "diagram-requirement")]
impl<'parsed> RequirementFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        &'parsed diagrams::requirement::RequirementDiagramModel,
        crate::requirement::RequirementPreparedArtifact,
    > {
        &self.pair
    }

    pub(crate) const fn paint_theme(&self) -> &crate::requirement::RequirementPaintThemePlan {
        &self.paint_theme
    }
}

#[cfg(feature = "diagram-treemap")]
impl<'parsed> TreemapFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::treemap::TreemapStyle {
        &self.style
    }
}

#[cfg(feature = "diagram-radar")]
impl<'parsed> RadarFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::radar::RadarDiagramModel, RadarDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn series_paint(&self) -> &crate::radar::RadarSeriesPaintPlan {
        &self.series_paint
    }

    pub(crate) const fn typography_theme(&self) -> &crate::radar::RadarTypographyThemePlan {
        &self.typography_theme
    }
}

#[cfg(feature = "diagram-xychart")]
impl<'parsed> XyChartFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::xy_chart::XyChartDiagramRenderModel, XyChartDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::xychart::XyChartTypographyThemePlan {
        &self.typography_theme
    }

    pub(crate) fn bar_legend_plots(&self) -> &[usize] {
        &self.bar_legend_plots
    }

    pub(crate) fn data_label_color(&self) -> &str {
        &self.data_label_color
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-tree-view")]
pub(crate) struct TreeViewFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::tree_view::TreeViewDiagramModel, TreeViewDiagramLayout>,
    theme: crate::tree_view::TreeViewCssBinding,
}

#[derive(Debug)]
#[cfg(feature = "diagram-mindmap")]
pub(crate) struct MindmapFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>,
    appearance: crate::mindmap::MindmapAppearance,
    prepared_labels: crate::mindmap::MindmapPreparedLabels,
}

#[cfg(feature = "diagram-mindmap")]
impl<'parsed> MindmapFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn appearance(&self) -> &crate::mindmap::MindmapAppearance {
        &self.appearance
    }

    pub(crate) const fn prepared_labels(&self) -> &crate::mindmap::MindmapPreparedLabels {
        &self.prepared_labels
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-er")]
pub(crate) struct ErFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::er::ErDiagramModel, ErDiagramLayout>,
    entity_theme: crate::er::ErEntityThemePlan,
    route_contract: crate::graph_layout::RouteContract,
}

#[cfg(feature = "diagram-er")]
impl<'parsed> ErFamilyArtifact<'parsed> {
    pub(crate) const fn route_contract(&self) -> &crate::graph_layout::RouteContract {
        &self.route_contract
    }

    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::er::ErDiagramModel, ErDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn entity_theme(&self) -> &crate::er::ErEntityThemePlan {
        &self.entity_theme
    }
}

#[cfg(feature = "diagram-pie")]
impl<'parsed> PieFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::pie::PieDiagramModel, PieDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn appearance(&self) -> &crate::pie::PieAppearance {
        &self.appearance
    }
}

#[cfg(feature = "diagram-timeline")]
impl<'parsed> TimelineFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::timeline::TimelineDiagramModel, TimelineDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::timeline::TimelineStyle {
        &self.style
    }
}

#[cfg(feature = "diagram-tree-view")]
impl<'parsed> TreeViewFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::tree_view::TreeViewDiagramModel, TreeViewDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn theme(&self) -> &crate::tree_view::TreeViewCssBinding {
        &self.theme
    }
}

#[cfg(feature = "diagram-gantt")]
impl<'parsed> GanttFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn style(&self) -> &crate::gantt::GanttStyle {
        &self.style
    }

    pub(crate) fn task_source_indices(&self) -> &[usize] {
        &self.task_source_indices
    }
}

#[derive(Debug)]
#[cfg(feature = "diagram-state")]
pub(crate) struct StateFamilyArtifact<'parsed> {
    pair: FamilyPair<&'parsed diagrams::state::StateDiagramRenderModel, StateDiagramLayout>,
    label_sidecar: crate::state::StateLabelSidecar,
    route_contract: crate::graph_layout::RouteContract,
    style: crate::state::StateStylePlan,
}

#[cfg(feature = "diagram-state")]
impl<'parsed> StateFamilyArtifact<'parsed> {
    pub(crate) fn pair(
        &self,
    ) -> &FamilyPair<&'parsed diagrams::state::StateDiagramRenderModel, StateDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn label_sidecar(&self) -> &crate::state::StateLabelSidecar {
        &self.label_sidecar
    }

    pub(crate) const fn route_contract(&self) -> &crate::graph_layout::RouteContract {
        &self.route_contract
    }

    pub(crate) const fn style(&self) -> &crate::state::StateStylePlan {
        &self.style
    }
}

#[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
#[derive(Debug)]
#[cfg(feature = "diagram-architecture")]
pub(crate) struct ArchitectureFamilyArtifact<'parsed> {
    pair: FamilyPair<
        &'parsed diagrams::architecture::ArchitectureDiagramModel,
        ArchitectureDiagramLayout,
    >,
    group_theme: crate::architecture::ArchitectureGroupThemePlan,
    #[cfg(feature = "layout-shoal")]
    shoal: Option<crate::architecture::ArchitectureShoalGeometry>,
}

#[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
#[cfg(feature = "diagram-architecture")]
impl<'parsed> ArchitectureFamilyArtifact<'parsed> {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        &'parsed diagrams::architecture::ArchitectureDiagramModel,
        ArchitectureDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn group_theme(&self) -> &crate::architecture::ArchitectureGroupThemePlan {
        &self.group_theme
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn shoal(&self) -> Option<&crate::architecture::ArchitectureShoalGeometry> {
        self.shoal.as_ref()
    }
}

#[derive(Debug)]
pub(crate) enum BuiltinFamilyArtifact<'parsed> {
    #[cfg(feature = "diagram-agentflow")]
    Agentflow {
        semantic: &'parsed diagrams::agentflow::AgentflowDiagramRenderModel,
        flow: Box<FlowchartFamilyArtifact<'parsed, FlowchartLayout>>,
        projected_metadata: Box<PresentationMetadata>,
    },
    Error(Box<ErrorFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(Box<MindmapFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-state")]
    State(Box<StateFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-sequence")]
    Sequence(
        Box<
            FamilyPair<
                &'parsed diagrams::sequence::SequenceDiagramModel,
                crate::sequence::SequencePreparedArtifact,
            >,
        >,
    ),
    #[cfg(feature = "diagram-zenuml")]
    Zenuml(Box<ZenumlFamilyArtifact<'parsed>>),
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Flowchart(Box<FlowchartFamilyArtifact<'parsed, FlowchartLayout>>),
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Swimlane(Box<FlowchartFamilyArtifact<'parsed, SwimlaneLayout>>),
    #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
    #[cfg(feature = "diagram-architecture")]
    Architecture(Box<ArchitectureFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-class")]
    Class(Box<ClassFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-c4")]
    C4(Box<C4FamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-cynefin")]
    Cynefin(Box<CynefinFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-wardley")]
    Wardley(Box<WardleyFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-railroad")]
    Railroad(Box<RailroadFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-kanban")]
    Kanban(
        Box<
            FamilyPair<
                &'parsed diagrams::kanban::KanbanDiagramRenderModel,
                crate::kanban::KanbanPreparedArtifact,
            >,
        >,
    ),
    #[cfg(feature = "diagram-gantt")]
    Gantt(Box<GanttFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-pie")]
    Pie(Box<PieFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-packet")]
    Packet(Box<PacketFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-timeline")]
    Timeline(Box<TimelineFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-journey")]
    Journey(Box<JourneyFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-requirement")]
    Requirement(Box<RequirementFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-sankey")]
    Sankey(Box<SankeyFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-radar")]
    Radar(Box<RadarFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-info")]
    Info(Box<InfoFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-treemap")]
    Treemap(Box<TreemapFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-block")]
    Block(Box<BlockFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-er")]
    Er(Box<ErFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChart(Box<QuadrantChartFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-xychart")]
    XyChart(Box<XyChartFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(Box<GitGraphFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(Box<TreeViewFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-ishikawa")]
    Ishikawa(Box<IshikawaFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling(Box<EventModelingFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-venn")]
    Venn(Box<VennFamilyArtifact<'parsed>>),
    #[cfg(feature = "diagram-usecase")]
    Usecase(
        Box<
            FamilyPair<
                &'parsed diagrams::usecase::UsecaseDiagramModel,
                crate::usecase::UsecasePreparedArtifact,
            >,
        >,
    ),
}

// The ordered JSON value preserves field order and stores floats as f64. Keep these wire
// fields in declaration order and serialize floating-point geometry as f64 for byte parity.
#[derive(serde::Serialize)]
struct LayoutGeometryProjection<'a> {
    schema_version: u32,
    family_id: DiagramFamilyId,
    diagram_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout_adaptation: Option<crate::LayoutAdaptationObservation>,
    layout: LayoutProjection<'a>,
}

#[derive(serde::Serialize)]
enum LayoutProjection<'a> {
    #[cfg(feature = "diagram-agentflow")]
    AgentflowDiagram(&'a FlowchartLayout),
    #[cfg(feature = "diagram-block")]
    BlockDiagram(&'a BlockDiagramLayout),
    #[cfg(feature = "diagram-requirement")]
    RequirementDiagram(&'a RequirementDiagramLayout),
    #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
    #[cfg(feature = "diagram-architecture")]
    ArchitectureDiagram(&'a ArchitectureDiagramLayout),
    #[cfg(feature = "diagram-mindmap")]
    MindmapDiagram(&'a MindmapDiagramLayout),
    #[cfg(feature = "diagram-sankey")]
    SankeyDiagram(&'a SankeyDiagramLayout),
    #[cfg(feature = "diagram-radar")]
    RadarDiagram(&'a RadarDiagramLayout),
    #[cfg(feature = "diagram-treemap")]
    TreemapDiagram(&'a TreemapDiagramLayout),
    #[cfg(feature = "diagram-venn")]
    VennDiagram(&'a VennDiagramLayout),
    #[cfg(feature = "diagram-usecase")]
    UsecaseDiagram(&'a crate::usecase::UsecaseDiagramLayout),
    #[cfg(feature = "diagram-xychart")]
    XyChartDiagram(&'a XyChartDiagramLayout),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChartDiagram(&'a QuadrantChartDiagramLayout),
    #[serde(rename = "FlowchartV2")]
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Flowchart(&'a FlowchartLayout),
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    SwimlaneDiagram(&'a SwimlaneLayout),
    #[serde(rename = "StateDiagramV2")]
    #[cfg(feature = "diagram-state")]
    StateDiagram(&'a StateDiagramLayout),
    #[serde(rename = "ClassDiagramV2")]
    #[cfg(feature = "diagram-class")]
    ClassDiagram(&'a ClassDiagramLayout),
    #[cfg(feature = "diagram-er")]
    ErDiagram(&'a ErDiagramLayout),
    #[cfg(feature = "diagram-sequence")]
    SequenceDiagram(&'a SequenceDiagramLayout),
    #[cfg(feature = "diagram-zenuml")]
    ZenumlDiagram(&'a crate::zenuml::ZenumlDiagramLayout),
    #[cfg(feature = "diagram-info")]
    InfoDiagram(&'a InfoDiagramLayout),
    #[cfg(feature = "diagram-packet")]
    PacketDiagram(&'a PacketDiagramLayout),
    #[cfg(feature = "diagram-timeline")]
    TimelineDiagram(&'a TimelineDiagramLayout),
    #[cfg(feature = "diagram-pie")]
    PieDiagram(&'a PieDiagramLayout),
    #[cfg(feature = "diagram-journey")]
    JourneyDiagram(&'a JourneyDiagramLayout),
    #[cfg(feature = "diagram-kanban")]
    KanbanDiagram(&'a KanbanDiagramLayout),
    #[cfg(feature = "diagram-git-graph")]
    GitGraphDiagram(&'a GitGraphDiagramLayout),
    #[cfg(feature = "diagram-tree-view")]
    TreeViewDiagram(&'a TreeViewDiagramLayout),
    #[cfg(feature = "diagram-ishikawa")]
    IshikawaDiagram(&'a IshikawaDiagramLayout),
    #[cfg(feature = "diagram-event-modeling")]
    EventModelingDiagram(&'a EventModelingDiagramLayout),
    #[cfg(feature = "diagram-cynefin")]
    CynefinDiagram(&'a CynefinDiagramLayout),
    #[cfg(feature = "diagram-wardley")]
    WardleyDiagram(&'a WardleyDiagramLayout),
    #[cfg(feature = "diagram-railroad")]
    RailroadDiagram(&'a RailroadDiagramLayout),
    #[cfg(feature = "diagram-gantt")]
    GanttDiagram(&'a GanttDiagramLayout),
    #[cfg(feature = "diagram-c4")]
    C4Diagram(&'a C4DiagramLayout),
    ErrorDiagram(&'a ErrorDiagramLayout),
}

impl BuiltinFamilyArtifact<'_> {
    pub fn family_id(&self) -> DiagramFamilyId {
        match self {
            Self::Error(_) => DiagramFamilyId::ERROR,
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(_) => DiagramFamilyId::MINDMAP,
            #[cfg(feature = "diagram-state")]
            Self::State(_) => DiagramFamilyId::STATE,
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(_) => DiagramFamilyId::SEQUENCE,
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(_) => DiagramFamilyId::ZENUML,
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(_) => DiagramFamilyId::FLOWCHART,
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Swimlane(_) => DiagramFamilyId::SWIMLANE,
            #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(_) => DiagramFamilyId::ARCHITECTURE,
            #[cfg(feature = "diagram-class")]
            Self::Class(_) => DiagramFamilyId::CLASS,
            #[cfg(feature = "diagram-c4")]
            Self::C4(_) => DiagramFamilyId::C4,
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(_) => DiagramFamilyId::CYNEFIN,
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(_) => DiagramFamilyId::WARDLEY,
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(_) => DiagramFamilyId::RAILROAD,
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(_) => DiagramFamilyId::KANBAN,
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(_) => DiagramFamilyId::GANTT,
            #[cfg(feature = "diagram-pie")]
            Self::Pie(_) => DiagramFamilyId::PIE,
            #[cfg(feature = "diagram-packet")]
            Self::Packet(_) => DiagramFamilyId::PACKET,
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(_) => DiagramFamilyId::TIMELINE,
            #[cfg(feature = "diagram-journey")]
            Self::Journey(_) => DiagramFamilyId::JOURNEY,
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(_) => DiagramFamilyId::REQUIREMENT,
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(_) => DiagramFamilyId::SANKEY,
            #[cfg(feature = "diagram-radar")]
            Self::Radar(_) => DiagramFamilyId::RADAR,
            #[cfg(feature = "diagram-info")]
            Self::Info(_) => DiagramFamilyId::INFO,
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(_) => DiagramFamilyId::TREEMAP,
            #[cfg(feature = "diagram-block")]
            Self::Block(_) => DiagramFamilyId::BLOCK,
            #[cfg(feature = "diagram-er")]
            Self::Er(_) => DiagramFamilyId::ER,
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(_) => DiagramFamilyId::QUADRANT_CHART,
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(_) => DiagramFamilyId::XY_CHART,
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(_) => DiagramFamilyId::GIT_GRAPH,
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(_) => DiagramFamilyId::TREE_VIEW,
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(_) => DiagramFamilyId::ISHIKAWA,
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(_) => DiagramFamilyId::EVENT_MODELING,
            #[cfg(feature = "diagram-venn")]
            Self::Venn(_) => DiagramFamilyId::VENN,
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow { .. } => DiagramFamilyId::AGENTFLOW,
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(_) => DiagramFamilyId::USECASE,
        }
    }

    fn prepared_math_evidence(&self) -> PreparedMathEvidenceLease {
        match self {
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow { flow, .. } => flow.svg_label_sidecar().prepared_math_evidence(),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(artifact) => artifact.svg_label_sidecar().prepared_math_evidence(),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Swimlane(artifact) => artifact.svg_label_sidecar().prepared_math_evidence(),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(artifact) => artifact.layout.math_sidecar().prepared_math_evidence(),
            _ => PreparedMathEvidenceLease::default(),
        }
    }

    fn compatibility_json(
        &self,
        metadata: &PresentationMetadata,
        output_context: &PresentationContext,
        control: &merman_model::OperationControl,
    ) -> Result<ManagedJson> {
        let control = control.for_phase(OperationPhase::Semantic);
        control.checkpoint()?;
        match self {
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow { semantic, .. } => semantic
                .compatibility_json_with_context_controlled(metadata, output_context, &control)?
                .map_err(Error::SemanticProjection),
            Self::Error(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-state")]
            Self::State(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(pair) => pair.compatibility_json(metadata, output_context, &control),
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Swimlane(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-class")]
            Self::Class(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-c4")]
            Self::C4(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(pair) => pair.compatibility_json(metadata, output_context, &control),
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-pie")]
            Self::Pie(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-packet")]
            Self::Packet(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-journey")]
            Self::Journey(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-radar")]
            Self::Radar(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-info")]
            Self::Info(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-block")]
            Self::Block(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-er")]
            Self::Er(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(artifact) => {
                artifact
                    .pair
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-venn")]
            Self::Venn(artifact) => {
                artifact
                    .pair()
                    .compatibility_json(metadata, output_context, &control)
            }
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(pair) => pair.compatibility_json(metadata, output_context, &control),
        }
    }

    fn layout_projection(&self) -> LayoutProjection<'_> {
        match self {
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow { flow, .. } => LayoutProjection::AgentflowDiagram(flow.pair.layout()),
            Self::Error(artifact) => LayoutProjection::ErrorDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(artifact) => LayoutProjection::MindmapDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-state")]
            Self::State(artifact) => LayoutProjection::StateDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout().layout()),
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(artifact) => LayoutProjection::ZenumlDiagram(artifact.pair().layout()),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(artifact) => LayoutProjection::Flowchart(artifact.pair.layout()),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Swimlane(artifact) => LayoutProjection::SwimlaneDiagram(artifact.pair.layout()),
            #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(artifact) => {
                LayoutProjection::ArchitectureDiagram(artifact.pair.layout())
            }
            #[cfg(feature = "diagram-class")]
            Self::Class(artifact) => LayoutProjection::ClassDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-c4")]
            Self::C4(artifact) => LayoutProjection::C4Diagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(artifact) => LayoutProjection::CynefinDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(artifact) => LayoutProjection::WardleyDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(artifact) => LayoutProjection::RailroadDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout().layout()),
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(artifact) => LayoutProjection::GanttDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-pie")]
            Self::Pie(artifact) => LayoutProjection::PieDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-packet")]
            Self::Packet(artifact) => LayoutProjection::PacketDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(artifact) => LayoutProjection::TimelineDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-journey")]
            Self::Journey(artifact) => LayoutProjection::JourneyDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(artifact) => {
                LayoutProjection::RequirementDiagram(artifact.pair.layout().layout())
            }
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(artifact) => LayoutProjection::SankeyDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-radar")]
            Self::Radar(artifact) => LayoutProjection::RadarDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-info")]
            Self::Info(artifact) => LayoutProjection::InfoDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(artifact) => LayoutProjection::TreemapDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-block")]
            Self::Block(artifact) => LayoutProjection::BlockDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-er")]
            Self::Er(artifact) => LayoutProjection::ErDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(artifact) => {
                LayoutProjection::QuadrantChartDiagram(artifact.pair.layout())
            }
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(artifact) => LayoutProjection::XyChartDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(artifact) => LayoutProjection::GitGraphDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(artifact) => LayoutProjection::TreeViewDiagram(artifact.pair.layout()),
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(artifact) => LayoutProjection::IshikawaDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(artifact) => {
                LayoutProjection::EventModelingDiagram(artifact.pair().layout())
            }
            #[cfg(feature = "diagram-venn")]
            Self::Venn(artifact) => LayoutProjection::VennDiagram(artifact.pair().layout()),
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(pair) => LayoutProjection::UsecaseDiagram(pair.layout().layout()),
        }
    }
}

pub struct FamilyRenderArtifact<'parsed> {
    metadata: &'parsed PresentationMetadata,
    output_context: &'parsed PresentationContext,
    family: BuiltinFamilyArtifact<'parsed>,
    required_capabilities: Vec<RenderCapability>,
    context: FamilyRenderContext,
}

/// Owned projection of the Gantt time scale used by comparison tooling.
///
/// The projection deliberately hides the full family layout. Its inverse uses the same rounded
/// pixel mapping as the renderer, so coordinates that fall between representable task times return
/// `None` instead of inventing a timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GanttTimeAxisDiagnostics {
    min_ms: i64,
    max_ms: i64,
    left_x: f64,
    drawable_width: f64,
}

impl GanttTimeAxisDiagnostics {
    #[cfg(feature = "diagram-gantt")]
    fn from_layout(layout: &GanttDiagramLayout) -> Option<Self> {
        let min_ms = layout.tasks.iter().map(|task| task.start_ms).min()?;
        let max_ms = layout.tasks.iter().map(|task| task.end_ms).max()?;
        if max_ms <= min_ms {
            return None;
        }

        let left_x = layout.left_padding;
        let drawable_width = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
        if !left_x.is_finite() || !drawable_width.is_finite() {
            return None;
        }

        Some(Self {
            min_ms,
            max_ms,
            left_x,
            drawable_width,
        })
    }

    /// Resolves an exact rendered x coordinate back to a Unix timestamp in milliseconds.
    pub fn unix_millis_at_rendered_x(&self, target_x: f64) -> Option<i64> {
        if !target_x.is_finite() {
            return None;
        }

        let span_ms = (i128::from(self.max_ms) - i128::from(self.min_ms)) as f64;
        let scaled_x = target_x - self.left_x;
        if !span_ms.is_finite() || !scaled_x.is_finite() {
            return None;
        }

        let estimate = self.min_ms as f64 + span_ms * (scaled_x / self.drawable_width);
        if !estimate.is_finite() {
            return None;
        }

        let mut lo = estimate.round() as i64;
        let mut hi = lo;
        let mut step = 1_i64;
        for _ in 0..80 {
            if self.rendered_x(lo)? <= target_x {
                break;
            }
            hi = lo;
            lo = lo.saturating_sub(step);
            step = step.saturating_mul(2);
        }

        step = 1;
        for _ in 0..80 {
            if self.rendered_x(hi)? >= target_x {
                break;
            }
            lo = hi;
            hi = hi.saturating_add(step);
            step = step.saturating_mul(2);
        }

        let lo_x = self.rendered_x(lo)?;
        let hi_x = self.rendered_x(hi)?;
        if !(lo_x <= target_x && target_x <= hi_x) {
            return None;
        }

        while lo < hi {
            let half_distance = ((i128::from(hi) - i128::from(lo)) / 2) as i64;
            let mid = lo + half_distance;
            if self.rendered_x(mid)? < target_x {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }

        (self.rendered_x(lo)? == target_x).then_some(lo)
    }

    fn rendered_x(&self, unix_millis: i64) -> Option<f64> {
        let offset_ms = (i128::from(unix_millis) - i128::from(self.min_ms)) as f64;
        let span_ms = (i128::from(self.max_ms) - i128::from(self.min_ms)) as f64;
        let x = self.left_x + (offset_ms / span_ms * self.drawable_width).round();
        x.is_finite().then_some(x)
    }
}

/// A completed family SVG produced by the canonical typed render operation.
///
/// Root completion evidence is private to the renderer and cannot be named by callers:
///
/// ```compile_fail
/// use merman_render::svg::RootedSvg;
/// ```
///
/// A raw string cannot be substituted for a completed family SVG:
///
/// ```compile_fail
/// use merman_render::family::RenderedFamilySvg;
///
/// let forged: RenderedFamilySvg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
/// ```
pub struct RenderedFamilySvg {
    layout_adaptation: Option<crate::LayoutAdaptationObservation>,
    shoal_layout_frame: Option<Arc<crate::ShoalLayoutFrame>>,
    svg: String,
    prepared_math_evidence: PreparedMathEvidenceLease,
    prepared_math_evidence_valid: bool,
    family_id: DiagramFamilyId,
    output_mutated: bool,
    metadata: PresentationMetadata,
    required_capabilities: Vec<RenderCapability>,
    session: RenderSession,
}

/// Opaque output paired with the family report frozen after all requested SVG work.
///
/// The live [`RenderSession`] remains in pre-completion SVG types so pipelines can record their
/// measurements. Converting to this type replaces that mutable session with [`RenderSessionReport`].
/// Completion therefore exposes no route for recording more measurements:
///
/// ```compile_fail
/// use merman_render::family::FamilyRenderCompletion;
///
/// fn measure_after_completion(completion: FamilyRenderCompletion<String>) {
///     let _ = completion.session();
/// }
/// ```
///
/// An output and a report from separate renders cannot be assembled into a completion:
///
/// ```compile_fail
/// use merman_render::family::{FamilyRenderCompletion, FamilyRenderReport};
/// use merman_render::svg::ResvgCompatibleSvg;
///
/// fn rebind(output: ResvgCompatibleSvg, report: FamilyRenderReport) {
///     let _ = FamilyRenderCompletion { output, report };
/// }
/// ```
#[must_use = "completed render output and its frozen report should be consumed together"]
pub struct FamilyRenderCompletion<T> {
    layout_adaptation: Option<crate::LayoutAdaptationObservation>,
    output: T,
    report: FamilyRenderReport,
}

impl<T> FamilyRenderCompletion<T> {
    /// Placement observations retained independently of theme and admission reports.
    pub const fn layout_adaptation(&self) -> Option<crate::LayoutAdaptationObservation> {
        self.layout_adaptation
    }

    pub const fn output(&self) -> &T {
        &self.output
    }

    pub const fn report(&self) -> &FamilyRenderReport {
        &self.report
    }

    pub fn into_output_and_report(self) -> (T, FamilyRenderReport) {
        (self.output, self.report)
    }
}

impl RenderedFamilySvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    /// Returns the requested placement hints from this successful Shoal render.
    pub fn shoal_layout_frame(&self) -> Option<Arc<crate::ShoalLayoutFrame>> {
        self.shoal_layout_frame.clone()
    }

    pub fn metadata(&self) -> &PresentationMetadata {
        &self.metadata
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        RenderFamilyKind::from_family_id(self.family_id())
    }

    pub fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    /// Returns the optional capabilities admitted during this artifact's preparation.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required_capabilities
    }

    /// Applies an output pipeline while retaining the renderer-owned family capability.
    pub fn apply_pipeline(mut self, pipeline: &SvgPipeline) -> Result<Self> {
        self.session.checkpoint(OperationPhase::Postprocess)?;
        let output_metadata = self.output_metadata()?;
        let requires_prepared_math_projection = pipeline.requires_prepared_math_projection();
        let preserves_prepared_math = self.prepared_math_evidence_valid
            && pipeline.preserves_prepared_math_evidence(output_metadata.family_id());
        let supplies_prepared_math = self.prepared_math_evidence_valid
            && (preserves_prepared_math || requires_prepared_math_projection);
        let source_svg = std::mem::take(&mut self.svg);
        let processed_svg = pipeline.process_generated_with_metadata_and_math_evidence(
            source_svg,
            &output_metadata,
            &self.session,
            supplies_prepared_math.then_some(&self.prepared_math_evidence),
        )?;
        self.svg = processed_svg;
        if !preserves_prepared_math {
            self.prepared_math_evidence_valid = false;
        }
        self.session.work_meter().preflight_svg_byte_count(
            self.svg.len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )?;
        self.output_mutated |= pipeline.has_custom_postprocessors(output_metadata.family_id());
        self.session.checkpoint(OperationPhase::Postprocess)?;
        Ok(self)
    }

    /// Finalizes the typed family output for resvg/raster consumption.
    pub fn finalize_resvg(self, pipeline: &SvgPipeline) -> Result<RenderedResvgCompatibleSvg> {
        self.session.checkpoint(OperationPhase::Export)?;
        let output_metadata = self.output_metadata()?;
        let prepared_math_evidence_valid = self.prepared_math_evidence_valid
            && pipeline.preserves_prepared_math_evidence(output_metadata.family_id());
        if !self.prepared_math_evidence.is_empty() && !prepared_math_evidence_valid {
            return Err(Error::svg_postprocess(
                "prepared-math-terminal-receipt",
                "renderer-owned prepared-math evidence was invalidated before native projection",
            ));
        }
        let prepared_math_evidence = if prepared_math_evidence_valid {
            self.prepared_math_evidence
        } else {
            PreparedMathEvidenceLease::default()
        };
        let svg = pipeline.process_owned_resvg_compatible_with_metadata_and_evidence(
            self.svg,
            &output_metadata,
            &self.session,
            prepared_math_evidence,
            prepared_math_evidence_valid,
        )?;
        self.session.work_meter().preflight_svg_byte_count(
            svg.as_str().len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Export,
        )?;
        let output_mutated =
            self.output_mutated || pipeline.has_custom_postprocessors(output_metadata.family_id());
        self.session.checkpoint(OperationPhase::Export)?;
        Ok(RenderedResvgCompatibleSvg {
            layout_adaptation: self.layout_adaptation,
            svg,
            family_id: self.family_id,
            output_mutated,
            session: self.session,
        })
    }

    /// Completes the exact artifact selected for the standalone SVG target.
    pub fn finalize_standalone(
        self,
        pipeline: Option<&SvgPipeline>,
    ) -> Result<RenderedStandaloneSvg> {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::Postprocess,
        );
        match pipeline {
            Some(pipeline) if pipeline.preset() == SvgPipelinePreset::ResvgSafe => self
                .finalize_resvg(pipeline)
                .map(RenderedResvgCompatibleSvg::into_standalone),
            Some(pipeline) => self
                .apply_pipeline(pipeline)?
                .finalize_exact_standalone(pipeline),
            None => self.finalize_exact_standalone(&SvgPipeline::parity()),
        }
    }

    fn finalize_exact_standalone(self, pipeline: &SvgPipeline) -> Result<RenderedStandaloneSvg> {
        self.session.checkpoint(OperationPhase::Postprocess)?;
        let artifact = StandaloneSvgArtifact::finalize_exact(self.svg, pipeline, &self.session)?;
        self.session.checkpoint(OperationPhase::Postprocess)?;
        Ok(RenderedStandaloneSvg {
            layout_adaptation: self.layout_adaptation,
            artifact,
            family_id: self.family_id,
            output_mutated: self.output_mutated,
            session: self.session,
        })
    }

    fn output_metadata(&self) -> Result<SvgPostprocessMetadata> {
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(
            &self.svg,
            SvgPostprocessExecution::new(&self.session),
        )?;
        Ok(metadata
            .with_family_id(self.family_id())
            .with_diagram_type(self.metadata.diagram_type.clone())
            .with_optional_diagram_title(self.metadata.title.clone()))
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<String> {
        FamilyRenderCompletion {
            layout_adaptation: self.layout_adaptation,
            output: self.svg,
            report: FamilyRenderReport::freeze(self.family_id, self.session, self.output_mutated),
        }
    }
}

/// Renderer-owned family output after the terminal resvg compatibility finalizer.
pub struct RenderedResvgCompatibleSvg {
    layout_adaptation: Option<crate::LayoutAdaptationObservation>,
    svg: ResvgCompatibleSvg,
    family_id: DiagramFamilyId,
    output_mutated: bool,
    session: RenderSession,
}

impl RenderedResvgCompatibleSvg {
    pub fn svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        RenderFamilyKind::from_family_id(self.family_id())
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<ResvgCompatibleSvg> {
        FamilyRenderCompletion {
            layout_adaptation: self.layout_adaptation,
            output: self.svg,
            report: FamilyRenderReport::freeze(self.family_id, self.session, self.output_mutated),
        }
    }

    fn into_standalone(self) -> RenderedStandaloneSvg {
        RenderedStandaloneSvg {
            layout_adaptation: self.layout_adaptation,
            artifact: StandaloneSvgArtifact::from_resvg_compatible(self.svg),
            family_id: self.family_id,
            output_mutated: self.output_mutated,
            session: self.session,
        }
    }
}

/// Renderer-owned family output paired with the exact standalone SVG terminal artifact.
pub struct RenderedStandaloneSvg {
    layout_adaptation: Option<crate::LayoutAdaptationObservation>,
    artifact: StandaloneSvgArtifact,
    family_id: DiagramFamilyId,
    output_mutated: bool,
    session: RenderSession,
}

impl RenderedStandaloneSvg {
    /// Whether the operation used host-provided text metrics.
    pub fn has_host_measurements(&self) -> bool {
        self.session.has_host_measurements()
    }

    pub fn artifact(&self) -> &StandaloneSvgArtifact {
        &self.artifact
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        RenderFamilyKind::from_family_id(self.family_id())
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    /// Returns the SVG bytes without materializing a completion report.
    pub fn into_string(self) -> String {
        self.artifact.into_string()
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<StandaloneSvgArtifact> {
        FamilyRenderCompletion {
            layout_adaptation: self.layout_adaptation,
            output: self.artifact,
            report: FamilyRenderReport::freeze(self.family_id, self.session, self.output_mutated),
        }
    }
}

impl<'parsed> FamilyRenderArtifact<'parsed> {
    fn graph_placement(&self) -> Option<&crate::layout_frame::GraphPlacement> {
        match &self.family {
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            BuiltinFamilyArtifact::Flowchart(flow) => Some(&flow.pair.layout().placement),
            #[cfg(all(
                feature = "layout-shoal",
                any(feature = "diagram-flowchart", feature = "diagram-swimlane")
            ))]
            BuiltinFamilyArtifact::Swimlane(flow) => flow
                .pair
                .layout()
                .shoal
                .as_ref()
                .map(|shoal| &shoal.placement),
            #[cfg(feature = "diagram-agentflow")]
            BuiltinFamilyArtifact::Agentflow { flow, .. } => Some(&flow.pair.layout().placement),
            #[cfg(all(feature = "layout-shoal", feature = "diagram-architecture"))]
            BuiltinFamilyArtifact::Architecture(artifact) => {
                artifact.shoal().map(|geometry| geometry.placement())
            }
            #[cfg(all(feature = "layout-shoal", feature = "diagram-c4"))]
            BuiltinFamilyArtifact::C4(artifact) => {
                artifact.shoal().map(|geometry| geometry.placement())
            }
            #[cfg(all(feature = "layout-shoal", feature = "diagram-class"))]
            BuiltinFamilyArtifact::Class(artifact) => artifact
                .route_contract()
                .canonical_routes()
                .map(|routes| &routes.placement),
            #[cfg(all(feature = "layout-shoal", feature = "diagram-state"))]
            BuiltinFamilyArtifact::State(artifact) => artifact
                .route_contract()
                .canonical_routes()
                .map(|routes| &routes.placement),
            #[cfg(all(feature = "layout-shoal", feature = "diagram-er"))]
            BuiltinFamilyArtifact::Er(artifact) => artifact
                .route_contract()
                .canonical_routes()
                .map(|routes| &routes.placement),
            #[cfg(all(feature = "layout-shoal", feature = "diagram-requirement"))]
            BuiltinFamilyArtifact::Requirement(artifact) => artifact
                .pair
                .layout()
                .canonical_routes()
                .map(|routes| &routes.placement),
            #[cfg(all(feature = "layout-shoal", feature = "diagram-usecase"))]
            BuiltinFamilyArtifact::Usecase(pair) => pair
                .layout()
                .canonical_routes()
                .map(|routes| &routes.placement),
            _ => None,
        }
    }

    /// Width observations from the family placement that produced this artifact.
    pub fn layout_adaptation(&self) -> Option<crate::LayoutAdaptationObservation> {
        match &self.family {
            #[cfg(feature = "diagram-kanban")]
            BuiltinFamilyArtifact::Kanban(pair) => pair.layout().layout_adaptation(),
            _ => self.graph_placement().and_then(|layout| layout.adaptation),
        }
    }

    fn new(
        parsed: &'parsed PresentationDiagram,
        family: BuiltinFamilyArtifact<'parsed>,
        context: FamilyRenderContext,
    ) -> Result<Self> {
        let actual_family = family.family_id();
        let expected_family = context.family_id();
        if actual_family != expected_family {
            return Err(Error::InvalidModel {
                message: format!(
                    "planned render family {expected_family} produced {actual_family} artifact"
                ),
            });
        }

        Ok(Self {
            metadata: parsed.metadata(),
            output_context: parsed.render_context(),
            family,
            required_capabilities: Vec::new(),
            context,
        })
    }

    pub fn metadata(&self) -> &PresentationMetadata {
        self.metadata
    }

    fn paint_metadata(&self) -> &PresentationMetadata {
        #[cfg(feature = "diagram-agentflow")]
        if let BuiltinFamilyArtifact::Agentflow {
            projected_metadata, ..
        } = &self.family
        {
            return projected_metadata;
        }
        self.metadata
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        RenderFamilyKind::from_family_id(self.family_id())
    }

    pub fn family_id(&self) -> DiagramFamilyId {
        self.family.family_id()
    }

    pub fn gantt_time_axis_diagnostics(&self) -> Option<GanttTimeAxisDiagnostics> {
        match &self.family {
            #[cfg(feature = "diagram-gantt")]
            BuiltinFamilyArtifact::Gantt(artifact) => {
                GanttTimeAxisDiagnostics::from_layout(artifact.pair().layout())
            }
            _ => None,
        }
    }

    /// Projects the versioned geometry of this prepared layout.
    pub fn layout_geometry_json(&self) -> Result<ManagedJson> {
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let geometry = ManagedJson::from(serde_json::to_value(self.layout_geometry_projection())?);
        self.context.session().checkpoint(OperationPhase::Emit)?;
        Ok(geometry)
    }

    /// Writes the versioned geometry of this prepared layout as one JSON value.
    ///
    /// The final emission checkpoint commits the write. Writer failures can leave partial output.
    pub fn write_layout_geometry_json(
        &self,
        writer: impl std::io::Write,
        pretty: bool,
    ) -> Result<()> {
        let projection = self.layout_geometry_projection();
        self.context.session().checkpoint(OperationPhase::Emit)?;
        if pretty {
            serde_json::to_writer_pretty(writer, &projection)?;
        } else {
            serde_json::to_writer(writer, &projection)?;
        }
        Ok(())
    }

    fn layout_geometry_projection(&self) -> LayoutGeometryProjection<'_> {
        LayoutGeometryProjection {
            schema_version: 1,
            family_id: self.family_id(),
            diagram_type: &self.metadata.diagram_type,
            layout_adaptation: self.layout_adaptation(),
            layout: self.family.layout_projection(),
        }
    }

    /// Projects the full semantic/layout snapshot from this artifact's paired family owners.
    ///
    /// Callers cannot inject an unrelated semantic payload into the envelope.
    ///
    /// ```compile_fail,E0599
    /// fn inject(
    ///     artifact: &merman_render::family::FamilyRenderArtifact,
    ///     semantic: merman_model::ManagedJson,
    /// ) {
    ///     let _ = artifact.layout_json_with_compatibility_json(semantic);
    /// }
    /// ```
    pub fn layout_json(&self) -> Result<ManagedJson> {
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let semantic = self.family.compatibility_json(
            self.metadata,
            self.output_context,
            self.context.session().work_meter().control(),
        )?;
        self.assemble_layout_json(semantic)
    }

    fn assemble_layout_json(&self, semantic: ManagedJson) -> Result<ManagedJson> {
        let layout = serde_json::to_value(self.family.layout_projection())?;

        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "diagram_type".to_string(),
            serde_json::Value::String(self.metadata.diagram_type.clone()),
        );
        metadata.insert(
            "title".to_string(),
            self.metadata
                .title
                .as_ref()
                .map_or(serde_json::Value::Null, |title| {
                    serde_json::Value::String(title.clone())
                }),
        );
        metadata.insert(
            "config".to_string(),
            self.metadata
                .config
                .to_managed_json()
                .into_unmanaged_value(),
        );
        metadata.insert(
            "effective_config".to_string(),
            self.metadata
                .effective_config
                .as_config()
                .to_managed_json()
                .into_unmanaged_value(),
        );

        if let Some(observation) = self.layout_adaptation() {
            metadata.insert(
                "layout_adaptation".to_owned(),
                serde_json::to_value(observation)?,
            );
        }
        let mut projection = serde_json::Map::new();
        projection.insert("meta".to_string(), serde_json::Value::Object(metadata));
        projection.insert("semantic".to_string(), semantic.into_unmanaged_value());
        projection.insert("layout".to_string(), layout);
        let projection = ManagedJson::from(serde_json::Value::Object(projection));
        self.context.session().checkpoint(OperationPhase::Emit)?;
        Ok(projection)
    }

    /// Returns occurrence-ordered edge paint geometry without serializing SVG.
    ///
    /// Points refine the layout routes with endpoint clipping and line hops. Labels use the
    /// same prepared positioning decisions as SVG; no SVG byte budget is consumed.
    pub fn edge_geometry_json(self) -> Result<Vec<crate::model::EdgePaintGeometry>> {
        self.context.session().checkpoint(OperationPhase::Layout)?;
        if self.context.session().portability_requirement()
            == ThemePortabilityRequirement::RequirePortable
        {
            return Err(Error::PortabilityUnavailableForTarget {
                target: "edge-geometry-json",
            });
        }
        let geometry = crate::svg::edge_geometry_family_artifact(
            &self.family,
            self.paint_metadata(),
            self.context.execution(),
        )?;
        self.context.session().checkpoint(OperationPhase::Emit)?;
        Ok(geometry)
    }

    pub fn render_svg(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
    ) -> Result<RenderedFamilySvg> {
        self.render_svg_for_target(options, debug, None, crate::svg::SvgGeneration::Full)
    }

    #[doc(hidden)]
    pub fn render_svg_for_target(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
        pipeline: Option<&SvgPipeline>,
        default_generation: crate::svg::SvgGeneration,
    ) -> Result<RenderedFamilySvg> {
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::SvgEmit);
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let generation = pipeline.map_or(default_generation, |pipeline| {
            pipeline.generation_for(self.family_id(), default_generation)
        });
        let trace_stage = debug.flowchart_edge_trace().map(|(edge_id, destination)| {
            let staging = FlowchartEdgeTraceCollector::default();
            let staged_debug = debug
                .clone()
                .with_flowchart_edge_trace(edge_id.to_owned(), staging.clone());
            (destination.clone(), staging, staged_debug)
        });
        let render_debug = trace_stage
            .as_ref()
            .map_or(debug, |(_, _, staged_debug)| staged_debug);
        if self.context.session().portability_requirement()
            == ThemePortabilityRequirement::RequirePortable
            && let Some(theme) = self.context.session().native_theme()
            && let Some(message) =
                crate::theme::unsupported_features(theme.resolved(), self.family_id()).next()
        {
            return Err(Error::UnsupportedNativeThemeFeature {
                family_id: self.family_id(),
                message,
            });
        }
        let (svg, output_mutated) =
            render_family_artifact_svg(&self, options, render_debug, generation)?;
        admit_rendered_svg_output(self.context.session(), &svg)?;
        let layout_adaptation = self.layout_adaptation();
        let shoal_layout_frame = self
            .graph_placement()
            .and_then(|layout| layout.frame.clone());
        let prepared_math_evidence = self.family.prepared_math_evidence();
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let family_id = self.family_id();

        if let Some((destination, staging, _)) = trace_stage {
            for trace in staging.drain() {
                destination.record(trace);
            }
        }

        Ok(RenderedFamilySvg {
            layout_adaptation,
            shoal_layout_frame,
            svg,
            prepared_math_evidence,
            prepared_math_evidence_valid: true,
            family_id,
            output_mutated,
            metadata: self.metadata.clone(),
            required_capabilities: self.required_capabilities,
            session: self.context.session,
        })
    }
}

fn admit_rendered_svg_output(session: &RenderSession, svg: &str) -> Result<()> {
    // Termination wins over the final output ceiling when both become observable during emit.
    session.checkpoint(OperationPhase::Emit)?;
    session.work_meter().preflight_svg_byte_count(
        svg.len(),
        ResourceLimitPhase::SvgOutput,
        OperationPhase::Emit,
    )?;
    Ok(())
}

#[inline(never)]
fn render_family_artifact_svg(
    artifact: &FamilyRenderArtifact,
    request: &SvgRenderOptions,
    debug: &SvgDebugOptions,
    generation: crate::svg::SvgGeneration,
) -> Result<(String, bool)> {
    let options = crate::svg::normalize_svg_render_options(request, artifact.context.session())?;
    let execution = artifact.context.execution();
    let metadata = artifact.paint_metadata();
    #[cfg(all(
        feature = "diagram-architecture",
        any(feature = "layout-cytoscape", feature = "layout-shoal")
    ))]
    if let BuiltinFamilyArtifact::Architecture(architecture) = &artifact.family {
        return crate::svg::render_architecture_family_artifact(
            architecture,
            &metadata.effective_config,
            execution,
            &options,
            debug,
        );
    }
    crate::svg::render_builtin_family_artifact(
        &artifact.family,
        metadata,
        execution,
        &options,
        debug,
        generation,
    )
}

#[cfg(feature = "diagram-agentflow")]
fn project_agentflow_flowchart_config(
    config: &merman_model::presentation::PresentationConfig,
    backend: GraphLayoutBackend,
    control: &merman_model::OperationControl,
) -> Result<merman_model::presentation::PresentationConfig> {
    let mut projected = config.as_config().clone();
    let root = config.as_value();
    let agentflow = root.get("agentflow").unwrap_or(&serde_json::Value::Null);
    // Agentflow's renderer owns the root viewport. Defaults here mirror its `??`
    // expressions; normal MermaidEngine configuration already includes schema defaults.
    // Overwrite even absent family values so unrelated Flowchart settings cannot leak in.
    for (key, fallback) in [
        ("diagramPadding", serde_json::json!(8)),
        ("useMaxWidth", serde_json::json!(true)),
        ("titleTopMargin", serde_json::json!(0)),
    ] {
        let value = agentflow.get(key).filter(|value| !value.is_null());
        projected.set_value(
            &format!("flowchart.{key}"),
            value
                .map(merman_model::json::clone_value_nonrecursive)
                .unwrap_or(fallback),
        );
    }
    // Dagre takes a truthy root override before the renderer's family spacing.
    // Mermaid's ELK adapter ignores all three configuration namespaces for spacing.
    // Our shared adapter reads Flowchart settings, so keep its default spacing for ELK.
    let uses_family_spacing = matches!(
        backend,
        GraphLayoutBackend::Dagre | GraphLayoutBackend::Shoal
    );
    for key in ["nodeSpacing", "rankSpacing"] {
        let value = uses_family_spacing
            .then(|| {
                root.get(key)
                    .filter(|value| crate::config::json_value_is_truthy(value))
                    .or_else(|| {
                        agentflow
                            .get(key)
                            .filter(|value| crate::config::json_value_is_truthy(value))
                    })
            })
            .flatten();
        projected.set_value(
            &format!("flowchart.{key}"),
            value
                .map(merman_model::json::clone_value_nonrecursive)
                .unwrap_or_else(|| serde_json::json!(50)),
        );
    }
    // AgentflowDB assigns these values to each node before the shared shape helpers
    // read them. Node-specific values precede Flowchart configuration; only a falsy
    // wrappingWidth falls through via labelHelper's JavaScript `||` expression.
    for key in ["minNodeWidth", "wrappingWidth"] {
        if let Some(value) = agentflow.get(key) {
            if key == "wrappingWidth" && !crate::config::json_value_is_truthy(value) {
                continue;
            }
            projected.set_value(
                &format!("flowchart.{key}"),
                merman_model::json::clone_value_nonrecursive(value),
            );
        }
    }
    let parts = merman_model::presentation::FrozenPresentationConfigParts::new(projected)
        .with_explicit_config_paths(Arc::clone(config.shared_explicit_config_paths()));
    // Existing render admission excludes config. This construction policy validates derived data
    // without recharging the request's model limits or declaring source authority.
    let construction_policy = merman_model::resources::InputResourcePolicy::for_profile(
        merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
    );
    merman_model::presentation::PresentationConfig::try_from_frozen_parts_controlled(
        parts,
        &construction_policy,
        control,
    )?
    .map_err(Error::from)
}

/// Prepares one family-owned typed semantic model for layout and SVG rendering.
///
/// Compatibility JSON is deliberately not accepted by this interface:
///
/// ```compile_fail
/// use merman_render::{LayoutOptions, environment::RenderEnvironment};
///
/// let session = RenderEnvironment::deterministic().begin_session().unwrap();
/// let raw_json = serde_json::json!({ "type": "flowchart-v2" });
/// let _ = merman_render::family::prepare(raw_json, &LayoutOptions::default(), session);
/// ```
///
/// Family semantic/layout pairing is private and therefore cannot be assembled independently:
///
/// ```compile_fail
/// use merman_render::family::FamilyPair;
/// ```
pub fn prepare<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    session: RenderSession,
) -> Result<FamilyRenderArtifact<'parsed>> {
    prepare_for(parsed, options, session, FamilyPreparation::Render)
}

/// Prepared geometry and its semantic projection. It cannot be used to emit SVG.
#[doc(hidden)]
pub struct FamilyLayoutArtifact<'parsed>(FamilyRenderArtifact<'parsed>);

impl FamilyLayoutArtifact<'_> {
    pub fn gantt_time_axis_diagnostics(&self) -> Option<GanttTimeAxisDiagnostics> {
        self.0.gantt_time_axis_diagnostics()
    }

    pub fn layout_json(&self) -> Result<ManagedJson> {
        self.0.layout_json()
    }

    pub fn layout_geometry_json(&self) -> Result<ManagedJson> {
        self.0.layout_geometry_json()
    }

    pub fn write_layout_geometry_json(
        &self,
        writer: impl std::io::Write,
        pretty: bool,
    ) -> Result<()> {
        self.0.write_layout_geometry_json(writer, pretty)
    }
}

#[doc(hidden)]
pub fn prepare_layout<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    session: RenderSession,
) -> Result<FamilyLayoutArtifact<'parsed>> {
    prepare_for(parsed, options, session, FamilyPreparation::Layout).map(FamilyLayoutArtifact)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FamilyPreparation {
    Render,
    Layout,
    #[cfg(all(test, feature = "all-diagrams"))]
    RenderWithoutFlowchartSvgLabels,
}

fn prepare_for<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    session: RenderSession,
    preparation: FamilyPreparation,
) -> Result<FamilyRenderArtifact<'parsed>> {
    let (plan, adaptation) = capability::plan_render_request(parsed, Some(options), &session)?;
    plan.ensure_available()?;
    let resolved;
    let options = if adaptation == options.adaptation {
        options
    } else {
        resolved = options.clone().with_adaptation(adaptation);
        &resolved
    };
    let required_capabilities = plan.required_capabilities().to_vec();
    let expected_family = plan.family_id();
    let context = FamilyRenderContext::resolve(session, expected_family, plan.backend());
    // Family adapter setup and projection surround the narrower kernel and measurement scopes.
    #[cfg(feature = "benchmark-stages")]
    let _stage =
        merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::LayoutPrepare);
    // Keep the heterogeneous router's debug-build caller slots out of the Dagre and FCoSE
    // preparation chains. Share the completion path so each route needs only one artifact slot.
    let mut artifact = match expected_family {
        #[cfg(feature = "diagram-class")]
        DiagramFamilyId::CLASS => preparation::prepare_class_render(parsed, options, context),
        #[cfg(all(
            feature = "diagram-architecture",
            any(feature = "layout-cytoscape", feature = "layout-shoal")
        ))]
        DiagramFamilyId::ARCHITECTURE => {
            preparation::prepare_architecture_render(parsed, options, context)
        }
        _ => preparation::prepare_non_class_render(parsed, options, context, preparation),
    }?;
    artifact.required_capabilities = required_capabilities;
    artifact
        .context
        .session()
        .checkpoint(OperationPhase::Layout)?;
    Ok(artifact)
}

#[cfg(all(
    test,
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
mod flowchart_layout_tests;

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
