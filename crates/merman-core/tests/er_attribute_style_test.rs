#![cfg(feature = "diagram-er")]

use merman_core::SemanticParser;
use merman_model::{families::er::ErDiagramModel, semantic::SemanticDiagramModel};

fn parse_er(source: &str) -> ErDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid source according to the selected ER grammar");
    let SemanticDiagramModel::Er(model) = parsed.model else {
        panic!("ER source must produce an ER model");
    };
    model
}

#[test]
fn empty_backtick_bodies_do_not_emit_attribute_words() {
    // erDiagram.jison's block/block_bq transitions emit only nonempty bodies.
    for newline in ["\n", "\r\n"] {
        for body in [
            "``int ``id",
            "`` int id",
            "int`` id",
            "int id``",
            "````int`` id``",
            "`int``id`",
            "int``id",
        ] {
            let model = parse_er(&format!("erDiagram{newline}A {{{body}}}{newline}"));
            let attrs = &model.entities["A"].attributes;
            assert_eq!(attrs.len(), 1, "{body}");
            assert_eq!(attrs[0].ty, "int", "{body}");
            assert_eq!(attrs[0].name, "id", "{body}");
        }
        for body in ["``", "````", " `` `` "] {
            let model = parse_er(&format!("erDiagram{newline}A {{{body}}}{newline}"));
            assert!(model.entities["A"].attributes.is_empty(), "{body}");
        }
    }
    let model = parse_er("erDiagram\nA {int id``PK ``\"comment\"}\n");
    assert_eq!(model.entities["A"].attributes[0].keys, ["PK"]);
    assert_eq!(model.entities["A"].attributes[0].comment, "comment");
}

#[test]
fn ordinary_attribute_words_follow_the_selected_character_range() {
    for word in ["µ", "µint", "À", "café", "属性", "😀", "a😀"] {
        for body in [format!("{word} id"), format!("int {word}")] {
            let model = parse_er(&format!("erDiagram\nA {{{body}}}\n"));
            assert_eq!(model.entities["A"].attributes.len(), 1, "{body}");
        }
    }
    for ch in ['\u{007f}', '\u{0080}', '\u{0085}', '\u{00ad}', '\u{00bf}'] {
        for word in [ch.to_string(), format!("a{ch}")] {
            for body in [format!("{word} id"), format!("int {word}")] {
                assert!(
                    SemanticParser::new()
                        .parse_diagram_sync(&format!("erDiagram\nA {{{body}}}\n"))
                        .is_err(),
                    "{body:?}"
                );
            }
        }
    }
    let model = parse_er("erDiagram\nA {`\u{0085}` `\u{00bf}`}\n");
    assert_eq!(model.entities["A"].attributes[0].ty, "\u{0085}");
    assert_eq!(model.entities["A"].attributes[0].name, "\u{00bf}");
}

#[test]
fn style_whitespace_is_skipped_only_at_token_boundaries() {
    for newline in ["\n", "\r\n"] {
        for (authored, expected) in [
            ("fill: red, stroke: blue", vec!["fill:red", "stroke:blue"]),
            ("fill:\u{0085}red", vec!["fill:\u{0085}red"]),
            ("fill:\u{feff}red", vec!["fill:red"]),
            ("fill:re\u{feff}d", vec!["fill:re\u{feff}d"]),
            ("fill:red\u{00a0}", vec!["fill:red\u{00a0}"]),
            ("fill:red\u{2028}", vec!["fill:red\u{2028}"]),
            ("fill:red #abc", vec!["fill:red#abc"]),
        ] {
            let source = format!(
                "erDiagram{newline}A{newline}style A {authored}{newline}classDef accent {authored}{newline}"
            );
            let model = parse_er(&source);
            assert_eq!(model.entities["A"].css_styles, expected, "{authored:?}");
            assert_eq!(model.classes["accent"].styles, expected, "{authored:?}");
        }
        let model = parse_er(&format!(
            "erDiagram{newline}A{newline}style \u{00a0}A fill:red{newline}classDef \u{feff}accent fill:blue{newline}"
        ));
        assert_eq!(model.entities["A"].css_styles, ["fill:red"]);
        assert_eq!(model.classes["accent"].styles, ["fill:blue"]);
    }
}

