use crate::types::Range;
use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticSeverity,
    DiagnosticSpan,
};
use serde::{Deserialize, Serialize};

pub fn analysis_payload_to_diagnostics(payload: &AnalysisPayload) -> Vec<EditorDiagnostic> {
    payload
        .diagnostics
        .iter()
        .map(analysis_diagnostic_to_editor)
        .collect()
}

pub fn analysis_diagnostic_to_editor(diagnostic: &AnalysisDiagnostic) -> EditorDiagnostic {
    EditorDiagnostic {
        range: diagnostic
            .span
            .as_ref()
            .map(range_from_span)
            .unwrap_or_default(),
        severity: diagnostic.severity,
        code: EditorDiagnosticCode::String(diagnostic.id.clone()),
        source: "merman".to_string(),
        message: diagnostic.message.clone(),
        related: diagnostic
            .related
            .iter()
            .filter_map(|related| {
                Some(EditorDiagnosticRelated {
                    message: related.message.clone(),
                    range: related.span.as_ref().map(range_from_span)?,
                })
            })
            .collect(),
        data: diagnostic_data(diagnostic),
    }
}

pub fn diagnostic_data(diagnostic: &AnalysisDiagnostic) -> Option<DiagnosticCodeActionData> {
    Some(DiagnosticCodeActionData {
        id: diagnostic.id.clone(),
        code: diagnostic.code,
        code_name: diagnostic.code_name.clone(),
        category: diagnostic.category,
        diagram_type: diagnostic.diagram_type.clone(),
        help: diagnostic.help.clone(),
        fixes: diagnostic.fixes.clone(),
    })
}

fn range_from_span(span: &DiagnosticSpan) -> Range {
    Range::new(
        crate::types::Position::new(span.lsp_range.start.line, span.lsp_range.start.character),
        crate::types::Position::new(span.lsp_range.end.line, span.lsp_range.end.character),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDiagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: EditorDiagnosticCode,
    pub source: String,
    pub message: String,
    pub related: Vec<EditorDiagnosticRelated>,
    pub data: Option<DiagnosticCodeActionData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EditorDiagnosticCode {
    Number(i32),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDiagnosticRelated {
    pub message: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCodeActionData {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_name: Option<String>,
    #[serde(default = "default_diagnostic_category")]
    pub category: DiagnosticCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagram_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<DiagnosticFix>,
}

fn default_diagnostic_category() -> DiagnosticCategory {
    DiagnosticCategory::Internal
}
