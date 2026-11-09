use super::{Node, TitleKind};
use crate::diagrams::shapes::is_valid_pinned_shape;
use serde_json::Value;

pub(super) fn value_to_string(v: &Value) -> Option<String> {
    crate::inline_config::value_to_string(v)
}

pub(super) fn value_to_bool(v: &Value) -> Option<bool> {
    crate::inline_config::value_to_bool(v)
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
                return Ok(Some(
                    crate::utils::parse_js_number(value).unwrap_or(f64::NAN),
                ));
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
    document: &Value,
) -> std::result::Result<(), String> {
    let map = match document.as_object() {
        Some(m) => m,
        None => return Ok(()),
    };

    let mut provided_label: Option<String> = None;
    let mut provided_label_type: Option<TitleKind> = None;
    let mut provided_visual = false;
    for (k, v) in map {
        match k.as_str() {
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
                    node.label_span = None;
                    node.label_selection = None;
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
            node.label_span = None;
            node.label_selection = None;
        }
    }

    Ok(())
}
