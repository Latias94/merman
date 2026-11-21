//! Pure Mermaid-compatible JSON projection of declared family data.

use super::ClassDiagram;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};

pub fn project(
    model: &ClassDiagram,
    meta: &CompatJsonMetadata<'_>,
) -> Result<serde_json::Value, CompatProjectionError> {
    let mut value =
        serde_json::to_value(model).expect("Class typed model must remain JSON-serializable");
    if let Some(relations) = value
        .get_mut("relations")
        .and_then(serde_json::Value::as_array_mut)
    {
        for relation in relations {
            if let Some(relation) = relation.as_object_mut() {
                for field in ["relationTitle1", "relationTitle2"] {
                    if relation.get(field).is_some_and(serde_json::Value::is_null) {
                        relation.insert(
                            field.to_string(),
                            serde_json::Value::String("none".to_string()),
                        );
                    }
                }
            }
        }
    }
    let object = value
        .as_object_mut()
        .expect("Class typed model must serialize to a JSON object");
    object.remove("namespaceFacadeAliases");
    object.insert(
        "type".to_string(),
        serde_json::Value::String(meta.diagram_type.to_string()),
    );
    Ok(value)
}
