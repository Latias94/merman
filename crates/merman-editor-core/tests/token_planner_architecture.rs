use merman_editor_core::{
    DocumentKind, DocumentWorkspace, PlannedTokenKind, plan_semantic_tokens_for_snapshot,
};

#[test]
fn architecture_parser_lexemes_project_to_utf16_without_overlap() {
    let cases = [
        concat!(
            "architecture-beta\r\n",
            "group api(cloud)[\"API 服务\"]\r\n",
            "service db(database)[Database] in api\r\n",
            "service app(server)[Application] in api\r\n",
            "db:L -- R:app\r\n",
        ),
        concat!(
            "architecture-beta\n",
            "service before(server)[Before]\n",
            "service broken(\n",
            "service after(server)[\"🤓 After\"]\n",
        ),
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            format!("file:///tmp/architecture-{index}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let plan =
            plan_semantic_tokens_for_snapshot(&snapshot).expect("architecture semantic token plan");

        for expected in [
            PlannedTokenKind::Keyword,
            PlannedTokenKind::Variable,
            PlannedTokenKind::Delimiter,
        ] {
            assert!(
                plan.tokens().iter().any(|token| token.kind == expected),
                "missing {expected:?}: {:?}",
                plan.tokens()
            );
        }
        for pair in plan.tokens().windows(2) {
            assert!(
                (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
                "overlapping token plan: {pair:?}"
            );
        }
    }
}
