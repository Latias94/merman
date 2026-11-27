#[cfg(feature = "diagram-class")]
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "diagram-class", feature = "diagram-er"))]
use std::sync::Arc;

pub use merman_render_core::{
    Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};

#[derive(Debug)]
#[cfg(feature = "diagram-class")]
pub struct ClassNodeRowMetrics {
    pub members: Vec<crate::text::TextMetrics>,
    pub methods: Vec<crate::text::TextMetrics>,
    pub(crate) _retained_reservation: Option<crate::resources::PreparedTextRetainedReservation>,
}

#[derive(Debug)]
#[cfg(feature = "diagram-class")]
#[non_exhaustive]
pub struct ClassPreparedHtmlLabel {
    pub metrics: crate::text::TextMetrics,
    pub max_width_px: i64,
    pub xhtml: String,
}

#[derive(Debug)]
#[cfg(feature = "diagram-class")]
pub struct ClassPreparedHtmlNodeLabels {
    pub title: ClassPreparedHtmlLabel,
    pub annotation: Option<ClassPreparedHtmlLabel>,
    pub members: Vec<ClassPreparedHtmlLabel>,
    pub methods: Vec<ClassPreparedHtmlLabel>,
    pub(crate) _retained_reservation: Option<crate::resources::PreparedTextRetainedReservation>,
}

/// Prepared labels share ownership through `Arc` without copying their admitted allocations.
#[derive(Debug)]
#[cfg(feature = "diagram-class")]
pub enum ClassNodeLabelPlan {
    RowMetrics(ClassNodeRowMetrics),
    PreparedHtml(ClassPreparedHtmlNodeLabels),
}

/// Edge geometry prepared by the same family-local route and label decisions as SVG.
///
/// `LayoutEdge::points` precedes endpoint clipping. This projection refines those routes without
/// serializing SVG. It reports the `data-points` polyline, before later cluster path clipping,
/// rather than the complete visible curve.
///
/// Coordinates are in the **same space as `LayoutEdge::points`** (layout coordinates), so a
/// consumer can treat this as a refinement of the layout projection rather than a second,
/// unrelated geometry system.
#[derive(Debug, Clone, Serialize)]
pub struct EdgePaintGeometry {
    /// Matches `LayoutEdge::id`.
    pub id: String,
    /// Edge polyline after clipping to the endpoint shapes' ink, and after any swimlane line
    /// hops. This is the exact point list emitted as the `data-points` attribute.
    ///
    /// `None` when the renderer produced no path for this edge (for example an edge with fewer
    /// than two layout points).
    pub points: Option<Vec<LayoutPoint>>,
    /// Center of the edge label after paint-time positioning, in layout coordinates.
    ///
    /// `None` when the edge has no positioned label.
    pub label_position: Option<LayoutPoint>,
}

/// Versioned container for [`EdgePaintGeometry`].
///
/// Versioned independently of the crate, so consumers can pin a payload shape without pinning a
/// merman release — the same contract `svg-plan-json` establishes for the capability plan.
#[derive(Debug, Clone, Serialize)]
pub struct EdgePaintGeometryOutput {
    schema_version: u32,
    diagram_type: String,
    edges: Vec<EdgePaintGeometry>,
}

impl EdgePaintGeometryOutput {
    /// Bumped whenever the payload shape changes incompatibly.
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(diagram_type: String, edges: Vec<EdgePaintGeometry>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            diagram_type,
            edges,
        }
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    pub fn edges(&self) -> &[EdgePaintGeometry] {
        &self.edges
    }

    /// Encodes geometry with exact byte admission and cooperative cancellation.
    ///
    /// Typed geometry construction does not consume this encoded-output budget.
    pub fn write_json_controlled(
        &self,
        writer: &mut impl std::io::Write,
        resources: crate::resources::RenderResourcePolicy,
        control: &merman_model::OperationControl,
    ) -> crate::Result<()> {
        let meter =
            crate::resources::OperationWorkMeter::new_with_control(resources, control.clone());
        let mut output = GeometryJsonWriter {
            writer,
            meter: &meter,
            written: 0,
            terminal: None,
        };
        let result = serde_json::to_writer(&mut output, self);
        if let Some(error) = output.terminal {
            return Err(error.into());
        }
        result?;
        meter.checkpoint(merman_model::OperationPhase::Emit)?;
        Ok(())
    }
}

