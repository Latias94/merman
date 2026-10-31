use super::*;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::{MermaidConfig, OperationControl};
#[cfg(feature = "presentation")]
use crate::{MermaidEngine, ParseOptions, RenderSemanticModel, SourcePresentationMetadata};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};

#[cfg(feature = "presentation")]
fn model(source: &str) -> StateDiagramRenderModel {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("state parses")
        .expect("state detected");
    let RenderSemanticModel::State(model) = parsed.model() else {
        panic!("expected state model");
    };
    model.clone()
}

fn chain(depth: usize) -> String {
    let mut source = String::from("stateDiagram-v2\n");
    for index in 0..depth {
        source.push_str(&format!("state S{index} {{\n"));
    }
    source.push_str("Leaf\n");
    for _ in 0..depth {
        source.push_str("}\n");
    }
    source
}

#[cfg(feature = "presentation")]
fn meta() -> SourcePresentationMetadata {
    SourcePresentationMetadata {
        diagram_type: "stateDiagram-v2".to_string(),
        config: MermaidConfig::empty_object(),
        effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
        title: None,
    }
}

#[cfg(feature = "compat-json")]
fn compatibility(model: &StateDiagramRenderModel) -> crate::ManagedSemanticJson {
    crate::ManagedSemanticJson::from(
        render_model_to_compat_json(model, &meta()).expect("state compatibility projection"),
    )
}

#[test]
#[cfg(feature = "compat-json")]
fn state_semantic_graph_matches_full_model_without_presentation_materialization() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\n",
        "state Outer {\ndirection RL\n[*] --> A: ready\n",
        "note right of A : remember\n}\nOuter --> Done: finish\n",
        "classDef active fill:red,color:blue\nclass A active\n",
        "click Done href \"https://example.com\"\n",
    );
    parse::reset_state_syntax_construction_count();
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
    render_model::reset_projected_document_count();
    db::reset_dom_id_count();
    let semantic = parse::parse_state_semantic_model_controlled(
        source,
        "stateDiagram-v2",
        &MermaidConfig::default(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .unwrap()
    .model
    .unwrap();
    assert_eq!(parse::state_syntax_construction_count(), 1);
    #[cfg(feature = "editor-facts")]
    assert_eq!(crate::editor::test_work::read(), Default::default());
    assert_eq!(render_model::projected_document_count(), 0);
    assert_eq!(db::dom_id_count(), 0);

    let render = model(source);
    assert!(db::dom_id_count() > 0);
    assert_eq!(semantic.direction, render.direction);
    assert_eq!(semantic.document.root(), render.document.root());
    assert_eq!(semantic.document.len(), render.document.len());
    assert_eq!(semantic.nodes.len(), render.nodes.len());
    assert_eq!(semantic.edges.len(), render.edges.len());
    for (semantic, render) in semantic.nodes.iter().zip(&render.nodes) {
        let mut expected = serde_json::to_value(render).unwrap();
        let expected = expected.as_object_mut().unwrap();
        for field in [
            "labelStyle",
            "domId",
            "padding",
            "rx",
            "ry",
            "colorIndex",
            "wrappingWidth",
            "minWidth",
        ] {
            expected.remove(field);
        }
        expected.insert("explicitDir".to_string(), json!(render.explicit_dir));
        assert_eq!(serde_json::to_value(semantic).unwrap(), json!(expected));
    }
    for (semantic, render) in semantic.edges.iter().zip(&render.edges) {
        let mut expected = serde_json::to_value(render).unwrap();
        expected.as_object_mut().unwrap().remove("arrowTypeEnd");
        assert_eq!(serde_json::to_value(semantic).unwrap(), expected);
    }
    assert_eq!(semantic.states["Outer"].doc, render.states["Outer"].doc);
    assert_eq!(
        serde_json::to_value(&semantic.links).unwrap(),
        serde_json::to_value(&render.links).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&semantic.style_classes).unwrap(),
        serde_json::to_value(&render.style_classes).unwrap()
    );
    assert!(semantic.nodes.iter().any(|node| node.shape == "note"));
    assert!(
        semantic
            .nodes
            .iter()
            .any(|node| node.explicit_dir == Some(true))
    );
    assert!(semantic.nodes.iter().any(|node| {
        node.css_compiled_styles
            .iter()
            .any(|style| style == "fill:red")
    }));
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
}

