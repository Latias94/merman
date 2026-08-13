use super::layout::{SequenceLayout, calculate_layout_with_resources};
use super::model::{AsciiSequenceDiagram, SequenceArrowHead};
use super::notes::apply_note_gutters;
use super::plan::SequenceRowPlan;
use super::text::{SequenceLine, blank_line, padded_line, trim_right};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::resource::ResourceContext;

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
    pub(super) solid_line: char,
    pub(super) dotted_line: char,
    pub(super) self_top_right: char,
    pub(super) self_bottom: char,
    pub(super) unicode_markers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceEndpointGlyph {
    pub(super) tip: char,
    pub(super) lineward_stem: Option<char>,
}

impl SequenceEndpointGlyph {
    const fn single(tip: char) -> Self {
        Self {
            tip,
            lineward_stem: None,
        }
    }

    const fn filled_half(tip: char) -> Self {
        Self {
            tip,
            lineward_stem: Some('|'),
        }
    }
}

impl SequenceChars {
    pub(super) fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
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
                solid_line: '-',
                dotted_line: '.',
                self_top_right: '+',
                self_bottom: '+',
                unicode_markers: false,
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
                solid_line: '─',
                dotted_line: '┈',
                self_top_right: '┐',
                self_bottom: '┘',
                unicode_markers: true,
            },
        }
    }

    pub(super) fn arrow_right(self, marker: SequenceArrowHead) -> Option<SequenceEndpointGlyph> {
        Some(match marker {
            SequenceArrowHead::None => return None,
            SequenceArrowHead::Filled => SequenceEndpointGlyph::single(self.filled_arrow_right),
            SequenceArrowHead::Cross => SequenceEndpointGlyph::single(self.destroyed_mark),
            SequenceArrowHead::Point => SequenceEndpointGlyph::single(')'),
            SequenceArrowHead::FilledHalfTop => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('◢')
                } else {
                    SequenceEndpointGlyph::filled_half('\\')
                }
            }
            SequenceArrowHead::FilledHalfBottom => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('◥')
                } else {
                    SequenceEndpointGlyph::filled_half('/')
                }
            }
            SequenceArrowHead::OpenHalfTop => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('╲')
                } else {
                    SequenceEndpointGlyph::single('\\')
                }
            }
            SequenceArrowHead::OpenHalfBottom => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('╱')
                } else {
                    SequenceEndpointGlyph::single('/')
                }
            }
        })
    }

    pub(super) fn arrow_left(self, marker: SequenceArrowHead) -> Option<SequenceEndpointGlyph> {
        Some(match marker {
            SequenceArrowHead::None => return None,
            SequenceArrowHead::Filled => SequenceEndpointGlyph::single(self.filled_arrow_left),
            SequenceArrowHead::Cross => SequenceEndpointGlyph::single(self.destroyed_mark),
            SequenceArrowHead::Point => SequenceEndpointGlyph::single('('),
            SequenceArrowHead::FilledHalfTop => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('◣')
                } else {
                    SequenceEndpointGlyph::filled_half('/')
                }
            }
            SequenceArrowHead::FilledHalfBottom => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('◤')
                } else {
                    SequenceEndpointGlyph::filled_half('\\')
                }
            }
            SequenceArrowHead::OpenHalfTop => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('╱')
                } else {
                    SequenceEndpointGlyph::single('/')
                }
            }
            SequenceArrowHead::OpenHalfBottom => {
                if self.unicode_markers {
                    SequenceEndpointGlyph::single('╲')
                } else {
                    SequenceEndpointGlyph::single('\\')
                }
            }
        })
    }

    pub(super) fn central_decoration(self) -> char {
        if self.unicode_markers { '○' } else { 'o' }
    }
}

#[cfg(test)]
pub(crate) fn render_sequence_diagram(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut resources = ResourceContext::new(options.resources);
    render_sequence_diagram_with_resources(diagram, options, &mut resources)
}

pub(crate) fn render_sequence_diagram_with_resources(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    options.validate()?;
    if diagram.participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    debug_assert_eq!(resources.policy(), options.resources);
    let chars = SequenceChars::for_options(options);
    let mut layout = calculate_layout_with_resources(diagram, options, resources)?;
    apply_note_gutters(diagram, &mut layout, resources)?;
    let row_plan = SequenceRowPlan::build(
        diagram,
        &layout,
        &chars,
        options.sequence_mirror_actors,
        resources,
    )?;
    row_plan.render(diagram, &layout, &chars, options, resources)
}

pub(super) fn build_lifeline_line(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let width = resources.checked_grid_add(layout.total_width, 1)?;
    let mut line = blank_line(width, layout.width_profile, resources)?;
    for (index, center) in layout.participant_centers.iter().enumerate() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        line.try_set_role(
            *center,
            lifeline_char(index, chars, active_counts),
            lifeline_role(index, active_counts),
        )?;
    }
    trim_right(line)
}

