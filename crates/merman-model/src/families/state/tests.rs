use super::*;
use crate::resources::ModelComplexity;
use serde_json::json;

fn state_with_document(id: &str, doc: StateDocumentId) -> StateStatement {
    let mut state = StateStatementState::new(id.to_string());
    state.doc = Some(doc);
    StateStatement::State(state)
}

fn nested_document() -> (StateDocument, StateDocumentId) {
    let mut builder = StateDocumentBuilder::default();
    let child = builder.append(vec![StateStatement::State(StateStatementState::new(
        "Leaf".into(),
    ))]);
    let root = builder.append(vec![state_with_document("Outer", child)]);
    (builder.build(Some(root)).unwrap(), child)
}

#[test]
fn completed_documents_keep_allocation_order_and_owned_statement_storage() {
    let mut builder = StateDocumentBuilder::default();
    let root = builder.append(Vec::new());
    let child = builder.append(vec![StateStatement::Direction("LR".into())]);
    builder
        .edit(root)
        .unwrap()
        .push(state_with_document("Outer", child));
    let storage = builder.get(child).unwrap().as_ptr();
    let document = builder.build(Some(root)).unwrap();
    assert_eq!(document.root(), Some(root));
    assert_eq!(document.get(child).unwrap().as_ptr(), storage);
    assert_eq!(
        document.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        [root, child]
    );
    assert_eq!(root.index(), 0);
    assert_eq!(child.index(), 1);
}

#[test]
fn completion_rejects_invalid_roots_children_and_non_tree_documents() {
    assert!(
        StateDocumentBuilder::default()
            .build(None)
            .unwrap()
            .is_empty()
    );
    let mut foreign = StateDocumentBuilder::default();
    foreign.append(Vec::new());
    let absent = foreign.append(Vec::new());
    assert_eq!(
        StateDocumentBuilder::default()
            .build(Some(absent))
            .unwrap_err(),
        StateDocumentError::InvalidRoot
    );

    let mut builder = StateDocumentBuilder::default();
    builder.append(Vec::new());
    assert_eq!(
        builder.build(None).unwrap_err(),
        StateDocumentError::InvalidRoot
    );

    let mut builder = StateDocumentBuilder::default();
    let root = builder.append(vec![state_with_document("Missing", absent)]);
    assert_eq!(
        builder.build(Some(root)).unwrap_err(),
        StateDocumentError::InvalidChild
    );

    let mut builder = StateDocumentBuilder::default();
    let root = builder.append(Vec::new());
    builder
        .edit(root)
        .unwrap()
        .push(state_with_document("Cycle", root));
    assert_eq!(
        builder.build(Some(root)).unwrap_err(),
        StateDocumentError::InvalidTree
    );

    let mut builder = StateDocumentBuilder::default();
    let child = builder.append(Vec::new());
    let root = builder.append(vec![
        state_with_document("One", child),
        state_with_document("Two", child),
    ]);
    assert_eq!(
        builder.build(Some(root)).unwrap_err(),
        StateDocumentError::InvalidTree
    );

    let mut builder = StateDocumentBuilder::default();
    builder.append(Vec::new());
    let root = builder.append(Vec::new());
    assert_eq!(
        builder.build(Some(root)).unwrap_err(),
        StateDocumentError::InvalidTree
    );

    let mut builder = StateDocumentBuilder::default();
    let child = builder.append(Vec::new());
    let mut endpoint = StateStatementState::new("A".into());
    endpoint.doc = Some(child);
    let root = builder.append(vec![StateStatement::Relation(Box::new(
        StateStatementRelation {
            state1: endpoint,
            state2: StateStatementState::new("B".into()),
            description: None,
        },
    ))]);
    assert_eq!(
        builder.build(Some(root)).unwrap_err(),
        StateDocumentError::InvalidTree
    );
}

#[test]
fn canonical_flat_view_preserves_records_sharing_one_document() {
    let (document, child) = nested_document();
    let root = document.root().unwrap();
    let mut model = StateDiagramModel {
        document,
        direction: "LR".into(),
        ..Default::default()
    };
    for id in ["Outer", "Shared"] {
        model.states.insert(
            id.into(),
            StateRecord {
                id: id.into(),
                doc: Some(child),
                ..Default::default()
            },
        );
    }
    let view = StateTerminalView::from_semantic(&model);
    let value = serde_json::to_value(view).unwrap();
    assert_eq!(value["document"]["root"], root.index());
    assert_eq!(value["document"]["documents"].as_array().unwrap().len(), 2);
    assert_eq!(
        value["document"]["documents"][root.index()][0]["doc"],
        child.index()
    );
    assert_eq!(value["states"]["Outer"]["doc"], child.index());
    assert_eq!(value["states"]["Shared"]["doc"], child.index());
    assert_eq!(
        serde_json::to_value(model.states["Outer"].compatibility_fields()).unwrap()["doc"],
        json!(null)
    );
    let statement = &model.document.get(root).unwrap()[0];
    assert_eq!(
        serde_json::to_value(statement.compatibility_fields()).unwrap()["doc"],
        json!(null)
    );
    assert_eq!(view.complexity(), model.model_complexity());
    let json_complexity = ModelComplexity::from_json(&value);
    assert_eq!(
        view.complexity().nesting_depth,
        json_complexity.nesting_depth
    );
    assert!(view.complexity().text_bytes <= json_complexity.text_bytes);
    assert!(value.get("config").is_none());
}

