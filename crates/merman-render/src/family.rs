use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::model::*;
use crate::presentation::{
    FlowchartPresentationPolicy, PresentationAspectResolution, PresentationProfile,
    PresentationRenderPolicy,
};
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    FlowchartEdgeTraceCollector, ResvgCompatibleSvg, SvgDebugOptions, SvgPipeline,
    SvgPostprocessExecution, SvgPostprocessMetadata, SvgRenderOptions,
};
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, RenderCapability, Result};
use merman_core::OperationPhase;
use merman_core::diagrams;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{BuiltinRenderSemantic, ParseMetadata, ParsedDiagramRender, RenderSemanticModel};
use merman_display_list::{
    DRAWING_LIST_MEDIA_TYPE, DrawingListDocument, DrawingListLimits, DrawingListPolicy,
};
use std::fmt;
use std::sync::OnceLock;

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
}

impl RenderFamilyKind {
    /// The complete built-in family catalog owned by the typed renderer.
    ///
    /// Keep this list next to the exhaustive `as_str` mapping so coverage tooling can compare
    /// external evidence against the actual renderer vocabulary without maintaining a second
    /// family enum.  The array is intentionally ordered by enum declaration order.
    pub const ALL: [Self; 33] = [
        Self::Error,
        Self::Mindmap,
        Self::State,
        Self::Sequence,
        Self::Zenuml,
        Self::Flowchart,
        Self::Swimlane,
        Self::Architecture,
        Self::Class,
        Self::C4,
        Self::Cynefin,
        Self::Wardley,
        Self::Railroad,
        Self::Kanban,
        Self::Gantt,
        Self::Pie,
        Self::Packet,
        Self::Timeline,
        Self::Journey,
        Self::Requirement,
        Self::Sankey,
        Self::Radar,
        Self::Info,
        Self::Treemap,
        Self::Block,
        Self::Er,
        Self::QuadrantChart,
        Self::XyChart,
        Self::GitGraph,
        Self::TreeView,
        Self::Ishikawa,
        Self::EventModeling,
        Self::Venn,
    ];

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
        }
    }
}

/// Capabilities required by one parsed typed render operation before layout starts.
///
/// Requirements come from the canonically paired semantic model and effective Mermaid config;
/// availability comes from the compiled layout backends and the operation's render session.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCapabilityPlan {
    diagram_type: String,
    required: Vec<RenderCapability>,
    missing: Vec<RenderCapability>,
    presentation_profile: Option<PresentationProfile>,
    presentation_aspects: Vec<PresentationAspectResolution>,
}

impl RenderCapabilityPlan {
    /// Returns the detected Mermaid diagram type used by render dispatch.
    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    /// Returns every optional capability this operation requires.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required
    }

    /// Returns the required capabilities unavailable in the planned render session.
    pub fn missing_capabilities(&self) -> &[RenderCapability] {
        &self.missing
    }

    /// Returns the selected first-party presentation profile, if any.
    pub const fn presentation_profile(&self) -> Option<PresentationProfile> {
        self.presentation_profile
    }

    /// Returns the selected presentation profile's stable ID, if any.
    pub const fn presentation_profile_id(&self) -> Option<&'static str> {
        match self.presentation_profile {
            Some(profile) => Some(profile.id()),
            None => None,
        }
    }

    /// Returns per-aspect resolution for the selected presentation profile.
    pub fn presentation_aspects(&self) -> &[PresentationAspectResolution] {
        &self.presentation_aspects
    }

    /// Iterates over stable semantic IDs for every required capability.
    pub fn required_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.required.iter().copied().map(RenderCapability::id)
    }

    /// Iterates over stable semantic IDs for every missing capability.
    pub fn missing_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.missing.iter().copied().map(RenderCapability::id)
    }

    /// Reports whether the planned render session satisfies every requirement.
    pub fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }

    fn ensure_available(&self) -> Result<()> {
        let Some(capability) = self.missing.first().copied() else {
            return Ok(());
        };
        Err(Error::MissingCapability {
            capability,
            diagram_type: self.diagram_type.clone(),
        })
    }
}

impl fmt::Display for RenderFamilyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub(crate) struct FamilyPair<S, L> {
    semantic: S,
    layout: L,
}

impl<S, L> FamilyPair<S, L> {
    fn new(semantic: S, layout: L) -> Self {
        Self { semantic, layout }
    }

    pub(crate) fn semantic(&self) -> &S {
        &self.semantic
    }

    pub(crate) fn layout(&self) -> &L {
        &self.layout
    }
}

impl<S: BuiltinRenderSemantic, L> FamilyPair<S, L> {
    fn compatibility_json(
        &self,
        metadata: &ParseMetadata,
    ) -> merman_core::Result<serde_json::Value> {
        self.semantic.compatibility_json(metadata)
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartFamilyArtifact<L> {
    pair: FamilyPair<diagrams::flowchart::FlowchartModel, L>,
    render_context: diagrams::flowchart::FlowchartRenderContext,
    svg_label_sidecar: crate::flowchart::FlowchartSvgLabelSidecar,
    policy: Option<FlowchartPresentationPolicy>,
}

impl<L> FlowchartFamilyArtifact<L> {
    pub(crate) fn pair(&self) -> &FamilyPair<diagrams::flowchart::FlowchartModel, L> {
        &self.pair
    }

    pub(crate) fn render_context(&self) -> &diagrams::flowchart::FlowchartRenderContext {
        &self.render_context
    }

    pub(crate) fn svg_label_sidecar(&self) -> &crate::flowchart::FlowchartSvgLabelSidecar {
        &self.svg_label_sidecar
    }

    pub(crate) const fn policy(&self) -> Option<FlowchartPresentationPolicy> {
        self.policy
    }
}

#[derive(Debug)]
pub(crate) enum BuiltinFamilyArtifact {
    Error(Box<FamilyPair<diagrams::error_diagram::ErrorDiagramRenderModel, ErrorDiagramLayout>>),
    Mindmap(Box<FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>>),
    State(Box<FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramLayout>>),
    Sequence(
        Box<
            FamilyPair<
                diagrams::sequence::SequenceDiagramRenderModel,
                crate::sequence::SequencePreparedArtifact,
            >,
        >,
    ),
    Zenuml(
        Box<
            FamilyPair<
                diagrams::zenuml::ZenumlDiagramRenderModel,
                crate::zenuml::ZenumlDiagramLayout,
            >,
        >,
    ),
    Flowchart(Box<FlowchartFamilyArtifact<FlowchartLayout>>),
    Swimlane(Box<FlowchartFamilyArtifact<SwimlaneLayout>>),
    #[cfg(feature = "layout-cytoscape")]
    Architecture(
        Box<
            FamilyPair<
                diagrams::architecture::ArchitectureDiagramRenderModel,
                ArchitectureDiagramLayout,
            >,
        >,
    ),
    Class(Box<FamilyPair<ClassDiagram, ClassDiagramLayout>>),
    C4(Box<FamilyPair<diagrams::c4::C4DiagramRenderModel, C4DiagramLayout>>),
    Cynefin(Box<FamilyPair<diagrams::cynefin::CynefinDiagramRenderModel, CynefinDiagramLayout>>),
    Wardley(Box<FamilyPair<diagrams::wardley::WardleyDiagramRenderModel, WardleyDiagramLayout>>),
    Railroad(
        Box<FamilyPair<diagrams::railroad::RailroadDiagramRenderModel, RailroadDiagramLayout>>,
    ),
    Kanban(
        Box<
            FamilyPair<
                diagrams::kanban::KanbanDiagramRenderModel,
                crate::kanban::KanbanPreparedArtifact,
            >,
        >,
    ),
    Gantt(Box<FamilyPair<diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout>>),
    Pie(Box<FamilyPair<diagrams::pie::PieDiagramRenderModel, PieDiagramLayout>>),
    Packet(Box<FamilyPair<diagrams::packet::PacketDiagramRenderModel, PacketDiagramLayout>>),
    Timeline(
        Box<FamilyPair<diagrams::timeline::TimelineDiagramRenderModel, TimelineDiagramLayout>>,
    ),
    Journey(Box<FamilyPair<diagrams::journey::JourneyDiagramRenderModel, JourneyDiagramLayout>>),
    Requirement(
        Box<
            FamilyPair<
                diagrams::requirement::RequirementDiagramRenderModel,
                crate::requirement::RequirementPreparedArtifact,
            >,
        >,
    ),
    Sankey(Box<FamilyPair<diagrams::sankey::SankeyDiagramRenderModel, SankeyDiagramLayout>>),
    Radar(Box<FamilyPair<diagrams::radar::RadarDiagramRenderModel, RadarDiagramLayout>>),
    Info(Box<FamilyPair<diagrams::info::InfoDiagramRenderModel, InfoDiagramLayout>>),
    Treemap(Box<FamilyPair<diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout>>),
    Block(Box<FamilyPair<diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout>>),
    Er(Box<FamilyPair<diagrams::er::ErDiagramRenderModel, ErDiagramLayout>>),
    QuadrantChart(
        Box<
            FamilyPair<
                diagrams::quadrant_chart::QuadrantChartRenderModel,
                QuadrantChartDiagramLayout,
            >,
        >,
    ),
    XyChart(Box<FamilyPair<diagrams::xychart::XyChartDiagramRenderModel, XyChartDiagramLayout>>),
    GitGraph(Box<FamilyPair<diagrams::git_graph::GitGraphRenderModel, GitGraphDiagramLayout>>),
    TreeView(
        Box<FamilyPair<diagrams::tree_view::TreeViewDiagramRenderModel, TreeViewDiagramLayout>>,
    ),
    Ishikawa(
        Box<FamilyPair<diagrams::ishikawa::IshikawaDiagramRenderModel, IshikawaDiagramLayout>>,
    ),
    EventModeling(
        Box<
            FamilyPair<
                diagrams::eventmodeling::EventModelingDiagramRenderModel,
                EventModelingDiagramLayout,
            >,
        >,
    ),
    Venn(Box<FamilyPair<diagrams::venn::VennDiagramRenderModel, VennDiagramLayout>>),
}

#[derive(serde::Serialize)]
enum LayoutProjection<'a> {
    BlockDiagram(&'a BlockDiagramLayout),
    RequirementDiagram(&'a RequirementDiagramLayout),
    #[cfg(feature = "layout-cytoscape")]
    ArchitectureDiagram(&'a ArchitectureDiagramLayout),
    MindmapDiagram(&'a MindmapDiagramLayout),
    SankeyDiagram(&'a SankeyDiagramLayout),
    RadarDiagram(&'a RadarDiagramLayout),
    TreemapDiagram(&'a TreemapDiagramLayout),
    VennDiagram(&'a VennDiagramLayout),
    XyChartDiagram(&'a XyChartDiagramLayout),
    QuadrantChartDiagram(&'a QuadrantChartDiagramLayout),
    #[serde(rename = "FlowchartV2")]
    Flowchart(&'a FlowchartLayout),
    SwimlaneDiagram(&'a SwimlaneLayout),
    #[serde(rename = "StateDiagramV2")]
    StateDiagram(&'a StateDiagramLayout),
    #[serde(rename = "ClassDiagramV2")]
    ClassDiagram(&'a ClassDiagramLayout),
    ErDiagram(&'a ErDiagramLayout),
    SequenceDiagram(&'a SequenceDiagramLayout),
    ZenumlDiagram(&'a crate::zenuml::ZenumlDiagramLayout),
    InfoDiagram(&'a InfoDiagramLayout),
    PacketDiagram(&'a PacketDiagramLayout),
    TimelineDiagram(&'a TimelineDiagramLayout),
    PieDiagram(&'a PieDiagramLayout),
    JourneyDiagram(&'a JourneyDiagramLayout),
    KanbanDiagram(&'a KanbanDiagramLayout),
    GitGraphDiagram(&'a GitGraphDiagramLayout),
    TreeViewDiagram(&'a TreeViewDiagramLayout),
    IshikawaDiagram(&'a IshikawaDiagramLayout),
    EventModelingDiagram(&'a EventModelingDiagramLayout),
    CynefinDiagram(&'a CynefinDiagramLayout),
    WardleyDiagram(&'a WardleyDiagramLayout),
    RailroadDiagram(&'a RailroadDiagramLayout),
    GanttDiagram(&'a GanttDiagramLayout),
    C4Diagram(&'a C4DiagramLayout),
    ErrorDiagram(&'a ErrorDiagramLayout),
}

fn clone_json_value_nonrecursive(value: &serde_json::Value) -> serde_json::Value {
    let mut cloned = rustc_hash::FxHashMap::default();
    let mut stack = vec![(value, false)];

    while let Some((current, visited)) = stack.pop() {
        let current_ptr = std::ptr::from_ref(current);
        if visited {
            let value = match current {
                serde_json::Value::Null => serde_json::Value::Null,
                serde_json::Value::Bool(value) => serde_json::Value::Bool(*value),
                serde_json::Value::Number(value) => serde_json::Value::Number(value.clone()),
                serde_json::Value::String(value) => serde_json::Value::String(value.clone()),
                serde_json::Value::Array(items) => serde_json::Value::Array(
                    items
                        .iter()
                        .filter_map(|item| cloned.remove(&std::ptr::from_ref(item)))
                        .collect(),
                ),
                serde_json::Value::Object(entries) => {
                    let mut object = serde_json::Map::new();
                    for (key, child) in entries {
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(child)) {
                            object.insert(key.clone(), value);
                        }
                    }
                    serde_json::Value::Object(object)
                }
            };
            cloned.insert(current_ptr, value);
            continue;
        }

        stack.push((current, true));
        match current {
            serde_json::Value::Array(items) => {
                for item in items.iter().rev() {
                    stack.push((item, false));
                }
            }
            serde_json::Value::Object(entries) => {
                for child in entries.values().rev() {
                    stack.push((child, false));
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }

    cloned
        .remove(&std::ptr::from_ref(value))
        .unwrap_or(serde_json::Value::Null)
}

impl BuiltinFamilyArtifact {
    pub fn kind(&self) -> RenderFamilyKind {
        match self {
            Self::Error(_) => RenderFamilyKind::Error,
            Self::Mindmap(_) => RenderFamilyKind::Mindmap,
            Self::State(_) => RenderFamilyKind::State,
            Self::Sequence(_) => RenderFamilyKind::Sequence,
            Self::Zenuml(_) => RenderFamilyKind::Zenuml,
            Self::Flowchart(_) => RenderFamilyKind::Flowchart,
            Self::Swimlane(_) => RenderFamilyKind::Swimlane,
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(_) => RenderFamilyKind::Architecture,
            Self::Class(_) => RenderFamilyKind::Class,
            Self::C4(_) => RenderFamilyKind::C4,
            Self::Cynefin(_) => RenderFamilyKind::Cynefin,
            Self::Wardley(_) => RenderFamilyKind::Wardley,
            Self::Railroad(_) => RenderFamilyKind::Railroad,
            Self::Kanban(_) => RenderFamilyKind::Kanban,
            Self::Gantt(_) => RenderFamilyKind::Gantt,
            Self::Pie(_) => RenderFamilyKind::Pie,
            Self::Packet(_) => RenderFamilyKind::Packet,
            Self::Timeline(_) => RenderFamilyKind::Timeline,
            Self::Journey(_) => RenderFamilyKind::Journey,
            Self::Requirement(_) => RenderFamilyKind::Requirement,
            Self::Sankey(_) => RenderFamilyKind::Sankey,
            Self::Radar(_) => RenderFamilyKind::Radar,
            Self::Info(_) => RenderFamilyKind::Info,
            Self::Treemap(_) => RenderFamilyKind::Treemap,
            Self::Block(_) => RenderFamilyKind::Block,
            Self::Er(_) => RenderFamilyKind::Er,
            Self::QuadrantChart(_) => RenderFamilyKind::QuadrantChart,
            Self::XyChart(_) => RenderFamilyKind::XyChart,
            Self::GitGraph(_) => RenderFamilyKind::GitGraph,
            Self::TreeView(_) => RenderFamilyKind::TreeView,
            Self::Ishikawa(_) => RenderFamilyKind::Ishikawa,
            Self::EventModeling(_) => RenderFamilyKind::EventModeling,
            Self::Venn(_) => RenderFamilyKind::Venn,
        }
    }

    fn compatibility_json(
        &self,
        metadata: &ParseMetadata,
    ) -> merman_core::Result<serde_json::Value> {
        match self {
            Self::Error(pair) => pair.compatibility_json(metadata),
            Self::Mindmap(pair) => pair.compatibility_json(metadata),
            Self::State(pair) => pair.compatibility_json(metadata),
            Self::Sequence(pair) => pair.compatibility_json(metadata),
            Self::Zenuml(pair) => pair.compatibility_json(metadata),
            Self::Flowchart(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Swimlane(artifact) => artifact.pair.compatibility_json(metadata),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(pair) => pair.compatibility_json(metadata),
            Self::Class(pair) => pair.compatibility_json(metadata),
            Self::C4(pair) => pair.compatibility_json(metadata),
            Self::Cynefin(pair) => pair.compatibility_json(metadata),
            Self::Wardley(pair) => pair.compatibility_json(metadata),
            Self::Railroad(pair) => pair.compatibility_json(metadata),
            Self::Kanban(pair) => pair.compatibility_json(metadata),
            Self::Gantt(pair) => pair.compatibility_json(metadata),
            Self::Pie(pair) => pair.compatibility_json(metadata),
            Self::Packet(pair) => pair.compatibility_json(metadata),
            Self::Timeline(pair) => pair.compatibility_json(metadata),
            Self::Journey(pair) => pair.compatibility_json(metadata),
            Self::Requirement(pair) => pair.compatibility_json(metadata),
            Self::Sankey(pair) => pair.compatibility_json(metadata),
            Self::Radar(pair) => pair.compatibility_json(metadata),
            Self::Info(pair) => pair.compatibility_json(metadata),
            Self::Treemap(pair) => pair.compatibility_json(metadata),
            Self::Block(pair) => pair.compatibility_json(metadata),
            Self::Er(pair) => pair.compatibility_json(metadata),
            Self::QuadrantChart(pair) => pair.compatibility_json(metadata),
            Self::XyChart(pair) => pair.compatibility_json(metadata),
            Self::GitGraph(pair) => pair.compatibility_json(metadata),
            Self::TreeView(pair) => pair.compatibility_json(metadata),
            Self::Ishikawa(pair) => pair.compatibility_json(metadata),
            Self::EventModeling(pair) => pair.compatibility_json(metadata),
            Self::Venn(pair) => pair.compatibility_json(metadata),
        }
    }

    fn layout_projection(&self) -> LayoutProjection<'_> {
        match self {
            Self::Error(pair) => LayoutProjection::ErrorDiagram(pair.layout()),
            Self::Mindmap(pair) => LayoutProjection::MindmapDiagram(pair.layout()),
            Self::State(pair) => LayoutProjection::StateDiagram(pair.layout()),
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout().layout()),
            Self::Zenuml(pair) => LayoutProjection::ZenumlDiagram(pair.layout()),
            Self::Flowchart(artifact) => LayoutProjection::Flowchart(artifact.pair.layout()),
            Self::Swimlane(artifact) => LayoutProjection::SwimlaneDiagram(artifact.pair.layout()),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(pair) => LayoutProjection::ArchitectureDiagram(pair.layout()),
            Self::Class(pair) => LayoutProjection::ClassDiagram(pair.layout()),
            Self::C4(pair) => LayoutProjection::C4Diagram(pair.layout()),
            Self::Cynefin(pair) => LayoutProjection::CynefinDiagram(pair.layout()),
            Self::Wardley(pair) => LayoutProjection::WardleyDiagram(pair.layout()),
            Self::Railroad(pair) => LayoutProjection::RailroadDiagram(pair.layout()),
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout().layout()),
            Self::Gantt(pair) => LayoutProjection::GanttDiagram(pair.layout()),
            Self::Pie(pair) => LayoutProjection::PieDiagram(pair.layout()),
            Self::Packet(pair) => LayoutProjection::PacketDiagram(pair.layout()),
            Self::Timeline(pair) => LayoutProjection::TimelineDiagram(pair.layout()),
            Self::Journey(pair) => LayoutProjection::JourneyDiagram(pair.layout()),
            Self::Requirement(pair) => LayoutProjection::RequirementDiagram(pair.layout().layout()),
            Self::Sankey(pair) => LayoutProjection::SankeyDiagram(pair.layout()),
            Self::Radar(pair) => LayoutProjection::RadarDiagram(pair.layout()),
            Self::Info(pair) => LayoutProjection::InfoDiagram(pair.layout()),
            Self::Treemap(pair) => LayoutProjection::TreemapDiagram(pair.layout()),
            Self::Block(pair) => LayoutProjection::BlockDiagram(pair.layout()),
            Self::Er(pair) => LayoutProjection::ErDiagram(pair.layout()),
            Self::QuadrantChart(pair) => LayoutProjection::QuadrantChartDiagram(pair.layout()),
            Self::XyChart(pair) => LayoutProjection::XyChartDiagram(pair.layout()),
            Self::GitGraph(pair) => LayoutProjection::GitGraphDiagram(pair.layout()),
            Self::TreeView(pair) => LayoutProjection::TreeViewDiagram(pair.layout()),
            Self::Ishikawa(pair) => LayoutProjection::IshikawaDiagram(pair.layout()),
            Self::EventModeling(pair) => LayoutProjection::EventModelingDiagram(pair.layout()),
            Self::Venn(pair) => LayoutProjection::VennDiagram(pair.layout()),
        }
    }
}

pub struct FamilyRenderArtifact {
    metadata: ParseMetadata,
    compatibility_projection: OnceLock<std::result::Result<serde_json::Value, String>>,
    family: BuiltinFamilyArtifact,
    required_capabilities: Vec<RenderCapability>,
    session: RenderSession,
}

/// A completed renderer-neutral DrawingList produced from one typed family artifact.
pub struct RenderedDrawingList {
    document: DrawingListDocument,
    json: Vec<u8>,
    family_kind: RenderFamilyKind,
    metadata: ParseMetadata,
    required_capabilities: Vec<RenderCapability>,
    session: RenderSession,
}

impl RenderedDrawingList {
    pub fn document(&self) -> &DrawingListDocument {
        &self.document
    }

    pub fn json(&self) -> &[u8] {
        &self.json
    }

    pub const fn media_type(&self) -> &'static str {
        DRAWING_LIST_MEDIA_TYPE
    }

    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
    }

    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required_capabilities
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn into_parts(
        self,
    ) -> (
        DrawingListDocument,
        Vec<u8>,
        RenderFamilyKind,
        ParseMetadata,
        Vec<RenderCapability>,
        RenderSession,
    ) {
        (
            self.document,
            self.json,
            self.family_kind,
            self.metadata,
            self.required_capabilities,
            self.session,
        )
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SvgSerializationRoute {
    /// The SVG was encoded directly from the canonical renderer-neutral document.
    CanonicalDocument,
    /// The family/effect/debug request used the explicit legacy SVG compatibility bridge.
    LegacyBridge,
}

impl SvgSerializationRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalDocument => "canonical-document",
            Self::LegacyBridge => "legacy-bridge",
        }
    }
}

/// Structured evidence for an SVG request that used the compatibility bridge.
///
/// The bridge remains a deliberate migration seam.  Keeping its reason beside the route makes
/// a successful legacy SVG auditable without changing the public DrawingList protocol or the C
/// ABI.  The detail carried by [`Self::DrawingListUnavailable`] is the renderer's structured
/// capability message, not an instruction for a host to retry through another target.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SvgSerializationBridgeReason {
    /// The caller requested timing diagnostics, which are emitted by the legacy path today.
    TimingDiagnostics,
    /// The caller requested the flowchart edge trace diagnostic path.
    FlowchartEdgeTrace,
    /// The family has a typed document adapter but has not crossed the canonical SVG gate.
    LegacyFamily { family: RenderFamilyKind },
    /// The canonical document builder rejected a visual capability and SVG retained its
    /// source-backed legacy representation.
    DrawingListUnavailable { family: String, reason: String },
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
    svg: String,
    family_kind: RenderFamilyKind,
    metadata: ParseMetadata,
    required_capabilities: Vec<RenderCapability>,
    serialization_route: SvgSerializationRoute,
    serialization_bridge_reason: Option<SvgSerializationBridgeReason>,
    session: RenderSession,
}

impl RenderedFamilySvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
    }

    /// Returns the optional capabilities admitted during this artifact's preparation.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required_capabilities
    }

    /// Returns the actual SVG serializer route used for this result.
    ///
    /// A family admitted to the canonical cohort can still use the explicit bridge for a
    /// browser-only effect or a diagnostic request.  Exposing the route keeps that distinction
    /// observable instead of making the family-level coverage row look like a per-request claim.
    pub const fn serialization_route(&self) -> SvgSerializationRoute {
        self.serialization_route
    }

    /// Returns the structured reason when this SVG used the compatibility bridge.
    pub fn serialization_bridge_reason(&self) -> Option<&SvgSerializationBridgeReason> {
        self.serialization_bridge_reason.as_ref()
    }

    /// Applies an output pipeline while retaining the renderer-owned family capability.
    pub fn apply_pipeline(mut self, pipeline: &SvgPipeline) -> Result<Self> {
        self.session.checkpoint(OperationPhase::Postprocess)?;
        let output_metadata = self.output_metadata()?;
        self.svg = pipeline.process_owned_to_string_with_metadata(
            self.svg,
            &output_metadata,
            &self.session,
        )?;
        self.session.work_meter().preflight_svg_byte_count(
            self.svg.len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )?;
        self.session.checkpoint(OperationPhase::Postprocess)?;
        Ok(self)
    }

    /// Finalizes the typed family output for resvg/raster consumption.
    pub fn finalize_resvg(self, pipeline: &SvgPipeline) -> Result<RenderedResvgCompatibleSvg> {
        self.session.checkpoint(OperationPhase::Export)?;
        let output_metadata = self.output_metadata()?;
        let svg = pipeline.process_owned_resvg_compatible_with_metadata(
            self.svg,
            &output_metadata,
            &self.session,
        )?;
        self.session.work_meter().preflight_svg_byte_count(
            svg.as_str().len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Export,
        )?;
        self.session.checkpoint(OperationPhase::Export)?;
        Ok(RenderedResvgCompatibleSvg {
            svg,
            family_kind: self.family_kind,
            metadata: self.metadata,
            serialization_route: self.serialization_route,
            serialization_bridge_reason: self.serialization_bridge_reason,
            session: self.session,
        })
    }

    fn output_metadata(&self) -> Result<SvgPostprocessMetadata> {
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(
            &self.svg,
            SvgPostprocessExecution::new(&self.session),
        )?;
        Ok(metadata
            .with_family_kind(self.family_kind)
            .with_diagram_type(self.metadata.diagram_type.clone())
            .with_optional_diagram_title(self.metadata.title.clone()))
    }

    pub fn into_parts(self) -> (String, RenderFamilyKind, ParseMetadata, RenderSession) {
        (self.svg, self.family_kind, self.metadata, self.session)
    }
}

/// Renderer-owned family output after the terminal resvg compatibility finalizer.
pub struct RenderedResvgCompatibleSvg {
    svg: ResvgCompatibleSvg,
    family_kind: RenderFamilyKind,
    metadata: ParseMetadata,
    serialization_route: SvgSerializationRoute,
    serialization_bridge_reason: Option<SvgSerializationBridgeReason>,
    session: RenderSession,
}

impl RenderedResvgCompatibleSvg {
    pub fn svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub const fn serialization_route(&self) -> SvgSerializationRoute {
        self.serialization_route
    }

    /// Returns the structured reason when this SVG used the compatibility bridge.
    pub fn serialization_bridge_reason(&self) -> Option<&SvgSerializationBridgeReason> {
        self.serialization_bridge_reason.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        ResvgCompatibleSvg,
        RenderFamilyKind,
        ParseMetadata,
        RenderSession,
    ) {
        (self.svg, self.family_kind, self.metadata, self.session)
    }
}

impl FamilyRenderArtifact {
    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn family_kind(&self) -> RenderFamilyKind {
        self.family.kind()
    }

    pub fn gantt_time_axis_diagnostics(&self) -> Option<GanttTimeAxisDiagnostics> {
        let BuiltinFamilyArtifact::Gantt(pair) = &self.family else {
            return None;
        };
        GanttTimeAxisDiagnostics::from_layout(pair.layout())
    }

    /// Builds the complete canonical document and encodes its public DrawingList projection.
    ///
    /// Construction is all-or-nothing: the family adapter must return a validated document
    /// before any bytes are exposed to a caller. Families that have not crossed the migration
    /// gate return a structured capability error instead of an incomplete list.
    pub fn render_drawing_list(
        self,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
    ) -> Result<RenderedDrawingList> {
        self.render_drawing_list_with_diagram_id(policy, limits, None)
    }

    /// Renders using the same normalized instance identity as SVG.
    ///
    /// Identity-dependent geometry (such as Cynefin's default boundary seed) is resolved before
    /// document serialization. `None` preserves the family default; an explicit empty string
    /// follows the normal render-ID normalization rules. Explicit Mermaid seeds take precedence.
    pub fn render_drawing_list_with_diagram_id(
        self,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        diagram_id: Option<&str>,
    ) -> Result<RenderedDrawingList> {
        self.session.checkpoint(OperationPhase::Emit)?;
        let diagram_id = diagram_id
            .map(|id| crate::svg::normalize_render_diagram_id(id, &self.session))
            .transpose()?;
        let document = crate::drawing_list::build_for_family_with_diagram_id(
            &self.family,
            &self.metadata,
            policy,
            limits,
            diagram_id.as_deref(),
            &self.session,
        )?;
        document.admit_serialization(limits, &self.session)?;
        let json = document
            .public
            .canonical_json_bytes_with_limits(&limits)
            .map_err(|error| {
                crate::drawing_list::operation_document_error(
                    Error::DrawingListContract(error),
                    &self.session,
                )
            })?;
        self.session.checkpoint(OperationPhase::Emit)?;
        let public = document.into_public();
        let family_kind = self.family.kind();
        let Self {
            metadata,
            compatibility_projection: _,
            family: _,
            required_capabilities,
            session,
        } = self;
        Ok(RenderedDrawingList {
            document: public,
            json,
            family_kind,
            metadata,
            required_capabilities,
            session,
        })
    }

    pub fn layout_json(&self) -> Result<serde_json::Value> {
        self.session.checkpoint(OperationPhase::Emit)?;
        let semantic = self
            .compatibility_projection
            .get_or_init(|| {
                self.family
                    .compatibility_json(&self.metadata)
                    .map_err(|error| {
                        format!(
                            "failed to project {} compatibility JSON: {error}",
                            self.family.kind()
                        )
                    })
            })
            .as_ref()
            .map_err(|message| Error::InvalidModel {
                message: message.clone(),
            })?;
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
            clone_json_value_nonrecursive(self.metadata.config.as_value()),
        );
        metadata.insert(
            "effective_config".to_string(),
            clone_json_value_nonrecursive(self.metadata.effective_config.as_value()),
        );

        let mut projection = serde_json::Map::new();
        projection.insert("meta".to_string(), serde_json::Value::Object(metadata));
        projection.insert(
            "semantic".to_string(),
            clone_json_value_nonrecursive(semantic),
        );
        projection.insert("layout".to_string(), layout);
        self.session.checkpoint(OperationPhase::Emit)?;
        Ok(serde_json::Value::Object(projection))
    }

    pub fn render_svg(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
    ) -> Result<RenderedFamilySvg> {
        self.session.checkpoint(OperationPhase::Emit)?;
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
        let (svg, serialization_route, serialization_bridge_reason) =
            render_family_artifact_svg(&self, options, render_debug)?;
        admit_rendered_svg_output(&self.session, &svg)?;
        self.session.checkpoint(OperationPhase::Emit)?;
        if let Some((destination, staging, _)) = trace_stage {
            for trace in staging.drain() {
                destination.record(trace);
            }
        }
        let family_kind = self.family.kind();
        let Self {
            metadata,
            compatibility_projection: _,
            family: _,
            required_capabilities,
            session,
        } = self;

        Ok(RenderedFamilySvg {
            svg,
            family_kind,
            metadata,
            required_capabilities,
            serialization_route,
            serialization_bridge_reason,
            session,
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
) -> Result<(
    String,
    SvgSerializationRoute,
    Option<SvgSerializationBridgeReason>,
)> {
    let options = crate::svg::normalize_svg_render_options(request, &artifact.session)?;

    let bridge_reason = if debug.include_timing_diagnostics {
        Some(SvgSerializationBridgeReason::TimingDiagnostics)
    } else if debug.flowchart_edge_trace().is_some() {
        Some(SvgSerializationBridgeReason::FlowchartEdgeTrace)
    } else if !canonical_svg_family_enabled(artifact.family.kind()) {
        Some(SvgSerializationBridgeReason::LegacyFamily {
            family: artifact.family.kind(),
        })
    } else {
        None
    };
    if let Some(bridge_reason) = bridge_reason {
        return render_legacy_family_artifact_svg(artifact, &options, debug).map(|svg| {
            (
                svg,
                SvgSerializationRoute::LegacyBridge,
                Some(bridge_reason),
            )
        });
    }

    // The canonical document serializer is admitted family-by-family.  This keeps the existing
    // source-backed SVG parity contract green while a family cohort is being migrated; the
    // private renderer remains a bridge only for families/effects that do not yet have a
    // source-backed canonical SVG serializer.  A failed document build is never converted into
    // an empty SVG.
    match crate::drawing_list::build_for_family_with_diagram_id(
        &artifact.family,
        &artifact.metadata,
        DrawingListPolicy::AllowRasterSubtree,
        crate::drawing_list::DocumentBudget::SvgOperation,
        options.diagram_id.as_deref(),
        &artifact.session,
    ) {
        Ok(document) => crate::svg::render_document_svg(
            &document,
            &options,
            debug,
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .map(|svg| (svg, SvgSerializationRoute::CanonicalDocument, None)),
        Err(Error::DrawingListUnavailable { family, reason }) => {
            // A family may still contain a browser-only effect that the SVG adapter can emit
            // faithfully while the renderer-neutral contract is not yet able to represent it.
            // Keep this as an explicit, measurable migration bridge rather than hiding a partial
            // DrawingList behind the SVG target.
            render_legacy_family_artifact_svg(artifact, &options, debug).map(|svg| {
                (
                    svg,
                    SvgSerializationRoute::LegacyBridge,
                    Some(SvgSerializationBridgeReason::DrawingListUnavailable { family, reason }),
                )
            })
        }
        Err(error) => Err(error),
    }
}

#[inline(never)]
fn render_legacy_family_artifact_svg(
    artifact: &FamilyRenderArtifact,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    #[cfg(feature = "layout-cytoscape")]
    if let BuiltinFamilyArtifact::Architecture(pair) = &artifact.family {
        return crate::svg::render_architecture_family_artifact(
            pair,
            &artifact.metadata.effective_config,
            &artifact.session,
            options,
            debug,
        );
    }
    crate::svg::render_builtin_family_artifact(
        &artifact.family,
        &artifact.metadata,
        &artifact.session,
        options,
        debug,
    )
}

/// Returns the family cohort whose SVG output is currently emitted by the canonical document
/// serializer. Admission requires full-family source-backed SVG comparison, not only a focused
/// fixture. Candidate builders remain available independently of this public routing gate.
fn canonical_svg_family_enabled(family: RenderFamilyKind) -> bool {
    matches!(
        family,
        RenderFamilyKind::Error
            | RenderFamilyKind::Info
            | RenderFamilyKind::Cynefin
            | RenderFamilyKind::Packet
            | RenderFamilyKind::Pie
            | RenderFamilyKind::Sankey
            | RenderFamilyKind::QuadrantChart
    )
}

#[inline(never)]
fn prepare_pair<S, L>(
    semantic: S,
    layout: impl FnOnce(&S) -> Result<L>,
) -> Result<Box<FamilyPair<S, L>>> {
    let layout = layout(&semantic)?;
    Ok(Box::new(FamilyPair::new(semantic, layout)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FlowchartSvgLabelPreparation(bool);

impl FlowchartSvgLabelPreparation {
    const fn enabled(self) -> bool {
        self.0
    }
}

const DEFAULT_FLOWCHART_SVG_LABEL_PREPARATION: FlowchartSvgLabelPreparation =
    FlowchartSvgLabelPreparation(true);

fn prepare_flowchart_artifact<L>(
    semantic: diagrams::flowchart::FlowchartModel,
    render_context: diagrams::flowchart::FlowchartRenderContext,
    policy: Option<FlowchartPresentationPolicy>,
    svg_label_preparation: FlowchartSvgLabelPreparation,
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderContext,
        Option<&crate::flowchart::FlowchartSvgLabelSidecarBuilder>,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<L>>> {
    let svg_label_sidecar = svg_label_preparation
        .enabled()
        .then(crate::flowchart::FlowchartSvgLabelSidecarBuilder::default);
    let layout = layout(&semantic, &render_context, svg_label_sidecar.as_ref())?;
    let svg_label_sidecar = svg_label_sidecar
        .map(crate::flowchart::FlowchartSvgLabelSidecarBuilder::finish)
        .unwrap_or_default();
    Ok(Box::new(FlowchartFamilyArtifact {
        pair: FamilyPair::new(semantic, layout),
        render_context,
        svg_label_sidecar,
        policy,
    }))
}

fn semantic_flowchart_requires_math(model: &diagrams::flowchart::FlowchartModel) -> bool {
    model
        .nodes
        .iter()
        .filter_map(|node| node.label.as_deref())
        .chain(model.edges.iter().filter_map(|edge| edge.label.as_deref()))
        .chain(
            model
                .subgraphs
                .iter()
                .map(|subgraph| subgraph.title.as_str()),
        )
        .any(crate::math::contains_delimited_math)
}

fn sequence_requires_math(model: &diagrams::sequence::SequenceDiagramRenderModel) -> bool {
    model
        .actors
        .values()
        .map(|actor| actor.description.as_str())
        .chain(model.messages.iter().map(|message| message.message_text()))
        .chain(model.notes.iter().map(|note| note.message.as_str()))
        .any(crate::math::contains_delimited_math)
}

fn mindmap_requires_math(model: &diagrams::mindmap::MindmapDiagramRenderModel) -> bool {
    model
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .any(crate::math::contains_delimited_math)
}

fn parsed_render_requires_math(parsed: &ParsedDiagramRender) -> bool {
    match parsed.model() {
        RenderSemanticModel::Class(model) => crate::class::class_requires_math(model),
        RenderSemanticModel::Flowchart(model) => parsed.flowchart_render_context().map_or_else(
            || semantic_flowchart_requires_math(model),
            |render_context| {
                crate::flowchart::FlowchartRenderModelRef::new(model, render_context)
                    .requires_math()
            },
        ),
        RenderSemanticModel::Mindmap(model) => mindmap_requires_math(model),
        RenderSemanticModel::Sequence(model) => sequence_requires_math(model),
        _ => false,
    }
}

fn capability_is_available(capability: RenderCapability, session: &RenderSession) -> bool {
    session.supports_capability(capability)
}

fn required_capabilities(parsed: &ParsedDiagramRender) -> Vec<RenderCapability> {
    let mut required = Vec::with_capacity(2);
    let meta = parsed.metadata();
    let model = parsed.model();
    let effective_config = &meta.effective_config;
    match model {
        RenderSemanticModel::Architecture(_) => {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Mindmap(_)
            if !crate::mindmap::uses_tidy_tree_layout(effective_config.as_value()) =>
        {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Flowchart(_) | RenderSemanticModel::Class(_)
            if crate::uses_elk_layout(effective_config) =>
        {
            required.push(RenderCapability::LayoutElk);
        }
        RenderSemanticModel::Er(_) if crate::er::uses_elk_layout(effective_config.as_value()) => {
            required.push(RenderCapability::LayoutElk);
        }
        _ => {}
    }

    if parsed_render_requires_math(parsed) {
        required.push(RenderCapability::Math);
    }
    required
}

fn validate_render_input(parsed: &ParsedDiagramRender, session: &RenderSession) -> Result<()> {
    session.checkpoint(OperationPhase::Layout)?;
    let meta = parsed.metadata();
    let model = parsed.model();
    let diagram_type = meta.diagram_type.as_str();
    if let RenderSemanticModel::CustomJson(custom) = model {
        return Err(Error::NonRenderableCustomModel {
            diagram_type: meta.diagram_type.clone(),
            model_name: custom.model_name().to_string(),
            provenance: custom.provenance(),
        });
    }

    if !model.supports_diagram_type(diagram_type) {
        return Err(Error::InvalidModel {
            message: format!(
                "unexpected render model variant {} for diagram type: {diagram_type}",
                model.kind()
            ),
        });
    }

    session
        .work_meter()
        .preflight_parsed_render(parsed, OperationPhase::Layout)?;
    Ok(())
}

/// Plans capability admission for a canonically paired typed render model without running layout.
pub fn plan_render(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<RenderCapabilityPlan> {
    plan_render_with_policy(parsed, session, PresentationRenderPolicy::default())
}

/// Plans capability admission and selected presentation aspects without running layout.
pub fn plan_render_with_policy(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
    render_policy: PresentationRenderPolicy,
) -> Result<RenderCapabilityPlan> {
    let meta = parsed.metadata();
    let model = parsed.model();
    validate_render_input(parsed, session)?;
    let required = required_capabilities(parsed);
    let missing = required
        .iter()
        .copied()
        .filter(|capability| !capability_is_available(*capability, session))
        .collect();
    let flowchart_svg_applicable = matches!(model, RenderSemanticModel::Flowchart(_))
        && meta.effective_config.get_str("layout") != Some("swimlane");
    let presentation_aspects = render_policy.resolve_aspects(
        flowchart_svg_applicable,
        crate::uses_elk_layout(&meta.effective_config),
        capability_is_available(RenderCapability::LayoutElk, session),
    );

    Ok(RenderCapabilityPlan {
        diagram_type: meta.diagram_type.clone(),
        required,
        missing,
        presentation_profile: render_policy.profile(),
        presentation_aspects,
    })
}

#[inline(never)]
fn prepare_class_family(
    model: ClassDiagram,
    meta: &ParseMetadata,
    diagram_type: &str,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    Ok(BuiltinFamilyArtifact::Class(prepare_pair(
        model,
        |model| {
            crate::layout_class_typed_by_engine(
                diagram_type,
                model,
                &meta.effective_config,
                execution,
            )
        },
    )?))
}

#[inline(never)]
fn prepare_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    required_capabilities: Vec<RenderCapability>,
    session: RenderSession,
) -> Result<FamilyRenderArtifact> {
    let (meta, model) = parsed.into_parts();
    let RenderSemanticModel::Class(model) = model else {
        unreachable!("Class render dispatch requires a Class semantic model")
    };
    let diagram_type = meta.diagram_type.as_str();
    let execution = LayoutExecution::new(options, &session);
    let family = prepare_class_family(model, &meta, diagram_type, &execution)?;

    Ok(FamilyRenderArtifact {
        metadata: meta,
        compatibility_projection: OnceLock::new(),
        family,
        required_capabilities,
        session,
    })
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
pub fn prepare(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
) -> Result<FamilyRenderArtifact> {
    prepare_with_render_policy(
        parsed,
        options,
        session,
        PresentationRenderPolicy::default(),
    )
}

/// Prepares one family artifact with renderer policy derived from a resolved presentation.
pub fn prepare_with_render_policy(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
    render_policy: PresentationRenderPolicy,
) -> Result<FamilyRenderArtifact> {
    prepare_with_render_policy_impl(
        parsed,
        options,
        session,
        render_policy,
        DEFAULT_FLOWCHART_SVG_LABEL_PREPARATION,
    )
}

fn prepare_with_render_policy_impl(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
    render_policy: PresentationRenderPolicy,
    flowchart_svg_label_preparation: FlowchartSvgLabelPreparation,
) -> Result<FamilyRenderArtifact> {
    session.checkpoint(OperationPhase::Layout)?;
    let capability_plan = plan_render_with_policy(&parsed, &session, render_policy)?;
    capability_plan.ensure_available()?;
    let required_capabilities = capability_plan.required_capabilities().to_vec();
    // The heterogeneous router has one generic layout call per family. Keep its debug-build
    // caller slots out of the Class Dagre call chain, whose own phase frames are already deep.
    if matches!(parsed.model(), RenderSemanticModel::Class(_)) {
        let artifact = prepare_class_render(parsed, options, required_capabilities, session)?;
        artifact.session.checkpoint(OperationPhase::Layout)?;
        return Ok(artifact);
    }
    let artifact = prepare_non_class_render(
        parsed,
        options,
        session,
        render_policy,
        required_capabilities,
        flowchart_svg_label_preparation,
    )?;
    artifact.session.checkpoint(OperationPhase::Layout)?;
    Ok(artifact)
}

#[inline(never)]
fn prepare_non_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
    render_policy: PresentationRenderPolicy,
    required_capabilities: Vec<RenderCapability>,
    flowchart_svg_label_preparation: FlowchartSvgLabelPreparation,
) -> Result<FamilyRenderArtifact> {
    let (meta, model, render_context) = parsed.into_render_parts();
    let flowchart_label_sources = render_context.into_flowchart_render_context();
    let diagram_type = meta.diagram_type.as_str();
    let execution = LayoutExecution::new(options, &session);
    let effective_config = meta.effective_config.as_value();
    let title = meta.title.as_deref();
    let family = match model {
        RenderSemanticModel::Error(model) => {
            BuiltinFamilyArtifact::Error(prepare_pair(model, |model| {
                crate::error::layout_error_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Mindmap(model) => {
            BuiltinFamilyArtifact::Mindmap(prepare_pair(model, |model| {
                crate::mindmap::layout_mindmap_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.text_measurer(),
                    execution.math_renderer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::State(model) => {
            BuiltinFamilyArtifact::State(prepare_pair(model, |model| {
                crate::state::layout_state_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::Sequence(model) => {
            let text_measurer = session
                .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Layout);
            BuiltinFamilyArtifact::Sequence(prepare_pair(model, |model| {
                crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
                    model,
                    title,
                    effective_config,
                    &text_measurer,
                    execution.math_renderer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Zenuml(model) => {
            BuiltinFamilyArtifact::Zenuml(prepare_pair(model, |model| {
                crate::zenuml::layout_zenuml_diagram_typed(model, execution.text_measurer())
            })?)
        }
        RenderSemanticModel::Flowchart(model)
            if meta.effective_config.get_str("layout") == Some("swimlane") =>
        {
            BuiltinFamilyArtifact::Swimlane(prepare_flowchart_artifact(
                model,
                flowchart_label_sources,
                None,
                flowchart_svg_label_preparation,
                |model, label_sources, svg_label_sidecar| {
                    crate::swimlane::layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
                        model,
                        label_sources,
                        &meta.effective_config,
                        execution.text_measurer(),
                        execution.math_renderer(),
                        svg_label_sidecar,
                        execution.work_meter(),
                    )
                },
            )?)
        }
        RenderSemanticModel::Flowchart(model) => {
            BuiltinFamilyArtifact::Flowchart(prepare_flowchart_artifact(
                model,
                flowchart_label_sources,
                render_policy.flowchart(),
                flowchart_svg_label_preparation,
                |model, label_sources, svg_label_sidecar| {
                    crate::layout_flowchart_typed_with_render_labels_by_engine(
                        diagram_type,
                        model,
                        label_sources,
                        &meta.effective_config,
                        &execution,
                        svg_label_sidecar,
                    )
                },
            )?)
        }
        #[cfg(feature = "layout-cytoscape")]
        RenderSemanticModel::Architecture(model) => {
            BuiltinFamilyArtifact::Architecture(prepare_pair(model, |model| {
                crate::architecture::layout_architecture_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.operation_seed(),
                    execution.work_meter().as_ref(),
                )
            })?)
        }
        #[cfg(not(feature = "layout-cytoscape"))]
        RenderSemanticModel::Architecture(_) => {
            return Err(Error::MissingCapability {
                capability: RenderCapability::LayoutCytoscape,
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::Class(_) => {
            unreachable!("Class models use the stack-bounded family dispatch path")
        }
        RenderSemanticModel::C4(model) => {
            BuiltinFamilyArtifact::C4(prepare_pair(model, |model| {
                crate::c4::layout_c4_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                    execution.container_height,
                    execution.screen_available_width,
                )
            })?)
        }
        RenderSemanticModel::Cynefin(model) => {
            BuiltinFamilyArtifact::Cynefin(prepare_pair(model, |model| {
                crate::cynefin::layout_cynefin_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Wardley(model) => {
            BuiltinFamilyArtifact::Wardley(prepare_pair(model, |model| {
                crate::wardley::layout_wardley_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Railroad(model) => {
            BuiltinFamilyArtifact::Railroad(prepare_pair(model, |model| {
                crate::railroad::layout_railroad_diagram_typed_for_type(
                    model,
                    diagram_type,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Kanban(model) => {
            BuiltinFamilyArtifact::Kanban(prepare_pair(model, |model| {
                crate::kanban::prepare_kanban_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Gantt(model) => {
            BuiltinFamilyArtifact::Gantt(prepare_pair(model, |model| {
                crate::gantt::layout_gantt_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                    execution.local_time_zone(),
                )
            })?)
        }
        RenderSemanticModel::Pie(model) => {
            BuiltinFamilyArtifact::Pie(prepare_pair(model, |model| {
                crate::pie::layout_pie_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Packet(model) => {
            BuiltinFamilyArtifact::Packet(prepare_pair(model, |model| {
                crate::packet::layout_packet_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Timeline(model) => {
            BuiltinFamilyArtifact::Timeline(prepare_pair(model, |model| {
                crate::timeline::layout_timeline_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Journey(model) => {
            BuiltinFamilyArtifact::Journey(prepare_pair(model, |model| {
                crate::journey::layout_journey_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Requirement(model) => {
            BuiltinFamilyArtifact::Requirement(prepare_pair(model, |model| {
                crate::requirement::layout_requirement_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Sankey(model) => {
            BuiltinFamilyArtifact::Sankey(prepare_pair(model, |model| {
                crate::sankey::layout_sankey_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Radar(model) => {
            BuiltinFamilyArtifact::Radar(prepare_pair(model, |model| {
                crate::radar::layout_radar_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Info(model) => {
            BuiltinFamilyArtifact::Info(prepare_pair(model, |model| {
                crate::info::layout_info_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Treemap(model) => {
            BuiltinFamilyArtifact::Treemap(prepare_pair(model, |model| {
                crate::treemap::layout_treemap_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Block(model) => {
            BuiltinFamilyArtifact::Block(prepare_pair(model, |model| {
                crate::block::layout_block_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Er(model) => {
            #[cfg(feature = "layout-elk")]
            {
                BuiltinFamilyArtifact::Er(prepare_pair(model, |model| {
                    crate::er::layout_er_diagram_typed_with_elk_operation_seed(
                        model,
                        effective_config,
                        execution.text_measurer(),
                        execution.elk_operation_seed(),
                        execution.work_meter(),
                    )
                })?)
            }
            #[cfg(not(feature = "layout-elk"))]
            BuiltinFamilyArtifact::Er(prepare_pair(model, |model| {
                crate::er::layout_er_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::QuadrantChart(model) => {
            BuiltinFamilyArtifact::QuadrantChart(prepare_pair(model, |model| {
                crate::quadrantchart::layout_quadrantchart_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::XyChart(model) => {
            BuiltinFamilyArtifact::XyChart(prepare_pair(model, |model| {
                crate::xychart::layout_xychart_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::GitGraph(model) => {
            BuiltinFamilyArtifact::GitGraph(prepare_pair(model, |model| {
                crate::gitgraph::layout_gitgraph_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::TreeView(model) => {
            BuiltinFamilyArtifact::TreeView(prepare_pair(model, |model| {
                crate::tree_view::layout_tree_view_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Ishikawa(model) => {
            BuiltinFamilyArtifact::Ishikawa(prepare_pair(model, |model| {
                crate::ishikawa::layout_ishikawa_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::EventModeling(model) => {
            BuiltinFamilyArtifact::EventModeling(prepare_pair(model, |model| {
                crate::eventmodeling::layout_eventmodeling_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Venn(model) => {
            BuiltinFamilyArtifact::Venn(prepare_pair(model, |model| {
                crate::venn::layout_venn_diagram_typed_with_work_meter(
                    model,
                    title,
                    effective_config,
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::CustomJson(_) => {
            unreachable!("custom JSON models return before built-in family dispatch")
        }
    };
    Ok(FamilyRenderArtifact {
        metadata: meta,
        compatibility_projection: OnceLock::new(),
        family,
        required_capabilities,
        session,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity, TextMeasurementReport,
        TextMeasurementResultKind,
    };
    use crate::text::{TextMetrics, WrapMode};
    use merman_core::{
        CustomJsonProvenance, CustomJsonRenderModel, Engine, OperationControl, ParseOptions,
    };
    use serde_json::{Value, json};

    // Mutation tests opt into diagnostic associations; source parity keeps production defaults.
    fn drawing_list_svg_diagnostics() -> SvgDebugOptions {
        SvgDebugOptions {
            include_drawing_list_metadata: true,
            ..Default::default()
        }
    }

    fn custom_semantic_parser(
        _code: &str,
        meta: &ParseMetadata,
        control: &merman_core::OperationControl,
    ) -> merman_core::OperationControlResult<merman_core::Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(
            json!({ "type": meta.diagram_type, "owner": "semantic" }),
        ))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &ParseMetadata,
        control: &merman_core::OperationControl,
    ) -> merman_core::OperationControlResult<merman_core::Result<CustomJsonRenderModel>> {
        control.checkpoint()?;
        Ok(Ok(CustomJsonRenderModel::new(
            "custom-flowchart",
            json!({ "owner": "render" }),
        )))
    }

    fn session() -> RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn tree_view_candidate_projects_scopes_without_losing_edited_semantics() {
        use merman_display_list::SemanticRole;
        for declarations in [
            "",
            "accTitle: Authored tree\naccDescr: Authored description\n",
        ] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(
                    json!({"securityLevel": "loose"}),
                ))
                .parse_diagram_for_render_model_sync(
                    &format!("treeView-beta\n{declarations}Root\n    child ## details\n"),
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let options = SvgRenderOptions {
                diagram_id: Some("tree-scopes".to_owned()),
                ..Default::default()
            };
            let render = |document: &crate::drawing_list::RenderDocument,
                          debug: &SvgDebugOptions| {
                crate::svg::render_document_svg(
                    document,
                    &options,
                    debug,
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            let svg = render(&document, &SvgDebugOptions::default());
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let root = xml.root_element();
            if declarations.is_empty() {
                assert!(root.attribute("aria-labelledby").is_none());
                assert!(root.attribute("aria-describedby").is_none());
            } else {
                assert_eq!(
                    root.attribute("aria-labelledby"),
                    Some("chart-title-tree-scopes")
                );
                assert_eq!(
                    root.attribute("aria-describedby"),
                    Some("chart-desc-tree-scopes")
                );
                let tags = root
                    .children()
                    .filter(|node| node.is_element())
                    .take(3)
                    .map(|node| node.tag_name().name())
                    .collect::<Vec<_>>();
                assert_eq!(tags, ["title", "desc", "style"]);
            }
            let tree = root
                .children()
                .find(|node| node.attribute("class") == Some("tree-view"))
                .unwrap();
            assert!(
                !tree
                    .descendants()
                    .any(|node| node.has_tag_name("title") || node.has_tag_name("desc"))
            );
            assert!(tree.children().any(|node| node.has_tag_name("line")));
            let child = tree
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("child"))
                .unwrap();
            assert_eq!(child.parent().unwrap().parent(), Some(tree));
            let origin = (child.attribute("x"), child.attribute("y"));

            let diagnostic = render(&document, &drawing_list_svg_diagnostics());
            let diagnostic_xml = roxmltree::Document::parse(&diagnostic).unwrap();
            assert_eq!(
                xml.descendants().filter(|node| node.is_element()).count(),
                diagnostic_xml
                    .descendants()
                    .filter(|node| node.is_element())
                    .count()
            );
            assert!(
                diagnostic_xml
                    .descendants()
                    .any(|node| node.has_tag_name("text")
                        && node.attribute("data-merman-semantic-id").is_some())
            );
            assert!(
                diagnostic_xml
                    .descendants()
                    .any(|node| node.has_tag_name("line")
                        && node.attribute("data-merman-semantic-id").is_some())
            );

            let node = document
                .public
                .semantics
                .iter_mut()
                .find(|annotation| {
                    annotation.role == SemanticRole::Node
                        && annotation.title.as_deref() == Some("child")
                })
                .unwrap();
            let node_id = node.id.clone();
            let label = document
                .public
                .semantics
                .iter_mut()
                .find(|annotation| annotation.id == format!("{node_id}.label"))
                .unwrap();
            label.title = Some("Independent label name".into());
            let label_edit = render(&document, &SvgDebugOptions::default());
            let label_xml = roxmltree::Document::parse(&label_edit).unwrap();
            for name in ["child", "Independent label name"] {
                assert!(
                    label_xml
                        .descendants()
                        .any(|node| node.has_tag_name("title") && node.text() == Some(name))
                );
            }
            let node = document
                .public
                .semantics
                .iter_mut()
                .find(|annotation| annotation.id == node_id)
                .unwrap();
            node.title = Some("Independent node name".into());
            node.description = Some("Independent node description".into());
            node.link = Some("https://example.com/tree?a=1&b=2".into());
            let edited = render(&document, &SvgDebugOptions::default());
            let edited_xml = roxmltree::Document::parse(&edited).unwrap();
            for (tag, content) in [
                ("title", "Independent node name"),
                ("title", "Independent label name"),
                ("desc", "Independent node description"),
            ] {
                assert!(
                    edited_xml
                        .descendants()
                        .any(|node| node.has_tag_name(tag) && node.text() == Some(content))
                );
            }
            assert!(edited_xml.descendants().any(|node| node.has_tag_name("a")
                && node.attribute("href") == Some("https://example.com/tree?a=1&b=2")));
            let child = edited_xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("child"))
                .unwrap();
            assert_eq!((child.attribute("x"), child.attribute("y")), origin);
        }
    }

    #[test]
    fn tree_view_candidate_uses_public_paint_and_background() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "treeView-beta\nsrc/ :::highlight icon(folder) ## source directory\n    main.rs icon(file) ## entry point\n    lib.rs icon(none)\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let options = SvgRenderOptions {
            diagram_id: Some("tree-paint".to_owned()),
            ..Default::default()
        };
        let render = |document: &crate::drawing_list::RenderDocument,
                      config: &serde_json::Value| {
            crate::svg::render_document_svg(
                document,
                &options,
                &drawing_list_svg_diagnostics(),
                config,
                &artifact.session,
            )
            .unwrap()
        };
        let config = artifact.metadata.effective_config.as_value();
        let original = render(&document, config);
        let changed_config = json!({
            "theme": "dark",
            "themeCSS": ".treeView-node-label { fill: red !important; }",
            "themeVariables": {"fontFamily": "Courier", "treeView": {
                "labelColor": "red", "lineColor": "red", "highlightBg": "red"
            }}
        });
        assert_eq!(
            original,
            render(&document, &changed_config),
            "serializing the same TreeView document must not reinterpret config"
        );
        let xml = roxmltree::Document::parse(&original).unwrap();
        assert!(
            xml.root_element()
                .attribute("style")
                .unwrap()
                .contains("background-color: white")
        );
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("data-merman-resource") == Some("treeView.background"))
        );

        let style = xml
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(style.contains("text[class=\"treeView-node-label\"]{"));
        assert!(style.contains("text[class=\"treeView-node-label treeView-node-dir highlight\"]{"));
        assert!(style.contains("white-space:pre"));
        assert!(style.contains("line[class=\"treeView-node-line\"]{"));
        assert!(style.contains("rect[class=\"treeView-highlight-bg\"]{"));
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("line"))
                .all(|node| node.attribute("stroke").is_none()
                    && node.attribute("stroke-width").is_some())
        );
        let highlight = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("treeView-highlight-bg"))
            .unwrap();
        assert_eq!(highlight.attribute("rx"), Some("3"));
        assert_eq!(highlight.attribute("ry"), None);
        assert_eq!(highlight.attribute("fill"), None);

        let mut edited_paths = document.clone();
        let (id, stroke) = edited_paths
            .public
            .commands
            .iter_mut()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style }
                    if path.as_str().starts_with("treeView.line.") =>
                {
                    Some((path.as_str().to_owned(), style.stroke.as_mut().unwrap()))
                }
                _ => None,
            })
            .unwrap();
        stroke.paint = Paint::solid(Color::rgba(0x12, 0x34, 0x56, 128));
        stroke.width = 7.0;
        stroke.dash_array = vec![2.0, 3.0];
        let edited_svg = render(&edited_paths, config);
        let edited_xml = roxmltree::Document::parse(&edited_svg).unwrap();
        let line = edited_xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some(&id))
            .unwrap();
        assert_eq!(line.attribute("stroke"), Some("#123456"));
        assert_eq!(line.attribute("stroke-width"), Some("7"));
        assert_eq!(line.attribute("stroke-dasharray"), Some("2,3"));
        assert!(
            line.attribute("stroke-opacity")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 0.5
        );
        assert!(
            !edited_xml
                .descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .any(|css| css.contains("line[class=\"treeView-node-line\"]{")),
            "one changed edge must disable the shared rule for all edge siblings"
        );
        let main = xml
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("main.rs"))
            .unwrap();
        assert_eq!(
            main.attribute("fill"),
            None,
            "identical sibling styles share CSS"
        );
        let icons = xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-icon"))
            .collect::<Vec<_>>();
        assert_eq!(icons.len(), 2);
        for icon in icons {
            let transform = icon.attribute("transform").unwrap();
            let tokens = svgtypes::TransformListParser::from(transform)
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("icon positioning must be a valid SVG transform");
            assert!(matches!(
                tokens.as_slice(),
                [svgtypes::TransformListToken::Translate { .. }]
            ));
        }

        let mut resized = document.clone();
        let mut scales = 0;
        for command in &mut resized.public.commands {
            if let DrawingCommand::ConcatTransform { transform } = command
                && transform.a != 1.0
            {
                transform.a = 2.0;
                transform.d = 2.0;
                scales += 1;
            }
        }
        assert_eq!(scales, 2);
        let resized = render(&resized, config);
        let resized_xml = roxmltree::Document::parse(&resized).unwrap();
        for icon in resized_xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-icon"))
        {
            let viewport = icon
                .children()
                .find(|node| node.has_tag_name("svg"))
                .unwrap();
            assert_eq!(viewport.attribute("width"), Some("48"));
            assert_eq!(viewport.attribute("height"), Some("48"));
            assert_eq!(viewport.attribute("viewBox"), Some("0 0 24 24"));
        }

        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } if run.text == "main.rs" => {
                    run.text = "main  file".to_owned();
                    run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 255));
                    run.style.font_size = 23.0;
                    run.style.font.weight = 700;
                }
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "treeView.background" =>
                {
                    style.fill = Some(Paint::solid(Color::rgba(171, 205, 239, 255)));
                }
                _ => {}
            }
        }
        let edited = render(&document, config);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert!(
            !xml.root_element()
                .attribute("style")
                .unwrap_or("")
                .contains("background-color:")
        );
        let label = xml
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("main  file"))
            .unwrap();
        let unchanged = xml
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("lib.rs"))
            .unwrap();
        assert!(
            unchanged.attribute("fill").is_some(),
            "heterogeneous siblings both keep explicit paint"
        );
        let style = xml
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(!style.contains("text[class=\"treeView-node-label\"]{"));
        assert_eq!(label.attribute("fill"), Some("#123456"));
        assert_eq!(label.attribute("font-size"), Some("23"));
        assert_eq!(label.attribute("font-weight"), Some("700"));
        assert_eq!(label.attribute("dominant-baseline"), Some("middle"));
        assert_eq!(
            label.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            Some("preserve")
        );
        let background = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some("treeView.background"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#abcdef"));
        document.public.commands.retain(|command| {
            !matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "treeView.background")
        });
        let transparent = render(&document, config);
        let xml = roxmltree::Document::parse(&transparent).unwrap();
        assert!(
            !xml.root_element()
                .attribute("style")
                .unwrap_or("")
                .contains("background-color:")
        );
    }

    #[test]
    fn quadrant_candidate_retains_only_equivalent_paint_spelling() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "themeVariables": {
                    "textColor": "#123456", "quadrant1TextFill": "ff0000",
                    "quadrant2Fill": "#AbCdEf", "quadrant3Fill": "#1234"
                }
            })))
            .parse_diagram_for_render_model_sync(
                "quadrantChart\nquadrant-1 Plan\nA: [0.7, 0.8]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .expect("invalid presentation paint inherits the public root color");
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                &json!({"themeVariables": {"textColor": "red"}}),
                &artifact.session,
            )
            .unwrap()
        };
        let original = render(&document);
        let xml = roxmltree::Document::parse(&original).unwrap();
        let plan = xml
            .descendants()
            .find(|node| node.text() == Some("Plan") && node.has_tag_name("text"))
            .unwrap();
        assert_eq!(plan.attribute("fill"), Some("ff0000"));
        assert!(original.contains("fill:#123456"));
        assert!(
            xml.descendants()
                .any(|node| node.attribute("fill") == Some("#AbCdEf"))
        );
        let alpha = xml
            .descendants()
            .find(|node| node.attribute("fill") == Some("#1234"))
            .unwrap();
        assert_eq!(
            alpha.attribute("fill-opacity"),
            None,
            "source alpha must not be multiplied twice"
        );
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawText { run } = command
                && run.text == "Plan"
            {
                assert_eq!(run.style.fill, Paint::solid(Color::rgba(18, 52, 86, 255)));
                run.style.fill = Paint::solid(Color::rgba(255, 0, 0, 128));
            }
        }
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let plan = xml
            .descendants()
            .find(|node| node.text() == Some("Plan") && node.has_tag_name("text"))
            .unwrap();
        assert_eq!(plan.attribute("fill"), Some("#ff0000"));
        assert_eq!(plan.attribute("fill-opacity"), Some("0.5019607843137255"));
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "quadrantchart.point.0.shape"
            {
                style.fill = None;
                style.stroke = Some(merman_display_list::StrokeStyle {
                    paint: Paint::solid(Color::rgba(120, 154, 188, 255)),
                    width: 3.0,
                    dash_array: vec![],
                    dash_offset: 0.0,
                    line_cap: merman_display_list::LineCap::Butt,
                    line_join: merman_display_list::LineJoin::Miter,
                    miter_limit: 4.0,
                });
            }
        }
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let point = xml
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(point.attribute("fill"), Some("none"));
        assert_eq!(point.attribute("stroke"), Some("#789abc"));
        assert_eq!(
            point.attribute("stroke-width"),
            Some("3"),
            "inert source width cannot override an added public stroke"
        );
        let stroke = document
            .public
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "quadrantchart.point.0.shape" =>
                {
                    style.stroke.clone()
                }
                _ => None,
            })
            .unwrap();
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "quadrantchart.quadrant.0.shape"
            {
                style.stroke = Some(stroke.clone());
            }
        }
        let path = document
            .public
            .resources
            .iter_mut()
            .find(|resource| resource.id().as_str() == "quadrantchart.quadrant.0.shape")
            .unwrap()
            .path_mut()
            .unwrap();
        use merman_display_list::{PathSegment, Point};
        path.segments = vec![
            PathSegment::MoveTo {
                to: Point::new(1.0, 1.0),
            },
            PathSegment::LineTo {
                to: Point::new(1.0, 1.0),
            },
            PathSegment::LineTo {
                to: Point::new(1.0, 9.0),
            },
            PathSegment::LineTo {
                to: Point::new(1.0, 9.0),
            },
            PathSegment::Close,
        ];
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert!(
            xml.descendants().any(
                |node| node.has_tag_name("path") && node.attribute("stroke") == Some("#789abc")
            ),
            "a zero-width rect must not hide a newly stroked public path"
        );
    }

    #[test]
    fn quadrant_candidate_projects_public_styles_and_semantics() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "quadrantChart\n  title Portfolio\n  x-axis Low --> High\n  A: [0.7, 0.8]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument,
                      config: &serde_json::Value| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                config,
                &artifact.session,
            )
            .unwrap()
        };
        let original = render(&document, artifact.metadata.effective_config.as_value());
        assert_eq!(
            original,
            render(
                &document,
                &serde_json::json!({"fontFamily":"monospace", "themeCSS":"text { fill: red; }"})
            ),
            "SVG visual state must not be read from external config"
        );
        let xml = roxmltree::Document::parse(&original).unwrap();
        let main = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("main"))
            .unwrap();
        assert_eq!(main.attribute("id"), None);
        assert_eq!(main.attribute("role"), None);
        assert!(!main.descendants().any(|node| node.has_tag_name("title")));
        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } if run.text == "A" => {
                    run.text = "Edited".to_owned();
                    run.origin.x = 13.0;
                    run.style.fill = Paint::solid(Color::rgba(255, 0, 0, 128));
                }
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "quadrantchart.background" =>
                {
                    style.fill = None;
                }
                _ => {}
            }
        }
        document
            .public
            .semantics
            .iter_mut()
            .find(|semantic| semantic.id == "quadrantchart.point.0")
            .unwrap()
            .description = Some("Independent description".to_owned());
        let edited = render(&document, &serde_json::json!({}));
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert!(
            !xml.root_element()
                .attribute("style")
                .unwrap_or_default()
                .contains("background-color")
        );
        let text = xml
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Edited"))
            .unwrap();
        assert_eq!(text.attribute("x"), Some("13"));
        assert_eq!(text.attribute("fill"), Some("#ff0000"));
        assert_eq!(text.attribute("fill-opacity"), Some("0.5019607843137255"));
        assert!(xml.descendants().any(
            |node| node.has_tag_name("desc") && node.text() == Some("Independent description")
        ));
    }

    #[test]
    fn tree_view_registered_groups_project_shared_public_paint() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let pack = serde_json::json!({"prefix":"test","width":24,"height":24,"icons":{"groups":{"body":r##"<g id="outer" fill="#234567"><g id="shared" fill="#123456" stroke="#654321" stroke-width="2" fill-opacity="0.5" style="stroke-linecap:round"><path d="M0 0H4V4Z"/><path fill="#123456" fill-opacity="0.5" d="M5 0H9V4Z"/></g><g id="overridden" fill="#abcdef"><path fill="#123456" d="M0 5H4V9Z"/></g></g>"##}}});
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            &serde_json::to_vec(&pack).unwrap(),
        )])
        .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(test:groups)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_icon_registry(registry)
            .begin_session()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let legacy = render_legacy_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .unwrap();
        let legacy_xml = roxmltree::Document::parse(&legacy).unwrap();
        let ids: Vec<_> = legacy_xml
            .descendants()
            .filter(|node| node.has_tag_name("g"))
            .filter_map(|node| node.attribute("id"))
            .collect();
        assert_eq!(ids.len(), 3);
        let candidate = render(&document);
        let xml = roxmltree::Document::parse(&candidate).unwrap();
        let group = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(ids[1]))
            .unwrap();
        for property in ["fill", "stroke", "stroke-width", "style"] {
            let source = legacy_xml
                .descendants()
                .find(|node| node.attribute("id") == Some(ids[1]))
                .unwrap();
            assert_eq!(
                group.attribute(property).map(|s| s.trim_end_matches(';')),
                source.attribute(property),
                "{property}"
            );
        }
        assert_eq!(group.attribute("fill-opacity"), Some("0.5"));
        let children: Vec<_> = group.children().filter(|node| node.is_element()).collect();
        assert_eq!(children.len(), 2);
        for property in ["fill", "stroke", "stroke-width", "stroke-linecap"] {
            assert_eq!(
                children[0].attribute(property),
                None,
                "inherited {property}"
            );
        }
        assert_eq!(
            children[0].attribute("fill-opacity"),
            None,
            "inherited paint opacity remains on the source group"
        );
        assert_eq!(
            children[1].attribute("fill"),
            Some("#123456"),
            "explicit child declarations stay local"
        );
        assert_eq!(children[1].attribute("fill-opacity"), Some("0.5"));
        for id in [ids[0], ids[2]] {
            assert_eq!(
                xml.descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap()
                    .attribute("fill"),
                None,
                "nested or fully overridden source paint is not reconstructed"
            );
        }
        // A changed leaf invalidates sharing. The unaffected leaf must still paint the old color.
        let first = document.public.commands.iter().position(|command| matches!(command, DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".asset.shape.0"))).unwrap();
        if let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[first] {
            style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 128)));
        }
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let group = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(ids[1]))
            .unwrap();
        assert_eq!(group.attribute("fill"), None);
        let children: Vec<_> = group.children().filter(|node| node.is_element()).collect();
        assert_eq!(children[0].attribute("fill"), Some("#ff000080"));
        assert_eq!(children[1].attribute("fill"), Some("#123456"));
        // Matching edits recover the shared source placement using the new public color.
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str().ends_with(".asset.shape.1")
            {
                style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 128)));
            }
        }
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let group = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(ids[1]))
            .unwrap();
        assert_eq!(group.attribute("fill"), Some("#ff000080"));
    }

    #[test]
    fn tree_view_registered_groups_preserve_independent_paint_opacity() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let pack = serde_json::json!({"prefix":"test","width":24,"height":24,"icons":{"alpha":{"body":r##"<g fill-opacity="0.5" style="stroke-opacity:0.25"><path fill="#ff000080" stroke="#0000ff80" d="M0 0H4V4Z"/><path fill="#ff000080" stroke="#0000ff80" d="M5 0H9V4Z"/></g>"##}}});
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            &serde_json::to_vec(&pack).unwrap(),
        )])
        .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(test:alpha)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_icon_registry(registry)
            .begin_session()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        for command in &document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str().contains(".asset.shape.")
            {
                assert_eq!(
                    style.fill,
                    Some(Paint::solid(Color::rgba(255, 0, 0, 128)).with_opacity(0.5))
                );
                assert_eq!(style.stroke.as_ref().unwrap().paint.opacity(), 0.25);
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let paths: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#ff000080"))
            .collect();
        assert_eq!(paths.len(), 2);
        for path in paths {
            assert_eq!(path.attribute("fill-opacity"), None);
            assert_eq!(path.attribute("stroke-opacity"), None);
            assert_eq!(path.attribute("stroke"), Some("#0000ff80"));
            let group = path.parent().unwrap();
            assert_eq!(group.attribute("fill-opacity"), Some("0.5"));
            assert_eq!(group.attribute("stroke-opacity"), None);
            assert_eq!(group.attribute("style"), Some("stroke-opacity:0.25;"));
        }
        // Editing only public opacity must leave intrinsic color alpha unchanged and
        // rebuild the inherited declarations, not replay the source factors.
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str().contains(".asset.shape.")
            {
                style.fill = style.fill.take().map(|paint| paint.with_opacity(0.75));
                let stroke = style.stroke.as_mut().unwrap();
                stroke.paint = stroke.paint.clone().with_opacity(0.125);
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let paths: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#ff000080"))
            .collect();
        assert_eq!(paths.len(), 2);
        for path in paths {
            let group = path.parent().unwrap();
            assert_eq!(group.attribute("fill-opacity"), Some("0.75"));
            assert_eq!(group.attribute("style"), Some("stroke-opacity:0.125;"));
        }
    }

    #[test]
    fn tree_view_registered_inline_styles_project_current_public_paint() {
        use merman_display_list::{
            BlendMode, Color, DrawingCommand, DrawingResource, Paint, PathSegment,
        };
        let pack = serde_json::json!({"prefix":"test","width":24,"height":24,"icons":{"styles":{"body":r##"<g fill="#2a68ad" style="stroke:#802000" stroke-width="2" fill-opacity="0.5" stroke-opacity="0.25"><path id="inherited" d="M1 1H8V8H1Z"/><path id="current" color="#10a020" style="fill:currentColor;stroke:currentColor;fill-opacity:1;stroke-opacity:1;fill-rule:evenodd;stroke-width:3;stroke-linecap:round;stroke-linejoin:bevel;stroke-miterlimit:6;stroke-dasharray:2 1;stroke-dashoffset:1;opacity:1" d="M12 1H20V8H12Z"/><path id="override" fill="#ee8800" stroke="none" d="M1 12H8V20H1Z"/></g><g fill="currentColor" color="#ff0000"><circle id="inherited-current" color="#0000ff" cx="17" cy="17" r="4"/></g>"##}}});
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            &serde_json::to_vec(&pack).unwrap(),
        )])
        .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(test:styles)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_icon_registry(registry)
            .begin_session()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let legacy = render_legacy_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let candidate = render(&document);
        let legacy_xml = roxmltree::Document::parse(&legacy).unwrap();
        let xml = roxmltree::Document::parse(&candidate).unwrap();
        let paths: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("id").is_some())
            .collect();
        assert_eq!(paths.len(), 3);
        assert_eq!(
            paths
                .iter()
                .map(|node| node.attribute("id"))
                .collect::<Vec<_>>(),
            legacy_xml
                .descendants()
                .filter(|node| node.has_tag_name("path") && node.attribute("id").is_some())
                .map(|node| node.attribute("id"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            paths[0].attribute("style"),
            None,
            "inherited inline declarations must not acquire child-level precedence"
        );
        assert_eq!(
            paths[1].attribute("style"),
            Some(
                "fill:#10a020;stroke:#10a020;fill-opacity:1;stroke-opacity:1;fill-rule:evenodd;stroke-width:3;stroke-linecap:round;stroke-linejoin:bevel;stroke-miterlimit:6;stroke-dasharray:2,1;stroke-dashoffset:1;opacity:1;"
            )
        );
        assert_eq!(paths[2].attribute("style"), None);
        let id = paths[1].attribute("id").unwrap().to_owned();
        // A public paint edit must update inline values, not replay the source currentColor.
        let draw_index = document.public.commands.iter().position(|command| matches!(command, DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".asset.shape.1"))).unwrap();
        if let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[draw_index] {
            style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 128)));
            style.stroke = None;
        }
        // Force generic geometry. Declaration placement remains associated with the path.
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str().ends_with(".asset.shape.1")
                && let PathSegment::MoveTo { to } = &mut path.segments[0]
            {
                to.x += 1.0;
            }
        }
        document.public.commands.insert(
            draw_index,
            DrawingCommand::SetBlendMode {
                blend_mode: BlendMode::Multiply,
            },
        );
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited)
            .expect("one combined style attribute even with public blending");
        let path = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(id.as_str()))
            .unwrap();
        assert_ne!(path.attribute("d"), Some("M12 1H20V8H12Z"));
        let inline = path.attribute("style").unwrap();
        assert!(inline.contains("fill:#ff000080;stroke:none;fill-opacity:1;stroke-opacity:1;"));
        assert!(inline.contains("mix-blend-mode:multiply;"));
        assert!(!inline.contains("#10a020"));
    }

    #[test]
    fn tree_view_registered_groups_preserve_scoped_identity_and_public_transforms() {
        use merman_display_list::{DrawingCommand, DrawingResource, PathSegment, Transform};
        let pack = serde_json::json!({
            "prefix": "test", "width": 20, "height": 10,
            "icons": { "grouped": { "body": r#"<path d="M0 0H1V1Z"/><g id="outer" transform="translate(1 2)"><rect id="empty" width="0" height="10"/><g id="inner" transform="scale(2)"><path id="face" d="M0 0H2V3Z"/></g></g><path id="tail" d="M1 1H2"/>"# } },
            "aliases": { "turned": { "parent": "grouped", "rotate": 1 } }
        });
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            &serde_json::to_vec(&pack).unwrap(),
        )])
        .unwrap();
        let mut diagram_ids = Vec::new();
        for diagram_id in ["treeView", "asset-custom"] {
            let mut alias_ids = Vec::new();
            for name in ["grouped", "turned"] {
                let parsed = Engine::new()
                    .parse_diagram_for_render_model_sync(
                        &format!("treeView-beta\nRoot icon(test:{name})\n"),
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap();
                let session = crate::environment::RenderEnvironment::deterministic()
                    .with_icon_registry(registry.clone())
                    .begin_session()
                    .unwrap();
                let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
                let options = SvgRenderOptions {
                    diagram_id: Some(diagram_id.to_owned()),
                    ..SvgRenderOptions::default()
                };
                let legacy = render_legacy_family_artifact_svg(
                    &artifact,
                    &options,
                    &SvgDebugOptions::default(),
                )
                .unwrap();
                let mut document = crate::drawing_list::build_for_family_with_diagram_id(
                    &artifact.family,
                    &artifact.metadata,
                    DrawingListPolicy::VectorOnly,
                    DrawingListLimits::default(),
                    Some(diagram_id),
                    &artifact.session,
                )
                .unwrap();
                let render = |document: &crate::drawing_list::RenderDocument| {
                    crate::svg::render_document_svg(
                        document,
                        &options,
                        &drawing_list_svg_diagnostics(),
                        artifact.metadata.effective_config.as_value(),
                        &artifact.session,
                    )
                    .unwrap()
                };
                let svg = render(&document);
                let source = roxmltree::Document::parse(&legacy).unwrap();
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let expected_ids: Vec<_> = source
                    .descendants()
                    .filter_map(|node| node.attribute("id"))
                    .filter(|id| id.starts_with("IconifyId"))
                    .map(str::to_owned)
                    .collect();
                let actual_ids: Vec<_> = xml
                    .descendants()
                    .filter_map(|node| node.attribute("id"))
                    .filter(|id| id.starts_with("IconifyId"))
                    .map(str::to_owned)
                    .collect();
                assert_eq!(
                    expected_ids.len(),
                    5,
                    "two groups, two paths, and the inert rectangle with source ID 1"
                );
                assert_eq!(actual_ids, expected_ids);
                assert!(actual_ids[1].ends_with('1'));
                assert!(actual_ids[2].ends_with('2'));
                assert!(actual_ids[3].ends_with('3'));
                let outer = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[0].as_str()))
                    .unwrap();
                let inner = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[2].as_str()))
                    .unwrap();
                let face = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[3].as_str()))
                    .unwrap();
                assert_eq!(inner.parent(), Some(outer));
                assert_eq!(face.parent(), Some(inner));
                assert_eq!(inner.attribute("transform"), Some("scale(2)"));
                assert_eq!(face.attribute("transform"), None);
                assert_eq!(outer.attribute("transform"), Some("translate(1 2)"));
                let asset_structure = |xml: &roxmltree::Document<'_>| {
                    xml.descendants()
                        .find(|node| {
                            node.has_tag_name("svg") && node.attribute("width") == Some("14")
                        })
                        .unwrap()
                        .descendants()
                        .filter(|node| node.is_element())
                        .map(|node| {
                            (
                                node.tag_name().name().to_owned(),
                                node.attribute("id").map(str::to_owned),
                                node.attribute("transform").map(str::to_owned),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    asset_structure(&xml),
                    asset_structure(&source),
                    "alias wrappers and source element scopes"
                );
                alias_ids.push(actual_ids.clone());

                if name == "grouped" && diagram_id == "treeView" {
                    let mut edited = document.clone();
                    let crate::drawing_list::SvgStructureBody::TreeView(body) =
                        &mut edited.svg.body
                    else {
                        panic!("TreeView sidecar")
                    };
                    let (&empty_start, empty) = body
                        .assets
                        .scopes
                        .iter_mut()
                        .find(|(_, scope)| scope.dom_id.as_deref() == Some(actual_ids[1].as_str()))
                        .unwrap();
                    let crate::drawing_list::AssetScopeKind::EmptyPrimitive(geometry) =
                        &mut empty.kind
                    else {
                        panic!("empty source primitive")
                    };
                    geometry
                        .attributes
                        .iter_mut()
                        .find(|(key, _)| *key == "width")
                        .unwrap()
                        .1 = "4".to_owned();
                    let svg = render(&edited);
                    assert!(
                        !roxmltree::Document::parse(&svg)
                            .unwrap()
                            .descendants()
                            .any(|node| node.attribute("id") == Some(actual_ids[1].as_str())),
                        "a hint cannot introduce painted geometry"
                    );

                    let mut edited = document.clone();
                    let paint = edited.public.commands.iter().find(|command| matches!(command, DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".asset.shape.0"))).unwrap().clone();
                    edited.public.commands.insert(empty_start + 1, paint);
                    if let crate::drawing_list::SvgStructureBody::TreeView(body) =
                        &mut edited.svg.body
                    {
                        body.assets.scopes.get_mut(&empty_start).unwrap().end += 1;
                    }
                    let svg = render(&edited);
                    let xml = roxmltree::Document::parse(&svg).unwrap();
                    assert_eq!(
                        xml.descendants()
                            .filter(|node| node
                                .attribute("data-merman-resource")
                                .is_some_and(|id| id.ends_with(".asset.shape.0")))
                            .count(),
                        2,
                        "empty-element projection must not consume a new draw command"
                    );
                }

                // A changed public matrix must override stale source transform spelling.
                let group_start = match &document.svg.body {
                    crate::drawing_list::SvgStructureBody::TreeView(body) => {
                        *body
                            .assets
                            .scopes
                            .iter()
                            .find(|(_, scope)| {
                                scope.dom_id.as_deref() == Some(actual_ids[0].as_str())
                            })
                            .unwrap()
                            .0
                    }
                    _ => panic!("TreeView sidecar"),
                };
                if let DrawingCommand::ConcatTransform { transform } =
                    &mut document.public.commands[group_start + 1]
                {
                    *transform = Transform {
                        e: 8.0,
                        f: 9.0,
                        ..Transform::IDENTITY
                    };
                } else {
                    panic!("source group transform")
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let outer = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[0].as_str()))
                    .unwrap();
                assert_ne!(outer.attribute("transform"), Some("translate(1 2)"));
                let inner = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[2].as_str()))
                    .unwrap();
                assert_eq!(inner.attribute("transform"), Some("matrix(2 0 0 2 8 9)"));

                // Editing a path uses generic geometry but keeps the source DOM identity.
                for resource in &mut document.public.resources {
                    if let DrawingResource::Path(path) = resource
                        && path.id.as_str().ends_with(".asset.shape.2")
                        && let PathSegment::MoveTo { to } = &mut path.segments[0]
                    {
                        to.x += 1.0;
                    }
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let face = xml
                    .descendants()
                    .find(|node| node.attribute("id") == Some(actual_ids[3].as_str()))
                    .unwrap();
                assert_ne!(face.attribute("d"), Some("M0 0H2V3Z"));

                // A stale scope boundary cannot create a group across a different save lifetime.
                if let crate::drawing_list::SvgStructureBody::TreeView(body) =
                    &mut document.svg.body
                {
                    body.assets.scopes.get_mut(&group_start).unwrap().end += 1;
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                assert!(
                    !xml.descendants()
                        .any(|node| node.attribute("id") == Some(actual_ids[0].as_str()))
                );
                assert!(
                    xml.descendants()
                        .any(|node| node.attribute("id") == Some(actual_ids[3].as_str()))
                );
            }
            assert_eq!(
                alias_ids[0], alias_ids[1],
                "alias names do not change same-node source IDs"
            );
            diagram_ids.push(alias_ids.remove(0));
        }
        assert_ne!(diagram_ids[0], diagram_ids[1]);
    }

    #[test]
    fn tree_view_registered_primitives_keep_source_spelling_and_public_paint() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, PathSegment};

        let source = r#"<path d="M0 0h4v2z"/><rect x="1" y="2" width="4" height="6" rx="1"/><circle cx="5" cy="6" r="2"/><ellipse cx="6" cy="7" rx="2" ry="3"/><line x1="1" y1="2" x2="3" y2="4"/><polygon points="1,1 4,1 2,3"/><polyline points="1,1 4,1 2,3"/>"#;
        let pack = serde_json::json!({
            "prefix": "test", "width": 20, "height": 10,
            "icons": { "primitives": { "body": source } }
        });
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            &serde_json::to_vec(&pack).unwrap(),
        )])
        .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(test:primitives)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_icon_registry(registry)
            .begin_session()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let legacy = render_legacy_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .unwrap();
        let svg = render(&document);
        let legacy_xml = roxmltree::Document::parse(&legacy).unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let original = legacy_xml
            .descendants()
            .find(|node| node.attribute("viewBox") == Some("0 0 20 10"))
            .unwrap();
        let projected = xml
            .descendants()
            .find(|node| node.attribute("viewBox") == Some("0 0 20 10"))
            .unwrap();
        let originals: Vec<_> = original
            .children()
            .filter(|node| node.is_element())
            .collect();
        let projections: Vec<_> = projected
            .children()
            .filter(|node| node.is_element())
            .collect();
        assert_eq!(originals.len(), 7);
        assert_eq!(projections.len(), originals.len());
        for (original, projected) in originals.into_iter().zip(projections) {
            assert_eq!(original.tag_name().name(), projected.tag_name().name());
            for attribute in original.attributes() {
                assert_eq!(
                    projected.attribute(attribute.name()),
                    Some(attribute.value())
                );
            }
        }
        // Source spelling cannot retain stale paints when a host edits the public command.
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str().ends_with(".asset.shape.1")
            {
                style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 128)));
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let rect = xml
            .descendants()
            .find(|node| {
                node.attribute("data-merman-resource")
                    .is_some_and(|id| id.ends_with(".asset.shape.1"))
            })
            .unwrap();
        assert!(rect.has_tag_name("rect"));
        assert_eq!(rect.attribute("fill"), Some("#ff000080"));
        assert_eq!(rect.attribute("fill-opacity"), None);
        // Edited public geometry is drawn generically, never replaced by the old rectangle.
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str().ends_with(".asset.shape.1")
                && let PathSegment::MoveTo { to } = &mut path.segments[0]
            {
                to.x += 1.0;
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants()
                .find(|node| node
                    .attribute("data-merman-resource")
                    .is_some_and(|id| id.ends_with(".asset.shape.1")))
                .unwrap()
                .has_tag_name("path")
        );
        // A malformed hint also falls back; it is not an alternate source of geometry.
        let crate::drawing_list::SvgStructureBody::TreeView(body) = &mut document.svg.body else {
            panic!("TreeView sidecar")
        };
        let (_, hint) = body
            .assets
            .primitives
            .iter_mut()
            .find(|(id, _)| id.ends_with(".asset.shape.2"))
            .unwrap();
        hint.attributes
            .iter_mut()
            .find(|(key, _)| *key == "r")
            .unwrap()
            .1 = "invalid".to_owned();
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants()
                .find(|node| node
                    .attribute("data-merman-resource")
                    .is_some_and(|id| id.ends_with(".asset.shape.2")))
                .unwrap()
                .has_tag_name("path")
        );
    }

    #[test]
    fn tree_view_registered_asset_projects_the_public_viewport() {
        let registry = crate::svg::IconRegistry::from_packs([crate::svg::IconPack::new(
            br#"{"prefix":"test","width":20,"height":10,"icons":{"box":{"body":"<path fill=\"currentColor\" d=\"M0 0H20V10H0Z\"/>"}},"aliases":{"turned":{"parent":"box","rotate":1}}}"#,
        )]).unwrap();
        for (name, expected_box) in [("box", "0 0 20 10"), ("turned", "0 0 10 20")] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(
                    &format!("treeView-beta\nRoot icon(test:{name})\n"),
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_icon_registry(registry.clone())
                .begin_session()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let legacy = render_legacy_family_artifact_svg(
                &artifact,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&legacy).unwrap();
            assert!(
                xml.descendants()
                    .any(|node| node.attribute("viewBox") == Some(expected_box)),
                "a registered asset, not the unknown-icon fallback"
            );
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let render = |document: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let viewport = xml
                .descendants()
                .find(|node| node.attribute("viewBox") == Some(expected_box))
                .expect("registered asset source coordinate basis");
            assert_eq!(viewport.attribute("width"), Some("14"));
            assert_eq!(viewport.attribute("height"), Some("14"));
            assert_eq!(
                viewport.parent().unwrap().attribute("class"),
                Some("treeView-node-icon")
            );
            assert!(!xml.descendants().any(|node| node.has_tag_name("clipPath")));
            assert!(viewport.descendants().any(|node| node.has_tag_name("path")));
            assert!(!viewport.descendants().any(|node| node.has_tag_name("text")));
            let asset_matrix = |svg: &str| {
                let xml = roxmltree::Document::parse(svg).unwrap();
                let path = xml
                    .descendants()
                    .find(|node| {
                        node.attribute("data-merman-resource")
                            .is_some_and(|id| id.ends_with(".asset.shape.0"))
                    })
                    .unwrap();
                let mut matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                for node in path
                    .ancestors()
                    .take_while(|node| !node.has_tag_name("svg"))
                {
                    if let Some(value) = node.attribute("transform") {
                        let t = value.parse::<svgtypes::Transform>().unwrap();
                        let [a, b, c, d, e, f] = matrix;
                        matrix = [
                            t.a * a + t.c * b,
                            t.b * a + t.d * b,
                            t.a * c + t.c * d,
                            t.b * c + t.d * d,
                            t.a * e + t.c * f + t.e,
                            t.b * e + t.d * f + t.f,
                        ];
                    }
                }
                matrix
            };
            let original = asset_matrix(&svg);
            let mut edited = document.clone();
            let crate::drawing_list::SvgStructureBody::TreeView(body) = &mut edited.svg.body else {
                panic!("TreeView sidecar")
            };
            let hint = body.asset_view_boxes.values_mut().next().unwrap();
            hint.x += 5.0;
            hint.y -= 3.0;
            let actual = asset_matrix(&render(&edited));
            let expected = [
                original[0],
                original[1],
                original[2],
                original[3],
                original[4] + 5.0,
                original[5] - 3.0,
            ];
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "viewBox shift is compensated in public geometry"
                );
            }
            for resource in &mut edited.public.resources {
                if let merman_display_list::DrawingResource::Path(path) = resource
                    && path.id.as_str().ends_with(".asset.clip")
                    && let merman_display_list::PathSegment::MoveTo { to } = &mut path.segments[0]
                {
                    to.x += 1.0;
                }
            }
            let svg = render(&edited);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(xml.descendants().any(|node| node.has_tag_name("clipPath")));
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("svg"))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn tree_view_unknown_icon_projects_public_viewport_and_paint() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, PathSegment};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(missing:icon)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(!xml.descendants().any(|node| node.has_tag_name("defs")));
        let viewport = xml
            .descendants()
            .find(|node| node.attribute("viewBox") == Some("0 0 80 80"))
            .unwrap();
        assert_eq!(viewport.attribute("width"), Some("14"));
        assert_eq!(viewport.attribute("height"), Some("14"));
        assert_eq!(
            viewport.parent().unwrap().attribute("class"),
            Some("treeView-node-icon")
        );
        let label = viewport
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.attribute("transform"), Some("translate(21.16 64.67)"));
        assert_eq!(label.first_element_child().unwrap().text(), Some("?"));

        for (clip_factor, scale_factor, width, view_box) in [
            (2.0, 1.0, "28", "0 0 160 160"),
            (1.0, 2.0, "14", "0 0 40 40"),
        ] {
            let mut edited = document.clone();
            for resource in &mut edited.public.resources {
                if let DrawingResource::Path(path) = resource
                    && path.id.as_str().ends_with(".asset.clip")
                {
                    for segment in &mut path.segments {
                        if let PathSegment::MoveTo { to } | PathSegment::LineTo { to } = segment {
                            to.x *= clip_factor;
                            to.y *= clip_factor;
                        }
                    }
                }
            }
            for command in &mut edited.public.commands {
                if let DrawingCommand::ConcatTransform { transform } = command
                    && transform.a == 0.175
                    && transform.d == 0.175
                {
                    transform.a *= scale_factor;
                    transform.d *= scale_factor;
                }
            }
            let svg = render(&edited);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let viewport = xml
                .descendants()
                .find(|node| node.attribute("viewBox") == Some(view_box))
                .unwrap();
            assert_eq!(viewport.attribute("width"), Some(width));
            assert_eq!(viewport.attribute("height"), Some(width));
        }
        let mut shared_clip = document.clone();
        let clip = document
            .public
            .commands
            .iter()
            .find(|command| matches!(command, DrawingCommand::ClipPath { .. }))
            .unwrap()
            .clone();
        let insert = shared_clip.public.commands.len() - 1;
        shared_clip.public.commands.splice(
            insert..insert,
            [DrawingCommand::Save, clip, DrawingCommand::Restore],
        );
        let svg = render(&shared_clip);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants().any(|node| node.has_tag_name("clipPath")),
            "a generic use of the same resource still needs its definition"
        );
        assert!(
            xml.descendants()
                .any(|node| node.attribute("viewBox") == Some("0 0 80 80"))
        );

        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawPath { path, style }
                    if path.as_str().ends_with(".unknown.background") =>
                {
                    style.fill = Some(Paint::solid(Color::rgba(0x12, 0x34, 0x56, 128)));
                }
                DrawingCommand::DrawText { run } if run.text == "?" => {
                    run.text = "!".into();
                    run.origin.x = 19.0;
                }
                _ => {}
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let background = xml
            .descendants()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert!(
            background
                .attribute("style")
                .unwrap()
                .contains("fill:#123456;")
        );
        assert!(
            background
                .attribute("style")
                .unwrap()
                .contains("fill-opacity:0.501960")
        );
        let label = xml
            .descendants()
            .find(|node| node.attribute("transform") == Some("translate(19 64.67)"))
            .unwrap();
        assert_eq!(label.first_element_child().unwrap().text(), Some("!"));

        // A nonrectangular public clip cannot be replaced by an implicit rectangular viewport.
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str().ends_with(".asset.clip")
                && let PathSegment::MoveTo { to } = &mut path.segments[0]
            {
                to.x = 2.0;
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(xml.descendants().any(|node| node.has_tag_name("clipPath")));
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("svg"))
                .count(),
            1
        );
    }

    #[test]
    fn tree_view_candidate_icon_projection_preserves_edited_geometry_and_paint() {
        use merman_display_list::{
            Color, DrawingCommand, DrawingResource, FillRule, Paint, PathSegment,
        };
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "treeView-beta\nRoot icon(folder)\n  child icon(file)\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let icons: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node
                        .attribute("data-merman-resource")
                        .is_some_and(|id| id.ends_with(".icon"))
            })
            .collect();
        assert_eq!(icons.len(), 2);
        for (icon, name) in icons
            .iter()
            .zip(["mermaid-treeview:folder", "mermaid-treeview:file"])
        {
            assert_eq!(
                icon.attribute("d"),
                Some(
                    crate::tree_view::tree_view_builtin_icon(name)
                        .unwrap()
                        .path_data
                )
            );
            assert!(icon.parent().unwrap().has_tag_name("svg"));
        }
        let id = icons[0].attribute("data-merman-resource").unwrap();
        let mut recolored = document.clone();
        for command in &mut recolored.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == id
            {
                style.fill = Some(Paint::solid(Color::rgba(0x12, 0x34, 0x56, 128)));
                style.fill_rule = FillRule::EvenOdd;
            }
        }
        let svg = render(&recolored);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let icon = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some(id))
            .unwrap();
        assert_eq!(icon.attribute("fill"), Some("currentColor"));
        assert_eq!(icon.attribute("fill-rule"), Some("evenodd"));
        assert!(icon.attribute("style").unwrap().contains("color:#123456;"));
        assert!(
            icon.attribute("style")
                .unwrap()
                .contains("fill-opacity:0.501960")
        );

        let mut moved = recolored.clone();
        for resource in &mut moved.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == id
            {
                let PathSegment::MoveTo { to } = &mut path.segments[0] else {
                    panic!("icon starts with MoveTo")
                };
                to.x = 100.0;
            }
        }
        let svg = render(&moved);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let icon = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some(id))
            .unwrap();
        assert!(icon.attribute("d").unwrap().starts_with("M 100 "));
        assert_eq!(icon.attribute("fill"), Some("#123456"));
        assert_eq!(icon.attribute("fill-rule"), Some("evenodd"));
        assert_eq!(
            icon.ancestors()
                .filter(|node| node.has_tag_name("svg"))
                .count(),
            1,
            "modified public geometry must not inherit a private 24x24 clip"
        );
        assert!(icon.attribute("transform").is_some());

        let stroke = document
            .public
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { style, .. } => style.stroke.clone(),
                _ => None,
            })
            .unwrap();
        let mut stroked = document.clone();
        for command in &mut stroked.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == id
            {
                style.stroke = Some(stroke.clone());
            }
        }
        let svg = render(&stroked);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let icon = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some(id))
            .unwrap();
        assert!(
            icon.attribute("stroke")
                .is_some_and(|paint| paint != "none")
        );
        assert_eq!(
            icon.ancestors()
                .filter(|node| node.has_tag_name("svg"))
                .count(),
            1
        );
    }

    #[test]
    fn canonical_svg_diagnostics_only_add_association_attributes() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(json!({
            "securityLevel": "loose"
        }))).parse_diagram_for_render_model_sync(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nTask :a, 2026-01-01, 1d\nclick a href \"https://example.com/task?a=1&b=2\"\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        document
            .public
            .semantics
            .iter_mut()
            .find(|entry| entry.id == "gantt.task.0")
            .unwrap()
            .description = Some("Delivery section".into());
        let run = document
            .public
            .commands
            .iter_mut()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text == "Task" => Some(run),
                _ => None,
            })
            .unwrap();
        // Translucent public text exercises generic text as well as compact task projection.
        run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
        let options = SvgRenderOptions {
            diagram_id: Some("diagnostics".into()),
            ..Default::default()
        };
        let render = |debug: &SvgDebugOptions| {
            crate::svg::render_document_svg(
                &document,
                &options,
                debug,
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let production = render(&SvgDebugOptions::default());
        let diagnostic = render(&drawing_list_svg_diagnostics());
        let production = roxmltree::Document::parse(&production).unwrap();
        let diagnostic = roxmltree::Document::parse(&diagnostic).unwrap();
        let fields = [
            "data-merman-resource",
            "data-merman-semantic-id",
            "data-merman-bounds",
            "data-merman-text-obligation",
        ];
        assert!(
            !production
                .descendants()
                .any(|node| node.attributes().any(|attr| fields.contains(&attr.name())))
        );
        for name in fields {
            assert!(
                diagnostic
                    .descendants()
                    .any(|node| node.attribute(name).is_some()),
                "{name}"
            );
        }
        assert_eq!(
            production.descendants().count(),
            diagnostic.descendants().count()
        );
        for (plain, debug) in production.descendants().zip(diagnostic.descendants()) {
            assert_eq!(plain.node_type(), debug.node_type());
            assert_eq!(plain.tag_name(), debug.tag_name());
            assert_eq!(plain.text(), debug.text());
            let attrs = |node: roxmltree::Node<'_, '_>| {
                let mut attrs = node
                    .attributes()
                    .filter(|attr| !fields.contains(&attr.name()))
                    .map(|attr| {
                        (
                            attr.namespace().map(str::to_owned),
                            attr.name().to_owned(),
                            attr.value().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                attrs.sort();
                attrs
            };
            assert_eq!(attrs(plain), attrs(debug));
        }
        assert!(
            production
                .descendants()
                .any(|node| node.attribute("id") == Some("diagnostics-a"))
        );
        assert!(
            production
                .descendants()
                .any(|node| node.attribute("href") == Some("https://example.com/task?a=1&b=2"))
        );
        assert!(
            production
                .descendants()
                .any(|node| node.attribute("aria-description") == Some("Delivery section"))
        );
    }

    #[test]
    fn gantt_source_whitespace_does_not_consume_final_text_quota() {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection {}\nTask :a, 2026-01-01, 1d\n",
            "#32;".repeat(1024),
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let build = |limits| {
            crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                limits,
                &artifact.session,
            )
        };
        let document = build(DrawingListLimits::default()).unwrap();
        let text_bytes: usize = document
            .public
            .commands
            .iter()
            .map(|command| match command {
                merman_display_list::DrawingCommand::DrawText { run } => run.text.len(),
                _ => 0,
            })
            .sum();
        assert!(text_bytes < 1024);
        let bounded = build(DrawingListLimits {
            max_text_bytes: text_bytes,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(document.public.commands, bounded.public.commands);
    }

    #[test]
    fn canonical_producers_resolve_source_entities_before_public_text() {
        use merman_display_list::DrawingCommand;
        for source in [
            "packet\n0-7: \"#quot; #35;quot; &quot;\"\n",
            "pie\n\"#quot; #35;quot; &quot;\": 1\n",
            "sankey-beta\n#quot; #35;quot; &quot;,B,1\n",
            "cynefin-beta\nclear\n  \"#quot; #35;quot; &quot;\"\n",
            "quadrantChart\n  \"#quot; #35;quot; &quot;\": [0.5, 0.5]\n",
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection #quot; #35;quot; &quot;\nTask :a, 2026-01-01, 1d\n",
        ] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "sankey": { "showValues": false }
                })))
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let expected = "\" #quot; &quot;";
            assert!(
                document
                    .public
                    .commands
                    .iter()
                    .any(|command| matches!(command,
                        DrawingCommand::DrawText { run } if run.text == expected
                    )),
                "missing resolved public text for {source:?}"
            );
            assert!(
                document
                    .public
                    .semantics
                    .iter()
                    .any(|annotation| annotation.title.as_deref() == Some(expected)),
                "missing resolved semantic title for {source:?}"
            );
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                xml.descendants()
                    .any(|node| node.is_text() && node.text() == Some(expected)),
                "missing literal canonical text for {source:?}"
            );
        }
    }

    #[test]
    fn gantt_candidate_preserves_literal_public_text_and_accessibility() {
        use merman_display_list::DrawingCommand;
        let source = "gantt\ntitle #; Title\naccTitle: #; Accessible\naccDescr: #; Description\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection #; Phase\n#; Task :a, 2026-01-01, 1d\n";
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions {
                    diagram_id: Some("literal".to_owned()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for text in [
            "#; Title",
            "#; Accessible",
            "#; Description",
            "#; Phase",
            "#; Task",
        ] {
            assert!(
                xml.descendants()
                    .any(|node| node.is_text() && node.text() == Some(text)),
                "missing literal {text:?}"
            );
        }
        let literal = "#quot; &nbsp; &#160; ﬂ°quot¶ß A]]>B";
        let run = document
            .public
            .commands
            .iter_mut()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text.starts_with("#; Task") => Some(run),
                _ => None,
            })
            .unwrap();
        run.text = literal.to_owned();
        document
            .public
            .semantics
            .iter_mut()
            .find(|s| s.id == "gantt.document")
            .unwrap()
            .description = Some(literal.to_owned());
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.descendants()
                .find(|node| node.attribute("id") == Some("literal-a-text"))
                .unwrap()
                .text(),
            Some(literal)
        );
        assert_eq!(
            xml.descendants()
                .find(|node| node.has_tag_name("desc"))
                .unwrap()
                .text(),
            Some(literal)
        );
    }

    #[test]
    fn gantt_zero_width_tasks_retain_identity_without_painting_a_line() {
        use merman_display_list::DrawingCommand;
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "gantt\ndateFormat YY-MM-DD\nsection A\ny68: t1, 68-01-01, 1d\ny69: t2, 69-01-01, 1d\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let BuiltinFamilyArtifact::Gantt(pair) = &artifact.family else {
            panic!("Gantt")
        };
        assert!(pair.layout().tasks.iter().all(|task| task.bar.width == 0.0));
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let bars: Vec<_> = document
            .public
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawingCommand::DrawPath { path, style }
                    if path.as_str().starts_with("gantt.task.") =>
                {
                    Some(style)
                }
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2);
        assert!(
            bars.iter()
                .all(|style| style.fill.is_none() && style.stroke.is_none()),
            "a collapsed SVG rectangle has no paint, unlike a stroked closed path"
        );
        assert!(
            document
                .public
                .semantics
                .iter()
                .any(|s| s.id == "gantt.task.0" && s.title.as_deref() == Some("y69"))
        );
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let svg_bars: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.attribute("data-merman-resource")
                    .is_some_and(|id| id.starts_with("gantt.task."))
            })
            .collect();
        assert_eq!(svg_bars.len(), 2);
        for bar in svg_bars {
            assert!(bar.has_tag_name("rect"));
            assert_eq!(bar.attribute("width"), Some("0"));
            assert_eq!(bar.attribute("rx"), Some("3"));
            assert_eq!(bar.attribute("ry"), Some("3"));
            let style = bar.attribute("style").unwrap();
            assert!(style.contains("fill:none;"));
            assert!(style.contains("stroke:none;"));
            assert!(bar.attribute("id").is_some());
        }
        assert!(
            xml.descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("y69"))
        );

        // A host may intentionally stroke the retained path. Rect projection would hide it.
        let stroke = document
            .public
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style } if path.as_str() == "gantt.today.line" => {
                    style.stroke.clone()
                }
                _ => None,
            })
            .unwrap();
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str().starts_with("gantt.task.")
            {
                style.stroke = Some(stroke.clone());
            }
        }
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let edited: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.attribute("data-merman-resource")
                    .is_some_and(|id| id.starts_with("gantt.task."))
            })
            .collect();
        assert_eq!(edited.len(), 2);
        for bar in edited {
            assert!(bar.has_tag_name("path"));
            assert!(bar.attribute("stroke").is_some_and(|paint| paint != "none"));
        }
    }

    #[test]
    fn gantt_rectangles_clamp_horizontal_and_vertical_corner_radii_independently() {
        use merman_display_list::{DrawingResource, PathSegment};
        for height in [20.0, 4.0] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "gantt": { "useWidth": 800, "barHeight": height }
                })))
                .parse_diagram_for_render_model_sync(
                    "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Work\nShort :s, 2026-01-01, 1d\nHorizon :h, 2026-01-01, 200d\nMarker :vert, v, 2026-01-02, 1d\n",
                    ParseOptions::strict(),
                ).unwrap().unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let BuiltinFamilyArtifact::Gantt(pair) = &artifact.family else {
                panic!("Gantt")
            };
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let render = |document: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            for (index, task) in pair.layout().tasks.iter().enumerate() {
                let rx = task.bar.rx.min(task.bar.width / 2.0);
                let ry = task.bar.ry.min(task.bar.height / 2.0);
                assert!(rx > 0.0 && ry > 0.0);
                let id = format!("gantt.task.{index}.bar");
                let path = document
                    .public
                    .resources
                    .iter()
                    .find_map(|resource| match resource {
                        DrawingResource::Path(path) if path.id.as_str() == id => Some(path),
                        _ => None,
                    })
                    .unwrap();
                let arcs = path
                    .segments
                    .iter()
                    .filter_map(|segment| match segment {
                        PathSegment::ArcTo {
                            radius_x, radius_y, ..
                        } => Some((*radius_x, *radius_y)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(arcs, vec![(rx, ry); 4], "height={height}, task={}", task.id);
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let rect = xml
                    .descendants()
                    .find(|node| node.attribute("data-merman-resource") == Some(id.as_str()))
                    .unwrap();
                assert!(
                    rect.has_tag_name("rect"),
                    "height={height}, task={}, path={:?}",
                    task.id,
                    path.segments
                );
                assert_eq!(
                    rect.attribute("rx").unwrap().parse::<f64>().unwrap(),
                    task.bar.rx
                );
                assert_eq!(
                    rect.attribute("ry").unwrap().parse::<f64>().unwrap(),
                    task.bar.ry
                );
                // SVG clamps the representation independently on each axis, just like the path.
                assert_eq!(task.bar.rx.min(task.bar.width / 2.0), rx);
                assert_eq!(task.bar.ry.min(task.bar.height / 2.0), ry);
            }
            let task = &pair.layout().tasks[0];
            let id = "gantt.task.0.bar";
            for hint in [
                merman_display_list::Point::new(7.0, task.bar.ry),
                merman_display_list::Point::new(0.1, 0.1),
            ] {
                let crate::drawing_list::SvgStructureBody::Gantt(body) = &mut document.svg.body
                else {
                    unreachable!()
                };
                body.task_radius_attributes.insert(id.into(), hint);
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let rect = xml
                    .descendants()
                    .find(|node| node.attribute("data-merman-resource") == Some(id))
                    .unwrap();
                let rx: f64 = rect.attribute("rx").unwrap().parse().unwrap();
                let ry: f64 = rect.attribute("ry").unwrap().parse().unwrap();
                assert!(
                    (rx.min(task.bar.width / 2.0) - task.bar.rx.min(task.bar.width / 2.0)).abs()
                        < 1e-9
                );
                assert!(
                    (ry.min(task.bar.height / 2.0) - task.bar.ry.min(task.bar.height / 2.0)).abs()
                        < 1e-9
                );
            }
            let crate::drawing_list::SvgStructureBody::Gantt(body) = &mut document.svg.body else {
                unreachable!()
            };
            body.task_radius_attributes
                .insert(id.into(), merman_display_list::Point::new(3.0, 3.0));
            // A public straight-corner rectangle must not inherit the old source rounding.
            let mut straight = document.clone();
            let crate::drawing_list::SvgStructureBody::Gantt(body) = &mut straight.svg.body else {
                unreachable!()
            };
            body.task_radius_attributes
                .insert(id.into(), merman_display_list::Point::new(1e-12, 1e-12));
            let resource = straight
                .public
                .resources
                .iter_mut()
                .find_map(|resource| match resource {
                    DrawingResource::Path(path) if path.id.as_str() == id => Some(path),
                    _ => None,
                })
                .unwrap();
            let point = merman_display_list::Point::new;
            resource.segments = vec![
                PathSegment::MoveTo {
                    to: point(task.bar.x, task.bar.y),
                },
                PathSegment::LineTo {
                    to: point(task.bar.x + task.bar.width, task.bar.y),
                },
                PathSegment::LineTo {
                    to: point(task.bar.x + task.bar.width, task.bar.y + task.bar.height),
                },
                PathSegment::LineTo {
                    to: point(task.bar.x, task.bar.y + task.bar.height),
                },
                PathSegment::Close,
            ];
            let svg = render(&straight);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(id))
                .unwrap();
            assert_eq!(rect.attribute("rx"), None);
            assert_eq!(rect.attribute("ry"), None);
            let path = document
                .public
                .resources
                .iter_mut()
                .find_map(|resource| match resource {
                    DrawingResource::Path(path) if path.id.as_str() == "gantt.task.0.bar" => {
                        Some(path)
                    }
                    _ => None,
                })
                .unwrap();
            let PathSegment::ArcTo { radius_y, .. } = &mut path.segments[2] else {
                panic!("corner")
            };
            *radius_y += 0.25;
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let path = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some("gantt.task.0.bar"))
                .unwrap();
            assert!(
                path.has_tag_name("path"),
                "an independently edited corner must not be replaced with a rectangle"
            );
        }
    }

    #[test]
    fn gantt_resolves_section_and_task_label_cascade_before_serialization() {
        use merman_display_list::{Color, DrawingCommand, Paint, TextAnchor};
        // Source CSS combines specificity, importance and declaration order. In particular,
        // clickable wins over active/done inside labels, but done-outside wins over clickable.
        for (tags, duration, linked, expected) in [
            ("active", "30d", true, "#330000"),
            ("done", "30d", true, "#330000"),
            ("done", "1d", true, "#440000"),
            ("active", "1d", false, "#220000"),
            ("vert", "1d", false, "#550000"),
            ("vert", "1d", true, "#330000"),
            ("vert, active, crit", "1d", false, "#220000"),
            ("vert, done", "1d", false, "#440000"),
        ] {
            let link = if linked {
                "click a href \"https://example.com/task\"\n"
            } else {
                ""
            };
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection First\nLabel :{tags}, a, 2026-01-02, {duration}\n{link}section Second\nHorizon :h, 2026-01-01, 100d\nsection Third\nOther :o, 2026-01-04, 1d\n"
            );
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "securityLevel": "loose",
                    "gantt": { "useWidth": 800 },
                    "themeVariables": {
                        "textColor": "#660000", "taskTextColor": "#110000",
                        "taskTextDarkColor": "#220000", "taskTextClickableColor": "#330000",
                        "taskTextOutsideColor": "#440000", "vertLineColor": "#550000",
                        "sectionBkgColor": "#001100", "altSectionBkgColor": "#002200",
                        "sectionBkgColor2": "#003300"
                    }
                })))
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let BuiltinFamilyArtifact::Gantt(pair) = &artifact.family else {
                panic!("Gantt")
            };
            let task = pair
                .layout()
                .tasks
                .iter()
                .find(|task| task.id == "a")
                .unwrap();
            let outside = task.label.class.contains("taskTextOutside");
            assert_eq!(
                outside,
                duration == "1d",
                "fixture must exercise the intended placement"
            );
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let run = document
                .public
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text == "Label" => Some(run),
                    _ => None,
                })
                .unwrap();
            let red = u8::from_str_radix(&expected[1..3], 16).unwrap();
            assert_eq!(
                run.style.fill,
                Paint::solid(Color::rgba(red, 0, 0, 255)),
                "{tags}, {duration}, linked={linked}"
            );
            assert_eq!(run.style.font.weight, if linked { 700 } else { 400 });
            if tags.contains("vert") {
                assert_eq!(run.anchor, TextAnchor::Middle);
                assert_eq!(run.style.font_size, 15.0);
            }
            for (index, row) in pair.layout().rows.iter().enumerate() {
                let green = if row.class.contains("section0") {
                    0x11
                } else if row.class.contains("section1") {
                    0x22
                } else {
                    0x33
                };
                let style = document
                    .public
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        DrawingCommand::DrawPath { path, style }
                            if path.as_str() == format!("gantt.row.{index}") =>
                        {
                            Some(style)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    style.fill,
                    Some(Paint::solid(Color::rgba(0, green, 0, 255))),
                    "{}",
                    row.class
                );
            }
            let rendered = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions {
                    diagram_id: Some("cascade".to_owned()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&rendered).unwrap();
            let label = xml
                .descendants()
                .find(|node| node.attribute("id") == Some("cascade-a-text"))
                .unwrap();
            assert!(
                label
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("fill:{expected};"))
            );
            if tags.contains("vert") {
                assert!(
                    label
                        .attribute("style")
                        .unwrap()
                        .contains("text-anchor:middle;")
                );
            }
        }
    }

    #[test]
    fn gantt_excludes_project_public_paint_and_preserve_general_strokes() {
        use merman_display_list::{Color, DrawingCommand, Paint, StrokeStyle};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\nTask :a, 2019-02-01, 2019-02-05\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let baseline = render(&document);
        let xml = roxmltree::Document::parse(&baseline).unwrap();
        let rect = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("exclude-range"))
            .unwrap();
        assert!(rect.has_tag_name("rect"));
        assert_eq!(rect.attribute("fill"), None);
        assert_eq!(rect.attribute("stroke"), None);
        assert!(rect.attribute("style").unwrap().contains("fill:#eeeeee;"));
        assert!(rect.attribute("transform-origin").is_some());
        let command = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
                    DrawingCommand::DrawPath { path, .. } if path.as_str() == "gantt.exclude.0"
                )
            })
            .unwrap();
        let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[command] else {
            unreachable!()
        };
        style.fill = Some(Paint::solid(Color::rgba(18, 52, 86, 128)));
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let rect = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("exclude-range"))
            .unwrap();
        let css = rect.attribute("style").unwrap();
        assert!(css.contains("fill:#123456;"));
        let alpha: f64 = css
            .split(';')
            .find_map(|value| value.strip_prefix("fill-opacity:"))
            .unwrap()
            .parse()
            .unwrap();
        assert!((alpha - 128.0 / 255.0).abs() < 1e-6);
        let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[command] else {
            unreachable!()
        };
        style.stroke = Some(StrokeStyle {
            paint: Paint::solid(Color::rgba(171, 205, 239, 255)),
            width: 3.0,
            dash_array: vec![2.0, 4.0],
            dash_offset: 0.0,
            line_cap: merman_display_list::LineCap::Butt,
            line_join: merman_display_list::LineJoin::Miter,
            miter_limit: 4.0,
        });
        let edited = render(&document);
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let rect = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("exclude-range"))
            .unwrap();
        assert_eq!(rect.attribute("fill"), Some("#123456"));
        assert_eq!(rect.attribute("stroke"), Some("#abcdef"));
        assert_eq!(rect.attribute("stroke-width"), Some("3"));
        assert_eq!(rect.attribute("stroke-dasharray"), Some("2,4"));
    }

    #[test]
    fn gantt_transform_origins_retain_source_dom_without_becoming_visual_inputs() {
        use merman_display_list::{DrawingCommand, FillRule, Point, ResourceId, Transform};
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "gantt": { "useWidth": 1100, "leftPadding": 50, "rightPadding": 50 }
            })))
            .parse_diagram_for_render_model_sync(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\nsection A\nShort :s, 2019-02-01, 1d\nMilestone :milestone, m, 2019-02-05, 1d\nVertical :vert, v, 2019-02-07, 1d\nHorizon :h, 2019-02-01, 2019-02-11\n",
                ParseOptions::strict(),
            ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |doc: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                doc,
                &SvgRenderOptions {
                    diagram_id: Some("origins".into()),
                    ..Default::default()
                },
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        // The ten-day, 1000px domain is 100px/day. Origin uses full endTime, while Short's
        // visible bar ends before the weekend and Vertical spans the diagram rather than its row.
        for (id, expected) in [
            ("s", "200px 60px"),
            ("m", "500px 84px"),
            // Mermaid assigns vert order=-1 without consuming an ordinary task row.
            ("v", "700px 36px"),
            ("h", "550px 108px"),
            ("exclude-2019-02-02", "200px 86px"),
            ("exclude-2019-02-09", "900px 110px"),
        ] {
            let id = format!("origins-{id}");
            let element = xml
                .descendants()
                .find(|node| node.attribute("id") == Some(id.as_str()))
                .unwrap();
            assert_eq!(
                element.attribute("transform-origin"),
                Some(expected),
                "{id}"
            );
        }
        let element = xml
            .descendants()
            .find(|node| node.attribute("id") == Some("origins-m"))
            .unwrap();
        let path_id = element
            .attribute("data-merman-resource")
            .unwrap()
            .to_owned();
        let path_index = document.public.commands.iter().position(|command| {
            matches!(command, DrawingCommand::DrawPath { path, .. } if path.as_str() == path_id)
        }).unwrap();
        let DrawingCommand::ConcatTransform {
            transform: milestone,
        } = document.public.commands[path_index - 1]
        else {
            panic!("milestone transform")
        };
        let check_mapping = |node: roxmltree::Node<'_, '_>, public: Transform, ancestor: Point| {
            let origin: Vec<f64> = node
                .attribute("transform-origin")
                .unwrap()
                .split_whitespace()
                .map(|part| part.strip_suffix("px").unwrap().parse().unwrap())
                .collect();
            assert_eq!(origin.len(), 2);
            assert_eq!(node.attribute("transform"), None);
            let matrix = node
                .attribute("style")
                .and_then(|css| {
                    css.split(';')
                        .find_map(|entry| entry.strip_prefix("transform:"))
                })
                .map(|value| {
                    value
                        .strip_prefix("matrix(")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap()
                        .split(',')
                        .map(|part| part.parse::<f64>().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            assert_eq!(matrix.len(), 6);
            // Evaluate SVG's actual origin/matrix/origin sequence, not the encoder's formula.
            for point in [
                Point::new(0.0, 0.0),
                Point::new(19.0, 3.0),
                Point::new(-7.0, 21.0),
            ] {
                let local = Point::new(point.x - origin[0], point.y - origin[1]);
                let actual = Point::new(
                    ancestor.x + origin[0] + matrix[0] * local.x + matrix[2] * local.y + matrix[4],
                    ancestor.y + origin[1] + matrix[1] * local.x + matrix[3] * local.y + matrix[5],
                );
                let expected = Point::new(
                    ancestor.x + public.a * point.x + public.c * point.y + public.e,
                    ancestor.y + public.b * point.x + public.d * point.y + public.f,
                );
                assert!(
                    (actual.x - expected.x).abs() < 1e-9,
                    "{actual:?} != {expected:?}"
                );
                assert!(
                    (actual.y - expected.y).abs() < 1e-9,
                    "{actual:?} != {expected:?}"
                );
            }
        };
        for matrix in [
            Transform::IDENTITY,
            milestone,
            Transform {
                a: 0.9999998,
                b: 0.25,
                c: -0.1,
                d: 1.0000001,
                e: 2.0,
                f: -3.0,
            },
        ] {
            document.public.commands[path_index - 1] =
                DrawingCommand::ConcatTransform { transform: matrix };
            for origin in [
                Point::new(500.0, 84.0),
                Point::new(1000.0000002, -5000.0000004),
            ] {
                let crate::drawing_list::SvgStructureBody::Gantt(body) = &mut document.svg.body
                else {
                    panic!("Gantt")
                };
                body.path_transform_bases.insert(path_id.clone(), origin);
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let element = xml
                    .descendants()
                    .find(|node| node.attribute("data-merman-resource") == Some(path_id.as_str()))
                    .unwrap();
                check_mapping(element, matrix, Point::new(0.0, 0.0));
            }
        }
        // Clip projection moves the existing transform to an ancestor; compensate only the
        // remaining element matrix, not that ancestor's already-emitted translation.
        document.public.commands[path_index - 1] = DrawingCommand::ConcatTransform {
            transform: milestone,
        };
        document.public.commands.splice(
            path_index - 1..path_index - 1,
            [
                DrawingCommand::ConcatTransform {
                    transform: Transform {
                        e: 37.0,
                        f: 19.0,
                        ..Transform::IDENTITY
                    },
                },
                DrawingCommand::ClipPath {
                    path: ResourceId::new("gantt.background"),
                    fill_rule: FillRule::NonZero,
                },
            ],
        );
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let element = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some(path_id.as_str()))
            .unwrap();
        let parent = element.parent().unwrap();
        assert!(parent.attribute("clip-path").is_some());
        assert_eq!(parent.attribute("transform"), Some("matrix(1 0 0 1 37 19)"));
        check_mapping(element, milestone, Point::new(37.0, 19.0));
    }

    #[test]
    fn gantt_milestones_transform_rectangles_and_strokes_without_transforming_labels() {
        use merman_display_list::{DrawingCommand, DrawingResource, PathSegment, Point};
        for tags in ["milestone", "milestone, vert"] {
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Release\nGate :{tags}, gate, 2026-01-01, 1d\nNext :next, 2026-01-03, 1d\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let BuiltinFamilyArtifact::Gantt(pair) = &artifact.family else {
                panic!("Gantt")
            };
            let task = &pair.layout().tasks[0];
            let bar = &task.bar;
            let center_x = bar.x + bar.width / 2.0;
            // Upstream transform-origin follows the task row even for a vertical milestone.
            let center_y = task.order as f64 * 24.0 + 50.0 + 10.0;
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let path_id = "gantt.task.0.bar";
            let index = document
                .public
                .commands
                .iter()
                .position(|command| {
                    matches!(command,
                        DrawingCommand::DrawPath { path, .. } if path.as_str() == path_id
                    )
                })
                .unwrap();
            let DrawingCommand::ConcatTransform { transform } =
                &document.public.commands[index - 1]
            else {
                panic!("milestone geometry and stroke must share a public transform")
            };
            assert!(matches!(
                document.public.commands[index - 2],
                DrawingCommand::Save
            ));
            assert!(matches!(
                document.public.commands[index + 1],
                DrawingCommand::Restore
            ));
            let map = |point: Point| {
                Point::new(
                    transform.a * point.x + transform.c * point.y + transform.e,
                    transform.b * point.x + transform.d * point.y + transform.f,
                )
            };
            let center = map(Point::new(center_x, center_y));
            assert!((center.x - center_x).abs() < 1e-9);
            assert!((center.y - center_y).abs() < 1e-9);
            let right = map(Point::new(center_x + 10.0, center_y));
            assert!((right.x - center.x - 8.0 / 2.0_f64.sqrt()).abs() < 1e-9);
            assert!((right.y - center.y - 8.0 / 2.0_f64.sqrt()).abs() < 1e-9);
            let DrawingCommand::DrawPath { style, .. } = &document.public.commands[index] else {
                unreachable!()
            };
            assert!(
                (style.stroke.as_ref().unwrap().width * transform.a.hypot(transform.b) - 1.6).abs()
                    < 1e-9
            );
            let path = document
                .public
                .resources
                .iter()
                .find_map(|resource| match resource {
                    DrawingResource::Path(path) if path.id.as_str() == path_id => Some(path),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                path.segments.first(),
                Some(&PathSegment::MoveTo {
                    to: Point::new(bar.x + 3.0, bar.y)
                })
            );
            let render = |doc: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    doc,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            assert!(rect.has_tag_name("rect"));
            assert_eq!(rect.attribute("rx"), Some("3"));
            assert!(rect.attribute("stroke-width").is_none());
            assert!(rect.attribute("fill").is_none());
            assert!(rect.attribute("style").unwrap().contains("stroke-width:2;"));
            assert_eq!(
                rect.attribute("data-merman-semantic-id"),
                Some("gantt.task.0")
            );
            assert_eq!(rect.attribute("aria-label"), None);
            assert!(
                rect.parent()
                    .unwrap()
                    .attribute("data-merman-semantic-id")
                    .is_none()
            );
            assert_eq!(rect.attribute("transform"), None);
            let matrix = rect
                .attribute("style")
                .unwrap()
                .split(';')
                .find_map(|entry| entry.strip_prefix("transform:"))
                .unwrap()
                .to_owned();
            assert!(matrix.starts_with("matrix("));
            let label = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Gate"))
                .unwrap();
            assert_eq!(label.attribute("transform"), None);
            let next = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some("gantt.task.1.bar"))
                .unwrap();
            assert_eq!(next.attribute("transform"), None);
            let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[index]
            else {
                unreachable!()
            };
            style.fill = Some(merman_display_list::Paint::solid(
                merman_display_list::Color::rgba(18, 52, 86, 128),
            ));
            style.stroke.as_mut().unwrap().width = 5.0;
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            let css = rect.attribute("style").unwrap();
            assert!(css.contains("fill:#123456;"));
            assert!(css.contains("stroke-width:5;"));
            let alpha = css
                .split(';')
                .find_map(|entry| entry.strip_prefix("fill-opacity:"))
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!((alpha - 128.0 / 255.0).abs() < 1e-6);
            // Non-default stroke semantics must survive via the general serializer.
            let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[index]
            else {
                unreachable!()
            };
            style.stroke.as_mut().unwrap().dash_array = vec![3.0, 2.0];
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            assert_eq!(rect.attribute("stroke-dasharray"), Some("3,2"));
            assert_eq!(rect.attribute("fill"), Some("#123456"));
            let DrawingCommand::ConcatTransform { transform } =
                &mut document.public.commands[index - 1]
            else {
                unreachable!()
            };
            transform.e += 7.0;
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            assert_eq!(rect.attribute("transform"), None);
            let changed_matrix = rect
                .attribute("style")
                .unwrap()
                .split(';')
                .find_map(|entry| entry.strip_prefix("transform:"))
                .unwrap();
            assert_ne!(changed_matrix, matrix);
            // General dash paint, CSS transform and blend must coexist in one valid style block.
            document.public.commands.insert(
                index,
                DrawingCommand::SetBlendMode {
                    blend_mode: merman_display_list::BlendMode::Multiply,
                },
            );
            let blended = render(&document);
            let xml = roxmltree::Document::parse(&blended).unwrap();
            let rect = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            assert_eq!(rect.attribute("stroke-dasharray"), Some("3,2"));
            let css = rect.attribute("style").unwrap();
            assert!(css.contains("transform:matrix("));
            assert!(css.contains("mix-blend-mode:multiply;"));
            document.public.commands.remove(index);
            let path = document
                .public
                .resources
                .iter_mut()
                .find_map(|resource| match resource {
                    DrawingResource::Path(path) if path.id.as_str() == path_id => Some(path),
                    _ => None,
                })
                .unwrap();
            let PathSegment::MoveTo { to } = &mut path.segments[0] else {
                unreachable!()
            };
            to.x += 1.0;
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let path = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(path_id))
                .unwrap();
            assert!(
                path.has_tag_name("path"),
                "edited geometry no longer projects as a rectangle"
            );
            assert_eq!(
                path.attribute("data-merman-semantic-id"),
                Some("gantt.task.0")
            );
            assert_eq!(path.attribute("aria-label"), None);
        }
    }

    #[test]
    fn gantt_task_dom_attribute_uses_document_policy_in_both_text_projections() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        for source_security in ["strict", "loose"] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "securityLevel": source_security,
                    "gantt": { "barHeight": 28 }
                })))
                .parse_diagram_for_render_model_sync(
                    "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nTask :a, 2026-01-01, 1d\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            for alpha in [255, 128] {
                let run = document
                    .public
                    .commands
                    .iter_mut()
                    .find_map(|command| match command {
                        DrawingCommand::DrawText { run } if run.text == "Task" => Some(run),
                        _ => None,
                    })
                    .unwrap();
                assert_ne!(
                    run.bounds.height, 28.0,
                    "DOM annotation is not text geometry"
                );
                run.style.fill = Paint::solid(Color::rgba(18, 52, 86, alpha));
                for encoder_security in ["strict", "loose"] {
                    let svg = crate::svg::render_document_svg(
                        &document,
                        &SvgRenderOptions::default(),
                        &drawing_list_svg_diagnostics(),
                        &json!({"securityLevel": encoder_security}),
                        &artifact.session,
                    )
                    .unwrap();
                    let xml = roxmltree::Document::parse(&svg).unwrap();
                    let label = xml
                        .descendants()
                        .find(|node| node.has_tag_name("text") && node.text() == Some("Task"))
                        .unwrap();
                    assert_eq!(
                        label.attribute("text-height"),
                        (source_security == "loose").then_some("28"),
                        "source={source_security}, encoder={encoder_security}, alpha={alpha}"
                    );
                    assert_eq!(
                        label.attribute("data-merman-bounds").is_some(),
                        alpha != 255
                    );
                }
            }
        }
    }

    #[test]
    fn gantt_section_text_projects_public_lines_including_empty_lines() {
        use merman_display_list::{DrawingCommand, TextBaseline};
        for (section, expected, offsets, preserve, second_preserve) in [
            (
                "Alpha Beta<br/>Gamma",
                vec!["Alpha Beta", "Gamma"],
                vec![-0.5, 0.5],
                false,
                false,
            ),
            (
                "<br/>Alpha<br/><br/>Beta<br/>",
                vec!["", "Alpha", "", "Beta", ""],
                vec![-2.0, 1.0, 1.0, 2.0, 2.0],
                false,
                false,
            ),
            (
                "#quot;<br/>#amp;",
                vec!["\"", "&"],
                vec![-0.5, 0.5],
                false,
                false,
            ),
            (
                "A\u{a0}B<br/>C",
                vec!["A\u{a0}B", "C"],
                vec![-0.5, 0.5],
                false,
                false,
            ),
            ("A<br/> B", vec!["A", " B"], vec![-0.5, 0.5], true, true),
            ("A <br/> B", vec!["A ", "B"], vec![-0.5, 0.5], true, false),
            (
                "A<br/> B<br/> \t<br/> C<br/> \t<br/>",
                vec!["A", " B", " ", "C", "", ""],
                vec![-2.5, -1.5, -0.5, 0.5, 0.5, 0.5],
                true,
                true,
            ),
        ] {
            let source = format!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection {section}\nTask :a, 2026-01-01, 1d\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let start = document
                .public
                .commands
                .iter()
                .position(|command| {
                    matches!(command,
                        DrawingCommand::BeginSemanticGroup { semantic_id }
                        if semantic_id == "gantt.section.0"
                    )
                })
                .unwrap();
            let lines = document.public.commands[start + 1..]
                .iter()
                .take_while(|command| !matches!(command, DrawingCommand::EndSemanticGroup))
                .map(|command| match command {
                    DrawingCommand::DrawText { run } => run.as_ref(),
                    _ => panic!("section text"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                lines
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(
                lines
                    .iter()
                    .all(|run| run.baseline == TextBaseline::Central)
            );
            let render = |document: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            let rendered = render(&document);
            let xml = roxmltree::Document::parse(&rendered).unwrap();
            let text = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node.attribute("class") == Some("sectionTitle sectionTitle0")
                })
                .unwrap();
            assert!(
                text.parent()
                    .unwrap()
                    .attribute("data-merman-semantic-id")
                    .is_none()
            );
            assert_eq!(
                text.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                preserve.then_some("preserve")
            );
            assert_eq!(
                text.attribute("dy").unwrap(),
                format!("{}em", -(expected.len() as f64 - 1.0) / 2.0)
            );
            let center: f64 = text.attribute("y").unwrap().parse().unwrap();
            let spans = text
                .children()
                .filter(|node| node.has_tag_name("tspan"))
                .collect::<Vec<_>>();
            assert_eq!(spans.len(), lines.len());
            for (index, (span, run)) in spans.iter().zip(&lines).enumerate() {
                assert_eq!(span.text().unwrap_or_default(), run.text);
                assert_eq!(span.attribute("alignment-baseline"), Some("central"));
                assert_eq!(span.attribute("dy"), (index != 0).then_some("1em"));
                assert_eq!(
                    span.attribute("x").unwrap().parse::<f64>().unwrap(),
                    run.origin.x
                );
                let offset = offsets[index];
                assert_eq!(center + offset * run.style.font_size, run.origin.y);
                if run.text.is_empty() {
                    assert_eq!(run.bounds.width, 0.0);
                    assert_eq!(run.bounds.height, 0.0);
                }
            }
            assert_eq!(text.attribute("aria-label"), None);
            let semantic_index = document
                .public
                .semantics
                .iter()
                .position(|entry| entry.id == "gantt.section.0")
                .unwrap();
            assert_eq!(
                document.public.semantics[semantic_index].title.as_deref(),
                Some(expected.join("\n").as_str())
            );
            let original_name = document.public.semantics[semantic_index].title.clone();
            let independent_name = "Custom &amp; <br> name";
            document.public.semantics[semantic_index].title = Some(independent_name.into());
            let named = render(&document);
            let xml = roxmltree::Document::parse(&named).unwrap();
            let named_text = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node.attribute("class") == Some("sectionTitle sectionTitle0")
                })
                .unwrap();
            assert_eq!(named_text.attribute("aria-label"), Some(independent_name));
            assert_eq!(
                named_text
                    .children()
                    .filter(|node| node.has_tag_name("tspan"))
                    .count(),
                expected.len()
            );
            document.public.semantics[semantic_index].title = original_name;
            // A moved line no longer has source line spacing. Keep its explicit public position.
            let DrawingCommand::DrawText { run } = &mut document.public.commands[start + 2] else {
                panic!("second line")
            };
            run.origin.y += 5.0;
            let moved_y = run.origin.y;
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let group = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some("gantt.section.0"))
                .unwrap();
            let lines = group
                .children()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), expected.len());
            assert_eq!(lines[1].text(), Some(expected[1]));
            assert_eq!(
                lines[1].attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                second_preserve.then_some("preserve")
            );
            assert_eq!(
                lines[1].attribute("y").unwrap().parse::<f64>().unwrap(),
                moved_y
            );
            let DrawingCommand::DrawText { run } = &mut document.public.commands[start + 2] else {
                panic!("second line")
            };
            run.text = " Alpha  Beta ".to_owned();
            run.style.fill = merman_display_list::Paint::solid(merman_display_list::Color::rgba(
                18, 52, 86, 128,
            ));
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let text = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some(" Alpha  Beta "))
                .unwrap();
            assert_eq!(
                text.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                Some("preserve"),
                "independent public whitespace survives the generic text serializer"
            );
        }
    }

    #[test]
    fn gantt_rows_project_source_rectangles_and_preserve_public_paint_edits() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, PathSegment};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Core\nTask :a, 2026-01-01, 2d\n",
                ParseOptions::strict(),
            ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let row = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("section section0"))
            .unwrap();
        assert_eq!(row.tag_name().name(), "rect");
        assert_eq!(row.attribute("x"), Some("0"));
        assert_eq!(row.attribute("y"), Some("48"));
        assert_eq!(row.attribute("height"), Some("24"));
        assert!(row.attribute("data-merman-resource").is_none());
        assert!(row.attribute("fill").is_none());
        assert!(row.attribute("opacity").is_none());
        assert!(
            row.attribute("style")
                .unwrap()
                .contains("stroke:none;opacity:0.2;")
        );

        let index = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
                    DrawingCommand::DrawPath { path, .. } if path.as_str() == "gantt.row.0"
                )
            })
            .unwrap();
        let DrawingCommand::SetOpacity { opacity } = &mut document.public.commands[index - 1]
        else {
            panic!("row opacity")
        };
        *opacity = 0.4;
        let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[index] else {
            panic!("row path")
        };
        style.fill = Some(Paint::solid(Color::rgba(17, 34, 51, 128)));
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == "gantt.row.0"
            {
                for segment in &mut path.segments {
                    match segment {
                        PathSegment::MoveTo { to } | PathSegment::LineTo { to } => {
                            to.x += 7.0;
                            to.y += 9.0;
                        }
                        PathSegment::Close => {}
                        _ => panic!("row rectangle"),
                    }
                }
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let row = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("section section0"))
            .unwrap();
        assert_eq!(row.attribute("x"), Some("7"));
        assert_eq!(row.attribute("y"), Some("57"));
        let expected = format!(
            "fill:#112233;fill-opacity:{};stroke:none;opacity:0.4;",
            128.0 / 255.0
        );
        assert_eq!(row.attribute("style"), Some(expected.as_str()));

        document.public.commands.insert(
            index,
            DrawingCommand::ConcatTransform {
                transform: merman_display_list::Transform {
                    e: 5.0,
                    f: 11.0,
                    ..merman_display_list::Transform::IDENTITY
                },
            },
        );
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let row = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("section section0"))
            .unwrap();
        assert_eq!(row.attribute("transform"), Some("matrix(1 0 0 1 5 11)"));
        assert_eq!(row.attribute("style"), Some(expected.as_str()));

        document.public.commands.insert(
            index,
            DrawingCommand::SetBlendMode {
                blend_mode: merman_display_list::BlendMode::Multiply,
            },
        );
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).expect("noncompact row remains valid SVG");
        let row = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("section section0"))
            .unwrap();
        assert_eq!(row.attribute("fill"), Some("#112233"));
        assert_eq!(row.attribute("opacity"), Some("0.4"));
        assert_eq!(row.attribute("style"), Some("mix-blend-mode: multiply;"));
    }

    #[test]
    fn gantt_empty_diagram_today_marker_uses_the_browser_zero_coordinate() {
        use merman_display_list::{DrawingResource, PathSegment, Point};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "gantt\ntitle Empty    schedule\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let marker = document
            .public
            .resources
            .iter_mut()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "gantt.today.line" => Some(path),
                _ => None,
            })
            .expect("the empty source diagram still paints a today marker");
        assert_eq!(
            marker.segments,
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 25.0)
                },
                PathSegment::LineTo {
                    to: Point::new(0.0, 75.0)
                },
            ]
        );
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(xml.root_element().attribute("aria-labelledby"), None);
        assert!(!xml.descendants().any(|node| node.has_tag_name("title")));
        assert_eq!(
            xml.descendants()
                .find(|node| node.attribute("class") == Some("titleText"))
                .unwrap()
                .text(),
            Some("Empty schedule")
        );
        let line = xml
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("today"))
            .unwrap();
        assert_eq!(line.parent().unwrap().attribute("class"), Some("today"));
        assert_eq!(line.attribute("x1"), Some("0"));
        assert_eq!(line.attribute("x2"), Some("0"));
        assert_eq!(line.attribute("y1"), Some("25"));
        assert_eq!(line.attribute("y2"), Some("75"));
        assert!(line.attribute("style").unwrap().contains("stroke:#ff0000;"));
        assert!(!svg.contains("NaN"));
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == "gantt.today.line"
            {
                for segment in &mut path.segments {
                    match segment {
                        PathSegment::MoveTo { to } | PathSegment::LineTo { to } => to.x = 37.0,
                        _ => panic!("marker line"),
                    }
                }
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let line = xml
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("today"))
            .unwrap();
        assert_eq!(line.attribute("x1"), Some("37"));
        assert_eq!(line.attribute("x2"), Some("37"));
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("gantt\ntodayMarker off\n", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let disabled = prepare(parsed, &LayoutOptions::default(), session())
            .unwrap()
            .render_drawing_list(DrawingListPolicy::VectorOnly, DrawingListLimits::default())
            .unwrap();
        assert!(
            !disabled
                .document()
                .semantics
                .iter()
                .any(|semantic| semantic.id == "gantt.today")
        );
    }

    #[test]
    fn gantt_today_collection_preserves_marker_children_and_public_metadata() {
        use merman_display_list::DrawingCommand;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker stroke:blue,stroke-width:5px,opacity:0.5\nTask :a, 2026-01-01, 2d\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        assert_eq!(
            document
                .public
                .semantics
                .iter()
                .find(|semantic| semantic.id == "gantt.today")
                .unwrap()
                .title,
            None
        );
        let today_path = document.public.commands.iter().position(|command| {
            matches!(command, DrawingCommand::DrawPath { path, .. } if path.as_str() == "gantt.today.line")
        }).unwrap();
        for (title, description, opacity) in [
            (None, None, 0.5),
            (Some("Today"), None, 0.25),
            (Some("Edited marker"), Some("Public description"), 0.0),
            (None, Some("Description without a name"), 1.0),
        ] {
            let DrawingCommand::SetOpacity { opacity: value } =
                &mut document.public.commands[today_path - 1]
            else {
                panic!("today opacity");
            };
            *value = opacity;
            let semantic = document
                .public
                .semantics
                .iter_mut()
                .find(|semantic| semantic.id == "gantt.today")
                .unwrap();
            semantic.title = title.map(str::to_owned);
            semantic.description = description.map(str::to_owned);
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let group = xml
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("today"))
                .unwrap();
            assert_eq!(group.attribute("id"), None);
            assert_eq!(
                group.attribute("role"),
                (title.is_some() || description.is_some()).then_some("group")
            );
            assert_eq!(group.attribute("aria-label"), title);
            assert_eq!(group.attribute("aria-description"), description);
            let children: Vec<_> = group
                .children()
                .filter(roxmltree::Node::is_element)
                .collect();
            assert_eq!(children.len(), 1);
            assert!(children[0].has_tag_name("line"));
            assert_eq!(children[0].attribute("class"), Some("today"));
            assert!(children[0].attribute("stroke").is_none());
            assert_eq!(children[0].attribute("opacity"), None);
            let style = children[0].attribute("style").unwrap();
            let opacity_declarations: Vec<_> = style
                .split(';')
                .filter(|part| part.starts_with("opacity:"))
                .collect();
            let expected = format!("opacity:{opacity}");
            assert_eq!(
                opacity_declarations,
                if opacity == 1.0 {
                    vec![]
                } else {
                    vec![expected.as_str()]
                }
            );
            assert!(style.contains("stroke:#0000ff;"));
            assert!(style.contains("stroke-width:5;"));
            assert!(
                children[0]
                    .attribute("style")
                    .unwrap()
                    .contains("stroke-width:")
            );
        }
    }

    #[test]
    fn gantt_collection_projection_preserves_public_navigation_and_metadata() {
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "securityLevel": "loose", "gantt": { "topAxis": true }
            })))
            .parse_diagram_for_render_model_sync(
                "gantt\ntitle Schedule\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection Delivery\nTask :a, 2026-01-01, 3d\n",
                ParseOptions::strict(),
            ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let baseline = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        for id in ["gantt.axis.bottom", "gantt.axis.top", "gantt.today"] {
            assert_eq!(
                baseline
                    .public
                    .semantics
                    .iter()
                    .find(|entry| entry.id == id)
                    .unwrap()
                    .title,
                None
            );
        }
        for id in [
            "gantt.document",
            "gantt.title",
            "gantt.excludes",
            "gantt.rows",
            "gantt.tasks",
            "gantt.sections",
            "gantt.section.0",
            "gantt.axis.bottom",
            "gantt.axis.top",
            "gantt.axis.bottom.tick.0",
            "gantt.today",
        ] {
            let mut document = baseline.clone();
            let semantic = document
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap();
            semantic.title = Some("Public <name>".to_owned());
            semantic.description = Some("Public & description".to_owned());
            semantic.link = Some("https://example.com/schedule?a=1&b=2".to_owned());
            let rendered = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&rendered).unwrap();
            let group = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some(id))
                .unwrap_or_else(|| panic!("missing public scope {id}"));
            assert!(
                group
                    .ancestors()
                    .any(|node| node.attribute("href")
                        == Some("https://example.com/schedule?a=1&b=2")),
                "missing link for {id}"
            );
            assert!(
                group
                    .children()
                    .any(|node| node.has_tag_name("title") && node.text() == Some("Public <name>")),
                "missing title for {id}"
            );
            assert!(
                group
                    .children()
                    .any(|node| node.has_tag_name("desc")
                        && node.text() == Some("Public & description")),
                "missing description for {id}"
            );
        }
        for id in [
            "gantt.document",
            "gantt.title",
            "gantt.section.0",
            "gantt.axis.bottom",
            "gantt.axis.top",
            "gantt.axis.bottom.tick.0",
        ] {
            let mut document = baseline.clone();
            document
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .title = Some("Distinct accessible name".to_owned());
            let rendered = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&rendered).unwrap();
            assert!(
                xml.descendants().any(|node| node.attribute("aria-label")
                    == Some("Distinct accessible name")
                    || node.has_tag_name("title")
                        && node.text() == Some("Distinct accessible name")),
                "missing independent name for {id}"
            );
        }
        let rendered = crate::svg::render_document_svg(
            &baseline,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&rendered).unwrap();
        let ticks: Vec<_> = xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("tick"))
            .collect();
        assert!(!ticks.is_empty());
        assert!(
            ticks
                .iter()
                .all(|node| node.attribute("aria-label").is_none()
                    && node.attribute("role").is_none())
        );
        for grid in xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("grid"))
        {
            assert_eq!(grid.attribute("aria-label"), None);
            assert_eq!(grid.attribute("role"), None);
        }
        for id in [
            "gantt.section.0",
            "gantt.axis.bottom.tick.0",
            "gantt.today",
            "gantt.axis.bottom",
        ] {
            let mut document = baseline.clone();
            document
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .role = merman_display_list::SemanticRole::Node;
            let rendered = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&rendered).unwrap();
            let group = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some(id))
                .unwrap();
            assert!(
                group
                    .attribute("class")
                    .unwrap()
                    .split_whitespace()
                    .any(|class| class == "semantic-node")
            );
        }
    }

    #[test]
    fn gantt_candidate_root_projects_public_collections_and_authored_accessibility() {
        use merman_display_list::{Color, DrawingCommand, Paint};

        for (diagram_title, accessibility, excludes, collection_offset) in [
            ("Example", "", "", 0),
            (
                "Example",
                "accTitle: Schedule\naccDescr: Delivery plan\n",
                "excludes weekends\n",
                1,
            ),
            ("", "", "", 0),
        ] {
            let title_source = if diagram_title.is_empty() {
                String::new()
            } else {
                format!("title {diagram_title}\n")
            };
            let source = format!(
                "gantt\n{title_source}{accessibility}dateFormat YYYY-MM-DD\n{excludes}todayMarker off\nsection Core\nTask :a, 2026-01-01, 1d\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let render = |document: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions {
                        diagram_id: Some("schedule".to_owned()),
                        ..Default::default()
                    },
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap()
            };
            let rendered = render(&document);
            let svg = roxmltree::Document::parse(&rendered).unwrap();
            let root = svg.root_element();
            assert_eq!(
                root.attribute("aria-labelledby"),
                (!accessibility.is_empty()).then_some("chart-title-schedule"),
                "only authored accTitle creates a root title reference"
            );
            assert_eq!(
                root.attribute("aria-describedby"),
                (!accessibility.is_empty()).then_some("chart-desc-schedule")
            );
            let groups = root
                .children()
                .filter(|node| node.has_tag_name("g"))
                .collect::<Vec<_>>();
            assert_eq!(
                groups.len(),
                5 + collection_offset,
                "source root owns empty, grid, rows, tasks and section groups"
            );
            if collection_offset != 0 {
                assert!(
                    !groups[1].children().any(|node| node.is_element()),
                    "empty exclude collection must retain its source wrapper"
                );
            }
            assert!(groups[2 + collection_offset].children().any(|node| {
                node.has_tag_name("rect")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.contains("section"))
            }));
            assert!(
                groups[3 + collection_offset]
                    .descendants()
                    .any(|node| node.attribute("id") == Some("schedule-a"))
            );
            let title = root
                .children()
                .find(|node| {
                    node.has_tag_name("text") && node.attribute("class") == Some("titleText")
                })
                .unwrap();
            assert_eq!(title.text().unwrap_or_default(), diagram_title);
            assert!(title.attribute("font-size").is_none());
            assert!(title.attribute("data-merman-bounds").is_none());
            assert!(!title.attribute("style").unwrap().contains("font-size:"));
            assert!(
                svg.descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .collect::<String>()
                    .contains("#schedule .titleText{font-size:18px;}")
            );
            assert!(!svg.descendants().any(|node| {
                node.has_tag_name("rect")
                    && node
                        .attribute("width")
                        .and_then(|width| width.parse::<f64>().ok())
                        == Some(document.public.viewport.bounds.width)
                    && node.attribute("fill") == Some("#ffffff")
            }));

            // Changing the public background must disable root CSS projection, not double-paint it.
            let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[2] else {
                panic!("background command")
            };
            style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 128)));
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            assert!(
                !xml.root_element()
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("background")
            );
            let red_backgrounds = xml
                .descendants()
                .filter(|node| node.attribute("fill") == Some("#ff0000"))
                .collect::<Vec<_>>();
            assert_eq!(red_backgrounds.len(), 1);
            assert_eq!(
                red_backgrounds[0]
                    .attribute("fill-opacity")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                128.0 / 255.0
            );
            let run = document
                .public
                .commands
                .iter_mut()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text == diagram_title => Some(run),
                    _ => None,
                })
                .unwrap();
            run.text = "  Changed <title>  ".to_owned();
            run.origin = merman_display_list::Point::new(37.0, 19.0);
            run.style.font_size = 23.0;
            run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 255));
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let title = xml
                .root_element()
                .children()
                .find(|node| node.attribute("class") == Some("titleText"))
                .unwrap();
            assert_eq!(title.text(), Some("  Changed <title>  "));
            assert_eq!(title.attribute("x"), Some("37"));
            assert_eq!(title.attribute("y"), Some("19"));
            assert!(!title.attribute("style").unwrap().contains("font-size:"));
            assert!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .collect::<String>()
                    .contains("#schedule .titleText{font-size:23px;}")
            );
            assert!(title.attribute("style").unwrap().contains("fill:#123456;"));
            assert_eq!(
                title.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                Some("preserve")
            );
            let run = document
                .public
                .commands
                .iter_mut()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text == "  Changed <title>  " => {
                        Some(run)
                    }
                    _ => None,
                })
                .unwrap();
            run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            let title = xml
                .root_element()
                .children()
                .find(|node| node.attribute("class") == Some("titleText"))
                .unwrap();
            assert_eq!(title.text(), Some("  Changed <title>  "));
            assert_eq!(
                title.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                Some("preserve")
            );
            assert_eq!(title.attribute("fill"), Some("#123456"));
            assert_eq!(title.attribute("font-size"), Some("23"));
            assert!(
                !xml.descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .any(|css| css.contains(".titleText{"))
            );
            assert_eq!(
                title
                    .attribute("fill-opacity")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                128.0 / 255.0
            );
            // A second public title run must not inherit a class rule extracted from the first.
            let title_index = document
                .public
                .commands
                .iter()
                .position(|command| {
                    matches!(command,
                        DrawingCommand::DrawText { run } if run.text == "  Changed <title>  "
                    )
                })
                .unwrap();
            let DrawingCommand::DrawText { run } = &mut document.public.commands[title_index]
            else {
                unreachable!()
            };
            run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 255));
            let mut extra = document.public.commands[title_index].clone();
            let DrawingCommand::DrawText { run } = &mut extra else {
                unreachable!()
            };
            run.text = "Second title run".to_owned();
            run.style.font_size = 31.0;
            document.public.commands.insert(title_index + 1, extra);
            let edited = render(&document);
            let xml = roxmltree::Document::parse(&edited).unwrap();
            assert!(
                !xml.descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .any(|css| css.contains(".titleText{"))
            );
            for (text, size) in [("  Changed <title>  ", "23"), ("Second title run", "31")] {
                let node = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(text))
                    .unwrap();
                assert_eq!(node.attribute("font-size"), None);
                assert!(
                    node.attribute("style")
                        .unwrap()
                        .contains(&format!("font-size:{size}px;"))
                );
            }
        }
    }

    #[test]
    fn gantt_axes_project_source_dom_and_public_paint_edits() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, Point};
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "gantt": { "topAxis": true },
                "themeVariables": { "textColor": "#123456", "fontFamily": "Courier" }
            })))
            .parse_diagram_for_render_model_sync(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Core\nTask :a, 2026-01-01, 2d\n",
                ParseOptions::strict(),
            ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let axes = xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2);
        for (index, axis) in axes.iter().enumerate() {
            assert_eq!(axis.attribute("fill"), Some("none"));
            assert_eq!(axis.attribute("font-family"), Some("sans-serif"));
            assert_eq!(axis.attribute("font-size"), Some("10"));
            assert_eq!(axis.attribute("text-anchor"), Some("middle"));
            let domain = axis
                .children()
                .find(|node| node.attribute("class") == Some("domain"))
                .unwrap();
            assert_eq!(domain.attribute("stroke"), Some("currentColor"));
            assert_eq!(
                domain.attribute("style"),
                Some("color:#000000;fill:none;stroke-width:0;")
            );
            assert!(domain.attribute("data-merman-resource").is_none());
            assert!(domain.attribute("d").is_some());
            assert_eq!(domain.attribute("class"), Some("domain"));
            let tick = axis
                .children()
                .find(|node| node.attribute("class") == Some("tick"))
                .unwrap();
            let line = tick
                .children()
                .find(|node| node.has_tag_name("line"))
                .unwrap();
            assert_eq!(line.attribute("stroke"), Some("currentColor"));
            assert_eq!(
                line.attribute("style"),
                Some("color:#000000;fill:none;stroke-width:1;")
            );
            assert!(line.attribute("data-merman-resource").is_none());
            assert!(line.attribute("x1").is_none());
            let text = tick
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert_eq!(
                text.attribute("dy"),
                Some(if index == 0 { "1em" } else { "0em" })
            );
            assert_eq!(
                text.attribute("y"),
                Some(if index == 0 { "3" } else { "-3" })
            );
            assert_eq!(text.attribute("stroke"), Some("none"));
            assert_eq!(text.attribute("fill"), Some("#000000"));
            assert!(text.attribute("font-family").is_none());
            assert!(
                text.attribute("style")
                    .unwrap()
                    .contains("font-family:Courier;")
            );
            assert!(text.attribute("data-merman-bounds").is_none());
        }
        // A compact source-shaped element must still consume edited public paint and geometry.
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "gantt.axis.bottom.domain"
            {
                style.stroke.as_mut().unwrap().paint = Paint::solid(Color::rgba(18, 52, 86, 255));
            }
        }
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == "gantt.axis.bottom.domain"
            {
                let merman_display_list::PathSegment::MoveTo { to } = &mut path.segments[0] else {
                    panic!("domain start")
                };
                to.y = -17.0;
            }
        }
        let edited_axis = render(&document);
        let edited_xml = roxmltree::Document::parse(&edited_axis).unwrap();
        let domain = edited_xml
            .descendants()
            .find(|node| node.attribute("class") == Some("domain"))
            .unwrap();
        assert_eq!(domain.attribute("stroke"), Some("currentColor"));
        assert_eq!(
            domain.attribute("style"),
            Some("color:#123456;fill:none;stroke-width:0;")
        );
        assert!(domain.attribute("d").unwrap().starts_with("M0.5,-17V"));
        let domain_index = document.public.commands.iter().position(|command| matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "gantt.axis.bottom.domain"
        )).unwrap();
        document
            .public
            .commands
            .insert(domain_index, DrawingCommand::Save);
        document.public.commands.insert(
            domain_index + 1,
            DrawingCommand::SetBlendMode {
                blend_mode: merman_display_list::BlendMode::Multiply,
            },
        );
        document
            .public
            .commands
            .insert(domain_index + 3, DrawingCommand::Restore);
        let blended = render(&document);
        let blended_xml =
            roxmltree::Document::parse(&blended).expect("blended axis must have valid attributes");
        let blended_domain = blended_xml
            .descendants()
            .find(|node| node.attribute("class") == Some("domain"))
            .unwrap();
        assert_eq!(blended_domain.attribute("stroke"), Some("#123456"));
        assert_eq!(
            blended_domain.attribute("style"),
            Some("mix-blend-mode: multiply;")
        );
        let tick = document.public.commands.iter().position(|command| matches!(command,
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "gantt.axis.bottom.tick.0"
        )).unwrap();
        let DrawingCommand::DrawPath { path, style } = &mut document.public.commands[tick + 2]
        else {
            panic!("tick line")
        };
        let path_id = path.clone();
        let stroke = style.stroke.as_mut().unwrap();
        stroke.paint = Paint::solid(Color::rgba(18, 52, 86, 128));
        stroke.width = 2.25;
        stroke.dash_array = vec![2.0, 3.0];
        stroke.line_cap = merman_display_list::LineCap::Round;
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id == path_id
            {
                path.segments = vec![
                    merman_display_list::PathSegment::MoveTo {
                        to: Point::new(3.0, 5.0),
                    },
                    merman_display_list::PathSegment::LineTo {
                        to: Point::new(7.0, 11.0),
                    },
                ];
            }
        }
        let DrawingCommand::DrawText { run } = &mut document.public.commands[tick + 3] else {
            panic!("tick text")
        };
        run.origin = Point::new(4.0, 43.0);
        run.style.font_size = 20.0;
        run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
        run.anchor = merman_display_list::TextAnchor::End;
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let tick = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("tick"))
            .unwrap();
        let line = tick
            .children()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        for (key, value) in [("x1", "3"), ("y1", "5"), ("x2", "7"), ("y2", "11")] {
            assert_eq!(line.attribute(key), Some(value));
        }
        assert_eq!(line.attribute("stroke"), Some("#123456"));
        assert_eq!(
            line.attribute("stroke-opacity")
                .map(|value| value.parse::<f64>().unwrap()),
            Some(128.0 / 255.0)
        );
        assert_eq!(line.attribute("stroke-width"), Some("2.25"));
        assert_eq!(line.attribute("stroke-linecap"), Some("round"));
        assert_eq!(line.attribute("stroke-dasharray"), Some("2,3"));
        let text = tick
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("x"), Some("4"));
        assert_eq!(text.attribute("y"), Some("43"));
        assert_eq!(text.attribute("font-size"), Some("20"));
        assert_eq!(text.attribute("text-anchor"), Some("end"));
        assert_eq!(text.attribute("fill"), Some("#123456"));
        assert_eq!(
            text.attribute("fill-opacity")
                .map(|value| value.parse::<f64>().unwrap()),
            Some(128.0 / 255.0)
        );
    }

    #[test]
    fn gantt_task_names_follow_public_text_and_survive_edited_label_scopes() {
        use merman_display_list::{DrawingCommand, SemanticRole};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nAlpha     #quot;Beta#quot; :a, 2026-01-01, 1d\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let baseline = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let name = "Alpha \"Beta\"";
        let svg = render(&baseline);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for (id, tag) in [("gantt.task.0", "rect"), ("gantt.task.0.label", "text")] {
            let semantic = baseline
                .public
                .semantics
                .iter()
                .find(|entry| entry.id == id)
                .unwrap();
            assert_eq!(semantic.title.as_deref(), Some(name));
            assert_eq!(semantic.description, None);
            let element = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some(id))
                .unwrap();
            assert!(element.has_tag_name(tag));
            assert_eq!(element.attribute("role"), None);
            assert_eq!(element.attribute("aria-label"), None);
            if tag == "text" {
                assert_eq!(element.text(), Some(name));
            }
        }
        for id in ["gantt.task.0", "gantt.task.0.label"] {
            let mut document = baseline.clone();
            document
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .title = Some("Independent &amp; <br> name".into());
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let element = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some(id))
                .unwrap();
            assert_eq!(
                element.attribute("aria-label"),
                Some("Independent &amp; <br> name")
            );
            assert_eq!(element.attribute("role"), Some("img"));
            assert_eq!(
                xml.descendants()
                    .filter(
                        |node| node.attribute("aria-label") == Some("Independent &amp; <br> name")
                    )
                    .count(),
                1
            );
        }
        for id in ["gantt.task.0", "gantt.task.0.label"] {
            let mut document = baseline.clone();
            document
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .description = Some("Delivery section".into());
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let element = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some(id))
                .unwrap();
            assert_eq!(
                element.attribute("aria-description"),
                Some("Delivery section")
            );
        }
        let mut linked = baseline.clone();
        for id in ["gantt.task.0", "gantt.task.0.label"] {
            linked
                .public
                .semantics
                .iter_mut()
                .find(|entry| entry.id == id)
                .unwrap()
                .link = Some("https://example.com/task".into());
        }
        let svg = render(&linked);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let anchors: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("a") && node.attribute("href") == Some("https://example.com/task")
            })
            .collect();
        assert_eq!(anchors.len(), 2);
        let bar = anchors
            .iter()
            .flat_map(|anchor| anchor.children())
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(bar.attribute("aria-label"), Some(name));
        assert_eq!(bar.attribute("role"), Some("img"));
        let label = anchors
            .iter()
            .flat_map(|anchor| anchor.children())
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.text(), Some(name));
        assert_eq!(label.attribute("role"), None);
        let label_start = baseline.public.commands.iter().position(|command| matches!(command,
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "gantt.task.0.label"
        )).unwrap();
        for edit in ["remove", "multiple-runs", "duplicate-scope", "role"] {
            let mut document = baseline.clone();
            match edit {
                "remove" => {
                    document.public.commands.drain(label_start..label_start + 3);
                }
                "multiple-runs" => {
                    let text = document.public.commands[label_start + 1].clone();
                    document.public.commands.insert(label_start + 2, text);
                }
                "duplicate-scope" => {
                    let duplicate = document.public.commands[label_start..label_start + 3].to_vec();
                    document
                        .public
                        .commands
                        .splice(label_start..label_start, duplicate);
                }
                "role" => {
                    document
                        .public
                        .semantics
                        .iter_mut()
                        .find(|entry| entry.id == "gantt.task.0.label")
                        .unwrap()
                        .role = SemanticRole::Node;
                }
                _ => unreachable!(),
            }
            let svg = render(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let task = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some("gantt.task.0"))
                .unwrap();
            assert!(task.has_tag_name("g"), "{edit}");
            assert!(
                task.children()
                    .any(|node| node.has_tag_name("title") && node.text() == Some(name)),
                "{edit}"
            );
        }
    }

    #[test]
    fn gantt_static_navigation_preserves_both_hit_targets_and_public_link_edits() {
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(
                json!({"securityLevel": "loose"}),
            ))
            .parse_diagram_for_render_model_sync(
                // The navigation case from click_multiple_ids_href_loose.mmd; keep crate tests self-contained.
                r#"%%{init: {"securityLevel":"loose"}}%%
gantt
  title Click multiple ids href
  dateFormat YYYY-MM-DD
  section A
  Task1: a1, 2014-01-07, 3d
  Task2: a2, 2014-01-08, 3d

  click a1,a2 href "https://example.com"
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions {
                    diagram_id: Some("navigation".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let scopes: Vec<_> = document
            .public
            .semantics
            .iter()
            .filter(|semantic| semantic.link.is_some())
            .collect();
        assert_eq!(scopes.len(), 4);
        for id in [
            "gantt.task.0",
            "gantt.task.1",
            "gantt.task.0.label",
            "gantt.task.1.label",
        ] {
            assert_eq!(
                scopes
                    .iter()
                    .find(|scope| scope.id == id)
                    .unwrap()
                    .link
                    .as_deref(),
                Some("https://example.com")
            );
        }
        let initial = render(&document);
        let xml = roxmltree::Document::parse(&initial).unwrap();
        let anchors: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("a"))
            .collect();
        assert_eq!(anchors.len(), 4);
        let mut hit_targets = Vec::new();
        for anchor in anchors {
            assert_eq!(anchor.attribute("href"), Some("https://example.com"));
            assert_eq!(
                anchor.attribute("target"),
                None,
                "navigation target remains host-owned"
            );
            let children: Vec<_> = anchor
                .children()
                .filter(roxmltree::Node::is_element)
                .collect();
            assert_eq!(children.len(), 1);
            hit_targets.push(children[0].attribute("id").unwrap());
        }
        assert_eq!(
            hit_targets,
            [
                "navigation-a1",
                "navigation-a2",
                "navigation-a1-text",
                "navigation-a2-text"
            ],
            "independent anchors must preserve all-bars-before-labels painter order"
        );
        assert!(!xml.descendants().any(|node| {
            node.has_tag_name("script")
                || node
                    .attributes()
                    .any(|attribute| attribute.name().starts_with("on"))
        }));

        document
            .public
            .semantics
            .iter_mut()
            .find(|scope| scope.id == "gantt.task.0")
            .unwrap()
            .link = Some("https://example.com/edited?a=1&b=2".into());
        let edited = render(&document);
        assert_eq!(
            edited,
            initial.replacen(
                "href=\"https://example.com\"",
                "href=\"https://example.com/edited?a=1&amp;b=2\"",
                1
            ),
            "one public link edit changes only its anchor, preserving geometry, paint, names and the other hit targets"
        );
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert_eq!(
            xml.descendants()
                .find(|node| node.attribute("id") == Some("navigation-a1"))
                .unwrap()
                .parent()
                .unwrap()
                .attribute("href"),
            Some("https://example.com/edited?a=1&b=2")
        );
    }

    #[test]
    fn gantt_candidate_split_task_scopes_keep_links_and_unique_ids() {
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({"securityLevel": "loose"})))
            .parse_diagram_for_render_model_sync(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Core\nTask :a, 2026-01-01, 1d\nOther :b, 2026-01-03, 1d\nclick a href \"https://example.com/task\"\n",
                ParseOptions::strict(),
            ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        for semantic in &mut document.public.semantics {
            if semantic.id == "gantt.task.0" || semantic.id == "gantt.task.0.label" {
                semantic.title = Some("Public <task> & label".to_owned());
                semantic.description = Some("Public description".to_owned());
            }
        }
        let rendered = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions {
                diagram_id: Some("compact".to_owned()),
                ..Default::default()
            },
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let alternate_theme = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions {
                diagram_id: Some("compact".to_owned()),
                ..Default::default()
            },
            &drawing_list_svg_diagnostics(),
            &json!({
                "securityLevel": "loose",
                "themeCSS": "text { fill: red !important; }",
                "theme": "dark",
                "themeVariables": {
                    "fontFamily": "Courier",
                    "gridColor": "#123456",
                    "taskBkgColor": "#abcdef",
                    "textColor": "#fedcba"
                }
            }),
            &artifact.session,
        )
        .unwrap();
        assert!(
            rendered == alternate_theme,
            "Gantt serialization must not reinterpret theme configuration outside its document"
        );
        let svg = roxmltree::Document::parse(&rendered).unwrap();
        let ticks = svg
            .descendants()
            .filter(|node| {
                node.attribute("class")
                    .is_some_and(|class| class.split_whitespace().any(|token| token == "tick"))
            })
            .collect::<Vec<_>>();
        assert!(!ticks.is_empty());
        for tick in ticks {
            let layer = tick;
            assert_eq!(layer.attribute("class"), Some("tick"));
            assert!(
                layer.attribute("transform").is_some(),
                "tick translation belongs on its source layer"
            );
            let grid = layer.parent().unwrap();
            assert_eq!(grid.attribute("class"), Some("grid"));
            assert!(grid.attribute("transform").is_some());
            assert_eq!(layer.attribute("opacity"), Some("1"));
            assert_eq!(layer.attribute("style"), Some("opacity:0.8;"));
            assert!(layer.children().any(|node| node.has_tag_name("line")));
            assert!(layer.children().any(|node| node.has_tag_name("text")));
            assert!(
                layer
                    .children()
                    .filter(|node| node.is_element())
                    .all(|node| node.attribute("opacity").is_none()
                        && node.attribute("transform").is_none()),
                "opacity and translation must be applied once to the shared layer"
            );
        }
        assert!(
            !svg.descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .any(|css| css.contains("opacity:")),
            "CSS must not reapply public opacity"
        );
        let mut ids = std::collections::BTreeSet::new();
        for node in svg.descendants() {
            if let Some(id) = node.attribute("id") {
                assert!(ids.insert(id), "duplicate SVG id: {id}");
            }
        }
        for id in ["compact-a", "compact-a-text"] {
            let node = svg
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            assert!(node.parent().unwrap().has_tag_name("a"));
            assert_eq!(node.attribute("aria-label"), Some("Public <task> & label"));
            assert_eq!(
                node.attribute("aria-description"),
                Some("Public description")
            );
            assert!(node.attribute("data-merman-semantic-id").is_some());
            assert!(node.ancestors().any(|ancestor| {
                ancestor.has_tag_name("a")
                    && ancestor.attribute("href") == Some("https://example.com/task")
            }));
            assert!(
                node.attribute("class")
                    .is_some_and(|class| class.contains("clickable"))
            );
            if node.has_tag_name("text") {
                assert!(node.attribute("font-family").is_none());
                assert!(node.attribute("data-merman-bounds").is_none());
                assert!(node.attribute("text-anchor").is_none());
                assert!(
                    node.attribute("style")
                        .unwrap()
                        .contains("font-weight:700;")
                );
                assert_eq!(node.attribute("text-height"), Some("20"));
            }
        }
        let other = svg
            .descendants()
            .find(|node| node.attribute("id") == Some("compact-b"))
            .unwrap();
        let tasks = other.parent().unwrap();
        assert!(tasks.has_tag_name("g"));
        assert!(tasks.attribute("data-merman-semantic-id").is_none());
        assert_eq!(
            tasks
                .children()
                .filter(|node| node.is_element())
                .map(|node| node.tag_name().name())
                .collect::<Vec<_>>(),
            ["a", "rect", "a", "text"]
        );
        let task_text = document
            .public
            .commands
            .iter_mut()
            .find_map(|command| match command {
                merman_display_list::DrawingCommand::DrawText { run } if run.text == "Task" => {
                    Some(run)
                }
                _ => None,
            })
            .unwrap();
        task_text.text = "  Edited #; <&>  ".to_owned();
        task_text.origin = merman_display_list::Point::new(123.0, 45.0);
        task_text.style.font_size = 19.0;
        task_text.style.font.weight = 500;
        task_text.style.fill =
            merman_display_list::Paint::solid(merman_display_list::Color::rgba(18, 52, 86, 255));
        let edited = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions {
                diagram_id: Some("compact".to_owned()),
                ..Default::default()
            },
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&edited).unwrap();
        let label = xml
            .descendants()
            .find(|node| node.attribute("id") == Some("compact-a-text"))
            .unwrap();
        assert_eq!(label.text(), Some("  Edited #; <&>  "));
        assert_eq!(
            label.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            Some("preserve")
        );
        assert_eq!(label.attribute("x"), Some("123"));
        assert_eq!(label.attribute("y"), Some("45"));
        assert_eq!(label.attribute("font-size"), Some("19"));
        let style = label.attribute("style").unwrap();
        assert!(style.contains("fill:#123456;"));
        assert!(style.contains("font-weight:500;"));
        assert_eq!(label.attribute("aria-label"), Some("Public <task> & label"));
        assert_eq!(
            label.parent().unwrap().attribute("href"),
            Some("https://example.com/task")
        );
        for command in &mut document.public.commands {
            match command {
                merman_display_list::DrawingCommand::BeginLayer {
                    opacity,
                    blend_mode,
                    ..
                } => {
                    *opacity = 0.35;
                    *blend_mode = merman_display_list::BlendMode::Multiply;
                }
                merman_display_list::DrawingCommand::DrawText { run } => {
                    run.style.fill = merman_display_list::Paint::solid(
                        merman_display_list::Color::rgba(18, 52, 86, 128),
                    );
                    run.style.font_size = 17.0;
                    run.style.font.weight = 500;
                }
                _ => {}
            }
        }
        let first_tick = document.public.commands.iter().position(|command| matches!(command,
            merman_display_list::DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "gantt.axis.bottom.tick.0"
        )).unwrap();
        let merman_display_list::DrawingCommand::ConcatTransform { transform } =
            &mut document.public.commands[first_tick - 1]
        else {
            panic!("tick transform")
        };
        transform.e = 47.0;
        transform.f = 11.0;
        let edited = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert_eq!(
            xml.descendants()
                .find(|node| node.attribute("class") == Some("tick"))
                .unwrap()
                .attribute("transform"),
            Some("translate(47,11)")
        );
        for layer in xml
            .descendants()
            .filter(|node| node.attribute("class") == Some("tick"))
        {
            assert_eq!(layer.attribute("opacity"), Some("1"));
            assert_eq!(
                layer.attribute("style"),
                Some("opacity:0.35;mix-blend-mode:multiply;")
            );
        }
        for text in xml.descendants().filter(|node| node.has_tag_name("text")) {
            assert_eq!(text.attribute("fill"), Some("#123456"));
            assert_eq!(text.attribute("font-size"), Some("17"));
            assert_eq!(text.attribute("font-weight"), Some("500"));
        }
        // An extra valid primitive disables compact projection, but must not disappear.
        let extra_path = document.public.commands[first_tick + 2].clone();
        document.public.commands.insert(first_tick + 4, extra_path);
        let expanded = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&expanded).unwrap();
        let tick = xml
            .descendants()
            .find(|node| {
                node.attribute("data-merman-semantic-id") == Some("gantt.axis.bottom.tick.0")
            })
            .unwrap();
        let layer = tick
            .children()
            .find(|node| node.attribute("class") == Some("merman-layer"))
            .unwrap();
        assert_eq!(
            layer
                .children()
                .filter(|node| node.has_tag_name("line"))
                .count(),
            2
        );
        assert_eq!(
            layer
                .children()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            1
        );
        assert_eq!(layer.attribute("opacity"), Some("0.35"));
        assert!(
            layer
                .children()
                .filter(|node| node.is_element())
                .all(|node| { node.attribute("transform") == Some("matrix(1 0 0 1 47 11)") }),
            "the generic path retains the unprojected transform on each primitive"
        );
        assert!(
            xml.descendants()
                .any(|node| node.attribute("class") == Some("tick")),
            "other ticks keep their valid projection"
        );
        // New commands inside a task scope retain the full semantic wrapper and their effect.
        use merman_display_list::DrawingCommand;
        let bar = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
                    DrawingCommand::DrawPath { path, .. } if path.as_str() == "gantt.task.1.bar"
                )
            })
            .unwrap();
        document
            .public
            .semantics
            .iter_mut()
            .find(|entry| entry.id == "gantt.task.1")
            .unwrap()
            .description = Some("Explicit task description".into());
        document.public.commands.insert(bar, DrawingCommand::Save);
        document
            .public
            .commands
            .insert(bar + 1, DrawingCommand::SetOpacity { opacity: 0.4 });
        document
            .public
            .commands
            .insert(bar + 3, DrawingCommand::Restore);
        let expanded = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&expanded).unwrap();
        let bar = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some("gantt.task.1.bar"))
            .unwrap();
        assert_eq!(bar.attribute("opacity"), None);
        assert!(bar.attribute("style").unwrap().contains("opacity:0.4;"));
        let group = bar.parent().unwrap();
        assert!(group.has_tag_name("g"));
        assert_eq!(
            group.attribute("data-merman-semantic-id"),
            Some("gantt.task.1")
        );
        assert!(group.children().any(|node| node.has_tag_name("title")));
        assert!(group.children().any(|node| node.has_tag_name("desc")));
        for title in [None, Some("   ".to_owned())] {
            document
                .public
                .semantics
                .iter_mut()
                .find(|semantic| semantic.id == "gantt.task.0.label")
                .unwrap()
                .title = title;
            let hidden = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions {
                    include_nodes: false,
                    ..drawing_list_svg_diagnostics()
                },
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&hidden).unwrap();
            let bar = xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some("gantt.task.0"))
                .unwrap();
            assert_eq!(bar.attribute("display"), Some("none"));
            let label = xml
                .descendants()
                .find(|node| {
                    node.attribute("data-merman-semantic-id") == Some("gantt.task.0.label")
                })
                .unwrap();
            assert!(
                label.has_tag_name("g"),
                "unnamed labels retain their readable text, not an unnamed image role"
            );
            assert!(
                label
                    .children()
                    .any(|node| node.has_tag_name("text") && node.attribute("role").is_none())
            );
            assert!(label.ancestors().any(|node| node.has_tag_name("a")));
        }
    }

    #[test]
    fn sankey_label_svg_preserves_public_baselines_and_source_text_shells() {
        use merman_display_list::DrawingCommand;
        for (show_values, dy) in [(true, 0.0), (false, 0.35)] {
            let source = format!(
                "---\nconfig:\n  sankey:\n    showValues: {show_values}\n---\nsankey\nA,B,10\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            for edited in [false, true] {
                if edited {
                    for command in &mut document.public.commands {
                        if let DrawingCommand::DrawText { run } = command {
                            run.origin.y += 17.0;
                            run.style.font_size = 20.0;
                        }
                    }
                }
                let svg = crate::svg::render_document_svg(
                    &document,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    artifact.metadata.effective_config.as_value(),
                    &artifact.session,
                )
                .unwrap();
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let texts = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .collect::<Vec<_>>();
                let runs = document
                    .public
                    .commands
                    .iter()
                    .filter_map(|command| match command {
                        DrawingCommand::DrawText { run } => Some(run),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(texts.len(), runs.len());
                for (text, run) in texts.iter().zip(runs) {
                    assert!(text.attribute("data-merman-bounds").is_none());
                    assert_eq!(
                        text.attribute("dy"),
                        Some(if show_values { "0em" } else { "0.35em" })
                    );
                    let y: f64 = text.attribute("y").unwrap().parse().unwrap();
                    assert!((y + dy * run.style.font_size - run.origin.y).abs() < 0.001);
                    assert_eq!(text.text(), Some(run.text.as_str()));
                    let group = text.parent().unwrap();
                    assert_eq!(group.attribute("class"), Some("node-labels"));
                    assert_eq!(
                        group
                            .attribute("font-size")
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        run.style.font_size
                    );
                }
            }
            // A single edited label must not inherit a shared font size from its peers.
            let first = document
                .public
                .commands
                .iter_mut()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } => Some(run),
                    _ => None,
                })
                .unwrap();
            first.style.font_size = 23.0;
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let sizes = xml
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.attribute("font-size").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(sizes, ["23", "20"]);
        }
    }

    #[test]
    fn sankey_root_background_is_owned_by_the_public_paint_command() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("sankey-beta\nA,B,10\n", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let DrawingCommand::DrawPath { path, style } = &document.public.commands[2] else {
            panic!("Sankey must begin with an explicit background paint");
        };
        assert_eq!(path.as_str(), "sankey.background");
        assert_eq!(
            style.fill,
            Some(Paint::solid(Color::rgba(255, 255, 255, 255)))
        );
        let background = document
            .public
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "sankey.background" => {
                    Some(path)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(background.segments.len(), 5);
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        assert!(svg.contains("background-color: white;"));
        let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[2] else {
            unreachable!()
        };
        style.fill = Some(Paint::solid(Color::rgba(12, 34, 56, 128)));
        let svg = render(&document);
        assert!(!svg.contains("background-color: white;"));
        assert!(svg.contains("#0c2238"));
        document.public.commands.remove(2);
        let svg = render(&document);
        assert!(!svg.contains("background-color: white;") && !svg.contains("#0c2238"));
    }

    #[test]
    fn sankey_svg_places_referenced_public_gradients_inside_links() {
        use merman_display_list::{Color, DrawingResource};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "sankey-beta\nA,B,10\nB,C,5\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |doc: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                doc,
                &SvgRenderOptions {
                    diagram_id: Some("sankey-gradients".to_string()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let gradients = xml
            .descendants()
            .filter(|node| node.has_tag_name("linearGradient"))
            .collect::<Vec<_>>();
        assert_eq!(gradients.len(), 2);
        for (index, gradient) in gradients.iter().enumerate() {
            assert_eq!(gradient.parent().unwrap().attribute("class"), Some("link"));
            let id = format!("sankey-gradients-linearGradient-{}", index + 4);
            assert_eq!(gradient.attribute("id"), Some(id.as_str()));
            assert_eq!(gradient.attribute("gradientUnits"), Some("userSpaceOnUse"));
            for attribute in ["y1", "y2", "gradientTransform", "spreadMethod"] {
                assert!(
                    gradient.attribute(attribute).is_none(),
                    "default {attribute} must be omitted"
                );
            }
            let offsets = gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| node.attribute("offset").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(offsets, ["0%", "100%"]);
            let path = gradient.next_sibling_element().unwrap();
            assert!(path.has_tag_name("path"));
            assert_eq!(
                path.attribute("stroke"),
                Some(format!("url(#{id})").as_str())
            );
        }
        assert!(
            !xml.root_element()
                .children()
                .any(|node| node.has_tag_name("defs"))
        );
        for resource in &mut document.public.resources {
            if let DrawingResource::LinearGradient(gradient) = resource {
                gradient.stops[0].color = Color::rgba(12, 34, 56, 128);
                gradient.stops[0].offset = 0.25;
                gradient.transform.e = 7.0;
                gradient.start.y = 12.0;
                gradient.end.y = 34.0;
                gradient.spread = merman_display_list::GradientSpread::Reflect;
            }
        }
        let svg = render(&document);
        assert!(svg.contains("#0c2238"));
        assert!(svg.contains("gradientTransform=\"matrix(1 0 0 1 7 0)\""));
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for gradient in xml
            .descendants()
            .filter(|node| node.has_tag_name("linearGradient"))
        {
            assert_eq!(gradient.attribute("y1"), Some("12"));
            assert_eq!(gradient.attribute("y2"), Some("34"));
            assert_eq!(gradient.attribute("spreadMethod"), Some("reflect"));
            let stop = gradient.first_element_child().unwrap();
            assert_eq!(stop.attribute("offset"), Some("25%"));
            let alpha: f64 = stop.attribute("stop-opacity").unwrap().parse().unwrap();
            assert!((alpha - 128.0 / 255.0).abs() < 0.001);
        }
        // A retained resource is not proof of a paint operation. Removing the command must
        // remove its inline definition; it may remain an inert general resource definition.
        document.public.commands.retain(|command| {
            !matches!(command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "sankey.link.0.path"
            )
        });
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("linearGradient")
                    && node.parent().unwrap().attribute("class") == Some("link"))
                .count(),
            1
        );
    }

    #[test]
    fn sankey_node_svg_projects_public_local_geometry_and_transform() {
        use merman_display_list::{DrawingCommand, DrawingResource, PathSegment, Point};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "sankey-beta\nA,B,10\nB,C,5\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let node_id = "sankey.node.1.shape";
        let path = document
            .public
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == node_id => Some(path),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            path.segments.first(),
            Some(&PathSegment::MoveTo {
                to: Point::new(0.0, 0.0)
            })
        );
        let render = |doc: &crate::drawing_list::RenderDocument, diagram_id: Option<&str>| {
            crate::svg::render_document_svg(
                doc,
                &SvgRenderOptions {
                    diagram_id: diagram_id.map(str::to_owned),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        for prefix in [None, Some("sankey-node-test")] {
            let svg = render(&document, prefix);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let id =
                prefix.map_or_else(|| "node-2".to_string(), |prefix| format!("{prefix}-node-2"));
            let node = xml
                .descendants()
                .find(|node| node.attribute("id") == Some(id.as_str()))
                .unwrap();
            assert_eq!(node.attribute("class"), Some("node"));
            assert!(
                node.attribute("transform")
                    .unwrap()
                    .starts_with("translate(")
            );
            let rect = node
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert!(rect.attribute("x").is_none() && rect.attribute("y").is_none());
            assert!(rect.attribute("class").is_none());
            assert!(rect.attribute("shape-rendering").is_none());
            assert!(rect.attribute("stroke").is_none());
            assert!(svg.contains(".node rect{shape-rendering:crispEdges;}"));
        }
        let index = document.public.commands.iter().position(|command| matches!(command,
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "sankey.node.1"
        )).unwrap();
        let DrawingCommand::ConcatTransform { transform } =
            &mut document.public.commands[index - 1]
        else {
            panic!("node position belongs to the public transform");
        };
        transform.e = 42.0;
        transform.f = 19.0;
        let svg = render(&document, None);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = xml
            .descendants()
            .find(|node| node.attribute("id") == Some("node-2"))
            .unwrap();
        assert_eq!(node.attribute("transform"), Some("translate(42,19)"));
        assert_eq!(node.attribute("x"), Some("42"));
        assert_eq!(node.attribute("y"), Some("19"));
    }

    #[test]
    fn sankey_link_svg_projects_only_equivalent_single_strokes() {
        use merman_display_list::{BlendMode, Color, DrawingCommand, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("sankey\nA,B,10\nB,C,5\n", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let original = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        for case in [
            "default",
            "state",
            "alpha",
            "fill",
            "extra",
            "mixed-opacity",
            "stroke",
        ] {
            let mut document = original.clone();
            let mut first = true;
            for command in &mut document.public.commands {
                match command {
                    DrawingCommand::SetOpacity { opacity } if case == "state" => *opacity = 0.25,
                    DrawingCommand::SetOpacity { opacity } if case == "mixed-opacity" && first => {
                        *opacity = 0.25;
                        first = false;
                    }
                    DrawingCommand::SetBlendMode { blend_mode } if case == "state" => {
                        *blend_mode = BlendMode::Screen
                    }
                    DrawingCommand::DrawPath { path, style }
                        if path.as_str().starts_with("sankey.link.") =>
                    {
                        if case == "alpha" {
                            style.stroke.as_mut().unwrap().paint =
                                Paint::solid(Color::rgba(12, 34, 56, 128));
                        } else if case == "fill" {
                            style.fill = Some(Paint::solid(Color::rgba(12, 34, 56, 255)));
                        } else if case == "stroke" {
                            let stroke = style.stroke.as_mut().unwrap();
                            stroke.line_cap = merman_display_list::LineCap::Round;
                            stroke.dash_array = vec![2.0, 3.0];
                        }
                    }
                    _ => {}
                }
            }
            if case == "extra" {
                let index = document.public.commands.iter().position(|command| matches!(command,
                    DrawingCommand::DrawPath { path, .. } if path.as_str() == "sankey.link.0.path"
                )).unwrap();
                document
                    .public
                    .commands
                    .insert(index, document.public.commands[index].clone());
            }
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let links = xml
                .descendants()
                .find(|node| node.attribute("class") == Some("links"))
                .unwrap();
            let groups = links
                .children()
                .filter(|node| node.has_tag_name("g"))
                .collect::<Vec<_>>();
            let paths = links
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), if case == "extra" { 3 } else { 2 });
            if matches!(case, "default" | "state") {
                assert_eq!(links.attribute("fill"), Some("none"));
                assert_eq!(
                    links.attribute("stroke-opacity"),
                    Some(if case == "state" { "0.25" } else { "0.5" })
                );
                for group in groups {
                    assert_eq!(
                        group.attribute("style"),
                        Some(if case == "state" {
                            "mix-blend-mode: screen;"
                        } else {
                            "mix-blend-mode: multiply;"
                        })
                    );
                }
                for path in paths {
                    for attribute in [
                        "opacity",
                        "stroke-opacity",
                        "fill",
                        "class",
                        "data-merman-resource",
                        "style",
                        "stroke-linecap",
                    ] {
                        assert!(
                            path.attribute(attribute).is_none(),
                            "{case}: extra {attribute}"
                        );
                    }
                }
            } else {
                assert!(links.attribute("stroke-opacity").is_none());
                assert!(
                    groups
                        .iter()
                        .all(|group| group.attribute("style").is_none())
                );
                for path in paths {
                    assert!(path.attribute("opacity").is_some());
                    if case == "alpha" {
                        let alpha: f64 = path.attribute("stroke-opacity").unwrap().parse().unwrap();
                        assert!((alpha - 128.0 / 255.0).abs() < 0.001);
                    } else if case == "fill" {
                        assert_eq!(path.attribute("fill"), Some("#0c2238"));
                    } else if case == "stroke" {
                        assert_eq!(path.attribute("stroke-linecap"), Some("round"));
                        assert_eq!(path.attribute("stroke-dasharray"), Some("2,3"));
                    }
                }
            }
        }
    }

    #[test]
    fn journey_mouth_projection_preserves_edited_public_geometry() {
        use merman_display_list::{
            DrawingCommand, DrawingResource, PathResource, PathSegment, Point, Transform,
        };
        fn assert_curve(path: &PathResource, node: roxmltree::Node<'_, '_>, origin: Point) {
            let parsed = svgtypes::PathParser::from(node.attribute("d").unwrap())
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(parsed.len(), path.segments.len());
            for (actual, expected) in parsed.iter().zip(&path.segments) {
                let (x, y, to) = match (actual, expected) {
                    (
                        svgtypes::PathSegment::MoveTo { abs: true, x, y },
                        PathSegment::MoveTo { to },
                    )
                    | (
                        svgtypes::PathSegment::LineTo { abs: true, x, y },
                        PathSegment::LineTo { to },
                    ) => (*x, *y, to),
                    (
                        svgtypes::PathSegment::EllipticalArc {
                            abs: true,
                            rx,
                            ry,
                            x_axis_rotation,
                            large_arc,
                            sweep,
                            x,
                            y,
                        },
                        PathSegment::ArcTo {
                            radius_x,
                            radius_y,
                            x_axis_rotation_degrees,
                            large_arc: expected_large,
                            sweep_clockwise,
                            to,
                        },
                    ) => {
                        assert_eq!(
                            (*rx, *ry, *x_axis_rotation, *large_arc, *sweep),
                            (
                                *radius_x,
                                *radius_y,
                                *x_axis_rotation_degrees,
                                *expected_large,
                                *sweep_clockwise
                            )
                        );
                        (*x, *y, to)
                    }
                    (svgtypes::PathSegment::ClosePath { .. }, PathSegment::Close) => continue,
                    _ => panic!("public path segment was replaced"),
                };
                assert!((x + origin.x - to.x).abs() < 1e-9);
                assert!((y + origin.y - to.y).abs() < 1e-9);
            }
        }
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "journey\nsection Work\nHappy: 5: Alice\nNeutral: 3: Alice\nSad: 1: Alice\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        for edited in [false, true] {
            if edited {
                for resource in &mut document.public.resources {
                    if let DrawingResource::Path(path) = resource
                        && path.id.as_str() == "journey.task.0.face.mouth"
                    {
                        for segment in &mut path.segments {
                            match segment {
                                PathSegment::MoveTo { to }
                                | PathSegment::LineTo { to }
                                | PathSegment::ArcTo { to, .. } => {
                                    to.x += 13.0;
                                    to.y -= 17.0;
                                }
                                _ => {}
                            }
                        }
                        if let PathSegment::ArcTo {
                            radius_x,
                            x_axis_rotation_degrees,
                            large_arc,
                            ..
                        } = &mut path.segments[1]
                        {
                            *radius_x = 9.0;
                            *x_axis_rotation_degrees = 13.0;
                            *large_arc = false;
                        }
                        if let PathSegment::LineTo { to } = &mut path.segments[2] {
                            to.x += 2.0;
                            to.y -= 3.0;
                        }
                    }
                }
            }
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                &json!({}),
                &artifact.session,
            )
            .unwrap();
            let native = crate::svg::SvgPipeline::resvg_safe()
                .process_resvg_compatible(&svg, &artifact.session)
                .unwrap();
            for (output, width) in [(svg.as_str(), "1px"), (native.as_str(), "1")] {
                let xml = roxmltree::Document::parse(output).unwrap();
                for index in [0, 2] {
                    let id = format!("journey.task.{index}.face.mouth");
                    let path = document
                        .public
                        .resources
                        .iter()
                        .find_map(|resource| match resource {
                            DrawingResource::Path(path) if path.id.as_str() == id => Some(path),
                            _ => None,
                        })
                        .unwrap();
                    let node = xml
                        .descendants()
                        .find(|node| node.attribute("data-merman-resource") == Some(id.as_str()))
                        .unwrap();
                    assert_eq!(node.attribute("class"), Some("mouth"));
                    let translation = node
                        .attribute("transform")
                        .unwrap()
                        .strip_prefix("translate(")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let (x, y) = translation.split_once(',').unwrap();
                    assert_curve(
                        path,
                        node,
                        Point::new(x.trim().parse().unwrap(), y.trim().parse().unwrap()),
                    );
                }
                let neutral = xml
                    .descendants()
                    .find(|node| {
                        node.attribute("data-merman-resource") == Some("journey.task.1.face.mouth")
                    })
                    .unwrap();
                assert!(neutral.has_tag_name("line"));
                assert_eq!(neutral.attribute("class"), Some("mouth"));
                assert_eq!(neutral.attribute("stroke-width"), Some(width));
                assert_eq!(neutral.attribute("stroke"), Some("#666666"));
            }
        }
        // A user-space gradient must not move with the optional local path basis.
        let mut gradient_document = document.clone();
        gradient_document
            .public
            .resources
            .push(DrawingResource::LinearGradient(
                merman_display_list::LinearGradientResource {
                    id: "mouth-gradient".into(),
                    start: Point::new(0.0, 0.0),
                    end: Point::new(40.0, 0.0),
                    transform: Transform::IDENTITY,
                    spread: merman_display_list::GradientSpread::Pad,
                    stops: vec![
                        merman_display_list::GradientStop::new(
                            0.0,
                            merman_display_list::Color::rgba(255, 0, 0, 255),
                        ),
                        merman_display_list::GradientStop::new(
                            1.0,
                            merman_display_list::Color::rgba(0, 0, 255, 255),
                        ),
                    ],
                },
            ));
        for command in &mut gradient_document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "journey.task.0.face.mouth"
            {
                style.fill = Some(merman_display_list::Paint::resource(
                    "mouth-gradient".into(),
                ));
            }
        }
        let svg = crate::svg::render_document_svg(
            &gradient_document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            &json!({}),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = xml
            .descendants()
            .find(|node| {
                node.attribute("data-merman-resource") == Some("journey.task.0.face.mouth")
            })
            .unwrap();
        assert!(node.attribute("transform").is_none());
        assert!(node.attribute("fill").unwrap().starts_with("url(#"));
        let path = document
            .public
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "journey.task.0.face.mouth" => {
                    Some(path)
                }
                _ => None,
            })
            .unwrap();
        assert_curve(path, node, Point::new(0.0, 0.0));

        let index = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "journey.task.0.face.mouth")
            })
            .unwrap();
        document
            .public
            .commands
            .insert(index + 1, DrawingCommand::Restore);
        document.public.commands.splice(
            index..index,
            [
                DrawingCommand::Save,
                DrawingCommand::ConcatTransform {
                    transform: Transform {
                        a: 2.0,
                        d: 3.0,
                        e: 11.0,
                        f: 13.0,
                        ..Transform::IDENTITY
                    },
                },
            ],
        );
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            &json!({}),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = xml
            .descendants()
            .find(|node| {
                node.attribute("data-merman-resource") == Some("journey.task.0.face.mouth")
            })
            .unwrap();
        assert_eq!(node.attribute("transform"), Some("matrix(2 0 0 3 11 13)"));
        let path = document
            .public
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "journey.task.0.face.mouth" => {
                    Some(path)
                }
                _ => None,
            })
            .unwrap();
        assert_curve(path, node, Point::new(0.0, 0.0));
    }

    #[test]
    fn journey_primitive_styles_preserve_public_paint_and_compositing() {
        use merman_display_list::{
            BlendMode, Color, DrawingCommand, DrawingResource, GradientSpread, GradientStop,
            LineCap, LineJoin, LinearGradientResource, Paint, Point, Transform,
        };
        let parsed = Engine::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "themeVariables": {"textColor":"#123456", "faceColor":"#abcdef"}
            })))
            .parse_diagram_for_render_model_sync(
                "journey\nsection Work\nRead: 5: Alice\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                &json!({"themeVariables":{"textColor":"red", "faceColor":"black"}}),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for (id, attribute, expected) in [
            ("journey.task.0.face", "fill", "#abcdef"),
            ("journey.task.0.line", "stroke", "#123456"),
            ("journey.activity.line", "stroke", "#123456"),
        ] {
            let node = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some(id))
                .unwrap();
            let css = node
                .attribute("style")
                .unwrap()
                .split(';')
                .filter_map(|item| item.split_once(':'))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(css.get(attribute), Some(&expected));
            assert_eq!(css.get("opacity"), Some(&"1"));
            assert!(node.attribute("stroke-linecap").is_none());
            if id.ends_with(".face") {
                assert_eq!(node.attribute("overflow"), Some("visible"));
                assert!(node.attribute("fill").is_none());
            }
        }
        let face_index = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "journey.task.0.face")
            })
            .unwrap();
        let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[face_index]
        else {
            unreachable!()
        };
        style.fill = Some(Paint::resource("edited-gradient".into()));
        let stroke = style.stroke.as_mut().unwrap();
        stroke.paint = Paint::solid(Color::rgba(12, 34, 56, 128));
        stroke.width = 3.0;
        stroke.line_cap = LineCap::Round;
        stroke.line_join = LineJoin::Bevel;
        stroke.miter_limit = 7.0;
        stroke.dash_array = vec![3.0, 5.0];
        stroke.dash_offset = 2.0;
        document
            .public
            .resources
            .push(DrawingResource::LinearGradient(LinearGradientResource {
                id: "edited-gradient".into(),
                start: Point::new(0.0, 0.0),
                end: Point::new(40.0, 0.0),
                transform: Transform::IDENTITY,
                spread: GradientSpread::Pad,
                stops: vec![
                    GradientStop::new(0.0, Color::rgba(255, 0, 0, 128)),
                    GradientStop::new(1.0, Color::rgba(0, 0, 255, 255)),
                ],
            }));
        document
            .public
            .commands
            .insert(face_index + 1, DrawingCommand::Restore);
        document.public.commands.splice(
            face_index..face_index,
            [
                DrawingCommand::Save,
                DrawingCommand::SetOpacity { opacity: 0.5 },
                DrawingCommand::SetBlendMode {
                    blend_mode: BlendMode::Multiply,
                },
                DrawingCommand::ConcatTransform {
                    transform: Transform {
                        e: 11.0,
                        f: 13.0,
                        ..Transform::IDENTITY
                    },
                },
            ],
        );
        let svg = render(&document);
        let native = crate::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(&svg, &artifact.session)
            .unwrap();
        for output in [svg.as_str(), native.as_str()] {
            let xml = roxmltree::Document::parse(output).unwrap();
            let face = xml
                .descendants()
                .find(|node| node.attribute("data-merman-resource") == Some("journey.task.0.face"))
                .unwrap();
            let css = face
                .attribute("style")
                .unwrap()
                .split(';')
                .filter_map(|item| item.split_once(':'))
                .collect::<std::collections::BTreeMap<_, _>>();
            for (key, value) in [
                ("stroke", "#0c2238"),
                ("stroke-width", "3"),
                ("stroke-linecap", "round"),
                ("stroke-linejoin", "bevel"),
                ("stroke-miterlimit", "7"),
                ("stroke-dasharray", "3 5"),
                ("stroke-dashoffset", "2"),
                ("opacity", "0.5"),
                ("mix-blend-mode", "multiply"),
                ("fill-opacity", "1"),
            ] {
                assert_eq!(css.get(key), Some(&value), "public {key}");
            }
            assert_eq!(css["stroke-opacity"].parse::<f64>().unwrap(), 128.0 / 255.0);
            assert!(
                face.attribute("opacity").is_none(),
                "one state opacity declaration"
            );
            assert_eq!(face.attribute("transform"), Some("matrix(1 0 0 1 11 13)"));
            let gradient_id = css["fill"]
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            assert!(
                xml.descendants()
                    .any(|node| node.has_tag_name("linearGradient")
                        && node.attribute("id") == Some(gradient_id))
            );
        }
    }

    #[test]
    fn journey_legend_tspan_and_shared_defs_preserve_public_text() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        for placement in ["fo", "old"] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "journey": {"textPlacement": placement, "boxTextMargin": 13}
                })))
                .parse_diagram_for_render_model_sync(
                    "journey\nsection Work\nRead: 5: Alice\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let run = document
                .public
                .commands
                .iter_mut()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text == "Alice" => Some(run),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                run.origin.x, 66.0,
                "source tspan x is 40 + 2 * boxTextMargin"
            );
            run.text = "Edited <legend>".to_owned();
            run.origin.x = 91.0;
            run.origin.y = 123.0;
            run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
            run.style.font_size = 27.0;
            run.language = Some("en".to_owned());
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                &json!({"journey": {"boxTextMargin": 99}}),
                &artifact.session,
            )
            .unwrap();
            let native = crate::svg::SvgPipeline::resvg_safe()
                .process_resvg_compatible(&svg, &artifact.session)
                .unwrap();
            for output in [svg.as_str(), native.as_str()] {
                let xml = roxmltree::Document::parse(output).unwrap();
                let defs = xml
                    .root_element()
                    .children()
                    .filter(|node| node.has_tag_name("defs"))
                    .collect::<Vec<_>>();
                assert_eq!(
                    defs.len(),
                    1,
                    "{placement}: marker and clip share the source defs"
                );
                assert!(
                    defs[0]
                        .children()
                        .find(|node| node.is_element())
                        .unwrap()
                        .has_tag_name("marker")
                );
                assert_eq!(
                    defs[0]
                        .children()
                        .filter(|node| node.has_tag_name("clipPath"))
                        .count(),
                    if placement == "fo" { 2 } else { 0 }
                );
                let spans = xml
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("tspan") && node.text() == Some("Edited <legend>")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(spans.len(), 1);
                let span = spans[0];
                let text = span.parent().unwrap();
                assert!(text.has_tag_name("text"));
                assert_eq!(text.attribute("class"), Some("legend"));
                assert_eq!(text.attribute("x"), Some("40"));
                assert_eq!(span.attribute("x"), Some("91"));
                assert_eq!(text.attribute("y"), Some("123"));
                assert_eq!(text.attribute("fill"), Some("#123456"));
                assert_eq!(
                    text.attribute("fill-opacity")
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    128.0 / 255.0
                );
                assert_eq!(text.attribute("font-size"), Some("27"));
                assert_eq!(
                    text.attribute(("http://www.w3.org/XML/1998/namespace", "lang")),
                    Some("en")
                );
                assert!(
                    text.ancestors().any(|node| node
                        .children()
                        .any(|child| child.has_tag_name("title") && child.text() == Some("Alice"))),
                    "the independent public name remains available"
                );
            }
        }
    }

    #[test]
    fn journey_html_shell_and_native_pipeline_preserve_public_text_and_clip() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, PathSegment};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "journey\nsection First\nVisible: 5: Alice\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let run = document
            .public
            .commands
            .iter_mut()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text == "Visible" => Some(run),
                _ => None,
            })
            .unwrap();
        run.text = "Edited <literal>".to_owned();
        run.origin.x += 11.0;
        run.origin.y += 17.0;
        run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 255));
        run.style.font_size = 27.0;
        let origin = run.origin;
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == "journey.task.0.label.clip"
            {
                path.segments = vec![
                    PathSegment::MoveTo {
                        to: merman_display_list::Point::new(31.0, 42.0),
                    },
                    PathSegment::LineTo {
                        to: merman_display_list::Point::new(131.0, 42.0),
                    },
                    PathSegment::LineTo {
                        to: merman_display_list::Point::new(131.0, 102.0),
                    },
                    PathSegment::LineTo {
                        to: merman_display_list::Point::new(31.0, 102.0),
                    },
                    PathSegment::Close,
                ];
            }
        }
        for remove_clip in [false, true] {
            if remove_clip {
                document.public.commands.retain(|command| !matches!(command,
                    DrawingCommand::ClipPath { path, .. } if path.as_str() == "journey.task.0.label.clip"));
            }
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let readable = crate::svg::SvgPipeline::readable()
                .process_to_string(&svg, &artifact.session)
                .unwrap();
            let native = crate::svg::SvgPipeline::resvg_safe()
                .process_resvg_compatible(&svg, &artifact.session)
                .unwrap();
            for (mode, output) in [
                ("svg", svg.as_str()),
                ("readable", readable.as_str()),
                ("native", native.as_str()),
            ] {
                let xml = roxmltree::Document::parse(output).unwrap();
                let texts = xml
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("text") && node.text() == Some("Edited <literal>")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(texts.len(), 1, "{mode}: one resolved text projection");
                let text = texts[0];
                assert!(
                    text.ancestors()
                        .any(|node| node
                            .children()
                            .any(|child| child.has_tag_name("title")
                                && child.text() == Some("Visible"))),
                    "{mode}: text edits must preserve the independently named semantic scope"
                );
                assert!(
                    (text.attribute("x").unwrap().parse::<f64>().unwrap() - origin.x).abs() < 0.001
                );
                assert!(
                    (text.attribute("y").unwrap().parse::<f64>().unwrap() - origin.y).abs() < 0.001
                );
                assert_eq!(text.attribute("fill"), Some("#123456"));
                assert_eq!(text.attribute("font-size"), Some("27"));
                let clip = text
                    .ancestors()
                    .find_map(|node| node.attribute("clip-path"));
                if remove_clip {
                    assert!(
                        clip.is_none(),
                        "{mode}: deleting the public clip removes clipping"
                    );
                } else {
                    let clip_id = clip
                        .unwrap()
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let clip = xml
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("clipPath") && node.attribute("id") == Some(clip_id)
                        })
                        .unwrap();
                    assert_eq!(clip.attribute("clipPathUnits"), Some("userSpaceOnUse"));
                    let path = clip
                        .children()
                        .find(|node| node.has_tag_name("path"))
                        .unwrap();
                    assert_eq!(
                        path.attribute("d"),
                        Some("M 31 42 L 131 42 L 131 102 L 31 102 Z")
                    );
                }
                let fo = text
                    .ancestors()
                    .find(|node| node.has_tag_name("foreignObject"));
                if mode == "native" || remove_clip {
                    assert!(fo.is_none(), "{mode}: no independent HTML text fallback");
                    assert!(
                        !text
                            .ancestors()
                            .any(|node| node.attribute("transform") == Some("translate(-31, -42)"))
                    );
                } else {
                    let fo =
                        fo.expect("public clipped text retains the source HTML identity shell");
                    assert_eq!(fo.attribute("x"), Some("31"));
                    assert_eq!(fo.attribute("y"), Some("42"));
                    assert_eq!(fo.attribute("width"), Some("100"));
                    assert_eq!(fo.attribute("height"), Some("60"));
                    assert_eq!(fo.attribute("overflow"), Some("visible"));
                    assert!(
                        fo.descendants()
                            .any(|node| node.attribute("class") == Some("label"))
                    );
                }
            }
        }
    }

    #[test]
    fn journey_source_groups_preserve_order_and_independent_public_metadata() {
        use merman_display_list::{DrawingCommand, SemanticRole};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "journey\nsection First\nOne: 5: Alice\nsection Second\nTwo: 3: Bob\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let original = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let order = original
            .public
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawingCommand::BeginSemanticGroup { semantic_id }
                    if (semantic_id.starts_with("journey.section.")
                        || semantic_id.starts_with("journey.task."))
                        && semantic_id.split('.').count() == 3 =>
                {
                    Some(semantic_id.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            [
                "journey.section.0",
                "journey.task.0",
                "journey.section.1",
                "journey.task.1"
            ]
        );
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&original);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let root = xml.root_element();
        let groups = root
            .children()
            .filter(|node| node.has_tag_name("g"))
            .collect::<Vec<_>>();
        assert_eq!(
            groups.len(),
            5,
            "the source has an empty root group and section/task groups"
        );
        assert!(groups.iter().all(|group| group.attribute("id").is_none()));
        assert_eq!(
            root.children()
                .filter(|node| node.has_tag_name("circle"))
                .count(),
            2
        );
        assert!(groups[0].children().next().is_none());
        for (group, expected) in groups.iter().skip(1).zip([
            "journey-section section-type-0",
            "task task-type-0",
            "journey-section section-type-1",
            "task task-type-1",
        ]) {
            assert!(
                group
                    .children()
                    .any(|node| node.has_tag_name("rect")
                        && node.attribute("class") == Some(expected))
            );
        }
        for (task, actor) in [(groups[2], "Alice"), (groups[4], "Bob")] {
            let expression = task.children().find(|node| node.has_tag_name("g")).unwrap();
            assert_eq!(
                expression
                    .children()
                    .filter(|node| node.has_tag_name("circle"))
                    .count(),
                2
            );
            assert_eq!(
                expression
                    .children()
                    .filter(|node| node.has_tag_name("path") || node.has_tag_name("line"))
                    .count(),
                1
            );
            assert!(task.children().any(|node| {
                node.has_tag_name("circle")
                    && node
                        .children()
                        .any(|title| title.has_tag_name("title") && title.text() == Some(actor))
            }));
        }
        for id in [
            "journey.document",
            "journey.actor.0",
            "journey.section.0",
            "journey.task.0",
            "journey.activity",
            "journey.task.0.expression",
            "journey.task.0.actor.0",
        ] {
            for field in ["title", "description", "link", "role"] {
                let mut document = original.clone();
                let semantic = document
                    .public
                    .semantics
                    .iter_mut()
                    .find(|item| item.id == id)
                    .unwrap();
                match field {
                    "title" => semantic.title = Some("Independent &amp; name".into()),
                    "description" => semantic.description = Some("Independent description".into()),
                    "link" => semantic.link = Some("https://example.com/a?x=1&y=2".into()),
                    "role" => {
                        semantic.role = if semantic.role == SemanticRole::Label {
                            SemanticRole::Node
                        } else {
                            SemanticRole::Label
                        }
                    }
                    _ => unreachable!(),
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                match field {
                    "title" => assert!(
                        xml.descendants().any(|node| node.has_tag_name("title")
                            && node.text() == Some("Independent &amp; name")),
                        "{id}"
                    ),
                    "description" => assert!(
                        xml.descendants().any(|node| node.has_tag_name("desc")
                            && node.text() == Some("Independent description")),
                        "{id}"
                    ),
                    "link" => assert!(
                        xml.descendants()
                            .any(|node| node.attribute("href")
                                == Some("https://example.com/a?x=1&y=2")),
                        "{id}"
                    ),
                    "role" => assert!(
                        xml.descendants()
                            .any(|node| node.has_tag_name("g") && node.attribute("id").is_some()),
                        "{id}"
                    ),
                    _ => unreachable!(),
                }
            }
        }
    }

    #[test]
    fn journey_root_accessibility_preserves_explicit_and_independently_edited_names() {
        for title in ["", "title Alpha     #quot;Beta#quot;\n"] {
            for accessibility in [
                "",
                "accTitle: Authored name\naccDescr: Authored description\n",
            ] {
                let source =
                    format!("journey\n{title}{accessibility}section Delivery\nTask: 5: Alice\n");
                let parsed = Engine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
                let mut document = crate::drawing_list::build_for_family(
                    &artifact.family,
                    &artifact.metadata,
                    DrawingListPolicy::VectorOnly,
                    DrawingListLimits::default(),
                    &artifact.session,
                )
                .unwrap();
                let render = |document: &crate::drawing_list::RenderDocument| {
                    crate::svg::render_document_svg(
                        document,
                        &SvgRenderOptions::default(),
                        &SvgDebugOptions::default(),
                        artifact.metadata.effective_config.as_value(),
                        &artifact.session,
                    )
                    .unwrap()
                };
                let root_name = document
                    .public
                    .semantics
                    .iter()
                    .find(|s| s.id == "journey.document")
                    .unwrap()
                    .title
                    .as_deref();
                assert_eq!(
                    root_name,
                    Some(if !accessibility.is_empty() {
                        "Authored name"
                    } else if title.is_empty() {
                        "journey"
                    } else {
                        "Alpha \"Beta\""
                    })
                );
                if !title.is_empty() {
                    assert!(document.public.commands.iter().any(|command| matches!(command,
                        merman_display_list::DrawingCommand::DrawText { run } if run.text == "Alpha \"Beta\"")));
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let root = xml.root_element();
                assert_eq!(
                    root.attribute("aria-labelledby").is_some(),
                    !accessibility.is_empty()
                );
                assert_eq!(
                    root.attribute("aria-describedby").is_some(),
                    !accessibility.is_empty()
                );
                if !accessibility.is_empty() {
                    assert!(
                        root.children().any(|node| node.has_tag_name("title")
                            && node.text() == Some("Authored name"))
                    );
                    assert!(root.children().any(|node| node.has_tag_name("desc")
                        && node.text() == Some("Authored description")));
                }
                let semantic = document
                    .public
                    .semantics
                    .iter_mut()
                    .find(|s| s.id == "journey.document")
                    .unwrap();
                semantic.title = Some("Independent &amp; <name>".into());
                semantic.description = Some("Independent &amp; <description>".into());
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let root = xml.root_element();
                assert!(root.attribute("aria-labelledby").is_some());
                assert!(root.attribute("aria-describedby").is_some());
                assert!(root.children().any(|node| node.has_tag_name("title")
                    && node.text() == Some("Independent &amp; <name>")));
                assert!(root.children().any(|node| node.has_tag_name("desc")
                    && node.text() == Some("Independent &amp; <description>")));
            }
        }
    }

    #[test]
    fn journey_title_relative_font_size_uses_root_not_task_style() {
        use merman_display_list::DrawingCommand;
        for (root, task, unit, expected) in [
            (24, 14, "4ex", 48.0),
            (24, 40, "2em", 48.0),
            (24, 9, "150%", 36.0),
            (12, 40, "2em", 24.0),
            (24, 9, "2rem", 32.0),
            (48, 40, "2rem", 32.0),
            (24, 40, "18px", 18.0),
        ] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "fontSize": 10, "themeVariables": {"fontSize": format!("{root}px")},
                    "journey": {"taskFontSize": task, "titleFontSize": unit},
                })))
                .parse_diagram_for_render_model_sync(
                    "journey\ntitle Heading\nsection Work\nRead: 5: Alice\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let run = document
                .public
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawingCommand::DrawText { run } if run.text == "Heading" => Some(run),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                run.style.font_size, expected,
                "root={root}, task={task}, unit={unit}"
            );
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                &json!({"fontSize":99}),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let text = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Heading"))
                .unwrap();
            assert_eq!(
                text.attribute("font-size").unwrap().parse::<f64>().unwrap(),
                expected
            );
        }
    }

    #[test]
    fn journey_background_css_retains_public_alpha_and_absent_paints() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "journey\nsection Work\nRead: 5: Alice\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command {
                match path.as_str() {
                    "journey.section.0.background" => {
                        style.fill = Some(Paint::solid(Color::rgba(18, 52, 86, 128)));
                        style.stroke = None;
                    }
                    "journey.task.0.background" => {
                        style.fill = None;
                        let stroke = style.stroke.as_mut().unwrap();
                        stroke.paint = Paint::solid(Color::rgba(12, 34, 56, 64));
                        stroke.width = 3.0;
                    }
                    _ => {}
                }
            }
        }
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            &json!({"themeVariables":{"fillType0":"red"}}),
            &artifact.session,
        )
        .unwrap();
        let native = crate::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(&svg, &artifact.session)
            .unwrap();
        for output in [svg.as_str(), native.as_str()] {
            let xml = roxmltree::Document::parse(output).unwrap();
            for (id, fill, stroke, fill_alpha, stroke_alpha) in [
                (
                    "journey.section.0.background",
                    "#123456",
                    "none",
                    128.0 / 255.0,
                    1.0,
                ),
                (
                    "journey.task.0.background",
                    "none",
                    "#0c2238",
                    1.0,
                    64.0 / 255.0,
                ),
            ] {
                let node = xml
                    .descendants()
                    .find(|node| node.attribute("data-merman-resource") == Some(id))
                    .unwrap();
                assert!(node.has_tag_name("rect"));
                assert_eq!(node.attribute("rx"), Some("3"));
                assert_eq!(node.attribute("ry"), Some("3"));
                assert!(node.attribute("stroke-width").is_none());
                let css = node
                    .attribute("style")
                    .unwrap()
                    .split(';')
                    .filter_map(|item| item.split_once(':'))
                    .collect::<std::collections::BTreeMap<_, _>>();
                assert_eq!(css["fill"], fill);
                assert_eq!(css["stroke"], stroke);
                assert_eq!(css["fill-opacity"].parse::<f64>().unwrap(), fill_alpha);
                assert_eq!(css["stroke-opacity"].parse::<f64>().unwrap(), stroke_alpha);
                assert_eq!(css["opacity"], "1");
                if stroke != "none" {
                    assert_eq!(css["stroke-width"], "3");
                }
            }
        }
    }

    #[test]
    fn journey_document_owns_visible_label_and_background_colors() {
        use merman_display_list::{Color, DrawingCommand, Paint};

        for theme in ["default", "dark"] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "theme": theme,
                    "themeVariables": {"textColor": "#123456", "fillType0": "#abcdef"},
                    "journey": {"sectionFills": ["#fedcba"]}
                })))
                .parse_diagram_for_render_model_sync(
                    "journey\nsection Documentation\nRead: 5: Alice\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            for id in ["journey.section.0.background", "journey.task.0.background"] {
                let style = document
                    .public
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        DrawingCommand::DrawPath { path, style } if path.as_str() == id => {
                            Some(style)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    style.fill,
                    Some(Paint::solid(Color::rgba(171, 205, 239, 255))),
                    "{theme}: the theme's class fill overrides sectionFills for {id}"
                );
            }
            for label in ["Documentation", "Read"] {
                let run = document
                    .public
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        DrawingCommand::DrawText { run } if run.text == label => Some(run),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    run.style.fill,
                    Paint::solid(Color::rgba(18, 52, 86, 255)),
                    "the visible HTML label uses textColor, not the section's fill"
                );
            }
            for command in &document.public.commands {
                if let DrawingCommand::DrawPath { path, style } = command {
                    if path.as_str() == "journey.task.0.face" {
                        assert_eq!(style.stroke.as_ref().unwrap().width, 2.0);
                    }
                    if path.as_str() == "journey.task.0.face.mouth" {
                        assert_eq!(style.fill, Some(Paint::solid(Color::rgba(18, 52, 86, 255))));
                    }
                }
            }
            let render = |config: &serde_json::Value| {
                crate::svg::render_document_svg(
                    &document,
                    &SvgRenderOptions::default(),
                    &SvgDebugOptions::default(),
                    config,
                    &artifact.session,
                )
                .unwrap()
            };
            let svg = render(artifact.metadata.effective_config.as_value());
            assert_eq!(
                svg,
                render(&serde_json::json!({
                    "themeVariables": {"textColor": "red", "fillType0": "black"},
                    "themeCSS": "text { fill: transparent; }"
                })),
                "SVG must not reinterpret the resolved Journey colors"
            );
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                !xml.descendants().any(|node| node.has_tag_name("style")),
                "legacy CSS must not override the document's explicit paints"
            );
            for label in ["Documentation", "Read"] {
                let text = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(label))
                    .unwrap();
                assert_eq!(text.attribute("fill"), Some("#123456"));
            }
        }
    }

    #[test]
    fn journey_svg_root_dimensions_follow_the_public_viewport() {
        use merman_display_list::Rect;

        for use_max_width in [false, true] {
            for title in ["", "title Checkout\n"] {
                let parsed = Engine::new()
                    .with_site_config(merman_core::MermaidConfig::from_value(json!({
                        "journey": {"useMaxWidth": use_max_width}
                    })))
                    .parse_diagram_for_render_model_sync(
                        &format!("journey\n{title}section Cart\nPay: 5: Alice\n"),
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap();
                let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
                let mut document = crate::drawing_list::build_for_family(
                    &artifact.family,
                    &artifact.metadata,
                    DrawingListPolicy::VectorOnly,
                    DrawingListLimits::default(),
                    &artifact.session,
                )
                .unwrap();
                let original = document.public.viewport.bounds;
                // Public dimensions and origin are independently editable; stale sidecar values
                // must not keep the original intrinsic size or turn a translation into resizing.
                for bounds in [
                    original,
                    Rect::new(-11.0, -60.0, 321.0, 234.0),
                    Rect::new(10.0, 70.0, 321.0, 234.0),
                ] {
                    document.public.viewport.bounds = bounds;
                    let svg = crate::svg::render_document_svg(
                        &document,
                        &SvgRenderOptions::default(),
                        &SvgDebugOptions::default(),
                        artifact.metadata.effective_config.as_value(),
                        &artifact.session,
                    )
                    .unwrap();
                    let xml = roxmltree::Document::parse(&svg).unwrap();
                    let root = xml.root_element();
                    let view_box = root
                        .attribute("viewBox")
                        .unwrap()
                        .split_whitespace()
                        .map(|part| part.parse::<f64>().unwrap())
                        .collect::<Vec<_>>();
                    assert_eq!(view_box, [bounds.x, bounds.y, bounds.width, bounds.height]);
                    assert_eq!(
                        root.attribute("height").unwrap().parse::<f64>().unwrap(),
                        bounds.height + 25.0
                    );
                    assert_eq!(root.attribute("preserveAspectRatio"), Some("xMinYMin meet"));
                    if use_max_width {
                        assert_eq!(root.attribute("width"), Some("100%"));
                        assert!(
                            root.attribute("style")
                                .unwrap()
                                .contains(&format!("max-width: {}px", bounds.width))
                        );
                    } else {
                        assert_eq!(
                            root.attribute("width").unwrap().parse::<f64>().unwrap(),
                            bounds.width
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn generic_svg_preserves_solid_and_resource_paint_opacity() {
        use merman_display_list::{
            Color, DrawingCommand, DrawingResource, GradientSpread, GradientStop,
            LinearGradientResource, Paint, Point, Transform,
        };

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        document
            .public
            .resources
            .push(DrawingResource::LinearGradient(LinearGradientResource {
                id: "paint-opacity-gradient".into(),
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
                transform: Transform::IDENTITY,
                spread: GradientSpread::Pad,
                stops: vec![
                    GradientStop::new(0.0, Color::rgba(255, 0, 0, 128)),
                    GradientStop::new(1.0, Color::rgba(0, 0, 255, 255)),
                ],
            }));
        for (paint, opacity) in [
            (
                Paint::solid(Color::rgba(255, 0, 0, 128)).with_opacity(0.5),
                128.0 / 255.0 * 0.5,
            ),
            (
                Paint::resource("paint-opacity-gradient".into()).with_opacity(0.375),
                0.375,
            ),
        ] {
            let stroke = merman_display_list::StrokeStyle {
                paint: paint.clone(),
                width: 1.0,
                dash_array: Vec::new(),
                dash_offset: 0.0,
                line_cap: merman_display_list::LineCap::Butt,
                line_join: merman_display_list::LineJoin::Miter,
                miter_limit: 4.0,
            };
            for command in &mut document.public.commands {
                match command {
                    DrawingCommand::DrawPath { style, .. } => {
                        style.fill = Some(paint.clone());
                        style.stroke = Some(stroke.clone());
                    }
                    DrawingCommand::DrawText { run } => {
                        run.style.fill = paint.clone();
                        run.style.stroke = Some(stroke.clone());
                    }
                    _ => {}
                }
            }
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let shapes: Vec<_> = xml
                .descendants()
                .filter(|node| {
                    node.attribute("data-merman-resource") == Some("info.background")
                        || node.has_tag_name("text")
                })
                .collect();
            assert_eq!(shapes.len(), 2);
            for shape in shapes {
                for attribute in ["fill-opacity", "stroke-opacity"] {
                    let actual: f64 = shape.attribute(attribute).unwrap().parse().unwrap();
                    assert_eq!(
                        actual, opacity,
                        "{attribute} must preserve the floating-point factor"
                    );
                }
                if matches!(paint, Paint::Resource { .. }) {
                    assert!(shape.attribute("fill").unwrap().starts_with("url(#"));
                    assert!(shape.attribute("stroke").unwrap().starts_with("url(#"));
                }
            }
        }
    }

    #[test]
    fn info_document_owns_background_and_text_styles() {
        use merman_display_list::{Color, DrawingCommand, Paint};

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument,
                      config: &serde_json::Value| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                config,
                &artifact.session,
            )
            .unwrap()
        };
        let config = artifact.metadata.effective_config.as_value();
        let svg = render(&document, config);
        assert!(svg.contains("background-color: white;"));
        assert!(!svg.contains("data-merman-resource=\"info.background\""));
        assert_eq!(
            svg,
            render(
                &document,
                &serde_json::json!({
                    "themeVariables": {"textColor": "#ff0000", "fontFamily": "monospace"},
                    "themeCSS": ".version { opacity: 0; }"
                })
            ),
            "resolved Info commands must be independent of external CSS/config"
        );

        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } => {
                    run.style.fill = Paint::solid(Color::rgba(1, 2, 3, 128));
                    run.style.font.families = vec!["serif".into()];
                    run.origin.x = 125.0;
                }
                DrawingCommand::DrawPath { path, style } if path.as_str() == "info.background" => {
                    style.fill = Some(Paint::solid(Color::rgba(12, 34, 56, 255)));
                }
                _ => {}
            }
        }
        let edited = render(&document, config);
        assert!(!edited.contains("background-color: white;"));
        assert!(edited.contains("data-merman-resource=\"info.background\""));
        assert!(edited.contains("#0c2238"));
        assert!(edited.contains("#010203"));
        assert!(edited.contains("font-family:serif;"));
        assert!(edited.contains("fill-opacity:0.5019607843137255;"));
        assert!(edited.contains("<text x=\"125\""));

        document.public.commands.retain(|command| {
            !matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "info.background")
        });
        let removed = render(&document, config);
        assert!(!removed.contains("background-color:"));
        assert!(!removed.contains("#0c2238"));

        let text_index = document
            .public
            .commands
            .iter()
            .position(|command| matches!(command, DrawingCommand::DrawText { .. }))
            .unwrap();
        let DrawingCommand::DrawText { run } = &mut document.public.commands[text_index] else {
            unreachable!();
        };
        run.baseline = merman_display_list::TextBaseline::Hanging;
        document
            .public
            .commands
            .insert(text_index, DrawingCommand::SetOpacity { opacity: 0.25 });
        let expanded = render(&document, config);
        assert!(expanded.contains("dominant-baseline=\"hanging\""));
        assert!(expanded.contains("opacity=\"0.25\""));
    }

    #[test]
    fn sankey_document_owns_paint_order_and_svg_styles() {
        use merman_display_list::{Color, DrawingCommand, Paint};

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "sankey-beta\nA,B,10\nB,C,5\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let mut sequence = Vec::new();
        for command in &document.public.commands {
            let kind = match command {
                DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".shape") => 0,
                DrawingCommand::DrawText { .. } => 1,
                DrawingCommand::DrawPath { path, .. } if path.as_str().ends_with(".path") => 2,
                _ => continue,
            };
            sequence.push(kind);
        }
        assert_eq!(
            sequence
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [0, 1, 2].into()
        );
        assert!(
            sequence.windows(2).all(|pair| pair[0] <= pair[1]),
            "Sankey must paint all nodes, then all labels, then all links: {sequence:?}"
        );
        let render = |document: &crate::drawing_list::RenderDocument,
                      config: &serde_json::Value| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                config,
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document, artifact.metadata.effective_config.as_value());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let layers = xml
            .root_element()
            .children()
            .filter_map(|node| node.attribute("class"))
            .collect::<Vec<_>>();
        assert_eq!(layers, ["nodes", "node-labels", "links"]);
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("text"))
                .all(|node| node.parent().unwrap().attribute("class") == Some("node-labels"))
        );
        assert_eq!(
            svg,
            render(
                &document,
                &serde_json::json!({
                    "themeVariables": {"textColor":"#ff0000"},
                    "themeCSS": ".link { opacity:0; }"
                })
            ),
            "external config must not reinterpret an already resolved Sankey document"
        );
        for command in &mut document.public.commands {
            match command {
                DrawingCommand::SetOpacity { opacity } => *opacity = 0.25,
                DrawingCommand::DrawText { run } => {
                    run.style.fill = Paint::solid(Color::rgba(1, 2, 3, 255))
                }
                _ => {}
            }
        }
        let edited = render(&document, artifact.metadata.effective_config.as_value());
        assert!(edited.contains("#010203"));
        assert!(edited.contains("opacity=\"0.25\""));
        assert!(
            !edited.contains("stroke-opacity:0.5") && !edited.contains("stroke-opacity=\"0.5\""),
            "Sankey must not multiply resolved command opacity by legacy styles"
        );

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  sankey:\n    labelStyle: outlined\n---\nsankey\nA,B,10\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let outlined = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let document = crate::drawing_list::build_for_family(
            &outlined.family,
            &outlined.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &outlined.session,
        )
        .unwrap();
        let svg = render(&document, outlined.metadata.effective_config.as_value());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let labels = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 4);
        for node in &labels[..2] {
            assert_eq!(node.attribute("class"), Some("sankey-label-bg"));
            assert_eq!(node.attribute("stroke-width"), Some("4"));
            assert_eq!(node.attribute("paint-order"), Some("stroke fill"));
            assert_eq!(
                node.parent().unwrap().attribute("class"),
                Some("node-labels")
            );
        }
        for node in &labels[2..] {
            assert_eq!(node.attribute("class"), Some("sankey-label-fg"));
            assert!(node.attribute("stroke-width").is_none());
        }
    }

    #[test]
    fn canonical_svg_and_drawing_list_charge_the_same_document_work() {
        let prepare_packet = || {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(
                    "packet-beta\n0-7: \"Header\"\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            prepare(parsed, &LayoutOptions::default(), session()).unwrap()
        };
        let probe = prepare_packet();
        let document = crate::drawing_list::build_for_family(
            &probe.family,
            &probe.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &probe.session,
        )
        .unwrap();
        let expected = document.public.footprint().unwrap().work_units().unwrap();
        assert!(expected > 0);
        let svg_artifact = prepare_packet();
        let svg_before = svg_artifact.session.report().layout_work_units();
        let svg = svg_artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(
            svg.serialization_route(),
            SvgSerializationRoute::CanonicalDocument
        );
        let svg_work = svg.session.report().layout_work_units() - svg_before;
        let list_artifact = prepare_packet();
        let list_before = list_artifact.session.report().layout_work_units();
        let list = list_artifact
            .render_drawing_list(DrawingListPolicy::VectorOnly, DrawingListLimits::default())
            .unwrap();
        let list_work = list.session.report().layout_work_units() - list_before;
        assert!(
            svg_work >= expected,
            "SVG skipped canonical document work: {svg_work} < {expected}"
        );
        assert_eq!(
            svg_work, list_work,
            "target selection must not change document work charges"
        );
    }

    #[test]
    fn cynefin_identity_geometry_matches_the_source_svg_before_serialization() {
        for id in [None, Some("first"), Some(" a b "), Some("")] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync("cynefin-beta\n", ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let options = crate::svg::normalize_svg_render_options(
                &SvgRenderOptions {
                    diagram_id: id.map(str::to_owned),
                    ..SvgRenderOptions::default()
                },
                &artifact.session,
            )
            .unwrap();
            let source_svg =
                render_legacy_family_artifact_svg(&artifact, &options, &SvgDebugOptions::default())
                    .unwrap();
            let xml = roxmltree::Document::parse(&source_svg).unwrap();
            let expected = xml
                .descendants()
                .filter(|node| node.attribute("class") == Some("cynefinBoundary"))
                .map(|node| {
                    crate::drawing_list::parse_svg_path(node.attribute("d").unwrap()).unwrap()
                })
                .collect::<Vec<_>>();
            let result = artifact
                .render_drawing_list_with_diagram_id(
                    DrawingListPolicy::VectorOnly,
                    DrawingListLimits::default(),
                    id,
                )
                .unwrap();
            let actual = ["cynefin.boundary.fold", "cynefin.boundary.horizontal"].map(|id| {
                result
                    .document()
                    .resources
                    .iter()
                    .find_map(|resource| match resource {
                        merman_display_list::DrawingResource::Path(path)
                            if path.id.as_str() == id =>
                        {
                            Some(path.segments.clone())
                        }
                        _ => None,
                    })
                    .unwrap()
            });
            assert_eq!(
                actual.as_slice(),
                expected,
                "identity {id:?} must determine the same boundary paths for both outputs"
            );
        }
    }

    #[test]
    fn cynefin_document_projects_source_groups_and_distinct_text_baselines() {
        for (accessibility, title_id, description_id) in [
            ("", None, None),
            (
                "  accTitle: Accessible practices\n",
                Some("chart-title-cynefin"),
                None,
            ),
            (
                "  accDescr: Practice description\n",
                None,
                Some("chart-desc-cynefin"),
            ),
            (
                "  accTitle: Accessible practices\n  accDescr: Practice description\n",
                Some("chart-title-cynefin"),
                Some("chart-desc-cynefin"),
            ),
        ] {
            let source = format!(
                "cynefin-beta\n{accessibility}  title Practices\n  complex\n    \"Observe\"\n  complex --> clear : \"move\"\n"
            );
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            // Exercise the candidate serializer directly, without enabling or falling back through
            // the public SVG route while this family's remaining migration work is incomplete.
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let style = xml
                .root_element()
                .children()
                .find(|node| node.has_tag_name("style"))
                .unwrap();
            let empty_group = style.next_sibling_element().unwrap();
            assert!(empty_group.has_tag_name("g"));
            assert_eq!(
                empty_group
                    .children()
                    .filter(|node| node.is_element())
                    .count(),
                0
            );
            let marker = xml
                .descendants()
                .find(|node| node.has_tag_name("marker"))
                .expect("canonical Cynefin keeps the upstream SVG marker contract");
            assert_eq!(marker.attribute("id"), Some("cynefin-arrow-cynefin"));
            assert_eq!(marker.attribute("viewBox"), Some("0 0 10 10"));
            assert_eq!(marker.attribute("refX"), Some("9"));
            assert_eq!(marker.attribute("refY"), Some("5"));
            assert_eq!(marker.attribute("markerWidth"), Some("6"));
            assert_eq!(marker.attribute("markerHeight"), Some("6"));
            assert_eq!(marker.attribute("orient"), Some("auto-start-reverse"));
            assert!(marker.parent().unwrap().parent().unwrap() == xml.root_element());
            let marker_path = marker.first_element_child().unwrap();
            assert_eq!(
                marker_path.attribute("fill"),
                None,
                "marker paint comes from the resolved class rule"
            );
            assert_eq!(marker_path.attribute("stroke"), None);
            assert!(marker_path.attribute("d").unwrap().ends_with('z'));
            let arrow_line = xml
                .descendants()
                .find(|node| node.attribute("class") == Some("cynefinArrowLine"))
                .unwrap();
            assert_eq!(
                arrow_line.attribute("marker-end"),
                Some("url(#cynefin-arrow-cynefin)")
            );
            assert_eq!(arrow_line.attribute("stroke"), None);
            let domains: Vec<_> = xml
                .descendants()
                .filter(|node| node.attribute("class") == Some("cynefinDomain"))
                .collect();
            assert_eq!(domains.len(), 4);
            for domain in domains {
                assert_eq!(domain.attribute("fill-opacity"), Some("0.4"));
                assert_eq!(domain.attribute("opacity"), None);
            }
            assert_eq!(
                xml.root_element().attribute("aria-describedby"),
                description_id
            );
            for (tag, id, text) in [
                ("title", title_id, "Accessible practices"),
                ("desc", description_id, "Practice description"),
            ] {
                let nodes: Vec<_> = xml
                    .root_element()
                    .children()
                    .filter(|node| node.has_tag_name(tag))
                    .collect();
                if let Some(id) = id {
                    assert_eq!(nodes.len(), 2);
                    assert_eq!(nodes[0].attribute("id"), Some(id));
                    assert_eq!(nodes[1].attribute("id"), None);
                    assert_eq!(nodes[0].text(), Some(text));
                    assert_eq!(nodes[1].text(), Some(text));
                    assert!(nodes[0].range().start < style.range().start);
                    assert!(nodes[1].range().start > empty_group.range().start);
                } else {
                    assert!(nodes.is_empty());
                }
            }
            assert_eq!(xml.root_element().attribute("aria-labelledby"), title_id);
            for class in [
                "cynefin-backgrounds",
                "cynefin-boundaries",
                "cynefin-labels",
                "cynefin-subtitles",
                "cynefin-items",
                "cynefin-arrows",
            ] {
                assert!(
                    xml.descendants().any(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some(class)
                    }),
                    "missing source group {class}"
                );
            }
            for (label, baseline) in [("Observe", "central"), ("Practices", "middle")] {
                let text = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(label))
                    .unwrap();
                assert_eq!(
                    text.attribute("dominant-baseline"),
                    Some(baseline),
                    "{label}"
                );
                assert_eq!(
                    text.attribute("font-family"),
                    None,
                    "source text inherits its resolved class style"
                );
                assert_eq!(text.attribute("data-merman-bounds"), None);
            }
            let items = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("cynefin-items")
                })
                .unwrap();
            let boundaries = xml
                .descendants()
                .find(|node| node.attribute("class") == Some("cynefin-boundaries"))
                .unwrap();
            let confusion = boundaries.next_sibling_element().unwrap();
            assert_eq!(confusion.attribute("class"), Some("cynefinConfusion"));
            assert!(confusion.has_tag_name("path"));
            assert_eq!(
                confusion.next_sibling_element().unwrap().attribute("class"),
                Some("cynefin-labels")
            );
            let item = items
                .children()
                .find(|node| node.has_tag_name("g"))
                .unwrap();
            assert!(item.attribute("transform").is_some());
            assert!(item.children().any(|node| node.has_tag_name("rect")));
            assert!(item.children().any(|node| node.has_tag_name("text")));
            let BuiltinFamilyArtifact::Cynefin(pair) = &artifact.family else {
                panic!("expected Cynefin artifact");
            };
            let expected_item = &pair.layout().items[0];
            let (x, y) = item
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split_once(',')
                .unwrap();
            assert!((x.trim().parse::<f64>().unwrap() - expected_item.x).abs() < 0.001);
            assert!((y.trim().parse::<f64>().unwrap() - expected_item.y).abs() < 0.001);
            let text = item
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert_eq!(
                text.attribute("x").unwrap().parse::<f64>().unwrap(),
                expected_item.text_x
            );
            assert_eq!(
                text.attribute("y").unwrap().parse::<f64>().unwrap(),
                expected_item.text_y
            );
            let conflicting_config = json!({
                "themeCSS": ".cynefinItem { opacity: 0; }",
                "themeVariables": {"fontFamily": "monospace", "textColor": "red"},
                "cynefin": {"itemFontSize": 99, "boundaryWidth": 12}
            });
            let render = |document: &crate::drawing_list::RenderDocument, config: &Value| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions::default(),
                    &drawing_list_svg_diagnostics(),
                    config,
                    &artifact.session,
                )
                .unwrap()
            };
            assert_eq!(
                svg,
                render(&document, &conflicting_config),
                "Cynefin SVG must not reinterpret visual config after document construction"
            );
            let mut changed = 0;
            for command in &mut document.public.commands {
                use merman_display_list::{Color, DrawingCommand, Paint};
                match command {
                    DrawingCommand::DrawText { run } if run.text == "Observe" => {
                        run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
                        run.style.font_size = 27.0;
                        changed += 1;
                    }
                    DrawingCommand::DrawPath { path, style }
                        if style.fill.is_some()
                            && (path.as_str() == "cynefin.item.0.shape"
                                || path.as_str() == "cynefin.background") =>
                    {
                        style.fill = Some(Paint::solid(Color::rgba(18, 52, 86, 128)));
                        changed += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(changed, 3);
            let modified = render(&document, &conflicting_config);
            let modified_xml = roxmltree::Document::parse(&modified).unwrap();
            assert!(
                !modified_xml
                    .root_element()
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("background-color")
            );
            for class in ["cynefinItem", "cynefinItemText", "cynefinBackground"] {
                let node = modified_xml
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap();
                if class == "cynefinItemText" {
                    let css = modified_xml
                        .descendants()
                        .find(|node| node.has_tag_name("style"))
                        .and_then(|node| node.text())
                        .unwrap();
                    let rule = css
                        .split(".cynefinItemText{")
                        .nth(1)
                        .unwrap()
                        .split('}')
                        .next()
                        .unwrap();
                    assert!(rule.contains("fill:#123456;"));
                    assert!(rule.contains("fill-opacity:0.5019607843137255;"));
                    assert!(rule.contains("font-size:27px;"));
                    assert_eq!(node.attribute("fill"), None);
                    continue;
                }
                assert_eq!(node.attribute("fill"), Some("#123456"));
                let alpha = node
                    .attribute("fill-opacity")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                let fill_opacity = if class == "cynefinItem" { 0.95 } else { 1.0 };
                assert!((alpha - fill_opacity * 128.0 / 255.0).abs() < 0.001);
            }
            let index = document
                .public
                .commands
                .iter()
                .position(|command| {
                    matches!(command,
                merman_display_list::DrawingCommand::DrawText { run } if run.text == "Observe")
                })
                .unwrap();
            let mut extra = document.public.commands[index].clone();
            let merman_display_list::DrawingCommand::DrawText { run } = &mut extra else {
                unreachable!();
            };
            run.text = "Different style".into();
            run.style.font_size = 14.0;
            document.public.commands.insert(index + 1, extra);
            let heterogeneous = render(&document, &conflicting_config);
            let heterogeneous_xml = roxmltree::Document::parse(&heterogeneous).unwrap();
            assert!(!heterogeneous.contains(".cynefinItemText{"));
            for (label, size) in [("Observe", "27"), ("Different style", "14")] {
                let node = heterogeneous_xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(label))
                    .unwrap();
                assert_eq!(node.attribute("font-size"), Some(size));
                assert_eq!(node.attribute("fill"), Some("#123456"));
            }
            for command in &mut document.public.commands {
                if let merman_display_list::DrawingCommand::DrawPath { path, style } = command
                    && path.as_str() == "cynefin.boundary.fold"
                {
                    style.stroke.as_mut().unwrap().width = 9.0;
                }
            }
            let heterogeneous = render(&document, &conflicting_config);
            assert!(!heterogeneous.contains(".cynefinBoundary{"));
            let xml = roxmltree::Document::parse(&heterogeneous).unwrap();
            let widths = xml
                .descendants()
                .filter(|node| node.attribute("class") == Some("cynefinBoundary"))
                .map(|node| node.attribute("stroke-width").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(widths, ["9", "2"]);

            let stroke = document.public.commands.iter().find_map(|command| {
                if let merman_display_list::DrawingCommand::DrawPath { path, style } = command
                    && path.as_str() == "cynefin.boundary.fold"
                {
                    style.stroke.clone()
                } else {
                    None
                }
            });
            for with_stroke in [false, true] {
                for command in &mut document.public.commands {
                    if let merman_display_list::DrawingCommand::DrawPath { path, style } = command
                        && path.as_str() == "cynefin.domain.complex.background"
                    {
                        style.fill = Some(merman_display_list::Paint::solid(
                            merman_display_list::Color::rgba(11, 22, 33, 128),
                        ));
                        style.stroke = with_stroke.then(|| stroke.clone().unwrap());
                    }
                }
                let svg = render(&document, &conflicting_config);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let domain = xml
                    .descendants()
                    .find(|node| {
                        node.attribute("class") == Some("cynefinDomain")
                            && node.attribute("fill") == Some("#0b1621")
                    })
                    .unwrap();
                let alpha: f64 = domain.attribute("fill-opacity").unwrap().parse().unwrap();
                let expected = 128.0 / 255.0 * if with_stroke { 1.0 } else { 0.4 };
                assert!((alpha - expected).abs() < 0.001);
                assert_eq!(domain.attribute("opacity"), with_stroke.then_some("0.4"));
            }
        }
    }

    #[test]
    fn venn_normal_line_host_metrics_reach_both_svg_projections_without_remeasurement() {
        struct NormalLineHost {
            metrics: crate::text::NormalLineMetrics,
            calls: AtomicUsize,
        }

        impl HostTextMeasurer for NormalLineHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                self.calls.fetch_add(1, Ordering::Relaxed);
                Ok(
                    (request.operation == TextMeasurementOperation::NormalLineMetrics)
                        .then_some(HostTextMeasurement::NormalLineMetrics(self.metrics)),
                )
            }
        }

        for (line_height, baseline_offset) in [(23.0, 19.0), (31.0, 7.0)] {
            let host = Arc::new(NormalLineHost {
                metrics: crate::text::NormalLineMetrics {
                    line_height,
                    baseline_offset,
                },
                calls: AtomicUsize::new(0),
            });
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.venn-normal-line").unwrap(),
                "1",
            )
            .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    host.clone(),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(
                    "venn-beta\nset A[\"Alpha\"]:20\n  text A1[\"React\"]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let BuiltinFamilyArtifact::Venn(pair) = &artifact.family else {
                panic!("expected Venn artifact");
            };
            let node = &pair.layout.text_nodes[0];
            let expected_y = node.y + (node.height - line_height) / 2.0 + baseline_offset;
            let document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let run = document
                .public
                .commands
                .iter()
                .find_map(|command| match command {
                    merman_display_list::DrawingCommand::DrawText { run }
                        if run.text == "React" =>
                    {
                        Some(run)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(run.origin.y, expected_y);
            assert_eq!(run.bounds.height, line_height);
            assert_eq!(run.style.line_height, line_height);

            let calls = host.calls.load(Ordering::Relaxed);
            assert!(calls > 0);
            let svg = crate::svg::render_document_svg(
                &document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap();
            let native = crate::svg::SvgPipeline::resvg_safe()
                .process_to_string(&svg, &artifact.session)
                .unwrap();
            for output in [&svg, &native] {
                let xml = roxmltree::Document::parse(output).unwrap();
                let texts = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("text") && node.text() == Some("React"))
                    .collect::<Vec<_>>();
                assert_eq!(texts.len(), 1);
                let y: f64 = texts[0].attribute("y").unwrap().parse().unwrap();
                assert!(
                    (y - expected_y).abs() < 0.001,
                    "SVG must retain the resolved baseline"
                );
            }
            assert_eq!(
                host.calls.load(Ordering::Relaxed),
                calls,
                "serializers must not measure the label again"
            );
        }
    }

    #[test]
    fn venn_text_node_svg_projects_public_container_and_baselines_without_rewrapping() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\nset A[\"Alpha\"]:20\n  text A1[\"First <br> word with enough additional content to wrap naturally across several resolved lines\"]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                &json!({"themeCSS": "span {color:red}"}),
                &artifact.session,
            )
            .unwrap()
        };
        let initial = render(&document);
        assert!(
            initial.contains("<foreignObject"),
            "canonical text must retain the source HTML identity shell"
        );
        let initial_xml = roxmltree::Document::parse(&initial).unwrap();
        let initial_fo = initial_xml
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            initial_fo.parent().unwrap().attribute("class"),
            Some("venn-text-area"),
            "the source foreignObject must remain a direct text-area child"
        );
        assert_eq!(
            initial_fo.parent().unwrap().attribute("font-size"),
            Some("20px")
        );
        assert!(
            initial_fo
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .count()
                > 1,
            "the HTML shell must exercise multiple finalized lines"
        );
        let container = document
            .public
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "venn.text.0.container" => {
                    Some(path)
                }
                _ => None,
            })
            .expect("container geometry belongs to the public document");
        let merman_display_list::PathSegment::MoveTo { to: corner } = container.segments[0] else {
            panic!("rectangle origin");
        };
        let corner = corner;
        let semantic = document
            .public
            .semantics
            .iter_mut()
            .find(|semantic| semantic.id == "venn.text.0")
            .unwrap();
        semantic.description = Some("Public node description".to_owned());
        semantic.role = merman_display_list::SemanticRole::Node;
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawText { run } = command
                && run.text.contains("First")
            {
                run.origin.y += 17.0;
                run.bounds.y -= 3.0;
                run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
                run.style.font_size = 27.0;
                run.text = "Edited <literal>".to_owned();
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let fo = xml
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(fo.attribute("class"), Some("venn-text-node-fo"));
        assert_eq!(fo.attribute("x").unwrap().parse::<f64>().unwrap(), corner.x);
        assert_eq!(fo.attribute("y").unwrap().parse::<f64>().unwrap(), corner.y);
        let span = fo.children().find(|node| node.is_element()).unwrap();
        assert_eq!(
            span.tag_name().namespace(),
            Some("http://www.w3.org/1999/xhtml")
        );
        assert_eq!(span.attribute("class"), Some("venn-text-node"));
        let texts: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.text() == Some("Edited <literal>"))
            .collect();
        assert_eq!(
            texts.len(),
            1,
            "the HTML shell owns one canonical native text projection"
        );
        let run = document
            .public
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text == "Edited <literal>" => Some(run),
                _ => None,
            })
            .unwrap();
        for text in texts {
            assert_eq!(
                text.attribute("y").unwrap().parse::<f64>().unwrap(),
                run.origin.y
            );
            assert_eq!(text.attribute("fill"), Some("#123456"));
            assert_eq!(text.attribute("dominant-baseline"), Some("alphabetic"));
        }
        let semantic = xml
            .descendants()
            .find(|node| node.attribute("data-merman-semantic-id") == Some("venn.text.0"))
            .unwrap();
        assert_eq!(
            semantic.attribute("aria-description"),
            Some("Public node description")
        );
        assert!(
            !span
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("Public node description")),
            "accessibility metadata must not pollute the HTML label text content"
        );
        assert!(
            fo.parent().unwrap().attribute("font-size").is_none(),
            "mixed public font sizes cannot share a parent font-size projection"
        );
        assert!(!xml.descendants().any(|node| node.has_tag_name("text")
            && node.text().is_some_and(|text| text.contains("First"))));
        assert!(!svg.contains("<br>"));
        assert!(!svg.contains("white-space: normal"));
        let hidden = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &SvgDebugOptions {
                include_nodes: false,
                ..drawing_list_svg_diagnostics()
            },
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let hidden_xml = roxmltree::Document::parse(&hidden).unwrap();
        assert_eq!(
            hidden_xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some("venn.text.0"))
                .unwrap()
                .attribute("display"),
            Some("none")
        );
        let clusters_hidden = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &SvgDebugOptions {
                include_clusters: false,
                ..drawing_list_svg_diagnostics()
            },
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let clusters_xml = roxmltree::Document::parse(&clusters_hidden).unwrap();
        for class in ["venn-text-nodes", "venn-text-area"] {
            assert_eq!(
                clusters_xml
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap()
                    .attribute("display"),
                Some("none")
            );
        }

        let native = crate::svg::SvgPipeline::resvg_safe()
            .process_to_string(&svg, &artifact.session)
            .unwrap();
        let native_xml = roxmltree::Document::parse(&native).unwrap();
        assert!(!native.contains("<foreignObject"));
        assert_eq!(
            native_xml
                .descendants()
                .find(|node| node.attribute("data-merman-semantic-id") == Some("venn.text.0"))
                .unwrap()
                .attribute("aria-description"),
            Some("Public node description")
        );
        let native_text: Vec<_> = native_xml
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.text() == Some("Edited <literal>"))
            .collect();
        assert_eq!(
            native_text.len(),
            1,
            "headless output keeps one exact native branch, not a remeasured HTML overlay"
        );
        assert_eq!(
            native_text[0]
                .attribute("y")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            run.origin.y
        );

        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == "venn.text.0.container"
            {
                for segment in &mut path.segments {
                    match segment {
                        merman_display_list::PathSegment::MoveTo { to }
                        | merman_display_list::PathSegment::LineTo { to } => {
                            to.x += 13.0;
                            to.y += 11.0;
                        }
                        _ => {}
                    }
                }
            }
        }
        let moved = render(&document);
        let moved_xml = roxmltree::Document::parse(&moved).unwrap();
        let moved_fo = moved_xml
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            moved_fo.attribute("x").unwrap().parse::<f64>().unwrap(),
            corner.x + 13.0
        );
        assert_eq!(
            moved_fo.attribute("y").unwrap().parse::<f64>().unwrap(),
            corner.y + 11.0
        );
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "venn.text.0.container"
            {
                style.fill = Some(Paint::solid(Color::rgba(255, 0, 0, 255)));
            }
        }
        let painted = render(&document);
        assert!(
            !painted.contains("<foreignObject"),
            "a painted box no longer qualifies for the inert HTML shell"
        );
        assert!(painted.contains("fill=\"#ff0000\""));
        let painted_xml = roxmltree::Document::parse(&painted).unwrap();
        assert!(
            painted_xml
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("Edited <literal>"))
        );
    }

    #[test]
    fn venn_empty_text_node_keeps_its_public_container_and_html_identity() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\nset A[\"Alpha\"]:20\n  text A1[\" \"]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &drawing_list_svg_diagnostics(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let fo = xml
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let span = fo.children().find(|node| node.is_element()).unwrap();
        assert_eq!(span.attribute("class"), Some("venn-text-node"));
        assert_eq!(
            span.descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            0
        );
        assert!(
            span.descendants()
                .any(|node| node.attribute("data-merman-semantic-id") == Some("venn.text.0")),
            "an empty label still retains its semantic anchor"
        );
        assert!(!svg.contains("<switch>"));
        assert!(!svg.contains(">A1<"));
    }

    #[test]
    fn venn_rough_svg_projects_public_paths_paint_and_source_groups() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let input = "---\nconfig:\n  look: handDrawn\n  handDrawnSeed: 1\n---\nvenn-beta\nset A\nset B\nunion A,B\nstyle A,B fill:#ffe66d\n";
        let prepare_input = || {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
                .unwrap()
                .unwrap();
            prepare(parsed, &LayoutOptions::default(), session()).unwrap()
        };
        let source = prepare_input()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let source = roxmltree::Document::parse(source.svg()).unwrap();
        let artifact = prepare_input();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                &json!({}),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let svg = roxmltree::Document::parse(&svg).unwrap();
        for sets in ["A", "B", "A_B"] {
            let paths = |doc: &roxmltree::Document<'_>| {
                let area = doc
                    .descendants()
                    .find(|node| node.attribute("data-venn-sets") == Some(sets))
                    .unwrap();
                let group = area
                    .children()
                    .find(|node| node.has_tag_name("g"))
                    .expect("source rough wrapper");
                group
                    .children()
                    .filter(|node| node.has_tag_name("path"))
                    .map(|node| {
                        let color = merman_core::theme_color::ThemeColor::parse(
                            node.attribute("stroke").unwrap(),
                        )
                        .unwrap()
                        .rgba_channels();
                        (
                            node.attribute("d").unwrap().to_owned(),
                            node.attribute("stroke-width").unwrap().to_owned(),
                            node.attribute("fill").unwrap().to_owned(),
                            [color.red.round(), color.green.round(), color.blue.round()],
                            color.alpha,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let actual = paths(&svg);
            let expected = paths(&source);
            assert_eq!(actual.len(), expected.len(), "rough group {sets}");
            for (actual, expected) in actual.iter().zip(&expected) {
                assert_eq!(actual.0, expected.0, "path geometry for {sets}");
                assert_eq!(actual.1, expected.1, "stroke width for {sets}");
                assert_eq!(actual.2, expected.2, "fill for {sets}");
                assert_eq!(actual.3, expected.3, "8-bit RGB for {sets}");
                assert!((actual.4 - expected.4).abs() < 1e-12, "alpha for {sets}");
            }
        }
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "venn.area.0.rough-fill"
            {
                let stroke = style.stroke.as_mut().unwrap();
                stroke.paint = Paint::Solid {
                    color: Color {
                        red: 12,
                        green: 34,
                        blue: 56,
                        alpha: 255,
                    },
                    opacity: 1.0,
                };
                stroke.width = 3.25;
            }
        }
        let edited = render(&document);
        let edited = roxmltree::Document::parse(&edited).unwrap();
        let area = edited
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        let path = area
            .descendants()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        let color = merman_core::theme_color::ThemeColor::parse(path.attribute("stroke").unwrap())
            .unwrap()
            .rgba_channels();
        assert_eq!([color.red, color.green, color.blue], [12.0, 34.0, 56.0]);
        assert!((color.alpha - 0.3).abs() < 1e-12);
        assert_eq!(path.attribute("stroke-width"), Some("3.25"));
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "venn.area.0.rough-fill"
            {
                style.stroke.as_mut().unwrap().line_cap = merman_display_list::LineCap::Round;
            }
        }
        let edited = render(&document);
        let edited = roxmltree::Document::parse(&edited).unwrap();
        let area = edited
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        let path = area
            .descendants()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        assert_eq!(
            path.attribute("stroke-linecap"),
            Some("round"),
            "edited strokes must use the general projection without losing their cap"
        );
    }

    #[test]
    fn venn_document_styles_do_not_reinterpret_external_config() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "venn-beta\n title Product Surface\n set A[\"Core\"]:20\n set B[\"Editor\"]:14\n union A,B[\"Shared\"]:4\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument, config: &Value| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                config,
                &artifact.session,
            )
            .unwrap()
        };
        let config = artifact.metadata.effective_config.as_value();
        let svg = render(&document, config);
        let conflicting = json!({"themeCSS": "text { fill: red !important; }", "themeVariables": {"fontFamily": "monospace", "titleColor": "red", "vennSetTextColor": "blue"}});
        assert!(
            svg == render(&document, &conflicting),
            "Venn SVG must consume resolved document styles, not external theme CSS"
        );
        let area_draws: Vec<_> = document
            .public
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                if let DrawingCommand::DrawPath { path, style } = command
                    && path.as_str() == "venn.area.0.shape"
                {
                    Some((index, style))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(area_draws.len(), 2);
        for ((index, style), opacity) in area_draws.iter().zip([0.1, 0.95]) {
            assert!(
                matches!(document.public.commands[index - 1], DrawingCommand::SetOpacity { opacity: value } if value == opacity)
            );
            assert!(matches!(
                document.public.commands[index + 1],
                DrawingCommand::Restore
            ));
            let paint = style
                .fill
                .as_ref()
                .or_else(|| style.stroke.as_ref().map(|stroke| &stroke.paint))
                .unwrap();
            assert!(matches!(paint, Paint::Solid { color, .. } if color.alpha == 255));
        }
        assert!(document.public.commands.iter().any(|command| matches!(command,
            DrawingCommand::DrawPath { path, style } if path.as_str() == "venn.area.2.shape" && style.fill.is_none() && style.stroke.is_none()
        )), "transparent intersection must retain its geometry");
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let root = xml.root_element();
        assert_eq!(
            root.children()
                .filter(|node| node.is_element())
                .map(|node| node.tag_name().name())
                .collect::<Vec<_>>(),
            ["style", "g", "text", "g"]
        );
        let content = root
            .children()
            .filter(|node| node.is_element())
            .next_back()
            .unwrap();
        assert_eq!(content.attribute("transform"), Some("translate(0, 24)"));
        assert_eq!(
            root.children()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .attribute("x"),
            Some("50%")
        );
        assert_eq!(
            root.children()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .attribute("font-size"),
            Some("16px"),
            "retain the source's scaled presentation attribute"
        );
        assert!(
            root.children()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap_or_default()
                .contains("font-size:32px;"),
            "the public 32px title size must win through the source-shaped author rule"
        );
        let without_clusters = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &SvgDebugOptions {
                include_clusters: false,
                ..Default::default()
            },
            config,
            &artifact.session,
        )
        .unwrap();
        let without_clusters = roxmltree::Document::parse(&without_clusters).unwrap();
        assert!(
            without_clusters
                .descendants()
                .all(|node| node.attribute("display") != Some("none")),
            "structural content must not be treated as a cluster"
        );
        let areas = content
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        assert_eq!(areas.len(), 3);
        for (index, area) in areas.into_iter().enumerate() {
            assert!(area.attribute("class").unwrap().starts_with("venn-area "));
            assert!(area.attribute("data-venn-sets").is_some());
            let path = area
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            let data = path.attribute("d").unwrap();
            if index < 2 {
                assert!(
                    data.contains(" m ") && data.matches(" a ").count() == 2,
                    "classic circles must retain source relative command spelling"
                );
            }
            let resource_id = format!("venn.area.{index}.shape");
            let segments = document
                .public
                .resources
                .iter()
                .find_map(|resource| match resource {
                    merman_display_list::DrawingResource::Path(path)
                        if path.id.as_str() == resource_id =>
                    {
                        Some(&path.segments)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                &crate::drawing_list::parse_svg_path(data).unwrap(),
                segments,
                "SVG command spelling must preserve the public geometry exactly"
            );
            assert_eq!(
                area.children()
                    .filter(|node| node.is_element())
                    .map(|node| node.tag_name().name())
                    .collect::<Vec<_>>(),
                ["path", "text"]
            );
            let label = area
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert_eq!(label.attribute("class"), Some("label"));
            assert_eq!(label.children().filter(|node| node.is_element()).count(), 1);
            assert!(
                label.children().any(
                    |node| node.has_tag_name("tspan") && node.attribute("dy") == Some("0.35em")
                )
            );
        }
        assert!(
            xml.root_element()
                .attribute("style")
                .unwrap()
                .contains("background-color: white")
        );
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("data-merman-resource") == Some("venn.background"))
        );
        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } => {
                    run.style.fill = Paint::solid(Color::rgba(11, 22, 33, 128));
                    run.style.font_size = 27.0;
                    run.origin.x += 13.0;
                }
                DrawingCommand::DrawPath { path, style } if path.as_str() == "venn.background" => {
                    style.fill = Some(Paint::solid(Color::rgba(11, 22, 33, 128)));
                }
                _ => {}
            }
        }
        let svg = render(&document, &conflicting);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !xml.root_element()
                .attribute("style")
                .unwrap_or_default()
                .contains("background-color")
        );
        let mut text_count = 0;
        for node in xml.descendants().filter(|node| node.has_tag_name("text")) {
            text_count += 1;
            let style = node.attribute("style").unwrap();
            assert!(style.contains("fill: #0b1621;"));
            if node.attribute("class") == Some("venn-title") {
                assert_eq!(node.attribute("x"), Some("413"));
            } else {
                assert!(style.contains("font-size: 27px;"));
            }
        }
        assert_eq!(text_count, 4);
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("style"))
                .any(|node| node.text().unwrap_or_default().contains("font-size:27px;")),
            "edited public title style must replace the CSS projection"
        );
        let background = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some("venn.background"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#0b1621"));
        assert!(
            (background
                .attribute("fill-opacity")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                - 128.0 / 255.0)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn venn_public_scope_metadata_survives_native_projection() {
        use merman_display_list::SemanticRole;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\ntitle Overlap\nset A\nset B\nunion A,B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let baseline = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &drawing_list_svg_diagnostics(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let original = render(&baseline);
        let xml = roxmltree::Document::parse(&original).unwrap();
        let area = xml
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        let annotation = baseline
            .public
            .semantics
            .iter()
            .find(|entry| entry.id == "venn.area.0")
            .unwrap();
        assert_eq!(
            area.attribute("aria-description"),
            annotation.description.as_deref()
        );
        assert_eq!(annotation.description, None);
        assert_eq!(annotation.title.as_deref(), Some("A"));
        assert_eq!(area.attribute("aria-label"), None);
        let intersection = baseline
            .public
            .semantics
            .iter()
            .find(|entry| entry.id == "venn.area.2")
            .unwrap();
        assert_eq!(intersection.title, None);
        assert_eq!(intersection.description, None);
        for id in ["venn.content", "venn.title", "venn.area.0"] {
            for field in ["title", "description", "link", "role"] {
                let mut document = baseline.clone();
                let semantic = document
                    .public
                    .semantics
                    .iter_mut()
                    .find(|entry| entry.id == id)
                    .unwrap();
                match field {
                    "title" => semantic.title = Some("Independent &amp; <br> name".into()),
                    // An explicit description must survive even when it repeats old generated prose.
                    "description" => semantic.description = Some("Sets: A; size: 1".into()),
                    "link" => semantic.link = Some("https://example.com/area?a=1&b=2".into()),
                    "role" => semantic.role = SemanticRole::Edge,
                    _ => unreachable!(),
                }
                let svg = render(&document);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                match field {
                    "title" => assert!(
                        xml.descendants().any(|node| node.attribute("aria-label")
                            == Some("Independent &amp; <br> name")
                            || node.has_tag_name("title")
                                && node.text() == Some("Independent &amp; <br> name")),
                        "{id}"
                    ),
                    "description" => assert!(
                        xml.descendants()
                            .any(|node| node.attribute("aria-description")
                                == Some("Sets: A; size: 1")
                                || node.has_tag_name("desc")
                                    && node.text() == Some("Sets: A; size: 1")),
                        "{id}"
                    ),
                    "link" => assert!(
                        xml.descendants().any(|node| node.attribute("href")
                            == Some("https://example.com/area?a=1&b=2")),
                        "{id}"
                    ),
                    "role" => assert!(
                        xml.descendants()
                            .any(|node| node.attribute("data-merman-semantic-id") == Some(id)
                                && node.attribute("class").is_some_and(|classes| classes
                                    .split_whitespace()
                                    .any(|class| class == "semantic-edge"))),
                        "{id}"
                    ),
                    _ => unreachable!(),
                }
                let painted_tags = |xml: &roxmltree::Document<'_>| {
                    xml.descendants()
                        .filter(|node| {
                            node.is_element()
                                && matches!(node.tag_name().name(), "path" | "text" | "tspan")
                        })
                        .map(|node| {
                            (
                                node.tag_name().name().to_owned(),
                                node.attribute("d").map(str::to_owned),
                                node.text().map(str::to_owned),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let original_xml = roxmltree::Document::parse(&original).unwrap();
                assert_eq!(
                    painted_tags(&xml),
                    painted_tags(&original_xml),
                    "{id} {field}"
                );
            }
        }
    }

    #[test]
    fn venn_unpainted_area_label_retains_literal_public_name() {
        use merman_display_list::{DrawingCommand, Paint};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "venn-beta\nset A[\"Alpha   &amp; Beta\"]\nset B\nunion A,B[\"Shared\"]\nstyle A color:none\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let annotation = document
            .public
            .semantics
            .iter()
            .find(|entry| entry.id == "venn.area.0")
            .unwrap();
        assert_eq!(annotation.title.as_deref(), Some("Alpha &amp; Beta"));
        let run = document
            .public
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run }
                    if Some(run.text.as_str()) == annotation.title.as_deref() =>
                {
                    Some(run)
                }
                _ => None,
            })
            .unwrap();
        assert!(matches!(run.style.fill, Paint::Solid { color, .. } if color.alpha == 0));
        assert_eq!(
            document
                .public
                .semantics
                .iter()
                .find(|entry| entry.id == "venn.area.2")
                .unwrap()
                .title
                .as_deref(),
            Some("Shared")
        );
        let svg = crate::svg::render_document_svg(
            &document,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
            artifact.metadata.effective_config.as_value(),
            &artifact.session,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let area = xml
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        assert_eq!(area.attribute("aria-label"), None);
        assert!(area.descendants().any(|node| {
            node.has_tag_name("text")
                && node
                    .descendants()
                    .any(|child| child.is_text() && child.text() == Some("Alpha &amp; Beta"))
        }));
    }

    #[test]
    fn venn_projection_preserves_empty_labels_and_edited_paint_scopes() {
        use merman_display_list::{Color, DrawingCommand, LineCap, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\nset A\nset B\nunion A,B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let semantic = document
            .public
            .semantics
            .iter_mut()
            .find(|semantic| semantic.id == "venn.document")
            .unwrap();
        semantic.title = Some("Accessible Venn".to_owned());
        semantic.description = Some("Two sets".to_owned());
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.root_element()
                .children()
                .filter(|node| node.is_element())
                .map(|node| node.tag_name().name())
                .collect::<Vec<_>>(),
            ["title", "desc", "style", "g", "g"]
        );
        assert_eq!(
            xml.root_element().attribute("aria-labelledby"),
            Some("chart-title-venn")
        );
        let intersection = xml
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A_B"))
            .unwrap();
        let empty = intersection
            .descendants()
            .find(|node| node.has_tag_name("tspan"))
            .unwrap();
        assert_eq!(empty.text().unwrap_or_default(), "");
        assert!(empty.attribute("x").is_some() && empty.attribute("y").is_some());

        let fill_index = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
                    DrawingCommand::DrawPath { path, .. } if path.as_str() == "venn.area.0.shape"
                )
            })
            .unwrap();
        if let DrawingCommand::SetOpacity { opacity } =
            &mut document.public.commands[fill_index - 1]
        {
            *opacity = 0.37;
        } else {
            panic!("fill opacity scope")
        }
        if let DrawingCommand::DrawPath { style, .. } = &mut document.public.commands[fill_index] {
            style.fill = Some(Paint::solid(Color::rgba(11, 22, 33, 128)));
        }
        for command in &mut document.public.commands {
            if let DrawingCommand::ConcatTransform { transform } = command {
                transform.f = 37.0;
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let area = xml
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        assert_eq!(
            area.parent().unwrap().attribute("transform"),
            Some("translate(0, 37)")
        );
        let paths = area
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        let styles = paths[0].attribute("style").unwrap();
        let alpha: f64 = styles
            .split(';')
            .find_map(|item| item.trim().strip_prefix("fill-opacity:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!((alpha - 128.0 / 255.0 * 0.37).abs() < 0.001);
        assert!(styles.contains("fill: #0b1621") && styles.contains("stroke-opacity: 0.95"));
        assert!(
            area.children()
                .filter(|node| node.is_element())
                .all(|node| node.attribute("transform").is_none())
        );

        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "venn.area.0.shape"
                && let Some(stroke) = &mut style.stroke
            {
                stroke.line_cap = LineCap::Round;
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let area = xml
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap();
        assert_eq!(
            area.children()
                .filter(|node| node.has_tag_name("path"))
                .count(),
            2
        );
        assert!(
            area.children()
                .any(|node| node.attribute("stroke-linecap") == Some("round")
                    && node.attribute("opacity") == Some("0.95"))
        );
        let text_index = document
            .public
            .commands
            .iter()
            .position(|command| matches!(command, DrawingCommand::DrawText { .. }))
            .unwrap();
        document.public.commands.insert(
            text_index,
            DrawingCommand::SetBlendMode {
                blend_mode: merman_display_list::BlendMode::Multiply,
            },
        );
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(xml.descendants().any(|node| {
            node.has_tag_name("text")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("mix-blend-mode: multiply;"))
        }));
    }

    #[test]
    fn cynefin_fill_opacity_is_exact_and_independent_of_strokes() {
        use merman_display_list::{DrawingCommand, Paint};
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "cynefin-beta\ncomplex\n  \"Observe\"\nconfusion\n  \"A\"\n  \"B\"\n  \"C\"\n  \"D\"\n",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let crate::drawing_list::SvgStructureBody::Cynefin(body) = &document.svg.body else {
            panic!("expected Cynefin sidecar");
        };
        let mut observed = std::collections::BTreeSet::new();
        for (id, class) in &body.path_classes {
            let opacity = match class.as_str() {
                "cynefinConfusion" => 0.5,
                "cynefinItem" => 0.95,
                "cynefinItemOverflow" => 0.6,
                _ => continue,
            };
            observed.insert(class.as_str());
            let index = document
                .public
                .commands
                .iter()
                .position(|command| {
                    matches!(command,
                DrawingCommand::DrawPath { path, .. } if path.as_str() == id)
                })
                .unwrap();
            let [
                DrawingCommand::Save,
                DrawingCommand::SetOpacity { opacity: actual },
                DrawingCommand::DrawPath {
                    path: fill_id,
                    style: fill,
                },
                DrawingCommand::Restore,
                DrawingCommand::DrawPath {
                    path: stroke_id,
                    style: stroke,
                },
            ] = &document.public.commands[index - 2..index + 3]
            else {
                panic!("{id} must carry exact fill opacity separately from stroke");
            };
            assert_eq!(*actual, opacity);
            assert_eq!(fill_id, stroke_id);
            assert!(fill.stroke.is_none());
            assert!(matches!(fill.fill, Some(Paint::Solid { color, .. }) if color.alpha == 255));
            assert!(stroke.fill.is_none() && stroke.stroke.is_some());
        }
        assert_eq!(observed.len(), 3);
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for (class, opacity) in [
            ("cynefinConfusion", "0.5"),
            ("cynefinItem", "0.95"),
            ("cynefinItemOverflow", "0.6"),
        ] {
            let nodes: Vec<_> = xml
                .descendants()
                .filter(|node| node.attribute("class") == Some(class))
                .collect();
            assert_eq!(
                nodes.len(),
                body.path_classes
                    .values()
                    .filter(|value| value.as_str() == class)
                    .count()
            );
            for node in nodes {
                assert_eq!(node.attribute("fill-opacity"), Some(opacity));
                assert_eq!(node.attribute("opacity"), None);
                assert_eq!(node.attribute("stroke"), None);
            }
        }

        let fill_index = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "cynefin.item.0.shape")
            })
            .unwrap();
        for edit in [
            "opacity",
            "added-stroke",
            "removed-stroke",
            "outer-opacity",
            "outer-blend",
        ] {
            let mut edited = document.clone();
            match edit {
                "opacity" => {
                    edited.public.commands[fill_index - 1] =
                        DrawingCommand::SetOpacity { opacity: 0.37 }
                }
                "added-stroke" => {
                    let DrawingCommand::DrawPath { style: stroke, .. } =
                        &document.public.commands[fill_index + 2]
                    else {
                        unreachable!()
                    };
                    let DrawingCommand::DrawPath { style, .. } =
                        &mut edited.public.commands[fill_index]
                    else {
                        unreachable!()
                    };
                    style.stroke = stroke.stroke.clone();
                    style.stroke.as_mut().unwrap().width = 3.0;
                }
                "removed-stroke" => {
                    edited.public.commands.remove(fill_index + 2);
                }
                "outer-opacity" | "outer-blend" => {
                    edited
                        .public
                        .commands
                        .insert(fill_index + 3, DrawingCommand::Restore);
                    edited.public.commands.insert(
                        fill_index - 2,
                        if edit == "outer-opacity" {
                            DrawingCommand::SetOpacity { opacity: 0.4 }
                        } else {
                            DrawingCommand::SetBlendMode {
                                blend_mode: merman_display_list::BlendMode::Multiply,
                            }
                        },
                    );
                    edited
                        .public
                        .commands
                        .insert(fill_index - 2, DrawingCommand::Save);
                }
                _ => unreachable!(),
            }
            let svg = render(&edited);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let label = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Observe"))
                .unwrap();
            let shapes: Vec<_> = label
                .parent()
                .unwrap()
                .children()
                .filter(|node| node.attribute("class") == Some("cynefinItem"))
                .collect();
            let split = matches!(edit, "added-stroke" | "outer-opacity" | "outer-blend");
            assert_eq!(shapes.len(), if split { 2 } else { 1 }, "{edit}");
            if edit == "opacity" {
                assert_eq!(shapes[0].attribute("fill-opacity"), Some("0.37"));
                continue;
            }
            assert!(
                !svg.contains(".cynefinItem{"),
                "{edit}: fallback must not inherit sibling stroke CSS"
            );
            assert_eq!(shapes[0].attribute("opacity"), Some("0.95"));
            assert_eq!(
                shapes[0].attribute("stroke"),
                if edit == "added-stroke" {
                    Some("#333333")
                } else {
                    Some("none")
                }
            );
            if split {
                assert_eq!(shapes[1].attribute("fill"), Some("none"));
                assert_eq!(
                    shapes[1].attribute("opacity"),
                    if edit == "outer-opacity" {
                        Some("0.4")
                    } else {
                        None
                    }
                );
            }
            if edit == "outer-blend" {
                assert!(
                    shapes
                        .iter()
                        .all(|shape| shape.attribute("style") == Some("mix-blend-mode: multiply;"))
                );
            }
        }
    }

    #[test]
    fn cynefin_marker_projection_follows_public_path_and_paint_edits() {
        use merman_display_list::{
            Color, DrawingCommand, DrawingResource, Paint, PathSegment, Point,
        };
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "cynefin-beta\ncomplex --> clear\nclear --> complicated\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let render = |document: &crate::drawing_list::RenderDocument| {
            crate::svg::render_document_svg(
                document,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
                artifact.metadata.effective_config.as_value(),
                &artifact.session,
            )
            .unwrap()
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("marker"))
                .count(),
            1,
            "identical public marker bodies should share the source definition"
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("marker-end").is_some())
                .count(),
            2
        );
        // New public paths must not inherit registered transitions' shared CSS by ID shape.
        let mut added = document.clone();
        for resource in &mut added.public.resources {
            if let DrawingResource::Path(path) = resource
                && let Some(suffix) = path.id.as_str().strip_prefix("cynefin.transition.0.")
            {
                path.id = merman_display_list::ResourceId::new(format!(
                    "cynefin.transition.added.{suffix}"
                ));
            }
        }
        for command in &mut added.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && let Some(suffix) = path.as_str().strip_prefix("cynefin.transition.0.")
            {
                *path = merman_display_list::ResourceId::new(format!(
                    "cynefin.transition.added.{suffix}"
                ));
                if let Some(stroke) = &mut style.stroke {
                    stroke.paint = Paint::solid(Color::rgba(11, 22, 33, 128));
                }
                if style.fill.is_some() {
                    style.fill = Some(Paint::solid(Color::rgba(11, 22, 33, 128)));
                }
            }
        }
        let added_svg = render(&added);
        let added_xml = roxmltree::Document::parse(&added_svg).unwrap();
        assert_eq!(
            added_xml
                .descendants()
                .filter(|node| node.attribute("marker-end").is_some())
                .count(),
            1,
            "unregistered paths must remain independent of shared marker CSS"
        );
        for attribute in ["fill", "stroke"] {
            let path = added_xml
                .descendants()
                .find(|node| node.attribute(attribute) == Some("#0b1621"))
                .unwrap();
            assert!(path.attribute("class").is_none());
        }
        let marker_id = "cynefin.transition.0.arrowhead";
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == marker_id
            {
                path.segments[1] = PathSegment::LineTo {
                    to: Point::new(8.0, 5.0),
                };
            }
        }
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == marker_id
            {
                style.fill = Some(Paint::solid(Color::rgba(11, 22, 33, 128)));
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("marker"))
                .count(),
            2
        );
        let edited = xml
            .descendants()
            .find(|node| node.attribute("fill") == Some("#0b1621"))
            .unwrap();
        assert!(edited.parent().unwrap().has_tag_name("marker"));
        assert!(edited.attribute("d").unwrap().contains("L 8 5"));
        assert!(
            (edited
                .attribute("fill-opacity")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                - 128.0 / 255.0)
                .abs()
                < 0.001
        );

        // An edited marker outside the original viewBox must stay an ordinary public path,
        // otherwise the SVG marker viewport would silently clip geometry.
        for resource in &mut document.public.resources {
            if let DrawingResource::Path(path) = resource
                && path.id.as_str() == marker_id
            {
                path.segments[1] = PathSegment::LineTo {
                    to: Point::new(12.0, 5.0),
                };
            }
        }
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let edited = xml
            .descendants()
            .find(|node| node.attribute("fill") == Some("#0b1621"))
            .unwrap();
        assert!(!edited.parent().unwrap().has_tag_name("marker"));
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("marker-end").is_some())
                .count(),
            1
        );

        document.public.commands.retain(|command| {
            !matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == marker_id)
        });
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("fill") == Some("#0b1621"))
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("marker"))
                .count(),
            1
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("marker-end").is_some())
                .count(),
            1
        );

        let edge_index = document.public.commands.iter().position(|command| matches!(command,
            DrawingCommand::DrawPath { path, .. } if path.as_str() == "cynefin.transition.1.line"
        )).unwrap();
        document
            .public
            .commands
            .insert(edge_index, DrawingCommand::SetOpacity { opacity: 0.5 });
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("marker-end").is_some())
        );
        let arrow = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("cynefinArrowHead"))
            .unwrap();
        assert_eq!(arrow.attribute("opacity"), Some("0.5"));
        document.public.commands[edge_index] = DrawingCommand::SetBlendMode {
            blend_mode: merman_display_list::BlendMode::Multiply,
        };
        let svg = render(&document);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("marker-end").is_some())
        );
        let arrow = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("cynefinArrowHead"))
            .unwrap();
        assert!(
            arrow
                .attribute("style")
                .unwrap()
                .contains("mix-blend-mode: multiply")
        );
    }

    #[test]
    fn pie_svg_uses_public_paint_text_and_background_not_external_config() {
        use merman_display_list::{Color, DrawingCommand, Paint};
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "pie\n  title Releases\n  \"A\" : 3\n  \"B\" : 1\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .unwrap();
        let options = SvgRenderOptions {
            diagram_id: Some("pie-public-source".to_string()),
            ..Default::default()
        };
        let debug = drawing_list_svg_diagnostics();
        let render = |document: &crate::drawing_list::RenderDocument, config: &Value| {
            crate::svg::render_document_svg(document, &options, &debug, config, &artifact.session)
                .unwrap()
        };
        let config = artifact.metadata.effective_config.as_value();
        let conflicting = json!({"themeCSS": ".pieCircle {opacity:0;}", "themeVariables": {"pieTitleTextColor": "red", "fontFamily": "monospace"}});
        let baseline = render(&document, config);
        assert_eq!(baseline, render(&document, &conflicting));
        let mut changed = 0;
        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } if run.text == "Releases" => {
                    run.text = "Public title".to_string();
                    run.style.fill = Paint::solid(Color::rgba(18, 52, 86, 128));
                    changed += 1;
                }
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "pie.slice.0.shape"
                        || path.as_str() == "pie.background" =>
                {
                    style.fill = Some(Paint::solid(Color::rgba(171, 205, 239, 255)));
                    changed += 1;
                }
                _ => {}
            }
        }
        assert_eq!(changed, 3);
        let edited = render(&document, config);
        assert_ne!(baseline, edited);
        assert_eq!(edited, render(&document, &conflicting));
        assert!(edited.contains("Public title"));
        assert!(edited.contains("fill:#123456;fill-opacity:0.5019607843137255"));
        let xml = roxmltree::Document::parse(&edited).unwrap();
        assert!(
            !xml.root_element()
                .attribute("style")
                .unwrap_or_default()
                .contains("background-color")
        );
        let background = xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some("pie.background"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#abcdef"));
        let slice = xml
            .descendants()
            .find(|node| node.attribute("class") == Some("pieCircle"))
            .unwrap();
        assert_eq!(slice.attribute("fill"), Some("#abcdef"));
        assert!(
            slice
                .attribute("style")
                .unwrap()
                .contains("fill:rgba(171,205,239,1);")
        );

        // A legal command edit must not depend on the builder's optional local save scopes.
        let plot = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "pie.plot")
            })
            .unwrap();
        assert!(matches!(
            document.public.commands[plot - 1],
            DrawingCommand::Save
        ));
        document.public.commands[plot - 1] = DrawingCommand::ConcatTransform {
            transform: merman_display_list::Transform {
                e: 5.0,
                f: 7.0,
                ..merman_display_list::Transform::IDENTITY
            },
        };
        let title = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "pie.title")
            })
            .unwrap();
        assert!(matches!(
            document.public.commands[title - 1],
            DrawingCommand::Restore
        ));
        document.public.commands.remove(title - 1);
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawText { run } = command
                && run.text == "Public title"
            {
                run.style.stroke = Some(merman_display_list::StrokeStyle {
                    paint: Paint::solid(Color::rgba(255, 0, 0, 255)),
                    width: 2.0,
                    dash_array: Vec::new(),
                    dash_offset: 0.0,
                    line_cap: merman_display_list::LineCap::Butt,
                    line_join: merman_display_list::LineJoin::Miter,
                    miter_limit: 4.0,
                });
                run.style.paint_order = merman_display_list::TextPaintOrder::StrokeThenFill;
            }
        }
        let title_text = document
            .public
            .commands
            .iter()
            .position(|command| {
                matches!(command,
            DrawingCommand::DrawText { run } if run.text == "Public title")
            })
            .unwrap();
        document.public.commands.insert(
            title_text,
            DrawingCommand::SetBlendMode {
                blend_mode: merman_display_list::BlendMode::Multiply,
            },
        );
        let transformed = render(&document, config);
        let xml = roxmltree::Document::parse(&transformed)
            .expect("blend and text anchor must not duplicate style attributes");
        let title = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("text") && node.attribute("class") == Some("pieTitleText")
            })
            .unwrap();
        assert_eq!(
            title.attribute("transform"),
            Some("translate(5, 7) rotate(0)")
        );
        assert_eq!(title.attribute("stroke"), Some("#ff0000"));
        assert_eq!(title.attribute("stroke-width"), Some("2"));
        assert_eq!(title.attribute("paint-order"), Some("stroke fill"));
        assert!(
            title
                .attribute("style")
                .unwrap()
                .contains("mix-blend-mode: multiply;")
        );
        for command in &mut document.public.commands {
            if let DrawingCommand::DrawText { run } = command
                && run.text == "Public title"
            {
                run.style.stroke = None;
            }
        }
        let blend_only = render(&document, config);
        roxmltree::Document::parse(&blend_only)
            .expect("compact-eligible text with blend must remain valid XML");
    }

    #[test]
    fn packet_svg_uses_public_geometry_and_styles_not_external_config() {
        use merman_display_list::{
            Color, DrawingCommand, DrawingResource, Paint, PathSegment, Point,
        };

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "packet\n0-7: \"First\"\n8-15: \"Second\"\n",
                ParseOptions::strict(),
            )
            .expect("parse packet")
            .expect("detect packet");
        let artifact =
            prepare(parsed, &LayoutOptions::default(), session()).expect("prepare packet artifact");
        let mut document = crate::drawing_list::build_for_family(
            &artifact.family,
            &artifact.metadata,
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits::default(),
            &artifact.session,
        )
        .expect("build canonical packet document");
        let options = SvgRenderOptions {
            diagram_id: Some("packet-public-source".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = drawing_list_svg_diagnostics();
        let render = |document: &crate::drawing_list::RenderDocument, config: &Value| {
            crate::svg::render_document_svg(document, &options, &debug, config, &artifact.session)
                .expect("serialize canonical packet document")
        };
        let config = artifact.metadata.effective_config.as_value();
        let mut conflicting_config = config.clone();
        conflicting_config["packet"] = json!({
            "labelColor": "#ff0000",
            "labelFontSize": 99,
            "blockFillColor": "#00ff00",
            "blockStrokeColor": "#0000ff",
            "blockStrokeWidth": 17
        });
        conflicting_config["themeCSS"] =
            json!(".packetLabel { fill: url(javascript:alert(1)); font-size: 123px; }");
        let baseline = render(&document, config);
        assert_eq!(baseline, render(&document, &conflicting_config));
        // Uniform labels use upstream's class rule; after changing one label the unchanged
        // sibling must receive the same effective style through element attributes instead.
        assert!(baseline.contains(".packetLabel{fill:#000000;font-size:12px;}"));

        for command in &mut document.public.commands {
            if let DrawingCommand::DrawText { run } = command
                && run.text.is_empty()
            {
                run.text = "Public title\nSecond line".to_string();
            }
        }
        let multiline = render(&document, &conflicting_config);
        let multiline_xml = roxmltree::Document::parse(&multiline).unwrap();
        let title = multiline_xml
            .descendants()
            .find(|node| node.attribute("class") == Some("packetTitle"))
            .unwrap();
        let second_line = title
            .children()
            .find(|node| node.has_tag_name("tspan"))
            .expect("public multiline text must retain its second line");
        assert_eq!(second_line.text(), Some("Second line"));
        assert_eq!(second_line.attribute("dy"), Some("14"));

        let path = document
            .public
            .resources
            .iter_mut()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "packet.block.0.0.shape" => {
                    Some(path)
                }
                _ => None,
            })
            .expect("first packet rectangle resource");
        path.segments = vec![
            PathSegment::MoveTo {
                to: Point::new(11.0, 13.0),
            },
            PathSegment::LineTo {
                to: Point::new(58.0, 13.0),
            },
            PathSegment::LineTo {
                to: Point::new(58.0, 42.0),
            },
            PathSegment::LineTo {
                to: Point::new(11.0, 42.0),
            },
            PathSegment::Close,
        ];
        let mut changed_label = false;
        let mut changed_block = false;
        for command in &mut document.public.commands {
            match command {
                DrawingCommand::DrawText { run } if run.text == "First" => {
                    run.text = "Public label".to_string();
                    run.style.font_size = 31.0;
                    run.style.fill = Paint::solid(Color::rgba(0x12, 0x34, 0x56, 255));
                    changed_label = true;
                }
                DrawingCommand::DrawText { run } if matches!(run.text.as_str(), "0" | "8") => {
                    run.style.font_size = 17.0;
                }
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "packet.block.0.0.shape" =>
                {
                    style.fill = Some(Paint::solid(Color::rgba(0xab, 0xcd, 0xef, 255)));
                    changed_block = true;
                }
                _ => {}
            }
        }
        assert!(changed_label && changed_block);
        let modified = render(&document, config);
        assert_ne!(baseline, modified);
        assert_eq!(modified, render(&document, &conflicting_config));
        let modified_xml = roxmltree::Document::parse(&modified).expect("valid modified SVG");
        let rectangle = modified_xml
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("data-merman-resource") == Some("packet.block.0.0.shape")
            })
            .expect("modified public rectangle");
        for (attribute, expected) in [
            ("x", "11"),
            ("y", "13"),
            ("width", "47"),
            ("height", "29"),
            ("fill", "#abcdef"),
        ] {
            assert_eq!(
                rectangle.attribute(attribute),
                Some(expected),
                "{attribute}"
            );
        }
        let labels: Vec<_> = modified_xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("text") && node.attribute("class") == Some("packetLabel")
            })
            .collect();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].text(), Some("Public label"));
        assert_eq!(labels[0].attribute("font-size"), Some("31"));
        assert_eq!(labels[0].attribute("fill"), Some("#123456"));
        assert_eq!(labels[1].text(), Some("Second"));
        assert_eq!(labels[1].attribute("font-size"), Some("12"));
        assert_eq!(labels[1].attribute("fill"), Some("#000000"));
        assert!(!modified.contains("#packet-public-source .packetLabel{"));
        assert!(!modified.contains("#packet-public-source .packetBlock{"));
        assert!(!modified.contains("#packet-public-source .packetByte{"));
        for (class, size) in [("packetByte start", "17"), ("packetByte end", "10")] {
            let bytes = modified_xml
                .descendants()
                .filter(|node| node.attribute("class") == Some(class))
                .collect::<Vec<_>>();
            assert_eq!(bytes.len(), 2);
            assert!(
                bytes
                    .iter()
                    .all(|node| node.attribute("font-size") == Some(size))
            );
        }
        assert!(!modified.contains("javascript:"));

        for command in &mut document.public.commands {
            if let DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "packet.background"
            {
                style.fill = Some(Paint::solid(Color::rgba(0x11, 0x22, 0x33, 255)));
            }
        }
        let recolored = render(&document, &conflicting_config);
        let recolored_xml = roxmltree::Document::parse(&recolored).unwrap();
        assert!(
            !recolored_xml
                .root_element()
                .attribute("style")
                .unwrap_or_default()
                .contains("background-color: white")
        );
        let background = recolored_xml
            .descendants()
            .find(|node| node.attribute("data-merman-resource") == Some("packet.background"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#112233"));
    }

    #[test]
    fn family_coverage_inventory_matches_the_runtime_svg_gate() {
        #[derive(Deserialize)]
        struct CoverageMatrix {
            families: Vec<CoverageRow>,
        }

        #[derive(Deserialize)]
        struct CoverageRow {
            id: String,
            svg_serializer: String,
        }

        let matrix: CoverageMatrix = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/drawing-list/v1/family-coverage.json"
        )))
        .expect("family coverage fixture must be valid JSON");

        for family in RenderFamilyKind::ALL {
            let row = matrix
                .families
                .iter()
                .find(|row| row.id == family.as_str())
                .unwrap_or_else(|| panic!("missing coverage row for {}", family.as_str()));
            assert_eq!(
                row.svg_serializer == "canonical",
                canonical_svg_family_enabled(family),
                "coverage status for {} must match the runtime SVG route gate",
                family.as_str()
            );
        }
    }

    #[test]
    fn final_svg_admission_prefers_emit_cancellation_to_byte_limit() {
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_control(control.clone())
            .unwrap();
        let svg = "<svg/>";

        let limit = session
            .resource_policy()
            .check_svg_bytes(svg, ResourceLimitPhase::SvgOutput)
            .unwrap_err();
        assert_eq!(limit.limit, "max_svg_bytes");

        control.cancel();
        let error = admit_rendered_svg_output(&session, svg).unwrap_err();
        let Error::Cancelled(error) = error else {
            panic!("expected final SVG admission cancellation");
        };
        assert_eq!(error.phase, OperationPhase::Emit);
    }

    #[test]
    fn drawing_list_cancellation_during_family_emit_returns_no_output() {
        for (source, checkpoints) in [
            (
                "flowchart LR\nsubgraph service[Service]\nA[Parse] --> B[Layout]\nend\nB --> C[Emit]\n",
                4,
            ),
            (
                "pie showData title Releases\n\"Stable\" : 3\n\"Alpha\" : 1\n",
                20,
            ),
            (
                "timeline\nsection Planning\nPlan : Build\nShip : Done\n",
                20,
            ),
            (
                "C4Context\nPerson(user, \"User\")\nSystem(api, \"API\", \"Service\")\nRel(user, api, \"uses\")\n",
                20,
            ),
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .expect("parse diagram")
                .expect("detect diagram");
            let control = OperationControl::new();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_control(control.clone())
                .expect("begin render session");
            let artifact = prepare(parsed, &LayoutOptions::default(), session)
                .expect("prepare family artifact");
            if let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family {
                assert_eq!(flowchart.pair().layout().clusters.len(), 1);
                assert!(flowchart.pair().layout().edges.len() >= 2);
            }

            // Arm only after layout. Each input has more emission checkpoints than the armed
            // count, so cancellation stops a partially built candidate, never parsing or layout.
            control.cancel_after_checkpoints(checkpoints);
            let result = artifact.render_drawing_list(
                DrawingListPolicy::AllowRasterSubtree,
                DrawingListLimits::default(),
            );

            let Err(Error::Cancelled(cancelled)) = result else {
                panic!("mid-build cancellation must return a structured error without output");
            };
            assert_eq!(cancelled.phase, OperationPhase::Emit);
            assert_eq!(cancelled.reason, merman_core::CancelReason::Requested);
        }
    }

    #[test]
    fn final_svg_resource_terminal_replays_before_later_cancellation() {
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_control(control.clone())
            .unwrap();
        let svg = "<svg/>";

        let first = admit_rendered_svg_output(&session, svg)
            .expect_err("the formal SVG output admission must reject");
        let Error::ResourceLimitExceeded(first_limit) = first else {
            panic!("expected a resource rejection");
        };
        assert_eq!(first_limit.limit, "max_svg_bytes");
        assert_eq!(first_limit.actual, svg.len());
        assert_eq!(first_limit.max, 1);

        control.cancel();
        let replayed = admit_rendered_svg_output(&session, svg)
            .expect_err("the first SVG output terminal must remain sticky");
        let Error::ResourceLimitExceeded(replayed_limit) = replayed else {
            panic!("expected the resource terminal to replay");
        };
        assert_eq!(replayed_limit, first_limit);
    }

    #[test]
    fn requested_diagram_id_fanout_is_admitted_per_output_occurrence() {
        let diagram_id = "d".repeat(256);
        let baseline = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .expect("parse info diagram")
            .expect("detect info diagram");
        let baseline_session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin baseline render session");
        let baseline_svg = prepare(baseline, &LayoutOptions::default(), baseline_session)
            .expect("prepare baseline info diagram")
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some(diagram_id.clone()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render baseline info diagram");
        let baseline_len = baseline_svg.svg().len();
        assert!(baseline_len > diagram_id.len());
        assert!(baseline_svg.svg().matches(&diagram_id).count() >= 1);

        // Keep this assertion about the output admission contract, not about the number of
        // diagram-id occurrences in one particular SVG DOM shape.  The canonical serializer and
        // the source-backed bridge are both allowed to carry the ID in different structural
        // attributes as their migration progresses.
        let maximum = baseline_len - diagram_id.len();
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, maximum)
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .expect("parse info diagram")
            .expect("detect info diagram");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("begin render session");
        let artifact =
            prepare(parsed, &LayoutOptions::default(), session).expect("prepare info diagram");
        let error = match artifact.render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.clone()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        ) {
            Ok(_) => panic!("the diagram ID fanout must exceed the SVG byte ceiling"),
            Err(error) => error,
        };

        let Error::ResourceLimitExceeded(details) = error else {
            panic!("expected the family fanout preflight to reject");
        };
        assert_eq!(details.limit, "max_svg_bytes");
        assert_eq!(details.max, maximum);
        // Streaming admission reports the first rejected append, not a later full-document
        // size. Both forms must reject before exposing bytes beyond the configured ceiling.
        assert!(details.actual > maximum);
        assert!(details.actual <= baseline_len);
    }

    #[test]
    fn renderer_owned_metadata_scan_observes_the_artifact_session_control() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .expect("parse info diagram")
            .expect("detect info diagram");
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(control.clone())
            .expect("begin render session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare info diagram")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render info diagram");

        control.cancel();
        let error = rendered
            .output_metadata()
            .expect_err("metadata extraction must observe the artifact session control");

        let Error::Cancelled(cancelled) = error else {
            panic!("expected structured postprocess cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
    }

    fn text_measurement_call_count(session: &RenderSession) -> u64 {
        session
            .text_measurement_report()
            .entries()
            .iter()
            .map(crate::environment::TextMeasurementSummary::count)
            .sum()
    }

    #[derive(Debug, Clone, Copy)]
    enum SidecarHostOutcome {
        Success,
        Missing,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SidecarHostRequest {
        ordinal: usize,
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
        result_kind: TextMeasurementResultKind,
        text: String,
        font_size_bits: u64,
        max_width_bits: Option<u64>,
        wrap_mode: WrapMode,
    }

    struct SidecarRecordingHost {
        outcome: SidecarHostOutcome,
        requests: Mutex<Vec<SidecarHostRequest>>,
    }

    impl SidecarRecordingHost {
        fn new(outcome: SidecarHostOutcome) -> Self {
            Self {
                outcome,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn snapshot(&self) -> Vec<SidecarHostRequest> {
            self.requests.lock().expect("host request trace").clone()
        }
    }

    impl HostTextMeasurer for SidecarRecordingHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            let ordinal = {
                let mut requests = self.requests.lock().expect("host request trace");
                let ordinal = requests.len();
                requests.push(SidecarHostRequest {
                    ordinal,
                    phase: request.phase,
                    operation: request.operation,
                    result_kind: request.operation.required_result_kind(),
                    text: request.text.to_string(),
                    font_size_bits: request.style.font_size.to_bits(),
                    max_width_bits: request.max_width.map(f64::to_bits),
                    wrap_mode: request.wrap_mode,
                });
                ordinal
            };

            match self.outcome {
                SidecarHostOutcome::Success => Ok(Some(sidecar_host_measurement(request, ordinal))),
                SidecarHostOutcome::Missing => Ok(None),
            }
        }
    }

    struct CancellingSuccessfulHost {
        calls: AtomicUsize,
        control: OperationControl,
    }

    impl HostTextMeasurer for CancellingSuccessfulHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.control.cancel();
            Ok(Some(sidecar_host_measurement(request, 0)))
        }
    }

    fn sidecar_host_measurement(
        request: HostTextMeasurementRequest<'_>,
        ordinal: usize,
    ) -> HostTextMeasurement {
        let state_delta = (ordinal % 7) as f64 / 32.0;
        let raw_width = request
            .text
            .lines()
            .map(|line| line.chars().count() as f64 * 8.0)
            .fold(0.0_f64, f64::max)
            + state_delta;
        let max_width = request
            .max_width
            .filter(|width| width.is_finite() && *width > 0.0);
        let line_count = max_width
            .map(|width| (raw_width / width).ceil() as usize)
            .unwrap_or(1)
            .max(request.text.lines().count())
            .max(1)
            .min(request.text.len().saturating_add(1));
        let metrics = TextMetrics {
            width: max_width.map_or(raw_width, |width| raw_width.min(width)),
            height: line_count as f64 * 20.0 + state_delta,
            line_count,
        };

        match request.operation.required_result_kind() {
            TextMeasurementResultKind::NormalLineMetrics => {
                HostTextMeasurement::NormalLineMetrics(crate::text::NormalLineMetrics {
                    line_height: 20.0 + state_delta,
                    baseline_offset: 16.0 + state_delta,
                })
            }
            TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
            TextMeasurementResultKind::Length => {
                let length = match request.operation {
                    TextMeasurementOperation::RawBBoxHeight
                    | TextMeasurementOperation::SimpleBBoxHeight
                    | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    TextMeasurementOperation::CreateTextBBoxYOffset
                    | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => raw_width,
                };
                HostTextMeasurement::Length(length)
            }
            TextMeasurementResultKind::HorizontalExtents => {
                HostTextMeasurement::HorizontalExtents {
                    left: raw_width / 2.0,
                    right: raw_width / 2.0,
                }
            }
            TextMeasurementResultKind::WrappedWithRawWidth => {
                HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(raw_width),
                }
            }
        }
    }

    #[test]
    fn family_layout_stops_host_measurement_after_callback_cancellation() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
                ParseOptions::strict(),
            )
            .expect("parse mindmap")
            .expect("detect mindmap");
        let control = OperationControl::new();
        let host = Arc::new(CancellingSuccessfulHost {
            calls: AtomicUsize::new(0),
            control: control.clone(),
        });
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.family-cancelling-host").expect("profile id"),
            "1",
        )
        .expect("profile identity");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                host.clone(),
                TextMeasurementPhase::ALL,
            ))
            .begin_session_with_control(control)
            .expect("begin render session");

        let result = prepare(parsed, &LayoutOptions::default(), session);
        let Err(Error::Cancelled(cancelled)) = result else {
            panic!("family preparation must surface callback cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    }

    fn render_with_sidecar_host_control(
        preparation: FlowchartSvgLabelPreparation,
        outcome: SidecarHostOutcome,
    ) -> (bool, Vec<SidecarHostRequest>, TextMeasurementReport, String) {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph S["service words"]
  A["alpha beta<br/>gamma"]
end
A traced-edge@-->|edge label words| B["delta epsilon"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse flowchart")
            .expect("detect flowchart");
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.flowchart-sidecar-control").expect("profile id"),
            "1",
        )
        .expect("profile identity");
        let host = Arc::new(SidecarRecordingHost::new(outcome));
        let environment = crate::environment::RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                host.clone(),
                TextMeasurementPhase::ALL,
            ));
        let session = environment.begin_session().expect("render session");
        let artifact = prepare_with_render_policy_impl(
            parsed,
            &LayoutOptions::default(),
            session,
            PresentationRenderPolicy::default(),
            preparation,
        )
        .expect("prepare flowchart artifact");
        let indexed_sidecar = match &artifact.family {
            BuiltinFamilyArtifact::Flowchart(flowchart) => {
                flowchart
                    .svg_label_sidecar()
                    .node_owner("A", false)
                    .is_some()
                    && flowchart
                        .svg_label_sidecar()
                        .edge_owner("traced-edge", false)
                        .is_some()
            }
            _ => panic!("expected Flowchart family artifact"),
        };
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render Flowchart SVG");
        let trace = host.snapshot();
        let (svg, _, _, session) = rendered.into_parts();
        (
            indexed_sidecar,
            trace,
            session.text_measurement_report(),
            svg,
        )
    }

    #[cfg(feature = "layout-cytoscape")]
    fn prepare_mindmap_with_host_limit(
        max_layout_work_units: Option<usize>,
    ) -> (Result<FamilyRenderArtifact>, Arc<SidecarRecordingHost>) {
        let source = "mindmap\n  Root\n    First child\n    Second child\n";
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse mindmap")
            .expect("detect mindmap");
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.mindmap-cose-budget").expect("profile id"),
            "1",
        )
        .expect("profile identity");
        let host = Arc::new(SidecarRecordingHost::new(SidecarHostOutcome::Success));
        let mut environment = crate::environment::RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                host.clone(),
                TextMeasurementPhase::ALL,
            ));
        if let Some(limit) = max_layout_work_units {
            let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .expect("layout work limit");
            environment = environment.with_resource_policy(policy);
        }
        let session = environment.begin_session().expect("render session");
        (prepare(parsed, &LayoutOptions::default(), session), host)
    }

    #[test]
    fn public_flowchart_preparation_enables_prepared_svg_label_reuse() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|control label| B
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse flowchart")
            .expect("detect flowchart");
        let artifact = prepare_with_render_policy(
            parsed,
            &LayoutOptions::default(),
            session(),
            PresentationRenderPolicy::default(),
        )
        .expect("prepare public flowchart artifact");
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };

        assert!(
            flowchart
                .svg_label_sidecar()
                .node_owner("A", false)
                .is_some(),
            "the public Flowchart preparation path must build the label sidecar"
        );
    }

    #[test]
    fn routed_host_and_fallback_traces_match_a_no_sidecar_family_control() {
        for outcome in [SidecarHostOutcome::Success, SidecarHostOutcome::Missing] {
            let (control_indexed, control_trace, control_report, control_svg) =
                render_with_sidecar_host_control(FlowchartSvgLabelPreparation(false), outcome);
            let (sidecar_indexed, sidecar_trace, sidecar_report, sidecar_svg) =
                render_with_sidecar_host_control(FlowchartSvgLabelPreparation(true), outcome);

            assert!(
                !control_indexed,
                "the control must bypass sidecar preparation"
            );
            assert!(
                sidecar_indexed,
                "the candidate must prepare semantic owners"
            );
            assert!(!control_trace.is_empty());
            assert_eq!(sidecar_trace, control_trace);
            assert_eq!(sidecar_report, control_report);
            assert_eq!(sidecar_svg, control_svg);
        }
    }

    #[test]
    fn prepared_self_loop_edge_label_keeps_its_semantic_owner_through_family_dispatch() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A ordinary-edge@-->|ordinary owner sentinel| B
A self-loop-edge@-->|self loop semantic owner keeps wrapped label rows through the logical render id alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron| A
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse flowchart")
            .expect("detect flowchart");
        let artifact = prepare_with_render_policy_impl(
            parsed,
            &LayoutOptions::default(),
            session(),
            PresentationRenderPolicy::default(),
            FlowchartSvgLabelPreparation(true),
        )
        .expect("prepare flowchart family artifact");

        let rendered_svg = {
            let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
                panic!("expected Flowchart family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                flowchart.pair().semantic(),
                flowchart.render_context(),
            );
            let edge = model.edges.get(1).expect("self-loop edge");
            assert_eq!(edge.id, "self-loop-edge");
            let label = model
                .edge_label_for_render(edge)
                .expect("self-loop edge label");
            let owner = flowchart
                .svg_label_sidecar()
                .edge_owner(edge.id.as_str(), false)
                .expect("semantic self-loop owner");
            assert_eq!(owner, crate::flowchart::FlowchartSvgLabelOwner::Edge(1));
            assert_eq!(
                flowchart
                    .svg_label_sidecar()
                    .edge_owner("A-cyclic-special-mid", false),
                None
            );
            assert!(
                flowchart
                    .pair()
                    .layout()
                    .edges
                    .iter()
                    .any(|edge| edge.id == "self-loop-edge")
            );

            let config = crate::flowchart::FlowchartConfigView::new(
                artifact.metadata.effective_config.as_value(),
            );
            let font_family = config.font_family();
            let render_style = config.render_text_style(&font_family, config.render_font_size());
            let edge_width = config.layout_settings().edge_label_wrapping_width;
            let render_measurer = artifact
                .session
                .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
            let calls_before = text_measurement_call_count(&artifact.session);
            let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                Some(flowchart.svg_label_sidecar()),
                Some(owner),
                label,
                &render_measurer,
                &render_style,
                Some(edge_width),
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            assert!(matches!(
                &plan,
                crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
            ));
            let wrapped = plan.wrapped_lines();
            assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
            assert!(wrapped.len() >= 2, "{wrapped:?}");
            assert_eq!(
                text_measurement_call_count(&artifact.session),
                calls_before,
                "a prepared self-loop label must not invoke the SVG measurer again"
            );
            drop(wrapped);
            drop(plan);

            let hits_before_render = flowchart.svg_label_sidecar().prepared_hit_count(owner);
            let (svg, _, _) = render_family_artifact_svg(
                &artifact,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
            )
            .expect("render self-loop SVG");
            assert!(
                flowchart.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
                "the real Flowchart SVG renderer must consume the prepared self-loop label"
            );
            svg
        };

        assert!(!rendered_svg.contains("cyclic-special"), "{}", rendered_svg);
        let document = roxmltree::Document::parse(&rendered_svg).expect("valid self-loop SVG");
        let logical_path = document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("data-id") == Some("self-loop-edge")
        });
        assert!(logical_path, "{rendered_svg}");
        let label_group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("data-id") == Some("self-loop-edge")
            })
            .expect("logical self-loop label group");
        let text = label_group
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect::<String>();
        assert!(text.contains("self loop semantic owner"), "{text:?}");
        let row_count = label_group
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "text-outer-tspan")
                    })
            })
            .count();
        assert!(row_count >= 2, "rows={row_count}: {rendered_svg}");
    }

    #[test]
    fn prepared_swimlane_edge_label_is_consumed_by_the_real_svg_renderer() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A styled@-->|swimlane semantic owner keeps wrapped label rows through the generated labelRect| B
linkStyle default font-size:24px,font-weight:bold
linkStyle 0 font-size:12px,font-style:italic
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse Swimlane")
            .expect("detect Swimlane");
        let artifact = prepare_with_render_policy_impl(
            parsed,
            &LayoutOptions::default(),
            session(),
            PresentationRenderPolicy::default(),
            FlowchartSvgLabelPreparation(true),
        )
        .expect("prepare Swimlane family artifact");

        let rendered_svg = {
            let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
                panic!("expected Swimlane family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                swimlane.pair().semantic(),
                swimlane.render_context(),
            );
            let edge = model.edges.first().expect("styled Swimlane edge");
            assert_eq!(edge.id, "styled");
            let label = model
                .edge_label_for_render(edge)
                .expect("Swimlane edge label");
            let owner = swimlane
                .svg_label_sidecar()
                .edge_owner(edge.id.as_str(), true)
                .expect("semantic Swimlane edge owner");
            assert_eq!(
                owner,
                crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(0)
            );
            assert_eq!(
                swimlane.pair().layout().edges[0].label_node_id.as_deref(),
                Some("edge-label-A-B-styled")
            );

            let config = crate::flowchart::FlowchartConfigView::new(
                artifact.metadata.effective_config.as_value(),
            );
            let font_family = config.font_family();
            let base_style = config.render_text_style(&font_family, config.render_font_size());
            let default_edge_styles = model
                .edge_defaults
                .as_ref()
                .map_or(&[][..], |defaults| defaults.style.as_slice());
            let label_style = crate::flowchart::flowchart_swimlane_label_rect_text_style(
                &base_style,
                default_edge_styles,
                &edge.style,
            );
            let render_measurer = artifact
                .session
                .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
            let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                Some(swimlane.svg_label_sidecar()),
                Some(owner),
                label,
                &render_measurer,
                label_style.as_ref(),
                Some(config.render_wrapping_width()),
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            assert!(matches!(
                &plan,
                crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
            ));
            let wrapped = plan.wrapped_lines();
            assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
            assert!(wrapped.len() >= 2, "{wrapped:?}");
            drop(wrapped);
            drop(plan);

            let hits_before_render = swimlane.svg_label_sidecar().prepared_hit_count(owner);
            let (svg, _, _) = render_family_artifact_svg(
                &artifact,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
            )
            .expect("render Swimlane SVG");
            assert!(
                swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
                "the real Swimlane SVG renderer must consume the prepared labelRect"
            );
            svg
        };

        let document = roxmltree::Document::parse(&rendered_svg).expect("valid Swimlane SVG");
        let label_group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("id") == Some("edge-label-A-B-styled")
            })
            .expect("generated labelRect group");
        let visible = label_group
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .flat_map(str::chars)
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        assert_eq!(
            visible,
            "swimlanesemanticownerkeepswrappedlabelrowsthroughthegeneratedlabelRect"
        );
    }

    fn prepare_with_model_item_limit(
        source: &str,
        max_model_items: usize,
    ) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(
                        crate::resources::ResourceLimitId::MaxModelItems,
                        max_model_items,
                    )
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn prepare_with_layout_work_limit(
        source: &str,
        max_layout_work_units: usize,
    ) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(
                        crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                        max_layout_work_units,
                    )
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn prepare_with_unbounded_layout_work(source: &str) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    #[test]
    fn resolved_source_metadata_replays_with_exact_reported_work_budget() {
        for source in [
            "packet\naccTitle: abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ #quot;\n0-7: \"Byte\"\n",
            "packet\naccTitle: abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ literal\n0-7: \"Byte\"\n",
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection #quot;<br/> Alpha  Beta\nTask :a, 2026-01-01, 1d\n",
            "journey\ntitle Alpha    #quot;Beta#quot;\nsection Checkout\nPay: 5: Alice\n",
            "journey\ntitle Alpha    #quot;Beta#quot;\naccTitle: #quot;Accessible#quot;\naccDescr: A #amp; B\nsection Checkout\nPay: 5: Alice\n",
        ] {
            let render = |artifact: FamilyRenderArtifact| {
                artifact.render_drawing_list(
                    DrawingListPolicy::VectorOnly,
                    DrawingListLimits::default(),
                )
            };
            let unbounded = render(prepare_with_unbounded_layout_work(source).unwrap()).unwrap();
            let exact = unbounded.session.report().layout_work_units();
            let bounded = render(prepare_with_layout_work_limit(source, exact).unwrap())
                .expect("reported work must admit a replay of the same source");
            assert_eq!(bounded.session.report().layout_work_units(), exact);
            assert_eq!(bounded.json, unbounded.json);
            assert!(render(prepare_with_layout_work_limit(source, exact - 1).unwrap()).is_err());
        }
    }

    fn assert_model_item_limit(error: Error, actual: usize, max: usize) {
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_model_items resource limit error")
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
        assert_eq!(limit.actual, actual);
        assert_eq!(limit.max, max);
    }

    #[test]
    fn session_report_accounts_for_every_metered_layout_family() {
        let cases = vec![
            (
                "classDiagram\nclass A\nclass B\nA --> B\n",
                RenderFamilyKind::Class,
            ),
            (
                "stateDiagram-v2\n[*] --> Idle\nIdle --> Active\n",
                RenderFamilyKind::State,
            ),
            (
                "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
                RenderFamilyKind::Er,
            ),
            (
                "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
                RenderFamilyKind::Mindmap,
            ),
            (
                "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: hello\n",
                RenderFamilyKind::Sequence,
            ),
            (
                "kanban\n  todo[Todo]\n    task[Task]\n",
                RenderFamilyKind::Kanban,
            ),
            (
                "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Login\n  risk: high\n}\n",
                RenderFamilyKind::Requirement,
            ),
            ("sankey-beta\nA,B,10\n", RenderFamilyKind::Sankey),
            (
                "radar-beta\naxis A,B,C\ncurve score{1,2,3}\n",
                RenderFamilyKind::Radar,
            ),
            (
                "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
                RenderFamilyKind::Venn,
            ),
        ];

        #[cfg(feature = "layout-cytoscape")]
        let cases = {
            let mut cases = cases;
            cases.push((
                "mindmap\n  Root\n    First child\n    Second child\n",
                RenderFamilyKind::Mindmap,
            ));
            cases
        };

        for (source, expected_family) in cases {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::default())
                .unwrap()
                .expect("the layout-work fixture should produce a render model");
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

            assert_eq!(artifact.family_kind(), expected_family);
            assert!(
                artifact.session.report().layout_work_units() > 0,
                "{expected_family} must contribute layout work to the session report"
            );
        }
    }

    #[test]
    fn dagre_family_work_budgets_are_exact_and_preserve_layout_output() {
        let cases = [
            (
                "classDiagram\nnamespace Outer {\n  class A\n  class B\n}\nA --> B\n",
                RenderFamilyKind::Class,
            ),
            (
                "stateDiagram-v2\nstate Parent {\n  [*] --> Idle\n  Idle --> Active\n}\nParent --> Outside\n",
                RenderFamilyKind::State,
            ),
            (
                "erDiagram\nNODE {\n  string id\n}\nNODE ||--o{ NODE : leads\n",
                RenderFamilyKind::Er,
            ),
        ];

        for (source, expected_family) in cases {
            let unbounded = prepare_with_unbounded_layout_work(source).unwrap();
            assert_eq!(unbounded.family_kind(), expected_family);
            let exact = unbounded.session.report().layout_work_units();
            assert!(exact > 0, "{expected_family} must report layout work");
            let expected_layout = unbounded.layout_json().unwrap();

            let bounded = prepare_with_layout_work_limit(source, exact).unwrap();
            assert_eq!(bounded.session.report().layout_work_units(), exact);
            assert_eq!(bounded.layout_json().unwrap(), expected_layout);

            let error = match prepare_with_layout_work_limit(source, exact - 1) {
                Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
                Err(error) => error,
            };
            let Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected {expected_family} layout work rejection")
            };
            assert_eq!(limit.limit, "max_layout_work_units");
            assert_eq!(limit.max, exact - 1);
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn default_mindmap_cose_reports_kernel_work_and_has_an_exact_resource_boundary() {
        let (unbounded_result, unbounded_host) = prepare_mindmap_with_host_limit(None);
        let unbounded = unbounded_result.expect("unbounded COSE mindmap");
        let exact = unbounded.session.report().layout_work_units();
        assert!(
            exact > 78,
            "kernel work must exceed the 3-node adapter estimate"
        );
        let unbounded_layout = unbounded.layout_json().expect("unbounded layout json");
        let unbounded_trace = unbounded_host.snapshot();
        assert!(!unbounded_trace.is_empty());

        let (exact_result, exact_host) = prepare_mindmap_with_host_limit(Some(exact));
        let exact_artifact = exact_result.expect("exact COSE budget");
        assert_eq!(exact_artifact.session.report().layout_work_units(), exact);
        assert_eq!(
            exact_artifact.layout_json().expect("exact layout json"),
            unbounded_layout
        );
        assert_eq!(exact_host.snapshot(), unbounded_trace);

        let (short_result, _short_host) = prepare_mindmap_with_host_limit(Some(exact - 1));
        let error = match short_result {
            Ok(_) => panic!("exact minus one must reject COSE work"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_layout_work_units rejection")
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, exact - 1);

        let (early_result, early_host) = prepare_mindmap_with_host_limit(Some(1));
        assert!(matches!(early_result, Err(Error::ResourceLimitExceeded(_))));
        assert!(
            early_host.snapshot().is_empty(),
            "adapter admission must reject before the first host measurement"
        );
    }

    #[test]
    fn requirement_layout_projection_excludes_operation_prepared_labels() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"requirementDiagram
requirement req1 {
  id: 1
  text: User logs in
  risk: high
}
element system {
  type: service
}
system - satisfies -> req1
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Requirement source should produce a render model");
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let projection = artifact.layout_json().unwrap();
        let layout = &projection["layout"]["RequirementDiagram"];
        let fields = layout
            .as_object()
            .expect("Requirement layout projection should remain an object");

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Requirement);
        assert!(fields.contains_key("nodes"));
        assert!(fields.contains_key("edges"));
        assert!(fields.contains_key("bounds"));
        assert!(!fields.contains_key("labels"));
        assert!(!layout.to_string().contains("display_text"));
        let serialized_projection = projection.to_string();
        assert!(!serialized_projection.contains("max_width_px"));
        assert!(!serialized_projection.contains("keep_centered"));
        assert!(!serialized_projection.contains("divider_y_offset"));
        assert!(
            serde_json::from_value::<RequirementDiagramLayout>(layout.clone()).is_ok(),
            "prepared labels must not alter the public Requirement layout schema"
        );
    }

    #[test]
    fn dagre_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "flowchart TD\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("flowchart above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn swimlane_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "swimlane-beta LR\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Swimlane);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("swimlane above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn swimlane_rejects_pairwise_routing_work_before_layout() {
        let source = "swimlane-beta LR\nA --> B\nB --> C";
        let artifact = prepare_with_layout_work_limit(source, 1_000).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Swimlane);

        let error = match prepare_with_layout_work_limit(source, 1) {
            Err(error) => error,
            Ok(_) => panic!("swimlane above the layout work limit unexpectedly rendered"),
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_layout_work_units resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_layout_work_units");
        assert!(limit.actual > limit.max);
        assert_eq!(limit.max, 1);
    }

    #[test]
    fn mindmap_node_limit_is_checked_before_layout_allocation_or_backend_dispatch() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "mindmap\n  Root\n    First child\n    Second child\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxModelItems, 4)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("mindmap above the node limit unexpectedly reached layout"),
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_model_items resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
        assert_eq!(limit.actual, 5);
        assert_eq!(limit.max, 4);
    }

    #[test]
    fn flowchart_math_capability_uses_parser_owned_render_spelling() {
        for source in [
            "flowchart TD\nA[\"#36;#36;node#36;#36;\"]\n",
            "flowchart TD\nA -->|#36;#36;edge#36;#36;| B\n",
            "flowchart TD\nsubgraph S[\"#36;#36;group#36;#36;\"]\nA\nend\n",
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .without_math_renderer()
                .begin_session()
                .unwrap();

            let plan = plan_render(&parsed, &session).unwrap();
            assert!(
                !plan
                    .required_capabilities()
                    .contains(&RenderCapability::Math),
                "encoded dollar entities remain ordinary createText input: {source}"
            );
            prepare(parsed, &LayoutOptions::default(), session)
                .expect("encoded dollar entities must not require a Math renderer");
        }

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA[\"$$x$$\"]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();
        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
    }

    #[test]
    fn mindmap_math_label_requires_the_math_capability() {
        let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
        assert!(!plan.is_ready());

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("mindmap math label unexpectedly rendered without a math backend"),
        };
        assert!(matches!(
            error,
            Error::MissingCapability {
                capability: RenderCapability::Math,
                ref diagram_type,
            } if diagram_type == "mindmap"
        ));
    }

    #[derive(Debug)]
    struct MindmapMathRenderer;

    impl crate::math::MathRenderer for MindmapMathRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            text.contains("$$")
                .then(|| "<strong>rendered-mindmap-math</strong>".to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
            _style: &crate::text::TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: crate::text::WrapMode,
        ) -> Option<crate::text::TextMetrics> {
            text.contains("$$").then_some(crate::text::TextMetrics {
                width: 96.0,
                height: 24.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn mindmap_math_label_is_consumed_by_the_math_renderer() {
        let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(MindmapMathRenderer))
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert!(plan.missing_capabilities().is_empty());
        assert!(plan.is_ready());

        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(rendered.required_capabilities(), &[RenderCapability::Math]);
        assert!(rendered.svg().contains("rendered-mindmap-math"));
        assert!(!rendered.svg().contains("$$x^2$$"));
    }

    #[test]
    fn class_math_label_requires_the_math_capability() {
        let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
        assert!(!plan.is_ready());

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("Class math label unexpectedly rendered without a math backend"),
        };
        assert!(matches!(
            error,
            Error::MissingCapability {
                capability: RenderCapability::Math,
                ref diagram_type,
            } if diagram_type == "classDiagram"
        ));
    }

    #[derive(Debug)]
    struct ClassMathRenderer;

    impl crate::math::MathRenderer for ClassMathRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            text.contains("$$")
                .then(|| "<div>rendered-class-math</div>".to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
            _style: &crate::text::TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: crate::text::WrapMode,
        ) -> Option<crate::text::TextMetrics> {
            text.contains("$$").then_some(crate::text::TextMetrics {
                width: 96.0,
                height: 24.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn class_math_label_is_consumed_by_the_math_renderer() {
        let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert!(plan.missing_capabilities().is_empty());
        assert!(plan.is_ready());

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("rendered-class-math"));
        assert!(!rendered.svg().contains("$$x^2$$"));
    }

    fn render_class_math(source: &str) -> String {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .svg()
            .to_string()
    }

    #[test]
    fn class_math_label_forces_html_rendering_when_html_labels_are_disabled() {
        let svg = render_class_math(
            r#"---
config:
  htmlLabels: false
---
classDiagram
class Formula["$$x^2$$"]
"#,
        );

        assert!(svg.contains("rendered-class-math"));
        assert!(!svg.contains("$$x^2$$"));
    }

    #[test]
    fn class_relation_terminal_and_note_math_labels_use_the_math_renderer() {
        let svg = render_class_math(
            r#"classDiagram
class Formula
class Result
Formula "$$one$$" --> "$$many$$" Result : $$edge$$
note for Formula "$$note$$"
"#,
        );

        assert_eq!(svg.matches("rendered-class-math").count(), 4);
        assert!(!svg.contains("$$"));
        assert!(!svg.contains("<p><div>"));
    }

    #[test]
    fn class_annotation_and_interface_math_labels_require_and_use_math() {
        for source in [
            r#"classDiagram
class Formula <<$$annotation$$>>
"#,
            r#"classDiagram
class Formula
$$interface$$ ()-- Formula
"#,
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Class source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .without_math_renderer()
                .begin_session()
                .unwrap();

            let plan = plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
            assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);

            let svg = render_class_math(source);
            assert!(svg.contains("rendered-class-math"));
            assert!(!svg.contains("$$"));
            assert!(!svg.contains("<p><div>"));
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "flowchart-elk TD\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("ELK flowchart above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn custom_semantic_json_is_explicitly_non_renderable() {
        let mut engine = Engine::new();
        engine
            .diagram_registry_mut()
            .insert("customDiagram", custom_semantic_parser);
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "customDiagram",
                "customDiagram\npayload",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session()) {
            Err(error) => error,
            Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
        };
        let Error::NonRenderableCustomModel {
            diagram_type,
            model_name,
            provenance,
        } = error
        else {
            panic!("expected explicit custom-model capability error")
        };
        assert_eq!(diagram_type, "customDiagram");
        assert_eq!(model_name, "customDiagram");
        assert_eq!(provenance, CustomJsonProvenance::SemanticRegistryOverlay);
    }

    #[test]
    fn custom_render_overlay_cannot_masquerade_as_a_builtin_family() {
        let mut engine = Engine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", custom_render_parser);
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-v2",
                "flowchart TD\nA --> B",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session()) {
            Err(error) => error,
            Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
        };
        let Error::NonRenderableCustomModel {
            diagram_type,
            model_name,
            provenance,
        } = error
        else {
            panic!("expected explicit custom-model capability error")
        };
        assert_eq!(diagram_type, "flowchart-v2");
        assert_eq!(model_name, "custom-flowchart");
        assert_eq!(provenance, CustomJsonProvenance::RenderRegistryOverlay);
    }

    #[test]
    fn gantt_time_axis_diagnostics_invert_rendered_x_without_exposing_layout() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  gantt:
    useWidth: 130
    leftPadding: 10
    rightPadding: 20
---
gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Gantt);
        let diagnostics = artifact
            .gantt_time_axis_diagnostics()
            .expect("Gantt tasks should expose time-axis diagnostics");
        assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(43.0), Some(0));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(110.0), Some(2));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
        assert_eq!(diagnostics.unix_millis_at_rendered_x(f64::NAN), None);

        artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    }

    #[test]
    fn suppressed_parse_failure_uses_the_typed_error_artifact_and_renderer() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::lenient())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Error);
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("Syntax error in text"));
    }

    #[test]
    fn journey_marker_projection_preserves_public_geometry_and_theme_paint() {
        use merman_display_list::{Color, DrawingCommand, DrawingResource, Paint, PathSegment};

        for (theme_variables, expected_fill, expected_color) in [
            (json!({}), "#333333", Color::rgba(51, 51, 51, 255)),
            (
                json!({"textColor": "#123456"}),
                "#123456",
                Color::rgba(18, 52, 86, 255),
            ),
        ] {
            let parsed = Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "themeVariables": theme_variables
                })))
                .parse_diagram_for_render_model_sync(
                    "journey\nsection Work\nRead: 5: Alice\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
            let mut document = crate::drawing_list::build_for_family(
                &artifact.family,
                &artifact.metadata,
                DrawingListPolicy::VectorOnly,
                DrawingListLimits::default(),
                &artifact.session,
            )
            .unwrap();
            let arrow_style = document
                .public
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawingCommand::DrawPath { path, style }
                        if path.as_str() == "journey.activity.arrowhead" =>
                    {
                        Some(style)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(arrow_style.fill, Some(Paint::solid(expected_color)));
            let encode = |document: &crate::drawing_list::RenderDocument| {
                crate::svg::render_document_svg(
                    document,
                    &SvgRenderOptions::default(),
                    &SvgDebugOptions {
                        include_drawing_list_metadata: true,
                        ..Default::default()
                    },
                    &json!({"themeVariables": {"textColor": "red"}}),
                    &artifact.session,
                )
                .unwrap()
            };
            let svg = encode(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let marker = xml
                .descendants()
                .find(|node| node.has_tag_name("marker"))
                .unwrap();
            let arrow = marker
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(arrow.attribute("d"), Some("M 0,0 V 4 L6,2 Z"));
            assert_eq!(arrow.attribute("fill"), Some(expected_fill));
            assert!(xml.descendants().any(|node| node.has_tag_name("line")
                && node.attribute("marker-end").is_some()));

            let mut translucent = document.clone();
            let activity = translucent.public.commands.iter().position(|command| matches!(
                command, DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "journey.activity"
            )).unwrap();
            translucent
                .public
                .commands
                .insert(activity, DrawingCommand::SetOpacity { opacity: 0.5 });
            let translucent_svg = encode(&translucent);
            let translucent_xml = roxmltree::Document::parse(&translucent_svg).unwrap();
            assert!(!translucent_xml.descendants().any(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some()));
            assert!(
                translucent_xml
                    .descendants()
                    .any(|node| node.has_tag_name("path")
                        && node.attribute("data-merman-resource")
                            == Some("journey.activity.arrowhead")
                        && !node.ancestors().any(|parent| parent.has_tag_name("marker"))
                        && node.attribute("opacity") == Some("0.5"))
            );

            // An independently edited public arrow is no longer equivalent to the source marker.
            let arrow = document
                .public
                .resources
                .iter_mut()
                .find_map(|resource| match resource {
                    DrawingResource::Path(path)
                        if path.id.as_str() == "journey.activity.arrowhead" =>
                    {
                        Some(path)
                    }
                    _ => None,
                })
                .unwrap();
            let PathSegment::MoveTo { to } = &mut arrow.segments[0] else {
                panic!("triangle tip")
            };
            to.x += 7.0;
            let svg = encode(&document);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(!xml.descendants().any(|node| node.has_tag_name("marker")));
            assert!(xml.descendants().any(|node| node.has_tag_name("path")
                && node.attribute("data-merman-resource") == Some("journey.activity.arrowhead")));
        }
    }
}
