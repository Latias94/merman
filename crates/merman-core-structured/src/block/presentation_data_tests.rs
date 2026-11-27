use super::*;

#[test]
fn full_construction_keeps_empty_and_omitted_fields_without_json_selection() {
    let mut db = BlockDb::default();
    db.clear();
    let absent = db.insert_block(Block::new("absent".into()));
    let explicit = db.insert_block(Block {
        id: "explicit".into(),
        styles: Some(Vec::new()),
        directions: Some(Vec::new()),
        ..Default::default()
    });
    db.edges.push(Block {
        directions: Some(Vec::new()),
        ..Default::default()
    });
    let model = block_db_to_render_model(db, &OperationControl::new()).unwrap();
    assert!(matches!(
        model.projection_facts(),
        BlockProjectionFacts::Explicit { .. }
    ));
    assert_eq!(
        model.node_field_presence(absent).styles,
        BlockFieldPresence::Omitted
    );
    assert_eq!(
        model.node_field_presence(explicit).styles,
        BlockFieldPresence::Present
    );
    assert_eq!(
        model.node_field_presence(explicit).directions,
        BlockFieldPresence::Present
    );
    assert_eq!(model.edge_directions(0), Some([].as_slice()));
}

#[test]
fn semantic_construction_omits_presentation_data() {
    let control = OperationControl::new();
    let runtime = merman_model::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let parsed = parse_block_source_controlled(
        "block\nA:2\n",
        &MermaidConfig::default(),
        false,
        &runtime,
        &control,
    )
    .unwrap()
    .unwrap();
    let output = block_db_to_model(parsed.0, false, &control).unwrap();
    assert!(output.visuals.color_indices.is_empty());
}
