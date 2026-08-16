//! Target-neutral, terminal-safe projection for parser diagnostics.
//!
//! Core parser errors retain authored context for programmatic consumers. This module owns the
//! bounded display boundary used by render targets, bindings, and command-line hosts so those
//! consumers do not need an ASCII feature merely to report an error safely.

use std::borrow::Cow;
use std::convert::Infallible;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MAX_DIAGNOSTIC_GRAPHEMES: usize = 256;
const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 16 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const DIAGNOSTIC_ELLIPSIS: &str = "...";

/// Machine-readable context for a terminal-safe parser diagnostic.
///
/// Authored string fields are terminal-safe and bounded. Byte spans remain separate from the
/// human-readable message so bindings do not need to recover structure from display text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalDiagnosticDetails {
    pub code: String,
    pub span: Option<merman_core::SourceSpan>,
    pub span_kind: Option<merman_core::ParseDiagnosticSpanKind>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
}

/// Bounded terminal-safe projection of a core parser error.
///
/// The wrapped error is deliberately not exposed through [`std::error::Error::source`], because
/// its display and debug representations may retain authored source text or host-adapter error
/// messages. Use [`Self::terminal_diagnostic_details`] for stable machine-readable context.
pub struct TerminalDiagnostic {
    error: merman_core::Error,
}

/// Bounded terminal-safe projection of a runtime-policy failure.
///
/// This wrapper preserves capability classification without exposing host-adapter messages through
/// [`std::fmt::Display`], [`std::fmt::Debug`], or [`std::error::Error::source`].
#[derive(Clone, PartialEq, Eq)]
pub struct TerminalRuntimePolicyError {
    error: merman_core::runtime::RuntimePolicyError,
}

impl TerminalDiagnostic {
    #[must_use]
    pub fn terminal_safe_message(&self) -> String {
        safe_parse_error(&self.error)
    }

    #[must_use]
    pub fn terminal_diagnostic_details(&self) -> TerminalDiagnosticDetails {
        safe_parse_details(&self.error)
    }

    fn kind(&self) -> &'static str {
        match &self.error {
            merman_core::Error::OperationCancelled(_) => "cancelled",
            merman_core::Error::ThemeColor(_) => "theme_color",
            merman_core::Error::RuntimePolicy(_) => "runtime_policy",
            merman_core::Error::DetectType(_) => "detect_type",
            merman_core::Error::UnsupportedDiagram { .. } => "unsupported_diagram",
            merman_core::Error::DiagramParse { .. } => "diagram_parse",
            merman_core::Error::MalformedFrontMatter => "front_matter",
            merman_core::Error::InvalidDirectiveJson { .. } => "directive",
            merman_core::Error::InvalidFrontMatterYaml { .. } => "front_matter",
        }
    }
}

impl std::fmt::Debug for TerminalDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalDiagnostic")
            .field("kind", &self.kind())
            .field("message", &self.terminal_safe_message())
            .field("details", &self.terminal_diagnostic_details())
            .finish()
    }
}

impl std::fmt::Display for TerminalDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

impl std::error::Error for TerminalDiagnostic {}

impl From<merman_core::Error> for TerminalDiagnostic {
    fn from(error: merman_core::Error) -> Self {
        Self { error }
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for TerminalDiagnostic {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::from(merman_core::Error::RuntimePolicy(error))
    }
}

impl TerminalRuntimePolicyError {
    #[must_use]
    pub fn terminal_safe_message(&self) -> String {
        safe_runtime_policy_error(&self.error)
    }

    #[must_use]
    pub fn terminal_diagnostic_details(&self) -> TerminalDiagnosticDetails {
        runtime_policy_details()
    }

    #[must_use]
    pub const fn missing_capability(&self) -> Option<merman_core::runtime::RuntimeCapability> {
        self.error.missing_capability()
    }
}

impl std::fmt::Debug for TerminalRuntimePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalRuntimePolicyError")
            .field("message", &self.terminal_safe_message())
            .field("details", &self.terminal_diagnostic_details())
            .finish()
    }
}

impl std::fmt::Display for TerminalRuntimePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

impl std::error::Error for TerminalRuntimePolicyError {}

impl From<merman_core::runtime::RuntimePolicyError> for TerminalRuntimePolicyError {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self { error }
    }
}

