use merman_analysis::{FenceTextIndex, FenceTextIndexSource, SharedTextSlice, SourceMap};
use merman_core::{EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, SourceSpan};
use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentUri, DocumentWorkspace, FenceSnapshot, Position, Range,
    RenameError, document_symbols, folding_ranges, goto_definition, hover, prepare_rename,
    references, rename, selection_range, workspace_symbols,
};
use std::sync::Arc;

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
    assert_eq!(symbols[0].fact_source, FenceTextIndexSource::ParserComplete);
    assert!(
        symbols[0]
            .children
            .iter()
            .any(|symbol| symbol.name == "group")
    );
}

#[test]
fn hover_reports_the_active_outline_entry() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let hover = hover(&snapshot, Position::new(1, 0)).unwrap();

    assert!(hover.contents.value.contains("A"));
    assert!(hover.contents.value.contains("Diagram:"));
    assert_eq!(hover.fact_source, FenceTextIndexSource::ParserComplete);
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
    assert_eq!(hover.fact_source, FenceTextIndexSource::ParserComplete);
}

#[test]
fn hover_escapes_markdown_control_text_from_semantic_facts() {
    let text = "[link](https://example.invalid)\n".to_string();
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::new(
        "[link](https://example.invalid)",
        Some("![img](x) `detail`\nnext".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(0, 31),
        SourceSpan::new(0, 31),
    ));

    let text_index = FenceTextIndex::from_core_facts(facts);
    let shared_text = Arc::<str>::from(text.as_str());
    let snapshot = DocumentSnapshot {
        uri: DocumentUri::from("file:///tmp/example.mmd"),
        version: 1,
        kind: DocumentKind::Diagram,
        source: merman_analysis::SourceDescriptor::diagram().with_path("file:///tmp/example.mmd"),
        source_map: SourceMap::new(Arc::clone(&shared_text)),
        fences: vec![FenceSnapshot {
            source_id: "document".to_string(),
            index: 0,
            source: merman_analysis::SourceDescriptor::diagram()
                .with_path("file:///tmp/example.mmd"),
            start: 0,
            body_start: 0,
            body_end: text.len(),
            end: text.len(),
            text: SharedTextSlice::whole(Arc::clone(&shared_text)),
            fence_delimiter: None,
            diagram_type: Some("flowchart-v2".to_string()),
            text_index,
        }],
        text: shared_text,
    };

    let hover = hover(&snapshot, Position::new(0, 1)).unwrap();

    assert!(
        hover
            .contents
            .value
            .contains("\\[link\\]\\(https://example\\.invalid\\)")
    );
    assert!(hover.contents.value.contains("\\!\\[img\\]\\(x\\)"));
    assert!(hover.contents.value.contains("\\`detail\\` next"));
    assert!(
        !hover
            .contents
            .value
            .contains("[link](https://example.invalid)")
    );
    assert!(!hover.contents.value.contains("![img](x)"));
}

#[test]
fn payload_semantic_items_are_not_navigation_targets() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
        DocumentKind::Diagram,
    );

    let position = Position::new(1, 8);
    assert!(goto_definition(&snapshot, position).is_none());
    assert!(references(&snapshot, position, true).is_none());
    assert!(prepare_rename(&snapshot, position).is_none());
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
    let definition = goto_definition(&snapshot, position).unwrap();
    assert_eq!(definition.fact_source, FenceTextIndexSource::ParserComplete);
    let refs = references(&snapshot, position, true).unwrap();
    assert_eq!(refs.len(), 2);
    assert!(
        refs.iter()
            .all(|location| location.fact_source == FenceTextIndexSource::ParserComplete)
    );
    let prepare = prepare_rename(&snapshot, position).unwrap();
    assert_eq!(prepare.placeholder, "A");
    assert_eq!(prepare.fact_source, FenceTextIndexSource::ParserComplete);

    let edit = rename(&snapshot, position, "X").unwrap().unwrap();
    assert_eq!(edit.fact_source, FenceTextIndexSource::ParserComplete);
    assert_eq!(edit.changes.get(&snapshot.uri).unwrap().len(), 2);
}

