//! Pure Mermaid-compatible JSON projection of completed flat State documents.

use super::{
    CSS_EDGE_NOTE_EDGE, StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
    StateDocument, StateDocumentId, StateStatement, StateStatementState,
};
use crate::json::ManagedJson;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};

fn project_document(
    document: &StateDocument,
    root: StateDocumentId,
    control: &OperationControl,
) -> OperationControlResult<ManagedJson> {
    struct Frame<'a> {
        id: StateDocumentId,
        index: usize,
        values: Vec<ManagedJson>,
        owner: Option<&'a StateStatement>,
    }
    let mut stack = vec![Frame {
        id: root,
        index: 0,
        values: Vec::new(),
        owner: None,
    }];
    let mut steps = 0usize;
    loop {
        if steps.is_multiple_of(128) {
            control.checkpoint()?;
        }
        steps = steps.saturating_add(1);
        let frame = stack
            .last_mut()
            .expect("root projection frame exists until completion");
        let statements = document.get(frame.id).expect("completed state document");
        if let Some(stmt) = statements.get(frame.index) {
            frame.index += 1;
            if let StateStatement::State(state) = stmt
                && let Some(child) = state.doc
            {
                stack.push(Frame {
                    id: child,
                    index: 0,
                    values: Vec::new(),
                    owner: Some(stmt),
                });
            } else {
                frame.values.push(stmt_to_json_shallow(stmt, None));
            }
            continue;
        }
        let frame = stack.pop().expect("completed frame exists");
        let doc: ManagedJson = Value::Array(
            frame
                .values
                .into_iter()
                .map(ManagedJson::into_unmanaged_value)
                .collect(),
        )
        .into();
        #[cfg(any(test, feature = "test-support"))]
        test_support::record_projected_document();
        if let Some(owner) = frame.owner {
            let value = stmt_to_json_shallow(owner, Some(doc));
            stack
                .last_mut()
                .expect("a nested document has a parent frame")
                .values
                .push(value);
        } else {
            control.checkpoint()?;
            return Ok(doc);
        }
    }
}

pub fn project(
    model: &StateDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    Ok(project_controlled(model, meta, &OperationControl::new())
        .expect("a private operation control cannot be cancelled")?
        .into_unmanaged_value())
}

pub fn project_controlled(
    model: &StateDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
    let states = match state_records_to_compat_json(model, control)? {
        Ok(states) => states,
        Err(error) => return Ok(Err(error)),
    };
    control.checkpoint()?;
    let look = meta
        .config
        .as_value()
        .get("look")
        .cloned()
        .unwrap_or(Value::Null);
    let mut nodes = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        nodes.push(state_node_to_compat_json(node, &look));
    }
    let mut edges = Vec::with_capacity(model.edges.len());
    for (index, edge) in model.edges.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        edges.push(state_edge_to_compat_json(edge, &look));
    }
    control.checkpoint()?;
    let mut root = Map::with_capacity(13);
    root.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("other".to_string(), Value::Object(Map::new()));
    root.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    root.insert(
        "direction".to_string(),
        Value::String(model.direction.clone()),
    );
    root.insert("accTitle".to_string(), json!(&model.acc_title));
    root.insert("accDescr".to_string(), json!(&model.acc_descr));
    root.insert("states".to_string(), states.into_unmanaged_value());
    root.insert("relations".to_string(), json!(&model.relations));
    root.insert("styleClasses".to_string(), json!(&model.style_classes));
    root.insert("links".to_string(), json!(&model.links));
    Ok(Ok(Value::Object(root).into()))
}

fn state_records_to_compat_json(
    model: &StateDiagramRenderModel,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
    let mut out = Vec::with_capacity(model.states.len());
    for (index, (id, state)) in model.states.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let doc = match state.doc {
            Some(doc) => {
                if model.document.get(doc).is_none() {
                    return Ok(Err(CompatProjectionError::invalid_model(
                        "stateDiagram",
                        "State document id does not belong to this model",
                    )));
                }
                Some(project_document(&model.document, doc, control)?)
            }
            None => None,
        };
        let mut record = serde_json::to_value(state.compatibility_fields())
            .expect("state scalar fields serialize");
        record
            .as_object_mut()
            .expect("state record is an object")
            .insert(
                "doc".to_string(),
                doc.map(ManagedJson::into_unmanaged_value)
                    .unwrap_or(Value::Null),
            );
        out.push((id.clone(), ManagedJson::from(record)));
    }
    control.checkpoint()?;
    Ok(Ok(Value::Object(
        out.into_iter()
            .map(|(id, value)| (id, value.into_unmanaged_value()))
            .collect(),
    )
    .into()))
}

