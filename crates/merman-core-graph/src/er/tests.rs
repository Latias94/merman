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
        db.replay(arena, actions, &control, STYLE_REPLAY_LIMIT)
            .unwrap();
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
    let Err(ErFailure::Cancelled(cancelled)) =
        db.replay(arena, actions, &control, STYLE_REPLAY_LIMIT)
    else {
        panic!("subgraph completion observes cancellation");
    };
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

fn replay_styles_with_control(
    source: &str,
    control: &OperationControl,
    limit: StyleReplayCost,
) -> (ErDb, Result<(), ErFailure>) {
    let parsing = OperationControl::new();
    let (arena, actions) = ErSyntax::lex(source, &parsing)
        .unwrap()
        .into_actions(&parsing)
        .unwrap()
        .unwrap();
    let mut db = ErDb::new();
    let result = db.replay(arena, actions, control, limit);
    (db, result)
}

#[test]
fn er_style_replay_charges_exact_cumulative_copies_and_utf8_bytes() {
    let source = concat!(
        "erDiagram\n",
        "style FUTURE color:orange\n",
        "class FUTURE early\n",
        "subgraph SHARED\n",
        "INNER\n",
        "end\n",
        "SHARED\n",
        "FUTURE\n",
        "style SHARED,SHARED,MISSING fill:é,stroke:blue\n",
        "class SHARED,SHARED,MISSING hot,冷\n",
        "classDef hot,cold,hot fillcolor:fill,stroke:blue\n",
        "classDef hot fill:blue\n",
    );
    let exact = StyleReplayCost {
        string_entries: 22,
        text_bytes: 238,
    };
    let control = OperationControl::new();
    let (db, result) = replay_styles_with_control(source, &control, exact);
    result.unwrap();
    let styles = ["fill:é", "stroke:blue", "fill:é", "stroke:blue"];
    assert_eq!(db.entities["SHARED"].css_styles, styles);
    assert_eq!(db.entities["SHARED"].css_classes, "default hot 冷 hot 冷");
    assert_eq!(db.subgraphs[0].css_styles, styles);
    assert_eq!(db.subgraphs[0].classes, ["hot", "冷", "hot", "冷"]);
    assert_eq!(db.entities["FUTURE"].css_classes, "default");
    assert!(db.entities["FUTURE"].css_styles.is_empty());
    assert!(!db.entities.contains_key("MISSING"));
    assert_eq!(
        db.classes.keys().map(String::as_str).collect::<Vec<_>>(),
        ["hot", "cold"]
    );
    assert_eq!(
        db.classes["hot"].styles,
        [
            "fillcolor:fill",
            "stroke:blue",
            "fillcolor:fill",
            "stroke:blue",
            "fill:blue"
        ]
    );
    assert_eq!(
        db.classes["hot"].text_styles,
        ["bgFillcolor:bgFill", "bgFillcolor:bgFill"]
    );
    assert_eq!(db.classes["cold"].styles, ["fillcolor:fill", "stroke:blue"]);

    for (limit, expected_unit, expected_actual) in [
        (
            StyleReplayCost {
                string_entries: exact.string_entries - 1,
                ..exact
            },
            "string entries",
            exact.string_entries,
        ),
        (
            StyleReplayCost {
                text_bytes: exact.text_bytes - 1,
                ..exact
            },
            "UTF-8 bytes",
            exact.text_bytes,
        ),
    ] {
        let (db, result) = replay_styles_with_control(source, &control, limit);
        let Err(ErFailure::Syntax(ErSyntaxDiagnostic::StyleExpansionLimit {
            span,
            unit,
            actual,
            maximum,
        })) = result
        else {
            panic!("expected an ER style construction limit");
        };
        let keyword = source.rfind("classDef").unwrap();
        assert_eq!(span, SourceSpan::new(keyword, keyword + "classDef".len()));
        assert_eq!(unit, expected_unit);
        assert_eq!(actual, expected_actual);
        assert_eq!(maximum, expected_actual - 1);
        assert_eq!(db.classes["hot"].styles.len(), 4);
    }
}

#[test]
fn er_style_replay_refuses_a_whole_directive_before_copying() {
    let source = "erDiagram\nA\nB\nstyle A,B fill:red,color:blue\n";
    let limit = StyleReplayCost {
        string_entries: 3,
        text_bytes: 36,
    };
    let (db, result) = replay_styles_with_control(source, &OperationControl::new(), limit);
    assert!(matches!(
        result,
        Err(ErFailure::Syntax(ErSyntaxDiagnostic::StyleExpansionLimit {
            unit: "string entries",
            actual: 4,
            maximum: 3,
            ..
        }))
    ));
    assert!(
        db.entities
            .values()
            .all(|entity| entity.css_styles.is_empty())
    );

    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    let (db, result) = replay_styles_with_control(source, &control, limit);
    let Err(ErFailure::Cancelled(cancelled)) = result else {
        panic!("cancellation at admission precedes a construction limit");
    };
    assert_eq!(control.checkpoint(), Err(cancelled));
    assert!(
        db.entities
            .values()
            .all(|entity| entity.css_styles.is_empty())
    );
}

#[test]
fn er_style_replay_poll_interrupts_a_single_directive_between_copies() {
    let source = "erDiagram\nA\nB\nstyle A,B fill:red,color:blue\n";
    let control = OperationControl::new();
    control.cancel_after_checkpoints(6);
    let (db, result) = replay_styles_with_control(source, &control, STYLE_REPLAY_LIMIT);
    let Err(ErFailure::Cancelled(cancelled)) = result else {
        panic!("style copy loop observes cancellation");
    };
    assert_eq!(control.checkpoint(), Err(cancelled));
    assert_eq!(db.entities["A"].css_styles, ["fill:red", "color:blue"]);
    assert!(db.entities["B"].css_styles.is_empty());
}
