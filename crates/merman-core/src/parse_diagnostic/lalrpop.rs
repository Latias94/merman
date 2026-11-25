use super::{format_found_token, unexpected_eof_message, unexpected_token_message};
use crate::error::{ParseDiagnostic, ParseDiagnosticSpanKind, ParseErrorSourceSpan};
use merman_model::source::SourceSpan;

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
            unexpected_eof_message(expected.iter().map(String::as_str))
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, expected } => {
            unexpected_token_message(&token.1, expected.iter().map(String::as_str))
        }
        lalrpop_util::ParseError::ExtraToken { token } => {
            format!("unexpected extra {}", format_found_token(&token.1))
        }
        lalrpop_util::ParseError::User { error } => error.to_string(),
    }
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