#[test]
fn flowchart_rename_accepts_parser_legal_dotted_id() {
    let mut workspace = DocumentWorkspace::new();
    let flowchart = workspace.upsert(
        "file:///tmp/flowchart.mmd",
        1,
        "flowchart TD\nfoo.bar-->target\n".to_string(),
        DocumentKind::Diagram,
    );

    let edit = rename(&flowchart, Position::new(1, 0), "renamed.node")
        .expect("flowchart ids may contain dots")
        .expect("flowchart rename edit");
    let replacement = &edit.changes[&flowchart.uri][0].new_text;
    assert_eq!(replacement, "renamed.node");

    let renamed_text = flowchart.text.replacen("foo.bar", replacement, 1);
    let reparsed = workspace.upsert(
        flowchart.uri.clone(),
        2,
        renamed_text,
        DocumentKind::Diagram,
    );
    assert_eq!(
        reparsed.fences[0].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
}

#[test]
fn flowchart_rename_rejects_keyword_prefixed_dotted_ids() {
    let mut workspace = DocumentWorkspace::new();
    let flowchart = workspace.upsert(
        "file:///tmp/flowchart-keyword.mmd",
        1,
        "flowchart TD\nsource-->target\n".to_string(),
        DocumentKind::Diagram,
    );

    for candidate in ["end.foo", "graph.foo", "subgraph.foo"] {
        assert_eq!(
            rename(&flowchart, Position::new(1, 0), candidate),
            Err(RenameError::InvalidName),
            "{candidate} must follow the parser's keyword precedence"
        );
    }
}

#[test]
fn abnf_rename_rejects_parser_illegal_underscore() {
    let mut workspace = DocumentWorkspace::new();
    let abnf = workspace.upsert(
        "file:///tmp/grammar.mmd",
        1,
        "railroad-abnf-beta\nrule = \"a\"\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        rename(&abnf, Position::new(1, 0), "rule_name"),
        Err(RenameError::InvalidName),
        "ABNF rule names must not accept underscores"
    );
}

#[test]
fn git_graph_rename_accepts_reference_punctuation() {
    let mut workspace = DocumentWorkspace::new();
    let git_graph = workspace.upsert(
        "file:///tmp/git-graph.mmd",
        1,
        concat!(
            "gitGraph\n",
            "commit\n",
            "branch feature/base\n",
            "checkout feature/base\n",
        )
        .to_string(),
        DocumentKind::Diagram,
    );

    let edit = rename(&git_graph, Position::new(2, 8), "release/v1.2")
        .expect("gitGraph references may contain slashes and dots")
        .expect("gitGraph rename edit");
    let changes = &edit.changes[&git_graph.uri];
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .all(|change| change.new_text == "release/v1.2")
    );
}

#[test]
fn architecture_rename_rejects_reserved_identifier() {
    let mut workspace = DocumentWorkspace::new();
    let architecture = workspace.upsert(
        "file:///tmp/architecture.mmd",
        1,
        "architecture-beta\n  service server\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        rename(&architecture, Position::new(1, 10), "align"),
        Err(RenameError::InvalidName),
        "architecture reserved words must not be accepted as replacement ids"
    );
}

#[test]
fn shape_data_nodes_are_navigation_targets_but_edge_shape_data_is_not() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nD@{ shape: rounded }\nD --> E\nA e1@--> B\ne1@{ curve: basis }\n"
            .to_string(),
        DocumentKind::Diagram,
    );

    let symbols = document_symbols(&snapshot);
    assert!(symbols[0].children.iter().any(|symbol| symbol.name == "D"));

    let refs = references(&snapshot, Position::new(1, 0), true).unwrap();
    assert_eq!(refs.len(), 2);
    let prepare = prepare_rename(&snapshot, Position::new(1, 0)).unwrap();
    assert_eq!(prepare.placeholder, "D");

    assert!(prepare_rename(&snapshot, Position::new(4, 0)).is_none());
    assert!(references(&snapshot, Position::new(4, 0), true).is_none());
}

#[test]
fn mindmap_node_ids_are_renameable_and_payloads_are_not_navigation_targets() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "mindmap\nroot(Root Node)\n child1(Child 1)\n".to_string(),
        DocumentKind::Diagram,
    );

    let id_position = Position::new(1, 0);
    let prepare = prepare_rename(&snapshot, id_position).unwrap();
    assert_eq!(prepare.placeholder, "root");
    assert_eq!(prepare.fact_source, FenceTextIndexSource::ParserComplete);

    let refs = references(&snapshot, id_position, true).unwrap();
    assert_eq!(refs.len(), 1);

    let edit = rename(&snapshot, id_position, "root_alpha")
        .unwrap()
        .expect("expected rename edit");
    assert_eq!(
        edit.changes.get(&snapshot.uri).unwrap().len(),
        1,
        "rename should only update the mindmap node id"
    );

    let payload_position = Position::new(1, 5);
    assert!(goto_definition(&snapshot, payload_position).is_none());
    assert!(references(&snapshot, payload_position, true).is_none());
    assert!(prepare_rename(&snapshot, payload_position).is_none());
}

