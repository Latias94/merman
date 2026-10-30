//! Pure projection of frozen family data. Source routing and parse errors stay in the parent.

use merman_model::families::timeline::{TimelineDiagramModel, TimelineOutputData};
use serde_json::{Value, json};

pub(super) fn project(
    model: &TimelineDiagramModel,
    output_data: TimelineOutputData,
    diagram_type: &str,
) -> Value {
    if output_data == TimelineOutputData::Empty {
        return json!({});
    }
    let mut tasks =
        serde_json::to_value(&model.tasks).expect("Timeline tasks must remain JSON-serializable");
    for task in tasks
        .as_array_mut()
        .expect("Timeline tasks must serialize to an array")
    {
        if let Some(task) = task.as_object_mut() {
            task.remove("sectionIndex");
        }
    }
    json!({
        "type": diagram_type,
        "direction": &model.direction,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
        "tasks": tasks,
    })
}
