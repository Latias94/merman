#![cfg(feature = "diagram-sequence")]

use merman_core::SemanticParser;
use merman_model::{
    families::sequence::{LINETYPE_AUTONUMBER, SequenceDiagramModel, SequenceMessagePayload},
    semantic::SemanticDiagramModel,
};

fn parse_sequence(source: &str) -> SequenceDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Sequence source");
    let SemanticDiagramModel::Sequence(model) = parsed.model else {
        panic!("expected the canonical Sequence model");
    };
    model
}

#[test]
fn autonumber_decimal_overflow_preserves_number_and_json_semantics() {
    // NUM has no digit limit, and the selected grammar converts it with Number().
    for (number, expected) in [
        ("17976931348623157".to_string(), 17976931348623156.0),
        ("9".repeat(308), 1.0e308),
        ("9".repeat(309), f64::INFINITY),
    ] {
        for (directive, start, step) in [
            (format!("autonumber {number}"), expected, 1.0),
            (format!("autonumber 1 {number}"), 1.0, expected),
        ] {
            for newline in ["\n", "\r\n"] {
                let source =
                    format!("sequenceDiagram{newline}{directive}{newline}A->>B: first{newline}");
                let model = parse_sequence(&source);
                assert_eq!(model.actor_order, ["A", "B"]);
                assert_eq!(model.messages.len(), 2);
                let record = &model.messages[0];
                assert_eq!(record.id, "0");
                assert_eq!(record.message_type, LINETYPE_AUTONUMBER);
                let SequenceMessagePayload::Autonumber(numbering) = &record.message else {
                    panic!("autonumber must retain its typed payload");
                };
                assert_eq!(numbering.start, Some(start));
                assert_eq!(numbering.step, Some(step));
                assert!(numbering.visible);
                assert!(!record.wrap);
                assert_eq!(model.messages[1].id, "1");
                assert_eq!(model.messages[1].message_text(), "first");

                #[cfg(feature = "compat-json")]
                {
                    let compat = serde_json::to_value(model.compatibility_projection("sequence"))
                        .expect("non-finite numbering projects as JSON null");
                    let payload = &compat["messages"][0]["message"];
                    assert_eq!(payload["visible"], true);
                    for (field, expected) in [("start", start), ("step", step)] {
                        let value = payload.get(field).expect("numbering fields remain present");
                        if expected.is_infinite() {
                            assert!(value.is_null(), "{field} must follow JSON.stringify");
                        } else {
                            assert_eq!(value.as_f64(), Some(expected));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn overflowing_autonumber_keeps_numeric_boundaries_and_original_error_spans() {
    use merman_core::{Error, ParseDiagnosticSpanKind};
    use merman_model::source::SourceSpan;

    let number = "9".repeat(309);
    for (directive, invalid) in [
        (format!("autonumber {number};"), number.as_str()),
        (format!("autonumber 1 {number};"), number.as_str()),
        (format!("autonumber {number} unexpected"), "unexpected"),
        (format!("autonumber 1 {number} unexpected"), "unexpected"),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "\u{feff}---\ntitle: 原稿😀\n---\nsequenceDiagram\n",
                    "participant 顧客\n{}\nparticipant 後続\n顧客->>後続: ping\n",
                ),
                directive,
            )
            .replace('\n', newline);
            let start = source
                .find(invalid)
                .expect("invalid token is in the source");
            let expected = SourceSpan::new(start, start + invalid.len());
            let parser = SemanticParser::new();
            let Error::DiagramParse { diagnostic, .. } = parser
                .parse_diagram_sync(&source)
                .expect_err("invalid numeric boundaries and trailing tokens must fail")
            else {
                panic!("invalid autonumber must remain a parse error");
            };
            assert_eq!(diagnostic.span(), Some(expected), "{directive}");
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
            assert_eq!(&source[expected.start..expected.end], invalid);

            #[cfg(feature = "editor-facts")]
            {
                use merman_core::{
                    EditorSemanticCompleteness, EditorSemanticDiagnosticKind, ParsedEditorFacts,
                    SemanticParseOutcome, SemanticSnapshotCapture,
                };

                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(&source)
                else {
                    panic!("invalid Sequence source must retain a recovery snapshot");
                };
                assert!(matches!(
                    snapshot.outcome(),
                    SemanticParseOutcome::Failed(_)
                ));
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("invalid autonumber must retain requested editor facts");
                };
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.diagnostics.iter().any(|diagnostic| {
                    diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                        && diagnostic.span == Some(expected)
                }));
                for actor in ["顧客", "後続"] {
                    let selections: Vec<_> = facts
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.name == actor)
                        .map(|symbol| symbol.selection)
                        .collect();
                    let authored: Vec<_> = source
                        .match_indices(actor)
                        .map(|(start, text)| SourceSpan::new(start, start + text.len()))
                        .collect();
                    assert_eq!(selections, authored);
                }
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn overflowing_autonumber_snapshot_preserves_following_actor_spans() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    let source = format!(
        concat!(
            "\u{feff}---\r\ntitle: 原稿😀\r\n---\r\nsequenceDiagram\r\n",
            "autonumber {}\r\n顧客😀->>後続: first\r\n",
        ),
        "9".repeat(309),
    );
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(&source)
    else {
        panic!("overflowing autonumber must produce a semantic snapshot");
    };
    let SemanticDiagramModel::Sequence(model) = snapshot
        .outcome()
        .parsed_model()
        .expect("decimal overflow is valid Sequence syntax")
    else {
        panic!("expected the canonical Sequence model");
    };
    assert_eq!(model.actor_order, ["顧客😀", "後続"]);
    let SequenceMessagePayload::Autonumber(numbering) = &model.messages[0].message else {
        panic!("autonumber must retain its typed payload");
    };
    assert_eq!(numbering.start, Some(f64::INFINITY));
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("valid Sequence source must retain requested editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
    for actor in ["顧客😀", "後続"] {
        let start = source.find(actor).expect("actor is in the source");
        let expected = SourceSpan::new(start, start + actor.len());
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == actor)
            .expect("actor must retain an editor symbol");
        assert_eq!(symbol.selection, expected);
        assert_eq!(symbol.span, expected);
    }
}
