use super::*;

pub(super) fn authored_model(with_collisions: bool) -> ClassDiagramModel {
    let mut classes = IndexMap::new();
    classes.insert("Service".to_owned(), node("Service", Some("A.B")));
    if with_collisions {
        for id in ["note0", "interface0"] {
            classes.insert(id.to_owned(), node(id, None));
        }
    }
    let mut namespaces = IndexMap::new();
    for (id, label, parent, explicit) in [("A", "A", None, false), ("A.B", "B", Some("A"), true)] {
        namespaces.insert(
            id.to_owned(),
            Namespace {
                id: id.to_owned(),
                label: label.to_owned(),
                class_ids: if explicit {
                    vec!["Service".to_owned()]
                } else {
                    Vec::new()
                },
                note_ids: if explicit && with_collisions {
                    vec!["note0".to_owned()]
                } else {
                    Vec::new()
                },
                parent: parent.map(str::to_owned),
                explicit,
            },
        );
    }
    ClassDiagramModel {
        diagram_type: "classDiagram".to_owned(),
        direction: "TB".to_owned(),
        acc_title: None,
        acc_descr: None,
        classes,
        relations: if with_collisions {
            vec![ClassRelation {
                id: "0".to_owned(),
                id1: "interface0".to_owned(),
                id2: "Service".to_owned(),
                relation_title_1: None,
                relation_title_2: None,
                title: String::new(),
                relation: RelationShape {
                    type1: REL_LOLLIPOP,
                    type2: -1,
                    line_type: 0,
                },
            }]
        } else {
            Vec::new()
        },
        notes: if with_collisions {
            vec![ClassNote {
                id: "note0".to_owned(),
                class_id: None,
                text: "namespace note".to_owned(),
                parent: Some("A.B".to_owned()),
            }]
        } else {
            Vec::new()
        },
        interfaces: if with_collisions {
            vec![ClassInterface {
                id: "interface0".to_owned(),
                label: "IService".to_owned(),
                class_id: "Service".to_owned(),
            }]
        } else {
            Vec::new()
        },
        namespaces,
        namespace_facade_aliases: BTreeMap::new(),
        style_classes: IndexMap::new(),
        constants: ClassConstants {
            line_type: ClassLineTypeConstants {
                line: 0,
                dotted_line: 1,
            },
            relation_type: ClassRelationTypeConstants {
                none: -1,
                aggregation: 0,
                extension: 1,
                composition: 2,
                dependency: 3,
                lollipop: REL_LOLLIPOP,
            },
        },
        style_precedence_facts: ClassStylePrecedenceFacts::default(),
    }
}

fn node(id: &str, parent: Option<&str>) -> ClassNode {
    ClassNode {
        id: id.to_owned(),
        type_param: String::new(),
        label: id.to_owned(),
        text: id.to_owned(),
        css_classes: String::new(),
        methods: Vec::new(),
        members: Vec::new(),
        annotations: Vec::new(),
        styles: Vec::new(),
        parent: parent.map(str::to_owned),
        link: None,
        link_target: None,
        tooltip: None,
        have_callback: false,
        callback: None,
        callback_effective: false,
    }
}

#[test]
fn typed_provenance_preserves_targets_ordinals_and_namespace_aliases() {
    let mut model = authored_model(true);
    model
        .namespace_facade_aliases
        .insert("Facade".to_owned(), "A.B".to_owned());
    model.style_precedence_facts = ClassStylePrecedenceFacts::from_witnesses(
        Some(ClassStylePrecedenceWitness::new(
            "paint".to_owned(),
            "Service".to_owned(),
            vec!["stroke:red".to_owned()],
            3,
            11,
        )),
        Some(ClassStylePrecedenceWitness::new(
            "other".to_owned(),
            "note0".to_owned(),
            vec!["fill:blue".to_owned()],
            17,
            29,
        )),
        Some(ClassStyleDeclarationWitness::new(
            "Service".to_owned(),
            vec!["stroke:green".to_owned()],
            37,
        )),
        Some(ClassStyleDeclarationWitness::new(
            "paint".to_owned(),
            vec!["font-size:20px".to_owned()],
            41,
        )),
    );
    let wire = serde_json::to_value(&model).unwrap();
    assert!(wire["classes"]["Service"].get("domId").is_none());
    let restored: ClassDiagramModel = serde_json::from_value(wire).unwrap();
    assert_eq!(restored, model);
    let witness = restored
        .style_precedence_facts
        .assignment_before_definition_copy_witness()
        .unwrap();
    assert_eq!(witness.class_name(), "paint");
    assert_eq!(witness.target(), "Service");
    assert_eq!(witness.earlier_style_event_ordinal(), 3);
    assert_eq!(witness.later_style_event_ordinal(), 11);
    assert_eq!(restored.namespace_facade_aliases["Facade"], "A.B");
}

#[cfg(feature = "presentation-data")]
#[test]
fn full_wire_keeps_detached_identities_and_omits_canonical_style_witnesses() {
    let mut model = authored_model(true);
    model.style_precedence_facts = ClassStylePrecedenceFacts::from_witnesses(
        None,
        None,
        Some(ClassStyleDeclarationWitness::new(
            "Service".to_owned(),
            vec!["stroke:red".to_owned()],
            7,
        )),
        None,
    );
    let full = ClassDiagram::new(model.clone(), &crate::MermaidConfig::empty_object());
    let mut wire = serde_json::to_value(&full).unwrap();
    assert!(wire.get("stylePrecedenceFacts").is_none());
    assert!(wire.get("topology").is_none());
    assert_eq!(wire["notes"][0]["id"], "note0$1");
    wire["classes"]["Service"]["domId"] = "detached-service".into();
    wire["namespaces"]["A.B"]["domId"] = "detached-namespace".into();
    let detached: ClassDiagram = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        detached.class("Service").unwrap().dom_id,
        "detached-service"
    );
    assert_eq!(
        detached.topology_view().namespace("A.B").unwrap().route_id,
        "detached-namespace"
    );
    assert_eq!(
        detached.model().style_precedence_facts,
        ClassStylePrecedenceFacts::default()
    );
    assert_eq!(serde_json::to_value(&detached).unwrap(), wire);
    assert_eq!(full.into_model(), model);
}
