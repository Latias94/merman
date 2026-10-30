//! Canonical XY chart data and explicit output display policy.

use crate::MermaidConfig;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum XyChartAxisModel {
    #[serde(rename = "band")]
    Band {
        #[serde(default)]
        title: String,
        #[serde(default)]
        categories: Vec<String>,
    },
    #[serde(rename = "linear")]
    Linear {
        #[serde(default)]
        title: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XyChartPlotType {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "bar")]
    Bar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyChartPlotModel {
    #[serde(rename = "type")]
    pub plot_type: XyChartPlotType,
    #[serde(default, serialize_with = "serialize_xychart_plot_title")]
    pub title: Option<String>,
    pub values: Vec<f64>,
    pub data: Vec<(String, Option<f64>)>,
    #[serde(rename = "pointLabels", default, skip_serializing_if = "Vec::is_empty")]
    pub point_labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XyChartAxisDisplayPolicy {
    pub show_label: bool,
    pub show_title: bool,
    pub show_tick: bool,
    pub show_axis_line: bool,
}

impl Default for XyChartAxisDisplayPolicy {
    fn default() -> Self {
        Self {
            show_label: true,
            show_title: true,
            show_tick: true,
            show_axis_line: true,
        }
    }
}

impl XyChartAxisDisplayPolicy {
    fn from_config(config: &MermaidConfig, axis_key: &str) -> Self {
        let default = Self::default();
        Self {
            show_label: config
                .get_bool(&format!("xyChart.{axis_key}.showLabel"))
                .unwrap_or(default.show_label),
            show_title: config
                .get_bool(&format!("xyChart.{axis_key}.showTitle"))
                .unwrap_or(default.show_title),
            show_tick: config
                .get_bool(&format!("xyChart.{axis_key}.showTick"))
                .unwrap_or(default.show_tick),
            show_axis_line: config
                .get_bool(&format!("xyChart.{axis_key}.showAxisLine"))
                .unwrap_or(default.show_axis_line),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XyChartDisplayPolicy {
    pub show_title: bool,
    pub show_data_label: bool,
    pub show_data_label_outside_bar: bool,
    pub x_axis: XyChartAxisDisplayPolicy,
    pub y_axis: XyChartAxisDisplayPolicy,
}

impl Default for XyChartDisplayPolicy {
    fn default() -> Self {
        Self {
            show_title: true,
            show_data_label: false,
            show_data_label_outside_bar: false,
            x_axis: XyChartAxisDisplayPolicy::default(),
            y_axis: XyChartAxisDisplayPolicy::default(),
        }
    }
}

impl XyChartDisplayPolicy {
    /// Reads chart visibility from an already-admitted configuration.
    pub fn from_config(config: &MermaidConfig) -> Self {
        let default = Self::default();
        Self {
            show_title: config
                .get_bool("xyChart.showTitle")
                .unwrap_or(default.show_title),
            show_data_label: config
                .get_bool("xyChart.showDataLabel")
                .unwrap_or(default.show_data_label),
            show_data_label_outside_bar: config
                .get_bool("xyChart.showDataLabelOutsideBar")
                .unwrap_or(default.show_data_label_outside_bar),
            x_axis: XyChartAxisDisplayPolicy::from_config(config, "xAxis"),
            y_axis: XyChartAxisDisplayPolicy::from_config(config, "yAxis"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyChartDiagramModel {
    #[serde(default)]
    pub orientation: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(rename = "xAxis")]
    pub x_axis: XyChartAxisModel,
    #[serde(rename = "yAxis")]
    pub y_axis: XyChartAxisModel,
    #[serde(default)]
    pub plots: Vec<XyChartPlotModel>,
}

/// The canonical chart owner plus the requested display policy.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct XyChartDiagramRenderModel {
    pub semantic: XyChartDiagramModel,
    #[serde(skip, default)]
    pub display: XyChartDisplayPolicy,
}

#[cfg(feature = "presentation-data")]
impl std::ops::Deref for XyChartDiagramRenderModel {
    type Target = XyChartDiagramModel;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::DerefMut for XyChartDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl XyChartDiagramModel {
    /// Pairs canonical data with display choices read from the supplied configuration.
    pub fn into_render_model(self, config: &MermaidConfig) -> XyChartDiagramRenderModel {
        XyChartDiagramRenderModel {
            semantic: self,
            display: XyChartDisplayPolicy::from_config(config),
        }
    }
}

fn serialize_xychart_plot_title<S>(
    title: &Option<String>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(title.as_deref().unwrap_or_default())
}
