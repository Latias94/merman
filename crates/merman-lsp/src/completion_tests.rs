use crate::completion::{completion_for_snapshot, resolve_completion_item};
use crate::document_store::DocumentStore;
use tower_lsp::lsp_types::{
    CompletionTextEdit, Documentation, InsertTextFormat, MarkupKind, Position, Url,
};

#[test]
fn completion_items_carry_resolve_data() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flow".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "flowchart TD")
        .unwrap();
    let data = item.data.as_ref().expect("completion resolve data");

    assert_eq!(data["kind"], "diagram_header");
    assert_eq!(data["label"], "flowchart TD");
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
fn completion_projects_header_edit_ranges() {
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
fn completion_projects_core_snippet_items_to_lsp_snippets() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flow".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "flowchart template")
        .expect("flowchart template completion");

    assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    assert_eq!(
        item.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::SNIPPET)
    );
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert!(edit.new_text.contains("${1|TD,TB,BT,LR,RL|}"));
            assert_eq!(edit.range.start.character, 0);
            assert_eq!(edit.range.end.character, 4);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}

#[test]
fn completion_projects_class_name_items_to_lsp_classes() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nA-->B\nclassDef hot fill:#f00\nclass A h\n".to_string(),
    );
    let list = completion_for_snapshot(&snapshot, Position::new(3, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "hot")
        .expect("class name completion");

    assert_eq!(
        item.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::CLASS)
    );
    match item.text_edit.as_ref().unwrap() {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(edit.new_text, "hot");
            assert_eq!(edit.range.start.character, 8);
            assert_eq!(edit.range.end.character, 9);
        }
        other => panic!("unexpected text edit: {other:?}"),
    }
}
