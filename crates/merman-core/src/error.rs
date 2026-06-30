use crate::{SourceSpan, detect::DetectTypeError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseDiagnosticSpanKind {
    Exact,
    InsertionPoint,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    message: String,
    span: Option<SourceSpan>,
    span_kind: ParseDiagnosticSpanKind,
    code: Option<String>,
}

pub(crate) trait ParseErrorSourceSpan {
    fn source_span(&self) -> Option<SourceSpan>;
}

impl ParseErrorSourceSpan for String {
    fn source_span(&self) -> Option<SourceSpan> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ParseDiagnostic, ParseDiagnosticSpanKind};
    use crate::SourceSpan;

    #[test]
    fn diagram_parse_fallback_builds_structured_fallback_diagnostic() {
        let error = Error::diagram_parse_fallback("flowchart", "bad syntax");
        let display = error.to_string();

        let Error::DiagramParse {
            diagram_type,
            diagnostic,
        } = error
        else {
            panic!("expected diagram parse error");
        };

        assert_eq!(diagram_type, "flowchart");
        assert_eq!(diagnostic.message(), "bad syntax");
        assert_eq!(diagnostic.span(), None);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
        assert_eq!(diagnostic.code(), None);
        assert_eq!(display, "Diagram parse error (flowchart): bad syntax");
    }

    #[test]
    fn diagram_parse_diagnostic_preserves_structured_metadata() {
        let span = SourceSpan::new(2, 5);
        let error = Error::diagram_parse_diagnostic(
            "state",
            ParseDiagnostic::new("unexpected token")
                .with_span(span, ParseDiagnosticSpanKind::Exact)
                .with_code("merman.test"),
        );

        let Error::DiagramParse {
            diagram_type,
            diagnostic,
        } = error
        else {
            panic!("expected diagram parse error");
        };

        assert_eq!(diagram_type, "state");
        assert_eq!(diagnostic.message(), "unexpected token");
        assert_eq!(diagnostic.span(), Some(span));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        assert_eq!(diagnostic.code(), Some("merman.test"));
    }
}

impl ParseDiagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            span_kind: ParseDiagnosticSpanKind::Fallback,
            code: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan, span_kind: ParseDiagnosticSpanKind) -> Self {
        self.span = Some(span);
        self.span_kind = span_kind;
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> Option<SourceSpan> {
        self.span
    }

    pub fn span_kind(&self) -> ParseDiagnosticSpanKind {
        self.span_kind
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    DetectType(#[from] DetectTypeError),

    #[error("Unsupported diagram type: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },

    #[error("Diagram parse error ({diagram_type}): {}", diagnostic.message())]
    DiagramParse {
        diagram_type: String,
        diagnostic: ParseDiagnostic,
    },

    #[error(
        "Malformed YAML front-matter. If you were trying to use a YAML front-matter, please ensure that you've correctly opened and closed the YAML front-matter with un-indented `---` blocks"
    )]
    MalformedFrontMatter,

    #[error("Invalid directive JSON: {message}")]
    InvalidDirectiveJson { message: String },

    #[error("Invalid YAML front-matter: {message}")]
    InvalidFrontMatterYaml { message: String },
}

impl Error {
    pub fn diagram_parse_fallback(
        diagram_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::diagram_parse_diagnostic(diagram_type, ParseDiagnostic::new(message))
    }

    pub fn diagram_parse_diagnostic(
        diagram_type: impl Into<String>,
        diagnostic: ParseDiagnostic,
    ) -> Self {
        Self::DiagramParse {
            diagram_type: diagram_type.into(),
            diagnostic,
        }
    }
}