/// Normalizes untrusted authored text for terminal display without changing printable text.
///
/// Line feeds are structural. CRLF is normalized to LF; every other C0/C1 control, ESC, DEL, and
/// bidirectional formatting control is rendered as an uppercase `\u{HEX}` escape. Standalone
/// zero-width graphemes are escaped scalar by scalar, while joiners and variation selectors remain
/// intact when they belong to a positive-width grapheme. Applying this function twice is
/// idempotent.
#[must_use]
pub fn normalize_terminal_text(value: &str) -> Cow<'_, str> {
    if !needs_normalization(value) {
        return Cow::Borrowed(value);
    }

    let mut output = String::with_capacity(value.len());
    let normalized = visit_normalized_segments(value, |segment| {
        match segment {
            NormalizedSegment::Grapheme(grapheme) => output.push_str(grapheme),
            NormalizedSegment::VisibleEscape(ch) => push_visible_escape(&mut output, ch),
            NormalizedSegment::LineBreak => output.push('\n'),
        }
        Ok::<(), Infallible>(())
    });
    match normalized {
        Ok(()) => Cow::Owned(output),
        Err(never) => match never {},
    }
}

/// Produces a bounded, terminal-safe human-readable diagnostic.
///
/// Structured codes, spans, fields, and diagram types remain available through
/// [`TerminalDiagnosticDetails`] rather than being embedded into an unbounded display string.
#[must_use]
pub fn normalize_terminal_diagnostic(value: &str) -> String {
    let (raw_prefix, input_truncated) = bounded_grapheme_prefix(value);

    let safe = normalize_terminal_text(raw_prefix);
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

fn bounded_grapheme_prefix(value: &str) -> (&str, bool) {
    if value.len() <= MAX_DIAGNOSTIC_INPUT_BYTES {
        return (value, false);
    }

    let mut byte_end = MAX_DIAGNOSTIC_INPUT_BYTES;
    while !value.is_char_boundary(byte_end) {
        byte_end -= 1;
    }
    let byte_prefix = &value[..byte_end];
    let trailing_grapheme_start = byte_prefix
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(start, _)| start);
    (&byte_prefix[..trailing_grapheme_start], true)
}

fn safe_parse_error(error: &merman_core::Error) -> String {
    match error {
        merman_core::Error::OperationCancelled(_) => "parse operation cancelled".to_string(),
        merman_core::Error::ThemeColor(error) => match error {
            merman_core::theme_color::ColorError::UnsupportedFormat { input } => {
                bounded_message("Unsupported color format: \"", input, "\"")
            }
            merman_core::theme_color::ColorError::MixedColorSpaces => {
                "Cannot change both RGB and HSL channels at the same time".to_string()
            }
        },
        merman_core::Error::RuntimePolicy(error) => safe_runtime_policy_error(error),
        merman_core::Error::DetectType(error) => bounded_message(
            "No diagram type detected matching given configuration for text: ",
            &error.text,
            "",
        ),
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            bounded_message("Unsupported diagram type: ", diagram_type, "")
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => bounded_two_field_message(
            "Diagram parse error (",
            diagram_type,
            "): ",
            diagnostic.message(),
        ),
        merman_core::Error::MalformedFrontMatter => {
            "Malformed YAML front-matter. If you were trying to use a YAML front-matter, please ensure that you've correctly opened and closed the YAML front-matter with un-indented `---` blocks".to_string()
        }
        merman_core::Error::InvalidDirectiveJson { message } => {
            bounded_message("Invalid directive JSON: ", message, "")
        }
        merman_core::Error::InvalidFrontMatterYaml { message } => {
            bounded_message("Invalid YAML front-matter: ", message, "")
        }
    }
}

fn safe_parse_details(error: &merman_core::Error) -> TerminalDiagnosticDetails {
    let mut details = TerminalDiagnosticDetails {
        code: "merman.parse".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
    };
    match error {
        merman_core::Error::OperationCancelled(_) => {
            details.code = "merman.parse.cancelled".to_string();
        }
        merman_core::Error::ThemeColor(_) => {
            details.code = "merman.parse.theme_color".to_string();
            details.field = Some("theme_color".to_string());
        }
        merman_core::Error::RuntimePolicy(_) => return runtime_policy_details(),
        merman_core::Error::DetectType(_) => {
            details.code = "merman.parse.no_diagram_detected".to_string();
        }
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            details.code = "merman.parse.unsupported_diagram".to_string();
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            details.code = diagnostic
                .code()
                .map(normalize_terminal_diagnostic)
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| "merman.parse.diagram_parse".to_string());
            details.span = diagnostic.span();
            details.span_kind = diagnostic.span().map(|_| diagnostic.span_kind());
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::MalformedFrontMatter => {
            details.code = "merman.parse.front_matter.malformed".to_string();
            details.field = Some("front_matter".to_string());
        }
        merman_core::Error::InvalidDirectiveJson { .. } => {
            details.code = "merman.parse.directive.invalid_json".to_string();
            details.field = Some("directive".to_string());
        }
        merman_core::Error::InvalidFrontMatterYaml { .. } => {
            details.code = "merman.parse.front_matter.invalid_yaml".to_string();
            details.field = Some("front_matter".to_string());
        }
    }
    details
}

