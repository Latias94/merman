//! Completion contracts follow the linked parser catalog, including feature unions.
use merman_analysis::{Analyzer, DiagramParseDisposition, FenceTextIndexSource};
use merman_editor_core::{
    CompletionDataKind, DocumentKind, Position, analyze_document_snapshot_with_shared_text,
    completion_for_snapshot,
};
use std::collections::BTreeSet;
use std::sync::Arc;

#[test]
fn headers_offer_exactly_the_compiled_parser_aliases() {
    let snapshot = analyze_document_snapshot_with_shared_text(
        &Analyzer::new(),
        "file:///selection.mmd",
        1,
        Arc::from(""),
        DocumentKind::Diagram,
    )
    .unwrap();
    let completion = completion_for_snapshot(&snapshot, Position::new(0, 0));
    let headers = completion
        .items
        .iter()
        .filter(|item| {
            item.data
                .as_ref()
                .is_some_and(|data| data.kind == CompletionDataKind::DiagramHeader)
        })
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();
    let families = merman_core::diagram_family_capabilities();
    let expected = merman_core::diagram_header_facts()
        .iter()
        .filter(|header| {
            families.iter().any(|family| {
                family.diagram_type == header.diagram_type && family.has_semantic_parser
            })
        })
        .map(|header| header.label)
        .collect::<BTreeSet<_>>();
    assert_eq!(headers, expected);
    let templates = completion
        .items
        .iter()
        .filter(|item| {
            item.data
                .as_ref()
                .is_some_and(|data| data.kind == CompletionDataKind::Template)
        })
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();
    let available = |kind: &str| {
        families
            .iter()
            .any(|family| family.logical_family_kind == kind && family.has_semantic_parser)
    };
    assert_eq!(
        templates.contains("flowchart template"),
        available("flowchart")
    );
    assert_eq!(
        templates.contains("icon node template"),
        available("flowchart")
    );
    assert_eq!(
        templates.contains("frontmatter config template"),
        available("flowchart")
    );
    assert_eq!(
        templates.contains("themeCSS frontmatter template"),
        available("flowchart")
    );
    assert_eq!(
        templates.contains("sequence template"),
        available("sequence")
    );
    if !families
        .iter()
        .any(|family| family.logical_family_kind != "error" && family.has_semantic_parser)
    {
        assert!(headers.is_empty());
        assert!(templates.is_empty());
    }
}

#[test]
fn disabled_source_keeps_its_identity_without_parser_facts() {
    if merman_core::diagram_family_capabilities()
        .iter()
        .any(|family| family.diagram_type == "sequence" && family.has_semantic_parser)
    {
        return;
    }
    let analyzer = Analyzer::new();
    let snapshot = analyze_document_snapshot_with_shared_text(
        &analyzer,
        "file:///disabled.mmd",
        1,
        Arc::from("sequenceDiagram\nAlice->>Bob: Hello\n"),
        DocumentKind::Diagram,
    )
    .unwrap();
    let diagram = &snapshot.analysis_generation().diagrams()[0];
    assert_eq!(diagram.syntax().diagram_type.as_deref(), Some("sequence"));
    assert_eq!(
        diagram.parse_disposition(),
        DiagramParseDisposition::Unavailable
    );
    assert_eq!(diagram.syntax().source(), FenceTextIndexSource::Unavailable);
    let payload = snapshot
        .analysis_generation()
        .project(analyzer.options().diagnostic_policy());
    assert!(payload.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unsupported diagram type: sequence")
    }));
    assert!(
        completion_for_snapshot(&snapshot, Position::new(1, 5))
            .items
            .is_empty()
    );
}

#[test]
fn shape_edits_follow_the_compiled_parser_when_local_features_differ() {
    let flowchart_available = merman_core::diagram_family_capabilities()
        .iter()
        .any(|family| family.logical_family_kind == "flowchart" && family.has_semantic_parser);
    for (line, expected_start, expected_replacement) in [
        ("A((", 1, "@{ shape: circle }"),
        ("A@{ shape: rou", "A@{ shape: ".len(), "circle }"),
    ] {
        let snapshot = analyze_document_snapshot_with_shared_text(
            &Analyzer::new(),
            "file:///shape-selection.mmd",
            1,
            Arc::from(format!("flowchart TD\n{line}")),
            DocumentKind::Diagram,
        )
        .unwrap();
        let completion = completion_for_snapshot(&snapshot, Position::new(1, line.len()));
        let shape = completion
            .items
            .iter()
            .find(|item| item.label == "@{ shape: circle }");
        assert_eq!(shape.is_some(), flowchart_available, "{line}");
        if let Some(shape) = shape {
            assert_eq!(
                completion.fact_source,
                Some(FenceTextIndexSource::ParserRecovered),
                "{line}"
            );
            let edit = shape
                .text_edit
                .as_ref()
                .expect("shape completion text edit");
            assert_eq!(edit.range.start, Position::new(1, expected_start), "{line}");
            assert_eq!(edit.range.end, Position::new(1, line.len()), "{line}");
            assert_eq!(edit.new_text, expected_replacement, "{line}");
        }
    }
}