#[test]
fn state_semantic_graph_preserves_validation_and_cancellation() {
    for source in [
        "stateDiagram-v2\nstate Group {\nChild\n}\nGroup: first\nGroup: second\n",
        "stateDiagram-v2\nnote right of A : note\nstate A----note-0\n",
    ] {
        let semantic = parse::parse_state_semantic_model_controlled(
            source,
            "stateDiagram-v2",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model;
        let semantic_error = semantic.expect_err("canonical State validation rejects the input");
        assert!(matches!(semantic_error, crate::Error::DiagramParse { .. }));
        #[cfg(feature = "presentation")]
        {
            let render = parse::parse_state_model_for_render_controlled(
                source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(semantic_error.to_string(), render.unwrap_err().to_string());
        }
    }
    let control = OperationControl::new();
    control.cancel_after_checkpoints(20);
    let source = chain(128);
    assert!(
        parse::parse_state_semantic_model_controlled(
            &source,
            "stateDiagram-v2",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &control,
        )
        .is_err()
    );
    assert!(
        parse::parse_state_semantic_model_controlled(
            &source,
            "stateDiagram-v2",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .is_ok()
    );
}

#[test]
#[cfg(feature = "compat-json")]
fn state_flat_documents_preserve_dividers_and_nested_semantic_order() {
    let model = model(concat!(
        "stateDiagram-v2\ndirection LR\n",
        "state Outer {\ndirection RL\nstate Inner {\n",
        "[*] --> A: ready\n--\nB --> [*]: done\n}\n",
        "--\nnote right of C : note\nC --> D: next\n}\nOuter --> E\n",
    ));
    let root = model.document.root().expect("canonical root");
    let statements = model.document.get(root).expect("root statements");
    assert_eq!(statements.len(), 3);
    assert_eq!(
        model
            .document
            .iter()
            .map(|(_, statements)| statements.len())
            .sum::<usize>(),
        13
    );
    let outer = model.states["Outer"].doc.expect("outer document id");
    let partitions = model.document.get(outer).expect("outer partitions");
    assert_eq!(partitions.len(), 2);
    let StateStatement::State(first) = &partitions[0] else {
        panic!("first divider")
    };
    assert_eq!(first.id, "divider-id-2");
    let first_doc = model
        .document
        .get(first.doc.expect("first divider document"))
        .unwrap();
    assert!(matches!(first_doc[0], StateStatement::Direction(ref direction) if direction == "RL"));
    let StateStatement::State(inner) = &first_doc[1] else {
        panic!("inner composite")
    };
    let inner_parts = model.document.get(inner.doc.unwrap()).unwrap();
    let StateStatement::State(inner_first) = &inner_parts[0] else {
        panic!("inner divider")
    };
    assert_eq!(inner_first.id, "divider-id-1");
    let json = compatibility(&model);
    let doc = &json["states"]["Outer"]["doc"];
    assert_eq!(
        doc[0]["doc"][1]["doc"][0]["doc"][0]["state1"]["id"],
        "divider-id-1_start"
    );
    assert_eq!(doc[0]["doc"][1]["doc"][0]["doc"][0]["description"], "ready");
    assert_eq!(model.direction, "LR");
    let a = model.nodes.iter().find(|node| node.id == "A").unwrap();
    assert_eq!(a.parent_id.as_deref(), Some("divider-id-1"));
    assert_eq!(
        model
            .edges
            .iter()
            .filter(|edge| edge.classes == "transition")
            .map(|edge| edge.label.as_str())
            .collect::<Vec<_>>(),
        ["ready", "done", "next", ""]
    );
    assert_eq!(
        model
            .nodes
            .iter()
            .filter(|node| node.shape == "note")
            .count(),
        1
    );
    let note = model
        .nodes
        .iter()
        .find(|node| node.shape == "note")
        .unwrap();
    assert_eq!(note.id, "C----note-2");
    assert_eq!(note.dom_id, "state-C----note-2");
    assert_eq!(note.label.as_ref().and_then(Value::as_str), Some("note"));
    assert_eq!(note.position.as_deref(), Some("right of"));
    assert_eq!(note.parent_id.as_deref(), Some("C----parent"));
    assert_eq!(note.node_type.as_deref(), Some("node"));
    assert!(!note.is_group);
    let group = model
        .nodes
        .iter()
        .find(|node| node.id == "C----parent")
        .unwrap();
    assert!(group.is_group);
    assert_eq!(group.shape, "noteGroup");
    assert_eq!(group.position.as_deref(), Some("right of"));
    assert_eq!(group.label, note.label);
    let edge = model
        .edges
        .iter()
        .find(|edge| edge.classes == "transition note-edge")
        .unwrap();
    assert_eq!(
        (
            edge.start.as_str(),
            edge.end.as_str(),
            edge.arrow_type_end.as_str()
        ),
        ("C", "C----note-2", "")
    );
    let wire_note = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "C----note-2")
        .unwrap();
    assert_eq!(wire_note["label"], "note");
    assert_eq!(wire_note["position"], "right of");
    assert_eq!(wire_note["parentId"], "C----parent");
    assert_eq!(wire_note["isGroup"], false);
    let wire_edge = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["classes"] == "transition note-edge")
        .unwrap();
    assert_eq!(wire_edge["start"], "C");
    assert_eq!(wire_edge["end"], "C----note-2");
    assert_eq!(wire_edge["arrowhead"], "none");
}

#[test]
#[cfg(feature = "compat-json")]
fn state_root_document_lookup_preserves_first_root_composite() {
    let model = model(concat!(
        "stateDiagram-v2\nstate Container {\nstate Shared {\nNested\n}\n}\n",
        "state Shared {\nFirst\n}\nstate Shared {\nLater\n}\n",
    ));
    let root = model.document.get(model.document.root().unwrap()).unwrap();
    let [
        StateStatement::State(container),
        StateStatement::State(first),
        StateStatement::State(last),
    ] = root
    else {
        panic!("three root composites retain source order");
    };
    assert_eq!(
        (container.id.as_str(), first.id.as_str(), last.id.as_str()),
        ("Container", "Shared", "Shared")
    );
    let [StateStatement::State(nested)] = model.document.get(container.doc.unwrap()).unwrap()
    else {
        panic!("the nested same-id composite precedes both root definitions");
    };
    assert_eq!(nested.id, "Shared");
    let selected = model.states["Shared"].doc.unwrap();
    assert_eq!(selected, first.doc.unwrap());
    assert_ne!(selected, last.doc.unwrap());
    assert_ne!(selected, nested.doc.unwrap());
    let [StateStatement::State(child)] = model.document.get(selected).unwrap() else {
        panic!("the first root document remains the typed owner");
    };
    assert_eq!(child.id, "First");
    assert_eq!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["Container", "Shared", "Nested", "First", "Later"]
    );
    assert_eq!(
        compatibility(&model)["states"]["Shared"]["doc"],
        json!([{
            "stmt": "state", "id": "First", "type": "default",
            "description": null, "doc": null, "classes": [],
        }])
    );
}

