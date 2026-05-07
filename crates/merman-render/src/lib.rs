#![forbid(unsafe_code)]

//! Headless layout + rendering for Mermaid diagrams.
//!
//! This crate consumes `merman-core`'s semantic models and produces:
//! - a layout JSON (geometry + routes)
//! - Mermaid-like SVG output with DOM parity checks against upstream baselines

pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
mod entities;
pub mod er;
pub mod error;
pub mod flowchart;
pub mod gantt;
mod generated;
pub mod gitgraph;
pub mod info;
pub mod journey;
mod json;
pub mod kanban;
pub mod math;
pub mod mindmap;
pub mod model;
pub mod packet;
pub mod pie;
pub mod quadrantchart;
pub mod radar;
pub mod requirement;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod svg;
pub mod text;
pub mod timeline;
pub mod treemap;
mod trig_tables;
pub mod xychart;

use crate::math::MathRenderer;
use crate::model::{LayoutDiagram, LayoutMeta, LayoutedDiagram};
use crate::text::{DeterministicTextMeasurer, TextMeasurer};
use merman_core::{ParsedDiagram, ParsedDiagramRender, RenderSemanticModel};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct LayoutOptions {
    pub text_measurer: Arc<dyn TextMeasurer + Send + Sync>,
    /// Optional math renderer for `$$...$$` style labels.
    pub math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
    pub viewport_width: f64,
    pub viewport_height: f64,
    /// Enable experimental layout engines (e.g. Cytoscape COSE/FCoSE ports) for diagrams that
    /// currently use placeholder layouts in merman.
    pub use_manatee_layout: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            text_measurer: Arc::new(DeterministicTextMeasurer::default()),
            math_renderer: None,
            viewport_width: 800.0,
            viewport_height: 600.0,
            use_manatee_layout: false,
        }
    }
}

impl LayoutOptions {
    /// Returns layout defaults suitable for headless SVG rendering in UI integrations.
    ///
    /// Compared to `Default`, this uses a Mermaid-like text measurer backed by vendored font
    /// metrics (instead of deterministic placeholder metrics).
    pub fn headless_svg_defaults() -> Self {
        Self {
            text_measurer: Arc::new(crate::text::VendoredFontMetricsTextMeasurer::default()),
            // Mermaid parity fixtures for diagrams like mindmap/architecture rely on the COSE
            // layout port (manatee). Make the headless defaults "just work" for UI integrations.
            use_manatee_layout: true,
            ..Default::default()
        }
    }

    pub fn with_text_measurer(mut self, measurer: Arc<dyn TextMeasurer + Send + Sync>) -> Self {
        self.text_measurer = measurer;
        self
    }

    pub fn with_math_renderer(mut self, renderer: Arc<dyn MathRenderer + Send + Sync>) -> Self {
        self.math_renderer = Some(renderer);
        self
    }
}

pub fn layout_parsed(parsed: &ParsedDiagram, options: &LayoutOptions) -> Result<LayoutedDiagram> {
    let meta = LayoutMeta::from_parse_metadata(&parsed.meta);
    let layout = layout_parsed_layout_only(parsed, options)?;

    Ok(LayoutedDiagram {
        meta,
        semantic: Value::clone(&parsed.model),
        layout,
    })
}

pub fn layout_parsed_layout_only(
    parsed: &ParsedDiagram,
    options: &LayoutOptions,
) -> Result<LayoutDiagram> {
    let diagram_type = parsed.meta.diagram_type.as_str();
    let title = parsed.meta.title.as_deref();
    layout_json_by_type(
        diagram_type,
        &parsed.model,
        &parsed.meta.effective_config,
        title,
        options,
    )
}

