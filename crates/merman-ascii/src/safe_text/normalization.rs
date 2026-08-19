use super::width::grapheme_display_width;
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use std::borrow::Cow;
use std::convert::Infallible;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MAX_DIAGNOSTIC_GRAPHEMES: usize = 256;
const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 16 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const MAX_DIAGNOSTIC_NORMALIZED_BYTES: usize = MAX_DIAGNOSTIC_INPUT_BYTES * 10;
const DIAGNOSTIC_ELLIPSIS: &str = "...";
const NORMALIZATION_FAILURE_PLACEHOLDER: &str = "[terminal text unavailable]";

/// Normalizes untrusted authored text for terminal display without changing printable text.
///
/// Line feeds are structural. CRLF is normalized to LF; every other C0/C1 control, ESC, DEL, and
/// bidirectional formatting control is rendered as an uppercase `\u{HEX}` escape. Standalone
/// zero-width graphemes are escaped scalar by scalar, while joiners and variation selectors remain
/// intact when they belong to a positive-width grapheme. Applying this function twice is
/// idempotent. The exact normalized byte length is checked before allocation; capacity overflow
/// or allocation failure returns a fixed terminal-safe placeholder and never exposes the authored
/// controls.
pub fn normalize_terminal_text(value: &str) -> Cow<'_, str> {
    normalize_terminal_text_with_capacity(value, usize::MAX)
}

fn normalize_terminal_text_with_capacity(value: &str, maximum_capacity: usize) -> Cow<'_, str> {
    try_normalize_terminal_text_with_options(value, maximum_capacity, false)
        .unwrap_or(Cow::Borrowed(NORMALIZATION_FAILURE_PLACEHOLDER))
}

pub(super) fn try_normalize_terminal_line_text(value: &str) -> Result<Cow<'_, str>> {
    try_normalize_terminal_text_with_options(value, usize::MAX, true)
        .map_err(|()| AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str()))
}