#[test]
fn canonical_terminal_graph_borrows_owned_payloads() {
    let node: StateNode = serde_json::from_value(json!({
        "id": "A", "label": "ready", "description": ["description"], "isGroup": false,
        "type": null, "parentId": null, "cssClasses": "", "cssCompiledStyles": [],
        "cssStyles": [], "dir": null, "explicitDir": null, "shape": "rect", "position": null
    }))
    .unwrap();
    let model = StateDiagramModel {
        nodes: vec![node],
        edges: vec![StateEdge {
            start: "A".into(),
            end: "B".into(),
            label: "next".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let view = StateTerminalView::from_semantic(&model);
    let node = view.nodes().get(0).unwrap();
    let edge = view.edges().iter().next().unwrap();
    assert!(std::ptr::eq(node.id.as_ptr(), model.nodes[0].id.as_ptr()));
    assert!(std::ptr::eq(
        node.shape.as_ptr(),
        model.nodes[0].shape.as_ptr()
    ));
    assert!(std::ptr::eq(
        edge.label.as_ptr(),
        model.edges[0].label.as_ptr()
    ));
    assert!(node.label_style.is_empty());
    assert!(edge.arrow_type_end.is_empty());
    assert!(view.nodes().get(1).is_none());
}

#[test]
#[cfg(feature = "presentation-data")]
fn full_terminal_shape_retains_typed_fields_and_authored_direction_provenance() {
    let (document, child) = nested_document();
    let node: StateDiagramRenderNode = serde_json::from_value(json!({
        "id": "A", "shape": "rect", "labelStyle": "color:red", "label": "ready",
        "domId": "state-A", "wrappingWidth": 144.0, "minWidth": 132.0
    }))
    .unwrap();
    let mut model = StateDiagramRenderModel {
        document,
        nodes: vec![node],
        edges: vec![StateDiagramRenderEdge {
            id: "edge".into(),
            start: "A".into(),
            end: "B".into(),
            arrow_type_end: "arrow_barb".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    model.nodes[0].explicit_dir = Some(true);
    model.states.insert(
        "Outer".into(),
        StateRecord {
            id: "Outer".into(),
            doc: Some(child),
            ..Default::default()
        },
    );
    let node_value = serde_json::to_value(&model.nodes[0]).unwrap();
    assert!(node_value.get("explicitDir").is_none());
    let view = StateTerminalView::from_render(&model);
    let value = serde_json::to_value(view).unwrap();
    assert_eq!(value["nodes"][0]["explicitDir"], true);
    for (field, expected) in node_value.as_object().unwrap() {
        assert_eq!(&value["nodes"][0][field], expected, "field: {field}");
    }
    assert_eq!(value["edges"], serde_json::to_value(&model.edges).unwrap());
    assert_eq!(value["states"]["Outer"]["doc"], child.index());
    assert!(std::ptr::eq(
        view.nodes().get(0).unwrap().label_style.as_ptr(),
        model.nodes[0].label_style.as_ptr()
    ));
    assert!(std::ptr::eq(
        view.edges().iter().next().unwrap().arrow_type_end.as_ptr(),
        model.edges[0].arrow_type_end.as_ptr()
    ));
    assert!(model.model_complexity().items > 0);
    let mut detached = StateDiagramRenderModel {
        nodes: vec![model.nodes[0].clone()],
        ..StateDiagramRenderModel::default()
    };
    detached.nodes[0].explicit_dir = Some(true);
    let value = serde_json::to_value(StateTerminalView::from_render(&detached)).unwrap();
    assert_eq!(value["nodes"][0]["explicitDir"], true);
    assert!(
        value["document"]["documents"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn deep_document_completion_clone_complexity_and_disposal_do_not_recurse() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            const DEPTH: usize = 4000;
            let mut builder = StateDocumentBuilder::default();
            let mut root = builder.append(Vec::new());
            for index in 0..DEPTH {
                root = builder.append(vec![state_with_document(&format!("S{index}"), root)]);
            }
            let model = StateDiagramModel {
                document: builder.build(Some(root)).unwrap(),
                ..Default::default()
            };
            let complexity = model.model_complexity();
            assert!(complexity.nesting_depth >= DEPTH);
            let clone = model.clone();
            assert_eq!(clone.model_complexity(), complexity);
            assert_eq!(clone.document.len(), DEPTH + 1);
            drop(clone);
            drop(model);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn completion_cancellation_releases_construction_without_publishing_a_document() {
    let mut builder = StateDocumentBuilder::default();
    let root = builder.append((0..300).map(|_| StateStatement::Noop).collect());
    let control = crate::OperationControl::new();
    control.cancel_after_checkpoints(2);
    assert!(builder.build_controlled(Some(root), &control).is_err());
    assert!(
        StateDocumentBuilder::default()
            .build(None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cancellation_precedes_root_errors_and_structure_failures_use_the_inner_channel() {
    let mut builder = StateDocumentBuilder::default();
    builder.append(Vec::new());
    let control = crate::OperationControl::new();
    control.cancel();
    assert!(builder.build_controlled(None, &control).is_err());

    let mut builder = StateDocumentBuilder::default();
    builder.append(Vec::new());
    assert_eq!(
        builder
            .build_controlled(None, &crate::OperationControl::new())
            .unwrap()
            .unwrap_err(),
        StateDocumentError::InvalidRoot
    );
}
