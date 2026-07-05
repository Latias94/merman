use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{
    CompletionContext, CompletionDataKind, CompletionInsertTextFormat, CompletionItemKind,
    DocumentKind, DocumentWorkspace, Position, completion_documentation, completion_for_snapshot,
};

#[test]
fn completion_offers_known_node_ids_with_text_edits() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nB-->C\nC-->".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(3, 4));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 4);
}

#[test]
fn completion_offers_node_ids_for_directive_targets() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nstyle \n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 6));

    let item = list.items.iter().find(|item| item.label == "A").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "A");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 6);
    assert!(list.items.iter().any(|item| item.label == "B"));
}

#[test]
fn completion_offers_class_names_for_class_references() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nclassDef hot fill:#f00\nclass A h\n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(3, 9));

    let item = list.items.iter().find(|item| item.label == "hot").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(item.kind, CompletionItemKind::Class);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::ClassName
    );
    assert_eq!(edit.new_text, "hot");
    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 8);
}

#[test]
fn completion_offers_style_snippets_after_style_targets() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nstyle A \n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 8));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "fill/stroke style")
        .unwrap();

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(item.data.as_ref().unwrap().kind, CompletionDataKind::Style);
    assert!(item.insert_text.as_ref().unwrap().contains("stroke-width"));
}

#[test]
fn completion_offers_interaction_snippets_after_click_targets() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nclick A \n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 8));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "href link action")
        .unwrap();

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Interaction
    );
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
fn completion_ignores_markdown_fence_delimiter_lines() {
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
            "after\n",
        )
        .to_string(),
        DocumentKind::Markdown,
    );

    assert!(CompletionContext::from_snapshot(&snapshot, Position::new(1, 3)).is_none());
    assert!(
        completion_for_snapshot(&snapshot, Position::new(1, 3))
            .items
            .is_empty()
    );
    assert!(CompletionContext::from_snapshot(&snapshot, Position::new(4, 0)).is_none());
    assert!(
        completion_for_snapshot(&snapshot, Position::new(4, 0))
            .items
            .is_empty()
    );
}

#[test]
fn completion_allows_unclosed_markdown_fence_body() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.markdown",
        1,
        concat!("```mermaid\n", "flowchart TD\n", "A-->\n").to_string(),
        DocumentKind::Markdown,
    );

    assert!(CompletionContext::from_snapshot(&snapshot, Position::new(2, 4)).is_some());
    assert!(
        completion_for_snapshot(&snapshot, Position::new(2, 4))
            .items
            .iter()
            .any(|item| item.label == "A")
    );
}

#[test]
fn completion_uses_parser_identifier_context_after_operator() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nC-->".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 4));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));
    assert!(
        list.items.iter().all(|item| item.label != "-->"),
        "parser identifier context must not offer operator completions: {:?}",
        list.items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_after_pipe_edge_label_inserts_after_the_label() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nA -->|go|".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 9));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.range.end, edit.range.start);
}

#[test]
fn completion_after_pipe_edge_label_replaces_trailing_whitespace_slot() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\nA -->|go|   ".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 12));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.range.end.line, 2);
    assert_eq!(edit.range.end.character, 12);
}

#[test]
fn completion_keeps_known_node_ids_when_parser_recovers() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(3, 4)).unwrap();
    let list = completion_for_snapshot(&snapshot, Position::new(3, 4));

    assert_eq!(context.fact_source(), FenceTextIndexSource::ParserRecovered);
    assert!(
        list.items.iter().any(|item| item.label == "A"),
        "recovered parser context should still offer existing node ids"
    );
    assert!(
        list.items.iter().any(|item| item.label == "B"),
        "recovered parser context should still offer existing node ids"
    );
}

