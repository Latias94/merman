mod support;

use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{
    DocumentKind, Position, Range, RenameError, document_symbols, folding_ranges, goto_definition,
    hover, prepare_rename, references, rename, search_document_symbols, selection_range,
};
use support::SnapshotHarness;

#[test]
fn document_symbols_include_root_and_child_items() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
fn unavailable_body_does_not_manufacture_structure_or_navigation() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/unknown.mmd",
            1,
            "unknownDiagram\nPretendNode --> OtherNode\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let position = Position::new(1, 2);

    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::Unavailable
    );
    assert!(document_symbols(&snapshot).is_empty());
    assert!(search_document_symbols(&snapshot, "").is_empty());
    assert!(hover(&snapshot, position).is_none());
    assert!(goto_definition(&snapshot, position).is_none());
    assert!(references(&snapshot, position, true).is_none());
    assert!(prepare_rename(&snapshot, position).is_none());
}

#[test]
fn hover_reports_the_active_outline_entry() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let hover = hover(&snapshot, Position::new(1, 0)).unwrap();

    assert!(hover.contents.value.contains("A"));
    assert!(hover.contents.value.contains("Diagram:"));
    assert_eq!(hover.fact_source, FenceTextIndexSource::ParserComplete);
}

#[test]
fn hover_reports_payload_semantic_items() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let hover = hover(&snapshot, Position::new(1, 8)).unwrap();

    assert!(hover.contents.value.contains("Diagram Title"));
    assert!(hover.contents.value.contains("sequence title"));
    assert_eq!(hover.fact_source, FenceTextIndexSource::ParserComplete);
}

#[test]
fn hover_escapes_markdown_control_text_from_parser_snapshot() {
    let harness = SnapshotHarness::new();
    let title = "[link](https://example.invalid) ![img](x) `detail`";
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            format!("sequenceDiagram\ntitle: {title}\nAlice->>Bob: Hello\n"),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let hover = hover(&snapshot, Position::new(1, "title: ".len() + 1)).unwrap();

    assert!(
        hover
            .contents
            .value
            .contains("\\[link\\]\\(https://example\\.invalid\\)")
    );
    assert!(hover.contents.value.contains("\\!\\[img\\]\\(x\\)"));
    assert!(hover.contents.value.contains("\\`detail\\`"));
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
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let position = Position::new(1, 8);
    assert!(goto_definition(&snapshot, position).is_none());
    assert!(references(&snapshot, position, true).is_none());
    assert!(prepare_rename(&snapshot, position).is_none());
}

#[test]
fn navigation_ignores_payload_spans_and_tracks_entities() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
    assert_eq!(edit.changes.get(snapshot.uri()).unwrap().len(), 2);
    assert!(matches!(
        rename(&snapshot, position, "not a flowchart id"),
        Err(merman_editor_core::RenameError::InvalidName)
    ));
}