struct GeometryJsonWriter<'a, W> {
    writer: &'a mut W,
    meter: &'a crate::resources::OperationWorkMeter,
    written: usize,
    terminal: Option<crate::resources::OperationWorkError>,
}

impl<W: std::io::Write> std::io::Write for GeometryJsonWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for chunk in bytes.chunks(1024) {
            if let Err(error) = self
                .meter
                .preflight_geometry_json_bytes(self.written, chunk.len())
            {
                self.terminal = Some(error);
                return Err(std::io::Error::other("edge geometry output terminated"));
            }
            self.writer.write_all(chunk)?;
            self.written += chunk.len();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-block")]
pub struct BlockDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    #[serde(
        default,
        rename = "shapeGeometries",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub shape_geometries: Vec<crate::block::BlockShapeGeometry>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-requirement")]
pub struct RequirementDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-architecture")]
pub struct ArchitectureDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    /// Local Cytoscape child contribution phases for Architecture services.
    ///
    /// These bounds are exposed for source-backed group/content audits. SVG renderers should use
    /// their own phase-specific logic instead of treating this as a generic root-bounds source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cytoscape_service_bounds: Vec<ArchitectureCytoscapeServiceBounds>,
    /// Final FCoSE compound rectangles exposed for source-backed Architecture audits.
    ///
    /// Renderers should not treat these as Mermaid SVG group rects without a phase-specific
    /// source audit; they are layout-engine rects, not browser `node.boundingBox()` values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fcose_compound_bounds: Vec<ArchitectureCompoundBounds>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-architecture")]
pub struct ArchitectureCompoundBounds {
    pub id: String,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-architecture")]
pub struct ArchitectureCytoscapeServiceBounds {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_group: Option<String>,
    pub body_bounds: Bounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_bounds: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_metrics: Option<ArchitectureCytoscapeServiceLabelMetrics>,
    pub union_bounds: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-architecture")]
pub struct ArchitectureCytoscapeServiceLabelMetrics {
    pub text_width: f64,
    pub half_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-mindmap")]
pub struct MindmapDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
    /// Swimlane decoration owned by the selected renderer, not semantic Mindmap nodes.
    #[serde(skip)]
    pub swimlane_lanes: Vec<LayoutCluster>,
}

#[cfg(feature = "diagram-sankey")]
pub use merman_render_charts::sankey::{SankeyDiagramLayout, SankeyLinkLayout, SankeyNodeLayout};

#[cfg(feature = "diagram-radar")]
pub use merman_render_charts::radar::{
    RadarAxisLayout, RadarCurveLayout, RadarDiagramLayout, RadarGraticuleShapeLayout,
    RadarLegendItemLayout,
};

#[cfg(feature = "diagram-treemap")]
pub use merman_render_charts::treemap::{
    TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout,
};

#[cfg(feature = "diagram-venn")]
pub use merman_render_charts::venn::{
    VennAreaLayout, VennCircleLayout, VennDiagramLayout, VennTextAreaLayout,
    VennTextDebugCellLayout, VennTextNodeLayout,
};

#[cfg(feature = "diagram-xychart")]
pub use merman_render_charts::xychart::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartPathData, XyChartRectData, XyChartTextData,
};

#[cfg(feature = "diagram-quadrant-chart")]
pub use merman_render_charts::quadrantchart::{
    QuadrantChartAxisLabelData, QuadrantChartBorderLineData, QuadrantChartDiagramLayout,
    QuadrantChartPointData, QuadrantChartQuadrantData, QuadrantChartTextData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub struct FlowchartLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    /// Render-only occurrence ownership for `edges`.
    ///
    /// The typed sidecar is intentionally crate-private: compatibility JSON exposes Mermaid's raw
    /// edge id, while render internals bind geometry to the semantic occurrence key.
    #[serde(skip)]
    pub(crate) edge_owners: crate::flowchart::FlowchartEdgeOwners,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    /// Mermaid's DOM insertion order for each extracted root graph (`""` = top-level root).
    #[serde(skip)]
    pub dom_node_order_by_root: std::collections::HashMap<String, Vec<String>>,
    /// Retained for callers that inspect the Mermaid 12 DOM mode independently of layout geometry.
    #[serde(skip)]
    pub uses_elk_adapter_dom: bool,
    #[serde(skip)]
    pub(crate) route_contract: crate::graph_layout::RouteContract,
    #[serde(skip)]
    pub(crate) placement: crate::layout_frame::GraphPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub enum SwimlaneDirection {
    #[serde(rename = "TB")]
    Tb,
    #[serde(rename = "LR")]
    Lr,
    #[serde(rename = "BT")]
    Bt,
    #[serde(rename = "RL")]
    Rl,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
impl SwimlaneDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tb => "TB",
            Self::Lr => "LR",
            Self::Bt => "BT",
            Self::Rl => "RL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub struct SwimlaneTitleRect {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub struct SwimlaneNodeLayout {
    pub id: String,
    pub label: String,
    pub label_type: String,
    pub shape: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub top_lane_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label_width: f64,
    pub label_height: f64,
    pub layer: usize,
    pub order: usize,
    pub is_edge_label: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub struct SwimlaneLaneLayout {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    /// Prepared terminal label width shared by Swimlane layout and SVG rendering.
    pub title_label_width: f64,
    /// Prepared terminal label height shared by Swimlane layout and SVG rendering.
    pub title_label_height: f64,
    #[serde(default)]
    pub content_top: Option<f64>,
    #[serde(default)]
    pub title_rect: Option<SwimlaneTitleRect>,
    #[serde(default)]
    pub requested_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub struct SwimlaneEdgeLayout {
    pub id: String,
    pub from: String,
    pub to: String,
    pub points: Vec<LayoutPoint>,
    #[serde(default)]
    pub label_node_id: Option<String>,
    pub reversed_for_layout: bool,
    pub curve: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub struct SwimlaneLayout {
    pub direction: SwimlaneDirection,
    pub nodes: Vec<SwimlaneNodeLayout>,
    pub lanes: Vec<SwimlaneLaneLayout>,
    pub edges: Vec<SwimlaneEdgeLayout>,
    /// Render-only semantic occurrence ownership aligned with `edges`.
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    #[serde(skip)]
    pub(crate) edge_owners: crate::flowchart::FlowchartEdgeOwners,
    #[cfg(all(
        feature = "layout-shoal",
        any(feature = "diagram-flowchart", feature = "diagram-swimlane")
    ))]
    #[serde(skip)]
    pub(crate) shoal: Option<crate::swimlane::SwimlaneShoal>,
    pub bounds: Option<Bounds>,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
impl SwimlaneLayout {
    pub(crate) const fn has_canonical_routes(&self) -> bool {
        #[cfg(all(
            feature = "layout-shoal",
            any(feature = "diagram-flowchart", feature = "diagram-swimlane")
        ))]
        return self.shoal.is_some();
        #[cfg(not(all(
            feature = "layout-shoal",
            any(feature = "diagram-flowchart", feature = "diagram-swimlane")
        )))]
        false
    }
}

