#[cfg(feature = "presentation-data")]
use super::*;
#[cfg(feature = "presentation-data")]
use serde_json::json;

#[cfg(feature = "presentation-data")]
#[test]
fn full_output_data_preserves_empty_without_changing_typed_serialization() {
    let manual = GanttDiagramRenderModel::default();
    let empty = GanttDiagramRenderModel::from_output_data(
        GanttDiagramModel::default(),
        String::new(),
        GanttOutputData::Empty,
    );
    assert_eq!(manual.output_data(), GanttOutputData::Model);
    assert_eq!(empty.output_data(), GanttOutputData::Empty);
    assert_eq!(empty.clone().output_data(), GanttOutputData::Empty);
    assert_eq!(
        serde_json::to_value(&manual).unwrap(),
        serde_json::to_value(&empty).unwrap()
    );
    let deserialized: GanttDiagramRenderModel =
        serde_json::from_value(serde_json::to_value(&empty).unwrap()).unwrap();
    assert_eq!(deserialized.output_data(), GanttOutputData::Model);
}

#[cfg(feature = "presentation-data")]
#[test]
fn gantt_direct_task_constraints_have_explicit_json_and_reject_ambiguous_legacy_data() {
    let task = GanttRenderTask {
        semantic: GanttTask {
            start_constraint: GanttTaskStartConstraint::After {
                dependency_ids: vec!["design".to_string(), "review".to_string()],
            },
            end_constraint: GanttTaskEndConstraint::Until {
                dependency_ids: vec!["release".to_string()],
            },
            ..GanttTask::default()
        },
        ..GanttRenderTask::default()
    };
    let mut value = serde_json::to_value(&task).unwrap();

    assert_eq!(value["startConstraint"]["kind"], "after");
    assert_eq!(
        value["startConstraint"]["dependencyIds"],
        json!(["design", "review"])
    );
    assert_eq!(value["endConstraint"]["kind"], "until");
    assert_eq!(value["endConstraint"]["dependencyIds"], json!(["release"]));

    let object = value.as_object_mut().unwrap();
    object.remove("startConstraint");
    object.remove("endConstraint");
    assert!(
        serde_json::from_value::<GanttRenderTask>(value).is_err(),
        "legacy task JSON must not silently discard constraints stored only in raw fields"
    );
}
