//! Canonical Journey data and declared full-output facts.

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JourneyTask {
    /// Numeric satisfaction score, retaining finite fractional values and signed infinities.
    ///
    /// Canonical serialization keeps finite scores numeric and encodes infinities as exactly
    /// `"Infinity"` or `"-Infinity"`; deserialization accepts numbers and only those two strings.
    /// Invalid authored scores and direct NaN values use `0.0` with `score_is_nan = true`.
    #[serde(deserialize_with = "deserialize_journey_score")]
    pub score: f64,
    /// Overrides the score's face geometry and mouth selection with the legacy NaN state.
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

fn deserialize_journey_score<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Score {
        Number(f64),
        String(String),
    }

    match <Score as serde::Deserialize>::deserialize(deserializer)? {
        Score::Number(value) => Ok(value),
        Score::String(value) => match value.as_str() {
            "Infinity" => Ok(f64::INFINITY),
            "-Infinity" => Ok(f64::NEG_INFINITY),
            _ => Err(serde::de::Error::custom(
                "expected a number, \"Infinity\", or \"-Infinity\"",
            )),
        },
    }
}

impl serde::Serialize for JourneyTask {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let score_is_nan = self.score_is_nan || self.score.is_nan();
        let mut task = serializer.serialize_struct(
            "JourneyTask",
            5 + usize::from(score_is_nan) + usize::from(self.section_index.is_some()),
        )?;
        if self.score == f64::INFINITY {
            task.serialize_field("score", "Infinity")?;
        } else if self.score == f64::NEG_INFINITY {
            task.serialize_field("score", "-Infinity")?;
        } else {
            task.serialize_field("score", &if self.score.is_nan() { 0.0 } else { self.score })?;
        }
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
            (serde_json::json!(4), 4.0_f64),
            (serde_json::json!(3.5), 3.5),
            (serde_json::json!(2.75), 2.75),
            (serde_json::json!(-0.0), -0.0),
        ] {
            let wire = serde_json::json!({
                "score": score,
                "section": "Checkout",
                "sectionIndex": 0,
                "type": "Checkout",
                "task": "Pay"
            });
            let task: JourneyTask = serde_json::from_value(wire).unwrap();
            assert_eq!(task.score.to_bits(), expected.to_bits());
            assert!(!task.score_is_nan);
            assert_eq!(task.section_index, Some(0));
            let encoded = serde_json::to_value(task).unwrap();
            assert_eq!(encoded["score"], serde_json::json!(expected));
            let roundtrip: JourneyTask = serde_json::from_value(encoded).unwrap();
            assert_eq!(roundtrip.score.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn canonical_nonfinite_scores_preserve_infinity_signs_and_explicit_nan_flags() {
        for (score, explicit_nan, expected_wire, expected_nan) in [
            (f64::NAN, false, serde_json::json!(0.0), true),
            (f64::NAN, true, serde_json::json!(0.0), true),
            (f64::INFINITY, false, serde_json::json!("Infinity"), false),
            (
                f64::NEG_INFINITY,
                false,
                serde_json::json!("-Infinity"),
                false,
            ),
            (f64::INFINITY, true, serde_json::json!("Infinity"), true),
            (
                f64::NEG_INFINITY,
                true,
                serde_json::json!("-Infinity"),
                true,
            ),
            (4.0, true, serde_json::json!(4.0), true),
        ] {
            let task = JourneyTask {
                score,
                score_is_nan: explicit_nan,
                people: vec![],
                section: String::new(),
                section_index: None,
                task_type: String::new(),
                task: "Pay".to_string(),
            };
            let value = serde_json::to_value(task).unwrap();
            assert_eq!(value["score"], expected_wire);
            assert_eq!(value["scoreIsNaN"].as_bool(), expected_nan.then_some(true));
            let restored: JourneyTask = serde_json::from_value(value).unwrap();
            let expected_score = if score.is_nan() { 0.0 } else { score };
            assert_eq!(restored.score.to_bits(), expected_score.to_bits());
            assert_eq!(restored.score_is_nan, expected_nan);

            #[cfg(feature = "compat-json")]
            {
                let model = JourneyDiagramModel {
                    tasks: vec![restored],
                    ..Default::default()
                };
                let projected = compat::project(&model, JourneyOutputData::Model, "journey");
                assert_eq!(
                    projected["tasks"][0]["score"],
                    crate::json::number_value(expected_score),
                    "compatibility projection must replace canonical infinity strings with null",
                );
                assert_eq!(
                    projected["tasks"][0]["scoreIsNaN"].as_bool(),
                    expected_nan.then_some(true),
                );
            }
        }
    }

    #[test]
    fn canonical_scores_reject_other_strings_and_nonnumeric_values() {
        for score in [
            serde_json::json!("+Infinity"),
            serde_json::json!("infinity"),
            serde_json::json!(" Infinity"),
            serde_json::json!("-Infinity "),
            serde_json::json!("inf"),
            serde_json::json!("NaN"),
            serde_json::json!("3.5"),
            serde_json::json!("1e309"),
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!({"Infinity": null}),
        ] {
            let wire = serde_json::json!({
                "score": score,
                "section": "Checkout",
                "type": "Checkout",
                "task": "Pay",
            });
            assert!(serde_json::from_value::<JourneyTask>(wire).is_err());
        }
    }
}

#[cfg(feature = "compat-json")]
pub mod compat;
