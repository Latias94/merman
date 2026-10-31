//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{PieDiagramModel, PieOutputData};
use serde_json::{Value, json};

pub fn project(model: &PieDiagramModel, output_data: PieOutputData, diagram_type: &str) -> Value {
    if output_data == PieOutputData::Empty {
        return json!({});
    }
    json!({
        "type": diagram_type,
        "showData": &model.show_data,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
    })
}
