use super::super::{RelationGraphLabel, RelationGraphLine, concat_relation_lines};
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationLineChars {
    line_chars: [char; 4],
    junction: char,
}

impl RelationLineChars {
    pub(crate) fn new(line_chars: [char; 4], junction: char) -> Self {
        Self {
            line_chars,
            junction,
        }
    }

    fn contains(self, ch: char) -> bool {
        self.line_chars.contains(&ch) || ch == self.junction
    }
}

pub(super) fn draw_relation_span_inclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    let start = start_y.min(end_y);
    let end = start_y.max(end_y);
    for y in start..=end {
        put_relation_char(canvas, x, y, ch, chars);
    }
}

pub(super) fn draw_relation_span_exclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    if start_y <= end_y {
        for y in start_y..end_y {
            put_relation_char(canvas, x, y, ch, chars);
        }
        return;
    }

    for y in (end_y + 1)..=start_y {
        put_relation_char(canvas, x, y, ch, chars);
    }
}

pub(crate) fn marker_line_with_role(
    marker: char,
    center: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
) -> RelationGraphLine {
    concat_relation_lines(
        vec![
            RelationGraphLine::plain(" ".repeat(center), width_profile),
            RelationGraphLine::with_role(marker.to_string(), role, width_profile),
        ],
        width_profile,
    )
}

pub(crate) fn centered_text_line_with_role(
    text: &str,
    center: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
) -> RelationGraphLine {
    let half_width = display_width_with_profile(text, width_profile) / 2;
    let left_padding = center.saturating_sub(half_width);
    concat_relation_lines(
        vec![
            RelationGraphLine::plain(" ".repeat(left_padding), width_profile),
            RelationGraphLine::with_role(text.to_string(), role, width_profile),
        ],
        width_profile,
    )
}

pub(crate) fn centered_label_lines_with_role(
    label: &RelationGraphLabel,
    center: usize,
    role: AsciiColorRole,
) -> Vec<RelationGraphLine> {
    label
        .lines()
        .iter()
        .map(|line| centered_text_line_with_role(line, center, role, label.width_profile()))
        .collect()
}

pub(crate) fn label_lines_with_role(
    label: &RelationGraphLabel,
    role: AsciiColorRole,
) -> Vec<RelationGraphLine> {
    label
        .lines()
        .iter()
        .map(|line| RelationGraphLine::with_role(line.clone(), role, label.width_profile()))
        .collect()
}

pub(crate) fn put_relation_char(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    let next = match canvas.get(x, y) {
        Some(existing) if existing == ' ' || existing == ch => ch,
        Some(existing) if chars.contains(existing) && chars.contains(ch) => chars.junction,
        _ => ch,
    };
    let role = if next == chars.junction {
        AsciiColorRole::Junction
    } else {
        AsciiColorRole::EdgeLine
    };
    canvas.set_role(x, y, next, role);
}

pub(crate) fn write_centered_relation_text(
    canvas: &mut Canvas,
    center_x: usize,
    y: usize,
    text: &str,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
) {
    let text_half_width = display_width_with_profile(text, width_profile) / 2;
    canvas.write_text_role(center_x.saturating_sub(text_half_width), y, text, role);
}

pub(crate) fn write_centered_relation_label(
    canvas: &mut Canvas,
    center_x: usize,
    start_y: usize,
    label: &RelationGraphLabel,
    role: AsciiColorRole,
) {
    for (offset, line) in label.lines().iter().enumerate() {
        write_centered_relation_text(
            canvas,
            center_x,
            start_y + offset,
            line,
            role,
            label.width_profile(),
        );
    }
}
