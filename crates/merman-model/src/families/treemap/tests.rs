use super::*;

#[test]
fn manual_models_and_declared_empty_output_remain_distinct() {
    let semantic = TreemapDiagramModel {
        root: TreemapNode {
            children: Some(Vec::new()),
            ..Default::default()
        },
        ..Default::default()
    };
    let manual = TreemapDiagramRenderModel::from_semantic_model(semantic.clone());
    let empty = TreemapDiagramRenderModel::from_parts(
        semantic,
        TreemapVisuals::default(),
        TreemapOutputPresence::Empty,
    );
    assert_eq!(manual.output_presence(), TreemapOutputPresence::Model);
    assert_eq!(
        empty.clone().output_presence(),
        TreemapOutputPresence::Empty
    );
    assert_eq!(
        serde_json::to_value(&manual.semantic).unwrap(),
        serde_json::to_value(&empty.semantic).unwrap()
    );
}

#[test]
fn flat_relationship_validation_preserves_errors_and_cancellation() {
    let mut model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
        root: TreemapNode {
            children: Some(vec![0]),
            ..Default::default()
        },
        nodes: vec![TreemapNode::default()],
        ..Default::default()
    });
    let control = crate::OperationControl::new();
    assert_eq!(
        model
            .validate_child_relationships_controlled(&control)
            .unwrap(),
        Ok(())
    );
    model.nodes[0].children = Some(vec![0]);
    assert_eq!(
        model
            .validate_child_relationships_controlled(&control)
            .unwrap(),
        Err("cyclic or repeated treemap child relationship")
    );
    model.nodes[0].children = Some(vec![1]);
    assert_eq!(
        model
            .validate_child_relationships_controlled(&control)
            .unwrap(),
        Err("treemap child ID is out of range")
    );
    control.cancel();
    assert!(
        model
            .validate_child_relationships_controlled(&control)
            .is_err()
    );
}
