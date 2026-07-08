use super::text_scan::is_candidate_node_id;
use super::{
    ByteSpan, EditorSymbolKind, FenceCursorCompletionKind, FenceExpectedSyntaxKind,
    FenceSemanticRole, FenceTextIndex, FenceTextIndexSource, shape_object_value_prefix,
};
use merman_core::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, EditorSpanCoordinateSpace, SourceSpan,
};

#[test]
fn byte_span_contains_half_open_ranges_and_empty_insertions() {
    let span = ByteSpan { start: 0, end: 1 };
    assert!(span.contains(0));
    assert!(!span.contains(1));

    let empty_span = ByteSpan { start: 1, end: 1 };
    assert!(!empty_span.contains(0));
    assert!(empty_span.contains(1));
    assert!(!empty_span.contains(2));
}

#[test]
fn text_index_collects_node_ids() {
    let index = FenceTextIndex::from_text("flowchart TD\nA-->B\nB-->C\n", Some("flowchart-v2"));
    let ids = index.node_ids().cloned().collect::<Vec<_>>();

    assert_eq!(ids, vec!["A", "B", "C"]);
}

#[test]
fn text_index_treats_legacy_flowchart_as_module_facts() {
    let index = FenceTextIndex::from_text("flowchart TD\nA-->B\n", Some("flowchart"));
    let item = index
        .outline_items()
        .iter()
        .find(|item| item.name == "A")
        .expect("flowchart node outline item");

    assert_eq!(item.kind, EditorSymbolKind::Module);
}

#[test]
fn node_id_filter_skips_keywords_and_empty_tokens() {
    assert!(!is_candidate_node_id("flowchart"));
    assert!(!is_candidate_node_id("%comment"));
    assert!(is_candidate_node_id("node_1"));
}

#[test]
fn text_index_tracks_directive_prefixes() {
    let index = FenceTextIndex::from_text(
        "%%{init: {\"theme\": \"dark\"}}%%\nclassDef foo fill:#f00\n:::className\n",
        None,
    );

    assert!(index.has_directive_prefix("init"));
    assert!(index.has_directive_prefix("classDef"));
    assert!(index.has_directive_prefix(":::"));
}

#[test]
fn text_scan_records_payload_directive_prefixes_without_projecting_payload_symbols() {
    let index = FenceTextIndex::from_text(
        concat!(
            "flowchart TD\n",
            "click A href \"https://example.com\" \"Open user\" _blank\n",
            "linkStyle 0 stroke:#111,stroke-width:2px\n",
            "accTitle: Chart title\n",
            "accDescr: Chart description\n",
            "title Roadmap\n",
        ),
        Some("flowchart-v2"),
    );

    for prefix in ["click", "linkStyle", "accTitle", "accDescr", "title"] {
        assert!(index.has_directive_prefix(prefix));
    }
    for leaked in [
        "A", "href", "https", "example", "Open", "user", "_blank", "stroke", "Chart", "Roadmap",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "text-scan payload directive leaked {leaked:?} as a node id"
        );
    }
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "A" && item.detail.as_deref() == Some("interaction"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "0" && item.detail.as_deref() == Some("link style"))
    );
}

#[test]
fn text_scan_skips_class_directive_payload_prefixes() {
    let index = FenceTextIndex::from_text(
        concat!(
            "flowchart TD\n",
            "A-->B\n",
            "class User:::service\n",
            "style User fill:#fff\n",
            "click User href \"https://example.com\" \"Open user\" _blank\n",
            "classDef service fill:#eee\n",
            "cssClass A,B service\n",
            "link Alice: Endpoint @ https://alice.example.com\n",
            "callback Bob open(userId)\n",
            ":::service\n",
        ),
        Some("flowchart-v2"),
    );

    for prefix in ["classDef", "cssClass", "link", "callback", ":::"] {
        assert!(index.has_directive_prefix(prefix));
    }
    assert_eq!(
        index.node_ids().cloned().collect::<Vec<_>>(),
        vec!["A", "B"]
    );
    for leaked in [
        "service", "User", "Alice", "Endpoint", "https", "alice", "example", "com", "Bob", "open",
        "userId", "fill", "fff",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "class directive payload leaked {leaked:?} as a node id"
        );
    }
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "User" && item.detail.as_deref() == Some("class assignment"))
    );
    assert!(
        index.outline_items().iter().any(
            |item| item.name == "service" && item.detail.as_deref() == Some("class definition")
        )
    );
    assert_eq!(
        index.class_names().cloned().collect::<Vec<_>>(),
        vec!["service"]
    );
}

