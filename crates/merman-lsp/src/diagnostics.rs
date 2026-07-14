use crate::client_profile::{ClientProtocolProfile, DiagnosticProtocolProfile};
use crate::protocol::{VersionedDiagnosticCodeActionData, range_to_lsp};
#[cfg(test)]
use merman_analysis::AnalysisDiagnostic;
use merman_analysis::{AnalysisPayload, DiagnosticSeverity};
#[cfg(test)]
use merman_editor_core::analysis_diagnostic_to_editor;
use merman_editor_core::{
    EditorDiagnostic, EditorDiagnosticRelated,
    analysis_payload_to_diagnostics as analysis_payload_to_editor_diagnostics,
};
use tower_lsp::lsp_types::{
    CodeDescription, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity as LspSeverity,
    DiagnosticTag, Location, NumberOrString, Url,
};

#[cfg(test)]
pub(crate) fn analysis_payload_to_diagnostics(
    payload: &AnalysisPayload,
    uri: &Url,
) -> Vec<Diagnostic> {
    analysis_payload_to_diagnostics_with_profile(payload, uri, &ClientProtocolProfile::permissive())
}

#[cfg(test)]
pub(crate) fn analysis_payload_to_diagnostics_with_profile(
    payload: &AnalysisPayload,
    uri: &Url,
    profile: &ClientProtocolProfile,
) -> Vec<Diagnostic> {
    analysis_payload_to_editor_diagnostics(payload)
        .into_iter()
        .map(|diagnostic| editor_diagnostic_to_lsp(diagnostic, uri, profile.diagnostics))
        .collect()
}

#[cfg(test)]
pub(crate) fn analysis_payload_to_versioned_diagnostics(
    payload: &AnalysisPayload,
    uri: &Url,
    document_version: i32,
) -> Vec<Diagnostic> {
    analysis_payload_to_versioned_diagnostics_with_profile(
        payload,
        uri,
        document_version,
        &ClientProtocolProfile::permissive(),
    )
}

pub(crate) fn analysis_payload_to_versioned_diagnostics_with_profile(
    payload: &AnalysisPayload,
    uri: &Url,
    document_version: i32,
    profile: &ClientProtocolProfile,
) -> Vec<Diagnostic> {
    analysis_payload_to_editor_diagnostics(payload)
        .into_iter()
        .map(|diagnostic| {
            editor_diagnostic_to_versioned_lsp(
                diagnostic,
                uri,
                document_version,
                profile.diagnostics,
            )
        })
        .collect()
}

#[cfg(test)]
fn analysis_diagnostic_to_lsp(diagnostic: &AnalysisDiagnostic, uri: &Url) -> Diagnostic {
    editor_diagnostic_to_lsp(
        analysis_diagnostic_to_editor(diagnostic),
        uri,
        ClientProtocolProfile::permissive().diagnostics,
    )
}

#[cfg(test)]
pub(crate) fn analysis_diagnostic_to_versioned_lsp(
    diagnostic: &AnalysisDiagnostic,
    uri: &Url,
    document_version: i32,
) -> Diagnostic {
    editor_diagnostic_to_versioned_lsp(
        analysis_diagnostic_to_editor(diagnostic),
        uri,
        document_version,
        ClientProtocolProfile::permissive().diagnostics,
    )
}

#[cfg(test)]
fn editor_diagnostic_to_lsp(
    diagnostic: EditorDiagnostic,
    uri: &Url,
    profile: DiagnosticProtocolProfile,
) -> Diagnostic {
    let data = if profile.data {
        diagnostic
            .data
            .as_ref()
            .and_then(|data| serde_json::to_value(data).ok())
    } else {
        None
    };
    editor_diagnostic_to_lsp_with_data(diagnostic, uri, data, profile)
}

fn editor_diagnostic_to_versioned_lsp(
    diagnostic: EditorDiagnostic,
    uri: &Url,
    document_version: i32,
    profile: DiagnosticProtocolProfile,
) -> Diagnostic {
    let data = if profile.data {
        diagnostic.data.as_ref().and_then(|data| {
            serde_json::to_value(VersionedDiagnosticCodeActionData {
                inner: data.clone(),
                document_version,
            })
            .ok()
        })
    } else {
        None
    };
    editor_diagnostic_to_lsp_with_data(diagnostic, uri, data, profile)
}

fn editor_diagnostic_to_lsp_with_data(
    diagnostic: EditorDiagnostic,
    uri: &Url,
    data: Option<serde_json::Value>,
    profile: DiagnosticProtocolProfile,
) -> Diagnostic {
    let code = NumberOrString::String(diagnostic.code.clone());
    let code_description = if profile.code_description {
        code_description(&diagnostic.code)
    } else {
        None
    };
    let tags = if profile.deprecated_tag {
        diagnostic_tags(diagnostic.data.as_ref())
    } else {
        None
    };
    Diagnostic {
        range: range_to_lsp(diagnostic.range),
        severity: Some(severity_to_lsp(diagnostic.severity)),
        code: Some(code),
        source: Some(diagnostic.source),
        message: diagnostic.message,
        related_information: if profile.related_information {
            related_information(diagnostic.related, uri)
        } else {
            None
        },
        tags,
        code_description,
        data,
    }
}

fn code_description(code: &str) -> Option<CodeDescription> {
    if !code.starts_with("merman.") {
        return None;
    }
    Url::parse(
        "https://github.com/Latias94/merman/blob/main/docs/lsp/DIAGNOSTIC_PROTOCOL.md#canonical-rules",
    )
    .ok()
    .map(|href| CodeDescription { href })
}

fn diagnostic_tags(
    data: Option<&merman_editor_core::DiagnosticCodeActionData>,
) -> Option<Vec<DiagnosticTag>> {
    let data = data?;
    let deprecated = data.id.contains(".deprecated_")
        || data
            .help
            .as_deref()
            .is_some_and(|help| help.to_ascii_lowercase().contains("deprecated"));
    deprecated.then(|| vec![DiagnosticTag::DEPRECATED])
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
