mod support;

use merman_editor_core::{DocumentKind, PlannedTokenKind, plan_semantic_tokens_for_snapshot};
use support::SnapshotHarness;

#[test]
fn langium_basic_plans_merge_parser_lexemes_without_overlap() {
    let cases = [
        (
            "info",
            "info showInfo\n%% note\ntitle Runtime facts\n",
            PlannedTokenKind::String,
        ),
        (
            "pie",
            "pie showData\n%% note\n\"A\": 12.5\n",
            PlannedTokenKind::Number,
        ),
        (
            "packet",
            "packet-beta\n%% note\n0-7: \"Header\"\n",
            PlannedTokenKind::Operator,
        ),
        (
            "radar",
            concat!(
                "radar-beta:\n",
                "%% note\n",
                "axis perf[\"Performance\"], dx[\"DX\"]\n",
                "curve react {4, 5}\n",
            ),
            PlannedTokenKind::Delimiter,
        ),
    ];

    for (family, source, distinctive_kind) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{family}-langium.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan = plan_semantic_tokens_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("{family} plan failed: {error}"));

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5, "{family}");
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == PlannedTokenKind::Keyword),
            "{family} plan has no parser keyword: {:?}",
            plan.tokens()
        );
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == PlannedTokenKind::Comment),
            "{family} plan has no global comment: {:?}",
            plan.tokens()
        );
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == distinctive_kind),
            "{family} plan is missing {distinctive_kind:?}: {:?}",
            plan.tokens()
        );
        assert_non_overlapping(plan.tokens());
    }
}

#[test]
fn langium_basic_recovery_plans_cover_both_sides_of_the_error() {
    let cases = [
        ("info", "info showInfo\ntitle Before\n?\naccTitle: After\n"),
        ("pie", "pie showData\n\"Before\": 1\n?\n\"After\": 2\n"),
        ("packet", "packet\n0-7: \"Before\"\n?\n+8: \"After\"\n"),
        (
            "radar",
            "radar-beta\naxis Before\n?\naxis After\ncurve values {1, 2}\n",
        ),
    ];

    for (family, source) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{family}-langium-recovery.mmd"),
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
