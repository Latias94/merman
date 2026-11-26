use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaLineLayout {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub class_name: String,
    pub marker_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaTextLayout {
    pub text: String,
    pub lines: Vec<String>,
    pub class_name: String,
    pub x: f64,
    pub y: f64,
    pub anchor: String,
    pub line_height: f64,
    pub font_size: f64,
    pub bbox: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaLabelBoxLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaCauseLabelGroupLayout {
    pub label_box: IshikawaLabelBoxLayout,
    pub label: IshikawaTextLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaSubGroupLayout {
    pub line: IshikawaLineLayout,
    pub label: IshikawaTextLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaBranchLayout {
    pub line: IshikawaLineLayout,
    pub label_group: IshikawaCauseLabelGroupLayout,
    pub sub_groups: Vec<IshikawaSubGroupLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaPairLayout {
    pub upper: IshikawaBranchLayout,
    pub lower: Option<IshikawaBranchLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaHeadLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub path_d: String,
    pub label: IshikawaTextLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IshikawaDiagramLayout {
    pub bounds: Option<Bounds>,
    pub total_width: f64,
    pub total_height: f64,
    pub viewbox_x: f64,
    pub viewbox_y: f64,
    pub padding: f64,
    pub use_max_width: bool,
    pub font_size: f64,
    pub head: Option<IshikawaHeadLayout>,
    pub spine: Option<IshikawaLineLayout>,
    #[serde(default)]
    pub pairs: Vec<IshikawaPairLayout>,
}
