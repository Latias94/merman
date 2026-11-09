//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{QuadrantChartDiagramModel, QuadrantChartStyles};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

fn project_styles(styles: &QuadrantChartStyles) -> Value {
    json!({
        "radius": styles.radius.map(crate::json::number_value),
        "color": &styles.color,
        "strokeColor": &styles.stroke_color,
        "strokeWidth": &styles.stroke_width,
    })
}

pub fn project(
    model: &QuadrantChartDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(9);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("quadrants".to_string(), json!(&model.quadrants));
    out.insert("axes".to_string(), json!(&model.axes));
    out.insert(
        "points".to_string(),
        Value::Array(
            model
                .points
                .iter()
                .map(|point| {
                    json!({
                        "text": &point.text,
                        "x": point.x,
                        "y": point.y,
                        "className": &point.class_name,
                        "styles": project_styles(&point.styles),
                    })
                })
                .collect(),
        ),
    );
    out.insert(
        "classes".to_string(),
        Value::Object(
            model
                .classes
                .iter()
                .map(|(name, styles)| (name.clone(), project_styles(styles)))
                .collect(),
        ),
    );
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
