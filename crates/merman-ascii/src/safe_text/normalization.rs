use super::width::grapheme_display_width;
use crate::Result;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use std::borrow::Cow;
use std::convert::Infallible;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MAX_DIAGNOSTIC_GRAPHEMES: usize = 256;
const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 16 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const DIAGNOSTIC_ELLIPSIS: &str = "...";

/// Normalizes untrusted authored text for terminal display without changing printable text.
///
/// Line feeds are structural. CRLF is normalized to LF; every other C0/C1 control, ESC, DEL, and
/// bidirectional formatting control is rendered as an uppercase `\u{HEX}` escape. Standalone
/// zero-width graphemes are escaped scalar by scalar, while joiners and variation selectors remain
/// intact when they belong to a positive-width grapheme. Applying this function twice is
/// idempotent.
pub fn normalize_terminal_text(value: &str) -> Cow<'_, str> {
    if !needs_normalization(value) {
        return Cow::Borrowed(value);
    }

    let mut output = String::with_capacity(value.len());
    let normalized = visit_normalized_segments(value, |segment| {
        match segment.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => output.push_str(grapheme),
            NormalizedSegmentKind::VisibleEscape(ch) => push_visible_escape(&mut output, ch),
            NormalizedSegmentKind::LineBreak => output.push('\n'),
        }
        Ok::<(), Infallible>(())
    });
    match normalized {
        Ok(()) => Cow::Owned(output),
        Err(never) => match never {},
    }
}

/// Reports whether terminal-safe normalization changes the authored text without materializing
/// the normalized form.
pub(crate) fn terminal_text_requires_normalization(
    value: &str,
    resources: &ResourceContext,
) -> Result<bool> {
    for grapheme in value.graphemes(true) {
        resources.check_grapheme_bytes(grapheme.len())?;
        resources.charge_layout_work(grapheme.len().max(1))?;
        if grapheme_needs_normalization(grapheme) {
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
            Ok::<(), crate::AsciiError>(())
        })?;
        Ok(blank)
    })
}

/// Produces a bounded, terminal-safe human-readable diagnostic.
///
/// This is the display boundary for errors that may contain authored identifiers or parser text.
/// Structured codes and spans should remain separate fields at binding boundaries.
pub fn normalize_terminal_diagnostic(value: &str) -> String {
    let mut raw_prefix = String::with_capacity(value.len().min(MAX_DIAGNOSTIC_INPUT_BYTES));
    let mut input_truncated = false;
    for grapheme in value.graphemes(true) {
        if raw_prefix.len().saturating_add(grapheme.len()) > MAX_DIAGNOSTIC_INPUT_BYTES {
            input_truncated = true;
            break;
        }
        raw_prefix.push_str(grapheme);
    }

    let safe = normalize_terminal_text(&raw_prefix);
    let content_byte_limit = MAX_DIAGNOSTIC_BYTES - DIAGNOSTIC_ELLIPSIS.len();
    let mut output = String::with_capacity(safe.len().min(MAX_DIAGNOSTIC_BYTES));
    let mut output_truncated = input_truncated;
    for (index, grapheme) in safe.graphemes(true).enumerate() {
        if index == MAX_DIAGNOSTIC_GRAPHEMES
            || output.len().saturating_add(grapheme.len()) > content_byte_limit
        {
            output_truncated = true;
            break;
        }
        output.push_str(grapheme);
    }
    if output_truncated {
        output.push_str(DIAGNOSTIC_ELLIPSIS);
    }
    output
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NormalizedSegment<'a> {
    pub(super) kind: NormalizedSegmentKind<'a>,
    #[cfg(test)]
    pub(super) source_start: usize,
    #[cfg(test)]
    pub(super) source_end: usize,
    pub(super) source_grapheme_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum NormalizedSegmentKind<'a> {
    Grapheme(&'a str),
    VisibleEscape(char),
    LineBreak,
}

impl NormalizedSegment<'_> {
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

pub(super) fn visit_normalized_segments<'a, E>(
    value: &'a str,
    mut visit: impl FnMut(NormalizedSegment<'a>) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    for (source_start, grapheme) in value.grapheme_indices(true) {
        #[cfg(not(test))]
        let _ = source_start;
        #[cfg(test)]
        let source_end = source_start + grapheme.len();
        let source_grapheme_bytes = grapheme.len();
        if grapheme == "\r\n" || grapheme == "\n" {
            visit(NormalizedSegment {
                kind: NormalizedSegmentKind::LineBreak,
                #[cfg(test)]
                source_start,
                #[cfg(test)]
                source_end,
                source_grapheme_bytes,
            })?;
            continue;
        }

        if grapheme.chars().any(needs_control_escape) {
            for (offset, ch) in grapheme.char_indices() {
                let kind = if ch == '\n' {
                    NormalizedSegmentKind::LineBreak
                } else if needs_control_escape(ch) || UnicodeWidthChar::width(ch).unwrap_or(0) == 0
                {
                    NormalizedSegmentKind::VisibleEscape(ch)
                } else {
                    NormalizedSegmentKind::Grapheme(&grapheme[offset..offset + ch.len_utf8()])
                };
                visit(NormalizedSegment {
                    kind,
                    #[cfg(test)]
                    source_start,
                    #[cfg(test)]
                    source_end,
                    source_grapheme_bytes,
                })?;
            }
            continue;
        }

        if UnicodeWidthStr::width(grapheme) == 0 {
            for ch in grapheme.chars() {
                visit(NormalizedSegment {
                    kind: NormalizedSegmentKind::VisibleEscape(ch),
                    #[cfg(test)]
                    source_start,
                    #[cfg(test)]
                    source_end,
                    source_grapheme_bytes,
                })?;
            }
        } else {
            visit(NormalizedSegment {
                kind: NormalizedSegmentKind::Grapheme(grapheme),
                #[cfg(test)]
                source_start,
                #[cfg(test)]
                source_end,
                source_grapheme_bytes,
            })?;
        }
    }
    Ok(())
}

