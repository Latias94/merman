use crate::color::AsciiColorMode;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use std::borrow::Cow;
use std::fmt::Write as _;
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
    let controls_safe = escape_terminal_controls(value);
    escape_zero_width_graphemes(controls_safe)
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

/// Normalizes and encodes a family-owned line document.
///
/// StructuredText families intentionally do not invent a two-dimensional grid, but they still
/// share the same authored-text and HTML safety boundary as grid-backed renderers.
pub(crate) fn encode_text_lines(lines: Vec<String>, options: &AsciiRenderOptions) -> String {
    let normalized = lines
        .into_iter()
        .map(|line| normalize_terminal_text(&line).into_owned())
        .collect::<Vec<_>>();
    let mut output = normalized.join("\n");
    if options.color_mode == AsciiColorMode::Html {
        output = escape_html_text(&output);
    }
    output
}

fn escape_html_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    // HTML escaping classifies syntax scalars without participating in terminal layout.
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(ch),
        }
    }
    output
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
            Cow::Owned(normalized.replace('\n', "\\u{A}"))
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

fn measured_graphemes(
    value: &str,
    profile: TerminalWidthProfile,
) -> impl Iterator<Item = MeasuredGrapheme<'_>> {
    value.graphemes(true).map(move |text| MeasuredGrapheme {
        text,
        width: grapheme_display_width(text, profile),
    })
}

fn escape_terminal_controls(value: &str) -> Cow<'_, str> {
    // Control classification is intentionally scalar-based; segmentation follows normalization.
    if !value.chars().any(needs_control_escape) {
        return Cow::Borrowed(value);
    }

    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' && chars.peek() == Some(&'\n') {
            chars.next();
            output.push('\n');
        } else if needs_control_escape(ch) {
            push_visible_escape(&mut output, ch);
        } else {
            output.push(ch);
        }
    }
    Cow::Owned(output)
}

fn escape_zero_width_graphemes(value: Cow<'_, str>) -> Cow<'_, str> {
    if !value
        .graphemes(true)
        .any(|grapheme| grapheme != "\n" && UnicodeWidthStr::width(grapheme) == 0)
    {
        return value;
    }

    let mut output = String::with_capacity(value.len());
    for grapheme in value.graphemes(true) {
        if grapheme == "\n" || UnicodeWidthStr::width(grapheme) > 0 {
            output.push_str(grapheme);
        } else {
            // A zero-width cluster has no terminal cell, so expose each scalar deterministically.
            for ch in grapheme.chars() {
                push_visible_escape(&mut output, ch);
            }
        }
    }
    Cow::Owned(output)
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
    let _ = write!(output, "\\u{{{:X}}}", u32::from(ch));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_text_stays_borrowed() {
        let input = "Cafe\u{301} 👩‍💻 🇺🇸 中文";
        let normalized = normalize_terminal_text(input);

        assert!(matches!(normalized, Cow::Borrowed(_)));
        assert_eq!(normalized, input);
    }

    #[test]
    fn crlf_is_structural_but_tab_and_lone_carriage_return_are_visible() {
        assert_eq!(
            normalize_terminal_text("one\r\ntwo\tthree\rfour"),
            "one\ntwo\\u{9}three\\u{D}four"
        );
    }

    #[test]
    fn c0_c1_escape_del_and_bidi_controls_are_exhaustively_visible() {
        let bidi = [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ];
        let mut input = String::new();
        input.extend(['\0', '\u{1b}', '\u{7f}', '\u{80}', '\u{9f}']);
        input.extend(bidi);

        let normalized = normalize_terminal_text(&input);

        for control in input.chars() {
            assert!(
                !normalized.contains(control),
                "raw control {control:?} leaked"
            );
            assert!(
                normalized.contains(&format!("\\u{{{:X}}}", u32::from(control))),
                "missing visible escape for {control:?}: {normalized}"
            );
        }
    }

    #[test]
    fn standalone_zero_width_graphemes_are_visible() {
        assert_eq!(
            normalize_terminal_text("\u{301}\u{200d}\u{fe0f}"),
            "\\u{301}\\u{200D}\\u{FE0F}"
        );
    }

    #[test]
    fn legal_joiners_and_variation_selectors_survive_inside_visible_graphemes() {
        for input in ["👩‍💻", "✈️", "a\u{200c}b", "👍🏽"] {
            assert_eq!(normalize_terminal_text(input), input);
        }
    }

    #[test]
    fn normalization_is_idempotent() {
        let once = normalize_terminal_text("a\u{1b}\u{202e}\u{301}\r\nb").into_owned();
        let twice = normalize_terminal_text(&once);

        assert_eq!(twice, once);
    }

    #[test]
    fn diagnostics_truncate_only_at_grapheme_boundaries() {
        let input = "👩‍💻".repeat(MAX_DIAGNOSTIC_GRAPHEMES + 1);
        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(
            normalized.graphemes(true).count(),
            MAX_DIAGNOSTIC_GRAPHEMES + 3
        );
        assert!(normalized.ends_with("..."));
    }

    #[test]
    fn diagnostics_bound_control_escape_expansion_by_bytes() {
        let input = "\u{1b}".repeat(MAX_DIAGNOSTIC_INPUT_BYTES * 2);
        let normalized = normalize_terminal_diagnostic(&input);

        assert!(normalized.len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(normalized.ends_with(DIAGNOSTIC_ELLIPSIS));
        assert!(!normalized.contains('\u{1b}'));
    }

    #[test]
    fn diagnostics_reject_one_oversized_grapheme_without_splitting_it() {
        let mut input = String::from("a");
        input.extend(std::iter::repeat_n('\u{301}', MAX_DIAGNOSTIC_INPUT_BYTES));

        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(normalized, DIAGNOSTIC_ELLIPSIS);
        assert!(normalized.len() <= MAX_DIAGNOSTIC_BYTES);
    }
}
