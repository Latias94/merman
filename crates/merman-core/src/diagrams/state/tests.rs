use super::*;
use crate::resources::ModelComplexity;
use crate::{Engine, OperationControl, ParseOptions, RenderSemanticModel};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

fn model(source: &str) -> StateDiagramRenderModel {
    let parsed = Engine::new()
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

// Keep the former derived typed serialization shape as an independent accounting oracle.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FormerState<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    state_type: &'a str,
    descriptions: &'a [String],
    doc: Option<&'a Value>,
    note: &'a Option<StateDiagramRenderNote>,
    classes: &'a [String],
    styles: &'a [String],
    text_styles: &'a [String],
    start: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FormerModel<'a> {
    direction: &'a str,
    acc_title: &'a Option<String>,
    acc_descr: &'a Option<String>,
    nodes: &'a [StateDiagramRenderNode],
    edges: &'a [StateDiagramRenderEdge],
    relations: &'a [StateDiagramRenderRelation],
    links: &'a HashMap<String, StateDiagramRenderLinks>,
    states: HashMap<&'a str, FormerState<'a>>,
    style_classes: &'a indexmap::IndexMap<String, StateDiagramRenderStyleClass>,
}

fn former<'a>(model: &'a StateDiagramRenderModel, json: &'a Value) -> FormerModel<'a> {
    FormerModel {
        direction: &model.direction,
        acc_title: &model.acc_title,
        acc_descr: &model.acc_descr,
        nodes: &model.nodes,
        edges: &model.edges,
        relations: &model.relations,
        links: &model.links,
        states: model
            .states
            .iter()
            .map(|(id, state)| {
                let doc = &json["states"][id]["doc"];
                (
                    id.as_str(),
                    FormerState {
                        id: &state.id,
                        state_type: &state.state_type,
                        descriptions: &state.descriptions,
                        doc: (!doc.is_null()).then_some(doc),
                        note: &state.note,
                        classes: &state.classes,
                        styles: &state.styles,
                        text_styles: &state.text_styles,
                        start: state.start,
                    },
                )
            })
            .collect(),
        style_classes: &model.style_classes,
    }
}

#[test]
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
        model.document.documents.iter().map(Vec::len).sum::<usize>(),
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
    let json = model.to_json().expect("typed projection");
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
}

#[test]
fn state_typed_serde_and_complexity_retain_the_former_wire_shape() {
    for source in [
        "stateDiagram-v2\nA --> B: label\nnote right of A : note\n",
        "stateDiagram-v2\naccTitle: title\naccDescr: description\nstate A {\ndirection LR\nstate B {\nC:::active\n}\n}\nclassDef active fill:red\nclick A href \"https://example.com\"\n",
        "stateDiagram-v2\nstate A {\nB\n--\nC\n}\n",
        "stateDiagram-v2\n",
    ] {
        let model = model(source);
        let json = model.to_json().expect("typed projection");
        let former = former(&model, json.as_value());
        assert_eq!(
            serde_json::to_value(&model).unwrap(),
            serde_json::to_value(&former).unwrap()
        );
        assert_eq!(
            model.model_complexity(),
            ModelComplexity::from_serializable(&former)
        );
        assert!(json.get("document").is_none());
        assert!(json.get("config").is_none());
        assert!(json.get("type").is_none());
        let decoded: StateDiagramRenderModel =
            serde_json::from_value(json.as_value().clone()).unwrap();
        assert_eq!(decoded.to_json().unwrap(), json);
    }
}

fn emitted_container_depth(value: &Value) -> usize {
    let mut depth = 0usize;
    let mut stack = vec![(value, 0usize)];
    while let Some((value, parent_depth)) = stack.pop() {
        match value {
            Value::Array(values) => {
                let current = parent_depth + 1;
                depth = depth.max(current);
                stack.extend(values.iter().map(|value| (value, current)));
            }
            Value::Object(values) => {
                let current = parent_depth + 1;
                depth = depth.max(current);
                stack.extend(values.values().map(|value| (value, current)));
            }
            _ => {}
        }
    }
    depth
}

