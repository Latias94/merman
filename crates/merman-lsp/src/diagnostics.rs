use merman_analysis::{AnalysisDiagnostic, AnalysisPayload, DiagnosticSeverity};
use tower_lsp::lsp_types::{
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

fn position_from_utf16(value: merman_analysis::Utf16Position) -> Position {
    Position {
        line: value.line as u32,
        character: value.character as u32,
    }
}
