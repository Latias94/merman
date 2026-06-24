use merman_analysis::FenceTextIndexSource;
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
            .text_index
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
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(snapshot.fences[0].text_index.has_directive_prefix("init"));
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

#[test]
fn incomplete_flowchart_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "C"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "group")
    );
}

#[test]
fn sequence_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nactor Bob\nAlice->>Bob: Hi\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn incomplete_sequence_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\nBob->>".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn state_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\n[*] --> Idle\nIdle --> Running\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn incomplete_state_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\nIdle --> Running\nRunning -->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn class_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "classDiagram\nclass User\nUser <|-- Admin\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(index.node_ids().any(|id| id == "Admin"));
}

#[test]
fn incomplete_class_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDiagram\nclass User\nUser <|--".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "User"));
}

#[test]
fn er_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn incomplete_er_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "erDiagram\nCUSTOMER ||--o{ ORDER :".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}
