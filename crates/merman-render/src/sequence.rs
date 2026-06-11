use crate::Result;
use crate::math::MathRenderer;
use crate::model::{LayoutCluster, SequenceDiagramLayout};
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;
use serde_json::Value;

mod activation;
mod actors;
mod block_bounds;
mod block_steps;
pub(crate) mod config;
mod constants;
mod messages;
mod metrics;
mod notes;
mod orchestration;
mod rect;
mod root_bounds;

pub(crate) use activation::{sequence_activation_stack_bounds, sequence_activation_start_x};
pub(crate) use constants::{
    SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX, SEQUENCE_MESSAGE_WRAP_SLACK_FACTOR,
    SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX, sequence_actor_popup_panel_height,
    sequence_text_dimensions_height_px, sequence_text_line_step_px,
};
pub(crate) use metrics::{SequenceMathHeightMode, measure_sequence_math_label};
pub(crate) use notes::sequence_note_final_wrapped_lines;

use actors::{SequenceActorLayoutPlan, SequenceActorLayoutPlanContext, plan_sequence_actors};
use block_bounds::sequence_block_bounds;
use config::SequenceLayoutSettings;
use orchestration::{SequenceLayoutGraph, SequenceLayoutGraphContext, build_sequence_layout_graph};
use rect::sequence_rect_stack_x_bounds;
use root_bounds::{SequenceRootBoundsContext, sequence_root_bounds};

pub fn layout_sequence_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    layout_sequence_diagram_with_title(semantic, None, effective_config, measurer, math_renderer)
}

pub fn layout_sequence_diagram_with_title(
    semantic: &Value,
    diagram_title: Option<&str>,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    let model: SequenceDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    layout_sequence_diagram_typed_with_title(
        &model,
        diagram_title,
        effective_config,
        measurer,
        math_renderer,
    )
}

pub fn layout_sequence_diagram_typed(
    model: &SequenceDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    layout_sequence_diagram_typed_with_title(model, None, effective_config, measurer, math_renderer)
}

pub fn layout_sequence_diagram_typed_with_title(
    model: &SequenceDiagramRenderModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    let math_config = MermaidConfig::from_value(effective_config.clone());
    let settings = SequenceLayoutSettings::from_effective_config(effective_config);

    let SequenceActorLayoutPlan {
        actor_index,
        actor_widths,
        actor_base_heights,
        actor_box,
        actor_left_x,
        actor_centers_x,
        box_margins,
        actor_top_offset_y,
        max_actor_layout_height,
        has_boxes,
    } = plan_sequence_actors(SequenceActorLayoutPlanContext {
        model,
        measurer,
        actor_text_style: &settings.actor_text_style,
        note_text_style: &settings.note_text_style,
        msg_text_style: &settings.msg_text_style,
        math_config: &math_config,
        math_renderer,
        actor_width_min: settings.sequence_default_width,
        actor_height: settings.actor_height,
        actor_margin: settings.actor_margin,
        actor_font_size: settings.actor_text_style.font_size,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        wrap_padding: settings.wrap_padding,
        message_width_scale: settings.message_width_scale,
        message_font_size: settings.msg_text_style.font_size,
    })?;

    let clusters: Vec<LayoutCluster> = Vec::new();

    let SequenceLayoutGraph {
        mut nodes,
        edges,
        bottom_box_top_y,
    } = build_sequence_layout_graph(SequenceLayoutGraphContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        actor_base_heights: &actor_base_heights,
        actor_top_offset_y,
        max_actor_layout_height,
        actor_width_min: settings.sequence_default_width,
        sequence_default_width: settings.sequence_default_width,
        actor_height: settings.actor_height,
        message_margin: settings.message_margin,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        bottom_margin_adj: settings.bottom_margin_adj,
        label_box_height: settings.label_box_height,
        message_step: settings.message_step,
        message_text_line_height: settings.message_text_line_height,
        msg_label_offset: settings.msg_label_offset,
        message_font_size: settings.msg_text_style.font_size,
        message_width_scale: settings.message_width_scale,
        wrap_padding: settings.wrap_padding,
        mirror_actors: settings.mirror_actors,
        activation_width: settings.activation_width,
        measurer,
        msg_text_style: &settings.msg_text_style,
        note_text_style: &settings.note_text_style,
        math_config: &math_config,
        math_renderer,
    });

    // Mermaid's SVG `viewBox` is derived from `svg.getBBox()` plus diagram margins. Block frames
    // (`alt`, `par`, `loop`, `opt`, `break`, `critical`) can extend beyond the node/edge graph we
    // model in headless layout. Capture their extents so we can expand bounds before emitting the
    // final `viewBox`.
    let block_bounds = sequence_block_bounds(model, &nodes, &edges);

    let rect_x_bounds = sequence_rect_stack_x_bounds(
        model,
        &actor_index,
        &actor_centers_x,
        &edges,
        &nodes,
        settings.sequence_default_width,
        settings.box_margin,
    );
    if !rect_x_bounds.is_empty() {
        for n in &mut nodes {
            let Some(start_id) = n.id.strip_prefix("rect-") else {
                continue;
            };
            let Some((min_x, max_x)) = rect_x_bounds.get(start_id).copied() else {
                continue;
            };
            n.x = (min_x + max_x) / 2.0;
            n.width = (max_x - min_x).max(1.0);
        }
    }

    let bounds = Some(sequence_root_bounds(SequenceRootBoundsContext {
        model,
        diagram_title,
        nodes: &nodes,
        edges: &edges,
        block_bounds,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_left_x: &actor_left_x,
        actor_widths: &actor_widths,
        actor_box: &actor_box,
        box_margins: &box_margins,
        actor_width_min: settings.sequence_default_width,
        actor_height: settings.actor_height,
        bottom_box_top_y,
        diagram_margin_x: settings.diagram_margin_x,
        diagram_margin_y: settings.diagram_margin_y,
        bottom_margin_adj: settings.bottom_margin_adj,
        box_margin: settings.box_margin,
        wrap_padding: settings.wrap_padding,
        has_boxes,
        mirror_actors: settings.mirror_actors,
        measurer,
        msg_text_style: &settings.msg_text_style,
        note_text_style: &settings.note_text_style,
        math_config: &math_config,
        math_renderer,
    }));

    Ok(SequenceDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
    })
}

pub(crate) fn sequence_render_title<'a>(
    model_title: Option<&'a str>,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    if model_title.is_none_or(|t| t.trim().is_empty()) {
        if let Some(title) = diagram_title.map(str::trim).filter(|t| !t.is_empty()) {
            return Some(title);
        }
    }
    model_title
}
