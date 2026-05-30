use super::{LABEL_BUFFER_SPACE, LABEL_LEFT_MARGIN};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::text::{display_width, wrap_display_lines};

use super::layout::SequenceLayout;
use super::model::{SequenceArrowHead, SequenceLineStyle, SequenceMessage};
use super::render::{SequenceChars, build_lifeline_line, lifeline_char, lifeline_role};
use super::text::{SequenceLine, ensure_self_width, padded_line, trim_right, write_text_role};

pub(super) fn ensure_message_actors_visible(
    message: &SequenceMessage,
    visible_actors: &[bool],
) -> Result<()> {
    if visible_actors.get(message.from).copied().unwrap_or(false)
        && visible_actors.get(message.to).copied().unwrap_or(false)
    {
        return Ok(());
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "actor lifecycle visibility",
    })
}

fn message_label_lines(message: &SequenceMessage, max_width: usize) -> Vec<String> {
    if message.label.is_empty() {
        Vec::new()
    } else if message.wrap {
        wrap_display_lines(&message.label, max_width)
    } else {
        vec![message.label.clone()]
    }
}

pub(super) fn render_message(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    destroyed_actors: &[usize],
) -> Vec<SequenceLine> {
    let mut lines = Vec::new();
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];

    for label in message_label_lines(message, from.abs_diff(to).saturating_sub(LABEL_LEFT_MARGIN)) {
        let start = from.min(to) + LABEL_LEFT_MARGIN;
        let label_width = display_width(&label);
        let width = layout
            .total_width
            .max(start + label_width)
            .saturating_add(LABEL_BUFFER_SPACE);
        let mut line = padded_line(
            build_lifeline_line(layout, chars, active_counts, visible_actors),
            width,
        );
        write_text_role(&mut line, start, &label, AsciiColorRole::EdgeLabel);
        lines.push(trim_right(line));
    }

    let mut line = build_lifeline_line(layout, chars, active_counts, visible_actors);
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };

    if from < to {
        if destroyed_actors.contains(&message.from) {
            line.set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow);
        } else {
            line.set_role(from, chars.tee_right, AsciiColorRole::Junction);
        }
        for x in (from + 1)..to {
            line.set_role(x, style, AsciiColorRole::EdgeLine);
        }
        if destroyed_actors.contains(&message.to) && message.arrow == SequenceArrowHead::Cross {
            line.set_role(to - 1, style, AsciiColorRole::EdgeLine);
        } else {
            line.set_role(
                to - 1,
                chars.arrow_right(message.arrow),
                AsciiColorRole::EdgeArrow,
            );
        }
        if destroyed_actors.contains(&message.to) {
            line.set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow);
        } else {
            line.set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            );
        }
    } else {
        if destroyed_actors.contains(&message.to) {
            line.set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow);
        } else {
            line.set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            );
        }
        if destroyed_actors.contains(&message.to) && message.arrow == SequenceArrowHead::Cross {
            line.set_role(to + 1, style, AsciiColorRole::EdgeLine);
        } else {
            line.set_role(
                to + 1,
                chars.arrow_left(message.arrow),
                AsciiColorRole::EdgeArrow,
            );
        }
        for x in (to + 2)..from {
            line.set_role(x, style, AsciiColorRole::EdgeLine);
        }
        if destroyed_actors.contains(&message.from) {
            line.set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow);
        } else {
            line.set_role(from, chars.tee_left, AsciiColorRole::Junction);
        }
    }
    lines.push(trim_right(line));
    lines
}

pub(super) fn render_self_message(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    destroyed_actors: &[usize],
) -> Vec<SequenceLine> {
    let mut lines = Vec::new();
    let center = layout.participant_centers[message.from];
    let width = layout.self_message_width;

    for label in message_label_lines(message, layout.self_message_width + LABEL_BUFFER_SPACE) {
        let start = center + LABEL_LEFT_MARGIN;
        let needed = start + display_width(&label) + LABEL_BUFFER_SPACE;
        let mut line = ensure_self_width(
            build_lifeline_line(layout, chars, active_counts, visible_actors),
            layout,
            needed,
        );
        write_text_role(&mut line, start, &label, AsciiColorRole::EdgeLabel);
        lines.push(trim_right(line));
    }

    let mut top = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    top.set_role(center, chars.tee_right, AsciiColorRole::Junction);
    for offset in 1..width {
        top.set_role(center + offset, chars.horizontal, AsciiColorRole::EdgeLine);
    }
    top.set_role(
        center + width - 1,
        chars.self_top_right,
        AsciiColorRole::EdgeLine,
    );
    lines.push(trim_right(top));

    let mut middle = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    middle.set_role(center + width - 1, chars.vertical, AsciiColorRole::EdgeLine);
    lines.push(trim_right(middle));

    let mut bottom = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    if destroyed_actors.contains(&message.from) {
        bottom.set_role(center, chars.destroyed_mark, AsciiColorRole::EdgeArrow);
    } else {
        bottom.set_role(
            center,
            lifeline_char(message.from, chars, active_counts),
            lifeline_role(message.from, active_counts),
        );
    }
    bottom.set_role(
        center + 1,
        chars.arrow_left(message.arrow),
        AsciiColorRole::EdgeArrow,
    );
    for offset in 2..(width - 1) {
        bottom.set_role(center + offset, chars.horizontal, AsciiColorRole::EdgeLine);
    }
    bottom.set_role(
        center + width - 1,
        chars.self_bottom,
        AsciiColorRole::EdgeLine,
    );
    lines.push(trim_right(bottom));

    lines
}
