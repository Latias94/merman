use super::*;
use merman_model::families::state::StateStatement;
use std::fmt::Write as _;

#[test]
fn state_syntax_hands_off_flat_statements_and_the_root_id() {
    let source = concat!(
        "stateDiagram-v2\n",
        "state Outer {\nstate 内部 {\nA --> B: label\n}\n}\n",
        "After\n",
    );
    let control = OperationControl::new();
    let StateParsedDocument { builder, root } = StateSyntax::lex(source, &control)
        .unwrap()
        .into_document(&control)
        .unwrap()
        .unwrap();

    let [StateStatement::State(outer), StateStatement::State(after)] = builder.get(root).unwrap()
    else {
        panic!("the root keeps the outer state and its following sibling");
    };
    assert_eq!(outer.id, "Outer");
    assert_eq!(after.id, "After");
    let outer_id = outer.doc.unwrap();
    let [StateStatement::State(inner)] = builder.get(outer_id).unwrap() else {
        panic!("the outer state owns the inner state");
    };
    let inner_span = inner.id_span.unwrap();
    assert_eq!(&source[inner_span.start..inner_span.end], "内部");
    let inner_id = inner.doc.unwrap();
    let [StateStatement::Relation(relation)] = builder.get(inner_id).unwrap() else {
        panic!("the inner state owns the relationship");
    };
    assert_eq!(relation.state1.id, "A");
    assert_eq!(relation.state2.id, "B");
    assert_eq!(relation.description.as_deref(), Some("label"));
    assert_eq!(
        [inner_id.index(), outer_id.index(), root.index()],
        [0, 1, 2]
    );

    let document = builder
        .build_controlled(Some(root), &control)
        .unwrap()
        .unwrap();
    assert_eq!(document.root(), Some(root));
    assert_eq!(document.len(), 3);
}

#[test]
fn state_syntax_cancellation_interrupts_token_delivery_with_completed_children() {
    let mut source = String::from("stateDiagram-v2\nstate Outer {\n");
    for index in 0..192 {
        writeln!(source, "state Child{index} {{\n}}").unwrap();
    }
    source.push_str("}\n");

    let control = OperationControl::new();
    let syntax = StateSyntax::lex(&source, &control).unwrap();
    // Admit the pre-parse checkpoint and the first token batch.
    control.cancel_after_checkpoints(2);
    let Err(cancelled) = syntax.into_document(&control) else {
        panic!("token delivery must observe cancellation");
    };
    assert_eq!(control.checkpoint(), Err(cancelled));
}

#[cfg(feature = "editor-facts")]
#[test]
fn state_syntax_events_keep_later_utf8_tokens_after_lexical_failure() {
    let source = "stateDiagram-v2\r\nBefore\r\n-\r\n後段\r\n";
    let control = OperationControl::new();
    let syntax = StateSyntax::lex(source, &control).unwrap();
    let failed = syntax.events().iter().position(Result::is_err).unwrap();
    let start = source.find("後段").unwrap();
    assert!(syntax.events()[failed + 1..].iter().any(|event| {
        matches!(event, Ok((from, StateToken::Id(id), end))
            if id == "後段" && *from == start && *end == start + id.len())
    }));
    let diagnostic = syntax.into_document(&control).unwrap().unwrap_err();
    assert!(
        matches!(diagnostic, StateSyntaxDiagnostic::Lexical { error, .. }
        if error.message == "Unexpected character '-'")
    );
}
