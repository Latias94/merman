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
