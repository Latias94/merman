use super::super::RelationGraphLine;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::text::display_width;

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
) -> RelationGraphLine {
    let mut line = String::new();
    line.extend(std::iter::repeat_n(' ', center));
    line.push(marker);
    let mut roles = vec![None; center];
    roles.push(Some(role));
    RelationGraphLine::new(line, roles)
}

pub(crate) fn centered_text_line_with_role(
    text: &str,
    center: usize,
    role: AsciiColorRole,
) -> RelationGraphLine {
    let mut line = String::new();
    let half_width = display_width(text) / 2;
    let left_padding = center.saturating_sub(half_width);
    line.extend(std::iter::repeat_n(' ', left_padding));
    line.push_str(text);

    let mut roles = vec![None; left_padding];
    roles.extend(std::iter::repeat_n(Some(role), text.chars().count()));
    RelationGraphLine::new(line, roles)
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
) {
    let text_half_width = display_width(text) / 2;
    canvas.write_text_role(center_x.saturating_sub(text_half_width), y, text, role);
}
