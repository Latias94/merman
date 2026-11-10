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
    for (task, source) in tasks
        .as_array_mut()
        .expect("Journey tasks must serialize to an array")
        .iter_mut()
        .zip(&model.tasks)
    {
        if let Some(task) = task.as_object_mut() {
            task.remove("sectionIndex");
            // Retain the historical integer wire shape without truncating fractional scores.
            let score = source.score;
            if source.score_is_nan || score.is_nan() {
                task.insert("scoreIsNaN".to_string(), json!(true));
            } else {
                task.remove("scoreIsNaN");
            }
            let value = if score.is_nan() {
                json!(0)
            } else if score.is_infinite() {
                // JSON.stringify emits null for infinities without making them NaN scores.
                Value::Null
            } else if score.fract() == 0.0 && score >= i64::MIN as f64 && score < -(i64::MIN as f64)
            {
                json!(score as i64)
            } else {
                json!(score)
            };
            task.insert("score".to_string(), value);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::journey::JourneyTask;

    #[test]
    fn scores_preserve_fractional_values_integer_wire_shapes_and_nan_state() {
        for (score, score_is_nan, expected, expected_nan) in [
            (4.0, false, json!(4), false),
            (3.5, false, json!(3.5), false),
            (2.75, false, json!(2.75), false),
            (-0.0, false, json!(0), false),
            (0.0, true, json!(0), true),
            (4.0, true, json!(4), true),
            (f64::NAN, false, json!(0), true),
            (f64::INFINITY, false, Value::Null, false),
            (f64::NEG_INFINITY, false, Value::Null, false),
            // The first f64 at the positive i64 boundary must not saturate to i64::MAX.
            (-(i64::MIN as f64), false, json!(-(i64::MIN as f64)), false),
        ] {
            let model = JourneyDiagramModel {
                tasks: vec![JourneyTask {
                    score,
                    score_is_nan,
                    people: vec![],
                    section: "Checkout".to_string(),
                    section_index: Some(0),
                    task_type: "Checkout".to_string(),
                    task: "Pay".to_string(),
                }],
                ..Default::default()
            };
            let projection = project(&model, JourneyOutputData::Model, "journey");
            let task = &projection["tasks"][0];
            assert_eq!(task["score"], expected, "score {score:?}");
            assert_eq!(task["scoreIsNaN"].as_bool(), expected_nan.then_some(true));
            assert!(task.get("sectionIndex").is_none());
        }
    }
}
