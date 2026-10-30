use super::*;

fn direct_model() -> SequenceDiagramModel {
    SequenceDiagramModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        actor_order: vec!["B".into(), "A".into()],
        actors: BTreeMap::new(),
        boxes: Vec::new(),
        messages: Vec::new(),
        notes: Vec::new(),
        created_actors: BTreeMap::from([("A".into(), 3), ("unordered".into(), 5)]),
        destroyed_actors: BTreeMap::from([("A".into(), 8)]),
        actor_lifecycles: None,
    }
}

#[test]
fn direct_lifecycle_maps_and_resolved_actor_order_remain_distinct() {
    let mut model = direct_model();
    assert_eq!(model.created_actor_message_index("A"), Some(3));
    assert_eq!(model.destroyed_actor_message_index_at(1), Some(8));
    assert_eq!(model.created_actor_message_index("unordered"), Some(5));
    assert_eq!(model.actor_lifecycle_at(0), None);
    assert_eq!(model.actor_lifecycle_at(2), None);

    let resolved = SequenceActorLifecycle {
        created_at: Some(6),
        destroyed_at: Some(7),
    };
    model.actor_lifecycles = Some(vec![SequenceActorLifecycle::default(), resolved]);
    assert_eq!(model.actor_lifecycle("A"), Some(resolved));
    assert_eq!(model.created_actor_message_index_at(1), Some(6));
    assert_eq!(model.destroyed_actor_message_index("A"), Some(7));
    assert_eq!(model.actor_lifecycle("B"), None);
    assert_eq!(model.actor_lifecycle("unordered"), None);

    model.actor_lifecycles = Some(Vec::new());
    assert_eq!(model.actor_lifecycle("A"), None);
    assert_eq!(model.actor_lifecycle_at(1), None);
}

#[test]
fn typed_serde_retains_defaults_and_explicit_empty_lifecycles() {
    let mut model: SequenceDiagramModel = serde_json::from_str(
        r#"{"actorOrder":["A"],"actors":{"A":{"description":"Label","type":"participant"}},"messages":[]}"#,
    )
    .unwrap();
    assert!(model.boxes.is_empty());
    assert!(model.notes.is_empty());
    assert!(model.created_actors.is_empty());
    assert!(model.destroyed_actors.is_empty());
    assert!(model.actor_lifecycles.is_none());
    assert!(model.actors["A"].name.is_empty());
    assert!(!model.actors["A"].wrap);
    assert!(model.actors["A"].links.is_empty());
    assert!(model.actors["A"].properties.is_empty());
    assert!(
        serde_json::to_value(&model)
            .unwrap()
            .get("actorLifecycles")
            .is_none()
    );

    model.actor_lifecycles = Some(Vec::new());
    let value = serde_json::to_value(&model).unwrap();
    assert_eq!(value["actorLifecycles"], serde_json::json!([]));
    let restored: SequenceDiagramModel = serde_json::from_value(value).unwrap();
    assert_eq!(restored.actor_lifecycles, Some(Vec::new()));

    let autonumber: SequenceAutonumber = serde_json::from_str(r#"{"start":10,"step":5}"#).unwrap();
    assert!(autonumber.visible);
    assert_eq!(
        serde_json::to_string(&autonumber).unwrap(),
        r#"{"start":10,"step":5,"visible":true}"#
    );
}

#[test]
fn borrowed_projection_preserves_order_without_typed_lifecycle_witnesses() {
    let mut model = direct_model();
    model.actor_lifecycles = Some(vec![SequenceActorLifecycle::default(); 2]);
    let mut serialized = Vec::new();
    model
        .compatibility_projection("sequence")
        .serialize(&mut serde_json::Serializer::new(&mut serialized))
        .unwrap();
    let projection: Value = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(
        projection
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "type",
            "title",
            "accTitle",
            "accDescr",
            "actorOrder",
            "actors",
            "messages",
            "notes",
            "boxes",
            "createdActors",
            "destroyedActors",
            "constants",
        ]
    );
    assert_eq!(projection["actorOrder"], serde_json::json!(["B", "A"]));
    assert_eq!(projection["createdActors"]["A"], 3);
    assert_eq!(projection["destroyedActors"]["A"], 8);
    assert_eq!(
        projection["constants"]["placement"],
        serde_json::json!({"leftOf": 0, "rightOf": 1, "over": 2})
    );
    assert!(projection.get("actorLifecycles").is_none());
    assert_eq!(model.actor_lifecycles.as_ref().unwrap().len(), 2);
}

#[test]
fn complexity_retains_arbitrary_actor_json_depth() {
    let mut model = direct_model();
    let mut payload = Value::Null;
    for _ in 0..300 {
        payload = Value::Array(vec![payload]);
    }
    model.actors.insert(
        "A".into(),
        SequenceActor {
            name: "A".into(),
            description: "A".into(),
            actor_type: "participant".into(),
            wrap: false,
            links: serde_json::Map::new(),
            properties: serde_json::Map::from_iter([("payload".into(), payload)]),
        },
    );
    let complexity = SequenceComplexity::from_model(&model);
    assert_eq!(complexity.nesting_depth, 304);
    assert_eq!(complexity.items, 302);
    assert_eq!(
        complexity.as_model_complexity(),
        crate::resources::ModelComplexity::new(302, complexity.text_bytes, 304)
    );
}
