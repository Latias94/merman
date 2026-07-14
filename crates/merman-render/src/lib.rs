#![forbid(unsafe_code)]

//! Headless layout + rendering for Mermaid diagrams.
//!
//! This crate consumes `merman-core`'s semantic models and produces:
//! - a layout JSON (geometry + routes)
//! - Mermaid-like SVG output with DOM parity checks against upstream baselines

extern crate self as web_time;

#[cfg(feature = "cytoscape-layout")]
pub mod architecture;
#[cfg(feature = "cytoscape-layout")]
pub(crate) mod architecture_metrics;
pub mod block;
pub mod c4;
mod chart_palette;
pub mod class;
mod config;
pub mod cynefin;
mod dagre;
mod entities;
pub mod environment;
pub mod er;
pub mod error;
pub mod eventmodeling;
pub mod flowchart;
pub mod gantt;
mod generated;
pub mod gitgraph;
mod host_time;
pub mod info;
pub mod ishikawa;
pub mod journey;
mod json;
pub mod kanban;
pub mod math;
mod mermaid_style;
#[cfg(feature = "cytoscape-layout")]
pub mod mindmap;
pub mod model;
pub mod packet;
pub mod pie;
pub mod quadrantchart;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod resources;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod svg;
pub mod text;
mod theme;
pub mod timeline;
pub mod tree_view;
pub mod treemap;
mod trig_tables;
pub mod venn;
pub mod xychart;

pub(crate) use host_time::{Duration, Instant};

use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};
use crate::model::{LayoutDiagram, LayoutMeta, LayoutedDiagram};
use merman_core::diagrams::flowchart::FlowchartV2Model;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{ParsedDiagram, ParsedDiagramRender, RenderSemanticModel};
use serde_json::Value;

pub use resources::{
    ClassComplexity, FlowchartComplexity, RenderResourceLimits, RenderResourceProfile,
    ResourceLimitExceeded, ResourceLimitPhase,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error("SVG postprocessor `{pass}` failed: {message}")]
    SvgPostprocess { pass: String, message: String },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn svg_postprocess(pass: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SvgPostprocess {
            pass: pass.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub viewport_width: f64,
    pub viewport_height: f64,
    /// Enable experimental layout engines (e.g. Cytoscape COSE/FCoSE ports) for diagrams that
    /// currently use placeholder layouts in merman.
    pub use_manatee_layout: bool,
    /// Selects the Flowchart ELK backend.
    ///
    /// `SourcePorted` executes the Rust source port of ELK layered layout. `Compat` keeps the
    /// previous lightweight backend available as an explicit alpha fallback.
    pub flowchart_elk_backend: FlowchartElkBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowchartElkBackend {
    Compat,
    #[default]
    SourcePorted,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            viewport_width: 800.0,
            viewport_height: 600.0,
            use_manatee_layout: false,
            flowchart_elk_backend: FlowchartElkBackend::SourcePorted,
        }
    }
}

impl LayoutOptions {
    /// Returns geometry defaults suitable for headless SVG rendering.
    pub fn headless_svg_defaults() -> Self {
        Self {
            // Mermaid parity fixtures for diagrams like mindmap/architecture rely on the COSE
            // layout port (manatee). Make the headless defaults "just work" for UI integrations.
            use_manatee_layout: true,
            ..Default::default()
        }
    }
}

struct LayoutExecution<'a> {
    request: &'a LayoutOptions,
    session: &'a RenderSession,
    text_measurer: RoutedTextMeasurer<'a>,
}

impl<'a> LayoutExecution<'a> {
    fn new(request: &'a LayoutOptions, session: &'a RenderSession) -> Self {
        Self {
            request,
            session,
            text_measurer: session.text_measurer(TextMeasurementPhase::Layout),
        }
    }

    fn text_measurer(&self) -> &dyn crate::text::TextMeasurer {
        &self.text_measurer
    }

    fn math_renderer(&self) -> Option<&(dyn crate::math::MathRenderer + Send + Sync)> {
        self.session.math_renderer()
    }

    const fn resource_limits(&self) -> RenderResourceLimits {
        self.session.resource_limits()
    }

    const fn ambient_seed(&self) -> u64 {
        self.session.seed().seed().get()
    }
}

impl std::ops::Deref for LayoutExecution<'_> {
    type Target = LayoutOptions;

    fn deref(&self) -> &Self::Target {
        self.request
    }
}

