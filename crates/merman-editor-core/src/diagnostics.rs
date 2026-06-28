use crate::types::Range;
use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticFix, DiagnosticSeverity, DiagnosticSpan,
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
        code: diagnostic
            .code
            .map(EditorDiagnosticCode::Number)
            .unwrap_or_else(|| EditorDiagnosticCode::String(diagnostic.id.clone())),
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
        data: diagnostic_code_action_data(diagnostic),
    }
}

pub fn diagnostic_code_action_data(
    diagnostic: &AnalysisDiagnostic,
) -> Option<DiagnosticCodeActionData> {
    (!diagnostic.fixes.is_empty()).then(|| DiagnosticCodeActionData {
        id: diagnostic.id.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fixes: Vec<DiagnosticFix>,
}
