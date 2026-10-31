//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{JourneyDiagramModel, JourneyOutputData};
use serde_json::{Value, json};

pub fn project(
    model: &JourneyDiagramModel,
    output_data: JourneyOutputData,
    diagram_type: &str,
) -> Value {
    if output_data == JourneyOutputData::Empty {
        return json!({});
    }
    let mut tasks =
        serde_json::to_value(&model.tasks).expect("Journey tasks must remain JSON-serializable");
    for task in tasks
        .as_array_mut()
        .expect("Journey tasks must serialize to an array")
    {
        if let Some(task) = task.as_object_mut() {
            task.remove("sectionIndex");
        }
    }
    json!({
        "type": diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
        "tasks": tasks,
        "actors": &model.actors,
    })
}