#[test]
#[cfg(feature = "compat-json")]
fn state_compatibility_projection_preserves_nested_documents() {
    for source in [
        "stateDiagram-v2\nA --> B: label\nnote right of A : note\n",
        "stateDiagram-v2\naccTitle: title\naccDescr: description\nstate A {\ndirection LR\nstate B {\nC:::active\n}\n}\nclassDef active fill:red\nclick A href \"https://example.com\"\n",
        "stateDiagram-v2\nstate A {\nB\n--\nC\n}\n",
        "stateDiagram-v2\n",
    ] {
        let model = model(source);
        let json = compatibility(&model);
        assert!(json["states"].is_object());
        assert_eq!(json["type"], "stateDiagram-v2");
        assert!(json["config"].is_object());
        assert!(json.get("document").is_none());
    }
}

#[test]
#[cfg(feature = "compat-json")]
fn state_compatibility_projection_supports_deep_documents() {
    let model = model(&chain(130));
    let json = compatibility(&model);
    let mut output = Vec::new();
    json.write_json(&mut output)
        .expect("deep state compatibility JSON");
    assert!(output.starts_with(b"{"));
}

#[test]
#[cfg(feature = "compat-json")]
fn state_compatibility_projection_handles_shared_documents() {
    let mut model = model(&chain(130));
    let doc = model.states["S0"].doc;
    for index in 0..32 {
        let id = format!("Shared{index}");
        model.states.insert(
            id.clone(),
            StateRecord {
                id,
                doc,
                ..StateRecord::default()
            },
        );
    }
    let json = compatibility(&model);
    assert!(json["states"].is_object());
}

