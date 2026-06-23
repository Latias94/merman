use super::boxes::render_sequence_boxes;
use super::control::render_sequence_control_frames;
use super::layout::{SequenceLayout, calculate_layout};
use super::model::{AsciiSequenceDiagram, SequenceArrowHead};
use super::plan::SequenceRowPlan;
use super::text::{SequenceLine, padded_line, trim_right};
use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceChars {
    pub(super) top_left: char,
    pub(super) top_right: char,
    pub(super) bottom_left: char,
    pub(super) bottom_right: char,
    pub(super) horizontal: char,
    pub(super) vertical: char,
    pub(super) active_vertical: char,
    pub(super) destroyed_mark: char,
    pub(super) tee_down: char,
    pub(super) tee_up: char,
    pub(super) tee_right: char,
    pub(super) tee_left: char,
    pub(super) filled_arrow_right: char,
    pub(super) filled_arrow_left: char,
    pub(super) open_arrow_right: char,
    pub(super) open_arrow_left: char,
    pub(super) solid_line: char,
    pub(super) dotted_line: char,
    pub(super) self_top_right: char,
    pub(super) self_bottom: char,
}

impl SequenceChars {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                active_vertical: '#',
                destroyed_mark: 'x',
                tee_down: '+',
                tee_up: '+',
                tee_right: '+',
                tee_left: '+',
                filled_arrow_right: '>',
                filled_arrow_left: '<',
                open_arrow_right: '>',
                open_arrow_left: '<',
                solid_line: '-',
                dotted_line: '.',
                self_top_right: '+',
                self_bottom: '+',
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                active_vertical: '┃',
                destroyed_mark: '×',
                tee_down: '┬',
                tee_up: '┴',
                tee_right: '├',
                tee_left: '┤',
                filled_arrow_right: '►',
                filled_arrow_left: '◄',
                open_arrow_right: '>',
                open_arrow_left: '<',
                solid_line: '─',
                dotted_line: '┈',
                self_top_right: '┐',
                self_bottom: '┘',
            },
        }
    }

    pub(super) fn arrow_right(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_right,
            SequenceArrowHead::Open => self.open_arrow_right,
            SequenceArrowHead::Cross => self.destroyed_mark,
        }
    }

    pub(super) fn arrow_left(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_left,
            SequenceArrowHead::Open => self.open_arrow_left,
            SequenceArrowHead::Cross => self.destroyed_mark,
        }
    }
}

pub(crate) fn render_sequence_diagram(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    options.validate()?;
    if diagram.participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    let chars = SequenceChars::for_options(options);
    let layout = calculate_layout(diagram, options);
    let row_plan =
        SequenceRowPlan::build(diagram, &layout, &chars, options.sequence_mirror_actors)?;
    let (mut lines, control_frames) = row_plan.into_parts();

    if !control_frames.is_empty() {
        lines = render_sequence_control_frames(lines, &control_frames, &chars);
    }
    if !diagram.boxes.is_empty() {
        lines = render_sequence_boxes(lines, diagram, &layout, &chars);
    }
    if let Some(title) = diagram.title.as_deref() {
        prepend_title_line(&mut lines, title);
    }
    Ok(finish_sequence_lines(lines, options))
}

pub(super) fn build_lifeline_line(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
) -> SequenceLine {
    let mut line = SequenceLine::blank(layout.total_width + 1);
    for (index, center) in layout.participant_centers.iter().enumerate() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        line.set_role(
            *center,
            lifeline_char(index, chars, active_counts),
            lifeline_role(index, active_counts),
        );
    }
    trim_right(line)
}

pub(super) fn lifeline_char(index: usize, chars: &SequenceChars, active_counts: &[usize]) -> char {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        chars.active_vertical
    } else {
        chars.vertical
    }
}

pub(super) fn lifeline_role(index: usize, active_counts: &[usize]) -> AsciiColorRole {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        AsciiColorRole::SequenceActivation
    } else {
        AsciiColorRole::SequenceLifeline
    }
}

pub(super) fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    left: usize,
    overlay: &SequenceLine,
) -> SequenceLine {
    let needed = left + overlay.len();
    let mut line = padded_line(
        build_lifeline_line(layout, chars, active_counts, visible_actors),
        needed,
    );
    line.write_line(left, overlay);
    trim_right(line)
}

fn finish_sequence_lines(lines: Vec<SequenceLine>, options: &AsciiRenderOptions) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return lines
            .into_iter()
            .map(SequenceLine::into_text)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }

    if lines.is_empty() {
        return String::new();
    }

    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    if width == 0 {
        return "\n".repeat(lines.len());
    }

    let mut canvas = Canvas::new(width, lines.len());
    for (y, line) in lines.iter().enumerate() {
        line.write_to(&mut canvas, y);
    }

    canvas.finish_trimmed_with_options(options)
}

fn prepend_title_line(lines: &mut Vec<SequenceLine>, title: &str) {
    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    lines.insert(0, render_title_line(title, width));
}

fn render_title_line(title: &str, width: usize) -> SequenceLine {
    let title_width = display_width(title);
    let left = width.saturating_sub(title_width) / 2;
    let mut line = SequenceLine::blank(left);
    line.push_role_text(title, AsciiColorRole::Text);
    trim_right(line)
}