#[test]
fn text_scan_skips_sequence_directive_payload_prefixes() {
    let index = FenceTextIndex::from_text(
        concat!(
            "sequenceDiagram\n",
            "links a: { \"Repo\": \"https://repo.contoso.com/\" }\n",
            "properties a: { \"class\": \"internal-service-actor\", \"icon\": \"@clock\" }\n",
            "details Alice: {\"owner\": \"platform\"}\n",
        ),
        Some("sequence"),
    );

    for prefix in ["links", "properties", "details"] {
        assert!(index.has_directive_prefix(prefix));
    }
    assert!(index.node_ids().next().is_none());
    assert!(index.outline_items().is_empty());
}

#[test]
fn text_scan_classifies_gantt_section_without_leaking_payloads() {
    let index = FenceTextIndex::from_text(
        concat!(
            "gantt\n",
            "dateFormat YYYY-MM-DD\n",
            "axisFormat %Y-%m-%d\n",
            "tickInterval 1day\n",
            "includes 2026-01-09\n",
            "excludes weekends\n",
            "todayMarker off\n",
            "weekday monday\n",
            "weekend friday\n",
            "section Demo\n",
        ),
        Some("gantt"),
    );

    for prefix in [
        "dateFormat",
        "axisFormat",
        "tickInterval",
        "includes",
        "excludes",
        "todayMarker",
        "weekday",
        "weekend",
        "section",
    ] {
        assert!(index.has_directive_prefix(prefix));
    }
    for leaked in [
        "YYYY-MM-DD",
        "%Y-%m-%d",
        "1day",
        "2026-01-09",
        "weekends",
        "off",
        "monday",
        "friday",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "gantt directive payload leaked {leaked:?} as a node id"
        );
    }
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "Demo" && item.detail.as_deref() == Some("gantt section"))
    );
}

#[test]
fn text_scan_mindmap_keeps_labels_out_of_node_ids() {
    let index = FenceTextIndex::from_text(
        concat!(
            "mindmap\n",
            "root(Root Node)\n",
            " child1[Child 1]\n",
            " ::icon(bomb)\n",
            " :::hot\n",
            " %% comment about node ids\n",
            " child2\n",
        ),
        Some("mindmap"),
    );

    for required in ["root", "child1", "child2"] {
        assert!(
            index.node_ids().any(|id| id == required),
            "missing mindmap node id {required:?} from text-scan fallback"
        );
    }

    for leaked in [
        "Root", "Node", "Child", "1", ":", "bomb", "hot", "comment", "about", "ids",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "mindmap text-scan fallback leaked {leaked:?} as a node id"
        );
    }

    for required in ["root", "child1", "child2"] {
        assert!(
            index
                .outline_items()
                .iter()
                .any(|item| item.name == required),
            "missing mindmap outline item {required:?} from text-scan fallback"
        );
    }
}

#[test]
fn text_scan_skips_non_symbol_directive_prefixes() {
    let index = FenceTextIndex::from_text(
        concat!(
            "%%{initialize: {\"theme\": \"dark\"}}%%\n",
            "%%{wrap}%%\n",
            "flowchart TD\n",
            "A-->B\n",
        ),
        Some("flowchart-v2"),
    );

    assert!(index.has_directive_prefix("initialize"));
    assert!(index.has_directive_prefix("wrap"));
    assert_eq!(
        index.node_ids().cloned().collect::<Vec<_>>(),
        vec!["A", "B"]
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| matches!(item.name.as_str(), "initialize" | "wrap"))
    );
}

