//! Pure projection of frozen family data. Source routing and parse errors stay in the parent.

use merman_model::families::info::{InfoDiagramModel, InfoOutputData};
use serde_json::{Value, json};

pub(super) fn project(
    model: &InfoDiagramModel,
    output_data: InfoOutputData,
    diagram_type: &str,
) -> Value {
    match output_data {
        InfoOutputData::Empty => json!({}),
        InfoOutputData::Model => json!({"type": diagram_type, "showInfo": model.show_info}),
    }
}
