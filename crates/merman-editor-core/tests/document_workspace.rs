use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{DocumentKind, DocumentUri, DocumentWorkspace, Position};

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(snapshot.uri.as_str(), "file:///tmp/example.mmd");
    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].body_start, 0);
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
fn markdown_documents_create_multiple_fence_local_snapshots() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.markdown",
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "```\n",
            "middle\n",
            "```mermaid\n",
            "sequenceDiagram\n",
            "Alice->>Bob: Hi\n",
            "```\n",
            "after\n",
        )
        .to_string(),
        DocumentKind::Markdown,
    );

    assert_eq!(snapshot.fences.len(), 2);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(snapshot.fences[1].diagram_type.as_deref(), Some("sequence"));
    assert_eq!(
        snapshot.fences[0].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert_eq!(
        snapshot.fences[1].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert!(
        snapshot.fences[1]
            .text_index
            .node_ids()
            .any(|id| id == "Alice")
    );
}

#[test]
fn cursor_lookup_distinguishes_prose_from_mermaid_fences() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.md",
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        DocumentKind::Markdown,
    );

    assert!(snapshot.fence_at_position(Position::new(0, 2)).is_none());
    let fence = snapshot
        .fence_at_position(Position::new(2, 4))
        .expect("cursor inside fence");
    assert_eq!(fence.diagram_type.as_deref(), Some("flowchart-v2"));
}

#[test]
fn replacing_document_version_drops_stale_fence_state() {
    let mut workspace = DocumentWorkspace::new();
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let first = workspace.upsert(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let second = workspace.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);

    let stored = workspace.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert_eq!(stored.fences.len(), 1);
    assert_eq!(stored.fences[0].diagram_type.as_deref(), Some("sequence"));
    assert!(!stored.text.contains("flowchart TD"));
}
