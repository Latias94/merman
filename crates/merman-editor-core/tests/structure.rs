use merman_editor_core::{
    DocumentKind, DocumentWorkspace, Position, document_symbols, goto_definition, hover,
    prepare_rename, references, rename, workspace_symbols,
};

#[test]
fn document_symbols_include_root_and_child_items() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        DocumentKind::Diagram,
    );

    let symbols = document_symbols(&snapshot);

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "flowchart-v2 diagram");
    assert!(
        symbols[0]
            .children
            .iter()
            .any(|symbol| symbol.name == "group")
    );
}

#[test]
fn hover_reports_payload_semantic_items() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
        DocumentKind::Diagram,
    );

    let hover = hover(&snapshot, Position::new(1, 8)).unwrap();

    assert!(hover.contents.value.contains("Diagram Title"));
    assert!(hover.contents.value.contains("sequence title"));
}

#[test]
fn navigation_ignores_payload_spans_and_tracks_entities() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );

    let position = Position::new(1, 0);
    assert!(goto_definition(&snapshot, position).is_some());
    assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
    assert_eq!(
        prepare_rename(&snapshot, position).unwrap().placeholder,
        "A"
    );

    let edit = rename(&snapshot, position, "X").unwrap().unwrap();
    assert_eq!(edit.changes.get(&snapshot.uri).unwrap().len(), 2);
}

#[test]
fn workspace_symbols_filter_and_include_outline_items() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        DocumentKind::Diagram,
    );

    let all_symbols = workspace_symbols(&snapshot, "");
    assert!(all_symbols.iter().any(|symbol| symbol.name == "group"));
    assert!(all_symbols.iter().any(|symbol| symbol.name == "A"));

    let group_symbols = workspace_symbols(&snapshot, "group");
    assert_eq!(group_symbols.len(), 1);
    assert_eq!(group_symbols[0].name, "group");
}
