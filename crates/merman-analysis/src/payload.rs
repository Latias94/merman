use serde::{Deserialize, Serialize};

pub const ANALYSIS_PAYLOAD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Diagram,
    Markdown,
    Mdx,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diagram => "diagram",
            Self::Markdown => "markdown",
            Self::Mdx => "mdx",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDescriptor {
    pub kind: SourceKind,
    pub path: Option<String>,
    pub diagram_index: Option<usize>,
    pub language: String,
}

impl SourceDescriptor {
    pub fn diagram() -> Self {
        Self {
            kind: SourceKind::Diagram,
            path: None,
            diagram_index: None,
            language: "mermaid".to_string(),
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_diagram_index(mut self, diagram_index: usize) -> Self {
        self.diagram_index = Some(diagram_index);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    Parse,
    Semantic,
    Config,
    Resource,
    Compatibility,
    Layout,
    Render,
}

impl DiagnosticCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Semantic => "semantic",
            Self::Config => "config",
            Self::Resource => "resource",
            Self::Compatibility => "compatibility",
            Self::Layout => "layout",
            Self::Render => "render",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Utf16Position {
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: Utf16Position,
    pub end: Utf16Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub lsp_range: LspRange,
}

impl DiagnosticSpan {
    pub const fn new(
        byte_start: usize,
        byte_end: usize,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
        lsp_start: Utf16Position,
        lsp_end: Utf16Position,
    ) -> Self {
        Self {
            byte_start,
            byte_end,
            line,
            column,
            end_line,
            end_column,
            lsp_range: LspRange {
                start: lsp_start,
                end: lsp_end,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRelated {
    pub message: String,
    pub span: Option<DiagnosticSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisDiagnostic {
    pub id: String,
    pub severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub message: String,
    pub code: Option<i32>,
    pub code_name: Option<String>,
    pub diagram_type: Option<String>,
    pub span: Option<DiagnosticSpan>,
    #[serde(default)]
    pub related: Vec<DiagnosticRelated>,
    pub help: Option<String>,
}

impl AnalysisDiagnostic {
    pub fn error(
        id: impl Into<String>,
        category: DiagnosticCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            severity: DiagnosticSeverity::Error,
            category,
            message: message.into(),
            code: None,
            code_name: None,
            diagram_type: None,
            span: None,
            related: Vec::new(),
            help: None,
        }
    }

    pub fn with_code(mut self, code: i32, code_name: impl Into<String>) -> Self {
        self.code = Some(code);
        self.code_name = Some(code_name.into());
        self
    }

    pub fn with_diagram_type(mut self, diagram_type: impl Into<String>) -> Self {
        self.diagram_type = Some(diagram_type.into());
        self
    }

    pub const fn with_span(mut self, span: DiagnosticSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

impl Summary {
    pub fn from_diagnostics(diagnostics: &[AnalysisDiagnostic]) -> Self {
        diagnostics
            .iter()
            .fold(Self::default(), |mut summary, diagnostic| {
                match diagnostic.severity {
                    DiagnosticSeverity::Error => summary.errors += 1,
                    DiagnosticSeverity::Warning => summary.warnings += 1,
                    DiagnosticSeverity::Info => summary.infos += 1,
                    DiagnosticSeverity::Hint => summary.hints += 1,
                }
                summary
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisPayload {
    pub version: u32,
    pub valid: bool,
    pub summary: Summary,
    pub source: SourceDescriptor,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

impl AnalysisPayload {
    pub fn new(source: SourceDescriptor, diagnostics: Vec<AnalysisDiagnostic>) -> Self {
        let summary = Summary::from_diagnostics(&diagnostics);
        Self {
            version: ANALYSIS_PAYLOAD_VERSION,
            valid: summary.errors == 0,
            summary,
            source,
            diagnostics,
        }
    }

    pub fn valid(source: SourceDescriptor) -> Self {
        Self::new(source, Vec::new())
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn to_pretty_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
