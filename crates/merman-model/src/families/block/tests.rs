use super::*;

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
