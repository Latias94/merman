use super::super::{ErDb, ErParsedActions, ErSyntax};
use super::parse;
use crate::ParseDiagnosticSpanKind::{Exact, InsertionPoint};
use crate::{OperationControl, SourceSpan};
use std::fmt::Write as _;

fn parse_source(source: &str) -> ErParsedActions {
    let control = OperationControl::new();
    let tape = ErSyntax::lex(source, &control).unwrap();
    parse(tape.events.into_iter(), &control).unwrap()
}

#[test]
fn er_parser_preserves_expected_tokens_and_spans() {
    let endpoint = "erDiagram\nA ||--|| B {";
    let role = "erDiagram\nA ||--|| B : \t";
    let role_end = role.find(':').unwrap() + 1;
    let subgraph = "erDiagram\nsubgraph G\n \n";
    let attribute_block = "erDiagram\nA { int id   ";
    for (source, message, span, span_kind) in [
        (
            "\nA",
            "unexpected name; expected \"er diagram\"",
            SourceSpan::new(1, 2),
            Exact,
        ),
        (
            endpoint,
            "unexpected block start; expected style separator, colon",
            SourceSpan::new(endpoint.len() - 1, endpoint.len()),
            Exact,
        ),
        (
            role,
            "unexpected end of input; expected name, str",
            SourceSpan::new(role_end, role_end),
            InsertionPoint,
        ),
        (
            subgraph,
            concat!(
                "unexpected end of input; expected name, acc title, acc descr, ",
                "acc descr multiline, diagram direction, \"style\", \"class def\", ",
                "\"class\", \"subgraph\", \"end\""
            ),
            SourceSpan::new(subgraph.len(), subgraph.len()),
            InsertionPoint,
        ),
        (
            attribute_block,
            "EOF inside attribute block",
            SourceSpan::new(attribute_block.len(), attribute_block.len()),
            Exact,
        ),
    ] {
        let diagnostic = parse_source(source).unwrap_err();
        assert_eq!(diagnostic.message(), message, "{source:?}");
        assert_eq!(diagnostic.span(), Some(span), "{source:?}");
        assert_eq!(diagnostic.span_kind(), span_kind, "{source:?}");
    }
}

#[test]
fn er_parser_keeps_large_attribute_blocks_in_source_order() {
    let count = 4096;
    let mut source = String::from("erDiagram\nA {\n");
    for index in 0..count {
        writeln!(source, "int field_{index:04}").unwrap();
    }
    source.push_str("}\n");

    let (arena, actions) = parse_source(&source).unwrap();
    let mut db = ErDb::new();
    db.replay(arena, actions, &OperationControl::new()).unwrap();
    let model = db.into_semantic_model();
    let attributes = &model.entities["A"].attributes;
    assert_eq!(attributes.len(), count);
    for (index, attribute) in attributes.iter().enumerate() {
        assert_eq!(attribute.name, format!("field_{index:04}"));
    }
}
