use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::Url;

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].body_start, 0);
    assert_eq!(
        snapshot.fences[0].text,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n"
    );
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(
        snapshot.fences[0]
            .completion
            .has_directive_prefix("classDef")
    );
}

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\n%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B\n```\nafter\n"
            .to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert!(snapshot.fences[0].text.contains("flowchart TD"));
    assert!(snapshot.fences[0].completion.node_ids().any(|id| id == "A"));
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(snapshot.fences[0].completion.has_directive_prefix("init"));
}

#[test]
fn newer_versions_replace_the_stored_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    let first = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    let second = store.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(!stored.text.contains("flowchart TD"));
    assert_eq!(stored.fences.len(), 1);
    assert_eq!(stored.fences[0].diagram_type.as_deref(), Some("sequence"));
}
