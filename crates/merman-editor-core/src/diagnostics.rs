use crate::types::Range;
use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticFix, DiagnosticSeverity, DiagnosticSpan,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const RECOVERED_EDITOR_FACTS_RULE_ID: &str = "merman.parse.recovered_editor_facts";

pub fn analysis_payload_to_diagnostics(payload: &AnalysisPayload) -> Vec<EditorDiagnostic> {
    let mut seen = HashSet::new();
    payload
        .diagnostics
        .iter()
        .map(analysis_diagnostic_to_editor)
        .filter(|diagnostic| seen.insert(diagnostic.dedup_key()))
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
        message: diagnostic_message(diagnostic),
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

fn diagnostic_message(diagnostic: &AnalysisDiagnostic) -> String {
    if diagnostic.id == RECOVERED_EDITOR_FACTS_RULE_ID {
        return humanize_recovered_parser_message(&diagnostic.message);
    }
    diagnostic.message.clone()
}

fn humanize_recovered_parser_message(message: &str) -> String {
    let detail = message
        .split_once(" after parse error: ")
        .map(|(_, detail)| detail)
        .or_else(|| message.split_once(" from ").map(|(_, detail)| detail))
        .or_else(|| message.split_once(" before ").map(|(_, detail)| detail))
        .unwrap_or(message)
        .trim();

    if detail.is_empty() {
        "Mermaid syntax could not be fully parsed.".to_string()
    } else {
        format!("Mermaid syntax issue: {detail}")
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

impl EditorDiagnostic {
    fn dedup_key(&self) -> DiagnosticDedupKey {
        DiagnosticDedupKey {
            range: self.range,
            severity: self.severity.as_str().to_string(),
            code: self.code.clone(),
            source: self.source.clone(),
            message: self.message.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DiagnosticDedupKey {
    range: Range,
    severity: String,
    code: EditorDiagnosticCode,
    source: String,
    message: String,
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
    pub fixes: Vec<DiagnosticFix>,
}
