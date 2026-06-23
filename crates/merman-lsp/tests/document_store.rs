use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::Url;

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\n".to_string());

    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].body_start, 0);
    assert_eq!(snapshot.fences[0].text, "flowchart TD\nA-->B\n");
}
