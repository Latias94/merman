use merman_editor_core::{
    DocumentKind, DocumentWorkspace, SemanticTokenKind, SemanticTokenModifier,
    semantic_tokens_for_snapshot, semantic_tokens_for_snapshot_range,
};

#[test]
fn semantic_tokens_project_entity_outline_and_payload_roles() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        concat!(
            "gantt\n",
            "title Roadmap\n",
            "section Demo\n",
            "Task 1: id1,2014-01-01,1d\n",
            "Task 2: id2,after id1,2d\n",
        )
        .to_string(),
        DocumentKind::Diagram,
    );

    let tokens = semantic_tokens_for_snapshot(&snapshot);

    assert!(tokens.iter().any(|token| {
        token.kind == SemanticTokenKind::Variable && token.modifier == SemanticTokenModifier::Entity
    }));
    assert!(tokens.iter().any(|token| {
        token.kind == SemanticTokenKind::Namespace
            && token.modifier == SemanticTokenModifier::Outline
    }));
    assert!(tokens.iter().any(|token| {
        token.kind == SemanticTokenKind::String && token.modifier == SemanticTokenModifier::Payload
    }));
}

#[test]
fn semantic_tokens_use_document_ranges_and_utf16_lengths() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.md",
        1,
        "before\n```mermaid\nsequenceDiagram\ntitle: Diagram 🤓\n```\nafter\n".to_string(),
        DocumentKind::Markdown,
    );
    let tokens = semantic_tokens_for_snapshot(&snapshot);

    let payload_start = snapshot.text.find("Diagram 🤓").unwrap();
    let payload_end = payload_start + "Diagram 🤓".len();
    let payload_span = snapshot
        .source_map
        .span(payload_start, payload_end)
        .expect("payload span should map");

    assert!(tokens.iter().any(|token| {
        token.line == payload_span.lsp_range.start.line as u32
            && token.start == payload_span.lsp_range.start.character as u32
            && token.length
                == (payload_span.lsp_range.end.character - payload_span.lsp_range.start.character)
                    as u32
            && token.kind == SemanticTokenKind::String
            && token.modifier == SemanticTokenModifier::Payload
    }));
}

#[test]
fn semantic_tokens_range_prunes_non_overlapping_items_inside_single_fence() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        concat!(
            "gantt\n",
            "title Before\n",
            "accDescr {\n",
            "line one\n",
            "line two\n",
            "}\n",
            "title After\n",
        )
        .to_string(),
        DocumentKind::Diagram,
    );

    let full_tokens = semantic_tokens_for_snapshot(&snapshot);
    assert!(
        full_tokens
            .iter()
            .any(|token| token.line < 3 || token.line > 4),
        "fixture must contain tokens outside the requested line range: {full_tokens:?}"
    );

    let range_tokens = semantic_tokens_for_snapshot_range(&snapshot, 3, 4);

    assert!(
        range_tokens
            .iter()
            .all(|token| (3..=4).contains(&token.line)),
        "range tokens should exclude non-overlapping same-fence items: {range_tokens:?}"
    );
    assert!(range_tokens.iter().any(|token| {
        token.line == 3
            && token.kind == SemanticTokenKind::String
            && token.modifier == SemanticTokenModifier::Payload
    }));
    assert!(range_tokens.iter().any(|token| {
        token.line == 4
            && token.kind == SemanticTokenKind::String
            && token.modifier == SemanticTokenModifier::Payload
    }));
}

#[test]
fn semantic_tokens_full_document_line_range_matches_full_generation() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        concat!(
            "gantt\n",
            "title Roadmap\n",
            "section Demo\n",
            "Task 1: id1,2014-01-01,1d\n",
            "Task 2: id2,after id1,2d\n",
        )
        .to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        semantic_tokens_for_snapshot_range(&snapshot, 0, 99),
        semantic_tokens_for_snapshot(&snapshot)
    );
}
