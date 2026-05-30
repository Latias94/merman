use super::layout::SequenceLayout;
use crate::color::AsciiColorRole;
use crate::text::StyledLine;

pub(super) type SequenceLine = StyledLine;

pub(super) fn padded_line(mut line: SequenceLine, width: usize) -> SequenceLine {
    line.pad_to(width);
    line
}

pub(super) fn ensure_self_width(
    line: SequenceLine,
    layout: &SequenceLayout,
    needed: usize,
) -> SequenceLine {
    let width = (layout.total_width + layout.self_message_width + 1).max(needed);
    padded_line(line, width)
}

pub(super) fn write_text_role(
    line: &mut SequenceLine,
    start: usize,
    text: &str,
    role: AsciiColorRole,
) {
    line.write_text_role(start, text, role);
}

pub(super) fn trim_right(line: SequenceLine) -> SequenceLine {
    line.trim_right()
}
