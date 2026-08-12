use super::{ByteSpan, FenceTextIndex, FenceTextIndexSource};
use merman_core::{
    EditorRenamePolicy, EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole,
    EditorSemanticSymbol, SourceSpan,
};

fn entity(
    name: impl Into<String>,
    kind: EditorSemanticKind,
    span: (usize, usize),
    selection: (usize, usize),
) -> EditorSemanticSymbol {
    EditorSemanticSymbol::new(
        name,
        None,
        kind,
        SourceSpan::new(span.0, span.1),
        SourceSpan::new(selection.0, selection.1),
    )
}

fn payload(
    name: impl Into<String>,
    kind: EditorSemanticKind,
    span: (usize, usize),
    selection: (usize, usize),
) -> EditorSemanticSymbol {
    EditorSemanticSymbol::payload(
        name,
        None,
        kind,
        SourceSpan::new(span.0, span.1),
        SourceSpan::new(selection.0, selection.1),
    )
}

fn outline_items(index: &FenceTextIndex) -> impl Iterator<Item = &EditorSemanticSymbol> {
    index
        .semantic_items()
        .iter()
        .filter(|item| item.role.contributes_outline())
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
    assert!(EditorRenamePolicy::Identifier.accepts("node-alpha_1"));
    assert!(!EditorRenamePolicy::Identifier.accepts("node alpha"));

    assert!(EditorRenamePolicy::QualifiedIdentifier.accepts("Sales.Order_1"));
    assert!(!EditorRenamePolicy::QualifiedIdentifier.accepts("1Sales.Order"));
    assert!(!EditorRenamePolicy::QualifiedIdentifier.accepts("Sales..Order"));
    assert!(EditorRenamePolicy::EventModelingId.accepts("Order_1"));
    assert!(!EditorRenamePolicy::EventModelingId.accepts("Sales.Order"));

    assert!(EditorRenamePolicy::EventModelingFrameId.accepts("007"));
    assert!(!EditorRenamePolicy::EventModelingFrameId.accepts("1000"));
    assert!(!EditorRenamePolicy::None.is_renameable());
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
        outline_items(&index)
            .next()
            .and_then(|item| item.detail.as_deref()),
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
    facts.push_symbol(EditorSemanticSymbol::class_definition(
        "hot",
        Some("display wording changed".to_string()),
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
        .find(|item| item.kind == EditorSemanticKind::Module)
        .unwrap();
    let property_item = index
        .semantic_items()
        .iter()
        .find(|item| item.kind == EditorSemanticKind::Property)
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
            .any(|item| item.name == "section" && item.role == EditorSemanticRole::Outline)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "PK" && item.role == EditorSemanticRole::Payload)
    );
    assert_eq!(
        index
            .semantic_item_at_offset(9)
            .map(|item| item.name.as_str()),
        Some("PK")
    );
    assert_eq!(index.entity_item_at_offset(9), None);
    assert_eq!(index.symbol_at_offset(9), None);
    assert!(outline_items(&index).any(|item| item.name == "section"));
    assert!(!outline_items(&index).any(|item| item.name == "PK"));
}

#[test]
fn indexed_semantic_lookup_matches_the_linear_oracle_at_every_boundary() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(entity("outer", EditorSemanticKind::Module, (0, 20), (0, 5)));
    facts.push_symbol(entity(
        "longer",
        EditorSemanticKind::Module,
        (3, 13),
        (5, 8),
    ));
    facts.push_symbol(entity(
        "selection-start-later",
        EditorSemanticKind::Module,
        (4, 8),
        (7, 8),
    ));
    facts.push_symbol(entity(
        "selection-end-later",
        EditorSemanticKind::Module,
        (5, 9),
        (6, 8),
    ));
    facts.push_symbol(payload(
        "alpha",
        EditorSemanticKind::Property,
        (5, 9),
        (6, 7),
    ));
    facts.push_symbol(entity("zeta", EditorSemanticKind::Property, (5, 9), (6, 7)));
    facts.push_symbol(payload(
        "empty",
        EditorSemanticKind::Property,
        (10, 10),
        (10, 10),
    ));
    facts.push_symbol(entity(
        "right-adjacent",
        EditorSemanticKind::Module,
        (10, 12),
        (10, 12),
    ));
    let index = FenceTextIndex::from_core_facts(facts);

    for offset in 0..=21 {
        let (indexed, _) = index.semantic_item_id_at_offset_indexed(offset);
        assert_eq!(
            indexed,
            index.semantic_item_id_at_offset_linear(offset),
            "semantic lookup diverged at byte offset {offset}"
        );
    }

    assert_eq!(
        index
            .semantic_item_at_offset(6)
            .map(|item| item.name.as_str()),
        Some("alpha"),
        "name must be the final explicit semantic tie-break"
    );
    assert_eq!(
        index
            .semantic_item_at_offset(10)
            .map(|item| item.name.as_str()),
        Some("empty"),
        "an exact empty span must beat an adjacent non-empty span"
    );
}

