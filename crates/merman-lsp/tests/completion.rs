use merman_lsp::completion::completion_for_snapshot;
use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn completion_offers_known_node_ids_for_plain_mermaid_documents() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\nB-->C\n".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(1, 0));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));
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
fn completion_offers_node_ids_for_markdown_fences() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(uri, 1, "```mermaid\nflowchart TD\nA-->B\n```\n".to_string());
    let list = completion_for_snapshot(&snapshot, Position::new(2, 0));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));
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
