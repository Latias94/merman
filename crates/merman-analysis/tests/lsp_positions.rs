use lsp_types::Url;
use merman_analysis::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticRelated, SourceDescriptor,
    SourceMap, Utf16Position,
    lsp::{analysis_payload_to_diagnostics, position_from_utf16, uri_is_markdown},
};
use std::str::FromStr;

#[test]
fn utf16_position_converts_to_lsp_position() {
    let position = position_from_utf16(Utf16Position {
        line: 2,
        character: 4,
    });

    assert_eq!(position.line, 2);
    assert_eq!(position.character, 4);
}

#[test]
fn analysis_diagnostic_projection_preserves_related_information() {
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
    let uri = Url::from_str("file:///tmp/example.mmd").unwrap();
    let diagnostics = analysis_payload_to_diagnostics(&payload, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].related_information.as_ref().unwrap().len(),
        1
    );
}

#[test]
fn markdown_uri_detection_matches_expected_extensions() {
    let md = Url::from_str("file:///tmp/example.md").unwrap();
    let mdx = Url::from_str("file:///tmp/example.mdx").unwrap();
    let mmd = Url::from_str("file:///tmp/example.mmd").unwrap();

    assert!(uri_is_markdown(&md));
    assert!(uri_is_markdown(&mdx));
    assert!(!uri_is_markdown(&mmd));
}
