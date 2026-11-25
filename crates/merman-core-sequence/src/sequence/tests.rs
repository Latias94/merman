use super::{SequenceAction, SequenceSyntax, lexer::Lexer, parser};
use merman_model::families::sequence::LINETYPE_SOLID;
use merman_model::{CancelReason, OperationControl};
use std::cell::Cell;
use std::fmt::Write as _;

fn parser_actions(source: &str) -> Vec<SequenceAction> {
    let control = OperationControl::new();
    SequenceSyntax::lex(source, &control)
        .unwrap()
        .into_actions(&control)
        .unwrap()
        .expect("Sequence parser accepts the workload")
}

#[test]
fn sequence_message_actions_keep_database_replay_order() {
    let actions = parser_actions("sequenceDiagram\n顾客->>+服务: message\n");
    let [
        SequenceAction::EnsureParticipant { id: from },
        SequenceAction::EnsureParticipant { id: to },
        SequenceAction::AddMessage {
            from: message_from,
            to: message_to,
            signal_type: LINETYPE_SOLID,
            text,
            activate: true,
            central_connection: 0,
        },
        SequenceAction::ActiveStart { actor },
    ] = actions.as_slice()
    else {
        panic!("expected participant, message, and activation actions: {actions:?}");
    };
    assert_eq!((from.as_str(), to.as_str()), ("顾客", "服务"));
    assert_eq!((message_from, message_to, actor), (from, to, to));
    assert_eq!(text, "message");
}

#[test]
fn sequence_parser_emits_each_action_once_at_increasing_width_and_depth() {
    for size in [8, 128, 1_024] {
        let flat = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(size));
        assert_eq!(parser_actions(&flat).len(), size * 3);

        let mut nested = String::from("sequenceDiagram\n");
        for index in 0..size {
            writeln!(nested, "loop L{index}").unwrap();
        }
        nested.push_str("A->>B: message\n");
        nested.push_str(&"end\n".repeat(size));
        assert_eq!(parser_actions(&nested).len(), size * 2 + 3);

        let mut boxed = String::from("sequenceDiagram\nbox Team\n");
        for index in 0..size {
            writeln!(boxed, "participant P{index}").unwrap();
        }
        boxed.push_str("end\n");
        assert_eq!(parser_actions(&boxed).len(), size + 2);

        let mut branched = String::from("sequenceDiagram\nalt Start\nA->>B: first\n");
        for index in 0..size {
            writeln!(branched, "else E{index}\nA->>B: branch").unwrap();
        }
        branched.push_str("end\n");
        assert_eq!(parser_actions(&branched).len(), size * 4 + 5);
    }
}

#[test]
fn sequence_action_emission_observes_cancellation() {
    let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(1_024));
    let seen = Cell::new(0);
    let events = Lexer::new(&source).inspect(|_| seen.set(seen.get() + 1));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let error = parser::parse(events, &control)
        .expect_err("construction checkpoint interrupts action emission");
    assert_eq!(error.reason, CancelReason::Requested);
    assert!(seen.get() < 128);
}
