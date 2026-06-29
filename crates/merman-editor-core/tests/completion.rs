use merman_editor_core::{
    CompletionContext, CompletionDataKind, CompletionInsertTextFormat, DocumentKind,
    DocumentWorkspace, Position, completion_documentation, completion_for_snapshot,
};

#[test]
fn completion_offers_known_node_ids_with_text_edits() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nB-->C\n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(1, 1));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 0);
}

#[test]
fn completion_stays_fence_local_in_markdown_documents() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.markdown",
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "C-->\n",
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

    let flowchart_list = completion_for_snapshot(&snapshot, Position::new(4, 4));
    assert!(flowchart_list.items.iter().any(|item| item.label == "A"));
    assert!(flowchart_list.items.iter().any(|item| item.label == "B"));
    assert!(
        flowchart_list
            .items
            .iter()
            .all(|item| item.label != "Alice" && item.label != "Bob")
    );

    let sequence_list = completion_for_snapshot(&snapshot, Position::new(9, 14));
    assert!(sequence_list.items.is_empty());
}

#[test]
fn context_uses_parser_expected_syntax_for_shape_values() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA@{\n  shape: rou\n}\n".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(2, 11)).unwrap();
    let edit = context.shape_value_edit("circle").expect("shape edit");

    assert!(context.offer_shape_items());
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.replacement, "circle");
}

#[test]
fn completion_resolve_documentation_is_protocol_neutral() {
    let documentation = completion_documentation(&merman_editor_core::CompletionResolveData {
        kind: CompletionDataKind::DiagramHeader,
        label: "flowchart TD".to_string(),
    });

    assert!(documentation.contains("Starts a Mermaid"));
    assert!(documentation.contains("flowchart TD"));
}

#[test]
fn completion_offers_snippet_templates_at_diagram_start() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flow".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "flowchart template")
        .expect("flowchart template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert!(
        item.insert_text
            .as_ref()
            .unwrap()
            .contains("${1|TD,TB,BT,LR,RL|}")
    );
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Template
    );
}

#[test]
fn directive_helpers_use_snippet_placeholders() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "classDef ".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(0, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == ":::className")
        .expect("class helper completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(item.insert_text.as_deref(), Some(":::${1:className}"));
}
