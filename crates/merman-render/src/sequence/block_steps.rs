use super::activation::SequenceActivationState;
use super::messages::{
    SequenceMessageHorizontalContext, SequenceMessageHorizontalModel,
    sequence_message_horizontal_model,
};
use super::metrics::measure_svg_like_with_html_br;
use super::notes::{SequenceNoteHorizontalContext, sequence_note_horizontal_model};
use super::{bracketize_sequence_block_label, sequence_block_label_wrap_width};
use crate::math::MathRenderer;
use crate::text::{TextMeasurer, TextStyle, wrap_label_like_mermaid_lines};
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::{SequenceDiagramRenderModel, SequenceMessage};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct BlockStepPlanContext<'a> {
    pub(super) model: &'a SequenceDiagramRenderModel,
    pub(super) actor_index: &'a HashMap<&'a str, usize>,
    pub(super) actor_centers_x: &'a [f64],
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_margin: f64,
    pub(super) activation_width: f64,
    pub(super) box_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) label_box_height: f64,
    pub(super) label_box_width: f64,
    pub(super) sequence_default_width: f64,
    pub(super) wrap_padding: f64,
    pub(super) note_margin: f64,
    pub(super) is_neo: bool,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) msg_text_style: &'a TextStyle,
    pub(super) note_text_style: &'a TextStyle,
    pub(super) math_config: &'a MermaidConfig,
    pub(super) math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

pub(super) struct SequenceBlockPlan {
    pub(super) directive_steps: HashMap<String, f64>,
}

pub(super) fn plan_sequence_blocks(ctx: BlockStepPlanContext<'_>) -> SequenceBlockPlan {
    let frame_ctx = ctx.frame_width_context();
    let widths_by_id = calculate_sequence_block_widths(ctx);
    let step_ctx = BlockStepContext {
        block_base_step_empty: (2.0 * ctx.box_margin + ctx.box_text_margin).max(0.0),
        label_box_height: ctx.label_box_height,
        wrap_padding: ctx.wrap_padding,
    };

    let directive_steps = ctx
        .model
        .messages
        .iter()
        .filter(|msg| is_block_label_directive(msg.message_type))
        .map(|msg| {
            let frame_width = widths_by_id.get(&msg.id).copied();
            (
                msg.id.clone(),
                block_label_step(msg.message_text(), frame_width, frame_ctx, step_ctx),
            )
        })
        .collect();

    SequenceBlockPlan { directive_steps }
}

pub(super) fn calculate_sequence_block_widths(
    ctx: BlockStepPlanContext<'_>,
) -> HashMap<String, f64> {
    calculate_sequence_block_bounds(&ctx.model.messages, ctx.frame_width_context())
        .into_iter()
        .map(|(id, bounds)| (id, bounds.width))
        .collect()
}

pub(super) fn is_block_start(message_type: i32) -> bool {
    matches!(message_type, 10 | 12 | 15 | 19 | 27 | 30 | 32)
}

pub(super) fn is_block_section(message_type: i32) -> bool {
    matches!(message_type, 13 | 20 | 28)
}

pub(super) fn is_block_end(message_type: i32) -> bool {
    matches!(message_type, 11 | 14 | 16 | 21 | 29 | 31)
}

fn is_block_label_directive(message_type: i32) -> bool {
    is_block_start(message_type) || is_block_section(message_type)
}

fn block_label_step(
    raw_label: &str,
    frame_width: Option<f64>,
    frame_ctx: BlockFrameWidthContext<'_>,
    step_ctx: BlockStepContext,
) -> f64 {
    if raw_label.trim().is_empty() {
        return step_ctx.block_base_step_empty;
    }

    let label = bracketize_sequence_block_label(raw_label);
    let measured_label = match frame_width {
        Some(width) => wrap_label_like_mermaid_lines(
            &label,
            frame_ctx.measurer,
            frame_ctx.msg_text_style,
            sequence_block_label_wrap_width(width, step_ctx.wrap_padding),
        )
        .join("<br/>"),
        None => label,
    };
    let (_, height) = measure_svg_like_with_html_br(
        frame_ctx.measurer,
        &measured_label,
        frame_ctx.msg_text_style,
    );
    step_ctx.block_base_step_empty + height.max(step_ctx.label_box_height)
}

