use super::*;

#[test]
fn terminal_fallback_serializes_admitted_metadata_beyond_managed_serde_limit() {
    let mut metadata = Value::String("leaf".to_string());
    for _ in 0..129 {
        metadata = Value::Array(vec![metadata]);
    }
    let model = KanbanDiagramModel {
        nodes: vec![KanbanNodeModel {
            id: "card".to_string(),
            label: "Card".to_string(),
            level: 0,
            node_type: NODE_TYPE_DEFAULT,
            span: SourceSpan::new(0, 0),
            is_group: false,
            parent_id: None,
            ticket: None,
            priority: None,
            assigned: None,
            icon: None,
            css_classes: None,
            shape: None,
            metadata: Some(crate::ManagedJson::from_value(metadata)),
        }],
    };
    let typed_error = serde_json::to_value(&model).unwrap_err();
    assert!(typed_error.to_string().contains("128-container depth"));
    let terminal = KanbanTerminalView::from_model(&model);
    assert_eq!(
        terminal.complexity(),
        crate::resources::ModelComplexity::new(130, 12, 129)
    );
    let fallback = serde_json::to_value(terminal)
        .expect("admitted terminal metadata must use the caller's nesting policy");
    let mut value = &fallback["nodes"][0]["metadata"];
    for _ in 0..129 {
        let values = value.as_array().unwrap();
        assert_eq!(values.len(), 1);
        value = &values[0];
    }
    assert_eq!(value.as_str(), Some("leaf"));
}

#[cfg(feature = "presentation-data")]
#[test]
fn detached_terminal_fallback_keeps_hidden_authored_facts_without_changing_typed_wire() {
    let mut node = KanbanRenderNode::new("host", "Host card");
    node.ticket = Some("MC-42".to_string());
    node = node.with_compatibility(KanbanRenderNodeCompatibility {
        level: 7,
        css_classes: Some("host-class".to_string()),
        shape: Some("host-shape".to_string()),
        ..Default::default()
    });
    let model = KanbanDiagramRenderModel { nodes: vec![node] };
    let typed = serde_json::to_value(&model).unwrap();
    assert!(typed["nodes"][0].get("level").is_none());
    assert!(typed["nodes"][0].get("cssClasses").is_none());
    assert!(typed["nodes"][0].get("shape").is_none());
    let terminal = KanbanTerminalView::from_render(&model);
    let fallback = serde_json::to_value(terminal).unwrap();
    assert_eq!(fallback.as_object().unwrap().len(), 1);
    assert_eq!(fallback["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(
        fallback["nodes"][0],
        serde_json::json!({
            "id": "host", "label": "Host card", "isGroup": false, "parentId": null,
            "ticket": "MC-42", "priority": null, "assigned": null, "icon": null,
            "level": 7, "cssClasses": "host-class", "shape": "host-shape"
        })
    );
    let base = model_complexity_from_kanban(&model);
    assert_eq!(
        terminal.complexity(),
        crate::resources::ModelComplexity::new(
            base.items,
            base.text_bytes + "host-class".len() + "host-shape".len(),
            base.nesting_depth
        )
    );
    assert_eq!(serde_json::to_value(&model).unwrap(), typed);
}
