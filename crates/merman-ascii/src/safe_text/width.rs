use super::normalization::{normalize_terminal_text, visible_escape};
use crate::options::TerminalWidthProfile;
use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
/// Unlike [`SafeText`], line feeds are rendered visibly so a caller cannot accidentally smuggle a
/// second terminal row through a one-row cell surface.
pub(crate) struct SafeLine<'a> {
    value: Cow<'a, str>,
}

impl<'a> SafeText<'a> {
    pub(crate) fn new(value: &'a str) -> Self {
        Self {
            value: normalize_terminal_text(value),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        self.value.as_ref()
    }

    pub(crate) fn lines(&self) -> impl Iterator<Item = &str> {
        self.value.split('\n')
    }
}

impl<'a> SafeLine<'a> {
    pub(crate) fn new(value: &'a str) -> Self {
        let normalized = normalize_terminal_text(value);
        let value = if normalized.contains('\n') {
            let mut buffer = [0u8; 10];
            Cow::Owned(normalized.replace('\n', visible_escape('\n', &mut buffer)))
        } else {
            normalized
        };
        Self { value }
    }

    pub(crate) fn as_str(&self) -> &str {
        self.value.as_ref()
    }

    pub(crate) fn graphemes(
        &self,
        profile: TerminalWidthProfile,
    ) -> impl Iterator<Item = MeasuredGrapheme<'_>> {
        measured_graphemes(self.value.as_ref(), profile)
    }
}

pub(crate) fn terminal_line_display_width(value: &str, profile: TerminalWidthProfile) -> usize {
    SafeLine::new(value)
        .graphemes(profile)
        .map(MeasuredGrapheme::width)
        .sum()
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
