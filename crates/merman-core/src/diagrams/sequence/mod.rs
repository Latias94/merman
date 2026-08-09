include_checked_in_lalrpop_parser!(
    #[allow(
        clippy::empty_line_after_outer_attr,
        clippy::type_complexity,
        clippy::result_large_err
    )]
    sequence_grammar,
    "sequence_grammar.rs"
);

// Mermaid 11.16.1 sequence diagram constants (SequenceDB.LINETYPE / PLACEMENT).
const LINETYPE_SOLID: i32 = 0;
const LINETYPE_DOTTED: i32 = 1;
const LINETYPE_NOTE: i32 = 2;
const LINETYPE_SOLID_CROSS: i32 = 3;
const LINETYPE_DOTTED_CROSS: i32 = 4;
const LINETYPE_SOLID_OPEN: i32 = 5;
const LINETYPE_DOTTED_OPEN: i32 = 6;
const LINETYPE_LOOP_START: i32 = 10;
const LINETYPE_LOOP_END: i32 = 11;
const LINETYPE_ALT_START: i32 = 12;
const LINETYPE_ALT_ELSE: i32 = 13;
const LINETYPE_ALT_END: i32 = 14;
const LINETYPE_OPT_START: i32 = 15;
const LINETYPE_OPT_END: i32 = 16;
const LINETYPE_ACTIVE_START: i32 = 17;
const LINETYPE_ACTIVE_END: i32 = 18;
const LINETYPE_PAR_START: i32 = 19;
const LINETYPE_PAR_AND: i32 = 20;
const LINETYPE_PAR_END: i32 = 21;
const LINETYPE_RECT_START: i32 = 22;
const LINETYPE_RECT_END: i32 = 23;
const LINETYPE_SOLID_POINT: i32 = 24;
const LINETYPE_DOTTED_POINT: i32 = 25;
const LINETYPE_AUTONUMBER: i32 = 26;
const LINETYPE_CRITICAL_START: i32 = 27;
const LINETYPE_CRITICAL_OPTION: i32 = 28;
const LINETYPE_CRITICAL_END: i32 = 29;
const LINETYPE_BREAK_START: i32 = 30;
const LINETYPE_BREAK_END: i32 = 31;
const LINETYPE_PAR_OVER_START: i32 = 32;
const LINETYPE_BIDIRECTIONAL_SOLID: i32 = 33;
const LINETYPE_BIDIRECTIONAL_DOTTED: i32 = 34;
const LINETYPE_SOLID_TOP: i32 = 41;
const LINETYPE_SOLID_BOTTOM: i32 = 42;
const LINETYPE_STICK_TOP: i32 = 43;
const LINETYPE_STICK_BOTTOM: i32 = 44;
const LINETYPE_SOLID_ARROW_TOP_REVERSE: i32 = 45;
const LINETYPE_SOLID_ARROW_BOTTOM_REVERSE: i32 = 46;
const LINETYPE_STICK_ARROW_TOP_REVERSE: i32 = 47;
const LINETYPE_STICK_ARROW_BOTTOM_REVERSE: i32 = 48;
const LINETYPE_SOLID_TOP_DOTTED: i32 = 51;
const LINETYPE_SOLID_BOTTOM_DOTTED: i32 = 52;
const LINETYPE_STICK_TOP_DOTTED: i32 = 53;
const LINETYPE_STICK_BOTTOM_DOTTED: i32 = 54;
const LINETYPE_SOLID_ARROW_TOP_REVERSE_DOTTED: i32 = 55;
const LINETYPE_SOLID_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 56;
const LINETYPE_STICK_ARROW_TOP_REVERSE_DOTTED: i32 = 57;
const LINETYPE_STICK_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 58;
const LINETYPE_CENTRAL_CONNECTION: i32 = 59;
const LINETYPE_CENTRAL_CONNECTION_REVERSE: i32 = 60;
const LINETYPE_CENTRAL_CONNECTION_DUAL: i32 = 61;

