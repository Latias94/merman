#![cfg(feature = "diagram-state")]

use merman_core::SemanticParser;
#[cfg(feature = "editor-facts")]
use merman_core::{
    EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts, SemanticSnapshotCapture,
};
use merman_model::families::state::{StateDiagramModel, StateStatement};
use merman_model::semantic::SemanticDiagramModel;
#[cfg(feature = "editor-facts")]
use merman_model::source::SourceSpan;

fn state_model(source: &str) -> StateDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid State source");
    let SemanticDiagramModel::State(model) = parsed.model else {
        panic!("expected the canonical State model");
    };
    model
}

// Mermaid 12.3.0 stateDiagram.jison applies JS trim to DESCR/STATE_DESCR;
// stateDb.ts applies it again to descriptions and state/relation identifiers.
#[test]
fn descriptions_and_alias_labels_use_ecmascript_boundary_whitespace() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            for (boundary, expected) in [
                ("\u{85}", "\u{85}Title\u{85}"),
                ("\u{feff}", "Title"),
                ("\u{a0}", "Title"),
            ] {
                let source = format!(
                    "{header}\nA: {boundary}Title{boundary}\nstate \"{boundary}Title{boundary}\" as Alias\nstate \"Heading\" as Extra: {boundary}Title{boundary}\nA --> Alias: {boundary}Title{boundary}\n"
                )
                .replace('\n', newline);
                let model = state_model(&source);
                for id in ["A", "Alias"] {
                    assert_eq!(model.states[id].descriptions, [expected], "{source:?}");
                    let node = model.nodes.iter().find(|node| node.id == id).unwrap();
                    assert_eq!(
                        node.label.as_ref().and_then(|label| label.as_str()),
                        Some(expected),
                        "{source:?}",
                    );
                }
                assert_eq!(model.states["Extra"].descriptions, ["Heading", expected]);
                assert_eq!(model.relations[0].relation_title.as_deref(), Some(expected));
                assert_eq!(model.edges[0].label, expected);
            }
        }
    }
}

#[test]
fn alias_ids_and_all_reference_targets_keep_literal_nel() {
    for (raw_id, id) in [
        ("\u{85}状態\u{85}", "\u{85}状態\u{85}"),
        ("\u{85}", "\u{85}"),
        ("\u{feff}状態\u{feff}", "状態"),
    ] {
        let source = format!(
            "stateDiagram-v2\nstate \"Label\" as {raw_id}\n{raw_id} --> Done: Continue\nnote left of {raw_id}: Note\nclick {raw_id} href \"https://example.test/state\"\n"
        );
        let model = state_model(&source);
        assert_eq!(model.states.len(), 2, "{source:?}");
        assert_eq!(model.states[id].id, id);
        assert_eq!(model.states[id].descriptions, ["Label"]);
        assert_eq!(model.states[id].note.as_ref().unwrap().text, "Note");
        assert!(model.links.contains_key(id), "{source:?}");
        assert_eq!(model.relations[0].id1, id);
        assert!(model.edges.iter().any(|edge| {
            edge.classes == "transition" && edge.start == id && edge.end == "Done"
        }));
    }
}

#[test]
fn single_line_notes_and_style_payloads_use_ecmascript_trim() {
    for (boundary, expected) in [("\u{85}", "\u{85}"), ("\u{feff}", "")] {
        let source = format!(
            "stateDiagram-v2\nA\nnote left of A: {boundary}Note{boundary}\nclassDef accent {boundary}fill:red{boundary}\nclass A {boundary}accent{boundary}\nstyle A {boundary}stroke:blue{boundary}\n"
        );
        let model = state_model(&source);
        assert_eq!(
            model.states["A"].note.as_ref().unwrap().text,
            format!("{expected}Note{expected}"),
        );
        assert_eq!(
            model.states["A"].classes,
            [format!("{expected}accent{expected}")],
        );
        assert_eq!(
            model.states["A"].styles,
            [format!("{expected}stroke:blue{expected}")],
        );
        assert_eq!(
            model.style_classes["accent"].styles,
            [format!("{expected}fill:red{expected}")],
        );
    }
}

