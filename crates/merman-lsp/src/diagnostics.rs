use std::str::FromStr;

use crate::client_profile::{ClientProtocolProfile, DiagnosticProtocolProfile};
#[cfg(test)]
use crate::protocol::DiagnosticIdentityData;
use crate::protocol::{DiagnosticVersionData, range_to_lsp};
#[cfg(test)]
use merman_analysis::AnalysisDiagnostic;
use merman_analysis::{AnalysisDiagnosticTag, AnalysisPayload, DiagnosticSeverity};
#[cfg(test)]
use merman_editor_core::analysis_diagnostic_to_editor;
use merman_editor_core::{
    EditorDiagnostic, EditorDiagnosticRelated,
    analysis_payload_to_diagnostics as analysis_payload_to_editor_diagnostics,
};
use tower_lsp_server::ls_types::{
    CodeDescription, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity as LspSeverity,
    DiagnosticTag, Location, NumberOrString, Uri,
};

#[cfg(test)]
pub(crate) fn analysis_payload_to_diagnostics(
    payload: &AnalysisPayload,
    uri: &Uri,
) -> Vec<Diagnostic> {
    analysis_payload_to_diagnostics_with_profile(payload, uri, &ClientProtocolProfile::permissive())
}

#[cfg(test)]
pub(crate) fn analysis_payload_to_diagnostics_with_profile(
    payload: &AnalysisPayload,
    uri: &Uri,
    profile: &ClientProtocolProfile,
) -> Vec<Diagnostic> {
    analysis_payload_to_editor_diagnostics(payload)
        .into_iter()
        .map(|diagnostic| editor_diagnostic_to_lsp(diagnostic, uri, profile.diagnostics))
        .collect()
}

pub(crate) fn analysis_payload_to_versioned_diagnostics_with_profile(
    payload: &AnalysisPayload,
    uri: &Uri,
    document_version: i32,
    profile: &ClientProtocolProfile,
) -> Vec<Diagnostic> {
    let data = profile
        .diagnostics
        .data
        .then(|| serde_json::to_value(DiagnosticVersionData { document_version }).ok());
    analysis_payload_to_editor_diagnostics(payload)
        .into_iter()
        .map(|diagnostic| {
            editor_diagnostic_to_lsp_with_data(
                &diagnostic,
                uri,
                data.clone().flatten(),
                profile.diagnostics,
            )
        })
        .collect()
}

#[cfg(test)]
fn analysis_diagnostic_to_lsp(diagnostic: &AnalysisDiagnostic, uri: &Uri) -> Diagnostic {
    editor_diagnostic_to_lsp(
        analysis_diagnostic_to_editor(diagnostic),
        uri,
        ClientProtocolProfile::permissive().diagnostics,
    )
}

#[cfg(test)]
pub(crate) fn analysis_diagnostic_to_versioned_lsp(
    diagnostic: &AnalysisDiagnostic,
    uri: &Uri,
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
    uri: &Uri,
    profile: DiagnosticProtocolProfile,
) -> Diagnostic {
    let data = diagnostic_identity_data(&diagnostic, None, profile);
    editor_diagnostic_to_lsp_with_data(&diagnostic, uri, data, profile)
}

#[cfg(test)]
fn editor_diagnostic_to_versioned_lsp(
    diagnostic: EditorDiagnostic,
    uri: &Uri,
    document_version: i32,
    profile: DiagnosticProtocolProfile,
) -> Diagnostic {
    let data = diagnostic_identity_data(&diagnostic, Some(document_version), profile);
    editor_diagnostic_to_lsp_with_data(&diagnostic, uri, data, profile)
}

#[cfg(test)]
fn diagnostic_identity_data(
    diagnostic: &EditorDiagnostic,
    document_version: Option<i32>,
    profile: DiagnosticProtocolProfile,
) -> Option<serde_json::Value> {
    if !profile.data {
        return None;
    }
    let data = diagnostic.data.as_ref()?;
    serde_json::to_value(DiagnosticIdentityData {
        id: data.id.clone(),
        document_version,
    })
    .ok()
}

pub(crate) fn editor_diagnostic_to_lsp_with_data(
    diagnostic: &EditorDiagnostic,
    uri: &Uri,
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
        diagnostic_tags(&diagnostic.tags)
    } else {
        None
    };
    Diagnostic {
        range: range_to_lsp(diagnostic.range),
        severity: Some(severity_to_lsp(diagnostic.severity)),
        code: Some(code),
        source: Some(diagnostic.source.clone()),
        message: diagnostic.message.clone(),
        related_information: if profile.related_information {
            related_information(&diagnostic.related, uri)
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
    Uri::from_str(
        "https://github.com/Latias94/merman/blob/main/docs/lsp/DIAGNOSTIC_PROTOCOL.md#canonical-rules",
    )
    .ok()
    .map(|href| CodeDescription { href })
}

fn diagnostic_tags(tags: &[AnalysisDiagnosticTag]) -> Option<Vec<DiagnosticTag>> {
    let projected = tags
        .iter()
        .map(|tag| match tag {
            AnalysisDiagnosticTag::Deprecated => DiagnosticTag::DEPRECATED,
        })
        .collect::<Vec<_>>();
    (!projected.is_empty()).then_some(projected)
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
    related: &[EditorDiagnosticRelated],
    uri: &Uri,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let infos = related
        .iter()
        .map(|related| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: range_to_lsp(related.range),
            },
            message: related.message.clone(),
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
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
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
        .with_span(span);
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
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

        assert_eq!(
            diagnostics[0].related_information.as_ref().unwrap().len(),
            1
        );
    }

    #[test]
    fn payload_projection_exposes_diagnostic_identity_without_fix_metadata() {
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
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let projected = analysis_diagnostic_to_lsp(&diagnostic, &uri);
        let data = projected.data.expect("diagnostic identity");

        assert_eq!(data, serde_json::json!({ "id": "merman.test.fix" }));
    }
}
