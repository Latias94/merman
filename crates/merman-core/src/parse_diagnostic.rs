#![cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]

use crate::error::{ParseDiagnostic, ParseDiagnosticSpanKind, ParseErrorSourceSpan};
use crate::source_span::SourceSpan;

#[cfg(feature = "editor-facts")]
pub(crate) fn lalrpop_recovery_span<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    fallback_offset: usize,
) -> SourceSpan {
    match error {
        lalrpop_util::ParseError::InvalidToken { location } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, .. }
        | lalrpop_util::ParseError::ExtraToken { token } => SourceSpan::new(token.0, token.2),
        lalrpop_util::ParseError::User { .. } => SourceSpan::new(fallback_offset, fallback_offset),
    }
}

pub(crate) fn lalrpop_parse_diagnostic<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    fallback_offset: usize,
) -> ParseDiagnostic
where
    T: std::fmt::Debug,
    E: std::fmt::Display + ParseErrorSourceSpan,
{
    let message = format_lalrpop_parse_error(error);
    match error {
        lalrpop_util::ParseError::InvalidToken { location }
        | lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
            ParseDiagnostic::new(message).with_span(
                SourceSpan::new(*location, *location),
                ParseDiagnosticSpanKind::InsertionPoint,
            )
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, .. }
        | lalrpop_util::ParseError::ExtraToken { token } => ParseDiagnostic::new(message)
            .with_span(
                SourceSpan::new(token.0, token.2),
                ParseDiagnosticSpanKind::Exact,
            ),
        lalrpop_util::ParseError::User { error } => {
            if let Some(span) = error.source_span() {
                ParseDiagnostic::new(message).with_span(span, ParseDiagnosticSpanKind::Exact)
            } else {
                ParseDiagnostic::new(message).with_span(
                    SourceSpan::new(fallback_offset, fallback_offset),
                    ParseDiagnosticSpanKind::Fallback,
                )
            }
        }
    }
}

pub(crate) fn format_lalrpop_parse_error<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
) -> String
where
    T: std::fmt::Debug,
    E: std::fmt::Display,
{
    match error {
        lalrpop_util::ParseError::InvalidToken { .. } => "unexpected token".to_string(),
        lalrpop_util::ParseError::UnrecognizedEof { expected, .. } => {
            let expected = format_expected_tokens(expected);
            if expected.is_empty() {
                "unexpected end of input".to_string()
            } else {
                format!("unexpected end of input; expected {expected}")
            }
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, expected } => {
            let expected = format_expected_tokens(expected);
            let found = format_found_token(&token.1);
            if expected.is_empty() {
                format!("unexpected {found}")
            } else {
                format!("unexpected {found}; expected {expected}")
            }
        }
        lalrpop_util::ParseError::ExtraToken { token } => {
            format!("unexpected extra {}", format_found_token(&token.1))
        }
        lalrpop_util::ParseError::User { error } => error.to_string(),
    }
}

fn format_expected_tokens(expected: &[String]) -> String {
    expected
        .iter()
        .map(|token| humanize_expected_token(token))
        .collect::<Vec<_>>()
        .join(", ")
}

fn humanize_expected_token(token: &str) -> String {
    match token {
        "Id" => "node identifier".to_string(),
        "EdgeLabel" => "edge label".to_string(),
        "Direction" => "diagram direction".to_string(),
        "AlphaNumToken" => "identifier".to_string(),
        "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "StringLit" => "string literal".to_string(),
        other => humanize_token_name(other),
    }
}

fn format_found_token<T>(token: &T) -> String
where
    T: std::fmt::Debug,
{
    let debug = format!("{token:?}");
    let variant = debug
        .split_once('(')
        .map(|(name, _)| name)
        .unwrap_or(debug.as_str());

    match variant {
        "Sep" | "Newline" => "statement separator".to_string(),
        "StyleSep" => "style separator".to_string(),
        "Amp" => "`&`".to_string(),
        "Comma" => "`,`".to_string(),
        "Plus" => "`+`".to_string(),
        "Minus" => "`-`".to_string(),
        "Arrow" => "edge operator".to_string(),
        "SignalType" => "message operator".to_string(),
        "Id" | "Actor" | "StyledId" => "identifier".to_string(),
        "Direction" | "DirectionStmt" => "diagram direction".to_string(),
        "EdgeLabel" | "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "NodeLabel" | "StateDescr" | "CompositState" => "node label".to_string(),
        "StringLit" => "string literal".to_string(),
        "Num" => "number".to_string(),
        other => humanize_token_name(other),
    }
}

fn humanize_token_name(token: &str) -> String {
    let token = token.strip_prefix("Kw").unwrap_or(token);
    let mut out = String::new();
    let mut previous_is_lowercase = false;

    for ch in token.chars() {
        if ch == '_' || ch == '-' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            previous_is_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_is_lowercase && !out.ends_with(' ') {
            out.push(' ');
        }

        if ch.is_ascii_digit() && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }

        out.push(ch.to_ascii_lowercase());
        previous_is_lowercase = ch.is_ascii_lowercase();
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::lalrpop_parse_diagnostic;
    use crate::ParseDiagnosticSpanKind;

    #[test]
    fn lalrpop_parse_diagnostic_preserves_unrecognized_token_span() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::UnrecognizedToken {
            token: (3, "bad", 6),
            expected: vec!["ID".to_string()],
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 10);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 3);
        assert_eq!(span.end, 6);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        assert!(diagnostic.message().contains("\"bad\""));
    }

    #[test]
    fn lalrpop_parse_diagnostic_preserves_eof_insertion_point() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::UnrecognizedEof {
            location: 12,
            expected: vec!["]".to_string()],
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 99);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 12);
        assert_eq!(span.end, 12);
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
        assert!(diagnostic.message().contains("unexpected end of input"));
    }

    #[test]
    fn lalrpop_parse_diagnostic_marks_user_errors_as_fallback() {
        let error = lalrpop_util::ParseError::<usize, &str, String>::User {
            error: "custom parse failure".to_string(),
        };

        let diagnostic = lalrpop_parse_diagnostic(&error, 8);

        let span = diagnostic.span().expect("diagnostic span");
        assert_eq!(span.start, 8);
        assert_eq!(span.end, 8);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
        assert_eq!(diagnostic.message(), "custom parse failure");
    }
}