#[derive(Clone, Copy)]
struct BlockFrameWidthContext<'a> {
    actor_index: &'a HashMap<&'a str, usize>,
    actor_centers_x: &'a [f64],
    actor_widths: &'a [f64],
    actor_margin: f64,
    activation_width: f64,
    label_box_width: f64,
    sequence_default_width: f64,
    wrap_padding: f64,
    note_margin: f64,
    is_neo: bool,
    measurer: &'a dyn TextMeasurer,
    msg_text_style: &'a TextStyle,
    note_text_style: &'a TextStyle,
    math_config: &'a MermaidConfig,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

#[derive(Clone, Copy)]
struct BlockStepContext {
    block_base_step_empty: f64,
    label_box_height: f64,
    wrap_padding: f64,
}

impl<'a> BlockStepPlanContext<'a> {
    fn frame_width_context(self) -> BlockFrameWidthContext<'a> {
        BlockFrameWidthContext {
            actor_index: self.actor_index,
            actor_centers_x: self.actor_centers_x,
            actor_widths: self.actor_widths,
            actor_margin: self.actor_margin,
            activation_width: self.activation_width,
            label_box_width: self.label_box_width,
            sequence_default_width: self.sequence_default_width,
            wrap_padding: self.wrap_padding,
            note_margin: self.note_margin,
            is_neo: self.is_neo,
            measurer: self.measurer,
            msg_text_style: self.msg_text_style,
            note_text_style: self.note_text_style,
            math_config: self.math_config,
            math_renderer: self.math_renderer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BlockHorizontalBounds {
    from: f64,
    to: f64,
    width: f64,
}

impl BlockHorizontalBounds {
    fn empty() -> Self {
        Self {
            from: f64::INFINITY,
            to: f64::NEG_INFINITY,
            width: 0.0,
        }
    }

    fn include_note(&mut self, start_x: f64, width: f64, label_box_width: f64) {
        self.from = self.from.min(start_x);
        self.to = self.to.max(start_x + width);
        self.width = self.width.max((self.to - self.from).abs()) - label_box_width;
    }

    fn include_message(
        &mut self,
        msg: &SequenceMessage,
        model: SequenceMessageHorizontalModel,
        ctx: BlockFrameWidthContext<'_>,
    ) {
        if model.start_x == model.stop_x {
            let Some(actor_id) = msg.from.as_deref() else {
                return;
            };
            let Some(actor_index) = ctx.actor_index.get(actor_id).copied() else {
                return;
            };
            let Some(actor_center_x) = ctx.actor_centers_x.get(actor_index).copied() else {
                return;
            };
            let Some(actor_width) = ctx.actor_widths.get(actor_index).copied() else {
                return;
            };
            let actor_left_x = actor_center_x - actor_width / 2.0;
            self.from = self
                .from
                .min(actor_left_x - model.width / 2.0)
                .min(actor_left_x - actor_width / 2.0);
            self.to = self
                .to
                .max(actor_left_x + model.width / 2.0)
                .max(actor_left_x + actor_width / 2.0);
            self.width = self.width.max((self.to - self.from).abs()) - ctx.label_box_width;
        } else {
            self.from = self.from.min(model.start_x);
            self.to = self.to.max(model.stop_x);
            self.width = self.width.max(model.width) - ctx.label_box_width;
        }
    }
}

#[derive(Debug)]
struct OpenBlock {
    aliases: Vec<String>,
    bounds: BlockHorizontalBounds,
}

impl OpenBlock {
    fn new(id: String) -> Self {
        Self {
            aliases: vec![id],
            bounds: BlockHorizontalBounds::empty(),
        }
    }
}

fn calculate_sequence_block_bounds(
    messages: &[SequenceMessage],
    ctx: BlockFrameWidthContext<'_>,
) -> HashMap<String, BlockHorizontalBounds> {
    let mut completed = HashMap::new();
    let mut stack: Vec<OpenBlock> = Vec::new();
    let mut activation_state = SequenceActivationState::new(ctx.activation_width);

    for msg in messages {
        if is_block_start(msg.message_type) {
            stack.push(OpenBlock::new(msg.id.clone()));
            continue;
        }
        if is_block_section(msg.message_type) {
            if !msg.message_text().is_empty()
                && let Some(current) = stack.last_mut()
            {
                current.aliases.push(msg.id.clone());
            }
            continue;
        }
        if is_block_end(msg.message_type) {
            if let Some(current) = stack.pop() {
                for alias in current.aliases {
                    completed.insert(alias, current.bounds);
                }
            }
            continue;
        }
        if activation_state.handle_directive(msg, ctx.actor_index, ctx.actor_centers_x) {
            continue;
        }
        if stack.is_empty() {
            continue;
        }

        if msg.placement.is_some() {
            let Some(note) = sequence_note_horizontal_model(
                msg,
                SequenceNoteHorizontalContext {
                    actor_index: ctx.actor_index,
                    actor_centers_x: ctx.actor_centers_x,
                    actor_widths: ctx.actor_widths,
                    actor_margin: ctx.actor_margin,
                    note_default_width: ctx.sequence_default_width,
                    note_margin: ctx.note_margin,
                    wrap_padding: ctx.wrap_padding,
                    measurer: ctx.measurer,
                    note_text_style: ctx.note_text_style,
                    math_config: ctx.math_config,
                    math_renderer: ctx.math_renderer,
                },
            ) else {
                continue;
            };
            for current in &mut stack {
                current
                    .bounds
                    .include_note(note.start_x, note.width, ctx.label_box_width);
            }
            continue;
        }

        let Some(message) = sequence_message_horizontal_model(
            msg,
            SequenceMessageHorizontalContext {
                actor_index: ctx.actor_index,
                actor_centers_x: ctx.actor_centers_x,
                activation_state: &activation_state,
                default_width: ctx.sequence_default_width,
                wrap_padding: ctx.wrap_padding,
                is_neo: ctx.is_neo,
                measurer: ctx.measurer,
                msg_text_style: ctx.msg_text_style,
            },
        ) else {
            continue;
        };
        for current in &mut stack {
            current.bounds.include_message(msg, message, ctx);
        }
    }

    completed
}

#[cfg(test)]
mod tests {
    use super::{
        BlockFrameWidthContext, BlockStepContext, block_label_step, calculate_sequence_block_bounds,
    };
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_core::MermaidConfig;
    use merman_core::diagrams::sequence::{SequenceMessage, SequenceMessagePayload};
    use std::collections::HashMap;

    struct ExpectedBlockLabelMeasurer;

    impl TextMeasurer for ExpectedBlockLabelMeasurer {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            assert_eq!(text, "[[Action 1]]");
            TextMetrics {
                width: 100.0,
                height: 16.0,
                line_count: 1,
            }
        }
    }

    fn message(
        id: &str,
        message_type: i32,
        from: Option<&str>,
        to: Option<&str>,
        text: &str,
    ) -> SequenceMessage {
        SequenceMessage {
            id: id.to_string(),
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            message_type,
            message: SequenceMessagePayload::Text(text.to_string()),
            wrap: false,
            activate: false,
            placement: (message_type == 2).then_some(1),
            central_connection: 0,
        }
    }

    fn bounds(messages: &[SequenceMessage]) -> HashMap<String, super::BlockHorizontalBounds> {
        let actor_index = HashMap::from([("A", 0), ("B", 1), ("C", 2)]);
        let centers = [100.0, 300.0, 500.0];
        let widths = [80.0, 80.0, 80.0];
        let measurer = DeterministicTextMeasurer::default();
        let msg_style = TextStyle::default();
        let note_style = TextStyle::default();
        let math_config = MermaidConfig::default();

        calculate_sequence_block_bounds(
            messages,
            BlockFrameWidthContext {
                actor_index: &actor_index,
                actor_centers_x: &centers,
                actor_widths: &widths,
                actor_margin: 40.0,
                activation_width: 10.0,
                label_box_width: 0.0,
                sequence_default_width: 0.0,
                wrap_padding: 0.0,
                note_margin: 0.0,
                is_neo: false,
                measurer: &measurer,
                msg_text_style: &msg_style,
                note_text_style: &note_style,
                math_config: &math_config,
                math_renderer: None,
            },
        )
    }

    #[test]
    fn activation_state_crosses_nested_block_boundaries() {
        let messages = vec![
            message("active", 17, Some("A"), Some("A"), ""),
            message("outer", 10, None, None, "outer"),
            message("inner", 15, None, None, "inner"),
            message("inner-end", 16, None, None, ""),
            message("edge", 5, Some("A"), Some("B"), ""),
            message("outer-end", 11, None, None, ""),
            message("inactive", 18, Some("A"), Some("A"), ""),
        ];

        let calculated = bounds(&messages);
        let outer = &calculated["outer"];

        assert_eq!((outer.from, outer.to, outer.width), (105.0, 299.0, 194.0));
        assert_eq!(calculated["inner"].width, 0.0);
    }

    #[test]
    fn ordinary_message_updates_bounds_after_self_message_and_note() {
        let messages = vec![
            message("loop", 10, None, None, "loop"),
            message("self", 5, Some("A"), Some("A"), ""),
            message("note", 2, Some("A"), Some("A"), ""),
            message("edge", 5, Some("B"), Some("C"), ""),
            message("end", 11, None, None, ""),
        ];

        let calculated = bounds(&messages);
        let loop_bounds = &calculated["loop"];

        assert_eq!(loop_bounds.from, 20.0);
        assert_eq!(loop_bounds.to, 499.0);
        assert_eq!(loop_bounds.width, 198.0);
    }

    #[test]
    fn ordinary_message_bounds_survive_later_self_message_and_note() {
        let messages = vec![
            message("loop", 10, None, None, "loop"),
            message("far", 5, Some("A"), Some("C"), ""),
            message("self", 5, Some("B"), Some("B"), ""),
            message("note", 2, Some("B"), Some("B"), ""),
            message("end", 11, None, None, ""),
        ];

        let calculated = bounds(&messages);
        let loop_bounds = &calculated["loop"];

        assert_eq!(loop_bounds.from, 101.0);
        assert_eq!(loop_bounds.to, 499.0);
        assert_eq!(loop_bounds.width, 398.0);
    }

    #[test]
    fn nested_blocks_and_section_aliases_share_final_width() {
        let messages = vec![
            message("outer", 10, None, None, "outer"),
            message("alt", 12, None, None, "first"),
            message("near", 5, Some("A"), Some("B"), ""),
            message("else", 13, None, None, "otherwise"),
            message("far", 5, Some("A"), Some("C"), ""),
            message("alt-end", 14, None, None, ""),
            message("outer-end", 11, None, None, ""),
        ];

        let calculated = bounds(&messages);

        assert_eq!(calculated["alt"], calculated["else"]);
        assert_eq!(calculated["alt"].width, 398.0);
        assert_eq!(calculated["outer"].width, 398.0);
    }

    #[test]
    fn empty_block_keeps_zero_width() {
        let messages = vec![
            message("empty", 10, None, None, "empty"),
            message("end", 11, None, None, ""),
        ];

        assert_eq!(bounds(&messages)["empty"].width, 0.0);
    }

    #[test]
    fn layout_measures_an_added_bracket_pair_around_a_bracketed_source_title() {
        let actor_index: HashMap<&str, usize> = HashMap::new();
        let actor_centers_x = [];
        let actor_widths = [];
        let measurer = ExpectedBlockLabelMeasurer;
        let text_style = TextStyle::default();
        let math_config = MermaidConfig::default();

        let step = block_label_step(
            "[Action 1]",
            None,
            BlockFrameWidthContext {
                actor_index: &actor_index,
                actor_centers_x: &actor_centers_x,
                actor_widths: &actor_widths,
                actor_margin: 0.0,
                activation_width: 0.0,
                label_box_width: 0.0,
                sequence_default_width: 0.0,
                wrap_padding: 0.0,
                note_margin: 0.0,
                is_neo: false,
                measurer: &measurer,
                msg_text_style: &text_style,
                note_text_style: &text_style,
                math_config: &math_config,
                math_renderer: None,
            },
            BlockStepContext {
                block_base_step_empty: 0.0,
                label_box_height: 0.0,
                wrap_padding: 0.0,
            },
        );

        assert_eq!(step, 16.0);
    }
}
