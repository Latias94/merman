use merman_editor_core::{
    DocumentKind, DocumentWorkspace, PlannedTokenKind, plan_semantic_tokens_for_snapshot,
};

#[test]
fn line_parser_families_produce_non_overlapping_semantic_token_plans() {
    let cases = [
        (
            "journey",
            "journey\nsection Checkout\nChoose item: 5: Alice, Bob\n",
            PlannedTokenKind::Event,
        ),
        (
            "gantt",
            concat!(
                "gantt\n",
                "dateFormat YYYY-MM-DD\n",
                "Task: task1, 2024-01-01, 2d\n",
            ),
            PlannedTokenKind::Date,
        ),
        (
            "sankey",
            "sankey-beta\n\"Source, A\",Target,12.5\n",
            PlannedTokenKind::Namespace,
        ),
    ];

    for (family, source, distinctive_kind) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            format!("file:///tmp/{family}-line-parser.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let plan = plan_semantic_tokens_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("{family} token plan failed: {error}"));

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5, "{family}");
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == PlannedTokenKind::Keyword),
            "{family} is missing its parser keyword: {:?}",
            plan.tokens()
        );
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == distinctive_kind),
            "{family} is missing {distinctive_kind:?}: {:?}",
            plan.tokens()
        );
        assert_non_overlapping(plan.tokens());
    }
}

#[test]
fn line_parser_recovery_plans_cover_tokens_before_and_after_the_error() {
    let cases = [
        (
            "journey",
            "journey\nBefore: 5: Alice\n: malformed\nAfter: 4: Bob\n",
        ),
        (
            "gantt",
            concat!(
                "gantt\n",
                "Before: before, 2024-01-01, 1d\n",
                "malformed statement\n",
                "After: after, 2024-01-02, 2d\n",
            ),
        ),
        ("sankey", "sankey\nBefore,Middle,1\nbroken\nAfter,End,2\n"),
    ];

    for (family, source) in cases {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            format!("file:///tmp/{family}-line-recovery.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let plan = plan_semantic_tokens_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("{family} recovery token plan failed: {error}"));
        assert_non_overlapping(plan.tokens());

        for text in ["Before", "After"] {
            let offset = source.find(text).expect("test token offset");
            let position = snapshot
                .source_map
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
