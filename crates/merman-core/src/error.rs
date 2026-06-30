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
    pub message: String,
    pub span: Option<SourceSpan>,
    pub span_kind: ParseDiagnosticSpanKind,
    pub code: Option<String>,
}

pub trait ParseErrorSourceSpan {
    fn source_span(&self) -> Option<SourceSpan>;
}

impl ParseErrorSourceSpan for String {
    fn source_span(&self) -> Option<SourceSpan> {
        None
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
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    DetectType(#[from] DetectTypeError),

    #[error("Unsupported diagram type: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },

    #[error("Diagram parse error ({diagram_type}): {message}")]
    DiagramParse {
        diagram_type: String,
        message: String,
    },

    #[error("Diagram parse error ({diagram_type}): {}", diagnostic.message)]
    DiagramParseDiagnostic {
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
    pub fn diagram_parse_diagnostic(
        diagram_type: impl Into<String>,
        diagnostic: ParseDiagnostic,
    ) -> Self {
        Self::DiagramParseDiagnostic {
            diagram_type: diagram_type.into(),
            diagnostic,
        }
    }
}
