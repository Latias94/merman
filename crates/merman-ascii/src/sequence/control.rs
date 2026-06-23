use super::model::SequenceControlKind;
use super::render::SequenceChars;
use super::text::{SequenceLine, padded_line, trim_right};
use crate::color::AsciiColorRole;
use crate::text::display_width;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceControlFrameNode {
    frame_index: usize,
    children: Vec<SequenceControlFrameNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SequenceControlBodyRow {
    Content(SequenceLine),
    Separator(usize),
}

pub(super) fn render_sequence_control_frames(
    lines: Vec<SequenceLine>,
    frames: &[SequenceControlFrame],
    chars: &SequenceChars,
) -> Vec<SequenceLine> {
    if frames.is_empty() || lines.is_empty() {
        return lines;
    }

    let tree = control_frame_tree(frames, lines.len());
    if tree.is_empty() {
        return lines;
    }

    render_control_range(&lines, frames, &tree, 0, lines.len(), chars)
}

fn render_control_range(
    lines: &[SequenceLine],
    frames: &[SequenceControlFrame],
    nodes: &[SequenceControlFrameNode],
    start_row: usize,
    end_row: usize,
    chars: &SequenceChars,
) -> Vec<SequenceLine> {
    let mut rendered = Vec::new();
    let mut row = start_row;

    for node in nodes {
        let frame = &frames[node.frame_index];
        let Some(node_end) = valid_frame_end_row(frame, lines.len()) else {
            continue;
        };

        if row < frame.start_row {
            rendered.extend(lines[row..frame.start_row].iter().cloned());
        }
        rendered.extend(render_frame_node(node, frames, lines, chars));
        row = node_end + 1;
    }

    if row < end_row {
        rendered.extend(lines[row..end_row].iter().cloned());
    }
    rendered
}

fn render_frame_node(
    node: &SequenceControlFrameNode,
    frames: &[SequenceControlFrame],
    lines: &[SequenceLine],
    chars: &SequenceChars,
) -> Vec<SequenceLine> {
    let frame = &frames[node.frame_index];
    let body_rows = render_frame_body(node, frames, lines, chars);
    let width = frame_width(frame, &body_rows);
    let mut rendered = Vec::with_capacity(body_rows.len() + 2);
    rendered.push(render_top_border(frame, width, chars));

    for row in body_rows {
        match row {
            SequenceControlBodyRow::Content(line) => {
                rendered.push(render_content_row(line, width, chars));
            }
            SequenceControlBodyRow::Separator(separator_index) => {
                rendered.push(render_separator_border(
                    frame,
                    &frame.separators[separator_index],
                    width,
                    chars,
                ));
            }
        }
    }

    rendered.push(render_bottom_border(width, chars));
    rendered
}

fn render_frame_body(
    node: &SequenceControlFrameNode,
    frames: &[SequenceControlFrame],
    lines: &[SequenceLine],
    chars: &SequenceChars,
) -> Vec<SequenceControlBodyRow> {
    let frame = &frames[node.frame_index];
    let end_row = frame
        .end_row
        .expect("control frame tree should only contain closed frames");
    let mut body_rows = Vec::new();
    let mut row = frame.start_row;
    let mut child_index = 0;
    let mut separator_index = 0;

    while row <= end_row {
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            body_rows.push(SequenceControlBodyRow::Separator(separator_index));
            separator_index += 1;
        }

        if let Some(child) = node.children.get(child_index) {
            let child_frame = &frames[child.frame_index];
            if child_frame.start_row == row {
                body_rows.extend(
                    indent_child_frame(render_frame_node(child, frames, lines, chars))
                        .into_iter()
                        .map(SequenceControlBodyRow::Content),
                );
                row = child_frame
                    .end_row
                    .expect("control frame tree should only contain closed frames")
                    + 1;
                child_index += 1;
                continue;
            }
        }

        body_rows.push(SequenceControlBodyRow::Content(lines[row].clone()));
        row += 1;
    }

    body_rows
}

fn control_frame_tree(
    frames: &[SequenceControlFrame],
    line_count: usize,
) -> Vec<SequenceControlFrameNode> {
    let mut roots = Vec::new();
    let mut stack: Vec<SequenceControlFrameNode> = Vec::new();

    for (frame_index, frame) in frames.iter().enumerate() {
        if valid_frame_end_row(frame, line_count).is_none() {
            continue;
        }

        while stack.last().is_some_and(|node| {
            let active = &frames[node.frame_index];
            active
                .end_row
                .is_some_and(|end_row| end_row < frame.start_row)
        }) {
            complete_node(&mut roots, &mut stack);
        }

        stack.push(SequenceControlFrameNode {
            frame_index,
            children: Vec::new(),
        });
    }

    while !stack.is_empty() {
        complete_node(&mut roots, &mut stack);
    }

    roots
}

fn complete_node(
    roots: &mut Vec<SequenceControlFrameNode>,
    stack: &mut Vec<SequenceControlFrameNode>,
) {
    let node = stack
        .pop()
        .expect("stack should contain a node to complete");
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}

fn valid_frame_end_row(frame: &SequenceControlFrame, line_count: usize) -> Option<usize> {
    let end_row = frame.end_row?;
    (frame.start_row < line_count && end_row < line_count && frame.start_row <= end_row)
        .then_some(end_row)
}

fn indent_child_frame(lines: Vec<SequenceLine>) -> Vec<SequenceLine> {
    lines.into_iter().map(indent_child_line).collect()
}

fn indent_child_line(line: SequenceLine) -> SequenceLine {
    let mut indented = SequenceLine::blank(line.len() + 1);
    indented.write_line(1, &line);
    indented
}

fn frame_width(frame: &SequenceControlFrame, rows: &[SequenceControlBodyRow]) -> usize {
    let max_row_width = rows
        .iter()
        .filter_map(|row| match row {
            SequenceControlBodyRow::Content(line) => Some(line.len()),
            SequenceControlBodyRow::Separator(_) => None,
        })
        .max()
        .unwrap_or(0);
    let title_width = display_width(&frame_title(frame));
    let separator_width = frame
        .separators
        .iter()
        .map(|separator| display_width(&separator_title(frame, separator)))
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
