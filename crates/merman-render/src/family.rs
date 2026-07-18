use crate::environment::RenderSession;
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{SvgDebugOptions, SvgRenderOptions};
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, Result};
use merman_core::diagrams;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{BuiltinRenderSemantic, ParseMetadata, ParsedDiagramRender, RenderSemanticModel};
use std::fmt;
use std::sync::OnceLock;

/// Stable identity for a built-in typed render family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderFamilyKind {
    Error,
    Mindmap,
    State,
    Sequence,
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Mindmap => "mindmap",
            Self::State => "state",
            Self::Sequence => "sequence",
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
pub(crate) enum BuiltinFamilyArtifact {
    Error(Box<FamilyPair<diagrams::error_diagram::ErrorDiagramRenderModel, ErrorDiagramLayout>>),
    #[cfg(feature = "cytoscape-layout")]
    Mindmap(Box<FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>>),
    State(Box<FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramLayout>>),
    Sequence(
        Box<FamilyPair<diagrams::sequence::SequenceDiagramRenderModel, SequenceDiagramLayout>>,
    ),
    Flowchart(Box<FamilyPair<diagrams::flowchart::FlowchartModel, FlowchartLayout>>),
    Swimlane(Box<FamilyPair<diagrams::flowchart::FlowchartModel, SwimlaneLayout>>),
    #[cfg(feature = "cytoscape-layout")]
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
    Kanban(Box<FamilyPair<diagrams::kanban::KanbanDiagramRenderModel, KanbanDiagramLayout>>),
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
                RequirementDiagramLayout,
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
    #[cfg(feature = "cytoscape-layout")]
    ArchitectureDiagram(&'a ArchitectureDiagramLayout),
    #[cfg(feature = "cytoscape-layout")]
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

#[derive(serde::Serialize)]
struct LayoutArtifactProjection<'a> {
    meta: LayoutMetadataProjection<'a>,
    semantic: &'a serde_json::Value,
    layout: LayoutProjection<'a>,
}

#[derive(serde::Serialize)]
struct LayoutMetadataProjection<'a> {
    diagram_type: &'a str,
    title: Option<&'a str>,
    config: &'a serde_json::Value,
    effective_config: &'a serde_json::Value,
}

impl BuiltinFamilyArtifact {
    pub fn kind(&self) -> RenderFamilyKind {
        match self {
            Self::Error(_) => RenderFamilyKind::Error,
            #[cfg(feature = "cytoscape-layout")]
            Self::Mindmap(_) => RenderFamilyKind::Mindmap,
            Self::State(_) => RenderFamilyKind::State,
            Self::Sequence(_) => RenderFamilyKind::Sequence,
            Self::Flowchart(_) => RenderFamilyKind::Flowchart,
            Self::Swimlane(_) => RenderFamilyKind::Swimlane,
            #[cfg(feature = "cytoscape-layout")]
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
            #[cfg(feature = "cytoscape-layout")]
            Self::Mindmap(pair) => pair.compatibility_json(metadata),
            Self::State(pair) => pair.compatibility_json(metadata),
            Self::Sequence(pair) => pair.compatibility_json(metadata),
            Self::Flowchart(pair) => pair.compatibility_json(metadata),
            Self::Swimlane(pair) => pair.compatibility_json(metadata),
            #[cfg(feature = "cytoscape-layout")]
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
            #[cfg(feature = "cytoscape-layout")]
            Self::Mindmap(pair) => LayoutProjection::MindmapDiagram(pair.layout()),
            Self::State(pair) => LayoutProjection::StateDiagram(pair.layout()),
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout()),
            Self::Flowchart(pair) => LayoutProjection::Flowchart(pair.layout()),
            Self::Swimlane(pair) => LayoutProjection::SwimlaneDiagram(pair.layout()),
            #[cfg(feature = "cytoscape-layout")]
            Self::Architecture(pair) => LayoutProjection::ArchitectureDiagram(pair.layout()),
            Self::Class(pair) => LayoutProjection::ClassDiagram(pair.layout()),
            Self::C4(pair) => LayoutProjection::C4Diagram(pair.layout()),
            Self::Cynefin(pair) => LayoutProjection::CynefinDiagram(pair.layout()),
            Self::Wardley(pair) => LayoutProjection::WardleyDiagram(pair.layout()),
            Self::Railroad(pair) => LayoutProjection::RailroadDiagram(pair.layout()),
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout()),
            Self::Gantt(pair) => LayoutProjection::GanttDiagram(pair.layout()),
            Self::Pie(pair) => LayoutProjection::PieDiagram(pair.layout()),
            Self::Packet(pair) => LayoutProjection::PacketDiagram(pair.layout()),
            Self::Timeline(pair) => LayoutProjection::TimelineDiagram(pair.layout()),
            Self::Journey(pair) => LayoutProjection::JourneyDiagram(pair.layout()),
            Self::Requirement(pair) => LayoutProjection::RequirementDiagram(pair.layout()),
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
    session: RenderSession,
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
    metadata: ParseMetadata,
    session: RenderSession,
}

