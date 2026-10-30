//! Pure Mermaid-compatible JSON projection of declared family data.

use super::UsecaseDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Value, json};

pub fn project(
    model: &UsecaseDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let json_nodes: Vec<_> = model
        .json_nodes
        .iter()
        .map(|node| {
            let mut value = json!({
                "id": node.id, "value": node.value, "propertyOrder": node.property_order,
                "classes": node.classes, "styles": node.styles,
            });
            if let Some(encoding) = node.string_encoding {
                value["stringEncoding"] = json!(encoding);
            }
            value
        })
        .collect();
    Ok(
        json!({ "type": meta.diagram_type, "direction": model.direction,
        "nodes": model.nodes, "boundaries": model.boundaries, "relationships": model.relationships,
        "notes": model.notes, "jsonNodes": json_nodes, "classDefs": model.class_defs,
        "title": model.title, "accTitle": model.acc_title, "accDescr": model.acc_description }),
    )
}
