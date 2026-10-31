//! Canonical Info data and declared full-output facts.

/// Canonical meaning of an Info diagram, without compatibility-output state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct InfoDiagramModel {
    #[serde(rename = "showInfo")]
    pub show_info: bool,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InfoOutputData {
    Empty,
    #[default]
    Model,
}

#[cfg(feature = "compat-json")]
pub mod compat;
