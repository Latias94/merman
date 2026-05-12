use crate::Result;
use crate::math::MathRenderer;
use crate::model::{LayoutCluster, LayoutEdge, LayoutNode, SequenceDiagramLayout};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;
use serde_json::Value;

mod activation;
mod actors;
mod block_bounds;
mod block_steps;
mod config;
mod constants;
mod messages;
mod metrics;
mod notes;
mod rect;
mod root_bounds;

pub(crate) use constants::{
    SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX,
    SEQUENCE_LEFT_OF_NOTE_FINAL_WRAP_SLACK_PX, SEQUENCE_MESSAGE_WRAP_SLACK_FACTOR,
    SEQUENCE_NOTE_WRAP_SLACK_PX, SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX,
    sequence_actor_popup_panel_height, sequence_text_dimensions_height_px,
    sequence_text_line_step_px,
};
pub(crate) use metrics::{SequenceMathHeightMode, measure_sequence_math_label};

use activation::SequenceActivationState;
use actors::{
    SequenceActorLayoutPlan, SequenceActorLayoutPlanContext, SequenceActorLifecycle,
    SequenceActorLifecycleContext, SequenceFooterActorContext, SequenceTopActorContext,
    append_sequence_footer_actors, append_sequence_top_actors, plan_sequence_actors,
    sequence_actor_is_type_width_limited,
};
use block_bounds::sequence_block_bounds;
use block_steps::{BlockStepPlanContext, plan_sequence_directive_steps};
use config::{config_f64, config_string};
use messages::{SequenceMessageLayoutContext, layout_sequence_message};
use notes::{SequenceNoteLayoutContext, layout_sequence_note};
use rect::{SequenceRectOpen, sequence_rect_stack_x_bounds};
use root_bounds::{SequenceRootBoundsContext, sequence_root_bounds};

pub fn layout_sequence_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    let model: SequenceDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    layout_sequence_diagram_typed(&model, effective_config, measurer, math_renderer)
}

