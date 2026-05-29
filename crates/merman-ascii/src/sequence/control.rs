use super::model::SequenceControlKind;
use super::render::SequenceChars;
use super::text::{padded_line, trim_right, write_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrame {
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
    pub(super) start_row: usize,
    pub(super) end_row: Option<usize>,
}

pub(super) fn render_sequence_control_frames(
    lines: Vec<String>,
    frames: &[SequenceControlFrame],
    chars: &SequenceChars,
) -> Vec<String> {
    if frames.is_empty() || lines.is_empty() {
        return lines;
    }

    let mut starts = vec![Vec::new(); lines.len()];
    let mut ends = vec![Vec::new(); lines.len()];
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

fn frame_width(frame: &SequenceControlFrame, lines: &[String]) -> usize {
    let end_row = frame.end_row.unwrap_or(frame.start_row);
    let max_row_width = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (index >= frame.start_row && index <= end_row).then_some(line.chars().count())
        })
        .max()
        .unwrap_or(0);
    let title_width = frame_title(frame).chars().count();

    max_row_width.saturating_add(3).max(title_width + 2).max(3)
}

fn render_top_border(frame: &SequenceControlFrame, width: usize, chars: &SequenceChars) -> String {
    render_border_row(
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        width,
        Some(&frame_title(frame)),
    )
}

fn render_bottom_border(width: usize, chars: &SequenceChars) -> String {
    render_border_row(
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        width,
        None,
    )
}

fn render_border_row(
    left: char,
    right: char,
    horizontal: char,
    width: usize,
    label: Option<&str>,
) -> String {
    let mut row = vec![horizontal; width];
    row[0] = left;
    row[width - 1] = right;
    if let Some(label) = label {
        write_text(&mut row, 1, label);
    }
    trim_right(row)
}

fn render_content_row(row: String, width: usize, chars: &SequenceChars) -> String {
    let mut row = padded_line(row, width);
    row[0] = chars.vertical;
    row[width - 1] = chars.vertical;
    trim_right(row)
}

fn frame_title(frame: &SequenceControlFrame) -> String {
    let keyword = frame.kind.keyword();
    if frame.label.is_empty() {
        format!(" {keyword} ")
    } else {
        format!(" {keyword} {} ", frame.label)
    }
}
