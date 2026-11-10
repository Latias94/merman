#![cfg(any(feature = "diagram-class", feature = "diagram-er"))]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

#[cfg(feature = "diagram-class")]
fn class(source: &str) -> merman_model::families::class::ClassDiagramModel {
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let SemanticDiagramModel::Class(model) = parsed.model else {
        panic!("expected Class model");
    };
    model
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_accessibility_uses_jison_prefixes_and_ecmascript_whitespace() {
    for newline in ["\n", "\r\n"] {
        for (padding, expected) in [
            ("\u{85}", "\u{85}Value\u{85}"),
            ("\u{feff}", "Value"),
            ("\u{a0}", "Value"),
        ] {
            let source = format!(
                "classDiagram{newline}accTitle: {padding}Value{padding}{newline}accDescr: {padding}Value{padding}{newline}class Later{newline}"
            );
            let model = class(&source);
            assert_eq!(model.acc_title.as_deref(), Some(expected));
            assert_eq!(model.acc_descr.as_deref(), Some(expected));
            assert!(model.classes.contains_key("Later"));
        }
        for directive in ["accTitle", "accDescr"] {
            for body in [
                format!("{directive}:{newline}Value"),
                format!("{directive}{newline}: Value"),
            ] {
                let model = class(&format!(
                    "classDiagram{newline}{body}{newline}class Later{newline}"
                ));
                let value = if directive == "accTitle" {
                    model.acc_title
                } else {
                    model.acc_descr
                };
                assert_eq!(value.as_deref(), Some("Value"));
            }
        }
        let model = class(&format!(
            "classDiagram{newline}accTitle junk: Value{newline}class Later{newline}"
        ));
        assert!(model.acc_title.is_none());
        assert_eq!(model.classes["accTitlejunk"].members[0].id, "Value");
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_descriptions_rely_on_the_shared_post_sanitization_normalizer() {
    for (padding, expected) in [
        ("\u{85}", "\u{85}First\n\u{85}Second\u{85}"),
        ("\u{feff}", "First\nSecond"),
    ] {
        let model = class(&format!(
            "classDiagram\naccDescr {{ {padding}First\n{padding}Second{padding} }}\nclass Later\n"
        ));
        assert_eq!(model.acc_descr.as_deref(), Some(expected));
    }
    let model = class("classDiagram\naccDescr {<!--gone-->\n\u{feff}Second}\nclass Later\n");
    assert_eq!(model.acc_descr.as_deref(), Some("\u{feff}Second"));
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_empty_blocks_do_not_manufacture_statements_or_merge_separators() {
    for newline in ["\n", "\r\n"] {
        for block in ["accDescr {}", "accDescr { \n }", "accDescr {\u{feff}}"] {
            let block = block.replace('\n', newline);
            for source in [
                format!("classDiagram{newline}{block}{newline}class Later{newline}"),
                format!(
                    "classDiagram{newline}class Before{newline}{block}{newline}class Later{newline}"
                ),
            ] {
                assert!(
                    SemanticParser::new().parse_diagram_sync(&source).is_err(),
                    "{source:?}"
                );
            }
            let source = format!("classDiagram{newline}{block} class Later{newline}");
            assert!(class(&source).classes.contains_key("Later"), "{source:?}");
            assert!(
                class(&format!(
                    "classDiagram{newline}class Later {block}{newline}"
                ))
                .classes
                .contains_key("LateraccDescr")
            );
        }
        let model = class(&format!(
            "classDiagram{newline}class A{newline} \u{feff}{newline}class B{newline}"
        ));
        assert!(model.classes.contains_key("A") && model.classes.contains_key("B"));
    }
    for source in ["classDiagram\n", "classDiagram\naccDescr {}"] {
        assert!(SemanticParser::new().parse_diagram_sync(source).is_err());
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_cardinality_labels_use_the_same_trim_set_as_member_values() {
    for (padding, one, two) in [
        ("\u{85}", "\u{85}1\u{85}", "\u{85}2\u{85}"),
        ("\u{feff}", "1", "2"),
    ] {
        let model = class(&format!(
            "classDiagram\nA \"{padding}1{padding}\" --> \"{padding}2{padding}\" B\n"
        ));
        assert_eq!(model.relations[0].relation_title_1.as_deref(), Some(one));
        assert_eq!(model.relations[0].relation_title_2.as_deref(), Some(two));
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_name_concatenation_stops_at_quoted_and_generic_terminals() {
    for statement in ["class `A` B", "class A~T~ B", "<<foo bar>> A"] {
        let source = format!("classDiagram\n{statement}\nclass Later\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{source:?}"
        );
        #[cfg(feature = "editor-facts")]
        {
            use merman_core::{ParsedEditorFacts, SemanticSnapshotCapture};
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("Class retains recovery facts");
            };
            assert!(snapshot.outcome().parsed_model().is_none());
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("requested Class facts");
            };
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        }
    }
    let model = class("classDiagram\nclass Foo `Bar~T~`\nFooBar --> Later\n");
    assert_eq!(model.classes["FooBar"].type_param, "T");
    #[cfg(feature = "editor-facts")]
    {
        use merman_core::{EditorRenamePolicy, ParsedEditorFacts, SemanticSnapshotCapture};
        let SemanticSnapshotCapture::Snapshot(snapshot) = SemanticParser::new()
            .capture_diagram_snapshot_sync("classDiagram\nclass Foo `Bar~T~`\nFooBar --> Later\n")
        else {
            panic!("Class snapshot");
        };
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Class facts");
        };
        assert!(
            facts.symbols.iter().any(|symbol| symbol.name == "FooBar"
                && symbol.rename_policy == EditorRenamePolicy::None)
        );
    }
}

#[cfg(all(feature = "diagram-class", feature = "editor-facts"))]
#[test]
fn class_annotation_recovery_restores_name_combination_at_the_next_line() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    let source = "\u{feff}---\r\ntitle: Original😀\r\n---\r\nclassDiagram\r\n<<foo\r\nclass La ter\r\nLater --> End\r\n";
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(source)
    else {
        panic!("Class recovery snapshot");
    };
    assert!(snapshot.outcome().parsed_model().is_none());
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Class recovery facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    let start = source.find("La ter").unwrap();
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"
        && symbol.selection == SourceSpan::new(start, start + "La ter".len())));
    assert!(!facts.symbols.iter().any(|symbol| symbol.name == "La"));
}

#[cfg(all(feature = "diagram-class", feature = "editor-facts"))]
#[test]
fn class_payload_and_concatenated_name_facts_keep_original_utf8_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    let source = "\u{feff}---\r\ntitle: 原稿😀\r\n---\r\nclassDiagram\r\naccTitle: \u{85}題名\u{85}\r\nclass 顧 客 ~T~\r\n顧客 --> Later\r\n";
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(source)
    else {
        panic!("Class retains a snapshot");
    };
    assert!(snapshot.outcome().parsed_model().is_some());
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Class retains editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    for (name, authored) in [("\u{85}題名\u{85}", "\u{85}題名\u{85}"), ("顧客", "顧 客")] {
        let start = source.find(authored).unwrap();
        assert!(
            facts.symbols.iter().any(|symbol| symbol.name == name
                && symbol.selection == SourceSpan::new(start, start + authored.len())),
            "{name}"
        );
    }
    let model = class(source);
    assert!(model.classes.contains_key("顧客"));
    assert_eq!(model.classes["顧客"].type_param, "T");
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_empty_description_blocks_leave_the_previous_assignment_and_following_entities() {
    for newline in ["\n", "\r\n"] {
        for body in ["", " ", "\u{feff}\u{a0}", "\n  "] {
            let body = body.replace('\n', newline);
            let source = format!(
                "erDiagram{newline}accDescr: Before{newline}accDescr {{{body}}}{newline}Later{newline}"
            );
            let parsed = SemanticParser::new().parse_diagram_sync(&source).unwrap();
            let SemanticDiagramModel::Er(model) = parsed.model else {
                panic!("expected ER model");
            };
            assert_eq!(model.acc_descr.as_deref(), Some("Before"));
            assert!(model.entities.contains_key("Later"));
        }
    }
    let parsed = SemanticParser::new()
        .parse_diagram_sync("erDiagram\naccDescr {<!--gone-->\n\u{feff}Second}\nLater\n")
        .unwrap();
    let SemanticDiagramModel::Er(model) = parsed.model else {
        panic!("expected ER model");
    };
    assert_eq!(model.acc_descr.as_deref(), Some("\u{feff}Second"));
}