#[test]
fn typed_reference_groups_keep_same_name_different_kinds_separate() {
    let snapshot = typed_reference_snapshot();

    let module_refs = references(&snapshot, Position::new(0, 0), true).unwrap();
    let property_refs = references(&snapshot, Position::new(1, 0), true).unwrap();

    assert_eq!(module_refs.len(), 1);
    assert_eq!(property_refs.len(), 1);

    let module_rename = rename(&snapshot, Position::new(0, 0), "ModuleShared")
        .unwrap()
        .unwrap();
    assert_eq!(
        module_rename.changes.get(&snapshot.uri).unwrap().len(),
        1,
        "rename should only touch the module group"
    );
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
    assert_eq!(
        group_symbols[0].fact_source,
        FenceTextIndexSource::ParserComplete
    );

    let uppercase_symbols = workspace_symbols(&snapshot, "A");
    assert!(
        uppercase_symbols.iter().any(|symbol| symbol.name == "A"),
        "workspace symbol query should be case-insensitive for Mermaid identifiers"
    );
}

#[test]
fn selection_range_returns_parser_backed_symbol_chain() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.mmd",
        1,
        "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        DocumentKind::Diagram,
    );

    let selection = selection_range(&snapshot, Position::new(2, 0)).unwrap();
    let ranges = selection_chain_ranges(&selection);

    assert_eq!(selection.fact_source, FenceTextIndexSource::ParserComplete);
    assert_eq!(
        ranges[0],
        Range::new(Position::new(2, 0), Position::new(2, 1))
    );
    assert!(ranges.len() >= 2);
    assert_eq!(ranges.last().unwrap().start, Position::new(0, 0));
}

#[test]
fn selection_range_ignores_markdown_prose() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        "file:///tmp/example.md",
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        DocumentKind::Markdown,
    );

    assert!(selection_range(&snapshot, Position::new(0, 1)).is_none());
    assert!(selection_range(&snapshot, Position::new(3, 0)).is_some());
    assert!(selection_range(&snapshot, Position::new(5, 0)).is_none());
}

#[test]
fn folding_ranges_include_markdown_fences() {
    let mut workspace = DocumentWorkspace::new();
    let markdown = workspace.upsert(
        "file:///tmp/example.md",
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        DocumentKind::Markdown,
    );
    let markdown_ranges = folding_ranges(&markdown);

    assert!(markdown_ranges.iter().any(|range| {
        range.range.start.line == 1
            && range.range.end.line == 4
            && range.fact_source == FenceTextIndexSource::ParserComplete
    }));
}

fn typed_reference_snapshot() -> DocumentSnapshot {
    let text = "Shared\nShared\n".to_string();
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::new(
        "Shared",
        Some("module entity".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(0, 6),
        SourceSpan::new(0, 6),
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        "Shared",
        Some("property entity".to_string()),
        EditorSemanticKind::Property,
        SourceSpan::new(7, 13),
        SourceSpan::new(7, 13),
    ));

    let text_index = FenceTextIndex::from_core_facts(facts);
    let shared_text = Arc::<str>::from(text.as_str());
    DocumentSnapshot {
        uri: DocumentUri::from("file:///tmp/example.mmd"),
        version: 1,
        kind: DocumentKind::Diagram,
        source: merman_analysis::SourceDescriptor::diagram().with_path("file:///tmp/example.mmd"),
        source_map: SourceMap::new(Arc::clone(&shared_text)),
        fences: vec![FenceSnapshot {
            source_id: "document".to_string(),
            index: 0,
            source: merman_analysis::SourceDescriptor::diagram()
                .with_path("file:///tmp/example.mmd"),
            start: 0,
            body_start: 0,
            body_end: text.len(),
            end: text.len(),
            text: SharedTextSlice::whole(Arc::clone(&shared_text)),
            fence_delimiter: None,
            diagram_type: Some("flowchart-v2".to_string()),
            text_index,
        }],
        text: shared_text,
    }
}

fn selection_chain_ranges(selection: &merman_editor_core::EditorSelectionRange) -> Vec<Range> {
    let mut ranges = Vec::new();
    let mut current = Some(selection);
    while let Some(selection) = current {
        ranges.push(selection.range);
        current = selection.parent.as_deref();
    }
    ranges
}
