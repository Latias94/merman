use super::SEQUENCE_FRAME_SIDE_PAD_PX;
use super::activation::SequenceActivationState;
use super::actors::{
    SequenceActorLifecycle, SequenceActorLifecycleContext, SequenceFooterActorContext,
    SequenceTopActorContext, append_sequence_footer_actors, append_sequence_top_actors,
    sequence_actor_is_type_width_limited,
};
use super::block_steps::{
    BlockStepPlanContext, SequenceBlockPlan, is_block_end, is_block_section, is_block_start,
    plan_sequence_blocks,
};
use super::messages::{SequenceMessageLayoutContext, layout_sequence_message};
use super::notes::{SequenceNoteLayoutContext, layout_sequence_note};
use super::rect::SequenceRectOpen;
use crate::math::MathRenderer;
use crate::model::{LayoutEdge, LayoutNode};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::{SequenceDiagramRenderModel, SequenceMessage};
use std::collections::HashMap;

pub(super) struct SequenceLayoutGraphContext<'a> {
    pub(super) model: &'a SequenceDiagramRenderModel,
    pub(super) actor_index: &'a HashMap<&'a str, usize>,
    pub(super) actor_centers_x: &'a [f64],
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_base_heights: &'a [f64],
    pub(super) actor_top_offset_y: f64,
    pub(super) max_actor_layout_height: f64,
    pub(super) sequence_default_width: f64,
    pub(super) actor_height: f64,
    pub(super) actor_margin: f64,
    pub(super) box_margin: f64,
    pub(super) note_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) label_box_height: f64,
    pub(super) label_box_width: f64,
    pub(super) right_angles: bool,
    pub(super) is_neo: bool,
    pub(super) wrap_padding: f64,
    pub(super) mirror_actors: bool,
    pub(super) activation_width: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) msg_text_style: &'a TextStyle,
    pub(super) note_text_style: &'a TextStyle,
    pub(super) math_config: &'a MermaidConfig,
    pub(super) math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

pub(super) struct SequenceLayoutGraph {
    pub(super) nodes: Vec<LayoutNode>,
    pub(super) edges: Vec<LayoutEdge>,
    pub(super) bottom_box_top_y: f64,
}

struct SequenceLayoutLoopState<'a> {
    cursor_y: f64,
    block_stopy_stack: Vec<Option<f64>>,
    rect_stack: Vec<SequenceRectOpen>,
    activation_state: SequenceActivationState,
    actor_lifecycle: SequenceActorLifecycle<'a>,
}

impl<'a> SequenceLayoutLoopState<'a> {
    fn new(ctx: &SequenceLayoutGraphContext<'a>) -> Self {
        let activation_state = SequenceActivationState::new(ctx.activation_width);
        let actor_lifecycle = SequenceActorLifecycle::new(SequenceActorLifecycleContext {
            actor_index: ctx.actor_index,
            actor_base_heights: ctx.actor_base_heights,
            created_actors: &ctx.model.created_actors,
            destroyed_actors: &ctx.model.destroyed_actors,
            actor_height: ctx.actor_height,
        });

        Self {
            cursor_y: ctx.actor_top_offset_y + ctx.max_actor_layout_height,
            block_stopy_stack: Vec::new(),
            rect_stack: Vec::new(),
            activation_state,
            actor_lifecycle,
        }
    }

    fn open_block(&mut self) {
        self.block_stopy_stack.push(None);
    }

    fn include_inserted_bottom(&mut self, inserted_bottom_y: f64, box_margin: f64) {
        include_block_stopy(&mut self.block_stopy_stack, inserted_bottom_y, box_margin);
    }

    fn close_block(&mut self, box_margin: f64) {
        self.cursor_y = close_block_cursor(&mut self.block_stopy_stack, self.cursor_y, box_margin);
    }
}