pub(super) fn retained_lifeline_width(
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
) -> Result<usize> {
    layout
        .participant_centers
        .iter()
        .enumerate()
        .filter(|(index, _)| visible_actors.get(*index).copied().unwrap_or(true))
        .try_fold(0usize, |width, (_, center)| {
            Ok(width.max(resources.checked_grid_add(*center, 1)?))
        })
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

#[allow(clippy::too_many_arguments)]
pub(super) fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    left: usize,
    overlay: &SequenceLine,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let needed = resources.checked_grid_add(left, overlay.len())?;
    let width = needed.max(resources.checked_grid_add(layout.total_width, 1)?);
    resources.grid_extent(width, 1)?;
    let mut line = padded_line(
        build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
        width,
    )?;
    line.try_write_line(left, overlay)?;
    trim_right(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TerminalWidthProfile;
    use crate::resource::AsciiResourceLimitId;
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceParticipant, SequenceParticipantLabel,
    };

    #[test]
    fn endpoint_glyphs_preserve_half_arrow_fill_in_every_structural_charset() {
        let ascii = SequenceChars::for_options(&AsciiRenderOptions::ascii());
        let unicode = SequenceChars::for_options(&AsciiRenderOptions::unicode());
        let mut cjk_options = AsciiRenderOptions::unicode();
        cjk_options.terminal_width_profile = TerminalWidthProfile::Cjk;
        let cjk = SequenceChars::for_options(&cjk_options);

        let ascii_cases = [
            (
                SequenceArrowHead::FilledHalfTop,
                ('\\', Some('|')),
                ('/', Some('|')),
            ),
            (
                SequenceArrowHead::FilledHalfBottom,
                ('/', Some('|')),
                ('\\', Some('|')),
            ),
            (SequenceArrowHead::OpenHalfTop, ('\\', None), ('/', None)),
            (SequenceArrowHead::OpenHalfBottom, ('/', None), ('\\', None)),
        ];
        for (marker, right, left) in ascii_cases {
            for chars in [ascii, cjk] {
                let right_glyph = chars.arrow_right(marker).unwrap();
                let left_glyph = chars.arrow_left(marker).unwrap();
                assert_eq!((right_glyph.tip, right_glyph.lineward_stem), right);
                assert_eq!((left_glyph.tip, left_glyph.lineward_stem), left);
            }
        }

        for (marker, right, left) in [
            (SequenceArrowHead::FilledHalfTop, '◢', '◣'),
            (SequenceArrowHead::FilledHalfBottom, '◥', '◤'),
            (SequenceArrowHead::OpenHalfTop, '╲', '╱'),
            (SequenceArrowHead::OpenHalfBottom, '╱', '╲'),
        ] {
            assert_eq!(
                unicode.arrow_right(marker),
                Some(SequenceEndpointGlyph::single(right))
            );
            assert_eq!(
                unicode.arrow_left(marker),
                Some(SequenceEndpointGlyph::single(left))
            );
        }
    }

    #[test]
    fn sequence_grid_extent_accepts_exact_limit_and_rejects_limit_minus_one() {
        let diagram = single_participant_diagram();
        let renders_with_limit = |limit| {
            let options = AsciiRenderOptions::ascii()
                .with_resource_limit(AsciiResourceLimitId::MaxGridCells, limit)
                .expect("valid grid limit");
            render_sequence_diagram(&diagram, &options)
        };

        let mut upper = 1usize;
        while renders_with_limit(upper).is_err() {
            upper = upper.checked_mul(2).expect("grid boundary must fit usize");
        }
        let mut lower = 1usize;
        while lower < upper {
            let midpoint = lower + (upper - lower) / 2;
            if renders_with_limit(midpoint).is_ok() {
                upper = midpoint;
            } else {
                lower = midpoint + 1;
            }
        }
        let exact_cells = lower;

        let exact = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
            .unwrap();
        assert!(render_sequence_diagram(&diagram, &exact).is_ok());

        let below = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
            .unwrap();
        let error = render_sequence_diagram(&diagram, &below).unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == exact_cells
                    && details.max == exact_cells - 1
        ));
    }

    #[test]
    fn sequence_plain_output_enforces_custom_grapheme_limit() {
        let mut diagram = single_participant_diagram();
        diagram.participants[0].label =
            SequenceParticipantLabel::from_raw("👨‍👩‍👧‍👦", false, TerminalWidthProfile::Unicode);
        let options = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGraphemeBytes, 4)
            .unwrap();

        let error = render_sequence_diagram(&diagram, &options).unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual > details.max
        ));
    }

    fn single_participant_diagram() -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
            title: None,
            participants: vec![SequenceParticipant {
                id: "p0".to_string(),
                label: SequenceParticipantLabel::from_raw(
                    "P0",
                    false,
                    TerminalWidthProfile::Unicode,
                ),
            }],
            lifecycles: vec![SequenceActorLifecycle::default()],
            boxes: Vec::new(),
            body: crate::sequence::tree::SequenceBody::default(),
        }
    }
}
