use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use crate::retained_weight::{ARC_ALLOCATION_OVERHEAD, RetainedWeight};

pub const ANALYSIS_PAYLOAD_VERSION: u32 = 1;
// Diagnostics and facts are independent contracts; facts advanced to schema 2 for the deliberate
// Flowchart-rich deletion while the diagnostics payload remains at schema 1.
pub const ANALYSIS_FACTS_PAYLOAD_VERSION: u32 = 2;

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

    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = RetainedWeight::default();
        weight.add_optional_string(&self.path);
        weight.add_string(&self.language);
        weight.finish()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(
        serialize_with = "serialize_diagnostic_fix_edits",
        deserialize_with = "deserialize_diagnostic_fix_edits"
    )]
    pub edits: Arc<[DiagnosticFixEdit]>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_preferred: bool,
}

impl DiagnosticFix {
    pub fn new(title: impl Into<String>, edits: Vec<DiagnosticFixEdit>) -> Self {
        Self {
            title: title.into(),
            edits: Arc::from(edits),
            is_preferred: false,
        }
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

fn serialize_diagnostic_fix_edits<S>(
    edits: &Arc<[DiagnosticFixEdit]>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    edits.as_ref().serialize(serializer)
}

fn deserialize_diagnostic_fix_edits<'de, D>(
    deserializer: D,
) -> Result<Arc<[DiagnosticFixEdit]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<DiagnosticFixEdit>::deserialize(deserializer).map(Arc::from)
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

#[derive(Debug, Default)]
pub(crate) struct DiagnosticRetainedWeight {
    weight: RetainedWeight,
    fix_edit_allocations: BTreeSet<usize>,
}

pub(crate) struct DiagnosticDynamicWeight<'a> {
    pub(crate) message_capacity: usize,
    pub(crate) diagram_type_capacity: Option<usize>,
    pub(crate) help_capacity: Option<usize>,
    pub(crate) related: &'a [DiagnosticRelated],
    pub(crate) related_capacity: usize,
    pub(crate) fixes: &'a [DiagnosticFix],
    pub(crate) fixes_capacity: usize,
}

impl DiagnosticRetainedWeight {
    pub(crate) fn add_diagnostic(&mut self, diagnostic: &AnalysisDiagnostic) {
        self.weight.add_string(&diagnostic.id);
        self.add_dynamic(DiagnosticDynamicWeight {
            message_capacity: diagnostic.message.capacity(),
            diagram_type_capacity: diagnostic.diagram_type.as_ref().map(String::capacity),
            help_capacity: diagnostic.help.as_ref().map(String::capacity),
            related: &diagnostic.related,
            related_capacity: diagnostic.related.capacity(),
            fixes: &diagnostic.fixes,
            fixes_capacity: diagnostic.fixes.capacity(),
        });
        self.weight.add_optional_string(&diagnostic.code_name);
    }

    pub(crate) fn add_candidate(&mut self, fields: DiagnosticDynamicWeight<'_>) {
        self.add_dynamic(fields);
    }

    fn add_dynamic(&mut self, fields: DiagnosticDynamicWeight<'_>) {
        let weight = &mut self.weight;
        weight.add(fields.message_capacity);
        if let Some(capacity) = fields.diagram_type_capacity {
            weight.add(capacity);
        }
        if let Some(capacity) = fields.help_capacity {
            weight.add(capacity);
        }
        weight.add_array::<DiagnosticRelated>(fields.related_capacity);
        for related in fields.related {
            weight.add_string(&related.message);
        }
        weight.add_array::<DiagnosticFix>(fields.fixes_capacity);
        for fix in fields.fixes {
            weight.add_string(&fix.title);
            if fix.edits.is_empty() {
                continue;
            }
            let allocation = Arc::as_ptr(&fix.edits) as *const DiagnosticFixEdit as usize;
            if !self.fix_edit_allocations.insert(allocation) {
                continue;
            }
            weight.add(ARC_ALLOCATION_OVERHEAD);
            weight.add_array::<DiagnosticFixEdit>(fix.edits.len());
            for edit in fix.edits.iter() {
                weight.add_string(&edit.replacement);
            }
        }
    }

    pub(crate) fn finish(self) -> usize {
        self.weight.finish()
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
        let mut summary = Self::default();
        for diagnostic in diagnostics {
            summary.record(diagnostic);
        }
        summary
    }

