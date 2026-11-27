use super::*;
use crate::state::StateSyntax;

fn parse_document(source: &str, control: &OperationControl) -> StateDocumentScratch {
    let document = StateSyntax::lex(source, control)
        .unwrap()
        .into_document(control)
        .unwrap()
        .expect("real State grammar accepts the fixture");
    StateDocumentScratch::from_parsed_parts(document.builder, document.root)
}

#[test]
fn state_consuming_projection_moves_owned_payloads_without_copying() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\naccTitle: Owned title\naccDescr: Owned description\n",
        "state \"Ready label\" as Ready\nReady: Owned detail\n",
        "note right of Ready : Owned note\n",
        "classDef active fill:red,color:blue\nclassDef other stroke:black\n",
        "class Ready active\nstyle Ready fill:green,font-weight:bold\n",
        "Ready --> Done: Owned transition\n[*] --> Ready: Entry\n",
        "click Ready \"https://example.com/first\" \"First tip\"\n",
        "click Ready \"https://example.com/second\" \"Second tip\"\n",
        "click Done \"https://example.com/only\" \"Only tip\"\n",
    );
    let control = OperationControl::new();
    let document = parse_document(source, &control);
    let mut db = StateDb::new();
    db.set_root_doc(document, &control).unwrap();
    // This legacy AST slot has no authored grammar production; populate its actual DB owner.
    db.set_text_style("Ready", "font-size:17px");

    // This proof exercises no-slack Vec storage transfer, including buffer identity.
    // Compact before pointer capture; the separate slack test covers retained capacity.
    let compact = |values: Vec<String>| values.into_boxed_slice().into_vec();
    for state in db.states.values_mut() {
        for values in [
            &mut state.descriptions,
            &mut state.classes,
            &mut state.styles,
            &mut state.text_styles,
        ] {
            *values = compact(std::mem::take(values));
            assert_eq!(values.capacity(), values.len());
        }
    }
    for class in db.style_classes.values_mut() {
        for values in [&mut class.styles, &mut class.text_styles] {
            *values = compact(std::mem::take(values));
            assert_eq!(values.capacity(), values.len());
        }
    }

    let document_storage = db.document.root_statements().as_ptr();
    let common = (
        db.direction.as_ref().unwrap().as_ptr(),
        db.acc_title.as_ref().unwrap().as_ptr(),
        db.acc_descr.as_ref().unwrap().as_ptr(),
    );
    let state_keys = db
        .state_order
        .iter()
        .map(|key| (key.clone(), key.as_ptr()))
        .collect::<Vec<_>>();
    assert_eq!(db.state_order, ["Ready", "Done", "root_start"]);
    let ready = &db.states["Ready"];
    assert!(ready.descriptions.iter().all(|value| !value.is_empty()));
    let record = (
        ready.id.as_ptr(),
        ready.ty.as_ptr(),
        ready.descriptions.as_ptr(),
        ready.classes.as_ptr(),
        ready.styles.as_ptr(),
        ready.text_styles.as_ptr(),
    );
    let payloads = ready
        .descriptions
        .iter()
        .chain(&ready.classes)
        .chain(&ready.styles)
        .chain(&ready.text_styles)
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    let note = ready.note.as_ref().unwrap();
    let note_payloads = (note.position.as_ref().unwrap().as_ptr(), note.text.as_ptr());
    let relation = &db.relations[0];
    let relation_payloads = (
        relation.id1.as_ptr(),
        relation.id2.as_ptr(),
        relation.relation_title.as_ref().unwrap().as_ptr(),
    );
    let class_payloads = db
        .style_classes
        .iter()
        .map(|(key, class)| {
            (
                key.clone(),
                key.as_ptr(),
                class.id.as_ptr(),
                class.styles.as_ptr(),
                class.text_styles.as_ptr(),
                class
                    .styles
                    .iter()
                    .chain(&class.text_styles)
                    .map(|value| value.as_ptr())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let link_payloads = db
        .links
        .iter()
        .map(|(key, declarations)| {
            (
                key.clone(),
                key.as_ptr(),
                declarations
                    .iter()
                    .map(|link| (link.url.as_ptr(), link.tooltip.as_ptr()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let model = StateSource(db)
        .into_render_model_controlled(&MermaidConfig::empty_object(), &control)
        .unwrap()
        .unwrap();

    assert_eq!(
        model
            .document
            .get(model.document.root().unwrap())
            .unwrap()
            .as_ptr(),
        document_storage
    );
    assert_eq!(
        (
            model.direction.as_ptr(),
            model.acc_title.as_ref().unwrap().as_ptr(),
            model.acc_descr.as_ref().unwrap().as_ptr()
        ),
        common
    );
    assert_eq!(model.direction, "LR");
    assert_eq!(model.acc_title.as_deref(), Some("Owned title"));
    assert_eq!(model.acc_descr.as_deref(), Some("Owned description"));
    for (id, pointer) in state_keys {
        assert_eq!(model.states.get_key_value(&id).unwrap().0.as_ptr(), pointer);
    }
    let ready = &model.states["Ready"];
    assert_eq!(
        (
            ready.id.as_ptr(),
            ready.state_type.as_ptr(),
            ready.descriptions.as_ptr(),
            ready.classes.as_ptr(),
            ready.styles.as_ptr(),
            ready.text_styles.as_ptr()
        ),
        record
    );
    assert_eq!(
        ready
            .descriptions
            .iter()
            .chain(&ready.classes)
            .chain(&ready.styles)
            .chain(&ready.text_styles)
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>(),
        payloads
    );
    assert_eq!(ready.state_type, "default");
    assert_eq!(model.states["root_start"].start, Some(true));
    assert!(ready.doc.is_none());
    assert_eq!(ready.descriptions, ["Ready label", "Owned detail"]);
    assert_eq!(ready.classes, ["active"]);
    assert_eq!(ready.styles, ["fill:green", "font-weight:bold"]);
    assert_eq!(ready.text_styles, ["font-size:17px"]);
    let note = ready.note.as_ref().unwrap();
    assert_eq!(
        (note.position.as_ref().unwrap().as_ptr(), note.text.as_ptr()),
        note_payloads
    );
    assert_eq!(note.position.as_deref(), Some("right of"));
    assert_eq!(note.text, "Owned note");
    let relation = &model.relations[0];
    assert_eq!(
        (
            relation.id1.as_ptr(),
            relation.id2.as_ptr(),
            relation.relation_title.as_ref().unwrap().as_ptr()
        ),
        relation_payloads
    );
    assert_eq!(relation.id1, "Ready");
    assert_eq!(relation.id2, "Done");
    assert_eq!(relation.relation_title.as_deref(), Some("Owned transition"));
    assert_eq!(
        model
            .relations
            .iter()
            .map(|relation| (
                relation.id1.as_str(),
                relation.id2.as_str(),
                relation.relation_title.as_deref()
            ))
            .collect::<Vec<_>>(),
        [
            ("Ready", "Done", Some("Owned transition")),
            ("root_start", "Ready", Some("Entry"))
        ]
    );
    assert_eq!(
        model
            .style_classes
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["active", "other"]
    );
    for (id, key_pointer, id_pointer, styles_pointer, text_styles_pointer, payloads) in
        class_payloads
    {
        let (key, class) = model.style_classes.get_key_value(&id).unwrap();
        assert_eq!(
            (
                key.as_ptr(),
                class.id.as_ptr(),
                class.styles.as_ptr(),
                class.text_styles.as_ptr()
            ),
            (key_pointer, id_pointer, styles_pointer, text_styles_pointer)
        );
        assert_eq!(
            class
                .styles
                .iter()
                .chain(&class.text_styles)
                .map(|value| value.as_ptr())
                .collect::<Vec<_>>(),
            payloads
        );
    }
    assert_eq!(
        model.style_classes["active"].styles,
        ["fill:red", "color:blue"]
    );
    assert_eq!(model.style_classes["active"].text_styles, ["fill:blue"]);
    assert_eq!(model.style_classes["other"].styles, ["stroke:black"]);
    assert!(model.style_classes["other"].text_styles.is_empty());
    for (id, key_pointer, pointers) in link_payloads {
        let (key, links) = model.links.get_key_value(&id).unwrap();
        assert_eq!(key.as_ptr(), key_pointer);
        let links = match links {
            StateLinks::One(link) => {
                assert_eq!(id, "Done");
                std::slice::from_ref(link)
            }
            StateLinks::Many(links) => {
                assert_eq!(id, "Ready");
                links.as_slice()
            }
        };
        assert_eq!(
            links
                .iter()
                .map(|link| (link.url.as_ptr(), link.tooltip.as_ptr()))
                .collect::<Vec<_>>(),
            pointers
        );
    }
    let StateLinks::One(link) = &model.links["Done"] else {
        panic!("one link retains the historical One representation");
    };
    assert_eq!(link.url, "https://example.com/only");
    assert_eq!(link.tooltip, "Only tip");
    let StateLinks::Many(links) = &model.links["Ready"] else {
        panic!("repeated links retain their source order");
    };
    assert_eq!(
        links
            .iter()
            .map(|link| (link.url.as_str(), link.tooltip.as_str()))
            .collect::<Vec<_>>(),
        [
            ("https://example.com/first", "First tip"),
            ("https://example.com/second", "Second tip")
        ]
    );
}

#[test]
fn state_consuming_projection_compacts_vec_slack_without_copying_strings() {
    let source = concat!(
        "stateDiagram-v2\nstate \"Ready label\" as Ready\nReady: Owned detail\n",
        "classDef active fill:red,color:blue\nclass Ready active\n",
        "style Ready fill:green,font-weight:bold\nReady --> Done\n",
    );
    let control = OperationControl::new();
    let document = parse_document(source, &control);
    let mut db = StateDb::new();
    db.set_root_doc(document, &control).unwrap();
    // This legacy AST slot has no authored grammar production; populate its actual DB owner.
    db.set_text_style("Ready", "font-size:17px");

    // Keep expected contents independent of both DB and projected model allocations.
    let expected: [&[&str]; 6] = [
        &["Ready label", "Owned detail"],
        &["active"],
        &["fill:green", "font-weight:bold"],
        &["font-size:17px"],
        &["fill:red", "color:blue"],
        &["fill:blue"],
    ];
    let ready = db.states.get_mut("Ready").unwrap();
    let active = db.style_classes.get_mut("active").unwrap();
    let fields = [
        &mut ready.descriptions,
        &mut ready.classes,
        &mut ready.styles,
        &mut ready.text_styles,
        &mut active.styles,
        &mut active.text_styles,
    ];
    let payloads = fields
        .into_iter()
        .zip(expected)
        .map(|(values, expected)| {
            assert_eq!(values.as_slice(), expected);
            assert!(!values.is_empty());
            assert!(values.iter().all(|value| !value.is_empty()));
            values.reserve(17);
            assert!(values.capacity() > values.len());
            values
                .iter()
                .map(|value| value.as_ptr())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let model = StateSource(db)
        .into_render_model_controlled(&MermaidConfig::empty_object(), &control)
        .unwrap()
        .unwrap();

    let ready = &model.states["Ready"];
    let active = &model.style_classes["active"];
    let fields = [
        &ready.descriptions,
        &ready.classes,
        &ready.styles,
        &ready.text_styles,
        &active.styles,
        &active.text_styles,
    ];
    for ((values, expected), pointers) in fields.into_iter().zip(expected).zip(payloads) {
        assert_eq!(values.capacity(), values.len());
        assert_eq!(values.as_slice(), expected);
        assert_eq!(
            values
                .iter()
                .map(|value| value.as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
    }
}

#[test]
fn state_container_color_slots_follow_mermaid_depth_first_order() {
    let control = OperationControl::new();
    let source = "stateDiagram-v2\nstate A {\n  [*] --> A1\n}\nstate B {\n  [*] --> B1\n}\n";
    let model = StateSyntax::lex(source, &control)
        .unwrap()
        .into_source(&control)
        .unwrap()
        .unwrap()
        .into_render_model_controlled(&MermaidConfig::empty_object(), &control)
        .unwrap()
        .unwrap();
    let a = model
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("A group");
    let b = model
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("B group");
    assert_eq!(a.color_index, Some(0));
    assert_eq!(b.color_index, Some(1));
}

#[test]
fn state_consuming_projection_preserves_programmatic_hidden_note_inputs() {
    let control = OperationControl::new();
    let source = "stateDiagram-v2\nnote right of Legacy : Hidden legacy text\n";
    let mut document = parse_document(source, &control);
    let root = document.root.unwrap();
    // The legacy hidden-note representation is programmatic, not authored grammar syntax.
    let [StateStatement::State(state)] = document.get_mut(root).as_mut_slice() else {
        panic!("real grammar produces the positioned-note owner");
    };
    state.note.as_mut().unwrap().position = None;
    let mut db = StateDb::new();
    db.set_root_doc(document, &control).unwrap();
    let model = StateSource(db)
        .into_render_model_controlled(&MermaidConfig::empty_object(), &control)
        .unwrap()
        .unwrap();

    let note = model.states["Legacy"].note.as_ref().unwrap();
    assert_eq!(note.text, "Hidden legacy text");
    assert_eq!(note.position, None);
    assert_eq!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["Legacy----parent", "Legacy----note-0", "Legacy"]
    );
    let group = &model.nodes[0];
    let note_node = &model.nodes[1];
    assert_eq!(
        (
            group.shape.as_str(),
            group.is_group,
            group.position.as_deref()
        ),
        ("noteGroup", true, None)
    );
    assert_eq!(note_node.shape, "note");
    assert_eq!(
        note_node.label.as_ref().and_then(Value::as_str),
        Some("Hidden legacy text")
    );
    assert_eq!(note_node.position, None);
    assert_eq!(note_node.parent_id.as_deref(), Some("Legacy----parent"));
    assert_eq!(group.label, note_node.label);
    assert_eq!(model.edges.len(), 1);
    let edge = &model.edges[0];
    assert_eq!(
        (
            edge.start.as_str(),
            edge.end.as_str(),
            edge.classes.as_str()
        ),
        ("Legacy", "Legacy----note-0", "transition note-edge")
    );
    assert!(edge.arrow_type_end.is_empty());
}

#[test]
fn state_semantic_model_does_not_collect_dom_ids_with_presentation_enabled() {
    let source = "stateDiagram-v2\nstate Outer {\n[*] --> A\nnote right of A : remember\n}\n";
    let control = OperationControl::new();
    let config = MermaidConfig::empty_object();
    let parse = || {
        StateSyntax::lex(source, &control)
            .unwrap()
            .into_source(&control)
            .unwrap()
            .unwrap()
    };
    reset_dom_id_count();
    let semantic = parse()
        .into_semantic_model_controlled(&config, &control)
        .unwrap()
        .unwrap();
    assert_eq!(dom_id_count(), 0);
    let render = parse()
        .into_render_model_controlled(&config, &control)
        .unwrap()
        .unwrap();
    assert_eq!(semantic.nodes.len(), render.nodes.len());
    assert!(dom_id_count() > 0);
}