#[test]
fn rename_obeys_eventmodeling_family_grammar_policies() {
    let harness = SnapshotHarness::new();
    let entities = harness
        .analyze(
            "file:///tmp/entities.mmd",
            1,
            "eventmodeling\nentity Sales.Order\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let entity_position = Position::new(1, "entity ".len());

    let entity_edit = rename(&entities, entity_position, "Sales.Invoice")
        .unwrap()
        .expect("qualified entity rename");
    assert_eq!(entity_edit.changes.get(entities.uri()).unwrap().len(), 1);
    assert!(matches!(
        rename(&entities, entity_position, "Sales-Order"),
        Err(merman_editor_core::RenameError::InvalidName)
    ));

    let frames = harness
        .analyze(
            "file:///tmp/frames.mmd",
            1,
            "eventmodeling\ntf 01 ui Shop.Cart\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let frame_position = Position::new(1, "tf ".len());

    assert!(rename(&frames, frame_position, "007").unwrap().is_some());
    assert!(matches!(
        rename(&frames, frame_position, "1000"),
        Err(merman_editor_core::RenameError::InvalidName)
    ));

    let grouped = harness
        .analyze(
            "file:///tmp/grouped.mmd",
            1,
            concat!(
                "eventmodeling\n",
                "entity ProductChanged\n",
                "tf 002 evt Changed\n",
                "gwt 002\n",
                "  given\n",
                "    evt ProductChanged\n",
                "  then\n",
                "    evt ProductChanged\n",
            )
            .to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let grouped_position = Position::new(1, "entity ".len());

    assert!(matches!(
        rename(&grouped, grouped_position, "Sales.Order"),
        Err(merman_editor_core::RenameError::InvalidName)
    ));
    let grouped_edit = rename(&grouped, grouped_position, "ProductUpdated")
        .unwrap()
        .expect("group rename satisfying every occurrence grammar");
    assert_eq!(grouped_edit.changes.get(grouped.uri()).unwrap().len(), 3);
}

#[test]
fn flowchart_rename_accepts_parser_legal_dotted_id() {
    let harness = SnapshotHarness::new();
    let flowchart = harness
        .analyze(
            "file:///tmp/flowchart.mmd",
            1,
            "flowchart TD\nfoo.bar-->target\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let edit = rename(&flowchart, Position::new(1, 0), "renamed.node")
        .expect("flowchart ids may contain dots")
        .expect("flowchart rename edit");
    let replacement = &edit.changes[flowchart.uri()][0].new_text;
    assert_eq!(replacement, "renamed.node");

    let renamed_text = flowchart.text().replacen("foo.bar", replacement, 1);
    let reparsed = harness
        .analyze(
            flowchart.uri().clone(),
            2,
            renamed_text,
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    assert_eq!(
        reparsed.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
}

#[test]
fn flowchart_rename_rejects_keyword_prefixed_dotted_ids() {
    let harness = SnapshotHarness::new();
    let flowchart = harness
        .analyze(
            "file:///tmp/flowchart-keyword.mmd",
            1,
            "flowchart TD\nsource-->target\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
    let harness = SnapshotHarness::new();
    let abnf = harness
        .analyze(
            "file:///tmp/grammar.mmd",
            1,
            "railroad-abnf-beta\nrule = \"a\";\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    assert_eq!(
        rename(&abnf, Position::new(1, 0), "rule_name"),
        Err(RenameError::InvalidName),
        "ABNF rule names must not accept underscores"
    );
}

#[test]
fn git_graph_rename_accepts_reference_punctuation() {
    let harness = SnapshotHarness::new();
    let git_graph = harness
        .analyze(
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
        )
        .expect("test source should be accepted");

    let edit = rename(&git_graph, Position::new(2, 8), "release/v1.2")
        .expect("gitGraph references may contain slashes and dots")
        .expect("gitGraph rename edit");
    let changes = &edit.changes[git_graph.uri()];
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .all(|change| change.new_text == "release/v1.2")
    );
}

#[test]
fn architecture_rename_rejects_reserved_identifier() {
    let harness = SnapshotHarness::new();
    let architecture = harness
        .analyze(
            "file:///tmp/architecture.mmd",
            1,
            "architecture-beta\n  service server\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    assert_eq!(
        rename(&architecture, Position::new(1, 10), "align"),
        Err(RenameError::InvalidName),
        "architecture reserved words must not be accepted as replacement ids"
    );
}

#[test]
fn shape_data_nodes_are_navigation_targets_but_edge_shape_data_is_not() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nD@{ shape: rounded }\nD --> E\nA e1@--> B\ne1@{ curve: basis }\n"
                .to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "mindmap\nroot(Root Node)\n child1(Child 1)\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
        edit.changes.get(snapshot.uri()).unwrap().len(),
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
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            concat!(
                "classDiagram\n",
                "namespace Shared {\n",
                "  class Member\n",
                "}\n",
                "Shared --> Other\n",
            )
            .to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let namespace_refs = references(&snapshot, Position::new(1, "namespace ".len()), true).unwrap();
    let class_refs = references(&snapshot, Position::new(4, 0), true).unwrap();

    assert_eq!(namespace_refs.len(), 1);
    assert_eq!(class_refs.len(), 1);

    let namespace_rename = rename(
        &snapshot,
        Position::new(1, "namespace ".len()),
        "NamespaceShared",
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        namespace_rename.changes.get(snapshot.uri()).unwrap().len(),
        1,
        "rename should only touch the namespace group"
    );
}

#[test]
fn document_symbol_search_filters_and_includes_outline_items() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let all_symbols = search_document_symbols(&snapshot, "");
    assert!(all_symbols.iter().any(|symbol| symbol.name == "group"));
    assert!(all_symbols.iter().any(|symbol| symbol.name == "A"));

    let group_symbols = search_document_symbols(&snapshot, "group");
    assert_eq!(group_symbols.len(), 1);
    assert_eq!(group_symbols[0].name, "group");
    assert_eq!(
        group_symbols[0].fact_source,
        FenceTextIndexSource::ParserComplete
    );

    let uppercase_symbols = search_document_symbols(&snapshot, "A");
    assert!(
        uppercase_symbols.iter().any(|symbol| symbol.name == "A"),
        "document symbol query should be case-insensitive for Mermaid identifiers"
    );
}

#[test]
fn selection_range_returns_parser_backed_symbol_chain() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

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
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.md",
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");

    assert!(selection_range(&snapshot, Position::new(0, 1)).is_none());
    assert!(selection_range(&snapshot, Position::new(3, 0)).is_some());
    assert!(selection_range(&snapshot, Position::new(5, 0)).is_none());
}

#[test]
fn folding_ranges_include_markdown_fences() {
    let harness = SnapshotHarness::new();
    let markdown = harness
        .analyze(
            "file:///tmp/example.md",
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");
    let markdown_ranges = folding_ranges(&markdown);

    assert!(markdown_ranges.iter().any(|range| {
        range.range.start.line == 1
            && range.range.end.line == 4
            && range.fact_source == FenceTextIndexSource::ParserComplete
    }));
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
