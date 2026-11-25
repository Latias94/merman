use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapSectionLayout {
    pub name: String,
    pub depth: i64,
    pub value: f64,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    #[serde(default)]
    pub class_selector: Option<String>,
    #[serde(default)]
    pub css_compiled_styles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapLeafLayout {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub parent_name: Option<String>,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    #[serde(default)]
    pub class_selector: Option<String>,
    #[serde(default)]
    pub css_compiled_styles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapDiagramLayout {
    pub title_height: f64,
    pub width: f64,
    pub height: f64,
    pub use_max_width: bool,
    pub diagram_padding: f64,
    pub show_values: bool,
    pub value_format: String,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub sections: Vec<TreemapSectionLayout>,
    #[serde(default)]
    pub leaves: Vec<TreemapLeafLayout>,
}