impl RenderedFamilySvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn into_parts(self) -> (String, ParseMetadata, RenderSession) {
        (self.svg, self.metadata, self.session)
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

    pub fn layout_json(&self) -> Result<serde_json::Value> {
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

        serde_json::to_value(LayoutArtifactProjection {
            meta: LayoutMetadataProjection {
                diagram_type: &self.metadata.diagram_type,
                title: self.metadata.title.as_deref(),
                config: self.metadata.config.as_value(),
                effective_config: self.metadata.effective_config.as_value(),
            },
            semantic,
            layout: self.family.layout_projection(),
        })
        .map_err(Error::from)
    }

    pub fn render_svg(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
    ) -> Result<RenderedFamilySvg> {
        let Self {
            metadata,
            compatibility_projection: _,
            family,
            session,
        } = self;
        let svg = crate::svg::render_builtin_family_artifact(
            &family,
            &metadata.effective_config,
            &metadata.diagram_type,
            metadata.title.as_deref(),
            &session,
            options,
            debug,
        )?;
        session
            .resource_limits()
            .check_svg_bytes(&svg, ResourceLimitPhase::SvgOutput)?;

        Ok(RenderedFamilySvg {
            svg,
            metadata,
            session,
        })
    }
}

#[inline(never)]
fn prepare_pair<S, L>(
    semantic: S,
    layout: impl FnOnce(&S) -> Result<L>,
) -> Result<Box<FamilyPair<S, L>>> {
    let layout = layout(&semantic)?;
    Ok(Box::new(FamilyPair::new(semantic, layout)))
}