#[test]
fn style_separators_preserve_exclusive_mode_and_validate_components() {
    for newline in ["\n", "\r\n"] {
        let model = parse_er(&format!(
            "erDiagram{newline}A{newline}style A fill:red;{newline} B{newline}"
        ));
        assert_eq!(model.entities["A"].css_styles, ["fill:red"]);
        assert!(model.entities.contains_key("B"));

        let model = parse_er(&format!(
            "erDiagram{newline}A{newline}style A fill:red {newline} B{newline}"
        ));
        assert_eq!(model.entities["A"].css_styles, ["fill:redB"]);
        assert!(!model.entities.contains_key("B"));

        for tail in [
            "fill:red;;",
            "fill:red; B",
            "fill:red; ",
            "fill:red;style A stroke:blue",
            "fill:red,",
            "fill:red,,stroke:blue",
            "fill:rgb(1,2,3)",
            "fill:red!bad",
            "fill:red.bad",
            "fill:red\\,stroke:blue",
        ] {
            for command in ["style A", "classDef accent"] {
                let source = format!("erDiagram{newline}A{newline}{command} {tail}{newline}");
                assert!(
                    SemanticParser::new().parse_diagram_sync(&source).is_err(),
                    "{source:?}"
                );
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
mod editor {
    use super::*;
    use merman_core::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, ParsedEditorFacts,
        SemanticParseOutcome, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    #[test]
    fn empty_backticks_and_compacted_styles_keep_authored_ranges() {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}A {{``int ``属性``PK}}{newline}style A fill: red, stroke: blue;{newline}After{newline}"
            );
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("ER must retain its semantic snapshot");
            };
            assert!(matches!(
                snapshot.outcome(),
                SemanticParseOutcome::Parsed(_)
            ));
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("ER must retain parser-owned editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for name in ["int", "属性", "PK", "After"] {
                let start = source.find(name).unwrap();
                assert!(
                    facts.symbols.iter().any(|symbol| {
                        symbol.name == name
                            && symbol.selection == SourceSpan::new(start, start + name.len())
                    }),
                    "{name}: {:?}",
                    facts.symbols
                );
            }
            let payload = "fill: red, stroke: blue";
            let start = source.find(payload).unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + payload.len())
            }));
        }
    }

    #[test]
    fn invalid_style_recovers_once_and_keeps_later_unicode_facts() {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}A{newline}style A fill:red; 💥{newline}Later {{int 属性}}{newline}"
            );
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("ER must retain recovered snapshots");
            };
            let SemanticParseOutcome::Failed(merman_core::Error::DiagramParse {
                diagnostic, ..
            }) = snapshot.outcome()
            else {
                panic!("invalid style tail must fail");
            };
            let start = source.find('💥').unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + '💥'.len_utf8()))
            );
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("ER must keep later editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            for name in ["Later", "属性"] {
                let start = source.find(name).unwrap();
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }));
            }
        }
    }

    #[test]
    fn style_payload_ranges_preserve_unicode_tails_owned_by_text_tokens() {
        for tail in ["\u{00a0}", "\u{feff}", "\u{2028}"] {
            for trailing in ["", " "] {
                let payload = format!("fill: red{tail}");
                let source = format!(
                    "\u{feff}---\r\ntitle: 顧客\r\n---\r\nerDiagram\r\nA\r\nstyle A {payload}{trailing};\r\nLater\r\n"
                );
                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    SemanticParser::new().capture_diagram_snapshot_sync(&source)
                else {
                    panic!("valid style retains its snapshot");
                };
                assert!(matches!(
                    snapshot.outcome(),
                    SemanticParseOutcome::Parsed(_)
                ));
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("valid style retains its editor facts");
                };
                let start = source.find(&payload).unwrap();
                let expected = SourceSpan::new(start, start + payload.len());
                assert!(
                    facts.expected_syntax.iter().any(|syntax| {
                        syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == expected
                    }),
                    "{payload:?} {trailing:?}: {:?}",
                    facts.expected_syntax
                );
            }
        }
    }

    #[test]
    fn nonempty_style_payloads_retain_trailing_completion_slots_at_eof() {
        for command in ["style A", "classDef accent"] {
            for trailing in [" ", "\t", " \t"] {
                let source = format!("erDiagram\nA\n{command} fill:red{trailing}");
                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    SemanticParser::new().capture_diagram_snapshot_sync(&source)
                else {
                    panic!("style payload retains its snapshot");
                };
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("style payload retains editor facts");
                };
                assert!(
                    facts.expected_syntax.iter().any(|syntax| {
                        syntax.kind == EditorExpectedSyntaxKind::StyleValue
                            && syntax.span == SourceSpan::new(source.len(), source.len())
                    }),
                    "{source:?}: {:?}",
                    facts.expected_syntax
                );
            }
        }
    }
}
