//! Pure projection of frozen family data. Source routing and parse errors stay in the parent.

use merman_model::families::radar::{RadarDiagramModel, RadarOutputData};
use serde_json::{Value, json};

pub(super) fn project(
    model: &RadarDiagramModel,
    output_data: RadarOutputData,
    diagram_type: &str,
    config: &merman_model::config::MermaidConfig,
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
        merman_model::json::clone_value_nonrecursive(config.as_value()),
    );
    Value::Object(out)
}