const PLACEMENT_LEFT_OF: i32 = 0;
const PLACEMENT_RIGHT_OF: i32 = 1;
const PLACEMENT_OVER: i32 = 2;

mod ast;
mod db;
mod lexer;
mod parse;
mod render_model;

use ast::Action;
pub(crate) use lexer::{LexError, Tok};

pub(crate) use parse::parse_sequence_json_and_editor_facts;
pub(crate) use parse::{parse_sequence, parse_sequence_model_for_render};
#[cfg(test)]
pub(crate) use parse::{
    reset_sequence_syntax_construction_count, sequence_syntax_construction_count,
};
pub(crate) use render_model::render_model_to_compat_json;
// Keep terminal consumers on the typed projection instead of duplicating LINETYPE decoding.
pub use render_model::{
    SequenceActor, SequenceAutonumber, SequenceBox, SequenceCentralDecoration,
    SequenceDiagramRenderModel, SequenceMessage, SequenceMessageDirection, SequenceMessageKind,
    SequenceMessageMarker, SequenceMessagePayload, SequenceMessageStroke, SequenceNote,
    SequenceSignalSemantics,
};

#[cfg(test)]
mod tests {
    use super::{
        SequenceCentralDecoration, SequenceMessageDirection, SequenceMessageKind,
        SequenceMessageMarker, SequenceMessageStroke, render_model_to_compat_json,
    };
    use crate::{Engine, ParseOptions, RenderSemanticModel};

    #[test]
    fn parser_preserves_bracketed_block_titles_for_the_renderer() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"sequenceDiagram
    par [Action 1]
        Alice->>Bob: First
    and [Action 2]
        Bob-->>Alice: Second
    end"#,
                ParseOptions::strict(),
            )
            .expect("parse should succeed")
            .expect("sequence diagram should be detected");
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected typed Sequence render model");
        };

        assert_eq!(model.messages[0].message_text(), "[Action 1]");
        assert_eq!(model.messages[2].message_text(), "[Action 2]");
    }

    #[test]
    fn typed_render_model_projects_exact_compatibility_json() {
        let source = r#"sequenceDiagram
title: Delivery
accTitle: Delivery sequence
autonumber 10 5
participant Alice
actor Bob
box rgb(240,240,240) Team
participant Carol
end
Alice->>Bob: Request
Note over Alice,Bob: Review
create participant Worker
Bob->>Worker: Start
destroy Worker
Worker-->>Bob: Done"#;
        let engine = crate::Engine::new();
        let compat = engine
            .parse_diagram_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let typed = engine
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Sequence(model) = typed.model() else {
            panic!("expected Sequence render model");
        };

        assert_eq!(
            render_model_to_compat_json(model, typed.metadata()).unwrap(),
            compat.model
        );
    }

    #[test]
    fn typed_render_model_projects_pinned_signal_semantics() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"sequenceDiagram
