#![cfg(feature = "diagram-sequence")]

use merman_core::SemanticParser;
use merman_model::{families::sequence::SequenceDiagramModel, semantic::SemanticDiagramModel};

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
fn titles_preserve_text_after_one_whitespace_separator() {
    // sequenceDiagram.jison passes substring(6/7) to commonDb without trimming the title.
    for newline in ["\n", "\r\n"] {
        for (directive, separator, title) in [
            ("title", " ", "\u{0085}Title\u{0085}"),
            ("title:", "\t", "\u{feff}Title\u{feff}"),
            ("title", "\u{00a0}", "\u{00a0}Title\u{00a0}"),
            ("title:", "\u{feff}", "  Title \t"),
        ] {
            let source = format!(
                "sequenceDiagram{newline}{directive}{separator}{title}{newline}A->>B: message{newline}"
            );
            let model = parse_sequence(&source);
            assert_eq!(model.title.as_deref(), Some(title), "{source:?}");
            assert_eq!(model.messages[0].message_text(), "message");
        }
    }

    let model = parse_sequence(
        "sequenceDiagram\ntitle: First; title: Final # ignored title\nA->>B: message\n",
    );
    assert_eq!(model.title.as_deref(), Some("Final "));
}

#[test]
fn title_tokens_require_a_separator_and_nonempty_payload() {
    for source in [
        "sequenceDiagram\ntitle:\u{85}Title\n",
        "sequenceDiagram\ntitle:Title\n",
        "sequenceDiagram\ntitle:\t\n",
    ] {
        assert!(
            SemanticParser::new().parse_diagram_sync(source).is_err(),
            "{source:?}"
        );
    }
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "sequenceDiagram{newline}title:{newline}A->>B: title text{newline}C->>D: message{newline}"
        );
        let model = parse_sequence(&source);
        assert_eq!(model.title.as_deref(), Some("A->>B: title text"));
        assert_eq!(model.actor_order, ["C", "D"]);
        assert_eq!(model.messages.len(), 1);
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn cross_line_title_facts_select_the_second_line_in_original_source() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    let source = "\u{feff}---\r\ntitle: Original😀\r\n---\r\nsequenceDiagram\r\ntitle:\r\nTitle😀\r\nA->>B: message\r\n";
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(source)
    else {
        panic!("Sequence snapshot");
    };
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Sequence facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    let title = facts
        .symbols
        .iter()
        .find(|symbol| symbol.detail.as_deref() == Some("sequence title"))
        .unwrap();
    let start = source.find("Title😀").unwrap();
    assert_eq!(title.name, "Title😀");
    assert_eq!(
        title.selection,
        SourceSpan::new(start, start + "Title😀".len())
    );
}

#[test]
fn accessibility_values_trim_ecmascript_whitespace_without_losing_nel() {
    for newline in ["\n", "\r\n"] {
        for (padding, retained) in [("\u{0085}", "\u{0085}"), ("\u{feff}", ""), ("\u{00a0}", "")] {
            let source = format!(
                concat!(
                    "sequenceDiagram\naccTitle: {padding}Accessible{padding}\n",
                    "accDescr: {padding}Description{padding}\nA->>B: message\n",
                ),
                padding = padding,
            )
            .replace('\n', newline);
            let model = parse_sequence(&source);
            assert_eq!(
                model.acc_title.as_deref(),
                Some(format!("{retained}Accessible{retained}").as_str()),
                "{source:?}",
            );
            assert_eq!(
                model.acc_descr.as_deref(),
                Some(format!("{retained}Description{retained}").as_str()),
                "{source:?}",
            );

            let multiline = format!(
                "sequenceDiagram{newline}accDescr {{ {padding}Description{padding} }}{newline}A->>B: message{newline}"
            );
            let model = parse_sequence(&multiline);
            assert_eq!(
                model.acc_descr.as_deref(),
                Some(format!("{retained}Description{retained}").as_str()),
                "{multiline:?}",
            );
        }
    }

    let model = parse_sequence("sequenceDiagram\naccTitle: \u{0085}\naccDescr { \u{0085} }\n");
    assert_eq!(model.acc_title.as_deref(), Some("\u{0085}"));
    assert_eq!(model.acc_descr.as_deref(), Some("\u{0085}"));
}

