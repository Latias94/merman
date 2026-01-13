use crate::{ParseMetadata, Result};
use serde_json::{Value, json};

pub fn parse_error(_code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(json!({
        "type": meta.diagram_type,
    }))
}