fn include_block_stopy(
    block_stopy_stack: &mut [Option<f64>],
    inserted_bottom_y: f64,
    box_margin: f64,
) {
    let depth = block_stopy_stack.len();
    for (index, stopy) in block_stopy_stack.iter_mut().enumerate() {
        let nested_margin = (depth - index) as f64 * box_margin;
        let candidate = inserted_bottom_y + nested_margin;
        *stopy = Some(stopy.map_or(candidate, |current| current.max(candidate)));
    }
}

fn close_block_cursor(
    block_stopy_stack: &mut Vec<Option<f64>>,
    cursor_y: f64,
    box_margin: f64,
) -> f64 {
    block_stopy_stack
        .pop()
        .flatten()
        .unwrap_or(cursor_y + box_margin)
}

fn handle_sequence_directive<'a>(
    msg: &'a SequenceMessage,
    directive_steps: &HashMap<String, f64>,
    state: &mut SequenceLayoutLoopState<'a>,
    ctx: &SequenceLayoutGraphContext<'a>,
) -> bool {
    if state
        .activation_state
        .handle_directive(msg, ctx.actor_index, ctx.actor_centers_x)
    {
        return true;
    }

    match msg.message_type {
        message_type if is_block_start(message_type) => {
            state.cursor_y += directive_steps
                .get(msg.id.as_str())
                .copied()
                .unwrap_or_default();
            state.open_block();
            true
        }
        message_type if is_block_end(message_type) => {
            state.close_block(ctx.box_margin);
            true
        }
        message_type if is_block_section(message_type) => {
            state.cursor_y += directive_steps
                .get(msg.id.as_str())
                .copied()
                .unwrap_or_default();
            true
        }
        _ => false,
    }
}

fn handle_sequence_rect(
    msg: &SequenceMessage,
    state: &mut SequenceLayoutLoopState<'_>,
    box_margin: f64,
    rect_step_start: f64,
    actor_centers_x: &[f64],
    nodes: &mut Vec<LayoutNode>,
) -> bool {
    match msg.message_type {
        22 => {
            state.rect_stack.push(SequenceRectOpen::new(
                msg.id.clone(),
                state.cursor_y + box_margin,
            ));
            state.cursor_y += rect_step_start;
            state.open_block();
            true
        }
        23 => {
            state.close_block(box_margin);
            if let Some(open) = state.rect_stack.pop() {
                let closed = open.close(actor_centers_x);
                nodes.push(closed.node);

                if let Some(parent) = state.rect_stack.last_mut() {
                    parent.include_min_max(
                        closed.left - SEQUENCE_FRAME_SIDE_PAD_PX,
                        closed.right + SEQUENCE_FRAME_SIDE_PAD_PX,
                        closed.bottom,
                    );
                }
            }
            true
        }
        _ => false,
    }
}

fn handle_sequence_note(
    msg: &SequenceMessage,
    state: &mut SequenceLayoutLoopState<'_>,
    ctx: &SequenceLayoutGraphContext<'_>,
    nodes: &mut Vec<LayoutNode>,
) -> bool {
    if msg.message_type != 2 {
        return false;
    }

    let Some(note) = layout_sequence_note(
        msg,
        SequenceNoteLayoutContext {
            actor_index: ctx.actor_index,
            actor_centers_x: ctx.actor_centers_x,
            actor_widths: ctx.actor_widths,
            actor_margin: ctx.actor_margin,
            note_default_width: ctx.sequence_default_width,
            note_margin: ctx.note_margin,
            wrap_padding: ctx.wrap_padding,
            box_margin: ctx.box_margin,
            cursor_y: state.cursor_y,
            measurer: ctx.measurer,
            note_text_style: ctx.note_text_style,
            math_config: ctx.math_config,
            math_renderer: ctx.math_renderer,
        },
    ) else {
        return true;
    };

    include_rect_stack_bounds(
        &mut state.rect_stack,
        note.rect_min_x,
        note.rect_max_x,
        note.rect_max_y,
    );
    state.include_inserted_bottom(note.rect_max_y, ctx.box_margin);

    nodes.push(note.node);
    state.cursor_y += note.cursor_step;
    true
}

