//! Canonical Timeline data and declared full-output facts.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimelineTask {
    pub id: i64,
    pub section: String,
    /// Index of the authored section occurrence in `TimelineDiagramModel::sections`.
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
    pub score: i64,
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TimelineDirection {
    #[default]
    #[serde(rename = "LR")]
    LeftToRight,
    #[serde(rename = "TD")]
    TopDown,
}

/// Canonical meaning of a Timeline diagram, without compatibility-output state.
///
/// Collection fields retain their canonical deserialization requirements. The retired full
/// wrapper's permissive collection defaults are not a canonical model import contract.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TimelineDiagramModel {
    #[serde(default)]
    pub direction: TimelineDirection,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<TimelineTask>,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineOutputData {
    Empty,
    #[default]
    Model,
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn canonical_wire_stays_flat_and_preserves_collection_requirements() {
        let expected = serde_json::json!({"direction": "LR", "title": "Retained", "accTitle": null, "accDescr": null, "sections": [], "tasks": []});
        let model: TimelineDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&model).unwrap(), expected);
        assert!(
            serde_json::from_value::<TimelineDiagramModel>(
                serde_json::json!({"title": "Retained"})
            )
            .is_err()
        );
        let roundtrip: TimelineDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), expected);
    }
}

#[cfg(feature = "compat-json")]
pub mod compat;
