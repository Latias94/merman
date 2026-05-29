use super::{LABEL_BUFFER_SPACE, LABEL_LEFT_MARGIN};
use crate::error::{AsciiError, Result};
use crate::text::{display_width, wrap_display_lines};

use super::layout::SequenceLayout;
use super::model::{SequenceArrowHead, SequenceLineStyle, SequenceMessage};
use super::render::{SequenceChars, build_lifeline, lifeline_char};
use super::text::{ensure_self_width, padded_line, trim_right, write_text};

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
) -> Vec<String> {
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
            build_lifeline(layout, chars, active_counts, visible_actors),
            width,
        );
        write_text(&mut line, start, &label);
        lines.push(trim_right(line));
    }

    let mut line = build_lifeline(layout, chars, active_counts, visible_actors)
        .chars()
        .collect::<Vec<_>>();
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };

    if from < to {
        line[from] = if destroyed_actors.contains(&message.from) {
            chars.destroyed_mark
        } else {
            chars.tee_right
        };
        for cell in line.iter_mut().take(to).skip(from + 1) {
            *cell = style;
        }
        line[to - 1] = if destroyed_actors.contains(&message.to)
            && message.arrow == SequenceArrowHead::Cross
        {
            style
        } else {
            chars.arrow_right(message.arrow)
        };
        line[to] = if destroyed_actors.contains(&message.to) {
            chars.destroyed_mark
        } else {
            lifeline_char(message.to, chars, active_counts)
        };
    } else {
        line[to] = if destroyed_actors.contains(&message.to) {
            chars.destroyed_mark
        } else {
            lifeline_char(message.to, chars, active_counts)
        };
        line[to + 1] = if destroyed_actors.contains(&message.to)
            && message.arrow == SequenceArrowHead::Cross
        {
            style
        } else {
            chars.arrow_left(message.arrow)
        };
        for cell in line.iter_mut().take(from).skip(to + 2) {
            *cell = style;
        }
        line[from] = if destroyed_actors.contains(&message.from) {
            chars.destroyed_mark
        } else {
            chars.tee_left
        };
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
) -> Vec<String> {
    let mut lines = Vec::new();
    let center = layout.participant_centers[message.from];
    let width = layout.self_message_width;

    for label in message_label_lines(message, layout.self_message_width + LABEL_BUFFER_SPACE) {
        let start = center + LABEL_LEFT_MARGIN;
        let needed = start + display_width(&label) + LABEL_BUFFER_SPACE;
        let mut line = ensure_self_width(
            build_lifeline(layout, chars, active_counts, visible_actors),
            layout,
            needed,
        );
        write_text(&mut line, start, &label);
        lines.push(trim_right(line));
    }

    let mut top = ensure_self_width(
        build_lifeline(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    top[center] = chars.tee_right;
    for offset in 1..width {
        top[center + offset] = chars.horizontal;
    }
    top[center + width - 1] = chars.self_top_right;
    lines.push(trim_right(top));

    let mut middle = ensure_self_width(
        build_lifeline(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    middle[center + width - 1] = chars.vertical;
    lines.push(trim_right(middle));

    let mut bottom = ensure_self_width(
        build_lifeline(layout, chars, active_counts, visible_actors),
        layout,
        0,
    );
    bottom[center] = if destroyed_actors.contains(&message.from) {
        chars.destroyed_mark
    } else {
        lifeline_char(message.from, chars, active_counts)
    };
    bottom[center + 1] = chars.arrow_left(message.arrow);
    for offset in 2..(width - 1) {
        bottom[center + offset] = chars.horizontal;
    }
    bottom[center + width - 1] = chars.self_bottom;
    lines.push(trim_right(bottom));

    lines
}
