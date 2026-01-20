#![forbid(unsafe_code)]

pub mod block;
pub mod c4;
pub mod class;
pub mod er;
pub mod flowchart;
pub mod gantt;
mod generated;
pub mod gitgraph;
pub mod info;
pub mod journey;
pub mod kanban;
pub mod model;
pub mod packet;
pub mod pie;
pub mod radar;
pub mod sequence;
pub mod state;
pub mod svg;
pub mod text;
pub mod timeline;
pub mod treemap;
pub mod xychart;

use crate::model::{LayoutDiagram, LayoutMeta, LayoutedDiagram};
use crate::text::{DeterministicTextMeasurer, TextMeasurer};
use merman_core::ParsedDiagram;
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
    pub viewport_width: f64,
    pub viewport_height: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            text_measurer: Arc::new(DeterministicTextMeasurer::default()),
            viewport_width: 800.0,
            viewport_height: 600.0,
        }
    }
}

pub fn layout_parsed(parsed: &ParsedDiagram, options: &LayoutOptions) -> Result<LayoutedDiagram> {
    let meta = LayoutMeta::from_parse_metadata(&parsed.meta);
    let diagram_type = parsed.meta.diagram_type.as_str();

    let layout = match diagram_type {
        "block" => LayoutDiagram::BlockDiagram(block::layout_block_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "radar" => LayoutDiagram::RadarDiagram(radar::layout_radar_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "treemap" => LayoutDiagram::TreemapDiagram(treemap::layout_treemap_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "flowchart-v2" => LayoutDiagram::FlowchartV2(flowchart::layout_flowchart_v2(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "stateDiagram" => LayoutDiagram::StateDiagramV2(state::layout_state_diagram_v2(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "classDiagram" | "class" => LayoutDiagram::ClassDiagramV2(class::layout_class_diagram_v2(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "er" | "erDiagram" => LayoutDiagram::ErDiagram(er::layout_er_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "sequence" => LayoutDiagram::SequenceDiagram(sequence::layout_sequence_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "info" => LayoutDiagram::InfoDiagram(info::layout_info_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "packet" => LayoutDiagram::PacketDiagram(packet::layout_packet_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "timeline" => LayoutDiagram::TimelineDiagram(timeline::layout_timeline_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "gantt" => LayoutDiagram::GanttDiagram(gantt::layout_gantt_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "c4" => LayoutDiagram::C4Diagram(c4::layout_c4_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
            options.viewport_width,
            options.viewport_height,
        )?),
        "journey" => LayoutDiagram::JourneyDiagram(journey::layout_journey_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "gitGraph" => LayoutDiagram::GitGraphDiagram(gitgraph::layout_gitgraph_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "kanban" => LayoutDiagram::KanbanDiagram(kanban::layout_kanban_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "pie" => LayoutDiagram::PieDiagram(pie::layout_pie_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        "xychart" => LayoutDiagram::XyChartDiagram(xychart::layout_xychart_diagram(
            &parsed.model,
            &meta.effective_config,
            options.text_measurer.as_ref(),
        )?),
        other => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: other.to_string(),
            });
        }
    };

    Ok(LayoutedDiagram {
        meta,
        semantic: Value::clone(&parsed.model),
        layout,
    })
}
