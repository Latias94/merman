use super::{FlowchartAst, Node, StatementEvent, Stmt, TitleKind};
use merman_core_preprocess::yaml::parse_mermaid_inline_yaml_with_root_order_controlled;
pub(super) use merman_model::config::inline_config_scalar_to_string as value_to_string;
use merman_model::{
    OperationControl, OperationControlResult, families::flowchart::shapes::is_valid_pinned_shape,
    number::parse_js_number,
};
use serde_json::Value;
use std::collections::HashMap;

pub(super) struct ShapeDataDocument {
    pub(super) value: Value,
    pub(super) key_order: Vec<String>,
}

pub(super) fn value_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn metadata_string(value: &Value) -> Option<String> {
    match value {
        Value::Null | Value::Bool(false) => None,
        Value::Number(number) if number.as_f64() == Some(0.0) => None,
        _ => value_to_string(value).filter(|text| !text.is_empty()),
    }
}

fn has_noncallable_asset_to_string(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(has_noncallable_asset_to_string),
        Value::Object(values) => values.contains_key("toString"),
        _ => false,
    }
}

fn asset_dimension(v: &Value) -> std::result::Result<Option<f64>, String> {
    if has_noncallable_asset_to_string(v) {
        return Err("Cannot convert object to primitive value".to_string());
    }
    let mut value = match v {
        Value::Null | Value::Bool(false) => return Ok(None),
        Value::Number(value) => return Ok(value.as_f64().filter(|value| *value != 0.0)),
        Value::Bool(true) => return Ok(Some(1.0)),
        Value::String(value) if value.is_empty() => return Ok(None),
        _ => v,
    };
    loop {
        match value {
            Value::Array(values) => match values.as_slice() {
                [] => return Ok(Some(0.0)),
                [item] => value = item,
                _ => return Ok(Some(f64::NAN)),
            },
            Value::Null => return Ok(Some(0.0)),
            Value::Number(value) => return Ok(value.as_f64()),
            Value::String(value) => {
                return Ok(Some(parse_js_number(value).unwrap_or(f64::NAN)));
            }
            _ => return Ok(Some(f64::NAN)),
        }
    }
}

fn sanitize_shape_label_type(label_type: Option<&str>) -> TitleKind {
    match label_type {
        Some("text") => TitleKind::Text,
        Some("string") => TitleKind::String,
        Some("markdown") => TitleKind::Markdown,
        _ => TitleKind::Markdown,
    }
}

pub(super) fn apply_shape_data_value_to_node(
    node: &mut Node,
    document: &ShapeDataDocument,
) -> std::result::Result<(), String> {
    let mut provided_label: Option<String> = None;
    let mut provided_label_type: Option<TitleKind> = None;
    let mut provided_visual = false;
    // Mermaid applies authored fields in order, including their coercion and shape errors.
    for key in &document.key_order {
        let Some(v) = document.value.get(key) else {
            continue;
        };
        match key.as_str() {
            "shape" => {
                let Some(shape) = v.as_str().filter(|shape| !shape.is_empty()) else {
                    continue;
                };
                // Mermaid rejects camelCase and underscore shapeData before shape-map lookup.
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }
                if !is_valid_pinned_shape(shape) {
                    return Err(format!("No such shape: {shape}."));
                }
                node.shape = Some(shape.to_string());
            }
            "label" => {
                if let Some(label) = metadata_string(v) {
                    provided_label = Some(label.clone());
                    node.label = Some(label);
                }
            }
            "labelType" => {
                provided_label_type =
                    Some(sanitize_shape_label_type(value_to_string(v).as_deref()));
            }
            "icon" => {
                if let Some(icon) = metadata_string(v) {
                    node.icon = Some(icon);
                    provided_visual = true;
                }
            }
            "form" => {
                if let Some(form) = metadata_string(v) {
                    node.form = Some(form);
                }
            }
            "pos" => {
                if let Some(pos) = metadata_string(v) {
                    node.pos = Some(pos);
                }
            }
            "img" => {
                if let Some(img) = metadata_string(v) {
                    node.img = Some(img);
                    provided_visual = true;
                }
            }
            "constraint" => {
                if let Some(constraint) = metadata_string(v) {
                    node.constraint = Some(constraint);
                }
            }
            "w" => {
                if let Some(w) = asset_dimension(v)? {
                    node.asset_width = Some(w);
                }
            }
            "h" => {
                if let Some(h) = asset_dimension(v)? {
                    node.asset_height = Some(h);
                }
            }
            _ => {}
        }
    }
    if provided_label.is_some() {
        node.label_type = provided_label_type.unwrap_or(TitleKind::Markdown);
    }

    // Mermaid clears the default label when an icon or img is set without an explicit label.
    let label_is_empty_or_missing = provided_label
        .as_deref()
        .map(|label| {
            label
                .trim_matches(super::is_ecmascript_trim_char)
                .is_empty()
        })
        .unwrap_or(true);
    if provided_visual && label_is_empty_or_missing {
        let current_text = node.label.as_deref().unwrap_or(node.id.as_str());
        if current_text == node.id {
            node.label = Some(String::new());
        }
    }

    Ok(())
}

pub(super) fn prepare_flowchart_shape_data(
    ast: &FlowchartAst,
    control: &OperationControl,
) -> OperationControlResult<HashMap<String, std::result::Result<ShapeDataDocument, String>>> {
    let mut documents = HashMap::new();
    for (visited, event) in ast.walk().enumerate() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let StatementEvent::Enter(statement) = event else {
            continue;
        };

        match statement {
            Stmt::Chain { node_groups, .. } => {
                for (index, node) in node_groups.iter().flatten().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    if let Some(source) = node.shape_data.as_deref() {
                        prepare_flowchart_shape_data_document(source, control, &mut documents)?;
                    }
                }
            }
            Stmt::Node(node) => {
                if let Some(source) = node.shape_data.as_deref() {
                    prepare_flowchart_shape_data_document(source, control, &mut documents)?;
                }
            }
            Stmt::Subgraph(_) | Stmt::AccessibilityValue(_) => {}
            Stmt::ShapeData { yaml, .. } => {
                prepare_flowchart_shape_data_document(yaml, control, &mut documents)?;
            }
            Stmt::Direction(_)
            | Stmt::Style(_)
            | Stmt::ClassDef(_)
            | Stmt::ClassAssign(_)
            | Stmt::Click(_)
            | Stmt::LinkStyle(_) => {}
        }
    }

    control.checkpoint()?;
    Ok(documents)
}

fn prepare_flowchart_shape_data_document(
    source: &str,
    control: &OperationControl,
    documents: &mut HashMap<String, std::result::Result<ShapeDataDocument, String>>,
) -> OperationControlResult<()> {
    if documents.contains_key(source) {
        return control.checkpoint();
    }
    let document = parse_mermaid_inline_yaml_with_root_order_controlled(source, control)?.map(
        |(mut value, key_order)| {
            if let Value::Object(map) = &mut value {
                for key in ["laneRole", "laneIndex", "drawnExtent"] {
                    map.remove(key);
                }
            }
            ShapeDataDocument { value, key_order }
        },
    );
    documents.insert(source.to_string(), document);
    Ok(())
}