/// Prepares one family-owned typed semantic model for layout and SVG rendering.
///
/// Compatibility JSON is deliberately not accepted by this interface:
///
/// ```compile_fail
/// use merman_render::{LayoutOptions, environment::RenderEnvironment};
///
/// let session = RenderEnvironment::parity().begin_session().unwrap();
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
    let ParsedDiagramRender { meta, model } = parsed;
    let diagram_type = meta.diagram_type.as_str();
    if let RenderSemanticModel::CustomJson(custom) = &model {
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

    if let RenderSemanticModel::Flowchart(model) = &model {
        session
            .resource_limits()
            .check_flowchart_complexity(model)?;
    }

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
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Mindmap(model) => {
            BuiltinFamilyArtifact::Mindmap(prepare_pair(model, |model| {
                crate::mindmap::layout_mindmap_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Mindmap(_) => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::State(model) => {
            BuiltinFamilyArtifact::State(prepare_pair(model, |model| {
                crate::state::layout_state_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Sequence(model) => {
            BuiltinFamilyArtifact::Sequence(prepare_pair(model, |model| {
                crate::sequence::layout_sequence_diagram_typed_with_title(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                    execution.math_renderer(),
                )
            })?)
        }
        RenderSemanticModel::Flowchart(model)
            if meta.effective_config.get_str("layout") == Some("swimlane") =>
        {
            BuiltinFamilyArtifact::Swimlane(prepare_pair(model, |model| {
                crate::swimlane::layout_swimlane_typed(
                    model,
                    &meta.effective_config,
                    execution.text_measurer(),
                    execution.math_renderer(),
                )
            })?)
        }
        RenderSemanticModel::Flowchart(model) => {
            BuiltinFamilyArtifact::Flowchart(prepare_pair(model, |model| {
                crate::layout_flowchart_typed_by_engine(
                    diagram_type,
                    model,
                    &meta.effective_config,
                    &execution,
                )
            })?)
        }
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Architecture(model) => {
            BuiltinFamilyArtifact::Architecture(prepare_pair(model, |model| {
                crate::architecture::layout_architecture_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.ambient_seed(),
                )
            })?)
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Architecture(_) => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::Class(model) => {
            BuiltinFamilyArtifact::Class(prepare_pair(model, |model| {
                crate::layout_class_typed_by_engine(
                    diagram_type,
                    model,
                    &meta.effective_config,
                    &execution,
                )
            })?)
        }
        RenderSemanticModel::C4(model) => {
            BuiltinFamilyArtifact::C4(prepare_pair(model, |model| {
                crate::c4::layout_c4_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                    execution.container_height,
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
                crate::kanban::layout_kanban_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Gantt(model) => {
            BuiltinFamilyArtifact::Gantt(prepare_pair(model, |model| {
                crate::gantt::layout_gantt_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                )
            })?)
        }
        RenderSemanticModel::Pie(model) => {
            BuiltinFamilyArtifact::Pie(prepare_pair(model, |model| {
                crate::pie::layout_pie_diagram_typed(
                    model,
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
                crate::requirement::layout_requirement_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Sankey(model) => {
            BuiltinFamilyArtifact::Sankey(prepare_pair(model, |model| {
                crate::sankey::layout_sankey_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Radar(model) => {
            BuiltinFamilyArtifact::Radar(prepare_pair(model, |model| {
                crate::radar::layout_radar_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
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
            BuiltinFamilyArtifact::Er(prepare_pair(model, |model| {
                crate::er::layout_er_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::QuadrantChart(model) => {
            BuiltinFamilyArtifact::QuadrantChart(prepare_pair(model, |model| {
                crate::quadrantchart::layout_quadrantchart_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::XyChart(model) => {
            BuiltinFamilyArtifact::XyChart(prepare_pair(model, |model| {
                crate::xychart::layout_xychart_diagram_typed(
                    model,
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
                crate::venn::layout_venn_diagram_typed(model, title, effective_config)
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
        session,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::{CustomJsonProvenance, CustomJsonRenderModel, Engine, ParseOptions};
    use serde_json::{Value, json};

    fn custom_semantic_parser(_code: &str, meta: &ParseMetadata) -> merman_core::Result<Value> {
        Ok(json!({ "type": meta.diagram_type, "owner": "semantic" }))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &ParseMetadata,
    ) -> merman_core::Result<CustomJsonRenderModel> {
        Ok(CustomJsonRenderModel::new(
            "custom-flowchart",
            json!({ "owner": "render" }),
        ))
    }

    fn session() -> RenderSession {
        crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap()
    }

    #[cfg(feature = "core-full")]
    fn prepare_with_flowchart_node_limit(
        source: &str,
        max_flowchart_nodes: usize,
    ) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(crate::resources::RenderResourceLimits {
                max_flowchart_nodes: Some(max_flowchart_nodes),
                ..crate::resources::RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    #[cfg(feature = "core-full")]
    fn assert_flowchart_node_limit(error: Error, actual: usize, max: usize) {
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_flowchart_nodes resource limit error")
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_flowchart_nodes");
        assert_eq!(limit.actual, actual);
        assert_eq!(limit.max, max);
    }

    #[cfg(feature = "core-full")]
    #[test]
    fn dagre_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "flowchart TD\nA --> B";
        let artifact = prepare_with_flowchart_node_limit(source, 2).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);

        let error = match prepare_with_flowchart_node_limit(source, 1) {
            Err(error) => error,
            Ok(_) => panic!("flowchart above the node limit unexpectedly rendered"),
        };
        assert_flowchart_node_limit(error, 2, 1);
    }

    #[cfg(feature = "core-full")]
    #[test]
    fn swimlane_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "swimlane-beta LR\nA --> B";
        let artifact = prepare_with_flowchart_node_limit(source, 2).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Swimlane);

        let error = match prepare_with_flowchart_node_limit(source, 1) {
            Err(error) => error,
            Ok(_) => panic!("swimlane above the node limit unexpectedly rendered"),
        };
        assert_flowchart_node_limit(error, 2, 1);
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
}