fn state_node_to_compat_json(node: &StateDiagramRenderNode, look: &Value) -> Value {
    if node.shape == "noteGroup" {
        return json!({
            "labelStyle": node.label_style,
            "shape": node.shape,
            "label": node.label,
            "cssClasses": node.css_classes,
            "cssStyles": node.css_styles,
            "cssCompiledStyles": node.css_compiled_styles,
            "id": node.id,
            "domId": node.dom_id,
            "type": "group",
            "isGroup": node.is_group,
            "padding": option_number_value(node.padding),
            "look": look,
            "position": node.position,
            "colorIndex": node.color_index,
            "wrappingWidth": option_number_value(node.wrapping_width),
            "minWidth": option_number_value(node.min_width),
        });
    }

    if node.shape == "note" {
        return json!({
            "labelStyle": node.label_style,
            "shape": node.shape,
            "label": node.label,
            "cssClasses": node.css_classes,
            "cssStyles": node.css_styles,
            "cssCompiledStyles": node.css_compiled_styles,
            "id": node.id,
            "domId": node.dom_id,
            "type": node.node_type,
            "isGroup": node.is_group,
            "padding": option_number_value(node.padding),
            "look": look,
            "position": node.position,
            "parentId": node.parent_id,
            "colorIndex": node.color_index,
            "wrappingWidth": option_number_value(node.wrapping_width),
            "minWidth": option_number_value(node.min_width),
        });
    }

    let mut out = Map::with_capacity(18);
    out.insert(
        "labelStyle".to_string(),
        Value::String(node.label_style.clone()),
    );
    out.insert("shape".to_string(), Value::String(node.shape.clone()));
    out.insert("label".to_string(), json!(&node.label));
    out.insert(
        "cssClasses".to_string(),
        Value::String(node.css_classes.clone()),
    );
    out.insert(
        "cssCompiledStyles".to_string(),
        json!(&node.css_compiled_styles),
    );
    out.insert("cssStyles".to_string(), json!(&node.css_styles));
    out.insert("id".to_string(), Value::String(node.id.clone()));
    out.insert("dir".to_string(), json!(&node.dir));
    out.insert("domId".to_string(), Value::String(node.dom_id.clone()));
    out.insert("type".to_string(), json!(&node.node_type));
    out.insert("isGroup".to_string(), Value::Bool(node.is_group));
    out.insert("padding".to_string(), option_number_value(node.padding));
    out.insert("rx".to_string(), option_number_value(node.rx));
    out.insert("ry".to_string(), option_number_value(node.ry));
    out.insert("look".to_string(), look.clone());
    out.insert("parentId".to_string(), json!(&node.parent_id));
    out.insert("colorIndex".to_string(), json!(&node.color_index));
    out.insert(
        "wrappingWidth".to_string(),
        option_number_value(node.wrapping_width),
    );
    out.insert("minWidth".to_string(), option_number_value(node.min_width));
    out.insert("centerLabel".to_string(), Value::Bool(true));
    if let Some(description) = &node.description {
        out.insert("description".to_string(), json!(description));
    }
    Value::Object(out)
}

fn state_edge_to_compat_json(edge: &StateDiagramRenderEdge, look: &Value) -> Value {
    let note_edge = edge.classes == CSS_EDGE_NOTE_EDGE;
    let mut out = Map::with_capacity(13);
    out.insert("id".to_string(), Value::String(edge.id.clone()));
    out.insert("start".to_string(), Value::String(edge.start.clone()));
    out.insert("end".to_string(), Value::String(edge.end.clone()));
    out.insert(
        "arrowhead".to_string(),
        Value::String(if note_edge { "none" } else { "normal" }.to_string()),
    );
    out.insert(
        "arrowTypeEnd".to_string(),
        Value::String(edge.arrow_type_end.clone()),
    );
    out.insert("style".to_string(), Value::String("fill:none".to_string()));
    out.insert("labelStyle".to_string(), Value::String(String::new()));
    if !note_edge {
        out.insert("label".to_string(), Value::String(edge.label.clone()));
    }
    out.insert("classes".to_string(), Value::String(edge.classes.clone()));
    out.insert(
        "arrowheadStyle".to_string(),
        Value::String("fill: #333".to_string()),
    );
    out.insert("labelpos".to_string(), Value::String("c".to_string()));
    out.insert("labelType".to_string(), Value::String("text".to_string()));
    out.insert("thickness".to_string(), Value::String("normal".to_string()));
    out.insert("look".to_string(), look.clone());
    Value::Object(out)
}

