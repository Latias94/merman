#![cfg(feature = "diagram-sequence")]

use merman_core::SemanticParser;
use merman_model::{
    families::sequence::{LINETYPE_CENTRAL_CONNECTION_REVERSE, SequenceDiagramModel},
    semantic::SemanticDiagramModel,
};
use serde_json::json;

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
fn actor_continuations_reuse_identity_across_messages_notes_and_menus() {
    for newline in ["\n", "\r\n"] {
        for id in [
            "api/v2",
            "api\\v2",
            "api(v2)",
            "api)v2",
            "api/(v2)",
            "api-v2/worker",
            "顧客/窓口",
            "😀(worker)",
        ] {
            for declared in [false, true] {
                let declaration = if declared {
                    format!("participant {id} as Service\n")
                } else {
                    String::new()
                };
                let source = format!(
                    concat!(
                        "sequenceDiagram\n{declaration}",
                        "{id}->>Other: request\nOther-->>{id}: reply\n",
                        "Note left of {id}: note\nNote over {id},Other: shared\n",
                        "links {id}: {{\"Docs\":\"https://example.test/docs\"}}\n",
                        "properties {id}: {{\"class\":\"service\"}}\n",
                    ),
                    declaration = declaration,
                    id = id,
                )
                .replace('\n', newline);
                let model = parse_sequence(&source);
                assert_eq!(model.actor_order, [id, "Other"], "{source:?}");
                assert_eq!(model.actors.len(), 2);
                let actor = &model.actors[id];
                assert_eq!(actor.name, id);
                assert_eq!(actor.description, if declared { "Service" } else { id });
                assert_eq!(actor.links["Docs"], "https://example.test/docs");
                assert_eq!(actor.properties["class"], "service");
                assert_eq!(model.messages.len(), 4);
                for (index, from, to, text) in [
                    (0, id, "Other", "request"),
                    (1, "Other", id, "reply"),
                    (2, id, id, "note"),
                    (3, id, "Other", "shared"),
                ] {
                    let message = &model.messages[index];
                    assert_eq!(message.from.as_deref(), Some(from));
                    assert_eq!(message.to.as_deref(), Some(to));
                    assert_eq!(message.message_text(), text);
                    assert_eq!(message.central_connection, 0);
                }
                assert_eq!(model.notes.len(), 2);
                assert_eq!(model.notes[0].actor, json!(id));
                assert_eq!(model.notes[1].actor, json!([id, "Other"]));
            }
        }
    }
}

#[test]
fn parentheses_consumed_inside_actor_continuations_remain_literal() {
    for id in ["A(B)()", "A-()", "A/()", "A(B) ()"] {
        let source = format!("sequenceDiagram\n{id}->>C: request\nC->>{id}: reply\n");
        let model = parse_sequence(&source);
        assert_eq!(model.actor_order, [id, "C"], "{source:?}");
        assert_eq!(model.messages.len(), 2);
        assert_eq!(model.messages[0].from.as_deref(), Some(id));
        assert_eq!(model.messages[1].to.as_deref(), Some(id));
        assert!(
            model
                .messages
                .iter()
                .all(|message| message.central_connection == 0 && !message.activate)
        );
    }

    for marker_gap in ["", " ", "\t"] {
        let source = format!("sequenceDiagram\nA(){marker_gap}->>C: request\n");
        let model = parse_sequence(&source);
        assert_eq!(model.actor_order, ["A", "C"]);
        assert_eq!(model.messages.len(), 2);
        assert_eq!(
            model.messages[0].central_connection,
            LINETYPE_CENTRAL_CONNECTION_REVERSE,
        );
        assert_eq!(model.messages[0].from.as_deref(), Some("A"));
    }
}

#[test]
fn actor_continuations_keep_arrow_and_central_marker_boundaries() {
    for statement in [
        "A()B->>C: embedded central marker",
        "A->>/B: missing actor head",
        "A->>\\B: missing actor head",
        "A->>(B): missing actor head",
        "A->>B-: incomplete continuation",
        "A->>B--C: bare double dash",
    ] {
        let source = format!("sequenceDiagram\n{statement}\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{source:?}",
        );
    }

    let model = parse_sequence("sequenceDiagram\nA/B-xC: close\n");
    assert_eq!(model.actor_order, ["A/B", "C"]);
    assert_eq!(model.messages.len(), 1);
    assert_eq!(model.messages[0].message_type, 3);
    assert_eq!(model.messages[0].from.as_deref(), Some("A/B"));
}