participant A
participant B
A->>B: filled
A->B: headless solid
A-->B: headless dotted
A-xB: cross
A-)B: point
A<<->>B: both
A-|\B: filled half top
A/|-B: reverse filled half top
A--//B: dotted open half bottom"#,
                ParseOptions::strict(),
            )
            .expect("parse should succeed")
            .expect("sequence diagram should be detected");
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected typed Sequence render model");
        };

        let expected = [
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::Filled,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::Cross,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::Point,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::Filled,
                SequenceMessageMarker::Filled,
                SequenceMessageDirection::Bidirectional,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageDirection::Forward,
            ),
            (
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::OpenHalfBottom,
                SequenceMessageDirection::Forward,
            ),
        ];

        assert_eq!(model.messages.len(), expected.len());
        for (message, expected) in model.messages.iter().zip(expected) {
            let semantics = message
                .signal_semantics()
                .expect("message should project signal semantics");
            assert_eq!(message.semantic_kind(), SequenceMessageKind::Signal);
            assert_eq!(
                (
                    semantics.stroke,
                    semantics.source_marker,
                    semantics.target_marker,
                    semantics.direction,
                ),
                expected,
                "{}",
                message.message_text()
            );
        }
    }

    #[test]
    fn typed_render_model_projects_every_pinned_half_arrow_semantic() {
        let cases = [
            (
                "A-|\\B: 0",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageDirection::Forward,
            ),
            (
                "A-|/B: 1",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::FilledHalfBottom,
                SequenceMessageDirection::Forward,
            ),
            (
                "A-\\\\B: 2",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::OpenHalfTop,
                SequenceMessageDirection::Forward,
            ),
            (
                "A-//B: 3",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::None,
                SequenceMessageMarker::OpenHalfBottom,
                SequenceMessageDirection::Forward,
            ),
            (
                "A--|\\B: 4",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageDirection::Forward,
            ),
            (
                "A--|/B: 5",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::FilledHalfBottom,
                SequenceMessageDirection::Forward,
            ),
            (
                "A--\\\\B: 6",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::OpenHalfTop,
                SequenceMessageDirection::Forward,
            ),
            (
                "A--//B: 7",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::None,
                SequenceMessageMarker::OpenHalfBottom,
                SequenceMessageDirection::Forward,
            ),
            (
                "A/|-B: 8",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A\\|-B: 9",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::FilledHalfBottom,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A//-B: 10",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::OpenHalfTop,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A\\\\-B: 11",
                SequenceMessageStroke::Solid,
                SequenceMessageMarker::OpenHalfBottom,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A/|--B: 12",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::FilledHalfTop,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A\\|--B: 13",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::FilledHalfBottom,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A//--B: 14",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::OpenHalfTop,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
            (
                "A\\\\--B: 15",
                SequenceMessageStroke::Dotted,
                SequenceMessageMarker::OpenHalfBottom,
                SequenceMessageMarker::None,
                SequenceMessageDirection::Reverse,
            ),
        ];
        let input = format!(
            "sequenceDiagram\nparticipant A\nparticipant B\n{}",
            cases
                .iter()
                .map(|(source, ..)| *source)
                .collect::<Vec<_>>()
                .join("\n")
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .expect("parse should succeed")
            .expect("sequence diagram should be detected");
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected typed Sequence render model");
        };

        assert_eq!(model.messages.len(), cases.len());
        for (message, (_, stroke, source, target, direction)) in model.messages.iter().zip(cases) {
            let semantics = message.signal_semantics().unwrap();
            assert_eq!(
                (
                    semantics.stroke,
                    semantics.source_marker,
                    semantics.target_marker,
                    semantics.direction,
                ),
                (stroke, source, target, direction),
                "{}",
                message.message_text()
            );
        }
    }

    #[test]
    fn typed_render_model_classifies_central_marker_records_without_losing_decorations() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"sequenceDiagram
participant A
participant B
A->>()B: target
A()->>B: source
A()->>()B: both"#,
                ParseOptions::strict(),
            )
            .expect("parse should succeed")
            .expect("sequence diagram should be detected");
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected typed Sequence render model");
        };

        assert_eq!(model.messages.len(), 7);
        assert_eq!(
            model.messages[0].central_decoration(),
            Some(SequenceCentralDecoration::Target)
        );
        assert_eq!(
            model.messages[1].semantic_kind(),
            SequenceMessageKind::CentralDecorationRecord
        );
        assert_eq!(
            model.messages[2].central_decoration(),
            Some(SequenceCentralDecoration::Source)
        );
        assert_eq!(
            model.messages[3].semantic_kind(),
            SequenceMessageKind::CentralDecorationRecord
        );
        assert_eq!(
            model.messages[4].central_decoration(),
            Some(SequenceCentralDecoration::Both)
        );
        assert!(model.messages[5..].iter().all(|message| {
            message.semantic_kind() == SequenceMessageKind::CentralDecorationRecord
        }));
    }
}
