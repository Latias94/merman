#![cfg(feature = "diagram-state")]

use merman_core::SemanticParser;
#[cfg(feature = "editor-facts")]
use merman_core::{
    EditorSemanticCompleteness, EditorSemanticFacts, EditorSemanticRole, ParsedEditorFacts,
    SemanticSnapshotCapture,
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

#[test]
fn multiline_note_payload_preserves_internal_whitespace() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "{header}\nnote right of A\n  first  \n    indented\t \n\n      deepest\n  last  \nend note\nA --> B\n"
            )
            .replace('\n', newline);
            let expected = "first  \n    indented\t \n\n      deepest\n  last";
            let model = state_model(&source);
            let note = model.states["A"].note.as_ref().unwrap();
            assert_eq!(note.text, expected, "{source}");
            assert_eq!(note.position.as_deref(), Some("right of"));
            let note_node = model
                .nodes
                .iter()
                .find(|node| node.shape == "note")
                .unwrap();
            assert_eq!(
                note_node.label.as_ref().and_then(|label| label.as_str()),
                Some(expected),
                "{source}",
            );
            assert_eq!(model.edges.len(), 2, "{source}");
            assert!(model.edges.iter().any(|edge| {
                edge.start == "A" && edge.end == "B" && edge.classes == "transition"
            }));
        }
    }
}

#[test]
fn multiline_note_payload_trims_only_ecmascript_boundary_whitespace() {
    for (body, expected) in [
        ("\u{feff} First\n  Second \u{feff}", "First\n  Second"),
        (
            "\u{0085}First\n  Second\u{0085}",
            "\u{0085}First\n  Second\u{0085}",
        ),
    ] {
        let source = format!("stateDiagram-v2\nnote left of A\n{body}\nend note\n");
        let model = state_model(&source);
        let note = model.states["A"].note.as_ref().unwrap();
        assert_eq!(note.text, expected, "{source}");
        let note_node = model
            .nodes
            .iter()
            .find(|node| node.shape == "note")
            .unwrap();
        assert_eq!(
            note_node.label.as_ref().and_then(|label| label.as_str()),
            Some(expected),
            "{source}",
        );
    }
}

#[test]
fn quoted_alias_composites_keep_nested_block_scope_and_literal_ids() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "{header}\nstate \"Outer label\" as Outer   \n{{\nstate \"Inner label\" as Inner:literal {{\nLeaf\n}}\nPeer\n}}\nOuter --> Done\n"
            )
            .replace('\n', newline);
            let model = state_model(&source);
            assert_eq!(model.nodes.len(), 5, "{source}");
            for (id, label, parent, is_group) in [
                ("Outer", "Outer label", None, true),
                ("Inner:literal", "Inner label", Some("Outer"), true),
                ("Leaf", "Leaf", Some("Inner:literal"), false),
                ("Peer", "Peer", Some("Outer"), false),
                ("Done", "Done", None, false),
            ] {
                let node = model.nodes.iter().find(|node| node.id == id).unwrap();
                assert_eq!(node.parent_id.as_deref(), parent, "{source}");
                assert_eq!(node.is_group, is_group, "{source}");
                assert_eq!(
                    node.label.as_ref().and_then(|value| value.as_str()),
                    Some(label),
                    "{source}",
                );
            }
            let outer_doc = model.states["Outer"].doc.unwrap();
            let [StateStatement::State(inner), StateStatement::State(peer)] =
                model.document.get(outer_doc).unwrap()
            else {
                panic!("the outer composite retains its two authored states");
            };
            assert_eq!(inner.id, "Inner:literal");
            assert_eq!(peer.id, "Peer");
            let [StateStatement::State(leaf)] = model.document.get(inner.doc.unwrap()).unwrap()
            else {
                panic!("the inner alias owns the leaf document");
            };
            assert_eq!(leaf.id, "Leaf");
            assert_eq!(model.edges.len(), 1, "{source}");
            assert_eq!(model.edges[0].start, "Outer");
            assert_eq!(model.edges[0].end, "Done");
        }
    }
}

#[test]
fn quoted_alias_composites_keep_literal_ids_after_root_newline() {
    for id in ["Outer:literal", "Outer-id", "外部-状态:組"] {
        for gap in ["\n", "\n\t", "\n\t\n"] {
            let source = format!("stateDiagram-v2\nstate \"Label\" as {id}{gap}{{\nLeaf\n}}\n");
            let model = state_model(&source);
            assert_eq!(model.nodes.len(), 2, "{source}");
            let node = model.nodes.iter().find(|node| node.id == id).unwrap();
            assert!(node.is_group, "{source}");
            assert_eq!(
                node.label.as_ref().and_then(|label| label.as_str()),
                Some("Label"),
                "{source}",
            );
            let child = model.nodes.iter().find(|node| node.id == "Leaf").unwrap();
            assert_eq!(child.parent_id.as_deref(), Some(id), "{source}");
            assert!(model.states[id].doc.is_some(), "{source}");
        }
    }
}