fn runtime_policy_details() -> TerminalDiagnosticDetails {
    TerminalDiagnosticDetails {
        code: "merman.runtime_policy".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
    }
}

fn safe_runtime_policy_error(error: &merman_core::runtime::RuntimePolicyError) -> String {
    match error {
        merman_core::runtime::RuntimePolicyError::SystemTimeZone(message) => {
            bounded_message("system time-zone adapter failed: ", message, "")
        }
        merman_core::runtime::RuntimePolicyError::SystemRandom(message) => {
            bounded_message("system random adapter failed: ", message, "")
        }
        _ => normalize_terminal_diagnostic(&error.to_string()),
    }
}

fn bounded_message(prefix: &str, value: &str, suffix: &str) -> String {
    let value = normalize_terminal_diagnostic(value);
    normalize_terminal_diagnostic(&format!("{prefix}{value}{suffix}"))
}

fn bounded_two_field_message(prefix: &str, first: &str, separator: &str, second: &str) -> String {
    let first = normalize_terminal_diagnostic(first);
    let second = normalize_terminal_diagnostic(second);
    normalize_terminal_diagnostic(&format!("{prefix}{first}{separator}{second}"))
}

#[derive(Clone, Copy)]
enum NormalizedSegment<'a> {
    Grapheme(&'a str),
    VisibleEscape(char),
    LineBreak,
}

