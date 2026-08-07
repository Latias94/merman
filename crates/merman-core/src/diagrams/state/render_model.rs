use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

use crate::{ParseMetadata, Result};

fn default_state_direction() -> String {
    "TB".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderModel {
    #[serde(default = "default_state_direction")]
    pub direction: String,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub nodes: Vec<StateDiagramRenderNode>,
    #[serde(default)]
    pub edges: Vec<StateDiagramRenderEdge>,
    #[serde(default)]
    pub relations: Vec<StateDiagramRenderRelation>,
    #[serde(default)]
    pub links: HashMap<String, StateDiagramRenderLinks>,
    #[serde(default)]
    pub states: HashMap<String, StateDiagramRenderState>,
    #[serde(default, rename = "styleClasses")]
    pub style_classes: IndexMap<String, StateDiagramRenderStyleClass>,
}

impl StateDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderStyleClass {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderState {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub state_type: String,
    #[serde(default)]
    pub descriptions: Vec<String>,
    #[serde(default)]
    pub doc: Option<Value>,
    #[serde(default)]
    pub note: Option<StateDiagramRenderNote>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
    #[serde(default)]
    pub start: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderRelation {
    pub id1: String,
    pub id2: String,
    #[serde(default, rename = "relationTitle")]
    pub relation_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderNote {
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderLink {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tooltip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StateDiagramRenderLinks {
    /// Preserve the historical compatibility shape for a single declaration.
    One(StateDiagramRenderLink),
    /// Preserve every repeated declaration in source order.
    Many(Vec<StateDiagramRenderLink>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderNode {
    pub id: String,
    #[serde(default, rename = "labelStyle")]
    pub label_style: String,
    #[serde(default)]
    pub label: Option<Value>,
    #[serde(default)]
    pub description: Option<Vec<String>>,
    #[serde(default, rename = "domId")]
    pub dom_id: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    #[serde(default, rename = "type")]
    pub node_type: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssCompiledStyles")]
    pub css_compiled_styles: Vec<String>,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(
        default,
        rename = "explicitDir",
        skip_serializing_if = "Option::is_none"
    )]
    pub explicit_dir: Option<bool>,
    #[serde(default)]
    pub padding: Option<f64>,
    #[serde(default)]
    pub rx: Option<f64>,
    #[serde(default)]
    pub ry: Option<f64>,
    pub shape: String,
    #[serde(default)]
    pub position: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub classes: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: String,
    #[serde(default)]
    pub label: String,
}

pub(crate) fn render_model_to_compat_json(
    model: &StateDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let look = meta
        .effective_config
        .as_value()
        .get("look")
        .cloned()
        .unwrap_or(Value::Null);
    let nodes = model
        .nodes
        .iter()
        .map(|node| state_node_to_compat_json(node, &look))
        .collect();
    let edges = model
        .edges
        .iter()
        .map(|edge| state_edge_to_compat_json(edge, &look))
        .collect();

    let mut root = Map::with_capacity(13);
    root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("other".to_string(), Value::Object(Map::new()));
    root.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    root.insert(
        "direction".to_string(),
        Value::String(model.direction.clone()),
    );
    root.insert("accTitle".to_string(), json!(&model.acc_title));
    root.insert("accDescr".to_string(), json!(&model.acc_descr));
    root.insert(
        "states".to_string(),
        state_records_to_compat_json(&model.states),
    );
    root.insert("relations".to_string(), json!(&model.relations));
    root.insert("styleClasses".to_string(), json!(&model.style_classes));
    root.insert("links".to_string(), json!(&model.links));
    Ok(Value::Object(root))
}

fn state_records_to_compat_json(states: &HashMap<String, StateDiagramRenderState>) -> Value {
    let mut out = Map::with_capacity(states.len());
    for (id, state) in states {
        let doc = state
            .doc
            .as_ref()
            .map(crate::config::clone_value_nonrecursive)
            .unwrap_or(Value::Null);
        let note = state
            .note
            .as_ref()
            .map(|note| {
                json!({
                    "position": note.position,
                    "text": note.text,
                })
            })
            .unwrap_or(Value::Null);
        let mut record = Map::with_capacity(9);
        record.insert("id".to_string(), Value::String(state.id.clone()));
        record.insert("type".to_string(), Value::String(state.state_type.clone()));
        record.insert(
            "descriptions".to_string(),
            crate::compatibility_json::string_array_value(&state.descriptions),
        );
        record.insert("doc".to_string(), doc);
        record.insert("note".to_string(), note);
        record.insert(
            "classes".to_string(),
            crate::compatibility_json::string_array_value(&state.classes),
        );
        record.insert(
            "styles".to_string(),
            crate::compatibility_json::string_array_value(&state.styles),
        );
        record.insert(
            "textStyles".to_string(),
            crate::compatibility_json::string_array_value(&state.text_styles),
        );
        record.insert(
            "start".to_string(),
            state.start.map(Value::Bool).unwrap_or(Value::Null),
        );
        out.insert(id.clone(), Value::Object(record));
    }
    Value::Object(out)
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
    out.insert("centerLabel".to_string(), Value::Bool(true));
    if let Some(explicit_dir) = node.explicit_dir {
        out.insert("explicitDir".to_string(), Value::Bool(explicit_dir));
    }
    if let Some(description) = &node.description {
        out.insert("description".to_string(), json!(description));
    }
    Value::Object(out)
}

fn state_edge_to_compat_json(edge: &StateDiagramRenderEdge, look: &Value) -> Value {
    let note_edge = edge.classes == "transition note-edge";
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
    value
        .map(crate::compatibility_json::number_value)
        .unwrap_or(Value::Null)
}