#[test]
fn quoted_alias_blocks_reject_blank_lines_and_nested_newline_openings() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            for (enclosing, gap) in [
                (false, "\n\n"),
                (true, "\n"),
                (true, "\n\t"),
                (true, "\n  "),
                (true, "\n\n"),
            ] {
                let alias = format!("state \"Label\" as Alias{gap}{{\nLeaf\n}}\n");
                let source = if enclosing {
                    format!("{header}\nstate Outer {{\n{alias}}}\n")
                } else {
                    format!("{header}\n{alias}")
                }
                .replace('\n', newline);
                assert!(
                    SemanticParser::new().parse_diagram_sync(&source).is_err(),
                    "the selected State parser rejects this block boundary: {source}",
                );
            }
        }
    }
}

#[test]
fn quoted_plain_aliases_keep_following_statement_boundaries() {
    for (alias, descriptions) in [
        ("Alias", &["Label"][..]),
        ("Alias:detail:discarded", &["Label", "detail"][..]),
    ] {
        let source = format!("stateDiagram-v2\nstate \"Label\" as {alias}\nNext\nAlias --> Next\n");
        let model = state_model(&source);
        assert_eq!(model.nodes.len(), 2);
        assert!(model.nodes.iter().all(|node| !node.is_group));
        assert_eq!(model.states["Alias"].descriptions, descriptions);
        assert!(model.states["Alias"].doc.is_none());
        assert!(model.states["Next"].doc.is_none());
        let statements = model.document.get(model.document.root().unwrap()).unwrap();
        assert_eq!(statements.len(), 3);
        assert_eq!(model.edges.len(), 1);
        assert_eq!(model.edges[0].start, "Alias");
        assert_eq!(model.edges[0].end, "Next");
    }
}

#[cfg(feature = "editor-facts")]
fn assert_original_fact(
    facts: &EditorSemanticFacts,
    source: &str,
    name: &str,
    role: EditorSemanticRole,
    marker: &str,
    original_value: &str,
) {
    let start = source.find(marker).expect("authored marker");
    let selection = SourceSpan::new(start, start + original_value.len());
    let symbol = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == name && symbol.role == role && symbol.selection == selection)
        .unwrap_or_else(|| panic!("missing {role:?} fact for {name} at {selection:?}"));
    assert_eq!(
        source.get(symbol.selection.start..symbol.selection.end),
        Some(original_value),
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn alias_and_multiline_note_facts_select_original_crlf_unicode_bytes() {
    let source = concat!(
        "\u{feff}---\ntitle: 状態の記録 🦀\n---\n",
        "%% removed before parsing\nstateDiagram-v2\n",
        "state \"外层 🦀\" as Outer\n{\n",
        "Child_状态\nnote right of Child_状态\n",
        "  第一行 🦀\n    第二行 β\nend note\n}\n",
        "Outer --> Done: 完了\n",
    )
    .replace('\n', "\r\n");
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(&source)
    else {
        panic!("valid State source must retain a snapshot");
    };
    let SemanticDiagramModel::State(model) = snapshot.outcome().parsed_model().unwrap() else {
        panic!("expected the canonical State model");
    };
    assert!(model.states["Outer"].doc.is_some());
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("State capture must retain editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty());
    for (name, role, marker) in [
        ("Outer", EditorSemanticRole::Entity, "Outer\r\n{"),
        ("Outer", EditorSemanticRole::Reference, "Outer --> Done"),
        (
            "Child_状态",
            EditorSemanticRole::Entity,
            "Child_状态\r\nnote",
        ),
        (
            "Child_状态",
            EditorSemanticRole::Reference,
            "Child_状态\r\n  第一行",
        ),
    ] {
        assert_original_fact(facts, &source, name, role, marker, name);
    }
    let original_note = "第一行 🦀\r\n    第二行 β";
    assert_original_fact(
        facts,
        &source,
        "第一行 🦀\n    第二行 β",
        EditorSemanticRole::Payload,
        original_note,
        original_note,
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn rejected_alias_block_boundaries_retain_later_original_facts() {
    for (enclosing, gap) in [(false, "\n\n"), (true, "\n\t")] {
        let alias = format!("state \"失敗 🦀\" as Outer{gap}{{\nChild\n}}\n");
        let invalid = if enclosing {
            format!("state Container {{\n{alias}}}\n")
        } else {
            alias
        };
        let source = format!(
            "\u{feff}---\ntitle: 回復\n---\nstateDiagram-v2\n{invalid}Later_状态\nFinal_状態\nLater_状态 --> Final_状態: 続行 🦀\n"
        )
        .replace('\n', "\r\n");
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("rejected State source must retain a recovery snapshot");
        };
        assert!(snapshot.outcome().parsed_model().is_none());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("State rejection must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(!facts.diagnostics.is_empty());
        for (name, role, marker) in [
            ("Later_状态", EditorSemanticRole::Entity, "Later_状态\r\n"),
            ("Final_状態", EditorSemanticRole::Entity, "Final_状態\r\n"),
            (
                "Later_状态",
                EditorSemanticRole::Reference,
                "Later_状态 -->",
            ),
            ("Final_状態", EditorSemanticRole::Reference, "Final_状態:"),
            ("続行 🦀", EditorSemanticRole::Payload, "続行 🦀"),
        ] {
            assert_original_fact(facts, &source, name, role, marker, name);
        }
    }
}
