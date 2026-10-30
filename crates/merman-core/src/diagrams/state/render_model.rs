pub use merman_model::families::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};

#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::{Map, json};

use super::CSS_EDGE_NOTE_EDGE;
#[cfg(feature = "compat-json")]
use super::StateDocument;
use super::model::{StateDiagramModel, StateModelConstruction, StateNode};
#[cfg(feature = "compat-json")]
use super::{StateDocumentId, StateStatement};
#[cfg(feature = "compat-json")]
use crate::{ManagedSemanticJson, PresentationMetadata, Result};
use crate::{OperationControl, OperationControlResult};

#[cfg(all(test, feature = "compat-json"))]
thread_local! {
    static STATE_PROJECTED_DOCUMENT_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(all(test, feature = "compat-json"))]
pub(super) fn reset_projected_document_count() {
    STATE_PROJECTED_DOCUMENT_COUNT.set(0);
}

#[cfg(all(test, feature = "compat-json"))]
pub(super) fn projected_document_count() -> usize {
    STATE_PROJECTED_DOCUMENT_COUNT.get()
}

pub(super) struct StateNodePresentation {
    pub dom_id: String,
    pub padding: Option<f64>,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub color_index: Option<usize>,
    pub wrapping_width: Option<f64>,
    pub min_width: Option<f64>,
}

pub(super) fn into_render_model(
    construction: StateModelConstruction,
    config: &crate::MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<StateDiagramRenderModel> {
    let StateDiagramModel {
        document,
        direction,
        acc_title,
        acc_descr,
        nodes,
        edges,
        relations,
        links,
        states,
        style_classes,
    } = construction.model;
    debug_assert_eq!(nodes.len(), construction.presentation.len());
    let mut render_nodes = Vec::with_capacity(nodes.len());
    for (index, (node, presentation)) in
        nodes.into_iter().zip(construction.presentation).enumerate()
    {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        render_nodes.push(render_node_from_semantic(node, presentation));
    }
    let mut render_edges = Vec::with_capacity(edges.len());
    for (index, edge) in edges.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let arrow_type_end = if edge.classes == CSS_EDGE_NOTE_EDGE {
            ""
        } else if config.get_str("look") == Some("neo") {
            "arrow_barb_neo"
        } else {
            "arrow_barb"
        };
        render_edges.push(StateDiagramRenderEdge {
            id: edge.id,
            start: edge.start,
            end: edge.end,
            classes: edge.classes,
            label: edge.label,
            arrow_type_end: arrow_type_end.to_string(),
        });
    }
    control.checkpoint()?;
    Ok(StateDiagramRenderModel {
        document,
        direction,
        acc_title,
        acc_descr,
        nodes: render_nodes,
        edges: render_edges,
        relations,
        links,
        states,
        style_classes,
    })
}

pub(crate) fn sanitize_common_db_fields(
    model: &mut StateDiagramRenderModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(feature = "compat-json")]
fn project_document(
    document: &StateDocument,
    root: StateDocumentId,
    control: &OperationControl,
) -> OperationControlResult<ManagedSemanticJson> {
    struct Frame<'a> {
        id: StateDocumentId,
        index: usize,
        values: Vec<ManagedSemanticJson>,
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
                frame
                    .values
                    .push(super::db::stmt_to_json_shallow(stmt, None));
            }
            continue;
        }
        let frame = stack.pop().expect("completed frame exists");
        let doc: ManagedSemanticJson = Value::Array(
            frame
                .values
                .into_iter()
                .map(ManagedSemanticJson::into_unmanaged_value)
                .collect(),
        )
        .into();
        #[cfg(test)]
        STATE_PROJECTED_DOCUMENT_COUNT.set(STATE_PROJECTED_DOCUMENT_COUNT.get() + 1);
        if let Some(owner) = frame.owner {
            let value = super::db::stmt_to_json_shallow(owner, Some(doc));
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

fn render_node_from_semantic(
    node: StateNode,
    presentation: StateNodePresentation,
) -> StateDiagramRenderNode {
    StateDiagramRenderNode {
        id: node.id,
        label_style: String::new(),
        label: node.label,
        description: node.description,
        dom_id: presentation.dom_id,
        is_group: node.is_group,
        node_type: node.node_type,
        parent_id: node.parent_id,
        css_classes: node.css_classes,
        css_compiled_styles: node.css_compiled_styles,
        css_styles: node.css_styles,
        dir: node.dir,
        explicit_dir: node.explicit_dir,
        padding: presentation.padding,
        rx: presentation.rx,
        ry: presentation.ry,
        shape: node.shape,
        position: node.position,
        color_index: presentation.color_index,
        wrapping_width: presentation.wrapping_width,
        min_width: presentation.min_width,
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &StateDiagramRenderModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    Ok(
        render_model_to_compat_json_controlled(model, meta, &OperationControl::new())
            .expect("a private operation control cannot be cancelled")?
            .into_unmanaged_value(),
    )
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &StateDiagramRenderModel,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedSemanticJson>> {
    let states = match state_records_to_compat_json(model, control)? {
        Ok(states) => states,
        Err(error) => return Ok(Err(error)),
    };
    control.checkpoint()?;
    let look = meta
        .effective_config
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
    root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("other".to_string(), Value::Object(Map::new()));
    root.insert(
        "config".to_string(),
        crate::json_value::clone_value_nonrecursive(meta.effective_config.as_value()),
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

#[cfg(feature = "compat-json")]
fn state_records_to_compat_json(
    model: &StateDiagramRenderModel,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedSemanticJson>> {
    let mut out = Vec::with_capacity(model.states.len());
    for (index, (id, state)) in model.states.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let doc = match state.doc {
            Some(doc) => {
                if model.document.get(doc).is_none() {
                    return Ok(Err(crate::Error::diagram_parse_fallback(
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
                doc.map(ManagedSemanticJson::into_unmanaged_value)
                    .unwrap_or(Value::Null),
            );
        out.push((id.clone(), ManagedSemanticJson::from(record)));
    }
    control.checkpoint()?;
    Ok(Ok(Value::Object(
        out.into_iter()
            .map(|(id, value)| (id, value.into_unmanaged_value()))
            .collect(),
    )
    .into()))
}

#[cfg(feature = "compat-json")]
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

#[cfg(feature = "compat-json")]
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

#[cfg(feature = "compat-json")]
fn option_number_value(value: Option<f64>) -> Value {
    value
        .map(crate::json_value::number_value)
        .unwrap_or(Value::Null)
}
