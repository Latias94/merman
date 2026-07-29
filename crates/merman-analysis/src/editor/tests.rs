use super::{
    ByteSpan, EditorSymbolKind, FenceCursorCompletionKind, FenceExpectedSyntaxKind,
    FenceRenamePolicy, FenceSemanticRole, FenceTextIndex, FenceTextIndexSource,
};
use merman_core::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, SourceSpan,
};

fn facts_expecting(kind: EditorExpectedSyntaxKind, span: SourceSpan) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    facts.push_expected_syntax(EditorExpectedSyntax::new(kind, span));
    facts
}

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
fn rename_policies_validate_family_owned_lexical_forms() {
    assert!(FenceRenamePolicy::Identifier.accepts("node-alpha_1"));
    assert!(!FenceRenamePolicy::Identifier.accepts("node alpha"));

    assert!(FenceRenamePolicy::QualifiedIdentifier.accepts("Sales.Order_1"));
    assert!(!FenceRenamePolicy::QualifiedIdentifier.accepts("1Sales.Order"));
    assert!(!FenceRenamePolicy::QualifiedIdentifier.accepts("Sales..Order"));
    assert!(FenceRenamePolicy::EventModelingId.accepts("Order_1"));
    assert!(!FenceRenamePolicy::EventModelingId.accepts("Sales.Order"));

    assert!(FenceRenamePolicy::EventModelingFrameId.accepts("007"));
    assert!(!FenceRenamePolicy::EventModelingFrameId.accepts("1000"));
    assert!(!FenceRenamePolicy::None.is_renameable());
}

#[test]
fn unavailable_index_offers_only_source_start_headers() {
    let index = FenceTextIndex::default();

    let header = index.cursor_context("flow", 4);
    assert_eq!(header.source(), FenceTextIndexSource::Unavailable);
    assert!(header.is_source_start());
    assert!(!header.has_parser_backed_facts());
    assert!(header.offers(FenceCursorCompletionKind::DiagramHeader));

    let body = "unknownDiagram\nA-->B";
    let body_context = index.cursor_context(body, body.len());
    assert!(!body_context.offers(FenceCursorCompletionKind::DiagramHeader));
    assert!(!body_context.offers(FenceCursorCompletionKind::NodeIdentifier));
    assert!(index.node_ids().next().is_none());
    assert!(index.outline_items().is_empty());
    assert!(index.semantic_items().is_empty());
}

#[test]
fn parser_backed_cursor_context_projects_expected_operator_and_directive_helpers() {
    let index = FenceTextIndex::from_core_facts(facts_expecting(
        EditorExpectedSyntaxKind::Operator,
        SourceSpan::new(14, 16),
    ));

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
    let source = "  A@{ shape: ";
    let index = FenceTextIndex::from_core_facts(facts_expecting(
        EditorExpectedSyntaxKind::ShapeValue,
        SourceSpan::new(source.len(), source.len()),
    ));
    let context = index.cursor_context(source, source.len());

    assert_eq!(context.prefix(), "A@{ shape: ");
    assert_eq!(context.prefix_start(), 2);
    assert_eq!(context.cursor(), "  A@{ shape: ".len());
    assert!(context.offers(FenceCursorCompletionKind::Shape));
    assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
}

#[test]
fn cursor_context_treats_lf_crlf_and_bare_cr_as_line_boundaries() {
    for line_ending in ["\n", "\r\n", "\r"] {
        let source = format!("flowchart TD{line_ending}  A@{{ shape: rou");
        let value_start = source.find("rou").unwrap();
        let index = FenceTextIndex::from_core_facts(facts_expecting(
            EditorExpectedSyntaxKind::ShapeValue,
            SourceSpan::new(value_start, source.len()),
        ));
        let context = index.cursor_context(&source, source.len());

        assert_eq!(context.prefix(), "A@{ shape: rou", "{line_ending:?}");
        assert_eq!(
            context.prefix_start(),
            "flowchart TD".len() + line_ending.len() + 2,
            "{line_ending:?}"
        );
        assert!(
            context.offers(FenceCursorCompletionKind::Shape),
            "{line_ending:?}"
        );
    }
}

#[test]
fn cursor_context_accepts_mermaid_shape_object_whitespace_variants() {
    for source in ["A@{shape: rou", "A@{       shape: rou", "A@{ shape : rou"] {
        let value_start = source.find("rou").unwrap();
        let index = FenceTextIndex::from_core_facts(facts_expecting(
            EditorExpectedSyntaxKind::ShapeValue,
            SourceSpan::new(value_start, source.len()),
        ));
        let context = index.cursor_context(source, source.len());
        assert!(
            context.offers(FenceCursorCompletionKind::Shape),
            "expected shape completion for {source:?}"
        );
    }
}

#[test]
fn cursor_context_clamps_to_utf8_char_boundaries() {
    let text = "\u{8282}\u{70b9}";
    let index = FenceTextIndex::default();
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
    let cloned = index.clone();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.shares_storage_with(&cloned));
    assert!(index.node_ids().any(|id| id == "A"));
    assert_eq!(index.first_reference_span("A").unwrap().start, 13);
    assert_eq!(
        index.outline_items()[0].detail.as_deref(),
        Some("flowchart node")
    );
    assert!(index.has_directive_prefix("classDef"));
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
