use super::model::SequenceControlKind;
use super::render::SequenceChars;
use super::text::{SequenceLine, padded_line, trim_right};
use crate::color::AsciiColorRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrame {
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
    pub(super) start_row: usize,
    pub(super) separators: Vec<SequenceControlFrameSeparator>,
    pub(super) end_row: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrameSeparator {
    pub(super) label: String,
    pub(super) row: usize,
}

impl SequenceControlFrame {
    pub(super) fn current_section_start_row(&self) -> usize {
        self.separators
            .last()
            .map(|separator| separator.row)
            .unwrap_or(self.start_row)
    }
}

pub(super) fn render_sequence_control_frames(
    lines: Vec<SequenceLine>,
    frames: &[SequenceControlFrame],
    chars: &SequenceChars,
) -> Vec<SequenceLine> {
    if frames.is_empty() || lines.is_empty() {
        return lines;
    }

    let mut starts = vec![Vec::new(); lines.len()];
    let mut ends = vec![Vec::new(); lines.len()];
    let mut separators = vec![Vec::new(); lines.len()];
    let widths = frames
        .iter()
        .map(|frame| frame_width(frame, &lines))
        .collect::<Vec<_>>();

    for (index, frame) in frames.iter().enumerate() {
        let Some(end_row) = frame.end_row else {
            continue;
        };
        if frame.start_row >= lines.len() || end_row >= lines.len() || frame.start_row > end_row {
            continue;
        }
        starts[frame.start_row].push(index);
        ends[end_row].push(index);
        for (separator_index, separator) in frame.separators.iter().enumerate() {
            if separator.row < lines.len() {
                separators[separator.row].push((index, separator_index));
            }
        }
    }

    let mut active = Vec::new();
    let mut rendered = Vec::with_capacity(lines.len() + frames.len() * 2);

    for (row_index, row) in lines.into_iter().enumerate() {
        for frame_index in &starts[row_index] {
            rendered.push(render_top_border(
                &frames[*frame_index],
                widths[*frame_index],
                chars,
            ));
            active.push(*frame_index);
        }

        for (frame_index, separator_index) in &separators[row_index] {
            rendered.push(render_separator_border(
                &frames[*frame_index],
                &frames[*frame_index].separators[*separator_index],
                widths[*frame_index],
                chars,
            ));
        }

        if let Some(frame_index) = active.last().copied() {
            rendered.push(render_content_row(row, widths[frame_index], chars));
        } else {
            rendered.push(row);
        }

        for frame_index in &ends[row_index] {
            if active.last().copied() == Some(*frame_index) {
                active.pop();
            }
            rendered.push(render_bottom_border(widths[*frame_index], chars));
        }
    }

    rendered
}

fn frame_width(frame: &SequenceControlFrame, lines: &[SequenceLine]) -> usize {
    let end_row = frame.end_row.unwrap_or(frame.start_row);
    let max_row_width = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (index >= frame.start_row && index <= end_row).then_some(line.len())
        })
        .max()
        .unwrap_or(0);
    let title_width = frame_title(frame).chars().count();
    let separator_width = frame
        .separators
        .iter()
        .map(|separator| separator_title(frame, separator).chars().count())
        .max()
        .unwrap_or(0);

    max_row_width
        .saturating_add(3)
        .max(title_width + 2)
        .max(3)
        .max(separator_width + 2)
}

fn render_top_border(
    frame: &SequenceControlFrame,
    width: usize,
    chars: &SequenceChars,
) -> SequenceLine {
    render_border_row(
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        width,
        Some(&frame_title(frame)),
    )
}

fn render_bottom_border(width: usize, chars: &SequenceChars) -> SequenceLine {
    render_border_row(
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        width,
        None,
    )
}

fn render_separator_border(
    frame: &SequenceControlFrame,
    separator: &SequenceControlFrameSeparator,
    width: usize,
    chars: &SequenceChars,
) -> SequenceLine {
    render_border_row(
        chars.tee_right,
        chars.tee_left,
        chars.horizontal,
        width,
        Some(&separator_title(frame, separator)),
    )
}

fn render_border_row(
    left: char,
    right: char,
    horizontal: char,
    width: usize,
    label: Option<&str>,
) -> SequenceLine {
    let mut row = SequenceLine::blank(width);
    for x in 0..width {
        row.set_role(x, horizontal, AsciiColorRole::SequenceFrame);
    }
    row.set_role(0, left, AsciiColorRole::SequenceFrame);
    row.set_role(width - 1, right, AsciiColorRole::SequenceFrame);
    if let Some(label) = label {
        row.write_text_role(1, label, AsciiColorRole::Text);
    }
    trim_right(row)
}

fn render_content_row(row: SequenceLine, width: usize, chars: &SequenceChars) -> SequenceLine {
    let mut row = padded_line(row, width);
    row.set_role(0, chars.vertical, AsciiColorRole::SequenceFrame);
    row.set_role(width - 1, chars.vertical, AsciiColorRole::SequenceFrame);
    trim_right(row)
}

fn frame_title(frame: &SequenceControlFrame) -> String {
    control_title(frame.kind.keyword(), &frame.label)
}

fn separator_title(
    frame: &SequenceControlFrame,
    separator: &SequenceControlFrameSeparator,
) -> String {
    control_title(
        frame
            .kind
            .separator_keyword()
            .unwrap_or_else(|| frame.kind.keyword()),
        &separator.label,
    )
}

fn control_title(keyword: &str, label: &str) -> String {
    if label.is_empty() {
        format!(" {keyword} ")
    } else {
        format!(" {keyword} {label} ")
    }
}