pub(super) fn try_append_normalized_segment(
    output: &mut String,
    segment: NormalizedSegment<'_>,
    allocation_error: fn() -> crate::AsciiError,
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
            push_visible_escape(output, ch);
        }
        NormalizedSegmentKind::LineBreak => {}
    }
    Ok(())
}

pub(super) fn visible_escape_len(ch: char) -> usize {
    let mut digits = 1;
    let mut value = u32::from(ch);
    while value >= 16 {
        digits += 1;
        value /= 16;
    }
    4 + digits
}

pub(super) fn visible_escape(ch: char, buffer: &mut [u8; 10]) -> &str {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let digits = visible_escape_len(ch) - 4;
    buffer[..3].copy_from_slice(b"\\u{");
    let mut value = u32::from(ch);
    for index in (0..digits).rev() {
        buffer[3 + index] = HEX[(value & 0x0f) as usize];
        value >>= 4;
    }
    buffer[3 + digits] = b'}';
    std::str::from_utf8(&buffer[..4 + digits]).expect("visible escapes contain only ASCII")
}

fn needs_normalization(value: &str) -> bool {
    value.graphemes(true).any(grapheme_needs_normalization)
}

fn grapheme_needs_normalization(grapheme: &str) -> bool {
    grapheme == "\r\n"
        || (grapheme != "\n"
            && (grapheme.chars().any(needs_control_escape)
                || UnicodeWidthStr::width(grapheme) == 0))
}

fn needs_control_escape(ch: char) -> bool {
    ch == '\r'
        || (ch <= '\u{1f}' && ch != '\n')
        || ('\u{7f}'..='\u{9f}').contains(&ch)
        || is_bidi_control(ch)
}

fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'
            | '\u{202b}'
            | '\u{202c}'
            | '\u{202d}'
            | '\u{202e}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

fn push_visible_escape(output: &mut String, ch: char) {
    let mut buffer = [0u8; 10];
    output.push_str(visible_escape(ch, &mut buffer));
}

#[cfg(test)]
pub(super) const fn diagnostic_limits() -> (usize, usize, usize, &'static str) {
    (
        MAX_DIAGNOSTIC_GRAPHEMES,
        MAX_DIAGNOSTIC_INPUT_BYTES,
        MAX_DIAGNOSTIC_BYTES,
        DIAGNOSTIC_ELLIPSIS,
    )
}