pub fn layout_sequence_diagram_typed(
    model: &SequenceDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SequenceDiagramLayout> {
    let math_config = MermaidConfig::from_value(effective_config.clone());
    let seq_cfg = effective_config.get("sequence").unwrap_or(&Value::Null);
    let diagram_margin_x = config_f64(seq_cfg, &["diagramMarginX"]).unwrap_or(50.0);
    let diagram_margin_y = config_f64(seq_cfg, &["diagramMarginY"]).unwrap_or(10.0);
    let bottom_margin_adj = config_f64(seq_cfg, &["bottomMarginAdj"]).unwrap_or(1.0);
    let box_margin = config_f64(seq_cfg, &["boxMargin"]).unwrap_or(10.0);
    let actor_margin = config_f64(seq_cfg, &["actorMargin"]).unwrap_or(50.0);
    let actor_width_min = config_f64(seq_cfg, &["width"]).unwrap_or(150.0);
    let actor_height = config_f64(seq_cfg, &["height"]).unwrap_or(65.0);
    let message_margin = config_f64(seq_cfg, &["messageMargin"]).unwrap_or(35.0);
    let wrap_padding = config_f64(seq_cfg, &["wrapPadding"]).unwrap_or(10.0);
    let box_text_margin = config_f64(seq_cfg, &["boxTextMargin"]).unwrap_or(5.0);
    let label_box_height = config_f64(seq_cfg, &["labelBoxHeight"]).unwrap_or(20.0);
    let mirror_actors = seq_cfg
        .get("mirrorActors")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Mermaid's `sequenceRenderer.setConf(...)` overrides per-sequence font settings whenever the
    // global `fontFamily` / `fontSize` / `fontWeight` are present (defaults are always present).
    let global_font_family = config_string(effective_config, &["fontFamily"]);
    let global_font_size = config_f64(effective_config, &["fontSize"]);
    let global_font_weight = config_string(effective_config, &["fontWeight"]);

    let message_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["messageFontFamily"]));
    let message_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["messageFontSize"]))
        .unwrap_or(16.0);
    let message_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["messageFontWeight"]));

    let actor_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["actorFontFamily"]));
    let actor_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["actorFontSize"]))
        .unwrap_or(16.0);
    let actor_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["actorFontWeight"]));

    // Upstream sequence uses `calculateTextDimensions(...).width` (SVG `getBBox`) when computing
    // message widths for spacing. Keep this scale at 1.0 and handle any residual differences via
    // the SVG-backed `TextMeasurer` implementation.
    let message_width_scale = 1.0;

    let actor_text_style = TextStyle {
        font_family: actor_font_family,
        font_size: actor_font_size,
        font_weight: actor_font_weight,
    };
    let note_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["noteFontFamily"]));
    let note_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["noteFontSize"]))
        .unwrap_or(16.0);
    let note_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["noteFontWeight"]));
    let note_text_style = TextStyle {
        font_family: note_font_family,
        font_size: note_font_size,
        font_weight: note_font_weight,
    };
    let msg_text_style = TextStyle {
        font_family: message_font_family,
        font_size: message_font_size,
        font_weight: message_font_weight,
    };

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
        actor_text_style: &actor_text_style,
        note_text_style: &note_text_style,
        msg_text_style: &msg_text_style,
        math_config: &math_config,
        math_renderer,
        actor_width_min,
        actor_height,
        actor_margin,
        actor_font_size,
        box_margin,
        box_text_margin,
        wrap_padding,
        message_width_scale,
        message_font_size,
    })?;

    let message_text_line_height = sequence_text_dimensions_height_px(message_font_size);
    let message_step = box_margin + 2.0 * message_text_line_height;
    let msg_label_offset = (2.0 * message_text_line_height - wrap_padding / 2.0).max(0.0);

    let mut edges: Vec<LayoutEdge> = Vec::new();
    let mut nodes: Vec<LayoutNode> = Vec::new();
    let clusters: Vec<LayoutCluster> = Vec::new();

    // Actor boxes: Mermaid renders both a "top" and "bottom" actor box.
    // The bottom boxes start after all messages are placed. Created actors will have their `y`
    // adjusted later once we know the creation message position.
    append_sequence_top_actors(
        &mut nodes,
        SequenceTopActorContext {
            actor_order: &model.actor_order,
            actors: &model.actors,
            actor_widths: &actor_widths,
            actor_centers_x: &actor_centers_x,
            actor_base_heights: &actor_base_heights,
            actor_top_offset_y,
            label_box_height,
        },
    );

    // Message edges.
    let directive_steps = plan_sequence_directive_steps(BlockStepPlanContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        message_margin,
        box_margin,
        box_text_margin,
        bottom_margin_adj,
        label_box_height,
        message_font_size,
        measurer,
        msg_text_style: &msg_text_style,
        math_config: &math_config,
        math_renderer,
        message_width_scale,
    });

    // Mermaid's sequence renderer advances a "cursor" even for non-message directives (notes,
    // rect blocks). To avoid overlapping bottom actors and to match upstream viewBox sizes, we
    // model these increments in headless layout as well.
    let note_width_single = actor_width_min;
    let rect_step_start = 20.0;
    let rect_step_end = 10.0;
    let note_gap = 10.0;
    // Mermaid note boxes use 10px vertical padding on both sides (20px total), on top of the
    // SVG `getBBox().height` of the note text.
    let note_text_pad_total = 2.0 * note_gap;
    let note_top_offset = message_step - note_gap;

    // Mermaid advances the message cursor before special actor shapes mutate their rendered
    // height, so the first message uses the base actor layout height rather than the final visual
    // bbox for boundary/control/entity/database/queue/collections actors.
    let mut cursor_y = actor_top_offset_y + max_actor_layout_height + message_step;
    let mut rect_stack: Vec<SequenceRectOpen> = Vec::new();
    let activation_width = config_f64(seq_cfg, &["activationWidth"])
        .unwrap_or(10.0)
        .max(1.0);
    let mut activation_state = SequenceActivationState::new(activation_width);

    let mut actor_lifecycle = SequenceActorLifecycle::new(SequenceActorLifecycleContext {
        actor_index: &actor_index,
        actor_widths: &actor_widths,
        actor_base_heights: &actor_base_heights,
        actors: &model.actors,
        created_actors: &model.created_actors,
        destroyed_actors: &model.destroyed_actors,
        actor_height,
        actor_width_min,
        label_box_height,
    });
    let actor_is_type_width_limited =
        |actor_id: &str| -> bool { sequence_actor_is_type_width_limited(&model.actors, actor_id) };

    for (msg_idx, msg) in model.messages.iter().enumerate() {
        if activation_state.handle_directive(msg, &actor_index, &actor_centers_x) {
            continue;
        }

        if let Some(step) = directive_steps.get(msg.id.as_str()).copied() {
            cursor_y += step;
            continue;
        }
        match msg.message_type {
            // rect start: advances cursor but draws later as a background `<rect>`.
            22 => {
                rect_stack.push(SequenceRectOpen::new(
                    msg.id.clone(),
                    cursor_y - note_top_offset,
                ));
                cursor_y += rect_step_start;
                continue;
            }
            // rect end
            23 => {
                if let Some(open) = rect_stack.pop() {
                    let closed = open.close(&actor_centers_x);
                    nodes.push(closed.node);

                    if let Some(parent) = rect_stack.last_mut() {
                        parent.include_min_max(
                            closed.left - 10.0,
                            closed.right + 10.0,
                            closed.bottom,
                        );
                    }
                }
                cursor_y += rect_step_end;
                continue;
            }
            _ => {}
        }

        // Notes (type=2) are laid out as nodes, not message edges.
        if msg.message_type == 2 {
            let Some(note) = layout_sequence_note(
                msg,
                SequenceNoteLayoutContext {
                    actor_index: &actor_index,
                    actor_centers_x: &actor_centers_x,
                    actor_widths: &actor_widths,
                    note_width_single,
                    note_text_pad_total,
                    note_top_offset,
                    note_gap,
                    cursor_y,
                    measurer,
                    note_text_style: &note_text_style,
                    math_config: &math_config,
                    math_renderer,
                },
            ) else {
                continue;
            };

            for open in rect_stack.iter_mut() {
                open.include_min_max(note.rect_min_x, note.rect_max_x, note.rect_max_y);
            }

            nodes.push(note.node);
            cursor_y += note.cursor_step;
            continue;
        }

        // Regular message edges.
        let Some(message) = layout_sequence_message(
            msg,
            SequenceMessageLayoutContext {
                actor_index: &actor_index,
                actor_centers_x: &actor_centers_x,
                actor_widths: &actor_widths,
                activation_state: &activation_state,
                msg_idx,
                actor_width_min,
                box_margin,
                wrap_padding,
                message_text_line_height,
                message_step,
                msg_label_offset,
                message_font_size,
                message_width_scale,
                cursor_y,
                measurer,
                msg_text_style: &msg_text_style,
                math_config: &math_config,
                math_renderer,
                created_actor_index: msg
                    .to
                    .as_deref()
                    .and_then(|to| actor_lifecycle.created_actor_index(to)),
                destroyed_from_index: msg
                    .from
                    .as_deref()
                    .and_then(|from| actor_lifecycle.destroyed_actor_index(from)),
                destroyed_to_index: msg
                    .to
                    .as_deref()
                    .and_then(|to| actor_lifecycle.destroyed_actor_index(to)),
                actor_is_type_width_limited: &actor_is_type_width_limited,
            },
        ) else {
            continue;
        };

        for open in rect_stack.iter_mut() {
            let lx = message.from_x.min(message.to_x) - 11.0;
            let rx = message.from_x.max(message.to_x) + 11.0;
            open.include_min_max(lx, rx, message.line_y);
        }

        let from = message.from;
        let to = message.to;
        let line_y = message.line_y;
        cursor_y += message.cursor_step;
        if message.is_self {
            // Mermaid adds extra vertical space for self-messages to accommodate the loop curve.
            cursor_y += 30.0;
        }

        cursor_y += actor_lifecycle.apply_message_y_adjustment(msg_idx, from, to, line_y);
        edges.push(message.edge);
    }

    let bottom_margin = 2.0 * box_margin;
    let bottom_box_top_y = (cursor_y - message_step) + bottom_margin;

    actor_lifecycle.apply_created_top_actor_positions(&mut nodes);

    append_sequence_footer_actors(
        &mut nodes,
        &mut edges,
        SequenceFooterActorContext {
            actor_order: &model.actor_order,
            actors: &model.actors,
            actor_widths: &actor_widths,
            actor_centers_x: &actor_centers_x,
            actor_base_heights: &actor_base_heights,
            actor_lifecycle: &actor_lifecycle,
            actor_top_offset_y,
            bottom_box_top_y,
            mirror_actors,
            label_box_height,
            box_text_margin,
        },
    );

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
        actor_width_min,
        box_margin,
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
        nodes: &nodes,
        edges: &edges,
        block_bounds,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_left_x: &actor_left_x,
        actor_widths: &actor_widths,
        actor_box: &actor_box,
        box_margins: &box_margins,
        actor_width_min,
        actor_height,
        bottom_box_top_y,
        diagram_margin_x,
        diagram_margin_y,
        bottom_margin_adj,
        box_margin,
        has_boxes,
        mirror_actors,
        measurer,
        msg_text_style: &msg_text_style,
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
