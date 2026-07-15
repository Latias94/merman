use serde::{Deserialize, Serialize};
use std::ops::Range;

pub const ANALYSIS_PAYLOAD_VERSION: u32 = 1;
// Diagnostics and facts are independent contracts that both begin at version 1.
pub const ANALYSIS_FACTS_PAYLOAD_VERSION: u32 = 1;

fn deserialize_payload_version<'de, D>(
    deserializer: D,
    expected: u32,
    contract: &str,
) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let version = u32::deserialize(deserializer)?;
    if version == expected {
        Ok(version)
    } else {
        Err(serde::de::Error::custom(format_args!(
            "unsupported {contract} payload version {version}; expected {expected}"
        )))
    }
}

fn deserialize_analysis_payload_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_payload_version(deserializer, ANALYSIS_PAYLOAD_VERSION, "analysis")
}

pub(crate) fn deserialize_analysis_facts_payload_version<'de, D>(
    deserializer: D,
) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_payload_version(
        deserializer,
        ANALYSIS_FACTS_PAYLOAD_VERSION,
        "analysis facts",
    )
}

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

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
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
    Internal,
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
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Utf16Position {
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: Utf16Position,
    pub end: Utf16Position,
}

impl LspRange {
    pub const fn new(start: Utf16Position, end: Utf16Position) -> Self {
        Self { start, end }
    }
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
        byte_range: Range<usize>,
        start: SourcePosition,
        end: SourcePosition,
        lsp_range: LspRange,
    ) -> Self {
        Self {
            byte_start: byte_range.start,
            byte_end: byte_range.end,
            line: start.line,
            column: start.column,
            end_line: end.line,
            end_column: end.column,
            lsp_range,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRelated {
    pub message: String,
    pub span: Option<DiagnosticSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFixEdit {
    pub span: DiagnosticSpan,
    pub replacement: String,
}

impl DiagnosticFixEdit {
    pub fn new(span: DiagnosticSpan, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFix {
    pub title: String,
    pub edits: Vec<DiagnosticFixEdit>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_preferred: bool,
}

impl DiagnosticFix {
    pub fn new(title: impl Into<String>, edits: Vec<DiagnosticFixEdit>) -> Self {
        Self {
            title: title.into(),
            edits,
            is_preferred: false,
        }
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<DiagnosticFix>,
}

impl AnalysisDiagnostic {
    pub fn new(
        id: impl Into<String>,
        severity: DiagnosticSeverity,
        category: DiagnosticCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            category,
            message: message.into(),
            code: None,
            code_name: None,
            diagram_type: None,
            span: None,
            related: Vec::new(),
            help: None,
            fixes: Vec::new(),
        }
    }

    pub fn error(
        id: impl Into<String>,
        category: DiagnosticCategory,
        message: impl Into<String>,
    ) -> Self {
        Self::new(id, DiagnosticSeverity::Error, category, message)
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

    pub fn with_fix(mut self, fix: DiagnosticFix) -> Self {
        self.fixes.push(fix);
        self
    }

    pub fn with_fixes(mut self, fixes: impl IntoIterator<Item = DiagnosticFix>) -> Self {
        self.fixes.extend(fixes);
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
    #[serde(deserialize_with = "deserialize_analysis_payload_version")]
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
