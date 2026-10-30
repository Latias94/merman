//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{PacketDiagramModel, PacketOutputData};
use serde_json::{Value, json};

pub fn project(
    model: &PacketDiagramModel,
    output_data: PacketOutputData,
    diagram_type: &str,
    config: &crate::config::MermaidConfig,
) -> Value {
    if output_data == PacketOutputData::Empty {
        return json!({});
    }
    let mut out = serde_json::Map::with_capacity(6);
    out.insert("type".to_string(), Value::String(diagram_type.to_string()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("packet".to_string(), json!(&model.packet));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(config.as_value()),
    );
    Value::Object(out)
}
