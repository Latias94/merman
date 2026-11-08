//! Canonical Journey data and declared full-output facts.

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JourneyTask {
    /// Numeric satisfaction score, retaining finite fractional values.
    ///
    /// Invalid or non-finite authored scores use `0.0` with `score_is_nan = true`.
    pub score: f64,
    #[serde(default, rename = "scoreIsNaN")]
    pub score_is_nan: bool,
    #[serde(default)]
    pub people: Vec<String>,
    pub section: String,
    /// Index of the authored section occurrence in `JourneyDiagramModel::sections`.
    ///
    /// `None` represents a task without parser-backed occurrence ownership, including tasks
    /// authored before any section and legacy direct models.
    #[serde(default, rename = "sectionIndex")]
    pub section_index: Option<usize>,
    #[serde(rename = "type")]
    pub task_type: String,
    pub task: String,
}

impl serde::Serialize for JourneyTask {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let invalid_score = !self.score.is_finite();
        let score_is_nan = self.score_is_nan || invalid_score;
        let mut task = serializer.serialize_struct(
            "JourneyTask",
            5 + usize::from(score_is_nan) + usize::from(self.section_index.is_some()),
        )?;
        // JSON has no non-finite numbers. Keep direct models round-trippable through the same
        // explicit invalid-score state used by source parsing and compatibility projection.
        task.serialize_field("score", &if invalid_score { 0.0 } else { self.score })?;
        if score_is_nan {
            task.serialize_field("scoreIsNaN", &true)?;
        }
        task.serialize_field("people", &self.people)?;
        task.serialize_field("section", &self.section)?;
        if let Some(index) = self.section_index {
            task.serialize_field("sectionIndex", &index)?;
        }
        task.serialize_field("type", &self.task_type)?;
        task.serialize_field("task", &self.task)?;
        task.end()
    }
}

/// Canonical meaning of a Journey diagram, without compatibility-output state.
///
/// Collection fields retain their canonical deserialization requirements. The retired full
/// wrapper's permissive collection defaults are not a canonical model import contract.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct JourneyDiagramModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<JourneyTask>,
    pub actors: Vec<String>,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JourneyOutputData {
    Empty,
    #[default]
    Model,
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn canonical_wire_stays_flat_and_preserves_collection_requirements() {
        let expected = serde_json::json!({"title": "Retained", "accTitle": null, "accDescr": null, "sections": [], "tasks": [], "actors": []});
        let model: JourneyDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&model).unwrap(), expected);
        assert!(
            serde_json::from_value::<JourneyDiagramModel>(serde_json::json!({"title": "Retained"}))
                .is_err()
        );
        let roundtrip: JourneyDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), expected);
    }

    #[test]
    fn canonical_tasks_accept_integer_and_fractional_scores_without_loss() {
        for (score, expected) in [
            (serde_json::json!(4), 4.0),
            (serde_json::json!(3.5), 3.5),
            (serde_json::json!(2.75), 2.75),
        ] {
            let wire = serde_json::json!({
                "score": score,
                "section": "Checkout",
                "sectionIndex": 0,
                "type": "Checkout",
                "task": "Pay"
            });
            let task: JourneyTask = serde_json::from_value(wire).unwrap();
            assert_eq!(task.score, expected);
            assert!(!task.score_is_nan);
            assert_eq!(task.section_index, Some(0));
            let roundtrip: JourneyTask =
                serde_json::from_value(serde_json::to_value(task).unwrap()).unwrap();
            assert_eq!(roundtrip.score, expected);
        }
    }

    #[test]
    fn canonical_direct_nonfinite_scores_roundtrip_as_explicit_invalid_state() {
        for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let task = JourneyTask {
                score,
                score_is_nan: false,
                people: vec![],
                section: String::new(),
                section_index: None,
                task_type: String::new(),
                task: "Pay".to_string(),
            };
            let value = serde_json::to_value(task).unwrap();
            assert_eq!(value["score"], serde_json::json!(0.0));
            assert_eq!(value["scoreIsNaN"], true);
            let restored: JourneyTask = serde_json::from_value(value).unwrap();
            assert_eq!(restored.score, 0.0);
            assert!(restored.score_is_nan);
        }
    }
}

#[cfg(feature = "compat-json")]
pub mod compat;