#[test]
fn text_scan_requires_directive_keyword_boundaries() {
    let index = FenceTextIndex::from_text(
        concat!(
            "flowchart TD\n",
            "clickableNode-->B\n",
            "classNode-->C\n",
            "styleNode-->D\n",
        ),
        Some("flowchart-v2"),
    );

    for required in ["clickableNode", "classNode", "styleNode"] {
        assert!(
            index.node_ids().any(|id| id == required),
            "missing node id {required:?} from text-scan fallback"
        );
        assert!(
            index
                .outline_items()
                .iter()
                .any(|item| item.name == required
                    && item.detail.as_deref() == Some("diagram element")),
            "missing diagram outline item {required:?} from text-scan fallback"
        );
    }
}

#[test]
fn text_scan_cursor_context_only_offers_source_start_headers() {
    let index = FenceTextIndex::from_text("flowchart TD\nA-->B\n", Some("flowchart-v2"));

    let header = index.cursor_context("flow", 4);
    assert_eq!(header.prefix(), "flow");
    assert_eq!(header.prefix_start(), 0);
    assert_eq!(header.source(), FenceTextIndexSource::TextScan);
    assert!(header.is_source_start());
    assert!(!header.has_parser_backed_facts());
    assert!(header.offers(FenceCursorCompletionKind::DiagramHeader));
    assert!(!header.offers(FenceCursorCompletionKind::NodeIdentifier));

    let kanban_header = index.cursor_context("kan", 3);
    assert!(kanban_header.offers(FenceCursorCompletionKind::DiagramHeader));
    assert!(!kanban_header.offers(FenceCursorCompletionKind::NodeIdentifier));

    let ambiguous = index.cursor_context("flowchart TD\nA-", "flowchart TD\nA-".len());
    assert!(!ambiguous.offers(FenceCursorCompletionKind::Operator));
    assert!(!ambiguous.offers(FenceCursorCompletionKind::NodeIdentifier));

    let operator = index.cursor_context("flowchart TD\nA-->B", "flowchart TD\nA--".len());
    assert!(!operator.offers(FenceCursorCompletionKind::Operator));
    assert!(!operator.offers(FenceCursorCompletionKind::NodeIdentifier));

    let directive = index.cursor_context("classDef foo fill:#f00", "classDef foo".len());
    assert_eq!(directive.directive_prefix(), Some("classDef"));
    assert!(directive.is_comment_or_directive_line());
    assert!(!directive.offers(FenceCursorCompletionKind::Directive));
    assert!(!directive.offers(FenceCursorCompletionKind::NodeIdentifier));

    for (source, prefix, expected_prefix) in [
        ("cssClass A,B service", "cssClass".len(), Some("cssClass")),
        (
            "link User href \"https://example.com\" \"Open user\" _blank",
            "link".len(),
            Some("link"),
        ),
        (
            "callback User open(userId)",
            "callback".len(),
            Some("callback"),
        ),
    ] {
        let context = index.cursor_context(source, prefix);
        assert_eq!(context.directive_prefix(), expected_prefix);
        assert!(context.is_comment_or_directive_line());
        assert!(!context.offers(FenceCursorCompletionKind::Directive));
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    let sequence_directive = index.cursor_context(
        "links a: { \"Repo\": \"https://repo.contoso.com/\" }",
        "links".len(),
    );
    assert_eq!(sequence_directive.directive_prefix(), Some("links"));
    assert!(sequence_directive.is_comment_or_directive_line());
    assert!(!sequence_directive.offers(FenceCursorCompletionKind::Directive));
    assert!(!sequence_directive.offers(FenceCursorCompletionKind::NodeIdentifier));

    let gantt_directive = index.cursor_context("section Demo", "section".len());
    assert_eq!(gantt_directive.directive_prefix(), Some("section"));
    assert!(gantt_directive.is_comment_or_directive_line());
    assert!(!gantt_directive.offers(FenceCursorCompletionKind::Directive));
    assert!(!gantt_directive.offers(FenceCursorCompletionKind::NodeIdentifier));

    let node = index.cursor_context("node_1", "node_1".len());
    assert!(!node.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn parser_backed_cursor_context_allows_prefix_limited_helpers() {
    let index = FenceTextIndex::from_core_facts(EditorSemanticFacts::new());

    let operator = index.cursor_context("flowchart TD\nA-->B", "flowchart TD\nA--".len());
    assert_eq!(operator.source(), FenceTextIndexSource::ParserComplete);
    assert!(operator.has_parser_backed_facts());
    assert!(operator.offers(FenceCursorCompletionKind::Operator));
    assert!(!operator.offers(FenceCursorCompletionKind::NodeIdentifier));

    let directive = index.cursor_context("classDef foo fill:#f00", "classDef foo".len());
    assert_eq!(directive.directive_prefix(), Some("classDef"));
    assert!(directive.offers(FenceCursorCompletionKind::Directive));
    assert!(!directive.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_uses_fence_local_offsets_and_parser_backed_shape_context() {
    let index = FenceTextIndex::from_core_facts(EditorSemanticFacts::new());
    let context = index.cursor_context("  A@{ shape: ", "  A@{ shape: ".len());

    assert_eq!(context.prefix(), "A@{ shape: ");
    assert_eq!(context.prefix_start(), 2);
    assert_eq!(context.cursor(), "  A@{ shape: ".len());
    assert!(context.offers(FenceCursorCompletionKind::Shape));
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_accepts_mermaid_shape_object_whitespace_variants() {
    let index = FenceTextIndex::from_core_facts(EditorSemanticFacts::new());

    for source in ["A@{shape: rou", "A@{       shape: rou", "A@{ shape : rou"] {
        let context = index.cursor_context(source, source.len());
        assert!(
            context.offers(FenceCursorCompletionKind::Shape),
            "expected shape completion for {source:?}"
        );
    }
}

#[test]
fn shape_object_value_prefix_reports_replacement_start() {
    let prefix = "A@{       shape : rou";
    let parsed = shape_object_value_prefix(prefix).expect("shape object prefix");

    assert_eq!(parsed.value_start, prefix.find("rou").unwrap());
    assert!(parsed.has_separator_space);
}

#[test]
fn shape_object_value_prefix_stops_after_shape_field_boundary() {
    for prefix in [
        "A@{ shape: rect, label: rou",
        "A@{ shape: rect, icon: \"rou",
        "A@{\n  shape: rect\n  label: rou",
    ] {
        assert!(
            shape_object_value_prefix(prefix).is_none(),
            "shape object completion must not cross into later fields: {prefix:?}"
        );
    }
}

#[test]
fn cursor_context_clamps_to_utf8_char_boundaries() {
    let text = "\u{8282}\u{70b9}";
    let index = FenceTextIndex::from_text(text, Some("flowchart-v2"));
    let context = index.cursor_context(text, 1);

    assert_eq!(context.cursor(), 0);
    assert_eq!(context.prefix(), "");
    assert!(context.offers(FenceCursorCompletionKind::DiagramHeader));
}

#[test]
fn cursor_context_uses_parser_expected_payload_to_suppress_generic_completion() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::new(
        "Alice",
        Some("sequence participant".to_string()),
        EditorSemanticKind::Event,
        SourceSpan::new(16, 21),
        SourceSpan::new(16, 21),
    ));
    facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
        merman_core::EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(28, 33),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context("sequenceDiagram\nAlice->Bob: Hello", 31);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::Payload)
    );
    assert!(context.completion_kinds().is_empty());
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    assert!(!context.offers(FenceCursorCompletionKind::DiagramHeader));
}

#[test]
fn cursor_context_uses_parser_expected_node_identifier_to_override_generic_completion() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::new(
        "A",
        Some("flowchart node".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(13, 14),
        SourceSpan::new(13, 14),
    ));
    facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
        merman_core::EditorExpectedSyntaxKind::NodeIdentifier,
        SourceSpan::new(17, 18),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context("flowchart TD\nA--> ", 17);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::NodeIdentifier)
    );
    assert_eq!(
        context.completion_kinds(),
        vec![FenceCursorCompletionKind::NodeIdentifier]
    );
    assert!(context.offers(FenceCursorCompletionKind::NodeIdentifier));
    assert!(!context.offers(FenceCursorCompletionKind::Operator));
}