fn option_number_value(value: Option<f64>) -> Value {
    value.map(crate::json::number_value).unwrap_or(Value::Null)
}

fn string_array_value(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn option_string_value(value: &Option<String>) -> Value {
    value
        .as_ref()
        .map(|v| Value::String(v.clone()))
        .unwrap_or(Value::Null)
}

fn state_stmt_ref_to_json(state: &StateStatementState) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), Value::String(state.id.clone()));
    obj.insert("type".to_string(), Value::String(state.ty.clone()));
    obj.insert("classes".to_string(), string_array_value(&state.classes));
    Value::Object(obj)
}

fn stmt_to_json_shallow(stmt: &StateStatement, doc: Option<ManagedJson>) -> ManagedJson {
    let value = match stmt {
        StateStatement::Noop => Value::Null,
        StateStatement::State(s) => {
            let mut obj = Map::new();
            obj.insert("stmt".to_string(), Value::String("state".to_string()));
            obj.insert("id".to_string(), Value::String(s.id.clone()));
            obj.insert("type".to_string(), Value::String(s.ty.clone()));
            obj.insert(
                "description".to_string(),
                option_string_value(&s.description),
            );
            obj.insert(
                "doc".to_string(),
                doc.map(ManagedJson::into_unmanaged_value)
                    .unwrap_or(Value::Null),
            );
            obj.insert("classes".to_string(), string_array_value(&s.classes));
            Value::Object(obj)
        }
        StateStatement::Relation(relation) => {
            let mut obj = Map::new();
            obj.insert("stmt".to_string(), Value::String("relation".to_string()));
            obj.insert(
                "state1".to_string(),
                state_stmt_ref_to_json(&relation.state1),
            );
            obj.insert(
                "state2".to_string(),
                state_stmt_ref_to_json(&relation.state2),
            );
            obj.insert(
                "description".to_string(),
                option_string_value(&relation.description),
            );
            Value::Object(obj)
        }
        StateStatement::ClassDef { id, classes } => {
            json!({ "stmt": "classDef", "id": id, "classes": classes })
        }
        StateStatement::ApplyClass { ids, class_name } => {
            json!({ "stmt": "applyClass", "id": ids, "styleClass": class_name })
        }
        StateStatement::Style { ids, styles } => {
            json!({ "stmt": "style", "id": ids, "styleClass": styles })
        }
        StateStatement::Direction(v) => json!({ "stmt": "dir", "value": v }),
        StateStatement::AccTitle(t) => json!(t),
        StateStatement::AccDescr(d) => json!(d),
        StateStatement::Click(c) => {
            json!({ "stmt": "click", "id": c.id, "url": c.url, "tooltip": c.tooltip })
        }
    };
    value.into()
}

