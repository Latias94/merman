use merman_editor_core::{
    DocumentKind, DocumentWorkspace, PlannedTokenKind, plan_semantic_tokens_for_snapshot,
};

#[test]
fn planner_emits_sorted_non_overlapping_utf16_tokens_and_packed_words() {
    let source = concat!(
        "---\n",
        "title: UTF-16\n",
        "---\n",
        "flowchart TD\n",
        "alpha[\"Emoji 🤓\"] --> beta\n",
        "%% trailing comment\n",
    );
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/unicode.mmd",
        1,
        source.to_string(),
        DocumentKind::Diagram,
    );

    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("valid token plan");
    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert!(
        plan.tokens()
            .iter()
            .any(|token| token.kind == PlannedTokenKind::Keyword)
    );
    assert!(
        plan.tokens()
            .iter()
            .any(|token| token.kind == PlannedTokenKind::Comment)
    );

    for pair in plan.tokens().windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlap: {pair:?}"
        );
    }
}

#[test]
fn markdown_fences_are_preprocessor_owned_delimiter_tokens() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.md",
        1,
        "before\n```mermaid\nflowchart TD\nA --> B\n```\nafter\n".to_string(),
        DocumentKind::Markdown,
    );
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("valid token plan");

    assert_eq!(
        plan.tokens()
            .iter()
            .filter(|token| token.kind == PlannedTokenKind::Delimiter)
            .count(),
        2,
        "only the opening and closing Markdown fence markers are global delimiter tokens"
    );
}

#[test]
fn planner_uses_exact_structured_marker_spans_for_all_fence_forms() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/fences.md",
        1,
        concat!(
            "  ````mermaid\n",
            "flowchart TD\n",
            "A --> B\n",
            "   ``````\n",
            ":::mermaid\n",
            "pie title Work\n",
            "  \"Done\" : 1\n",
            ":::::\n",
        )
        .to_string(),
        DocumentKind::Markdown,
    );
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("valid fence marker plan");

    assert_eq!(
        plan.tokens()
            .iter()
            .filter(|token| token.kind == PlannedTokenKind::Delimiter)
            .map(|token| token.length)
            .collect::<Vec<_>>(),
        vec![4, 6, 3, 5]
    );
}

#[test]
fn zenuml_recovery_does_not_emit_overlapping_tokens() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/recovered.mmd",
        1,
        concat!(
            "zenuml\n",
            "@Actor Client #FFEBE6\n",
            "Client->Service: first\n",
            "if (broken {\n",
            "Service->Client: after 中文\n",
        )
        .to_string(),
        DocumentKind::Diagram,
    );

    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("recovered facts remain valid");
    assert!(plan.tokens().windows(2).all(|pair| {
        (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start)
    }));
    assert!(plan.tokens().iter().any(|token| token.line == 4));
}