// The opening accDescr rule consumes JS whitespace; `}` emits no value token.
#[test]
fn empty_accessibility_blocks_do_not_assign_or_overwrite_descriptions() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            for body in ["", " \n\t ", "\u{feff}\u{a0}\n\t"] {
                for prior in [None, Some("Before")] {
                    let prefix = prior
                        .map(|value| format!("accDescr: {value}\n"))
                        .unwrap_or_default();
                    for gap in ["\n", " "] {
                        let source = format!(
                            "{header}\n{prefix}accDescr {{{body}}}{gap}Later --> Done: Continue\n"
                        )
                        .replace('\n', newline);
                        let model = state_model(&source);
                        assert_eq!(model.acc_descr.as_deref(), prior, "{source:?}");
                        assert_eq!(model.relations[0].id1, "Later");
                        assert_eq!(model.relations[0].id2, "Done");
                        let statements =
                            model.document.get(model.document.root().unwrap()).unwrap();
                        assert_eq!(
                            statements
                                .iter()
                                .filter(|statement| matches!(
                                    statement,
                                    StateStatement::AccDescr(_)
                                ))
                                .count(),
                            usize::from(prior.is_some()),
                            "an empty block must not produce an assignment: {source:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nel_accessibility_body_is_a_value_and_replaces_the_prior_description() {
    let model = state_model("stateDiagram-v2\naccDescr: Before\naccDescr { \u{85} }\nLater\n");
    assert_eq!(model.acc_descr.as_deref(), Some("\u{85}"));
    assert!(model.states.contains_key("Later"));
}

// commonDb.ts sanitizes before applying /\n\s+/g; a removed comment exposes blanks.
#[test]
fn accessibility_newline_folding_belongs_after_common_sanitization() {
    for newline in ["\n", "\r\n"] {
        for (body, expected) in [
            (" First\n \t\n\u{feff} Second ", "First\nSecond"),
            (
                "\u{85}First\n\u{85}Second\u{85}",
                "\u{85}First\n\u{85}Second\u{85}",
            ),
            ("First\n<!-- barrier --> \tSecond", "First\nSecond"),
            ("<!--gone-->\n\u{feff}Second", "\u{feff}Second"),
        ] {
            let source =
                format!("stateDiagram-v2\naccDescr {{{body}}}\nLater\n").replace('\n', newline);
            let model = state_model(&source);
            assert_eq!(model.acc_descr.as_deref(), Some(expected), "{source:?}");
        }
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn compatibility_and_render_paths_keep_state_whitespace_semantics() {
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use serde_json::json;

    let engine = MermaidEngine::new();
    for (boundary, id, label) in [
        ("\u{85}", "\u{85}状態\u{85}", "\u{85}Title\u{85}"),
        ("\u{feff}", "状態", "Title"),
    ] {
        let source = format!(
            "stateDiagram-v2\r\naccDescr {{<!--gone-->\r\n\u{feff}Second}}\r\naccDescr {{ \r\n\t}}\r\nstate \"{boundary}Title{boundary}\" as {boundary}状態{boundary}\r\n{boundary}状態{boundary} --> Done: {boundary}Title{boundary}\r\n"
        );
        let semantic = state_model(&source);
        assert_eq!(semantic.acc_descr.as_deref(), Some("\u{feff}Second"));
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .expect("valid State compatibility source")
            .expect("State compatibility model");
        let rendered = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .expect("valid State render source")
            .expect("State render model");
        assert_eq!(parsed.model, rendered.compatibility_json().unwrap());
        assert_eq!(parsed.model["accDescr"], json!("\u{feff}Second"));
        assert_eq!(parsed.model["states"][id]["descriptions"], json!([label]));
        assert_eq!(parsed.model["relations"][0]["id1"], json!(id));
        assert_eq!(parsed.model["relations"][0]["relationTitle"], json!(label));
        let RenderSemanticModel::State(rendered) = rendered.model() else {
            panic!("expected the typed State render model");
        };
        assert_eq!(rendered.acc_descr, semantic.acc_descr);
        assert_eq!(rendered.relations[0].id1, semantic.relations[0].id1);
        assert_eq!(
            rendered.states[id].descriptions,
            semantic.states[id].descriptions,
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn alias_reference_and_payload_facts_keep_raw_utf8_ranges_during_recovery() {
    for (boundary, id, leading) in [
        ("\u{85}", "\u{85}状態\u{85}", 0),
        ("\u{feff}", "状態", '\u{feff}'.len_utf8()),
    ] {
        for recovered in [false, true] {
            let invalid = if recovered {
                "state \"Invalid\" as Broken\n\n{\nChild\n}\n"
            } else {
                ""
            };
            let raw_id = format!("{boundary}状態{boundary}");
            let source = format!(
                "\u{feff}---\ntitle: 記録 🦀\n---\n%% removed before parsing\nstateDiagram-v2\n{invalid}state \"Alias label 🦀\" as {raw_id}: Detail\n{raw_id} --> Done: {boundary}続行 🦀{boundary}\nnote left of {raw_id}: {boundary}注釈 β{boundary}\nclick {raw_id} href \"https://example.test/state\"\n"
            )
            .replace('\n', "\r\n");
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("State source must retain a snapshot");
            };
            assert_eq!(snapshot.outcome().parsed_model().is_none(), recovered);
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("State source must retain editor facts");
            };
            assert_eq!(
                facts.completeness,
                if recovered {
                    EditorSemanticCompleteness::Recovered
                } else {
                    EditorSemanticCompleteness::Complete
                },
            );
            assert_eq!(facts.diagnostics.is_empty(), !recovered);
            let mut selections = Vec::new();
            for (index, (start, _)) in source.match_indices(raw_id.as_str()).enumerate() {
                let selection = SourceSpan::new(start + leading, start + leading + id.len());
                let role = if index == 0 {
                    EditorSemanticRole::Entity
                } else {
                    EditorSemanticRole::Reference
                };
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == id && symbol.role == role && symbol.selection == selection
                    })
                    .unwrap_or_else(|| panic!("missing {role:?} for {id:?} at {selection:?}"));
                assert_eq!(&source[selection.start..selection.end], id);
                assert!(symbol.rename_policy.accepts("Renamed_状態"));
                selections.push(selection);
            }
            assert_eq!(selections.len(), 4);
            assert_eq!(
                facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == id && symbol.rename_policy.is_renameable())
                    .count(),
                selections.len(),
            );
            for text in ["続行 🦀", "注釈 β"] {
                let expected = if boundary == "\u{85}" {
                    format!("{boundary}{text}{boundary}")
                } else {
                    text.to_string()
                };
                let start = source.find(expected.as_str()).unwrap();
                let selection = SourceSpan::new(start, start + expected.len());
                assert!(
                    facts.symbols.iter().any(|symbol| {
                        symbol.name == expected
                            && symbol.role == EditorSemanticRole::Payload
                            && symbol.selection == selection
                    }),
                    "missing original payload range for {expected:?}",
                );
            }

            if !recovered {
                let mut renamed = source.clone();
                for selection in selections.into_iter().rev() {
                    renamed.replace_range(selection.start..selection.end, "Renamed_状態");
                }
                let model = state_model(&renamed);
                assert!(!model.states.contains_key(id));
                assert_eq!(model.relations[0].id1, "Renamed_状態");
                assert!(model.links.contains_key("Renamed_状態"));
                assert!(model.states["Renamed_状態"].note.is_some());
            }
        }
    }
}
