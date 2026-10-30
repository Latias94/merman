//! Canonical Pie data and declared full-output facts.

/// Canonical meaning of a Pie diagram, without compatibility-output state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PieDiagramModel {
    #[serde(rename = "showData")]
    pub show_data: bool,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<PieSection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PieSection {
    pub label: String,
    pub value: f64,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PieOutputData {
    Empty,
    #[default]
    Model,
}
