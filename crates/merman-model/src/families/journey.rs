//! Canonical Journey data and declared full-output facts.

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JourneyTask {
    pub score: i64,
    #[serde(default, rename = "scoreIsNaN", skip_serializing_if = "is_false")]
    pub score_is_nan: bool,
    #[serde(default)]
    pub people: Vec<String>,
    pub section: String,
    /// Index of the authored section occurrence in `JourneyDiagramModel::sections`.
    ///
    /// `None` represents a task without parser-backed occurrence ownership, including tasks
    /// authored before any section and legacy direct models.
    #[serde(
        default,
        rename = "sectionIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub section_index: Option<usize>,
    #[serde(rename = "type")]
    pub task_type: String,
    pub task: String,
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
}