fn try_normalize_terminal_text_with_options(
    value: &str,
    maximum_capacity: usize,
    escape_line_feed: bool,
) -> std::result::Result<Cow<'_, str>, ()> {
    let Some(plan) = terminal_normalization_plan(value, escape_line_feed) else {
        return Err(());
    };
    if !plan.changed {
        return Ok(Cow::Borrowed(value));
    }
    if plan.output_bytes > maximum_capacity {
        return Err(());
    }

    let mut output = String::new();
    if output.try_reserve_exact(plan.output_bytes).is_err() {
        return Err(());
    }
    let normalized = visit_normalized_segments(value, |segment| {
        match segment.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => output.push_str(grapheme),
            NormalizedSegmentKind::VisibleEscape(ch) => push_visible_escape(&mut output, ch),
            NormalizedSegmentKind::LineBreak if escape_line_feed => {
                push_visible_escape(&mut output, '\n')
            }
            NormalizedSegmentKind::LineBreak => output.push('\n'),
        }
        Ok::<(), Infallible>(())
    });
    match normalized {
        Ok(()) => {
            debug_assert_eq!(output.len(), plan.output_bytes);
            Ok(Cow::Owned(output))
        }
        Err(never) => match never {},
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalNormalizationPlan {
    output_bytes: usize,
    changed: bool,
}

fn terminal_normalization_plan(
    value: &str,
    escape_line_feed: bool,
) -> Option<TerminalNormalizationPlan> {
    let mut output_bytes = 0usize;
    let mut changed = false;
    let planned = visit_normalized_segments(value, |segment| {
        let segment_bytes = match segment.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => grapheme.len(),
            NormalizedSegmentKind::VisibleEscape(ch) => {
                changed = true;
                visible_escape_len(ch)
            }
            NormalizedSegmentKind::LineBreak => {
                if escape_line_feed {
                    changed = true;
                    visible_escape_len('\n')
                } else {
                    changed |= segment.source_grapheme_bytes != 1;
                    1
                }
            }
        };
        output_bytes = output_bytes.checked_add(segment_bytes).ok_or(())?;
        Ok::<(), ()>(())
    });
    planned.ok().map(|()| TerminalNormalizationPlan {
        output_bytes,
        changed,
    })
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
        if (escape_line_feed && grapheme == "\n") || grapheme_needs_normalization(grapheme) {
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
    normalize_terminal_diagnostic_with_capacity(value, MAX_DIAGNOSTIC_BYTES)
}

fn normalize_terminal_diagnostic_with_capacity(value: &str, maximum_capacity: usize) -> String {
    let (raw_prefix, input_truncated) = bounded_grapheme_prefix(value);
    let safe = match try_normalize_terminal_text_with_options(
        raw_prefix,
        MAX_DIAGNOSTIC_NORMALIZED_BYTES,
        false,
    ) {
        Ok(safe) => safe,
        Err(()) => return owned_normalization_failure_placeholder(String::new()),
    };
    let content_byte_limit = MAX_DIAGNOSTIC_BYTES - DIAGNOSTIC_ELLIPSIS.len();
    let mut output = String::new();
    let mut truncated = input_truncated;
    let mut completed = true;
    for (index, grapheme) in safe.graphemes(true).enumerate() {
        let Some(next_bytes) = output.len().checked_add(grapheme.len()) else {
            return owned_normalization_failure_placeholder(output);
        };
        if index == MAX_DIAGNOSTIC_GRAPHEMES || next_bytes > content_byte_limit {
            truncated = true;
            completed = false;
            break;
        }
        if next_bytes > maximum_capacity || output.try_reserve(grapheme.len()).is_err() {
            return owned_normalization_failure_placeholder(output);
        }
        output.push_str(grapheme);
    }
    truncated |= !completed;
    if truncated {
        let Some(final_bytes) = output.len().checked_add(DIAGNOSTIC_ELLIPSIS.len()) else {
            return owned_normalization_failure_placeholder(output);
        };
        if final_bytes > maximum_capacity || output.try_reserve(DIAGNOSTIC_ELLIPSIS.len()).is_err()
        {
            return owned_normalization_failure_placeholder(output);
        }
        output.push_str(DIAGNOSTIC_ELLIPSIS);
    }
    output
}

fn bounded_grapheme_prefix(value: &str) -> (&str, bool) {
    if value.len() <= MAX_DIAGNOSTIC_INPUT_BYTES {
        return (value, false);
    }

    let mut byte_end = MAX_DIAGNOSTIC_INPUT_BYTES;
    while !value.is_char_boundary(byte_end) {
        byte_end -= 1;
    }
    let window = &value[..byte_end];
    let mut last_grapheme_start = 0usize;
    for (start, _) in window.grapheme_indices(true) {
        last_grapheme_start = start;
    }
    (&window[..last_grapheme_start], true)
}

fn owned_normalization_failure_placeholder(mut output: String) -> String {
    output.clear();
    if output.capacity() >= NORMALIZATION_FAILURE_PLACEHOLDER.len()
        || output
            .try_reserve_exact(NORMALIZATION_FAILURE_PLACEHOLDER.len())
            .is_ok()
    {
        output.push_str(NORMALIZATION_FAILURE_PLACEHOLDER);
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

fn grapheme_needs_normalization(grapheme: &str) -> bool {
    grapheme == "\r\n"
        || (grapheme != "\n"
            && (grapheme.chars().any(needs_control_escape)
                || UnicodeWidthStr::width(grapheme) == 0))
}

pub(super) fn needs_control_escape(ch: char) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_capacity_failure_returns_a_safe_static_placeholder() {
        let input = "\u{1b}".repeat(16);
        let normalized =
            normalize_terminal_text_with_capacity(&input, NORMALIZATION_FAILURE_PLACEHOLDER.len());

        assert!(matches!(normalized, Cow::Borrowed(_)));
        assert_eq!(normalized, NORMALIZATION_FAILURE_PLACEHOLDER);
        assert!(!normalized.contains('\u{1b}'));
        assert_eq!(normalize_terminal_text(&normalized), normalized);
    }

    #[test]
    fn diagnostic_capacity_failure_returns_a_safe_placeholder() {
        let input = "\u{1b}".repeat(64);
        let normalized = normalize_terminal_diagnostic_with_capacity(
            &input,
            NORMALIZATION_FAILURE_PLACEHOLDER.len(),
        );

        assert_eq!(normalized, NORMALIZATION_FAILURE_PLACEHOLDER);
        assert!(!normalized.contains('\u{1b}'));
    }

    #[test]
    fn diagnostic_prefix_bounds_one_oversized_grapheme_before_segmentation() {
        let input = format!("a{}b", "\u{301}".repeat(MAX_DIAGNOSTIC_INPUT_BYTES));
        let (prefix, truncated) = bounded_grapheme_prefix(&input);

        assert!(prefix.len() <= MAX_DIAGNOSTIC_INPUT_BYTES);
        assert!(truncated);
        assert_eq!(normalize_terminal_diagnostic(&input), "...");
    }

    #[test]
    fn diagnostic_counts_graphemes_after_visible_escape_projection() {
        let input = format!("{}\u{1b}\u{0903}", "a".repeat(250));
        let normalized = normalize_terminal_diagnostic(&input);

        assert!(!normalized.ends_with(DIAGNOSTIC_ELLIPSIS));
        assert_eq!(normalized.graphemes(true).count(), 256);
    }
}
