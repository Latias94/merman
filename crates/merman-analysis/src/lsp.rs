use crate::{AnalysisDiagnostic, AnalysisPayload, DiagnosticSeverity, Utf16Position};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity as LspSeverity, Location,
    NumberOrString, Position, Range, Url,
};

pub fn analysis_payload_to_diagnostics(payload: &AnalysisPayload, uri: &Url) -> Vec<Diagnostic> {
    payload
        .diagnostics
        .iter()
        .map(|diagnostic| analysis_diagnostic_to_lsp(diagnostic, uri))
        .collect()
}

pub fn analysis_diagnostic_to_lsp(diagnostic: &AnalysisDiagnostic, uri: &Url) -> Diagnostic {
    Diagnostic {
        range: diagnostic
            .span
            .as_ref()
            .map(|span| Range {
                start: position_from_utf16(span.lsp_range.start),
                end: position_from_utf16(span.lsp_range.end),
            })
            .unwrap_or_default(),
        severity: Some(match diagnostic.severity {
            DiagnosticSeverity::Error => LspSeverity::ERROR,
            DiagnosticSeverity::Warning => LspSeverity::WARNING,
            DiagnosticSeverity::Info => LspSeverity::INFORMATION,
            DiagnosticSeverity::Hint => LspSeverity::HINT,
        }),
        code: diagnostic
            .code
            .map(NumberOrString::Number)
            .or_else(|| Some(NumberOrString::String(diagnostic.id.clone()))),
        source: Some("merman".to_string()),
        message: diagnostic.message.clone(),
        related_information: related_information(diagnostic, uri),
        tags: None,
        code_description: None,
        data: None,
    }
}

pub fn position_from_utf16(value: Utf16Position) -> Position {
    Position {
        line: value.line as u32,
        character: value.character as u32,
    }
}

pub fn uri_is_markdown(uri: &Url) -> bool {
    crate::markdown::is_markdown_path(std::path::Path::new(uri.path()))
}

fn related_information(
    diagnostic: &AnalysisDiagnostic,
    uri: &Url,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let infos = diagnostic
        .related
        .iter()
        .filter_map(|related| {
            let span = related.span.as_ref()?;
            Some(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: Range {
                        start: position_from_utf16(span.lsp_range.start),
                        end: position_from_utf16(span.lsp_range.end),
                    },
                },
                message: related.message.clone(),
            })
        })
        .collect::<Vec<_>>();

    if infos.is_empty() { None } else { Some(infos) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, SourceDescriptor};

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
    fn markdown_uri_detection_matches_expected_extensions() {
        let md = Url::parse("file:///tmp/example.md").unwrap();
        let markdown = Url::parse("file:///tmp/example.markdown").unwrap();
        let mdx = Url::parse("file:///tmp/example.mdx").unwrap();
        let mmd = Url::parse("file:///tmp/example.mmd").unwrap();

        assert!(uri_is_markdown(&md));
        assert!(uri_is_markdown(&markdown));
        assert!(uri_is_markdown(&mdx));
        assert!(!uri_is_markdown(&mmd));
    }
}
