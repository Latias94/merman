#[cfg(test)]
use super::normalization::normalize_terminal_text;
use super::normalization::{
    NormalizedSegmentKind, try_normalize_terminal_line_text, visible_escape_len,
    visit_normalized_segments,
};
use crate::Result;
use crate::options::TerminalWidthProfile;
use std::borrow::Cow;
use std::convert::Infallible;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[cfg(test)]
pub(crate) struct SafeText<'a> {
    value: Cow<'a, str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeasuredGrapheme<'a> {
    text: &'a str,
    width: usize,
}

impl<'a> MeasuredGrapheme<'a> {
    pub(crate) fn text(self) -> &'a str {
        self.text
    }

    pub(crate) fn width(self) -> usize {
        self.width
    }
}

/// A normalized single terminal line.
///
/// Line feeds are rendered visibly so a caller cannot accidentally smuggle a second terminal row
/// through a one-row cell surface.
pub(crate) struct SafeLine<'a> {
    value: Cow<'a, str>,
}

#[cfg(test)]
impl<'a> SafeText<'a> {
    pub(crate) fn new(value: &'a str) -> Self {
        Self {
            value: normalize_terminal_text(value),
        }
    }

    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        self.value.as_ref()
    }

    pub(crate) fn lines(&self) -> impl Iterator<Item = &str> {
        self.value.split('\n')
    }
}

impl<'a> SafeLine<'a> {
    pub(crate) fn try_new(value: &'a str) -> Result<Self> {
        Ok(Self {
            value: try_normalize_terminal_line_text(value)?,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(value: &'a str) -> Self {
        Self::try_new(value).expect("test text should fit in memory")
    }

    pub(crate) fn graphemes(
        &self,
        profile: TerminalWidthProfile,
    ) -> impl Iterator<Item = MeasuredGrapheme<'_>> {
        measured_graphemes(self.value.as_ref(), profile)
    }
}

pub(crate) fn terminal_line_display_width(value: &str, profile: TerminalWidthProfile) -> usize {
    let mut width = 0usize;
    let measured = visit_normalized_segments(value, |segment| {
        let segment_width = match segment.kind {
            NormalizedSegmentKind::LineBreak => visible_escape_len('\n'),
            _ => segment.display_width(profile),
        };
        width = width.saturating_add(segment_width);
        Ok::<(), Infallible>(())
    });
    match measured {
        Ok(()) => width,
        Err(never) => match never {},
    }
}

pub(crate) fn grapheme_display_width(grapheme: &str, profile: TerminalWidthProfile) -> usize {
    match profile {
        TerminalWidthProfile::Unicode => UnicodeWidthStr::width(grapheme),
        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(grapheme),
    }
}

pub(crate) fn terminal_char_display_width(ch: char, profile: TerminalWidthProfile) -> usize {
    match profile {
        TerminalWidthProfile::Unicode => UnicodeWidthChar::width(ch),
        TerminalWidthProfile::Cjk => UnicodeWidthChar::width_cjk(ch),
    }
    .unwrap_or(0)
    .max(1)
}

pub(super) fn measured_graphemes(
    value: &str,
    profile: TerminalWidthProfile,
) -> impl Iterator<Item = MeasuredGrapheme<'_>> {
    value.graphemes(true).map(move |text| MeasuredGrapheme {
        text,
        width: grapheme_display_width(text, profile),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_line_width_matches_visible_control_projection() {
        assert_eq!(
            terminal_line_display_width("a\u{1b}\nb", TerminalWidthProfile::Unicode),
            "a\\u{1B}\\u{A}b".len()
        );
    }
}
