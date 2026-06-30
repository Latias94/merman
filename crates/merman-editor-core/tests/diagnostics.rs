use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticRelated, SourceDescriptor, SourceMap,
};
use merman_editor_core::{
    EditorDiagnosticCode, Range, analysis_diagnostic_to_editor, analysis_payload_to_diagnostics,
};

#[test]
fn diagnostics_projection_preserves_message_and_fix_metadata() {
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
    let projected = analysis_diagnostic_to_editor(&diagnostic);

    assert_eq!(projected.message, "test diagnostic");
    let data = projected.data.expect("diagnostic data");
    assert_eq!(data.id, "merman.test.fix");
    assert_eq!(data.category, DiagnosticCategory::Semantic);
    assert_eq!(data.fixes.len(), 1);
    assert_eq!(data.fixes[0].title, "Replace invalid text");
}

#[test]
fn payload_projection_is_protocol_neutral() {
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![AnalysisDiagnostic::error(
            "merman.parse.no_diagram",
            DiagnosticCategory::Parse,
            "no Mermaid diagram detected",
        )],
    );
    let diagnostics = analysis_payload_to_diagnostics(&payload);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "no Mermaid diagram detected");
    assert_eq!(
        diagnostics[0].code,
        EditorDiagnosticCode::String("merman.parse.no_diagram".to_string())
    );
}

#[test]
fn recovered_parser_messages_are_humanized_for_editor_surfaces() {
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![AnalysisDiagnostic::error(
            "merman.parse.recovered_editor_facts",
            DiagnosticCategory::Parse,
            "flowchart parser recovered after parse error: unexpected statement separator; expected edge label, node identifier",
        )],
    );

    let diagnostics = analysis_payload_to_diagnostics(&payload);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "Mermaid syntax issue: unexpected statement separator; expected edge label, node identifier"
    );
}

#[test]
fn duplicate_projected_diagnostics_are_deduplicated() {
    let map = SourceMap::new("flowchart TD\nA -->\n");
    let span = map.whole_source_span().unwrap();
    let diagnostic = AnalysisDiagnostic::error(
        "merman.parse.recovered_editor_facts",
        DiagnosticCategory::Parse,
        "flowchart parser recovered after parse error: unexpected statement separator",
    )
    .with_span(span);
    let payload = AnalysisPayload::new(
        SourceDescriptor::diagram(),
        vec![diagnostic.clone(), diagnostic],
    );

    let diagnostics = analysis_payload_to_diagnostics(&payload);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn deduplicated_diagnostics_preserve_related_and_fix_metadata() {
    let map = SourceMap::new("flowchart TD\nA --> B\n");
    let span = map.span(13, 14).unwrap();
    let base = AnalysisDiagnostic::error(
        "merman.test.fix",
        DiagnosticCategory::Semantic,
        "test diagnostic",
    )
    .with_span(span.clone());
    let with_related = AnalysisDiagnostic {
        related: vec![DiagnosticRelated {
            message: "related context".to_string(),
            span: Some(span.clone()),
        }],
        ..base.clone()
    };
    let with_fix = base.with_fix(DiagnosticFix::new(
        "Replace invalid text",
        vec![DiagnosticFixEdit::new(span, "fixed")],
    ));
    let payload = AnalysisPayload::new(SourceDescriptor::diagram(), vec![with_related, with_fix]);

    let diagnostics = analysis_payload_to_diagnostics(&payload);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].related.len(), 1);
    assert_eq!(
        diagnostics[0].data.as_ref().expect("metadata").fixes.len(),
        1
    );
    assert_ne!(diagnostics[0].range, Range::default());
}
