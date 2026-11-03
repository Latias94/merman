#![cfg(feature = "diagram-flowchart")]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

#[test]
fn bpmn_shapes_use_the_existing_flowchart_language_and_shared_registry() {
    let shapes: Vec<_> = merman_model::families::flowchart::shapes::public_pinned_shape_names()
        .filter(|shape| shape.starts_with("bpmn-"))
        .collect();
    assert_eq!(shapes.len(), 9);
    for shape in &shapes {
        let source = format!("flowchart LR\nA@{{ shape: {shape}, label: \"Step\" }} --> B\n");
        let parsed = SemanticParser::new().parse_diagram_sync(&source).unwrap();
        let SemanticDiagramModel::Flowchart(model) = parsed.model else {
            panic!("shared shape must stay in the Flowchart model");
        };
        let node = model.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!(node.layout_shape.as_deref(), Some(*shape));
    }
    assert!(
        SemanticParser::new()
            .parse_diagram_sync("bpmn\nA --> B\n")
            .is_err()
    );
}

#[test]
fn bpmn_source_fixtures_use_only_the_shared_public_shape_syntax() {
    for (source, count) in [
        (
            include_str!("../../../fixtures/flowchart/local_mermaid_12_3_bpmn_shapes.mmd"),
            9,
        ),
        (
            include_str!("../../../fixtures/flowchart/local_mermaid_12_3_bpmn_captions.mmd"),
            6,
        ),
    ] {
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let SemanticDiagramModel::Flowchart(model) = parsed.model else {
            panic!("BPMN shape fixture must remain a Flowchart");
        };
        assert_eq!(model.nodes.len(), count);
        assert!(model.nodes.iter().all(|node| {
            node.layout_shape.as_deref().is_some_and(|shape| {
                shape.starts_with("bpmn-")
                    && merman_model::families::flowchart::shapes::is_valid_pinned_shape(shape)
            })
        }));
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn bpmn_editor_names_are_exported_from_the_shared_shape_registry() {
    let names: Vec<_> = merman_core::editor::flowchart_shape_names().collect();
    for shape in merman_model::families::flowchart::shapes::public_pinned_shape_names()
        .filter(|shape| shape.starts_with("bpmn-"))
    {
        assert!(names.contains(&shape));
    }
    assert!(!names.contains(&"anchorTo"));
    assert!(!names.contains(&"markers"));
    assert!(!names.contains(&"dataDirection"));
}
