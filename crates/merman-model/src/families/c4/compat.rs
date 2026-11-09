//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{C4DiagramModel, C4RelationOffset};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

pub fn project(
    model: &C4DiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(11);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("c4Type".to_string(), Value::String(model.c4_type.clone()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("wrap".to_string(), Value::Bool(model.wrap));
    out.insert(
        "layout".to_string(),
        json!({
            "c4ShapeInRow": crate::json::number_value(model.layout.c4_shape_in_row),
            "c4BoundaryInRow": crate::json::number_value(model.layout.c4_boundary_in_row),
        }),
    );
    out.insert("shapes".to_string(), json!(&model.shapes));
    let mut boundaries = json!(&model.boundaries);
    if let Some(boundaries) = boundaries.as_array_mut() {
        for boundary in boundaries {
            let Some(boundary) = boundary.as_object_mut() else {
                continue;
            };
            if boundary.get("alias").and_then(Value::as_str) == Some("global") {
                boundary.entry("tags".to_string()).or_insert(Value::Null);
                boundary.entry("link".to_string()).or_insert(Value::Null);
            }
        }
    }
    out.insert("boundaries".to_string(), boundaries);
    let mut rels = json!(&model.rels);
    if let Some(projected) = rels.as_array_mut() {
        for (relation, projected) in model.rels.iter().zip(projected) {
            for (key, offset) in [
                ("offsetX", &relation.offset_x),
                ("offsetY", &relation.offset_y),
            ] {
                if let Some(C4RelationOffset::Number(value)) = offset {
                    projected[key] = crate::json::number_value(*value);
                }
            }
        }
    }
    out.insert("rels".to_string(), rels);
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