fn visit_normalized_segments<'a, E>(
    value: &'a str,
    mut visit: impl FnMut(NormalizedSegment<'a>) -> Result<(), E>,
) -> Result<(), E> {
    for grapheme in value.graphemes(true) {
        if grapheme == "\r\n" || grapheme == "\n" {
            visit(NormalizedSegment::LineBreak)?;
            continue;
        }

        if grapheme.chars().any(needs_control_escape) {
            for (offset, ch) in grapheme.char_indices() {
                let segment = if ch == '\n' {
                    NormalizedSegment::LineBreak
                } else if needs_control_escape(ch) || UnicodeWidthChar::width(ch).unwrap_or(0) == 0
                {
                    NormalizedSegment::VisibleEscape(ch)
                } else {
                    NormalizedSegment::Grapheme(&grapheme[offset..offset + ch.len_utf8()])
                };
                visit(segment)?;
            }
            continue;
        }

        if UnicodeWidthStr::width(grapheme) == 0 {
            for ch in grapheme.chars() {
                visit(NormalizedSegment::VisibleEscape(ch))?;
            }
        } else {
            visit(NormalizedSegment::Grapheme(grapheme))?;
        }
    }
    Ok(())
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

fn visible_escape_len(ch: char) -> usize {
    let mut digits = 1;
    let mut value = u32::from(ch);
    while value >= 16 {
        digits += 1;
        value /= 16;
    }
    4 + digits
}

fn visible_escape(ch: char, buffer: &mut [u8; 10]) -> &str {
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

fn push_visible_escape(output: &mut String, ch: char) {
    let mut buffer = [0u8; 10];
    output.push_str(visible_escape(ch, &mut buffer));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_text_normalization_is_safe_and_idempotent() {
        let once = normalize_terminal_text("a\u{1b}\u{202e}\u{301}\r\nb").into_owned();
        let twice = normalize_terminal_text(&once);

        assert_eq!(once, "a\\u{1B}\\u{202E}\\u{301}\nb");
        assert_eq!(twice, once);
    }

    #[test]
    fn terminal_diagnostic_is_bounded_without_splitting_graphemes() {
        let input = "👩‍💻".repeat(MAX_DIAGNOSTIC_GRAPHEMES + 1);
        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(
            normalized.graphemes(true).count(),
            MAX_DIAGNOSTIC_GRAPHEMES + DIAGNOSTIC_ELLIPSIS.len()
        );
        assert!(normalized.len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(normalized.ends_with(DIAGNOSTIC_ELLIPSIS));
    }

    #[test]
    fn terminal_diagnostic_bounds_one_oversized_grapheme_before_segmentation() {
        let input = format!("a{}", "\u{301}".repeat(MAX_DIAGNOSTIC_INPUT_BYTES * 4));
        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(normalized, DIAGNOSTIC_ELLIPSIS);
    }

    #[test]
    fn parser_projection_is_safe_and_preserves_structured_context() {
        let span = merman_core::SourceSpan::new(4, 9);
        let diagnostic = merman_core::ParseDiagnostic::new(format!(
            "bad\u{7}{}",
            "\u{301}".repeat(MAX_DIAGNOSTIC_INPUT_BYTES)
        ))
        .with_span(span, merman_core::ParseDiagnosticSpanKind::Exact)
        .with_code("merman.test\u{1b}");
        let error = TerminalDiagnostic::from(merman_core::Error::diagram_parse_diagnostic(
            "flow\u{1b}",
            diagnostic,
        ));

        let display = error.to_string();
        let debug = format!("{error:?}");
        let details = error.terminal_diagnostic_details();

        assert!(display.len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(!display.contains('\u{1b}'));
        assert!(!display.contains('\u{7}'));
        assert!(!debug.contains('\u{1b}'));
        assert!(!debug.contains('\u{7}'));
        assert_eq!(details.code, "merman.test\\u{1B}");
        assert_eq!(details.span, Some(span));
        assert_eq!(
            details.span_kind,
            Some(merman_core::ParseDiagnosticSpanKind::Exact)
        );
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{1B}"));
        assert!(std::error::Error::source(&error).is_none());

        let render_error = crate::RenderError::from(merman_core::Error::diagram_parse_fallback(
            "state\u{1b}",
            "bad\u{7}input",
        ));
        assert!(!render_error.to_string().contains('\u{1b}'));
        assert!(!render_error.to_string().contains('\u{7}'));
        assert!(!format!("{render_error:?}").contains('\u{1b}'));

        let detect_error = TerminalDiagnostic::from(merman_core::Error::DetectType(
            merman_core::detect::DetectTypeError {
                text: "not-a-diagram".to_string(),
            },
        ));
        assert_eq!(
            detect_error.to_string(),
            "No diagram type detected matching given configuration for text: not-a-diagram"
        );
    }

    #[test]
    fn runtime_policy_projection_is_safe_and_preserves_capability_classification() {
        let diagnostic = TerminalRuntimePolicyError::from(
            merman_core::runtime::RuntimePolicyError::SystemTimeZone(
                "adapter\u{1b}\u{7}".to_string(),
            ),
        );
        assert!(!diagnostic.to_string().contains('\u{1b}'));
        assert!(!diagnostic.to_string().contains('\u{7}'));
        assert!(!format!("{diagnostic:?}").contains('\u{1b}'));
        assert_eq!(
            diagnostic.terminal_diagnostic_details().code,
            "merman.runtime_policy"
        );
        assert_eq!(diagnostic.missing_capability(), None);

        let render_error =
            crate::RenderError::from(merman_core::runtime::RuntimePolicyError::SystemRandom(
                "adapter\u{1b}\u{7}".to_string(),
            ));
        assert!(!render_error.to_string().contains('\u{1b}'));
        assert!(!render_error.to_string().contains('\u{7}'));
        assert!(!format!("{render_error:?}").contains('\u{1b}'));

        let missing = TerminalRuntimePolicyError::from(
            merman_core::runtime::RuntimePolicyError::MissingCapability(
                merman_core::runtime::RuntimeCapability::SystemRandom,
            ),
        );
        assert_eq!(
            missing.missing_capability(),
            Some(merman_core::runtime::RuntimeCapability::SystemRandom)
        );
        assert!(std::error::Error::source(&missing).is_none());
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn facade_and_ascii_terminal_normalization_remain_identical() {
        let cases = [
            "plain text",
            "one\r\ntwo\tthree\rfour",
            "a\u{1b}\u{202e}\u{301}",
            "👩‍💻\u{200d}\u{fe0f}",
            "漢字",
        ];

        for input in cases {
            assert_eq!(
                normalize_terminal_text(input),
                merman_ascii::normalize_terminal_text(input),
                "normalization drifted for {input:?}"
            );
            assert_eq!(
                normalize_terminal_diagnostic(input),
                merman_ascii::normalize_terminal_diagnostic(input),
                "diagnostic normalization drifted for {input:?}"
            );
        }
    }
}
