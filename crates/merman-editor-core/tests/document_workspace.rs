use merman_analysis::{AnalysisOptions, Analyzer, FenceMarker, FenceTextIndexSource, SourceKind};
use merman_editor_core::{DocumentKind, DocumentUri, DocumentWorkspace, Position};
use std::sync::Arc;

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
    assert_eq!(snapshot.source.kind, SourceKind::Diagram);
    assert_eq!(snapshot.fences[0].source_id, "document");
    assert_eq!(snapshot.fences[0].body_start, 0);
    assert_eq!(snapshot.fences[0].body_end, snapshot.text.len());
    assert_eq!(snapshot.fences[0].source.kind, SourceKind::Diagram);
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
fn cloned_snapshots_share_immutable_text_buffers() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let cloned = snapshot.clone();

    assert!(Arc::ptr_eq(&snapshot.text, &cloned.text));
    let snapshot_fence_text = snapshot.fences[0].text.source_arc();
    let cloned_fence_text = cloned.fences[0].text.source_arc();
    assert!(Arc::ptr_eq(&snapshot_fence_text, &cloned_fence_text));
    assert!(Arc::ptr_eq(&snapshot.text, &snapshot_fence_text));
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
    assert_eq!(snapshot.source.kind, SourceKind::Markdown);
    assert_eq!(snapshot.fences[0].source_id, "mermaid-fence-1");
    assert_eq!(snapshot.fences[1].source_id, "mermaid-fence-2");
    assert_eq!(snapshot.fences[0].source.diagram_index, Some(0));
    assert_eq!(snapshot.fences[1].source.diagram_index, Some(1));
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
fn markdown_documents_use_shared_fence_policy_for_tilde_fences() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mdx",
        1,
        "before\n~~~mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n~~~~\nafter\n".to_string(),
        DocumentKind::Mdx,
    );

    assert_eq!(snapshot.source.kind, SourceKind::Mdx);
    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].source_id, "mermaid-fence-1");
    assert_eq!(snapshot.fences[0].source.kind, SourceKind::Mdx);
    assert_eq!(
        snapshot.fences[0].fence_delimiter.unwrap().marker(),
        FenceMarker::Tilde
    );
    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("sequence"));
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
fn cursor_lookup_includes_unclosed_markdown_fence_at_eof() {
    let mut workspace = DocumentWorkspace::new();
    for (source, position) in [
        (
            "before\n```mermaid\nflowchart TD\nA-->",
            Position::new(3, 4),
        ),
        (
            "before\n```mermaid\nflowchart TD\nA-->\n",
            Position::new(4, 0),
        ),
    ] {
        let snapshot = workspace.upsert(
            "file:///tmp/example.md",
            1,
            source.to_string(),
            DocumentKind::Markdown,
        );

        let fence = snapshot
            .fence_at_position(position)
            .expect("EOF should remain inside unclosed Mermaid fence");
        assert_eq!(fence.diagram_type.as_deref(), Some("flowchart-v2"));
    }
}

#[test]
fn build_snapshot_does_not_cache_document() {
    let workspace = DocumentWorkspace::new();
    let uri = DocumentUri::new("file:///tmp/example.mmd");
    let snapshot = workspace.build_snapshot(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(snapshot.uri, uri);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(workspace.get(&uri).is_none());
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

#[test]
fn replacing_analyzer_drops_cached_snapshots() {
    let limited_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );
    let mut workspace = DocumentWorkspace::with_analyzer(limited_analyzer);
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let limited = workspace.upsert(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(limited.fences.is_empty());
    assert!(workspace.get(&uri).is_some());

    workspace.replace_analyzer(Analyzer::new());

    assert!(workspace.get(&uri).is_none());
    let rebuilt = workspace.upsert(
        uri,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    assert_eq!(
        rebuilt.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
}