#[test]
fn state_divider_assignment_cancellation_releases_completed_grammar_documents() {
    let mut source = String::from("stateDiagram-v2\nstate Outer {\n");
    for index in 0..300 {
        source.push_str(&format!("S{index}\n--\n"));
    }
    source.push_str("}\n");
    let mut document = StateDocumentScratch::default();
    let root = state_grammar::RootParser::new()
        .parse(&mut document, Lexer::new(&source))
        .unwrap();
    document.root = Some(root);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let mut count = 0;
    let result = parse::assign_divider_ids(&mut document, &mut count, &control);
    assert!(result.is_err());
    assert!(
        count > 0 && count < 300,
        "assignment had useful work before cancellation"
    );
    drop(document);
    let model = parse::parse_state_semantic_model_controlled(
        "stateDiagram-v2\nA\n",
        "stateDiagram-v2",
        &MermaidConfig::default(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .unwrap()
    .model
    .unwrap();
    assert_eq!(model.nodes.len(), 1);
}

#[test]
#[cfg(feature = "compat-json")]
fn state_projection_cancellation_releases_completed_inner_documents() {
    let model = model(&chain(1000));
    let control = OperationControl::new();
    // One state-record checkpoint and one at each 128 projection steps. This reaches the
    // ascent, where completed nested JSON values already live in managed frame collections.
    control.cancel_after_checkpoints(10);
    render_model::reset_projected_document_count();
    assert!(matches!(
        render_model_to_compat_json_controlled(&model, &meta(), &control),
        Err(crate::OperationCancelled { .. })
    ));
    let completed = render_model::projected_document_count();
    assert!(
        completed > 0 && completed < 1000,
        "inner documents completed before projection cancellation"
    );
    let mut output = Vec::new();
    compatibility(&model).write_json(&mut output).unwrap();
    assert!(!output.is_empty());
}

#[test]
#[cfg(feature = "compat-json")]
fn state_foreign_document_ids_report_projection_failure() {
    let foreign = model(&chain(3));
    let mut target = model("stateDiagram-v2\nA\n");
    target.states.get_mut("A").unwrap().doc = foreign.states["S0"].doc;
    assert!(render_model_to_compat_json(&target, &meta()).is_err());
    assert_eq!(
        target.model_complexity(),
        model("stateDiagram-v2\nA\n").model_complexity()
    );
}

#[test]
fn state_semantic_deep_documents_clone_count_and_drop_without_recursion() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let source = chain(1000);
            let model = parse::parse_state_semantic_model_controlled(
                &source,
                "stateDiagram-v2",
                &MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            let clone = model.clone();
            assert_eq!(clone.document.len(), 1001);
            let complexity = model.model_complexity();
            assert!(complexity.items > model.nodes.len());
            assert_eq!(complexity.nesting_depth, 1000);
            assert_eq!(clone.model_complexity(), complexity);
            drop(model);
            drop(clone);
        })
        .unwrap()
        .join()
        .unwrap();
}