#[test]
fn empty_accessibility_blocks_preserve_previous_values_and_following_statements() {
    for newline in ["\n", "\r\n"] {
        for body in ["", " \t ", "\n \n", "\u{feff}", "\u{00a0}"] {
            for separator in ["\n", " "] {
                let source = format!(
                    "sequenceDiagram\naccDescr: Previous description\naccDescr {{{body}}}{separator}A->>B: message\n"
                )
                .replace('\n', newline);
                let model = parse_sequence(&source);
                assert_eq!(
                    model.acc_descr.as_deref(),
                    Some("Previous description"),
                    "{source:?}",
                );
                assert_eq!(model.actor_order, ["A", "B"], "{source:?}");
                assert_eq!(model.messages.len(), 1, "{source:?}");
                assert_eq!(model.messages[0].message_text(), "message");
            }
        }
    }
    let model = parse_sequence("sequenceDiagram\naccDescr {} A->>B: message\n");
    assert!(model.acc_descr.is_none());
    assert_eq!(model.messages.len(), 1);

    let model = parse_sequence(
        "sequenceDiagram\naccDescr: Previous description\naccDescr {\u{0085}} A->>B: message\n",
    );
    assert_eq!(model.acc_descr.as_deref(), Some("\u{0085}"));
    assert_eq!(model.messages.len(), 1);
}

#[test]
fn accessibility_sanitization_precedes_newline_whitespace_collapsing() {
    for (body, expected) in [
        ("<!--gone-->\n\u{feff}Second", "\u{feff}Second"),
        ("First\n \u{feff}Second", "First\nSecond"),
        ("<!--gone-->\n\u{0085}Second", "\u{0085}Second"),
    ] {
        let source = format!("sequenceDiagram\naccDescr {{{body}}}\nA->>B: message\n");
        let model = parse_sequence(&source);
        assert_eq!(model.acc_descr.as_deref(), Some(expected), "{source:?}");
        assert_eq!(model.messages.len(), 1);
    }
}

#[test]
fn accessibility_prefixes_require_ecmascript_whitespace_and_allow_line_boundaries() {
    for directive in ["accTitle", "accDescr"] {
        for padding in [" invalid", "\u{0085}"] {
            let source = format!("sequenceDiagram\n{directive}{padding}: value\n");
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "{source:?}",
            );
        }

        for padding in ["\n\t", "\u{feff}"] {
            let source = format!(
                "sequenceDiagram\n{directive}{padding}:{padding}Value\u{0085}\nA->>B: message\n"
            );
            let model = parse_sequence(&source);
            let value = if directive == "accTitle" {
                model.acc_title.as_deref()
            } else {
                model.acc_descr.as_deref()
            };
            assert_eq!(value, Some("Value\u{0085}"), "{source:?}");
            assert_eq!(model.messages.len(), 1, "{source:?}");
        }

        let source = format!(
            "sequenceDiagram\n{directive}:\nA->>B: payload # literal; retained\nC->>D: actual message\n"
        );
        let model = parse_sequence(&source);
        let value = if directive == "accTitle" {
            model.acc_title.as_deref()
        } else {
            model.acc_descr.as_deref()
        };
        assert_eq!(value, Some("A->>B: payload # literal; retained"));
        assert_eq!(model.actor_order, ["C", "D"]);
        assert_eq!(model.messages.len(), 1);
        assert_eq!(model.messages[0].message_text(), "actual message");
    }
}

