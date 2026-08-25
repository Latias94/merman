use super::width::grapheme_display_width;
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
pub(super) use merman_core::terminal_text::{
    TerminalTextSegmentKind as NormalizedSegmentKind,
    scalar_requires_visible_escape as needs_control_escape, visible_escape, visible_escape_len,
};
pub use merman_core::terminal_text::{normalize_terminal_diagnostic, normalize_terminal_text};
use merman_core::terminal_text::{
    terminal_grapheme_requires_normalization,
    try_normalize_terminal_line_text as try_normalize_core_terminal_line_text,
    visit_terminal_text_segments,
};
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn try_normalize_terminal_line_text(value: &str) -> Result<std::borrow::Cow<'_, str>> {
    try_normalize_core_terminal_line_text(value)
        .map_err(|_| AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str()))
}

/// Reports whether terminal-safe normalization changes the authored text without materializing
/// the normalized form.
pub(crate) fn terminal_text_requires_normalization(
    value: &str,
    resources: &ResourceContext,
) -> Result<bool> {
    terminal_text_requires_normalization_with_line_feed(value, false, resources)
}

/// Reports whether normalization for a single terminal line changes the authored text.
///
/// Unlike general terminal text, a line feed cannot remain structural inside a single rendered
/// row, so the composed-text path exposes it as a visible `\u{A}` escape.
pub(crate) fn terminal_single_line_text_requires_normalization(
    value: &str,
    resources: &ResourceContext,
) -> Result<bool> {
    terminal_text_requires_normalization_with_line_feed(value, true, resources)
}

fn terminal_text_requires_normalization_with_line_feed(
    value: &str,
    escape_line_feed: bool,
    resources: &ResourceContext,
) -> Result<bool> {
    for grapheme in value.graphemes(true) {
        resources.check_grapheme_bytes(grapheme.len())?;
        resources.charge_layout_work(grapheme.len().max(1))?;
        if (escape_line_feed && grapheme == "\n")
            || terminal_grapheme_requires_normalization(grapheme)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reports whether terminal-safe normalization would contain only trim whitespace without
/// retaining the normalized representation.
pub(crate) fn terminal_text_is_blank(value: &str, resources: &ResourceContext) -> Result<bool> {
    resources.transaction(|resources| {
        let mut blank = true;
        visit_normalized_segments(value, |segment| {
            segment.check_grapheme_budget(resources)?;
            resources.charge_layout_work(segment.layout_work())?;
            match segment.kind {
                NormalizedSegmentKind::Grapheme(grapheme) => {
                    if grapheme.chars().any(|ch| !ch.is_whitespace()) {
                        blank = false;
                    }
                }
                NormalizedSegmentKind::VisibleEscape(_) => blank = false,
                NormalizedSegmentKind::LineBreak => {}
            }
            Ok::<(), AsciiError>(())
        })?;
        Ok(blank)
    })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NormalizedSegment<'a> {
    pub(super) kind: NormalizedSegmentKind<'a>,
    pub(super) source_grapheme_bytes: usize,
}

impl NormalizedSegment<'_> {
    pub(super) fn is_trim_whitespace(self) -> bool {
        match self.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => grapheme_is_trim_whitespace(grapheme),
            NormalizedSegmentKind::LineBreak => true,
            NormalizedSegmentKind::VisibleEscape(_) => false,
        }
    }

    pub(super) fn check_grapheme_budget(self, resources: &ResourceContext) -> Result<()> {
        resources.check_grapheme_bytes(self.source_grapheme_bytes)?;
        match self.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => {
                resources.check_grapheme_bytes(grapheme.len())
            }
            NormalizedSegmentKind::VisibleEscape(_) => resources.check_grapheme_bytes(1),
            NormalizedSegmentKind::LineBreak => Ok(()),
        }
    }

    pub(super) fn layout_work(self) -> usize {
        match self.kind {
            NormalizedSegmentKind::Grapheme(_) | NormalizedSegmentKind::LineBreak => 1,
            NormalizedSegmentKind::VisibleEscape(ch) => visible_escape_len(ch),
        }
    }

    pub(super) fn display_width(self, profile: TerminalWidthProfile) -> usize {
        match self.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => grapheme_display_width(grapheme, profile),
            NormalizedSegmentKind::VisibleEscape(ch) => visible_escape_len(ch),
            NormalizedSegmentKind::LineBreak => 0,
        }
    }

    pub(super) fn text<'buffer>(self, buffer: &'buffer mut [u8; 10]) -> &'buffer str
    where
        Self: 'buffer,
    {
        match self.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => grapheme,
            NormalizedSegmentKind::VisibleEscape(ch) => visible_escape(ch, buffer),
            NormalizedSegmentKind::LineBreak => "\n",
        }
    }
}

/// Applies `str::trim`-like semantics without placing a slice boundary inside a grapheme.
pub(crate) fn grapheme_safe_trim(value: &str) -> &str {
    let mut start = 0usize;
    let mut end = 0usize;
    let mut retained = false;

    for (offset, grapheme) in value.grapheme_indices(true) {
        if grapheme_is_trim_whitespace(grapheme) {
            if !retained {
                start = offset + grapheme.len();
            }
        } else {
            retained = true;
            end = offset + grapheme.len();
        }
    }

    if retained { &value[start..end] } else { "" }
}

fn grapheme_is_trim_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_whitespace)
}

pub(super) fn visit_normalized_segments<'a, E>(
    value: &'a str,
    mut visit: impl FnMut(NormalizedSegment<'a>) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    visit_terminal_text_segments(value, |segment| {
        visit(NormalizedSegment {
            kind: segment.kind(),
            source_grapheme_bytes: segment.source_grapheme_bytes(),
        })
    })
}

pub(super) fn try_append_normalized_segment(
    output: &mut String,
    segment: NormalizedSegment<'_>,
    allocation_error: fn() -> AsciiError,
) -> Result<()> {
    match segment.kind {
        NormalizedSegmentKind::Grapheme(grapheme) => {
            output
                .try_reserve(grapheme.len())
                .map_err(|_| allocation_error())?;
            output.push_str(grapheme);
        }
        NormalizedSegmentKind::VisibleEscape(ch) => {
            let required = visible_escape_len(ch);
            output
                .try_reserve(required)
                .map_err(|_| allocation_error())?;
            let mut buffer = [0u8; 10];
            output.push_str(visible_escape(ch, &mut buffer));
        }
        NormalizedSegmentKind::LineBreak => {}
    }
    Ok(())
}
