use merman_lsp::completion::{completion_for_snapshot, resolve_completion_item};
use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::{CompletionTextEdit, Documentation, MarkupKind, Position, Url};

#[test]
fn completion_offers_known_node_ids_for_plain_mermaid_documents() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\nB-->C\n".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(1, 1));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(edit.new_text, "B");
            assert_eq!(edit.range.start.line, 1);
            assert_eq!(edit.range.start.character, 0);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}

#[test]
fn completion_items_carry_resolve_data() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "direction".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "direction TB")
        .unwrap();
    let data = item.data.as_ref().expect("completion resolve data");

    assert_eq!(data["kind"], "direction");
    assert_eq!(data["label"], "direction TB");
}

#[test]
fn completion_resolve_adds_documentation_without_changing_insert_fields() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flow".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .into_iter()
        .find(|item| item.label == "flowchart TD")
        .unwrap();
    let original_text_edit = item.text_edit.clone();
    let original_insert_text = item.insert_text.clone();

    let resolved = resolve_completion_item(item);

    assert_eq!(resolved.text_edit, original_text_edit);
    assert_eq!(resolved.insert_text, original_insert_text);
    match resolved.documentation.as_ref().unwrap() {
        Documentation::MarkupContent(markup) => {
            assert_eq!(markup.kind, MarkupKind::Markdown);
            assert!(markup.value.contains("Starts a Mermaid"));
            assert!(markup.value.contains("flowchart TD"));
        }
        other => panic!("unexpected completion documentation: {other:?}"),
    }
}

#[test]
fn completion_offers_direction_keywords() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "direction".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 9));

    assert!(list.items.iter().any(|item| item.label == "direction TB"));
    assert!(list.items.iter().any(|item| item.label == "direction LR"));
}

#[test]
fn completion_does_not_offer_node_ids_for_directive_lines() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "%%{init: {\"theme\":\"dark\"}}%%\n".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 10));

    assert!(list.items.iter().all(|item| item.label != "flowchart TD"));
    assert!(
        list.items
            .iter()
            .all(|item| item.kind != Some(tower_lsp::lsp_types::CompletionItemKind::VARIABLE))
    );
}

#[test]
fn completion_offers_directive_items_for_directive_lines() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDef foo fill:#f00".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 12));

    assert!(list.items.iter().any(|item| item.label == ":::className"));
}

#[test]
fn completion_does_not_fallback_to_header_for_other_directive_lines() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "click User href \"https://example.com\" \"Open user\" _blank".to_string(),
    );
    let list = completion_for_snapshot(&snapshot, Position::new(0, 18));

    assert!(list.items.iter().all(|item| item.label != "flowchart TD"));
}

#[test]
fn completion_offers_node_ids_for_markdown_fences() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(uri, 1, "```mermaid\nflowchart TD\nA-->B\n```\n".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(2, 0));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));
}

#[test]
fn completion_uses_sequence_parser_payload_context() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nAlice->Bob: Hello".to_string(),
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 14));

    assert!(
        list.items.is_empty(),
        "sequence payload context must not offer generic identifiers or headers: {:?}",
        list.items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_uses_gantt_parser_payload_context() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "gantt\ntitle Roadmap".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(1, 10));

    assert!(
        list.items.is_empty(),
        "gantt payload context must not offer generic identifiers or headers: {:?}",
        list.items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_uses_flowchart_parser_payload_context() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flowchart TD\nA[\"Start node\"]-->B".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(1, 5));

    assert!(
        list.items.is_empty(),
        "flowchart payload context must not offer generic identifiers or headers: {:?}",
        list.items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_offers_shape_keywords() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "A@{ shape: ".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 11));

    assert!(
        list.items
            .iter()
            .any(|item| item.label == "@{ shape: circle }")
    );
    assert!(
        list.items
            .iter()
            .any(|item| item.label == "@{ shape: stadium }")
    );

    let item = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .unwrap();
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(edit.new_text, "circle }");
            assert_eq!(edit.range.start.character, 11);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}

#[test]
fn completion_offers_shape_keywords_for_classic_shapes() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    for (source, position) in [
        ("A((", Position::new(0, 3)),
        ("A{{", Position::new(0, 3)),
        ("A[", Position::new(0, 2)),
        ("A[/", Position::new(0, 3)),
        ("A[\\", Position::new(0, 3)),
        ("A>", Position::new(0, 2)),
    ] {
        let snapshot = store.upsert(uri.clone(), 1, source.to_string());
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items
                .iter()
                .any(|item| item.label == "@{ shape: circle }"),
            "missing shape completion for {source:?}"
        );
    }
}

#[test]
fn completion_offers_header_edit_ranges() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flow".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "flowchart TD")
        .unwrap();
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(edit.new_text, "flowchart TD");
            assert_eq!(edit.range.start.character, 0);
            assert_eq!(edit.range.end.character, 4);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}

#[test]
fn completion_offers_gantt_header() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "ga".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 2));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "gantt")
        .unwrap();
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(edit.new_text, "gantt");
            assert_eq!(edit.range.start.character, 0);
            assert_eq!(edit.range.end.character, 2);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}