#[cfg(feature = "editor-facts")]
#[test]
fn actor_continuation_editor_facts_keep_original_unicode_spans_and_menu_reference_roles() {
    use merman_core::{
        EditorRenamePolicy, EditorSemanticCompleteness, EditorSemanticKind, EditorSemanticRole,
        ParsedEditorFacts, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    let source = concat!(
        "\u{feff}---\r\ntitle: 原稿😀\r\n---\r\nsequenceDiagram\r\n",
        "actor 顧客/窓口😀 as Client\r\n",
        "participant 服务(数据库) as Store\r\n",
        "顧客/窓口😀->>服务(数据库): request\r\n",
        "服务(数据库)-->>顧客/窓口😀: reply\r\n",
        "Note over 顧客/窓口😀,服务(数据库): review\r\n",
        "link 顧客/窓口😀: nowrap: Docs @ https://example.test/help\r\n",
        "links 服务(数据库): wrap: {\"Docs\":\"https://example.test/docs\"}\r\n",
        "properties 顧客/窓口😀: wrap: {\"class\":\"client\"}\r\n",
    );
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(source)
    else {
        panic!("valid Sequence source must produce a semantic snapshot");
    };
    let SemanticDiagramModel::Sequence(model) = snapshot
        .outcome()
        .parsed_model()
        .expect("valid Sequence source must retain its canonical model")
    else {
        panic!("expected the canonical Sequence model");
    };
    assert_eq!(model.actor_order, ["顧客/窓口😀", "服务(数据库)"]);
    assert_eq!(
        model.actors["顧客/窓口😀"].links["Docs"],
        "https://example.test/help",
    );
    assert_eq!(
        model.actors["服务(数据库)"].links["Docs"],
        "https://example.test/docs",
    );
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Sequence source must retain requested editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);

    for (id, kind, count) in [
        ("顧客/窓口😀", EditorSemanticKind::Variable, 6),
        ("服务(数据库)", EditorSemanticKind::Event, 5),
    ] {
        let occurrences: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == id)
            .collect();
        let expected: Vec<_> = source
            .match_indices(id)
            .map(|(start, text)| SourceSpan::new(start, start + text.len()))
            .collect();
        assert_eq!(expected.len(), count, "{id}");
        assert_eq!(occurrences.len(), count, "{id}");
        for (index, (symbol, span)) in occurrences.iter().zip(expected).enumerate() {
            assert_eq!(symbol.selection, span, "{id} occurrence {index}");
            assert_eq!(symbol.span, span, "{id} occurrence {index}");
            assert_eq!(&source[span.start..span.end], id);
            assert_eq!(symbol.kind, kind);
            assert_eq!(
                symbol.role,
                if index == 0 {
                    EditorSemanticRole::Entity
                } else {
                    EditorSemanticRole::Reference
                },
            );
            assert_eq!(symbol.rename_policy, EditorRenamePolicy::Identifier);
            assert!(symbol.rename_policy.is_renameable());
            assert!(symbol.rename_policy.accepts("RenamedParticipant"));
        }
    }

    for payload in [
        "Client",
        "Store",
        "nowrap: Docs @ https://example.test/help",
        "wrap: {\"Docs\":\"https://example.test/docs\"}",
        "wrap: {\"class\":\"client\"}",
    ] {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == payload)
            .expect("display aliases and menu text must remain payloads");
        let start = source.find(payload).unwrap();
        assert_eq!(
            symbol.selection,
            SourceSpan::new(start, start + payload.len()),
        );
        assert_eq!(symbol.role, EditorSemanticRole::Payload);
        assert_eq!(symbol.rename_policy, EditorRenamePolicy::None);
        assert!(!symbol.rename_policy.is_renameable());
    }
}
