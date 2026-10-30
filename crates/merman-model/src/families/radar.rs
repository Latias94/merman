//! Canonical Radar data and declared full-output facts.

use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarAxis {
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarCurve {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub entries: Vec<Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarOptions {
    #[serde(rename = "showLegend")]
    pub show_legend: bool,
    pub ticks: Value,
    pub max: Option<Value>,
    pub min: Value,
    pub graticule: String,
}

impl Default for RadarOptions {
    fn default() -> Self {
        Self {
            show_legend: true,
            ticks: Value::from(5),
            max: None,
            min: Value::from(0),
            graticule: "circle".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RadarDiagramModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub axes: Vec<RadarAxis>,
    #[serde(default)]
    pub curves: Vec<RadarCurve>,
    #[serde(default)]
    pub options: RadarOptions,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadarOutputData {
    Empty,
    #[default]
    Model,
}