pub fn layout_parsed(
    parsed: &ParsedDiagram,
    options: &LayoutOptions,
    session: &RenderSession,
) -> Result<LayoutedDiagram> {
    let meta = LayoutMeta::from_parse_metadata(&parsed.meta);
    let layout = layout_parsed_layout_only(parsed, options, session)?;

    Ok(LayoutedDiagram {
        meta,
        semantic: crate::json::clone_value_nonrecursive(&parsed.model),
        layout,
    })
}

pub fn layout_parsed_layout_only(
    parsed: &ParsedDiagram,
    options: &LayoutOptions,
    session: &RenderSession,
) -> Result<LayoutDiagram> {
    let execution = LayoutExecution::new(options, session);
    let diagram_type = parsed.meta.diagram_type.as_str();
    let title = parsed.meta.title.as_deref();
    layout_json_by_type(
        diagram_type,
        &parsed.model,
        &parsed.meta.effective_config,
        title,
        &execution,
    )
}

pub fn layout_parsed_render_layout_only(
    parsed: &ParsedDiagramRender,
    options: &LayoutOptions,
    session: &RenderSession,
) -> Result<LayoutDiagram> {
    let execution = LayoutExecution::new(options, session);
    layout_parsed_render_layout_only_in_execution(parsed, &execution)
}

