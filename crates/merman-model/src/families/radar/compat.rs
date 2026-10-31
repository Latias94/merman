//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{RadarDiagramModel, RadarOutputData};
use serde_json::{Value, json};

pub fn project(
    model: &RadarDiagramModel,
    output_data: RadarOutputData,
    diagram_type: &str,
    config: &crate::config::MermaidConfig,
) -> Value {
    if output_data == RadarOutputData::Empty {
        return json!({});
    }
    let mut out = serde_json::Map::with_capacity(8);
    out.insert("type".to_string(), Value::String(diagram_type.to_string()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("axes".to_string(), json!(&model.axes));
    out.insert("curves".to_string(), json!(&model.curves));
    out.insert("options".to_string(), json!(&model.options));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(config.as_value()),
    );
    Value::Object(out)
}