#[test]
fn box_color_and_title_boundaries_preserve_nel_and_trim_bom() {
    // SequenceDB.parseBoxData uses a case-sensitive regex before CSS.supports validates the color.
    for (header, name, fill) in [
        (
            "red \u{0085}Team\u{0085}",
            Some("\u{0085}Team\u{0085}"),
            "red",
        ),
        (
            "\u{0085}red Team\u{0085}",
            Some("\u{0085}red Team\u{0085}"),
            "transparent",
        ),
        ("red \u{0085}", Some("\u{0085}"), "red"),
        ("\u{0085}", Some("\u{0085}"), "transparent"),
        ("\u{feff}red Team\u{feff}", Some("Team"), "red"),
        ("red \u{feff}Team\u{feff}", Some("Team"), "red"),
        ("red", None, "red"),
        ("RED Team", Some("Team"), "RED"),
        ("rgb(1,2,3) Team", Some("Team"), "rgb(1,2,3)"),
        (
            "rgb(1,2,3) Team (service)",
            Some("rgb(1,2,3) Team (service)"),
            "transparent",
        ),
        (
            "rgb(1,2,3) Team (service) tail",
            Some("rgb(1,2,3) Team (service) tail"),
            "transparent",
        ),
        ("RGB(1,2,3) Team", Some("RGB(1,2,3) Team"), "transparent"),
        (
            "HSLA(10,20%,30%,.5) Team",
            Some("HSLA(10,20%,30%,.5) Team"),
            "transparent",
        ),
    ] {
        let source = format!("sequenceDiagram\nbox {header}\nparticipant A\nend\n");
        let model = parse_sequence(&source);
        assert_eq!(model.boxes.len(), 1, "{source:?}");
        let group = &model.boxes[0];
        assert_eq!(group.name.as_deref(), name, "{source:?}");
        assert_eq!(group.fill, fill, "{source:?}");
        assert_eq!(group.actor_keys, ["A"], "{source:?}");
    }
}