fn handle_sequence_message(
    msg: &SequenceMessage,
    msg_idx: usize,
    state: &mut SequenceLayoutLoopState<'_>,
    ctx: &SequenceLayoutGraphContext<'_>,
    edges: &mut Vec<LayoutEdge>,
    actor_is_type_width_limited: &dyn Fn(&str) -> bool,
) -> bool {
    let Some(message) = layout_sequence_message(
        msg,
        SequenceMessageLayoutContext {
            actor_index: ctx.actor_index,
            actor_centers_x: ctx.actor_centers_x,
            actor_widths: ctx.actor_widths,
            activation_state: &state.activation_state,
            msg_idx,
            actor_width_min: ctx.sequence_default_width,
            box_margin: ctx.box_margin,
            wrap_padding: ctx.wrap_padding,
            is_neo: ctx.is_neo,
            right_angles: ctx.right_angles,
            message_font_size: ctx.msg_text_style.font_size,
            cursor_y: state.cursor_y,
            measurer: ctx.measurer,
            msg_text_style: ctx.msg_text_style,
            math_config: ctx.math_config,
            math_renderer: ctx.math_renderer,
            created_actor_index: msg
                .to
                .as_deref()
                .and_then(|to| state.actor_lifecycle.created_actor_index(to)),
            destroyed_from_index: msg
                .from
                .as_deref()
                .and_then(|from| state.actor_lifecycle.destroyed_actor_index(from)),
            destroyed_to_index: msg
                .to
                .as_deref()
                .and_then(|to| state.actor_lifecycle.destroyed_actor_index(to)),
            actor_is_type_width_limited,
        },
    ) else {
        return false;
    };
    let super::messages::SequenceMessageLayout {
        edge,
        from_x,
        to_x,
        line_y,
        inserted_bottom_y,
        cursor_step,
    } = message;

    include_rect_stack_bounds(
        &mut state.rect_stack,
        from_x.min(to_x) - SEQUENCE_FRAME_SIDE_PAD_PX,
        from_x.max(to_x) + SEQUENCE_FRAME_SIDE_PAD_PX,
        inserted_bottom_y,
    );
    state.include_inserted_bottom(inserted_bottom_y, ctx.box_margin);

    state.cursor_y += cursor_step;
    state.cursor_y += state
        .actor_lifecycle
        .apply_message_y_adjustment(msg_idx, &edge.from, &edge.to, line_y);
    edges.push(edge);
    true
}

