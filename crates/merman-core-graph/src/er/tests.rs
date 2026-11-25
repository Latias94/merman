use super::*;
use std::fmt::Write as _;

#[test]
fn er_parser_keeps_large_attribute_blocks_in_source_order() {
    let count = 4096;
    let mut source = String::from("erDiagram\nA {\n");
    for index in 0..count {
        writeln!(source, "int field_{index:04}").unwrap();
    }
    source.push_str("}\n");

    let control = OperationControl::new();
    let model = ErSyntax::lex(&source, &control)
        .unwrap()
        .into_model(&control)
        .unwrap()
        .unwrap();
    let attributes = &model.entities["A"].attributes;
    assert_eq!(attributes.len(), count);
    for (index, attribute) in attributes.iter().enumerate() {
        assert_eq!(attribute.name, format!("field_{index:04}"));
    }
}

fn nested_subgraphs(depth: usize) -> String {
    let mut source = String::from("erDiagram\n");
    for index in 0..depth {
        source.push_str(&format!("subgraph G{index}\n"));
    }
    source.push_str("LEAF\n");
    for _ in 0..depth {
        source.push_str("end\n");
    }
    source
}

#[test]
fn er_flat_action_storage_and_membership_work_grow_with_records() {
    for depth in [32, 128, 512] {
        let source = nested_subgraphs(depth);
        let control = OperationControl::new();
        let syntax = ErSyntax::lex(&source, &control).unwrap();
        let parsed = syntax.into_actions(&control).unwrap();
        let (arena, actions) = parsed.unwrap();
        assert_eq!(arena.records.len(), depth + 1);
        assert_eq!(actions.first, actions.last, "one top-level subgraph");

        let mut db = ErDb::new();
        db.replay(arena, actions, &control).unwrap();
        assert_eq!(db.subgraphs.len(), depth);
        assert_eq!(db.membership_candidates, depth);
        assert_eq!(db.completed_members.len(), depth);
        assert_eq!(db.subgraphs[0].nodes, ["LEAF"]);
        assert_eq!(db.subgraphs[depth - 1].nodes, ["G1"]);
    }
}

#[test]
fn er_cancellation_during_one_deep_action_preserves_completed_inner_subgraphs() {
    let depth = 1024;
    let source = nested_subgraphs(depth);
    let parsing = OperationControl::new();
    let syntax = ErSyntax::lex(&source, &parsing).unwrap();
    let parsed = syntax.into_actions(&parsing).unwrap();
    let (arena, actions) = parsed.unwrap();
    assert_eq!(actions.first, actions.last, "one top-level action");

    let control = OperationControl::new();
    control.cancel_after_checkpoints(12);
    let mut db = ErDb::new();
    let cancelled = db.replay(arena, actions, &control).unwrap_err();
    assert_eq!(control.checkpoint(), Err(cancelled));
    assert!(
        !db.subgraphs.is_empty(),
        "inner completion started before cancellation"
    );
    assert!(
        db.subgraphs.len() < depth,
        "the outer action was interrupted"
    );
    assert_eq!(db.subgraphs[0].id, "G1023");
    assert_eq!(db.subgraphs[0].nodes, ["LEAF"]);
    drop(db);
    let after = OperationControl::new();
    assert!(
        ErSyntax::lex("erDiagram\nAFTER\n", &after)
            .unwrap()
            .into_model(&after)
            .unwrap()
            .is_ok()
    );
}

#[test]
fn er_cancellation_during_token_delivery_retains_the_terminal_outcome() {
    let source = nested_subgraphs(64);
    let control = OperationControl::new();
    let syntax = ErSyntax::lex(&source, &control).unwrap();
    // The pre-parse check and first token batch succeed before token delivery is interrupted.
    control.cancel_after_checkpoints(2);
    let Err(cancelled) = syntax.into_model(&control) else {
        panic!("token delivery observes cancellation");
    };
    assert_eq!(control.checkpoint(), Err(cancelled));
}