#[test]
fn indexed_semantic_lookup_preserves_full_ties_and_post_selection_entity_filtering() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(entity("outer", EditorSemanticKind::Module, (0, 12), (0, 5)));
    facts.push_symbol(payload(
        "inner",
        EditorSemanticKind::Property,
        (3, 7),
        (4, 5),
    ));
    facts.push_symbol(payload(
        "same",
        EditorSemanticKind::Property,
        (8, 10),
        (8, 9),
    ));
    facts.push_symbol(entity(
        "same",
        EditorSemanticKind::Property,
        (8, 10),
        (8, 9),
    ));
    let index = FenceTextIndex::from_core_facts(facts);

    assert_eq!(
        index
            .semantic_item_at_offset(4)
            .map(|item| item.name.as_str()),
        Some("inner")
    );
    assert_eq!(index.entity_item_at_offset(4), None);
    assert_eq!(
        index
            .entity_item_at_offset(1)
            .map(|item| item.name.as_str()),
        Some("outer")
    );
    assert_eq!(
        index.semantic_item_at_offset(8).map(|item| item.role),
        Some(EditorSemanticRole::Payload),
        "a complete tie must keep the first canonical item"
    );
    assert_eq!(index.entity_item_at_offset(8), None);
}

#[test]
fn indexed_reference_lookup_matches_btree_group_and_span_insertion_order() {
    let mut facts = EditorSemanticFacts::new();
    facts.push_symbol(entity("zeta", EditorSemanticKind::Module, (0, 12), (2, 7)));
    facts.push_symbol(entity(
        "alpha",
        EditorSemanticKind::Property,
        (0, 12),
        (4, 8),
    ));
    facts.push_symbol(entity(
        "alpha",
        EditorSemanticKind::Property,
        (0, 12),
        (4, 4),
    ));
    facts.push_symbol(entity(
        "alpha",
        EditorSemanticKind::Property,
        (0, 12),
        (8, 10),
    ));
    facts.push_symbol(entity(
        "alpha",
        EditorSemanticKind::Module,
        (0, 12),
        (10, 10),
    ));
    let index = FenceTextIndex::from_core_facts(facts);

    for offset in 0..=12 {
        let (indexed, _) = index.reference_at_offset_indexed(offset);
        assert_eq!(
            indexed,
            index.reference_at_offset_linear(offset),
            "reference lookup diverged at byte offset {offset}"
        );
    }

    assert_eq!(
        index.symbol_at_offset(4),
        Some(("alpha".to_string(), ByteSpan { start: 4, end: 8 })),
        "BTree group order must beat parser insertion order"
    );
    assert_eq!(
        index.symbol_at_offset(10),
        Some(("alpha".to_string(), ByteSpan { start: 10, end: 10 })),
        "empty reference spans match exactly at adjacent boundaries"
    );
}

#[test]
fn point_indexes_prune_non_overlapping_semantic_and_reference_intervals() {
    const ITEM_COUNT: usize = 4096;
    let mut facts = EditorSemanticFacts::new();
    for item_id in 0..ITEM_COUNT {
        let start = item_id * 3;
        facts.push_symbol(entity(
            format!("item-{item_id:04}"),
            EditorSemanticKind::Module,
            (start, start + 1),
            (start, start + 1),
        ));
    }
    let index = FenceTextIndex::from_core_facts(facts);
    let offset = (ITEM_COUNT - 1) * 3;

    let (semantic, semantic_visited) = index.semantic_item_id_at_offset_indexed(offset);
    let (reference, reference_visited) = index.reference_at_offset_indexed(offset);
    let balanced_height = usize::BITS as usize - ITEM_COUNT.leading_zeros() as usize;
    let visit_bound = balanced_height * 2 + 1;

    assert!(semantic.is_some());
    assert!(reference.is_some());
    assert!(
        semantic_visited <= visit_bound,
        "semantic query visited {semantic_visited} nodes; balanced-tree bound is {visit_bound}"
    );
    assert!(
        reference_visited <= visit_bound,
        "reference query visited {reference_visited} nodes; balanced-tree bound is {visit_bound}"
    );
}
