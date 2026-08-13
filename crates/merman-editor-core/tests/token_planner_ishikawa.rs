mod support;

use merman_editor_core::{DocumentKind, PlannedTokenKind, plan_semantic_tokens_for_snapshot};
use support::SnapshotHarness;

#[test]
fn ishikawa_parser_tokens_plan_across_crlf_and_unicode() {
    let cases = [
        "ishikawa-beta 主要问题\r\n    原因一\r\n",
        "ishikawa-beta Problem\r\n    🤓 Later\r\n",
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/ishikawa-{index}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("ishikawa token plan");

        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == PlannedTokenKind::Keyword)
        );
        assert!(plan.tokens().iter().any(|token| {
            matches!(
                token.kind,
                PlannedTokenKind::String | PlannedTokenKind::Namespace
            )
        }));
        for pair in plan.tokens().windows(2) {
            assert!(
                (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
                "overlapping tokens: {pair:?}"
            );
        }
    }
}
