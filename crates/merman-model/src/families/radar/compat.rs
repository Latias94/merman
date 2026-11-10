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
    out.insert(
        "curves".to_string(),
        Value::Array(
            model
                .curves
                .iter()
                .map(|curve| {
                    json!({
                        "name": &curve.name,
                        "label": &curve.label,
                        "entries": curve.entries.iter().copied().map(crate::json::number_value).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        ),
    );
    out.insert(
        "options".to_string(),
        json!({
            "showLegend": model.options.show_legend,
            "ticks": crate::json::number_value(model.options.ticks),
            "max": model.options.max.map(crate::json::number_value),
            "min": crate::json::number_value(model.options.min),
            "graticule": &model.options.graticule,
        }),
    );
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(config.as_value()),
    );
    Value::Object(out)
}
