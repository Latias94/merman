use merman_analysis::{AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, SourceDescriptor};
use merman_lsp::diagnostics::analysis_payload_to_diagnostics;
use tower_lsp::lsp_types::Url;

#[test]
fn diagnostics_projection_preserves_uri_and_message() {
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
fn diagnostics_projection_accepts_markdown_file_urls() {
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![AnalysisDiagnostic::error(
            "merman.parse.no_diagram",
            DiagnosticCategory::Parse,
            "no Mermaid diagram detected",
        )],
    );
    let uri = Url::parse("file:///tmp/example.md").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn diagnostics_projection_humanizes_recovered_parser_messages() {
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![AnalysisDiagnostic::error(
            "merman.parse.recovered_editor_facts",
            DiagnosticCategory::Parse,
            "flowchart parser recovered after parse error: unexpected statement separator",
        )],
    );
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "Mermaid syntax issue: unexpected statement separator"
    );
}
