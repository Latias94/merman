use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    SourceDescriptor, SourceMap,
};
use merman_editor_core::{analysis_diagnostic_to_editor, analysis_payload_to_diagnostics};

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
    assert_eq!(diagnostics[0].code, "merman.parse.no_diagram");
}

#[test]
fn projection_preserves_recovered_parser_messages_verbatim() {
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
        "flowchart parser recovered after parse error: unexpected statement separator; expected edge label, node identifier"
    );
}

#[test]
fn payload_projection_keeps_analysis_cardinality() {
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

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn projection_preserves_fix_metadata_per_diagnostic() {
    let map = SourceMap::new("flowchart TD\nA --> B\n");
    let span = map.span(13, 14).unwrap();
    let diagnostic = AnalysisDiagnostic::error(
        "merman.test.fix",
        DiagnosticCategory::Semantic,
        "test diagnostic",
    )
    .with_span(span.clone())
    .with_fix(DiagnosticFix::new(
        "Replace invalid text",
        vec![DiagnosticFixEdit::new(span, "fixed")],
    ));
    let payload = AnalysisPayload::new(SourceDescriptor::diagram(), vec![diagnostic]);

    let diagnostics = analysis_payload_to_diagnostics(&payload);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].data.as_ref().expect("metadata").fixes.len(),
        1
    );
}
