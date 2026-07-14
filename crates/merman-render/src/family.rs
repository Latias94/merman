use crate::environment::RenderSession;
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{SvgDebugOptions, SvgRenderOptions};
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
    Architecture,
    Class,
    C4,
    Cynefin,
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
            Self::Architecture => "architecture",
            Self::Class => "class",
            Self::C4 => "c4",
            Self::Cynefin => "cynefin",
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
    Mindmap(Box<FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>>),
    State(Box<FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramV2Layout>>),
    Sequence(
        Box<FamilyPair<diagrams::sequence::SequenceDiagramRenderModel, SequenceDiagramLayout>>,
    ),
    Flowchart(Box<FamilyPair<diagrams::flowchart::FlowchartV2Model, FlowchartV2Layout>>),
    Architecture(
        Box<
            FamilyPair<
                diagrams::architecture::ArchitectureDiagramRenderModel,
                ArchitectureDiagramLayout,
            >,
        >,
    ),
    Class(Box<FamilyPair<ClassDiagram, ClassDiagramV2Layout>>),
    C4(Box<FamilyPair<diagrams::c4::C4DiagramRenderModel, C4DiagramLayout>>),
    Cynefin(Box<FamilyPair<diagrams::cynefin::CynefinDiagramRenderModel, CynefinDiagramLayout>>),
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
    ArchitectureDiagram(&'a ArchitectureDiagramLayout),
    MindmapDiagram(&'a MindmapDiagramLayout),
    SankeyDiagram(&'a SankeyDiagramLayout),
    RadarDiagram(&'a RadarDiagramLayout),
    TreemapDiagram(&'a TreemapDiagramLayout),
    VennDiagram(&'a VennDiagramLayout),
    XyChartDiagram(&'a XyChartDiagramLayout),
    QuadrantChartDiagram(&'a QuadrantChartDiagramLayout),
    FlowchartV2(&'a FlowchartV2Layout),
    StateDiagramV2(&'a StateDiagramV2Layout),
    ClassDiagramV2(&'a ClassDiagramV2Layout),
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
            Self::Mindmap(_) => RenderFamilyKind::Mindmap,
            Self::State(_) => RenderFamilyKind::State,
            Self::Sequence(_) => RenderFamilyKind::Sequence,
            Self::Flowchart(_) => RenderFamilyKind::Flowchart,
            Self::Architecture(_) => RenderFamilyKind::Architecture,
            Self::Class(_) => RenderFamilyKind::Class,
            Self::C4(_) => RenderFamilyKind::C4,
            Self::Cynefin(_) => RenderFamilyKind::Cynefin,
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
            Self::Flowchart(pair) => pair.compatibility_json(metadata),
            Self::Architecture(pair) => pair.compatibility_json(metadata),
            Self::Class(pair) => pair.compatibility_json(metadata),
            Self::C4(pair) => pair.compatibility_json(metadata),
            Self::Cynefin(pair) => pair.compatibility_json(metadata),
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
            Self::State(pair) => LayoutProjection::StateDiagramV2(pair.layout()),
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout()),
            Self::Flowchart(pair) => LayoutProjection::FlowchartV2(pair.layout()),
            Self::Architecture(pair) => LayoutProjection::ArchitectureDiagram(pair.layout()),
            Self::Class(pair) => LayoutProjection::ClassDiagramV2(pair.layout()),
            Self::C4(pair) => LayoutProjection::C4Diagram(pair.layout()),
            Self::Cynefin(pair) => LayoutProjection::CynefinDiagram(pair.layout()),
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

fn pair<S, L>(semantic: S, layout: L) -> Box<FamilyPair<S, L>> {
    Box::new(FamilyPair::new(semantic, layout))
}

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

    let execution = LayoutExecution::new(options, &session);
    let effective_config = meta.effective_config.as_value();
    let title = meta.title.as_deref();
    let family = match model {
        RenderSemanticModel::Error(model) => {
            let layout = crate::error::layout_error_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Error(pair(model, layout))
        }
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Mindmap(model) => {
            let layout = crate::mindmap::layout_mindmap_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
                execution.use_manatee_layout,
            )?;
            BuiltinFamilyArtifact::Mindmap(pair(model, layout))
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Mindmap(_) => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::State(model) => {
            let layout = crate::state::layout_state_diagram_v2_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::State(pair(model, layout))
        }
        RenderSemanticModel::Sequence(model) => {
            let layout = crate::sequence::layout_sequence_diagram_typed_with_title(
                &model,
                title,
                effective_config,
                execution.text_measurer(),
                execution.math_renderer(),
            )?;
            BuiltinFamilyArtifact::Sequence(pair(model, layout))
        }
        RenderSemanticModel::Flowchart(model) => {
            let layout = crate::layout_flowchart_typed_by_engine(
                diagram_type,
                &model,
                &meta.effective_config,
                &execution,
            )?;
            BuiltinFamilyArtifact::Flowchart(pair(model, layout))
        }
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Architecture(model) => {
            let layout = crate::architecture::layout_architecture_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
                execution.use_manatee_layout,
                execution.ambient_seed(),
            )?;
            BuiltinFamilyArtifact::Architecture(pair(model, layout))
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Architecture(_) => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::Class(model) => {
            let layout = crate::layout_class_typed_by_engine(
                diagram_type,
                &model,
                &meta.effective_config,
                &execution,
            )?;
            BuiltinFamilyArtifact::Class(pair(model, layout))
        }
        RenderSemanticModel::C4(model) => {
            let layout = crate::c4::layout_c4_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
                execution.viewport_width,
                execution.viewport_height,
            )?;
            BuiltinFamilyArtifact::C4(pair(model, layout))
        }
        RenderSemanticModel::Cynefin(model) => {
            let layout = crate::cynefin::layout_cynefin_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Cynefin(pair(model, layout))
        }
        RenderSemanticModel::Railroad(model) => {
            let layout = crate::railroad::layout_railroad_diagram_typed_for_type(
                &model,
                diagram_type,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Railroad(pair(model, layout))
        }
        RenderSemanticModel::Kanban(model) => {
            let layout = crate::kanban::layout_kanban_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Kanban(pair(model, layout))
        }
        RenderSemanticModel::Gantt(model) => {
            let layout = crate::gantt::layout_gantt_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Gantt(pair(model, layout))
        }
        RenderSemanticModel::Pie(model) => {
            let layout = crate::pie::layout_pie_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Pie(pair(model, layout))
        }
        RenderSemanticModel::Packet(model) => {
            let layout = crate::packet::layout_packet_diagram_typed(
                &model,
                title,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Packet(pair(model, layout))
        }
        RenderSemanticModel::Timeline(model) => {
            let layout = crate::timeline::layout_timeline_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Timeline(pair(model, layout))
        }
        RenderSemanticModel::Journey(model) => {
            let layout = crate::journey::layout_journey_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Journey(pair(model, layout))
        }
        RenderSemanticModel::Requirement(model) => {
            let layout = crate::requirement::layout_requirement_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Requirement(pair(model, layout))
        }
        RenderSemanticModel::Sankey(model) => {
            let layout = crate::sankey::layout_sankey_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Sankey(pair(model, layout))
        }
        RenderSemanticModel::Radar(model) => {
            let layout = crate::radar::layout_radar_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Radar(pair(model, layout))
        }
        RenderSemanticModel::Info(model) => {
            let layout = crate::info::layout_info_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Info(pair(model, layout))
        }
        RenderSemanticModel::Treemap(model) => {
            let layout = crate::treemap::layout_treemap_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Treemap(pair(model, layout))
        }
        RenderSemanticModel::Block(model) => {
            let layout = crate::block::layout_block_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Block(pair(model, layout))
        }
        RenderSemanticModel::Er(model) => {
            let layout = crate::er::layout_er_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Er(pair(model, layout))
        }
        RenderSemanticModel::QuadrantChart(model) => {
            let layout = crate::quadrantchart::layout_quadrantchart_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::QuadrantChart(pair(model, layout))
        }
        RenderSemanticModel::XyChart(model) => {
            let layout = crate::xychart::layout_xychart_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::XyChart(pair(model, layout))
        }
        RenderSemanticModel::GitGraph(model) => {
            let layout = crate::gitgraph::layout_gitgraph_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::GitGraph(pair(model, layout))
        }
        RenderSemanticModel::TreeView(model) => {
            let layout = crate::tree_view::layout_tree_view_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::TreeView(pair(model, layout))
        }
        RenderSemanticModel::Ishikawa(model) => {
            let layout = crate::ishikawa::layout_ishikawa_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::Ishikawa(pair(model, layout))
        }
        RenderSemanticModel::EventModeling(model) => {
            let layout = crate::eventmodeling::layout_eventmodeling_diagram_typed(
                &model,
                effective_config,
                execution.text_measurer(),
            )?;
            BuiltinFamilyArtifact::EventModeling(pair(model, layout))
        }
        RenderSemanticModel::Venn(model) => {
            let layout = crate::venn::layout_venn_diagram_typed(&model, title, effective_config)?;
            BuiltinFamilyArtifact::Venn(pair(model, layout))
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