#[test]
fn state_serde_preflight_matches_empty_and_shallow_emitted_containers() {
    for source in [
        "stateDiagram-v2\n",
        "stateDiagram-v2\nstate Empty {\n}\n",
        "stateDiagram-v2\nA --> B: label\nnote right of A : note\nclick A href \"https://example.com/a\"\nclick A href \"https://example.com/b\"\n",
    ] {
        let model = model(source);
        let json = model.to_json().unwrap();
        assert_eq!(
            model.serialized_container_depth(),
            emitted_container_depth(json.as_value())
        );
        assert!(serde_json::to_string(&model).is_ok());
    }
}

#[test]
fn state_typed_serde_bounds_the_actual_emitted_containers() {
    let accepted = model(&chain(62));
    assert_eq!(accepted.model_complexity().nesting_depth, 127);
    assert_eq!(accepted.serialized_container_depth(), 128);
    assert_eq!(
        emitted_container_depth(accepted.to_json().unwrap().as_value()),
        128
    );
    assert_eq!(accepted.document.len(), 63);
    assert_eq!(
        accepted
            .document
            .documents
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        63
    );
    assert!(serde_json::to_string(&accepted).is_ok());
    let rejected = model(&chain(63));
    assert_eq!(rejected.model_complexity().nesting_depth, 129);
    assert_eq!(rejected.serialized_container_depth(), 130);
    assert_eq!(
        emitted_container_depth(rejected.to_json().unwrap().as_value()),
        130
    );
    render_model::reset_projected_document_count();
    let error =
        serde_json::to_string(&rejected).expect_err("generic serde must reject deep wire shape");
    assert!(error.to_string().contains("128"));
    assert_eq!(
        render_model::projected_document_count(),
        0,
        "generic serde rejects before projecting documents"
    );
    let mut output = Vec::new();
    rejected
        .to_json()
        .unwrap()
        .write_json(&mut output)
        .expect("iterative writer");
    assert!(output.starts_with(b"{"));
}

#[test]
fn state_serde_rejects_shared_deep_documents_before_required_output_duplication() {
    let mut model = model(&chain(130));
    let doc = model.states["S0"].doc;
    for index in 0..32 {
        let id = format!("Shared{index}");
        model.states.insert(
            id.clone(),
            StateDiagramRenderState {
                id,
                doc,
                ..StateDiagramRenderState::default()
            },
        );
    }
    render_model::reset_projected_document_count();
    let error = serde_json::to_writer(std::io::sink(), &model)
        .expect_err("deep shared docs exceed generic serde depth");
    assert!(error.to_string().contains("128"));
    assert_eq!(render_model::projected_document_count(), 0);
}

#[test]
fn state_divider_assignment_cancellation_releases_completed_grammar_documents() {
    let mut source = String::from("stateDiagram-v2\nstate Outer {\n");
    for index in 0..300 {
        source.push_str(&format!("S{index}\n--\n"));
    }
    source.push_str("}\n");
    let mut document = StateDocument::default();
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
    assert_eq!(model("stateDiagram-v2\nA\n").nodes.len(), 1);
}

#[test]
fn state_projection_cancellation_releases_completed_inner_documents() {
    let model = model(&chain(1000));
    let control = OperationControl::new();
    // One state-record checkpoint and one at each 128 projection steps. This reaches the
    // ascent, where completed nested JSON values already live in managed frame collections.
    control.cancel_after_checkpoints(10);
    render_model::reset_projected_document_count();
    assert!(model.to_json_controlled(&control).is_err());
    let completed = render_model::projected_document_count();
    assert!(
        completed > 0 && completed < 1000,
        "inner documents completed before projection cancellation"
    );
    let mut output = Vec::new();
    model.to_json().unwrap().write_json(&mut output).unwrap();
    assert!(!output.is_empty());
}

#[test]
fn state_foreign_document_ids_report_projection_failure() {
    let foreign = model(&chain(3));
    let mut target = model("stateDiagram-v2\nA\n");
    target.states.get_mut("A").unwrap().doc = foreign.states["S0"].doc;
    assert!(target.to_json().is_err());
    assert!(serde_json::to_string(&target).is_err());
    assert_eq!(
        target.model_complexity(),
        model("stateDiagram-v2\nA\n").model_complexity()
    );
}
