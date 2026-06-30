use crate::types::Range;
use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticSeverity,
    DiagnosticSpan,
};
use serde::{Deserialize, Serialize};

const RECOVERED_EDITOR_FACTS_RULE_ID: &str = "merman.parse.recovered_editor_facts";

pub fn analysis_payload_to_diagnostics(payload: &AnalysisPayload) -> Vec<EditorDiagnostic> {
    let mut diagnostics: Vec<EditorDiagnostic> = Vec::new();
    for diagnostic in payload
        .diagnostics
        .iter()
        .map(analysis_diagnostic_to_editor)
    {
        if let Some(existing) = diagnostics
            .iter_mut()
            .find(|existing| existing.dedup_key() == diagnostic.dedup_key())
        {
            existing.merge_metadata(diagnostic);
        } else {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
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
        data: diagnostic_data(diagnostic),
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

    fn merge_metadata(&mut self, other: Self) {
        if self.related.is_empty() {
            self.related = other.related;
        } else {
            self.related.extend(other.related);
        }
        match (&mut self.data, other.data) {
            (Some(current), Some(other)) => current.merge(other),
            (None, Some(other)) => self.data = Some(other),
            _ => {}
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

impl DiagnosticCodeActionData {
    fn merge(&mut self, other: Self) {
        if self.code.is_none() {
            self.code = other.code;
        }
        if self.code_name.is_none() {
            self.code_name = other.code_name;
        }
        if self.diagram_type.is_none() {
            self.diagram_type = other.diagram_type;
        }
        if self.help.is_none() {
            self.help = other.help;
        }
        if self.fixes.is_empty() {
            self.fixes = other.fixes;
        } else {
            self.fixes.extend(other.fixes);
        }
    }
}

fn default_diagnostic_category() -> DiagnosticCategory {
    DiagnosticCategory::Internal
}
