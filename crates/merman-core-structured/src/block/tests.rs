use super::*;

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

#[test]
fn block_space_materialization_observes_cancellation() {
    let mut space = Block::new("space".to_string());
    space.block_type = "space".to_string();
    space.label = Some(String::new());
    space.width = Some(1_024.0);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    let mut db = BlockDb::default();
    db.clear();

    assert!(matches!(
        db.set_hierarchy(
            BlockDocument {
                records: vec![space],
                blocks: vec![0],
                failure: None
            },
            &MermaidConfig::default(),
            false,
            &control
        ),
        Err(merman_model::operation::OperationCancelled { .. })
    ));
    assert!(db.blocks.len() > 2);
    assert!(db.blocks.len() < 1_024);
}

#[test]
fn block_space_expansion_budget_covers_standalone_and_inline_statements() {
    for (first, second) in [
        ("space:100000", "space:100001"),
        ("A space:100000 B", "C space:100001 D"),
        ("space:100000", "A space:100001 B"),
    ] {
        let source = format!("block\n{first}\n{second}\n");
        let control = OperationControl::new();
        let runtime = merman_model::runtime::RuntimePolicy::deterministic()
            .begin_operation()
            .unwrap();
        let mut parser = Parser::new(
            &source,
            #[cfg(feature = "editor-facts")]
            None,
            false,
            &runtime,
            &control,
        );
        parser.parse_header().unwrap();
        // Inspect syntax before the accepted first statement materializes any placeholders.
        let document = parser.parse_document(false).unwrap().unwrap();
        let failure = document.failure.expect("total expansion exceeds admission");
        let BlockIssueLocation::Exact(span) = failure.location else {
            panic!("expected bounded expansion diagnostic")
        };
        let start = source.find("100001").unwrap();
        assert_eq!(span, SourceSpan::new(start, start + 6));
        assert!(failure.message.contains("total block space width"));
        assert!(document.records.len() <= 4);
    }
}

#[test]
fn block_cancellation_releases_records_after_nested_completion() {
    const DEPTH: usize = 500;
    let input = deep_block_chain(DEPTH);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2 * (DEPTH + 3));
    let runtime = merman_model::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let mut parser = Parser::new(
        &input,
        #[cfg(feature = "editor-facts")]
        None,
        false,
        &runtime,
        &control,
    );
    parser.parse_header().unwrap();
    assert!(parser.parse_document(false).is_err());
    assert!(
        parser
            .records
            .iter()
            .any(|record| { record.block_type == "composite" && !record.children.is_empty() }),
        "cancellation must follow a completed nested child"
    );
    drop(parser);
    assert!(
        parse_block_source_controlled(
            "block\nA\n",
            &MermaidConfig::default(),
            false,
            &runtime,
            &OperationControl::new(),
        )
        .unwrap()
        .is_ok()
    );
}
