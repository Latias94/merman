use super::model::SequenceArrowHead;
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
mod tests {
    use super::*;
    use crate::options::TerminalWidthProfile;

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
}
