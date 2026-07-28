use merman_editor_core::{
    DocumentKind, DocumentWorkspace, PlannedTokenKind, plan_semantic_tokens_for_snapshot,
};

#[test]
fn structured_langium_plans_merge_parser_lexemes_without_overlap() {
    let cases = [
        (
            "cynefin",
            concat!(
                "cynefin-beta\n",
                "%% note\n",
                "complex \"Before\"\n",
                "complex --> clear : \"Move\"\n",
            ),
            PlannedTokenKind::Operator,
        ),
        (
            "treeView",
            concat!(
                "treeView-beta\n",
                "%% note\n",
                "root/\n",
                "    app.ts icon(file) ## Entry\n",
            ),
            PlannedTokenKind::Delimiter,
        ),
        (
            "treemap",
            concat!(
                "treemap-beta\n",
                "%% note\n",
                "classDef hot fill:#f00\n",
                "\"Root\":::hot\n",
                "  \"Leaf\": 10\n",
            ),
            PlannedTokenKind::Delimiter,
        ),
        (
            "venn",
            concat!(
                "venn-beta\n",
                "%% note\n",
                "set A[\"Alpha\"]:20\n",
                "set B\n",
                "union A,B\n",
            ),
            PlannedTokenKind::Delimiter,
        ),
    ];

    for (family, source, distinctive_kind) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                format!("file:///tmp/{family}-structured.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan = plan_semantic_tokens_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("{family} plan failed: {error}"));

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5, "{family}");
        for expected in [
            PlannedTokenKind::Keyword,
            PlannedTokenKind::Comment,
            distinctive_kind,
        ] {
            assert!(
                plan.tokens().iter().any(|token| token.kind == expected),
                "{family} plan is missing {expected:?}: {:?}",
                plan.tokens()
            );
        }
        assert_non_overlapping(plan.tokens());
    }
}

#[test]
fn structured_langium_recovery_plans_preserve_utf16_tokens_after_the_error() {
    let cases = [
        (
            "cynefin",
            "cynefin-beta\ncomplex \"🤓 Before\"\n?\nclear \"🤓 After\"\n",
        ),
        (
            "treeView",
            "treeView-beta\n\"🤓 Before\"\n:::broken\n\"🤓 After\"\n",
        ),
        ("treemap", "treemap\n\"🤓 Before\": 1\n?\n\"🤓 After\": 2\n"),
        (
            "venn",
            "venn-beta\nset \"🤓 Before\"\n?\nset \"🤓 After\"\n",
        ),
    ];

    for (family, source) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                format!("file:///tmp/{family}-structured-recovery.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan = plan_semantic_tokens_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("{family} recovery plan failed: {error}"));
        assert_non_overlapping(plan.tokens());

        for text in ["Before", "After"] {
            let offset = source.find(text).expect("test token offset");
            let position = snapshot
                .source_map()
                .utf16_position(offset)
                .expect("test token UTF-16 position");
            assert!(
                plan.tokens().iter().any(|token| {
                    token.line == position.line as u32
                        && token.start <= position.character as u32
                        && token.start + token.length > position.character as u32
                }),
                "{family} recovery plan does not cover {text}: {:?}",
                plan.tokens()
            );
        }
    }
}

fn assert_non_overlapping(tokens: &[merman_editor_core::PlannedToken]) {
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlap: {pair:?}"
        );
    }
}
