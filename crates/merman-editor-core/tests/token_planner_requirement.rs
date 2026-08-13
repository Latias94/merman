mod support;

use merman_editor_core::{DocumentKind, PlannedTokenKind, plan_semantic_tokens_for_snapshot};
use support::SnapshotHarness;

#[test]
fn requirement_parser_tokens_plan_across_crlf_unicode_and_recovery() {
    let cases = [
        concat!(
            "requirementDiagram\r\n",
            "functionalRequirement \"登录 需求\" {\r\n",
            "  id: REQ-login\r\n",
            "  text: \"用户可以登录\"\r\n",
            "  risk: high\r\n",
            "  verifyMethod: analysis\r\n",
            "}\r\n",
            "element api {\r\n",
            "  type: service\r\n",
            "}\r\n",
            "\"登录 需求\" - verifies -> api\r\n",
        ),
        concat!(
            "requirementDiagram\r\n",
            "requirement broken {\r\n",
            "  risk:\r\n",
            "}\r\n",
            "element 后续 {\r\n",
            "  type: service\r\n",
            "}\r\n",
        ),
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/requirement-{index}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan =
            plan_semantic_tokens_for_snapshot(&snapshot).expect("requirement semantic token plan");

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
        for kind in [PlannedTokenKind::Keyword, PlannedTokenKind::Delimiter] {
            assert!(
                plan.tokens().iter().any(|token| token.kind == kind),
                "missing {kind:?}: {:?}",
                plan.tokens()
            );
        }
        assert_non_overlapping(plan.tokens());

        let target = if index == 0 {
            "用户可以登录"
        } else {
            "后续"
        };
        let offset = source.find(target).expect("test token offset");
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
            "requirement token plan does not cover {target}: {:?}",
            plan.tokens()
        );
    }
}

fn assert_non_overlapping(tokens: &[merman_editor_core::PlannedToken]) {
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlapping tokens: {pair:?}"
        );
    }
}