/// Concrete instrumentation used by the existing source lifecycle tests.
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    thread_local! {
        static PROJECTED_DOCUMENT_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub fn reset_projected_document_count() {
        PROJECTED_DOCUMENT_COUNT.set(0);
    }
    pub fn projected_document_count() -> usize {
        PROJECTED_DOCUMENT_COUNT.get()
    }
    pub(super) fn record_projected_document() {
        PROJECTED_DOCUMENT_COUNT.set(PROJECTED_DOCUMENT_COUNT.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MermaidConfig;
    use crate::families::state::{
        StateDocumentBuilder, StateRecord, StateStatementClick, StateStatementNote,
        StateStatementRelation,
    };
    use crate::resources::ModelComplexity;

    #[test]
    fn state_document_counter_preserves_the_legacy_shallow_shape() {
        let mut builder = StateDocumentBuilder::default();
        let child = builder.append(vec![StateStatement::Direction("LR".to_owned())]);
        let mut state = StateStatementState::new("A".to_owned());
        state.description = Some("Label".to_owned());
        state.descriptions.push("canonical description".to_owned());
        state.doc = Some(child);
        state.note = Some(StateStatementNote {
            position: Some("right of".to_owned()),
            text: "canonical note".to_owned(),
        });
        state.classes.push("active".to_owned());
        state.styles.push("fill:red".to_owned());
        state.text_styles.push("color:blue".to_owned());
        state.start = Some(true);
        let statements = [
            (StateStatement::Noop, Value::Null),
            (
                StateStatement::State(state.clone()),
                json!({
                    "stmt": "state", "id": "A", "type": "default",
                    "description": "Label", "doc": null, "classes": ["active"]
                }),
            ),
            (
                StateStatement::State(StateStatementState::new("Empty".to_owned())),
                json!({
                    "stmt": "state", "id": "Empty", "type": "default",
                    "description": null, "doc": null, "classes": []
                }),
            ),
            (
                StateStatement::Relation(Box::new(StateStatementRelation {
                    state1: state,
                    state2: StateStatementState::new_typed("B".to_owned(), "end"),
                    description: Some("transition".to_owned()),
                })),
                json!({
                    "stmt": "relation",
                    "state1": { "id": "A", "type": "default", "classes": ["active"] },
                    "state2": { "id": "B", "type": "end", "classes": [] },
                    "description": "transition"
                }),
            ),
            (
                StateStatement::ClassDef {
                    id: "active".to_owned(),
                    classes: "fill:red".to_owned(),
                },
                json!({ "stmt": "classDef", "id": "active", "classes": "fill:red" }),
            ),
            (
                StateStatement::ApplyClass {
                    ids: "A,B".to_owned(),
                    class_name: "active".to_owned(),
                },
                json!({ "stmt": "applyClass", "id": "A,B", "styleClass": "active" }),
            ),
            (
                StateStatement::Style {
                    ids: "A".to_owned(),
                    styles: "color:blue".to_owned(),
                },
                json!({ "stmt": "style", "id": "A", "styleClass": "color:blue" }),
            ),
            (
                StateStatement::Direction("LR".to_owned()),
                json!({ "stmt": "dir", "value": "LR" }),
            ),
            (StateStatement::AccTitle("title".to_owned()), json!("title")),
            (
                StateStatement::AccDescr("description".to_owned()),
                json!("description"),
            ),
            (
                StateStatement::Click(StateStatementClick {
                    id: "A".to_owned(),
                    url: "https://example.com".to_owned(),
                    tooltip: "open".to_owned(),
                }),
                json!({
                    "stmt": "click", "id": "A", "url": "https://example.com", "tooltip": "open"
                }),
            ),
        ];
        for (statement, expected) in statements {
            let projected = stmt_to_json_shallow(&statement, None);
            let fields = statement.compatibility_fields();
            assert_eq!(projected.as_value(), &expected);
            assert_eq!(serde_json::to_value(&fields).unwrap(), expected);
            assert_eq!(
                ModelComplexity::from_serializable(&fields),
                ModelComplexity::from_json(&expected),
            );
        }
    }

    #[test]
    fn shared_document_references_expand_separately_for_each_state_record() {
        let mut builder = StateDocumentBuilder::default();
        let child = builder.append(vec![StateStatement::Direction("LR".to_owned())]);
        let mut owner = StateStatementState::new("Outer".to_owned());
        owner.doc = Some(child);
        let root = builder.append(vec![StateStatement::State(owner)]);
        let mut model = StateDiagramRenderModel {
            document: builder.build(Some(root)).unwrap(),
            ..StateDiagramRenderModel::default()
        };
        for id in ["A", "B"] {
            model.states.insert(
                id.to_owned(),
                StateRecord {
                    id: id.to_owned(),
                    doc: Some(root),
                    ..StateRecord::default()
                },
            );
        }
        let config = MermaidConfig::empty_object();
        let metadata = CompatJsonMetadata {
            diagram_type: "stateDiagram",
            config: &config,
            authored_config: &config,
            title: None,
            diagram_id: None,
        };
        test_support::reset_projected_document_count();
        let value = project(&model, &metadata).unwrap();
        assert_eq!(test_support::projected_document_count(), 4);
        assert_eq!(value["states"]["A"]["doc"], value["states"]["B"]["doc"]);
        assert_eq!(
            value["states"]["A"]["doc"][0]["doc"][0],
            json!({ "stmt": "dir", "value": "LR" })
        );
    }
}
