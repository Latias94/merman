//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{InfoDiagramModel, InfoOutputData};
use serde_json::{Value, json};

pub fn project(model: &InfoDiagramModel, output_data: InfoOutputData, diagram_type: &str) -> Value {
    match output_data {
        InfoOutputData::Empty => json!({}),
        InfoOutputData::Model => json!({"type": diagram_type, "showInfo": model.show_info}),
    }
}
