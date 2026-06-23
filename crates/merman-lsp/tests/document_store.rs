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

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert!(snapshot.fences[0].text.contains("flowchart TD"));
    assert!(snapshot.fences[0].completion.node_ids().any(|id| id == "A"));
}
