use merman_analysis::{AnalysisDiagnostic, AnalysisPayload, DiagnosticSeverity};
use merman_editor_core::{
    EditorDiagnostic, EditorDiagnosticCode, EditorDiagnosticRelated, Range as CoreRange,
    analysis_diagnostic_to_editor,
    analysis_payload_to_diagnostics as analysis_payload_to_editor_diagnostics,
};
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity as LspSeverity, Location,
    NumberOrString, Position, Range, Url,
};

pub fn analysis_payload_to_diagnostics(payload: &AnalysisPayload, uri: &Url) -> Vec<Diagnostic> {
    analysis_payload_to_editor_diagnostics(payload)
        .into_iter()
        .map(|diagnostic| editor_diagnostic_to_lsp(diagnostic, uri))
        .collect()
}

pub fn analysis_diagnostic_to_lsp(diagnostic: &AnalysisDiagnostic, uri: &Url) -> Diagnostic {
    editor_diagnostic_to_lsp(analysis_diagnostic_to_editor(diagnostic), uri)
}

pub fn editor_diagnostic_to_lsp(diagnostic: EditorDiagnostic, uri: &Url) -> Diagnostic {
    Diagnostic {
        range: range_to_lsp(diagnostic.range),
        severity: Some(severity_to_lsp(diagnostic.severity)),
        code: Some(match diagnostic.code {
            EditorDiagnosticCode::Number(code) => NumberOrString::Number(code),
            EditorDiagnosticCode::String(code) => NumberOrString::String(code),
        }),
        source: Some(diagnostic.source),
        message: diagnostic.message,
        related_information: related_information(diagnostic.related, uri),
        tags: None,
        code_description: None,
        data: diagnostic
            .data
            .and_then(|data| serde_json::to_value(data).ok()),
    }
}

fn severity_to_lsp(severity: DiagnosticSeverity) -> LspSeverity {
    match severity {
        DiagnosticSeverity::Error => LspSeverity::ERROR,
        DiagnosticSeverity::Warning => LspSeverity::WARNING,
        DiagnosticSeverity::Info => LspSeverity::INFORMATION,
        DiagnosticSeverity::Hint => LspSeverity::HINT,
    }
}

fn related_information(
    related: Vec<EditorDiagnosticRelated>,
    uri: &Url,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let infos = related
        .into_iter()
        .map(|related| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: range_to_lsp(related.range),
            },
            message: related.message,
        })
        .collect::<Vec<_>>();

    if infos.is_empty() { None } else { Some(infos) }
}

fn range_to_lsp(range: CoreRange) -> Range {
    Range {
        start: Position {
            line: range.start.line as u32,
            character: range.start.character as u32,
        },
        end: Position {
            line: range.end.line as u32,
            character: range.end.character as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_analysis::{
        AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
        DiagnosticRelated, SourceDescriptor, SourceMap,
    };

    #[test]
    fn payload_projection_preserves_message_and_uri() {
        let payload = AnalysisPayload::new(
            SourceDescriptor::diagram(),
            vec![AnalysisDiagnostic::error(
                "merman.parse.no_diagram",
                DiagnosticCategory::Parse,
                "no Mermaid diagram detected",
            )],
        );
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "no Mermaid diagram detected");
    }

    #[test]
    fn payload_projection_preserves_related_information() {
        let map = SourceMap::new("flowchart TD\nA-->B\n");
        let span = map.span(13, 14).unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            "merman.parse.diagram_parse",
            DiagnosticCategory::Parse,
            "boom",
        )
        .with_span(span.clone());
        let payload = AnalysisPayload::new(
            SourceDescriptor::diagram(),
            vec![AnalysisDiagnostic {
                related: vec![DiagnosticRelated {
                    message: "related".to_string(),
                    span: Some(span),
                }],
                ..diagnostic
            }],
        );
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

        assert_eq!(
            diagnostics[0].related_information.as_ref().unwrap().len(),
            1
        );
    }

    #[test]
    fn payload_projection_preserves_fix_metadata_in_diagnostic_data() {
        let map = SourceMap::new("bad");
        let span = map.whole_source_span().unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            "merman.test.fix",
            DiagnosticCategory::Semantic,
            "test diagnostic",
        )
        .with_fix(
            DiagnosticFix::new(
                "Replace invalid text",
                vec![DiagnosticFixEdit::new(span, "fixed")],
            )
            .preferred(),
        );
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let projected = analysis_diagnostic_to_lsp(&diagnostic, &uri);
        let data: merman_editor_core::DiagnosticCodeActionData =
            serde_json::from_value(projected.data.expect("diagnostic data")).unwrap();

        assert_eq!(data.id, "merman.test.fix");
        assert_eq!(data.fixes.len(), 1);
        assert_eq!(data.fixes[0].title, "Replace invalid text");
        assert!(data.fixes[0].is_preferred);
        assert_eq!(data.fixes[0].edits[0].replacement, "fixed");
    }
}
