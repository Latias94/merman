mod support;

use merman_editor_core::{DocumentKind, PlannedTokenKind, plan_semantic_tokens_for_snapshot};
use support::SnapshotHarness;

#[test]
fn block_parser_lexemes_project_to_utf16_tokens_without_overlap() {
    let cases = [
        concat!(
            "block-beta\r\n",
            "  columns 3\r\n",
            "  block:容器[\"组\"]\r\n",
            "    columns auto\r\n",
            "    user((\"用户\")):2\r\n",
            "    route<[\"流\"]>(right, down)\r\n",
            "    user -- \"发送\" --> api[\"接口\"]\r\n",
            "  end\r\n",
            "  classDef hot fill:#f00,stroke:#111\r\n",
            "  class user,api hot\r\n",
            "  style api fill:#0f0\r\n",
        ),
        concat!(
            "block-beta\r\n",
            "  A<[\"方向\"]>(right, sideways)\r\n",
            "  后续[\"完成\"]\r\n",
        ),
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/block-{index}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("block semantic token plan");

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
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
        assert_non_overlapping(plan.tokens());

        let target = if index == 0 { "接口" } else { "完成" };
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
            "block token plan does not cover {target}: {:?}",
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