    pub(crate) fn from_diagnostics_cancellable(
        diagnostics: &[AnalysisDiagnostic],
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        let mut summary = Self::default();
        for (index, diagnostic) in diagnostics.iter().enumerate() {
            if index.is_multiple_of(128) {
                cancellation.checkpoint()?;
            }
            summary.record(diagnostic);
        }
        cancellation.checkpoint()?;
        Ok(summary)
    }

    fn record(&mut self, diagnostic: &AnalysisDiagnostic) {
        match diagnostic.severity {
            DiagnosticSeverity::Error => self.errors += 1,
            DiagnosticSeverity::Warning => self.warnings += 1,
            DiagnosticSeverity::Info => self.infos += 1,
            DiagnosticSeverity::Hint => self.hints += 1,
        }
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

    pub(crate) fn new_cancellable(
        source: SourceDescriptor,
        diagnostics: Vec<AnalysisDiagnostic>,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        cancellation.checkpoint()?;
        let summary = Summary::from_diagnostics_cancellable(&diagnostics, cancellation)?;
        cancellation.checkpoint()?;
        Ok(Self {
            version: ANALYSIS_PAYLOAD_VERSION,
            valid: summary.errors == 0,
            summary,
            source,
            diagnostics,
        })
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

    /// Estimates heap allocations owned by this payload, excluding the payload value itself.
    ///
    /// This is a stable, saturating cache weight rather than allocator-specific RSS accounting.
    pub fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = RetainedWeight::default();
        let mut diagnostic_weight = DiagnosticRetainedWeight::default();
        weight.add(self.source.estimated_owned_heap_bytes());
        weight.add_array::<AnalysisDiagnostic>(self.diagnostics.capacity());
        for diagnostic in &self.diagnostics {
            diagnostic_weight.add_diagnostic(diagnostic);
        }
        weight.add(diagnostic_weight.finish());
        weight.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_span() -> DiagnosticSpan {
        DiagnosticSpan::new(
            0..0,
            SourcePosition::new(1, 1),
            SourcePosition::new(1, 1),
            LspRange::new(
                Utf16Position {
                    line: 0,
                    character: 0,
                },
                Utf16Position {
                    line: 0,
                    character: 0,
                },
            ),
        )
    }

    #[test]
    fn diagnostic_fix_clones_share_edits_without_changing_json() {
        let fix = DiagnosticFix::new(
            "shared fix",
            vec![DiagnosticFixEdit::new(empty_span(), "replacement")],
        )
        .preferred();
        let cloned = fix.clone();

        assert!(Arc::ptr_eq(&fix.edits, &cloned.edits));
        let json = serde_json::to_value(&fix).unwrap();
        assert!(json["edits"].is_array());
        assert_eq!(serde_json::from_value::<DiagnosticFix>(json).unwrap(), fix);
    }

    #[test]
    fn payload_weight_covers_related_messages_and_nested_fix_edits() {
        let base = AnalysisPayload::new(
            SourceDescriptor::diagram(),
            vec![AnalysisDiagnostic::error(
                "test",
                DiagnosticCategory::Semantic,
                "message",
            )],
        );
        let mut related = base.clone();
        related.diagnostics[0].related.push(DiagnosticRelated {
            message: "related allocation".repeat(8),
            span: None,
        });
        let mut fixed = related.clone();
        fixed.diagnostics[0].fixes.push(DiagnosticFix::new(
            "fix allocation".repeat(8),
            vec![DiagnosticFixEdit::new(
                empty_span(),
                "replacement allocation".repeat(8),
            )],
        ));

        assert!(related.estimated_owned_heap_bytes() > base.estimated_owned_heap_bytes());
        assert!(fixed.estimated_owned_heap_bytes() > related.estimated_owned_heap_bytes());
    }

    #[test]
    fn cancellable_payload_construction_checks_the_summary_tail() {
        let diagnostics = (0..512)
            .map(|index| {
                AnalysisDiagnostic::error(
                    format!("test-{index}"),
                    DiagnosticCategory::Semantic,
                    "message",
                )
            })
            .collect();
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(1);

        assert!(matches!(
            AnalysisPayload::new_cancellable(
                SourceDescriptor::diagram(),
                diagnostics,
                &cancellation,
            ),
            Err(crate::AnalysisCancelled)
        ));
    }
}
