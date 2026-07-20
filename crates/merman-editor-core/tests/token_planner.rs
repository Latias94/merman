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
            .filter(|token| {
                token.kind == PlannedTokenKind::Delimiter && matches!(token.line, 0 | 3 | 4 | 7)
            })
            .map(|token| (token.line, token.start, token.length))
            .collect::<Vec<_>>(),
        vec![(0, 2, 4), (3, 3, 6), (4, 0, 3), (7, 0, 5)]
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

#[test]
fn lexer_backed_family_plans_merge_grammar_lexemes_with_semantic_overlays() {
    let cases = [
        (
            "class",
            concat!(
                "classDiagram\n",
                "%% class comment\n",
                "class Order\n",
                "Order --|> Base : inherits\n",
                "classDef hot fill:#f00,stroke:#000\n",
            ),
            vec![
                PlannedTokenKind::Keyword,
                PlannedTokenKind::Comment,
                PlannedTokenKind::Operator,
                PlannedTokenKind::Class,
                PlannedTokenKind::Property,
                PlannedTokenKind::String,
            ],
        ),
        (
            "sequence",
            concat!(
                "sequenceDiagram\n",
                "%% sequence comment\n",
                "autonumber 10 5\n",
                "participant Client\n",
                "rect rgb(245,245,245)\n",
                "Client->>Service: create order\n",
                "end\n",
            ),
            vec![
                PlannedTokenKind::Keyword,
                PlannedTokenKind::Comment,
                PlannedTokenKind::Number,
                PlannedTokenKind::Operator,
                PlannedTokenKind::String,
                PlannedTokenKind::Event,
            ],
        ),
        (
            "state",
            concat!(
                "stateDiagram-v2\n",
                "state \"Waiting\" as Idle\n",
                "Idle --> Running: starts\n",
                "classDef active fill:#0f0,stroke:#333\n",
                "class Running active\n",
            ),
            vec![
                PlannedTokenKind::Keyword,
                PlannedTokenKind::Operator,
                PlannedTokenKind::Delimiter,
                PlannedTokenKind::Class,
                PlannedTokenKind::Property,
                PlannedTokenKind::String,
            ],
        ),
        (
            "er",
            concat!(
                "erDiagram\n",
                "CUSTOMER ||--o{ ORDER : places\n",
                "CUSTOMER {\n",
                "  string name PK\n",
                "}\n",
                "classDef hot fill:#f00,stroke:#000\n",
                "class CUSTOMER,ORDER hot\n",
            ),
            vec![
                PlannedTokenKind::Keyword,
                PlannedTokenKind::Operator,
                PlannedTokenKind::Delimiter,
                PlannedTokenKind::Struct,
                PlannedTokenKind::Property,
                PlannedTokenKind::String,
            ],
        ),
    ];

    for (name, source, expected_kinds) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            format!("file:///tmp/{name}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("valid family token plan");
        assert_plan_is_non_overlapping(plan.tokens());
        for expected in expected_kinds {
            assert!(
                plan.tokens().iter().any(|token| token.kind == expected),
                "{name} plan is missing {expected:?}: {:?}",
                plan.tokens()
            );
        }
    }
}

#[test]
fn lexer_backed_family_recovery_plans_cover_tokens_before_and_after_the_error() {
    let cases = [
        ("class", "classDiagram\nclass Before\n?\nclass After\n"),
        (
            "sequence",
            concat!(
                "sequenceDiagram\n",
                "participant Before\n",
                "?\n",
                "participant After\n",
                "Before->>After: ok\n",
            ),
        ),
        (
            "state",
            concat!(
                "stateDiagram-v2\n",
                "Before --> Middle\n",
                "- malformed\n",
                "After --> End\n",
            ),
        ),
        (
            "er",
            concat!(
                "erDiagram\n",
                "Before ||--o{ Middle : valid\n",
                "@ malformed\n",
                "After ||--|| End : retained\n",
            ),
        ),
    ];

    for (name, source) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            format!("file:///tmp/{name}-recovery.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("recoverable token plan");
        assert_plan_is_non_overlapping(plan.tokens());
        for text in ["Before", "After"] {
            let offset = source.find(text).expect("identifier offset");
            let position = snapshot
                .source_map
                .utf16_position(offset)
                .expect("identifier position");
            assert!(
                plan.tokens().iter().any(|token| {
                    token.line == position.line as u32
                        && token.start <= position.character as u32
                        && token.start + token.length > position.character as u32
                }),
                "{name} recovery plan does not cover {text}: {:?}",
                plan.tokens()
            );
        }
    }
}

fn assert_plan_is_non_overlapping(tokens: &[merman_editor_core::PlannedToken]) {
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlap: {pair:?}"
        );
    }
}