pub(super) fn build_sequence_layout_graph(
    ctx: SequenceLayoutGraphContext<'_>,
) -> SequenceLayoutGraph {
    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut edges: Vec<LayoutEdge> = Vec::new();

    append_sequence_top_actors(
        &mut nodes,
        SequenceTopActorContext {
            actor_order: &ctx.model.actor_order,
            actors: &ctx.model.actors,
            actor_widths: ctx.actor_widths,
            actor_centers_x: ctx.actor_centers_x,
            actor_base_heights: ctx.actor_base_heights,
            actor_top_offset_y: ctx.actor_top_offset_y,
            label_box_height: ctx.label_box_height,
        },
    );

    let SequenceBlockPlan { directive_steps } = plan_sequence_blocks(BlockStepPlanContext {
        model: ctx.model,
        actor_index: ctx.actor_index,
        actor_centers_x: ctx.actor_centers_x,
        actor_widths: ctx.actor_widths,
        actor_margin: ctx.actor_margin,
        activation_width: ctx.activation_width,
        box_margin: ctx.box_margin,
        box_text_margin: ctx.box_text_margin,
        label_box_height: ctx.label_box_height,
        label_box_width: ctx.label_box_width,
        sequence_default_width: ctx.sequence_default_width,
        wrap_padding: ctx.wrap_padding,
        note_margin: ctx.note_margin,
        is_neo: ctx.is_neo,
        measurer: ctx.measurer,
        msg_text_style: ctx.msg_text_style,
        note_text_style: ctx.note_text_style,
        math_config: ctx.math_config,
        math_renderer: ctx.math_renderer,
    });

    let rect_step_start = 2.0 * ctx.box_margin;
    let mut state = SequenceLayoutLoopState::new(&ctx);
    let actor_is_type_width_limited = |actor_id: &str| -> bool {
        sequence_actor_is_type_width_limited(&ctx.model.actors, actor_id)
    };

    for (msg_idx, msg) in ctx.model.messages.iter().enumerate() {
        if handle_sequence_directive(msg, &directive_steps, &mut state, &ctx) {
            continue;
        }

        if handle_sequence_rect(
            msg,
            &mut state,
            ctx.box_margin,
            rect_step_start,
            ctx.actor_centers_x,
            &mut nodes,
        ) {
            continue;
        }

        if handle_sequence_note(msg, &mut state, &ctx, &mut nodes) {
            continue;
        }

        if handle_sequence_message(
            msg,
            msg_idx,
            &mut state,
            &ctx,
            &mut edges,
            &actor_is_type_width_limited,
        ) {
            continue;
        }
    }

    let bottom_margin = 2.0 * ctx.box_margin;
    let bottom_box_top_y = state.cursor_y + bottom_margin;

    state
        .actor_lifecycle
        .apply_created_top_actor_positions(&mut nodes);

    append_sequence_footer_actors(
        &mut nodes,
        &mut edges,
        SequenceFooterActorContext {
            actor_order: &ctx.model.actor_order,
            actors: &ctx.model.actors,
            actor_widths: ctx.actor_widths,
            actor_centers_x: ctx.actor_centers_x,
            actor_base_heights: ctx.actor_base_heights,
            actor_lifecycle: &state.actor_lifecycle,
            actor_top_offset_y: ctx.actor_top_offset_y,
            bottom_box_top_y,
            mirror_actors: ctx.mirror_actors,
            label_box_height: ctx.label_box_height,
            box_text_margin: ctx.box_text_margin,
        },
    );

    SequenceLayoutGraph {
        nodes,
        edges,
        bottom_box_top_y,
    }
}

fn include_rect_stack_bounds(
    rect_stack: &mut [SequenceRectOpen],
    min_x: f64,
    max_x: f64,
    max_y: f64,
) {
    for open in rect_stack.iter_mut() {
        open.include_min_max(min_x, max_x, max_y);
    }
}

#[cfg(test)]
mod tests {
    use super::{close_block_cursor, include_block_stopy};

    #[test]
    fn block_stopy_uses_configured_margin_and_self_message_bounds() {
        let mut ordinary = vec![None];
        include_block_stopy(&mut ordinary, 100.0, 7.0);
        assert_eq!(close_block_cursor(&mut ordinary, 100.0, 7.0), 107.0);

        let mut self_message = vec![None];
        include_block_stopy(&mut self_message, 160.0, 7.0);
        assert_eq!(close_block_cursor(&mut self_message, 130.0, 7.0), 167.0);
    }

    #[test]
    fn nested_blocks_keep_numeric_depth_margins() {
        let mut stack = vec![None, None];
        include_block_stopy(&mut stack, 160.0, 7.0);

        assert_eq!(close_block_cursor(&mut stack, 130.0, 7.0), 167.0);
        assert_eq!(close_block_cursor(&mut stack, 167.0, 7.0), 174.0);
    }

    #[test]
    fn later_content_consumes_an_earlier_self_message_overhang() {
        let mut stack = vec![None];
        include_block_stopy(&mut stack, 160.0, 10.0);
        include_block_stopy(&mut stack, 220.0, 10.0);

        assert_eq!(close_block_cursor(&mut stack, 220.0, 10.0), 230.0);
    }
}