#[test]
fn cursor_context_uses_parser_expected_shape_value_to_override_generic_completion() {
    let mut facts = EditorSemanticFacts::new();
    let text = "flowchart TD\nA@{\n  shape: rou\n}\n";
    let value_start = text.find("rou").unwrap();
    facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
        merman_core::EditorExpectedSyntaxKind::ShapeValue,
        SourceSpan::new(value_start, value_start + "rou".len()),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context(text, value_start + 2);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::Shape)
    );
    assert_eq!(
        context.completion_kinds(),
        vec![FenceCursorCompletionKind::Shape]
    );
    assert!(context.offers(FenceCursorCompletionKind::Shape));
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_uses_parser_expected_shape_trigger_to_override_generic_completion() {
    let mut facts = EditorSemanticFacts::new();
    let text = "flowchart TD\nA((\n";
    let trigger_start = text.find("((").unwrap();
    facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
        merman_core::EditorExpectedSyntaxKind::ShapeTrigger,
        SourceSpan::new(trigger_start, trigger_start + 2),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context(text, trigger_start + 2);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::ShapeTrigger)
    );
    assert_eq!(
        context.completion_kinds(),
        vec![FenceCursorCompletionKind::Shape]
    );
    assert!(context.offers(FenceCursorCompletionKind::Shape));
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_uses_parser_expected_direction_value_to_override_generic_completion() {
    let mut facts = EditorSemanticFacts::new();
    let text = "flowchart TD\nsubgraph group\ndirection LR\nend\n";
    let value_start = text.find("LR").unwrap();
    facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
        merman_core::EditorExpectedSyntaxKind::DirectionValue,
        SourceSpan::new(value_start, value_start + "LR".len()),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context(text, value_start + 1);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::Direction)
    );
    assert_eq!(
        context.completion_kinds(),
        vec![FenceCursorCompletionKind::Direction]
    );
    assert!(context.offers(FenceCursorCompletionKind::Direction));
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_uses_parser_expected_id_list_to_override_directive_completion() {
    let mut facts = EditorSemanticFacts::new();
    let text = "erDiagram\nclassDef pink fill:#f9f";
    let expected_start = text.find("pink").unwrap();
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::IdList,
        SourceSpan::new(expected_start, expected_start + "pink".len()),
    ));
    let index = FenceTextIndex::from_core_facts(facts);
    let context = index.cursor_context(text, expected_start);

    assert_eq!(
        context.expected_syntax(),
        Some(FenceExpectedSyntaxKind::IdList)
    );
    assert_eq!(
        context.completion_kinds(),
        vec![FenceCursorCompletionKind::NodeIdentifier]
    );
    assert!(context.offers(FenceCursorCompletionKind::NodeIdentifier));
    assert!(!context.offers(FenceCursorCompletionKind::Directive));
}

