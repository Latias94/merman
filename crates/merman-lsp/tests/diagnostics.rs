use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, Analyzer, DiagnosticCategory, DiagnosticSeverity,
    SourceDescriptor,
};
use merman_lsp::diagnostics::analysis_payload_to_diagnostics;
use tower_lsp::lsp_types::{DiagnosticTag, NumberOrString, Url};

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
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.parse.no_diagram".to_string()
        ))
    );
    assert_eq!(
        diagnostics[0]
            .code_description
            .as_ref()
            .expect("code description")
            .href
            .as_str(),
        "https://github.com/Latias94/merman/blob/main/docs/lsp/DIAGNOSTIC_PROTOCOL.md#canonical-rules"
    );
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
fn diagnostics_projection_preserves_analysis_messages_verbatim() {
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
        "flowchart parser recovered after parse error: unexpected statement separator"
    );
}

#[test]
fn flowchart_parse_recovery_does_not_duplicate_lsp_diagnostics() {
    let payload = Analyzer::new().analyze("flowchart TD\nA[unterminated");
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "Unterminated node label (missing `]`)"
    );
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.parse.diagram_parse".to_string()
        ))
    );
}

#[test]
fn class_and_er_parse_spans_project_to_lsp_diagnostics() {
    let cases = [
        ("classDiagram\nA <|--", "class parse"),
        ("erDiagram\nCUSTOMER ||--o{ ORDER :", "er parse"),
    ];
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    for (source, label) in cases {
        let payload = Analyzer::new().analyze(source);
        let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

        assert_eq!(diagnostics.len(), 1, "{label}");
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "merman.parse.diagram_parse".to_string()
            )),
            "{label}"
        );
        assert!(
            diagnostics[0].range.start.line > 0 || diagnostics[0].range.start.character > 0,
            "{label} should not default to the document start"
        );
    }
}

#[test]
fn diagnostics_projection_preserves_rule_metadata_in_data() {
    let payload = Analyzer::new().analyze("flowchart TD\nA-->B\n");
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert!(diagnostics.is_empty());

    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![
            AnalysisDiagnostic::new(
                "merman.test.info",
                DiagnosticSeverity::Hint,
                DiagnosticCategory::Config,
                "test metadata",
            )
            .with_diagram_type("flowchart-v2")
            .with_help("See rule docs."),
        ],
    );
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);
    let data = diagnostics[0].data.as_ref().expect("diagnostic data");

    assert_eq!(data["id"], "merman.test.info");
    assert_eq!(data["category"], "config");
    assert_eq!(data["diagramType"], "flowchart-v2");
    assert_eq!(data["help"], "See rule docs.");
}

#[test]
fn deprecated_diagnostics_project_lsp_tag() {
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![AnalysisDiagnostic::new(
            "merman.compatibility.config.deprecated_flowchart_html_labels",
            DiagnosticSeverity::Warning,
            DiagnosticCategory::Config,
            "deprecated option",
        )],
    );
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert_eq!(diagnostics[0].tags, Some(vec![DiagnosticTag::DEPRECATED]));
}
