use super::*;

#[test]
fn canonical_counts_round_trip_nonfinite_values_without_erasing_presence() {
    for value in [
        0.0,
        -0.0,
        2.0,
        9_007_199_254_740_992.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let node = BlockNode {
            columns: Some(value),
            width_in_columns: Some(value),
            width: Some(value),
            ..Default::default()
        };
        let json = serde_json::to_value(&node).unwrap();
        if value == 2.0 {
            assert_eq!(json["columns"], serde_json::json!(2));
        }
        let decoded: BlockNode = serde_json::from_value(json).unwrap();
        for number in [decoded.columns, decoded.width_in_columns, decoded.width] {
            let number = number.expect("a present count cannot become a default");
            assert!(number.to_bits() == value.to_bits() || (number.is_nan() && value.is_nan()));
        }
    }
    for value in [
        serde_json::json!("infinity"),
        serde_json::json!("2"),
        serde_json::json!({"value": 2}),
    ] {
        let mut json = serde_json::to_value(BlockNode::default()).unwrap();
        json["columns"] = value;
        assert!(serde_json::from_value::<BlockNode>(json).is_err());
    }
}

#[test]
fn manual_field_presence_uses_model_values_and_explicit_empty_facts_survive() {
    let semantic = BlockDiagramModel {
        blocks_flat: vec![BlockNode {
            id: "root".into(),
            styles: vec!["fill:red".into()],
            ..Default::default()
        }],
        root: Some(0),
        ..Default::default()
    };
    let manual = BlockDiagramRenderModel::from_semantic_model(semantic.clone());
    assert!(matches!(
        manual.projection_facts(),
        BlockProjectionFacts::Model
    ));
    let fields = manual.node_field_presence(0);
    assert!(
        fields
            .styles
            .is_present(!manual.blocks_flat[0].styles.is_empty())
    );
    assert!(!fields.directions.is_present(false));

    let explicit = BlockDiagramRenderModel::from_parts(
        semantic,
        BlockVisuals::default(),
        BlockProjectionFacts::Explicit {
            nodes: vec![BlockNodeFieldPresence {
                styles: BlockFieldPresence::Omitted,
                directions: BlockFieldPresence::Present,
            }],
            edge_directions: vec![Some(Vec::new())],
        },
    );
    let cloned = explicit.clone();
    assert!(!cloned.node_field_presence(0).styles.is_present(true));
    assert!(cloned.node_field_presence(0).directions.is_present(false));
    assert_eq!(cloned.edge_directions(0), Some([].as_slice()));
    assert_eq!(cloned.edge_directions(1), None);
}
