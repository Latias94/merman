//! Pure projection of frozen family data. Source routing and parse errors stay in the parent.

use merman_model::families::pie::{PieDiagramModel, PieOutputData};
use serde_json::{Value, json};

pub(super) fn project(
    model: &PieDiagramModel,
    output_data: PieOutputData,
    diagram_type: &str,
) -> Value {
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