pub fn layout_parsed_render_layout_only(
    parsed: &ParsedDiagramRender,
    options: &LayoutOptions,
) -> Result<LayoutDiagram> {
    let diagram_type = parsed.meta.diagram_type.as_str();
    let effective_config = parsed.meta.effective_config.as_value();
    let title = parsed.meta.title.as_deref();

    match (&parsed.model, diagram_type) {
        (RenderSemanticModel::Mindmap(model), "mindmap") => Ok(LayoutDiagram::MindmapDiagram(
            mindmap::layout_mindmap_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
                options.use_manatee_layout,
            )?,
        )),
        (RenderSemanticModel::Architecture(model), "architecture") => Ok(
            LayoutDiagram::ArchitectureDiagram(architecture::layout_architecture_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
                options.use_manatee_layout,
            )?),
        ),
        (RenderSemanticModel::Flowchart(model), "flowchart-v2" | "flowchart" | "flowchart-elk") => {
            Ok(LayoutDiagram::FlowchartV2(
                flowchart::layout_flowchart_v2_typed(
                    model,
                    &parsed.meta.effective_config,
                    options.text_measurer.as_ref(),
                    options.math_renderer.as_deref(),
                )?,
            ))
        }
        (RenderSemanticModel::State(model), "stateDiagram" | "state") => Ok(
            LayoutDiagram::StateDiagramV2(state::layout_state_diagram_v2_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?),
        ),
        (RenderSemanticModel::Sequence(model), "sequence") => Ok(LayoutDiagram::SequenceDiagram(
            sequence::layout_sequence_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Class(model), "classDiagram" | "class") => Ok(
            LayoutDiagram::ClassDiagramV2(class::layout_class_diagram_v2_typed_with_config(
                model,
                &parsed.meta.effective_config,
                options.text_measurer.as_ref(),
            )?),
        ),
        (RenderSemanticModel::Kanban(model), "kanban") => Ok(LayoutDiagram::KanbanDiagram(
            kanban::layout_kanban_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Gantt(model), "gantt") => Ok(LayoutDiagram::GanttDiagram(
            gantt::layout_gantt_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Pie(model), "pie") => Ok(LayoutDiagram::PieDiagram(
            pie::layout_pie_diagram_typed(model, effective_config, options.text_measurer.as_ref())?,
        )),
        (RenderSemanticModel::Packet(model), "packet") => Ok(LayoutDiagram::PacketDiagram(
            packet::layout_packet_diagram_typed(
                model,
                title,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Timeline(model), "timeline") => Ok(LayoutDiagram::TimelineDiagram(
            timeline::layout_timeline_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Journey(model), "journey") => Ok(LayoutDiagram::JourneyDiagram(
            journey::layout_journey_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        (RenderSemanticModel::Requirement(model), "requirement") => Ok(
            LayoutDiagram::RequirementDiagram(requirement::layout_requirement_diagram_typed(
                model,
                effective_config,
                options.text_measurer.as_ref(),
            )?),
        ),
        (RenderSemanticModel::Json(semantic), _) => layout_json_by_type(
            diagram_type,
            semantic,
            &parsed.meta.effective_config,
            title,
            options,
        ),
        _ => Err(Error::InvalidModel {
            message: format!("unexpected render model variant for diagram type: {diagram_type}"),
        }),
    }
}

fn layout_json_by_type(
    diagram_type: &str,
    semantic: &Value,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    options: &LayoutOptions,
) -> Result<LayoutDiagram> {
    let effective_config_value = effective_config.as_value();

    match diagram_type {
        "error" => Ok(LayoutDiagram::ErrorDiagram(error::layout_error_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "block" => Ok(LayoutDiagram::BlockDiagram(block::layout_block_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "architecture" => Ok(LayoutDiagram::ArchitectureDiagram(
            architecture::layout_architecture_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
                options.use_manatee_layout,
            )?,
        )),
        "requirement" => Ok(LayoutDiagram::RequirementDiagram(
            requirement::layout_requirement_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "radar" => Ok(LayoutDiagram::RadarDiagram(radar::layout_radar_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "treemap" => Ok(LayoutDiagram::TreemapDiagram(
            treemap::layout_treemap_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "flowchart-v2" => Ok(LayoutDiagram::FlowchartV2(flowchart::layout_flowchart_v2(
            semantic,
            effective_config,
            options.text_measurer.as_ref(),
            options.math_renderer.as_deref(),
        )?)),
        "stateDiagram" => Ok(LayoutDiagram::StateDiagramV2(
            state::layout_state_diagram_v2(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "classDiagram" | "class" => Ok(LayoutDiagram::ClassDiagramV2(
            class::layout_class_diagram_v2_with_config(
                semantic,
                effective_config,
                options.text_measurer.as_ref(),
            )?,
        )),
        "er" | "erDiagram" => Ok(LayoutDiagram::ErDiagram(er::layout_er_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "sequence" | "zenuml" => Ok(LayoutDiagram::SequenceDiagram(
            sequence::layout_sequence_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "info" => Ok(LayoutDiagram::InfoDiagram(info::layout_info_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "packet" => Ok(LayoutDiagram::PacketDiagram(packet::layout_packet_diagram(
            semantic,
            title,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "timeline" => Ok(LayoutDiagram::TimelineDiagram(
            timeline::layout_timeline_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "gantt" => Ok(LayoutDiagram::GanttDiagram(gantt::layout_gantt_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "c4" => Ok(LayoutDiagram::C4Diagram(c4::layout_c4_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
            options.viewport_width,
            options.viewport_height,
        )?)),
        "journey" => Ok(LayoutDiagram::JourneyDiagram(
            journey::layout_journey_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "gitGraph" => Ok(LayoutDiagram::GitGraphDiagram(
            gitgraph::layout_gitgraph_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "kanban" => Ok(LayoutDiagram::KanbanDiagram(kanban::layout_kanban_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "pie" => Ok(LayoutDiagram::PieDiagram(pie::layout_pie_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        "xychart" => Ok(LayoutDiagram::XyChartDiagram(
            xychart::layout_xychart_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "quadrantChart" => Ok(LayoutDiagram::QuadrantChartDiagram(
            quadrantchart::layout_quadrantchart_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
            )?,
        )),
        "mindmap" => Ok(LayoutDiagram::MindmapDiagram(
            mindmap::layout_mindmap_diagram(
                semantic,
                effective_config_value,
                options.text_measurer.as_ref(),
                options.use_manatee_layout,
            )?,
        )),
        "sankey" => Ok(LayoutDiagram::SankeyDiagram(sankey::layout_sankey_diagram(
            semantic,
            effective_config_value,
            options.text_measurer.as_ref(),
        )?)),
        other => Err(Error::UnsupportedDiagram {
            diagram_type: other.to_string(),
        }),
    }
}
