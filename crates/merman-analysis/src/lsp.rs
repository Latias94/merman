use crate::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticFix, DiagnosticSeverity, Utf16Position,
};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity as LspSeverity, Location,
    NumberOrString, Position, Range, Url,
};
use merman_core::{Engine, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

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
        data: diagnostic_data(diagnostic),
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

pub fn diagram_type_for_text(text: &str) -> Option<String> {
    analysis_engine()
        .parse_metadata_sync(text, ParseOptions::strict())
        .ok()
        .flatten()
        .map(|meta| meta.diagram_type)
}

fn analysis_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

pub fn diagnostic_code_action_data(
    diagnostic: &AnalysisDiagnostic,
) -> Option<DiagnosticCodeActionData> {
    (!diagnostic.fixes.is_empty()).then(|| DiagnosticCodeActionData {
        id: diagnostic.id.clone(),
        fixes: diagnostic.fixes.clone(),
    })
}

fn diagnostic_data(diagnostic: &AnalysisDiagnostic) -> Option<Value> {
    diagnostic_code_action_data(diagnostic).and_then(|data| serde_json::to_value(data).ok())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCodeActionData {
    pub id: String,
    pub fixes: Vec<DiagnosticFix>,
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
    use crate::{
        AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
        SourceDescriptor, SourceMap,
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
        let data: DiagnosticCodeActionData =
            serde_json::from_value(projected.data.expect("diagnostic data")).unwrap();

        assert_eq!(data.id, "merman.test.fix");
        assert_eq!(data.fixes.len(), 1);
        assert_eq!(data.fixes[0].title, "Replace invalid text");
        assert!(data.fixes[0].is_preferred);
        assert_eq!(data.fixes[0].edits[0].replacement, "fixed");
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

    #[test]
    fn diagram_type_detection_matches_core_metadata() {
        assert_eq!(
            diagram_type_for_text("flowchart TD\nA-->B\n").as_deref(),
            Some("flowchart-v2")
        );
        assert_eq!(
            diagram_type_for_text("sequenceDiagram\nAlice->>Bob: Hi\n").as_deref(),
            Some("sequence")
        );
        assert_eq!(
            diagram_type_for_text("mindmap\nroot\n child\n").as_deref(),
            Some("mindmap")
        );
        assert_eq!(diagram_type_for_text("").as_deref(), None);
    }
}