#[test]
fn box_wrap_modifiers_keep_ecmascript_text_boundaries() {
    for (padding, retained) in [("\u{0085}", "\u{0085}"), ("\u{feff}", "")] {
        for (prefix, wrap) in [("wrap:", true), (":nowrap:", false)] {
            let source = format!(
                "sequenceDiagram\nbox transparent {prefix}{padding}Team{padding}\nparticipant A\nend\n"
            );
            let model = parse_sequence(&source);
            assert_eq!(
                model.boxes[0].name.as_deref(),
                Some(format!("{retained}Team{retained}").as_str()),
                "{source:?}",
            );
            assert_eq!(model.boxes[0].wrap, wrap, "{source:?}");
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn box_editor_payloads_share_model_boundaries_in_original_utf8_source() {
    use merman_core::{
        EditorRenamePolicy, EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts,
        SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    let cases = [
        ("red \u{0085}Team😀\u{0085}", Some("\u{0085}Team😀\u{0085}")),
        ("red \u{feff}Team😀\u{feff}", Some("Team😀")),
        ("\u{feff}red Other😀\u{feff}", Some("Other😀")),
        (
            "\u{0085}red Other😀\u{0085}",
            Some("\u{0085}red Other😀\u{0085}"),
        ),
        ("red \u{0085}", Some("\u{0085}")),
        ("RGB(1,2,3) Team😀", Some("RGB(1,2,3) Team😀")),
        ("red", None),
    ];
    let mut source = String::from("\u{feff}---\r\ntitle: Original😀\r\n---\r\nsequenceDiagram\r\n");
    let mut selections = Vec::new();
    for (index, (header, name)) in cases.iter().enumerate() {
        source.push_str("box ");
        if let Some(name) = name {
            let start = source.len() + header.rfind(*name).unwrap();
            selections.push(SourceSpan::new(start, start + name.len()));
        }
        source.push_str(header);
        source.push_str(&format!("\r\nparticipant P{index}\r\nend\r\n"));
    }

    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(&source)
    else {
        panic!("valid Sequence source must produce a semantic snapshot");
    };
    let SemanticDiagramModel::Sequence(model) = snapshot.outcome().parsed_model().unwrap() else {
        panic!("expected the canonical Sequence model");
    };
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Sequence source must retain requested editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
    for (group, (_, name)) in model.boxes.iter().zip(cases) {
        assert_eq!(group.name.as_deref(), name);
    }
    let payloads: Vec<_> = facts
        .symbols
        .iter()
        .filter(|symbol| symbol.detail.as_deref() == Some("sequence box"))
        .collect();
    assert_eq!(payloads.len(), selections.len());
    for (symbol, selection) in payloads.iter().zip(selections) {
        assert_eq!(symbol.selection, selection);
        assert_eq!(symbol.name, &source[selection.start..selection.end]);
        assert_eq!(symbol.role, EditorSemanticRole::Payload);
        assert_eq!(symbol.rename_policy, EditorRenamePolicy::None);
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn common_text_and_participant_facts_keep_their_distinct_normalization_contracts() {
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    let source = concat!(
        "\u{feff}---\r\ntitle: Original😀\r\n---\r\nsequenceDiagram\r\n",
        "title:\t\u{feff} Diagram😀 \u{feff}\r\n",
        "accTitle\u{feff}\r\n:\u{feff}\r\n \u{0085}Accessible😀\u{0085}\r\n",
        "accDescr { \u{feff}\u{0085}Description😀\u{0085}\u{feff} }\r\n",
        "accDescr { \u{feff}\r\n\t\u{00a0} } ",
        "participant \u{0085}Api😀\u{0085} as \u{feff}Service😀\u{feff}\r\n",
        "\u{0085}Api😀\u{0085}->>B: \u{feff}message\u{feff}\r\n",
        "Note over \u{0085}Api😀\u{0085}: \u{0085}note\u{0085}\r\n",
        "link \u{0085}Api😀\u{0085}: Docs @ https://example.test/docs\r\n",
    );
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(source)
    else {
        panic!("valid Sequence source must produce a semantic snapshot");
    };
    let SemanticDiagramModel::Sequence(model) = snapshot.outcome().parsed_model().unwrap() else {
        panic!("expected the canonical Sequence model");
    };
    assert_eq!(model.title.as_deref(), Some("\u{feff} Diagram😀 \u{feff}"));
    assert_eq!(
        model.acc_title.as_deref(),
        Some("\u{0085}Accessible😀\u{0085}"),
    );
    assert_eq!(
        model.acc_descr.as_deref(),
        Some("\u{0085}Description😀\u{0085}"),
    );
    let id = "\u{0085}Api😀\u{0085}";
    assert_eq!(model.actor_order, [id, "B"]);
    assert_eq!(model.actors[id].description, "Service😀");
    assert_eq!(model.actors[id].links["Docs"], "https://example.test/docs");
    assert_eq!(model.messages[0].message_text(), "message");
    assert_eq!(model.notes[0].message, "\u{0085}note\u{0085}");
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Sequence source must retain requested editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
    assert_eq!(
        facts
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.detail.as_deref() == Some("sequence accessibility description")
            })
            .count(),
        1,
    );

    for (name, detail) in [
        ("\u{feff} Diagram😀 \u{feff}", "sequence title"),
        (
            "\u{0085}Accessible😀\u{0085}",
            "sequence accessibility title",
        ),
        (
            "\u{0085}Description😀\u{0085}",
            "sequence accessibility description",
        ),
        ("Service😀", "sequence participant label"),
    ] {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some(detail))
            .unwrap();
        let start = source.find(name).unwrap();
        assert_eq!(symbol.name, name);
        assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
        assert_eq!(symbol.role, EditorSemanticRole::Payload);
    }

    let participants: Vec<_> = facts
        .symbols
        .iter()
        .filter(|symbol| symbol.name == id)
        .collect();
    let spans: Vec<_> = source
        .match_indices(id)
        .map(|(start, text)| SourceSpan::new(start, start + text.len()))
        .collect();
    assert_eq!(participants.len(), 4);
    for (index, (symbol, span)) in participants.iter().zip(spans).enumerate() {
        assert_eq!(symbol.selection, span);
        assert_eq!(symbol.span, span);
        assert_eq!(
            symbol.role,
            if index == 0 {
                EditorSemanticRole::Entity
            } else {
                EditorSemanticRole::Reference
            },
        );
    }
}
