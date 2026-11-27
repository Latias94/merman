#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
use serde::Deserialize;
use serde::Serialize;
#[cfg(feature = "diagram-er")]
use std::sync::Arc;

pub use merman_render_core::{
    Bounds, EdgePaintGeometry, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};

#[cfg(feature = "diagram-class")]
pub use merman_render_graph::class::{
    ClassDiagramLayout, ClassNodeLabelPlan, ClassNodeRowMetrics, ClassPreparedHtmlLabel,
    ClassPreparedHtmlNodeLabels, ClassRenderItem, ClassRenderRoot, ClassRenderRootId,
    ClassRenderTree,
};

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

#[cfg(feature = "diagram-block")]
pub use merman_render_structured::block::BlockDiagramLayout;

#[cfg(feature = "diagram-requirement")]
pub use merman_render_graph::requirement::RequirementDiagramLayout;

#[cfg(feature = "diagram-architecture")]
pub use merman_render_graph::architecture::{
    ArchitectureCompoundBounds, ArchitectureCytoscapeServiceBounds,
    ArchitectureCytoscapeServiceLabelMetrics, ArchitectureDiagramLayout,
};

#[cfg(feature = "diagram-mindmap")]
pub use merman_render_graph::mindmap::MindmapDiagramLayout;

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

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-mindmap"
))]
pub use merman_render_core::{
    SwimlaneDirection, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneNodeLayout,
    SwimlaneTitleRect,
};

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

#[cfg(feature = "diagram-er")]
impl ErDiagramLayout {
    pub(crate) fn svg_layout(&self) -> merman_render_graph::er::ErPreparedSvgLayout<'_> {
        use merman_render_graph::er::{ErPreparedSvgLayout, ErSvgLayoutView};

        ErPreparedSvgLayout {
            geometry: ErSvgLayoutView {
                clusters: &self.clusters,
                nodes: &self.nodes,
                edges: &self.edges,
                render_edges: &self.render_edges,
            },
            swimlane: self.swimlane.as_ref(),
            prepared_labels: &self.prepared_labels,
        }
    }
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

#[cfg(feature = "diagram-kanban")]
pub use merman_render_structured::kanban::{
    KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout,
};

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

#[cfg(feature = "diagram-event-modeling")]
pub use merman_render_structured::eventmodeling::{
    EventModelingBoxLayout, EventModelingDiagramLayout, EventModelingRelationLayout,
    EventModelingSwimlaneLayout,
};

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

#[cfg(feature = "diagram-c4")]
pub use merman_render_graph::c4::{
    C4BoundaryLayout, C4DiagramLayout, C4ImageLayout, C4RelLayout, C4ShapeLayout, C4TextBlockLayout,
};

pub use merman_render_core::error_diagram::ErrorDiagramLayout;