#[test]
fn text_index_projects_core_editor_facts() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_directive_prefix("classDef");
    facts.push_symbol(EditorSemanticSymbol::new(
        "A",
        Some("flowchart node".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(13, 14),
        SourceSpan::new(13, 14),
    ));

    let index = FenceTextIndex::from_core_facts(facts);

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "A"));
    assert_eq!(index.first_reference_span("A").unwrap().start, 13);
    assert_eq!(
        index.outline_items()[0].detail.as_deref(),
        Some("flowchart node")
    );
    assert!(index.has_directive_prefix("classDef"));
}

#[test]
fn text_index_marks_parser_coordinate_core_facts_without_position_indexes() {
    let mut facts = EditorSemanticFacts::new();
    facts.span_coordinate_space = EditorSpanCoordinateSpace::ParserInput;
    facts.push_directive_prefix("init");
    facts.push_symbol(EditorSemanticSymbol::new(
        "A",
        Some("flowchart node".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(3, 4),
        SourceSpan::new(3, 4),
    ));
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        SourceSpan::new(5, 5),
    ));

    let index = FenceTextIndex::from_core_facts(facts);

    assert_eq!(
        index.source(),
        FenceTextIndexSource::ParserCompleteDegradedSpans
    );
    assert!(index.source().is_parser_backed());
    assert!(!index.source().has_source_mapped_spans());
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.has_directive_prefix("init"));
    assert!(index.semantic_items().is_empty());
    assert!(index.outline_items().is_empty());
    assert!(index.expected_syntax().is_empty());
    assert_eq!(index.references().count(), 0);
}

