use super::*;
use merman_model::CancelReason;
use std::fmt::Write as _;

#[test]
fn ignored_statement_actions_are_collected_only_on_demand() {
    let source = "classDiagram\r\nFirst\r\nSecond\r\n";
    let control = OperationControl::new();
    let lex = || ClassSyntax::lex(source, &control).unwrap();
    assert_eq!(
        lex()
            .into_actions(false, &control)
            .unwrap()
            .unwrap()
            .count(),
        0
    );

    let spans = lex()
        .into_actions(true, &control)
        .unwrap()
        .unwrap()
        .map(|action| {
            let ClassAction::WarnIgnoredStatement { span } = action else {
                panic!("only ignored statements were authored");
            };
            &source[span.start..span.end]
        })
        .collect::<Vec<_>>();
    assert_eq!(spans, ["First", "Second"]);
}

#[test]
fn class_model_replay_keeps_warning_prefix_and_grammar_priority() {
    let source = "classDiagram\r\nFirst\r\nclass C {\r\nmethod)\r\n}\r\nSecond\r\n";
    let config = MermaidConfig::empty_object();
    let control = OperationControl::new();
    let mut warnings = Vec::new();

    let result = ClassSyntax::lex(source, &control)
        .unwrap()
        .into_model(&config, "classDiagram", Some(&mut warnings), &control)
        .unwrap();
    assert!(matches!(
        result,
        Err(ClassModelError::Db(message)) if message == "Invalid class method signature"
    ));
    assert_eq!(warnings.len(), 1);
    let span = warnings[0].span.unwrap();
    assert_eq!(&source[span.start..span.end], "First");

    let source = format!("{source}class ");
    warnings.clear();
    let result = ClassSyntax::lex(&source, &control)
        .unwrap()
        .into_model(&config, "classDiagram", Some(&mut warnings), &control)
        .unwrap();
    assert!(matches!(
        result,
        Err(ClassModelError::Grammar(
            ClassSyntaxDiagnostic::UnexpectedEof { .. }
        ))
    ));
    assert!(warnings.is_empty());
}

#[test]
#[cfg(feature = "editor-facts")]
fn normalized_names_preserve_their_authored_byte_spans() {
    let source = concat!(
        "classDiagram\r\n",
        "class Service API\r\n",
        "class `Quoted Name` 隣\r\n",
        "class Generic~T~ Next\r\n",
        "class Decl <<First Second>>\r\n",
    );
    let syntax = ClassSyntax::lex(source, &OperationControl::new()).unwrap();
    let names = syntax
        .events()
        .iter()
        .filter_map(|event| match event {
            Ok((start, Tok::Name(name), end)) => Some((name.as_str(), &source[*start..*end])),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            ("ServiceAPI", "Service API"),
            ("Quoted Name", "`Quoted Name`"),
            ("隣", "隣"),
            ("Generic~T~", "Generic~T~"),
            ("Next", "Next"),
            ("Decl", "Decl"),
            ("First", "First"),
            ("Second", "Second"),
        ]
    );
}

#[test]
#[cfg(feature = "editor-facts")]
fn lexical_expectations_preserve_recovery_slots_and_later_names() {
    let source =
        "classDiagram\r\ndirection sideways\r\nclick Service call handler(\r\nclass After\r\n";
    let control = OperationControl::new();
    let syntax = ClassSyntax::lex(source, &control).unwrap();
    let expectations = syntax
        .events()
        .iter()
        .filter_map(|event| {
            let error = event.as_ref().err()?;
            Some((
                error.expectation?,
                &source[error.span.start..error.span.end],
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expectations,
        [
            (ClassLexicalExpectation::Direction, "sideways"),
            (ClassLexicalExpectation::CallbackArguments, "("),
        ]
    );
    assert!(
        syntax.events().iter().any(|event| {
            matches!(event, Ok((_, ClassToken::Name(name), _)) if name == "After")
        })
    );
    assert!(matches!(
        syntax.into_actions(false, &control).unwrap(),
        Err(ClassSyntaxDiagnostic::Lexical(error)) if error.message == "invalid class direction"
    ));
}

#[test]
fn cancelled_syntax_construction_returns_the_operation_error() {
    let source = format!("classDiagram\n{}", "class C\n".repeat(256));
    let control = OperationControl::new();
    let syntax = ClassSyntax::lex(&source, &control).unwrap();
    control.cancel_after_checkpoints(2);
    let Err(cancelled) = syntax.into_actions(false, &control) else {
        panic!("Class action construction must observe cancellation");
    };
    assert_eq!(control.checkpoint(), Err(cancelled));
}

fn assert_carrier_work(source: &str, expected: (usize, usize, usize, usize)) {
    let control = OperationControl::new();
    let mut arena = ActionArena::default();
    let root = parser::parse(&mut arena, Lexer::new(source), &control)
        .unwrap()
        .expect("Class parser accepts the carrier workload");
    assert_eq!(arena.carrier_counts(), expected);
    let materialized = arena.into_actions(root).count();
    assert_eq!(materialized, expected.0);
    println!(
        "source_bytes={} action_records={} action_links={} class_id_records={} \
         class_id_links={} replayed_events={materialized}",
        source.len(),
        expected.0,
        expected.1,
        expected.2,
        expected.3,
    );
}

#[test]
fn grammar_carriers_attach_each_event_once_at_increasing_width_and_depth() {
    for size in [8, 128, 1_024] {
        let mut flat = String::from("classDiagram\n");
        for index in 0..size {
            writeln!(flat, "class C{index}").unwrap();
        }
        assert_carrier_work(&flat, (size, size - 1, 0, 0));

        let mut namespace = String::from("classDiagram\nnamespace N {\n");
        for index in 0..size {
            writeln!(namespace, "class C{index}").unwrap();
        }
        namespace.push_str("}\n");
        assert_carrier_work(&namespace, (size + 3, size + 2, size, size - 1));

        let mut nested = String::from("classDiagram\n");
        for index in 0..size {
            writeln!(nested, "namespace N{index} {{").unwrap();
        }
        nested.push_str("class Leaf\n");
        nested.push_str(&"}\n".repeat(size));
        assert_carrier_work(&nested, (size * 3 + 1, size * 3, 1, 0));
    }
}

#[test]
fn class_action_construction_observes_cancellation() {
    let source = format!("classDiagram\n{}", "class C\n".repeat(1_024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    let mut arena = ActionArena::default();
    let error = parser::parse(&mut arena, Lexer::new(&source), &control)
        .err()
        .expect("construction checkpoint interrupts the parser");
    assert!(arena.carrier_counts().0 < 1_024);
    assert_eq!(error.reason, CancelReason::Requested);
}

#[test]
fn classdef_target_lists_observe_cancellation() {
    let targets = (0..1_024)
        .map(|index| format!("C{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("classDiagram\nclassDef {targets} fill:red\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    let mut arena = ActionArena::default();
    let error = parser::parse(&mut arena, Lexer::new(&source), &control)
        .err()
        .expect("target list parsing must observe cancellation before expansion");
    assert_eq!(arena.carrier_counts().0, 0);
    assert_eq!(error.reason, CancelReason::Requested);
}