fn layout_parsed_render_layout_only_in_execution(
    parsed: &ParsedDiagramRender,
    options: &LayoutExecution<'_>,
) -> Result<LayoutDiagram> {
    let diagram_type = parsed.meta.diagram_type.as_str();
    let effective_config = parsed.meta.effective_config.as_value();
    let title = parsed.meta.title.as_deref();

    if !parsed.model.supports_diagram_type(diagram_type) {
        return Err(Error::InvalidModel {
            message: format!(
                "unexpected render model variant {} for diagram type: {diagram_type}",
                parsed.model.kind()
            ),
        });
    }

    match &parsed.model {
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Mindmap(model) => Ok(LayoutDiagram::MindmapDiagram(Box::new(
            mindmap::layout_mindmap_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
                options.use_manatee_layout,
            )?,
        ))),
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Mindmap(_) => Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }),
        #[cfg(feature = "cytoscape-layout")]
        RenderSemanticModel::Architecture(model) => Ok(LayoutDiagram::ArchitectureDiagram(
            Box::new(architecture::layout_architecture_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
                options.use_manatee_layout,
                options.ambient_seed(),
            )?),
        )),
        #[cfg(not(feature = "cytoscape-layout"))]
        RenderSemanticModel::Architecture(_) => Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }),
        RenderSemanticModel::Flowchart(model) => Ok(LayoutDiagram::FlowchartV2(Box::new(
            layout_flowchart_typed_by_engine(
                diagram_type,
                model,
                &parsed.meta.effective_config,
                options,
            )?,
        ))),
        RenderSemanticModel::State(model) => Ok(LayoutDiagram::StateDiagramV2(Box::new(
            state::layout_state_diagram_v2_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Sequence(model) => Ok(LayoutDiagram::SequenceDiagram(Box::new(
            sequence::layout_sequence_diagram_typed_with_title(
                model,
                title,
                effective_config,
                options.text_measurer(),
                options.math_renderer(),
            )?,
        ))),
        RenderSemanticModel::Class(model) => Ok(LayoutDiagram::ClassDiagramV2(Box::new(
            layout_class_typed_by_engine(
                diagram_type,
                model,
                &parsed.meta.effective_config,
                options,
            )?,
        ))),
        RenderSemanticModel::C4(model) => Ok(LayoutDiagram::C4Diagram(Box::new(
            c4::layout_c4_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
                options.viewport_width,
                options.viewport_height,
            )?,
        ))),
        RenderSemanticModel::Cynefin(model) => Ok(LayoutDiagram::CynefinDiagram(Box::new(
            cynefin::layout_cynefin_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Railroad(model) => Ok(LayoutDiagram::RailroadDiagram(Box::new(
            railroad::layout_railroad_diagram_typed_for_type(
                model,
                diagram_type,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Kanban(model) => Ok(LayoutDiagram::KanbanDiagram(Box::new(
            kanban::layout_kanban_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Gantt(model) => Ok(LayoutDiagram::GanttDiagram(Box::new(
            gantt::layout_gantt_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Pie(model) => Ok(LayoutDiagram::PieDiagram(Box::new(
            pie::layout_pie_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Packet(model) => Ok(LayoutDiagram::PacketDiagram(Box::new(
            packet::layout_packet_diagram_typed(
                model,
                title,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Timeline(model) => Ok(LayoutDiagram::TimelineDiagram(Box::new(
            timeline::layout_timeline_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Journey(model) => Ok(LayoutDiagram::JourneyDiagram(Box::new(
            journey::layout_journey_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Requirement(model) => Ok(LayoutDiagram::RequirementDiagram(Box::new(
            requirement::layout_requirement_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Sankey(model) => Ok(LayoutDiagram::SankeyDiagram(Box::new(
            sankey::layout_sankey_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Radar(model) => Ok(LayoutDiagram::RadarDiagram(Box::new(
            radar::layout_radar_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Info(model) => Ok(LayoutDiagram::InfoDiagram(Box::new(
            info::layout_info_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Treemap(model) => Ok(LayoutDiagram::TreemapDiagram(Box::new(
            treemap::layout_treemap_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Block(model) => Ok(LayoutDiagram::BlockDiagram(Box::new(
            block::layout_block_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::Er(model) => Ok(LayoutDiagram::ErDiagram(Box::new(
            er::layout_er_diagram_typed(model, effective_config, options.text_measurer())?,
        ))),
        RenderSemanticModel::QuadrantChart(model) => Ok(LayoutDiagram::QuadrantChartDiagram(
            Box::new(quadrantchart::layout_quadrantchart_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?),
        )),
        RenderSemanticModel::XyChart(model) => Ok(LayoutDiagram::XyChartDiagram(Box::new(
            xychart::layout_xychart_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::GitGraph(model) => Ok(LayoutDiagram::GitGraphDiagram(Box::new(
            gitgraph::layout_gitgraph_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::TreeView(model) => Ok(LayoutDiagram::TreeViewDiagram(Box::new(
            tree_view::layout_tree_view_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::Ishikawa(model) => Ok(LayoutDiagram::IshikawaDiagram(Box::new(
            ishikawa::layout_ishikawa_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?,
        ))),
        RenderSemanticModel::EventModeling(model) => Ok(LayoutDiagram::EventModelingDiagram(
            Box::new(eventmodeling::layout_eventmodeling_diagram_typed(
                model,
                effective_config,
                options.text_measurer(),
            )?),
        )),
        RenderSemanticModel::Venn(model) => Ok(LayoutDiagram::VennDiagram(Box::new(
            venn::layout_venn_diagram_typed(model, title, effective_config)?,
        ))),
        RenderSemanticModel::Json(semantic) => layout_json_by_type(
            diagram_type,
            semantic,
            &parsed.meta.effective_config,
            title,
            options,
        ),
    }
}

fn flowchart_uses_elk_layout(
    diagram_type: &str,
    effective_config: &merman_core::MermaidConfig,
) -> bool {
    diagram_type == "flowchart-elk" || effective_config.get_str("layout") == Some("elk")
}

fn class_uses_elk_layout(effective_config: &merman_core::MermaidConfig) -> bool {
    effective_config.get_str("layout") == Some("elk")
        || effective_config.get_str("class.defaultRenderer") == Some("elk")
}

fn layout_class_typed_by_engine(
    diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramV2Layout> {
    if class_uses_elk_layout(effective_config) {
        return layout_class_elk_typed_by_feature(diagram_type, model, effective_config, options);
    }

    options.resource_limits().check_class_complexity(model)?;
    class::layout_class_diagram_v2_typed_with_config(
        model,
        effective_config,
        options.text_measurer(),
    )
}

fn layout_class_json_by_engine(
    diagram_type: &str,
    semantic: &Value,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramV2Layout> {
    let model: ClassDiagram = crate::json::from_value_ref(semantic)?;
    if class_uses_elk_layout(effective_config) {
        return layout_class_elk_typed_by_feature(diagram_type, &model, effective_config, options);
    }

    options.resource_limits().check_class_complexity(&model)?;
    class::layout_class_diagram_v2_typed_with_config(
        &model,
        effective_config,
        options.text_measurer(),
    )
}

#[cfg(feature = "elk-layout")]
fn layout_class_elk_typed_by_feature(
    _diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramV2Layout> {
    options.resource_limits().check_class_complexity(model)?;
    class::layout_class_diagram_v2_elk_typed_with_config(
        model,
        effective_config,
        options.text_measurer(),
    )
}

#[cfg(not(feature = "elk-layout"))]
fn layout_class_elk_typed_by_feature(
    diagram_type: &str,
    _model: &ClassDiagram,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramV2Layout> {
    Err(Error::UnsupportedDiagram {
        diagram_type: diagram_type.to_string(),
    })
}

fn layout_flowchart_typed_by_engine(
    diagram_type: &str,
    model: &FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    if flowchart_uses_elk_layout(diagram_type, effective_config) {
        return layout_flowchart_elk_typed_by_feature(
            diagram_type,
            model,
            effective_config,
            options,
        );
    }

    options
        .resource_limits()
        .check_flowchart_complexity(model)?;
    flowchart::layout_flowchart_v2_typed(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
    )
}

#[cfg(feature = "elk-layout")]
fn layout_flowchart_elk_typed_by_feature(
    _diagram_type: &str,
    model: &FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    options
        .resource_limits()
        .check_flowchart_complexity(model)?;
    flowchart::elk::layout_flowchart_elk_typed(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        options.flowchart_elk_backend,
    )
}

#[cfg(not(feature = "elk-layout"))]
fn layout_flowchart_elk_typed_by_feature(
    diagram_type: &str,
    _model: &FlowchartV2Model,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    Err(Error::UnsupportedDiagram {
        diagram_type: diagram_type.to_string(),
    })
}

fn layout_flowchart_json_by_engine(
    diagram_type: &str,
    semantic: &Value,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    if flowchart_uses_elk_layout(diagram_type, effective_config) {
        return layout_flowchart_elk_json_by_feature(
            diagram_type,
            semantic,
            effective_config,
            options,
        );
    }

    let model: FlowchartV2Model = crate::json::from_value_ref(semantic)?;
    options
        .resource_limits()
        .check_flowchart_complexity(&model)?;
    flowchart::layout_flowchart_v2(
        semantic,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
    )
}

#[cfg(feature = "elk-layout")]
fn layout_flowchart_elk_json_by_feature(
    _diagram_type: &str,
    semantic: &Value,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    let model: FlowchartV2Model = crate::json::from_value_ref(semantic)?;
    options
        .resource_limits()
        .check_flowchart_complexity(&model)?;
    flowchart::elk::layout_flowchart_elk(
        semantic,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
        options.flowchart_elk_backend,
    )
}

#[cfg(not(feature = "elk-layout"))]
fn layout_flowchart_elk_json_by_feature(
    diagram_type: &str,
    _semantic: &Value,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
) -> Result<model::FlowchartV2Layout> {
    Err(Error::UnsupportedDiagram {
        diagram_type: diagram_type.to_string(),
    })
}

fn layout_json_by_type(
    diagram_type: &str,
    semantic: &Value,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    options: &LayoutExecution<'_>,
) -> Result<LayoutDiagram> {
    let effective_config_value = effective_config.as_value();

    match diagram_type {
        "error" => Ok(LayoutDiagram::ErrorDiagram(Box::new(
            error::layout_error_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        "block" => Ok(LayoutDiagram::BlockDiagram(Box::new(
            block::layout_block_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        #[cfg(feature = "cytoscape-layout")]
        "architecture" => Ok(LayoutDiagram::ArchitectureDiagram(Box::new(
            architecture::layout_architecture_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
                options.use_manatee_layout,
                options.ambient_seed(),
            )?,
        ))),
        #[cfg(not(feature = "cytoscape-layout"))]
        "architecture" => Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }),
        "requirement" => Ok(LayoutDiagram::RequirementDiagram(Box::new(
            requirement::layout_requirement_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "radar" => Ok(LayoutDiagram::RadarDiagram(Box::new(
            radar::layout_radar_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        "treemap" => Ok(LayoutDiagram::TreemapDiagram(Box::new(
            treemap::layout_treemap_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "venn" => Ok(LayoutDiagram::VennDiagram(Box::new(
            venn::layout_venn_diagram(semantic, title, effective_config_value)?,
        ))),
        "flowchart-v2" | "flowchart-elk" => Ok(LayoutDiagram::FlowchartV2(Box::new(
            layout_flowchart_json_by_engine(diagram_type, semantic, effective_config, options)?,
        ))),
        "stateDiagram" => Ok(LayoutDiagram::StateDiagramV2(Box::new(
            state::layout_state_diagram_v2(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "classDiagram" | "class" => Ok(LayoutDiagram::ClassDiagramV2(Box::new(
            layout_class_json_by_engine(diagram_type, semantic, effective_config, options)?,
        ))),
        "er" | "erDiagram" => Ok(LayoutDiagram::ErDiagram(Box::new(er::layout_er_diagram(
            semantic,
            effective_config_value,
            options.text_measurer(),
        )?))),
        "sequence" | "zenuml" => Ok(LayoutDiagram::SequenceDiagram(Box::new(
            sequence::layout_sequence_diagram_with_title(
                semantic,
                title,
                effective_config_value,
                options.text_measurer(),
                options.math_renderer(),
            )?,
        ))),
        "info" => Ok(LayoutDiagram::InfoDiagram(Box::new(
            info::layout_info_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        "packet" => Ok(LayoutDiagram::PacketDiagram(Box::new(
            packet::layout_packet_diagram(
                semantic,
                title,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "timeline" => Ok(LayoutDiagram::TimelineDiagram(Box::new(
            timeline::layout_timeline_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "gantt" => Ok(LayoutDiagram::GanttDiagram(Box::new(
            gantt::layout_gantt_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        "c4" => Ok(LayoutDiagram::C4Diagram(Box::new(c4::layout_c4_diagram(
            semantic,
            effective_config_value,
            options.text_measurer(),
            options.viewport_width,
            options.viewport_height,
        )?))),
        "cynefin" => Ok(LayoutDiagram::CynefinDiagram(Box::new(
            cynefin::layout_cynefin_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "railroad" | "railroadEbnf" | "railroadAbnf" | "railroadPeg" => Ok(
            LayoutDiagram::RailroadDiagram(Box::new(railroad::layout_railroad_diagram_for_type(
                semantic,
                diagram_type,
                effective_config_value,
                options.text_measurer(),
            )?)),
        ),
        "journey" => Ok(LayoutDiagram::JourneyDiagram(Box::new(
            journey::layout_journey_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "gitGraph" => Ok(LayoutDiagram::GitGraphDiagram(Box::new(
            gitgraph::layout_gitgraph_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "kanban" => Ok(LayoutDiagram::KanbanDiagram(Box::new(
            kanban::layout_kanban_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "pie" => Ok(LayoutDiagram::PieDiagram(Box::new(
            pie::layout_pie_diagram(semantic, effective_config_value, options.text_measurer())?,
        ))),
        "xychart" => Ok(LayoutDiagram::XyChartDiagram(Box::new(
            xychart::layout_xychart_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "quadrantChart" => Ok(LayoutDiagram::QuadrantChartDiagram(Box::new(
            quadrantchart::layout_quadrantchart_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        #[cfg(feature = "cytoscape-layout")]
        "mindmap" => Ok(LayoutDiagram::MindmapDiagram(Box::new(
            mindmap::layout_mindmap_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
                options.use_manatee_layout,
            )?,
        ))),
        #[cfg(not(feature = "cytoscape-layout"))]
        "mindmap" => Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }),
        "sankey" => Ok(LayoutDiagram::SankeyDiagram(Box::new(
            sankey::layout_sankey_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "treeView" => Ok(LayoutDiagram::TreeViewDiagram(Box::new(
            tree_view::layout_tree_view_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "ishikawa" => Ok(LayoutDiagram::IshikawaDiagram(Box::new(
            ishikawa::layout_ishikawa_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        "eventmodeling" => Ok(LayoutDiagram::EventModelingDiagram(Box::new(
            eventmodeling::layout_eventmodeling_diagram(
                semantic,
                effective_config_value,
                options.text_measurer(),
            )?,
        ))),
        other => Err(Error::UnsupportedDiagram {
            diagram_type: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::{Engine, ParseOptions};

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_accepts_diagram_type_aliases() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let layout =
            layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
                .unwrap();
        assert!(matches!(layout, LayoutDiagram::FlowchartV2(_)));
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_uses_elk_for_flowchart_default_renderer_config() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  flowchart:
    defaultRenderer: elk
---
flowchart TD
A-->B
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, "flowchart-elk");
        let layout =
            layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
                .unwrap();
        let LayoutDiagram::FlowchartV2(layout) = layout else {
            panic!("expected flowchart layout");
        };
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.y > a.y);
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_rejects_flowchart_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(RenderResourceLimits {
                max_flowchart_nodes: Some(1),
                ..RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();

        let err = layout_parsed_render_layout_only(&parsed, &options, &session).unwrap_err();

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_flowchart_nodes");
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_uses_elk_for_class_layout_config() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
direction LR
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, "classDiagram");
        let layout =
            layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
                .unwrap();
        let LayoutDiagram::ClassDiagramV2(layout) = layout else {
            panic!("expected class layout");
        };
        let animal = layout
            .nodes
            .iter()
            .find(|node| node.id == "Animal")
            .unwrap();
        let duck = layout.nodes.iter().find(|node| node.id == "Duck").unwrap();
        assert!(
            duck.x > animal.x,
            "ELK LR class layout should place Duck to the right of Animal; Animal={}, Duck={}",
            animal.x,
            duck.x
        );
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_uses_elk_for_class_default_renderer_config() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  class:
    defaultRenderer: elk
---
classDiagram
direction LR
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, "class");
        let layout =
            layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
                .unwrap();
        let LayoutDiagram::ClassDiagramV2(layout) = layout else {
            panic!("expected class layout");
        };
        let animal = layout
            .nodes
            .iter()
            .find(|node| node.id == "Animal")
            .unwrap();
        let duck = layout.nodes.iter().find(|node| node.id == "Duck").unwrap();
        assert!(
            duck.x > animal.x,
            "ELK LR class layout should place Duck to the right of Animal; Animal={}, Duck={}",
            animal.x,
            duck.x
        );
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_rejects_class_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(RenderResourceLimits {
                max_class_nodes: Some(1),
                ..RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();

        let err = layout_parsed_render_layout_only(&parsed, &options, &session).unwrap_err();

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_class_nodes");
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn json_dispatch_rejects_flowchart_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_sync("flowchart TD\nA-->B\nB-->C\nC-->D", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(RenderResourceLimits {
                max_flowchart_edges: Some(2),
                ..RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();

        let err = layout_parsed(&parsed, &options, &session).unwrap_err();

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_flowchart_edges");
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn json_dispatch_rejects_class_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_sync(
                "classDiagram\nAnimal <|-- Duck\nDuck <|-- Mallard",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_limits(RenderResourceLimits {
                max_class_edges: Some(1),
                ..RenderResourceLimits::unbounded_for_trusted_input()
            })
            .begin_session()
            .unwrap();

        let err = layout_parsed(&parsed, &options, &session).unwrap_err();

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_class_edges");
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_layouted_svg_preserves_flowchart_elk_roledescription() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_sync("flowchart-elk TD\nA-->B;", ParseOptions::strict())
            .unwrap()
            .unwrap();

        let layout_options = LayoutOptions::default();
        let layouted = layout_parsed(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layouted_svg(
            &layouted,
            &_session,
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("elk-smoke".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
        assert!(svg.contains("elk-smoke_flowchart-elk-pointEnd"));
        assert!(!svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(r#"<g><marker id="elk-smoke_flowchart-elk-pointEnd""#)
            .expect("ELK marker group");
        let defs_pos = svg
            .find(r#"<defs><filter id="elk-smoke-drop-shadow""#)
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_layouted_svg_uses_elk_adapter_dom_for_flowchart_layout_elk() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_sync(
                r#"---
config:
  layout: elk
---
flowchart LR
A{A} --> B & C
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let layout_options = LayoutOptions {
            flowchart_elk_backend: FlowchartElkBackend::SourcePorted,
            ..Default::default()
        };
        let layouted = layout_parsed(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layouted_svg(
            &layouted,
            &_session,
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("elk-layout-smoke".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(svg.contains("elk-layout-smoke_flowchart-v2-pointEnd"));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(r#"<g><marker id="elk-layout-smoke_flowchart-v2-pointEnd""#)
            .expect("ELK marker group");
        let defs_pos = svg
            .find(r#"<defs><filter id="elk-layout-smoke-drop-shadow""#)
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_layouted_svg_uses_right_angle_edges_for_flowchart_elk() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_sync("flowchart-elk LR\nA --> B\nA --> C", ParseOptions::strict())
            .unwrap()
            .unwrap();

        let layout_options = LayoutOptions {
            ..Default::default()
        };
        let layouted = layout_parsed(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layouted_svg(
            &layouted,
            &_session,
            &crate::svg::SvgRenderOptions::default(),
        )
        .unwrap();

        let path = edge_path_chunk(&svg, "L_A_B_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('L') && !d.contains('C'),
            "expected ELK edges to use right-angle paths without smooth curves by default: {d}"
        );
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_layouted_svg_keeps_source_ported_elk_rect_edge_boundary_points() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_sync(
                r#"---
config:
  htmlLabels: true
  flowchart:
    htmlLabels: true
  securityLevel: loose
---
flowchart-elk LR
id1(Start)-->id2(Stop)
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let layout_options = LayoutOptions {
            flowchart_elk_backend: FlowchartElkBackend::SourcePorted,
            ..Default::default()
        };
        let layouted = layout_parsed(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layouted_svg(
            &layouted,
            &_session,
            &crate::svg::SvgRenderOptions::default(),
        )
        .unwrap();

        let path = edge_path_chunk(&svg, "L_id1_id2_0");
        let d = edge_path_d(path);
        assert!(
            !d.contains('Q'),
            "straight ELK roundedRect edge should not gain a rounded corner: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            2,
            "unexpected ELK edge data-points: {points:?}"
        );
        assert_eq!(points[0], (77.015625, 39.0));
        assert_eq!(points[1], (117.015625, 39.0));
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    #[test]
    fn render_layouted_svg_keeps_source_ported_elk_self_loop_edges() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_sync("flowchart-elk TD\nA --> A", ParseOptions::strict())
            .unwrap()
            .unwrap();

        let layout_options = LayoutOptions {
            flowchart_elk_backend: FlowchartElkBackend::SourcePorted,
            ..Default::default()
        };
        let layouted = layout_parsed(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layouted_svg(
            &layouted,
            &_session,
            &crate::svg::SvgRenderOptions::default(),
        )
        .unwrap();

        let path = edge_path_chunk(&svg, "L_A_A_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('Q'),
            "ELK self-loop path should be rendered from the source-backed edge: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            4,
            "unexpected ELK self-loop data-points: {points:?}"
        );
        assert!(
            !svg.contains("A---A---1") && !svg.contains("cyclic-special"),
            "ELK renderer must not reuse Dagre self-loop helper nodes: {svg}"
        );
        assert!(svg.contains(r#"data-id="L_A_A_0" transform="translate(0,0)""#));
    }

    #[cfg(all(feature = "core-full", not(feature = "elk-layout")))]
    #[test]
    fn render_model_dispatch_rejects_flowchart_elk_without_feature() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedDiagram { diagram_type } if diagram_type == "flowchart-elk"
        ));
    }

    #[cfg(all(feature = "core-full", not(feature = "elk-layout")))]
    #[test]
    fn render_model_dispatch_rejects_class_elk_without_feature() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedDiagram { diagram_type } if diagram_type == "classDiagram"
        ));
    }

    #[test]
    fn render_model_dispatch_rejects_mismatched_typed_model() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let mut parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nAlice->>Bob: Hi",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        parsed.meta.diagram_type = "flowchart-v2".to_string();

        let err = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default(), &_session)
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("sequence"));
        assert!(message.contains("flowchart-v2"));
    }

    #[test]
    fn render_model_dispatch_renders_cynefin_svg() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let source = r#"cynefin-beta
  title Team Practices
  accTitle: Cynefin map
  accDescr: Practice movement
  complex
    "Pair programming"
  complicated
    "Architecture review"
  complex --> complicated : "Pattern emerges"
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let layout_options = LayoutOptions::default();
        let layout = layout_parsed_render_layout_only(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layout_svg_parts_for_render_model_with_metadata(
            &layout,
            &parsed.model,
            &parsed.meta.effective_config,
            &parsed.meta.diagram_type,
            parsed.meta.title.as_deref(),
            &_session,
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-test".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(svg.contains(r#"aria-roledescription="cynefin""#));
        assert!(svg.contains(r#"<g class="cynefin-backgrounds">"#));
        assert!(svg.contains(r#"class="cynefinDomain""#));
        assert!(svg.contains(r#"class="cynefinBoundary""#));
        assert!(svg.contains(r#"class="cynefinCliff""#));
        assert!(svg.contains(r#"class="cynefinItem""#));
        assert!(svg.contains("Pair programming"));
        assert!(svg.contains(r#"class="cynefinArrowLine""#));
        assert!(svg.contains("Pattern emerges"));
        assert!(svg.contains(r#"<title id="chart-title-cynefin-test">Cynefin map</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-cynefin-test">Practice movement</desc>"#));
        assert!(svg.contains("#cynefin-test .cynefinDomain{stroke:none;}"));
        assert_eq!(svg.matches("<title").count(), 2, "{svg}");
        assert_eq!(svg.matches("<desc").count(), 2, "{svg}");

        let scoped_title = svg
            .find(r#"<title id="chart-title-cynefin-test">"#)
            .expect("scoped accessibility title");
        let scoped_descr = svg
            .find(r#"<desc id="chart-desc-cynefin-test">"#)
            .expect("scoped accessibility description");
        let style = svg.find("<style>").expect("style");
        let framework_group = svg.find("<g/>").expect("Mermaid framework group");
        let renderer_title = svg
            .find("<title>Cynefin map</title>")
            .expect("renderer accessibility title");
        let renderer_descr = svg
            .find("<desc>Practice movement</desc>")
            .expect("renderer accessibility description");
        let root_group = svg
            .find(r#"<g transform="translate("#)
            .expect("cynefin root group");
        let defs = svg.find("<defs>").expect("transition marker defs");

        assert!(scoped_title < scoped_descr, "{svg}");
        assert!(scoped_descr < style, "{svg}");
        assert!(style < framework_group, "{svg}");
        assert!(framework_group < renderer_title, "{svg}");
        assert!(renderer_title < renderer_descr, "{svg}");
        assert!(renderer_descr < root_group, "{svg}");
        assert!(root_group < defs, "{svg}");
    }

    #[test]
    fn render_model_dispatch_keeps_whitespace_cynefin_transition_labels() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let source = r#"cynefin-beta
  complex
  complicated
  complex --> complicated : "   "
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let layout_options = LayoutOptions::default();
        let layout = layout_parsed_render_layout_only(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layout_svg_parts_for_render_model_with_metadata(
            &layout,
            &parsed.model,
            &parsed.meta.effective_config,
            &parsed.meta.diagram_type,
            parsed.meta.title.as_deref(),
            &_session,
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-whitespace".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            svg.contains(r#"class="cynefinArrowLabel""#),
            "a whitespace-only label is truthy in JavaScript and must emit a text node: {svg}"
        );
    }

    #[test]
    fn render_model_dispatch_renders_railroad_svg() {
        let _session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let source = r#"railroad-beta
accTitle: Railroad grammar
accDescr: Expression grammar
expr = sequence(nonterminal("term"), optional(special("guard")), zeroOrMore(terminal("+"))) ;
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let layout_options = LayoutOptions::default();
        let layout = layout_parsed_render_layout_only(&parsed, &layout_options, &_session).unwrap();
        let svg = crate::svg::render_layout_svg_parts_for_render_model_with_metadata(
            &layout,
            &parsed.model,
            &parsed.meta.effective_config,
            &parsed.meta.diagram_type,
            parsed.meta.title.as_deref(),
            &_session,
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("railroad-test".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(svg.contains(r#"aria-roledescription="railroad""#));
        assert!(svg.contains(r#"class="railroad-diagram""#));
        assert!(svg.contains(r#"class="railroad-rule""#));
        assert!(svg.contains(r#"class="railroad-rule-name""#));
        assert!(svg.contains(r#"class="railroad-nonterminal""#));
        assert!(svg.contains(r#"class="railroad-special""#));
        assert!(svg.contains(r#"class="railroad-terminal""#));
        assert!(svg.contains(r#"class="railroad-line""#));
        assert!(svg.contains("term"));
        assert!(svg.contains("? guard ?"));
        assert!(svg.contains("+"));
        assert!(svg.contains(r#"<title id="chart-title-railroad-test">Railroad grammar</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-railroad-test">Expression grammar</desc>"#));
        assert!(
            svg.contains("</style><g/><g class=\"railroad-rule\""),
            "{svg}"
        );
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    fn edge_path_chunk<'a>(svg: &'a str, edge_id: &str) -> &'a str {
        let id_attr = format!(r#"id="merman-{edge_id}""#);
        let id_start = svg.find(&id_attr).expect("edge id");
        let path_start = svg[..id_start].rfind("<path ").expect("edge path start");
        let path_end = svg[id_start..].find("/>").expect("edge path end") + id_start;
        &svg[path_start..path_end]
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    fn edge_path_d(path: &str) -> &str {
        let d_start = path.find(r#"d=""#).expect("edge path d") + r#"d=""#.len();
        let d_end = path[d_start..].find('"').expect("edge path d end") + d_start;
        &path[d_start..d_end]
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    fn edge_attr_value<'a>(path: &'a str, attr: &str) -> &'a str {
        let needle = format!(r#"{attr}=""#);
        let start = path.find(&needle).expect("edge attr") + needle.len();
        let end = path[start..].find('"').expect("edge attr end") + start;
        &path[start..end]
    }

    #[cfg(all(feature = "core-full", feature = "elk-layout"))]
    fn edge_data_points(path: &str) -> Vec<(f64, f64)> {
        use base64::Engine as _;

        let b64 = edge_attr_value(path, "data-points");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .expect("data-points base64");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("data-points JSON payload");
        json.as_array()
            .expect("data-points array")
            .iter()
            .map(|point| {
                (
                    point.get("x").and_then(serde_json::Value::as_f64).unwrap(),
                    point.get("y").and_then(serde_json::Value::as_f64).unwrap(),
                )
            })
            .collect()
    }
}
