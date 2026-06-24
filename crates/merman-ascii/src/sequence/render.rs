use super::layout::{SequenceLayout, calculate_layout};
use super::model::{AsciiSequenceDiagram, SequenceArrowHead};
use super::plan::SequenceRowPlan;
use super::text::{SequenceLine, padded_line, trim_right};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};

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
    Ok(row_plan.render(diagram, &layout, &chars, options))
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