#[cfg(feature = "diagram-state")]
pub use merman_render_graph::state::StateDiagramLayout;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-class")]
pub struct ClassDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    /// Whether SVG emission follows Mermaid's registered ELK renderer DOM structure.
    #[serde(skip)]
    pub uses_elk_adapter_dom: bool,
    #[serde(skip)]
    pub class_label_plans_by_id: FxHashMap<String, Arc<ClassNodeLabelPlan>>,
    /// DOM ownership produced by the same recursive Dagre preparation that owns layout.
    ///
    /// Compatibility layout JSON remains flat, while SVG rendering consumes this typed tree so it
    /// never has to reconstruct extracted namespace roots from the semantic model.
    #[serde(skip)]
    pub render_tree: ClassRenderTree,
}

#[derive(Debug, Clone, Default)]
#[cfg(feature = "diagram-class")]
pub struct ClassRenderRoot {
    pub namespace_id: Option<String>,
    pub cluster_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub items: Vec<ClassRenderItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg(feature = "diagram-class")]
pub struct ClassRenderRootId(pub usize);

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-class")]
pub struct ClassRenderTree {
    pub roots: Vec<ClassRenderRoot>,
    pub top: ClassRenderRootId,
}

#[cfg(feature = "diagram-class")]
impl Default for ClassRenderTree {
    fn default() -> Self {
        Self {
            roots: vec![ClassRenderRoot::default()],
            top: ClassRenderRootId(0),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-class")]
pub enum ClassRenderItem {
    Node(String),
    Subgraph(ClassRenderRootId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-er")]
pub struct ErDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    /// SVG-facing edge projection.
    ///
    /// Dagre needs self-loop helper nodes and three internal edge segments to produce stable
    /// ranks. Mermaid's common painter merges those segments back into one visible edge. Keep
    /// that projection beside the canonical layout without serializing it into layout artifacts.
    #[serde(default, skip)]
    pub(crate) render_edges: Vec<LayoutEdge>,
    /// Selected-provider lane geometry retained for SVG without rerunning layout. Ordinary
    /// Dagre/ELK layouts and compatibility JSON leave this renderer-owned projection absent.
    #[serde(skip)]
    pub(crate) swimlane: Option<crate::er::ErSwimlanePaintLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    /// Renderer-owned labels prepared by the layout pass and reused by theme evidence and SVG
    /// emission. This is intentionally excluded from compatibility layout JSON.
    #[serde(skip)]
    pub(crate) prepared_labels: Arc<crate::er::ErPreparedLabels>,
}

#[cfg(feature = "diagram-sequence")]
pub use merman_render_sequence::frame::{SequenceFrameLayout, SequenceFrameSectionLayout};

#[cfg(feature = "diagram-sequence")]
pub use merman_render_sequence::model::{
    SequenceActivationLayout, SequenceDiagramLayout, SequenceParticipantBoxLayout,
    SequenceTextLayout,
};

#[cfg(feature = "diagram-info")]
pub use merman_render_structured::info::InfoDiagramLayout;

#[cfg(feature = "diagram-packet")]
pub use merman_render_structured::packet::{
    PacketBlockLayout, PacketDiagramLayout, PacketWordLayout,
};

#[cfg(feature = "diagram-pie")]
pub use merman_render_charts::pie::{PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};

#[cfg(feature = "diagram-timeline")]
pub use merman_render_structured::timeline::{
    TimelineDiagramLayout, TimelineLineLayout, TimelineNodeLayout, TimelineSectionLayout,
    TimelineTaskLayout,
};

#[cfg(feature = "diagram-journey")]
pub use merman_render_structured::journey::{
    JourneyActorLegendItemLayout, JourneyActorLegendLineLayout, JourneyDiagramLayout,
    JourneyLineLayout, JourneyMouthKind, JourneySectionLayout, JourneyTaskActorCircleLayout,
    JourneyTaskLayout,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-kanban")]
pub struct KanbanSectionLayout {
    pub id: String,
    pub label: String,
    pub index: i64,
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub rect_y: f64,
    pub rect_height: f64,
    pub rx: f64,
    pub ry: f64,
    pub label_width: f64,
    #[serde(skip)]
    pub label_height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-kanban")]
pub struct KanbanItemLayout {
    pub id: String,
    pub label: String,
    pub parent_id: String,
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub rx: f64,
    pub ry: f64,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub assigned: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-kanban")]
pub struct KanbanDiagramLayout {
    pub bounds: Option<Bounds>,
    pub section_width: f64,
    pub padding: f64,
    pub max_label_height: f64,
    pub viewbox_padding: f64,
    #[serde(default = "crate::kanban::default_use_max_width")]
    pub use_max_width: bool,
    pub sections: Vec<KanbanSectionLayout>,
    pub items: Vec<KanbanItemLayout>,
}

#[cfg(feature = "diagram-git-graph")]
pub use merman_render_graph::gitgraph::{
    GitGraphArrowLayout, GitGraphBranchLayout, GitGraphCommitLayout, GitGraphDiagramLayout,
};

#[cfg(feature = "diagram-tree-view")]
pub use merman_render_structured::tree_view::{
    TreeViewDiagramLayout, TreeViewLineLayout, TreeViewNodeLayout,
};

#[cfg(feature = "diagram-ishikawa")]
pub use merman_render_structured::ishikawa::{
    IshikawaBranchLayout, IshikawaCauseLabelGroupLayout, IshikawaDiagramLayout, IshikawaHeadLayout,
    IshikawaLabelBoxLayout, IshikawaLineLayout, IshikawaPairLayout, IshikawaSubGroupLayout,
    IshikawaTextLayout,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-event-modeling")]
pub struct EventModelingSwimlaneLayout {
    pub index: i64,
    pub label: String,
    pub namespace: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-event-modeling")]
pub struct EventModelingBoxLayout {
    pub index: usize,
    pub frame_name: String,
    pub frame_kind: String,
    pub model_entity_type: String,
    pub entity_identifier: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub stroke: String,
    pub swimlane_index: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-event-modeling")]
pub struct EventModelingRelationLayout {
    pub source_frame: String,
    pub target_frame: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-event-modeling")]
pub struct EventModelingDiagramLayout {
    pub bounds: Option<Bounds>,
    pub total_width: f64,
    pub total_height: f64,
    pub viewbox_x: f64,
    pub viewbox_y: f64,
    pub padding: f64,
    pub use_max_width: bool,
    pub swimlanes: Vec<EventModelingSwimlaneLayout>,
    pub boxes: Vec<EventModelingBoxLayout>,
    pub relations: Vec<EventModelingRelationLayout>,
}

#[cfg(feature = "diagram-cynefin")]
pub use merman_render_structured::cynefin::{
    CynefinDiagramLayout, CynefinDomainLayout, CynefinItemLayout, CynefinTransitionLayout,
};

#[cfg(feature = "diagram-railroad")]
pub use merman_render_structured::railroad::{
    RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadRuleLayout,
};

#[cfg(feature = "diagram-gantt")]
pub use merman_render_structured::gantt::{
    GanttAxisTickLayout, GanttDiagramLayout, GanttExcludeRangeLayout, GanttRowLayout,
    GanttSectionTitleLayout, GanttTaskBarLayout, GanttTaskLabelLayout, GanttTaskLayout,
};

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-c4")]
pub(crate) struct C4TextRowLayout {
    /// Source words emitted into one Mermaid `createText` outer tspan.
    ///
    /// Keeping the source words (rather than only a joined string) preserves the same
    /// whitespace/token boundaries for layout and SVG emission.
    pub(crate) words: Vec<String>,
}

#[derive(Debug, Clone)]
#[cfg(feature = "diagram-c4")]
pub(crate) struct C4TextRenderPlan {
    /// Wrapped source-word rows consumed by the C4 SVG emitter.
    pub(crate) rows: Vec<C4TextRowLayout>,
    /// The section bbox x offset relative to the centered outer tspan anchor.
    pub(crate) bbox_x: f64,
    /// The section bbox y offset produced by Mermaid's createText DOM shape.
    pub(crate) bbox_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-c4")]
pub struct C4TextBlockLayout {
    pub text: String,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
    /// Unified C4 labels carry the exact wrapped rows used during measurement. Legacy C4
    /// boundary and relationship labels leave this unset and continue using their historical
    /// text path.
    /// This is an in-memory render artifact; the public layout projection intentionally keeps
    /// the stable historical C4 layout schema and does not expose tokenization details.
    #[serde(skip)]
    pub(crate) render_plan: Option<C4TextRenderPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-c4")]
pub struct C4ImageLayout {
    pub width: f64,
    pub height: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-c4")]
pub struct C4ShapeLayout {
    pub alias: String,
    pub parent_boundary: String,
    pub type_c4_shape: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub margin: f64,
    pub image: C4ImageLayout,
    pub type_block: C4TextBlockLayout,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub ty: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub techn: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-c4")]
pub struct C4BoundaryLayout {
    pub alias: String,
    pub parent_boundary: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub image: C4ImageLayout,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub ty: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg(feature = "diagram-c4")]
pub struct C4RelLayout {
    pub from: String,
    pub to: String,
    pub rel_type: String,
    pub start_point: LayoutPoint,
    pub end_point: LayoutPoint,
    #[serde(default)]
    pub offset_x: Option<merman_model::families::c4::C4RelationOffset>,
    #[serde(default)]
    pub offset_y: Option<merman_model::families::c4::C4RelationOffset>,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub techn: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg(feature = "diagram-c4")]
pub struct C4DiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    #[serde(default = "crate::c4::default_use_max_width")]
    pub use_max_width: bool,
    pub container_width: f64,
    pub container_height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_available_width: Option<f64>,
    pub c4_type: String,
    pub title: Option<String>,
    pub boundaries: Vec<C4BoundaryLayout>,
    pub shapes: Vec<C4ShapeLayout>,
    pub rels: Vec<C4RelLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDiagramLayout {
    pub viewbox_width: f64,
    pub viewbox_height: f64,
    pub max_width_px: f64,
}