#[test]
fn parser_backed_class_definitions_are_not_node_id_completions() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::new(
        "A",
        Some("flowchart node".to_string()),
        EditorSemanticKind::Module,
        SourceSpan::new(13, 14),
        SourceSpan::new(13, 14),
    ));
    facts.push_symbol(EditorSemanticSymbol::outline(
        "hot",
        Some("flowchart class definition".to_string()),
        EditorSemanticKind::Property,
        SourceSpan::new(24, 27),
        SourceSpan::new(24, 27),
    ));

    let index = FenceTextIndex::from_core_facts(facts);

    assert_eq!(index.node_ids().cloned().collect::<Vec<_>>(), vec!["A"]);
    assert_eq!(
        index.class_names().cloned().collect::<Vec<_>>(),
        vec!["hot"]
    );
}

#[test]
fn typed_reference_groups_separate_same_name_different_kinds() {
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

    let index = FenceTextIndex::from_core_facts(facts);
    let module_item = index
        .semantic_items()
        .iter()
        .find(|item| item.kind == EditorSymbolKind::Module)
        .unwrap();
    let property_item = index
        .semantic_items()
        .iter()
        .find(|item| item.kind == EditorSymbolKind::Property)
        .unwrap();

    assert_eq!(
        index.reference_spans_for_item(module_item),
        &[ByteSpan { start: 0, end: 6 }]
    );
    assert_eq!(
        index.reference_spans_for_item(property_item),
        &[ByteSpan { start: 7, end: 13 }]
    );
    assert_eq!(
        index.first_reference_span_for_item(module_item),
        Some(ByteSpan { start: 0, end: 6 })
    );
    assert_eq!(
        index.first_reference_span_for_item(property_item),
        Some(ByteSpan { start: 7, end: 13 })
    );
    assert_eq!(index.reference_spans("Shared").len(), 1);
}

#[test]
fn text_index_skips_payload_only_core_facts_for_completion() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(EditorSemanticSymbol::outline(
        "section",
        Some("gantt section".to_string()),
        EditorSemanticKind::Namespace,
        SourceSpan::new(0, 7),
        SourceSpan::new(0, 7),
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        "PK",
        Some("er attribute key".to_string()),
        EditorSemanticKind::Property,
        SourceSpan::new(8, 10),
        SourceSpan::new(8, 10),
    ));

    let index = FenceTextIndex::from_core_facts(facts);

    assert!(!index.node_ids().any(|id| id == "PK"));
    assert!(!index.node_ids().any(|id| id == "section"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "section" && item.role == FenceSemanticRole::Outline)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "PK" && item.role == FenceSemanticRole::Payload)
    );
    assert_eq!(
        index
            .semantic_item_at_offset(9)
            .map(|item| item.name.as_str()),
        Some("PK")
    );
    assert_eq!(index.entity_item_at_offset(9), None);
    assert_eq!(index.symbol_at_offset(9), None);
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "section")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "PK"));
}