#[test]
fn completion_payload_contexts_return_no_body_items() {
    for (source, position, label) in [
        (
            concat!("stateDiagram-v2\n", "state \"Small State\" as namedState\n"),
            Position::new(1, 8),
            "state",
        ),
        (
            "sequenceDiagram\nparticipant Alice\nAlice->Bob: Hello",
            Position::new(2, 14),
            "sequence",
        ),
        ("gantt\ntitle Roadmap", Position::new(1, 10), "gantt"),
        ("mindmap\nroot(Root Node)\n", Position::new(1, 8), "mindmap"),
        (
            "flowchart TD\nA[\"Start node\"]-->B",
            Position::new(1, 5),
            "flowchart",
        ),
        (
            "block\nA[\"Start node\"] --> B\n",
            Position::new(1, 5),
            "block",
        ),
    ] {
        let mut workspace = DocumentWorkspace::new();
        let snapshot = workspace.upsert(
            "file:///tmp/example.mmd",
            1,
            source.to_string(),
            DocumentKind::Diagram,
        );
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.is_empty(),
            "{label} payload context must not offer generic identifiers or headers: {:?}",
            list.items
                .iter()
                .map(|item| &item.label)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completion_bounds_text_scan_fallback_to_source_start() {
    let mut workspace = DocumentWorkspace::new();
    let source_start = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flow".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&source_start, Position::new(0, 4)).unwrap();
    let list = completion_for_snapshot(&source_start, Position::new(0, 4));

    assert_eq!(context.fact_source(), FenceTextIndexSource::TextScan);
    assert!(list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::DiagramHeader)
    }));
    assert!(list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::Template)
    }));

    let body = workspace.upsert(
        "file:///tmp/unknown.mmd",
        1,
        "unknownDiagram\nA-".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&body, Position::new(1, 2)).unwrap();
    let list = completion_for_snapshot(&body, Position::new(1, 2));

    assert_eq!(context.fact_source(), FenceTextIndexSource::TextScan);
    assert!(list.items.is_empty());
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
fn shape_value_completion_does_not_duplicate_existing_closing_brace() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA@{ shape: rou }\n".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(1, 14)).unwrap();
    let edit = context.shape_value_edit("circle").expect("shape edit");

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.replacement, "circle");
}

#[test]
fn shape_value_completion_appends_missing_brace_before_markdown_fence_close() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.markdown",
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A@{ shape: rou\n",
            "```\n",
            "after\n",
        )
        .to_string(),
        DocumentKind::Markdown,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(3, 14)).unwrap();
    let edit = context.shape_value_edit("circle").expect("shape edit");

    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 3);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.replacement, "circle }");
}

#[test]
fn shape_value_completion_ignores_host_document_tail_after_markdown_fence() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.markdown",
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A@{ shape: rou\n",
            "```\n",
            "host markdown } should not close the active shape\n",
        )
        .to_string(),
        DocumentKind::Markdown,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(3, 14)).unwrap();
    let edit = context.shape_value_edit("circle").expect("shape edit");

    assert_eq!(edit.replacement, "circle }");
}

#[test]
fn shape_value_completion_appends_missing_brace_before_next_diagram_statement() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA@{ shape: rou\nB --> C\n".to_string(),
        DocumentKind::Diagram,
    );
    let context = CompletionContext::from_snapshot(&snapshot, Position::new(1, 14)).unwrap();
    let edit = context.shape_value_edit("circle").expect("shape edit");

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.replacement, "circle }");
}

#[test]
fn completion_offers_parser_accepted_flowchart_shapes() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA@{\n  shape: rou\n}\n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 11));
    let labels = list
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(
        labels.contains(&"@{ shape: inv-trapezoid }"),
        "labels: {labels:?}"
    );
    assert!(
        labels.contains(&"@{ shape: notched-rectangle }"),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"@{ shape: inv_trapezoid }"),
        "labels: {labels:?}"
    );
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
fn completion_offers_icon_template_from_icon_prefix() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "icon".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "icon node template")
        .expect("icon node template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
}

#[test]
fn completion_offers_frontmatter_templates_at_document_start() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        String::new(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(0, 0));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "frontmatter config template")
        .expect("frontmatter template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert!(item.insert_text.as_ref().unwrap().contains("config:"));
}

#[test]
fn completion_offers_themecss_inside_frontmatter() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "---\nconfig:\n  theme\n---\nflowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(2, 7));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "themeCSS: |")
        .expect("themeCSS frontmatter completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Frontmatter
    );
}

#[test]
fn directive_helpers_use_snippet_placeholders() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nclassDef ".to_string(),
        DocumentKind::Diagram,
    );
    let list = completion_for_snapshot(&snapshot, Position::new(1, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == ":::className")
        .expect("class helper completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(item.insert_text.as_deref(), Some(":::${1:className}"));
}
