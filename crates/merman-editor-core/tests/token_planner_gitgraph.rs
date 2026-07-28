use merman_editor_core::{
    DocumentKind, DocumentWorkspace, PlannedTokenKind, plan_semantic_tokens_for_snapshot,
};

#[test]
fn gitgraph_parser_tokens_plan_across_crlf_unicode_and_recovery() {
    let cases = [
        concat!(
            "gitGraph TB:\r\n",
            "commit id:\"ROOT\" msg:\"开始\"\r\n",
            "branch \"功能\" order:2\r\n",
            "commit id:\"F1\"\r\n",
        ),
        concat!(
            "gitGraph\r\n",
            "commit id:\"C1\"\r\n",
            "checkout main trailing\r\n",
            "commit id:\"C2\" msg:\"后来\"\r\n",
        ),
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace
            .upsert(
                format!("file:///tmp/gitgraph-{index}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let plan =
            plan_semantic_tokens_for_snapshot(&snapshot).expect("gitGraph semantic token plan");

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
        for kind in [PlannedTokenKind::Keyword, PlannedTokenKind::Delimiter] {
            assert!(
                plan.tokens().iter().any(|token| token.kind == kind),
                "missing {kind:?}: {:?}",
                plan.tokens()
            );
        }
        assert_non_overlapping(plan.tokens());

        let target = if index == 0 { "开始" } else { "后来" };
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
            "gitGraph token plan does not cover {target}: {:?}",
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
