use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XyChartRectData {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    #[serde(rename = "strokeFill")]
    pub stroke_fill: String,
    #[serde(rename = "strokeWidth")]
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XyChartTextData {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub fill: String,
    #[serde(rename = "fontSize")]
    pub font_size: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(rename = "verticalPos")]
    pub vertical_pos: String,
    #[serde(rename = "horizontalPos")]
    pub horizontal_pos: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XyChartPathData {
    pub path: String,
    #[serde(default)]
    pub fill: Option<String>,
    #[serde(rename = "strokeFill")]
    pub stroke_fill: String,
    #[serde(rename = "strokeWidth")]
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum XyChartDrawableElem {
    #[serde(rename = "rect")]
    Rect {
        #[serde(rename = "groupTexts")]
        group_texts: Vec<String>,
        data: Vec<XyChartRectData>,
    },
    #[serde(rename = "text")]
    Text {
        #[serde(rename = "groupTexts")]
        group_texts: Vec<String>,
        data: Vec<XyChartTextData>,
    },
    #[serde(rename = "path")]
    Path {
        #[serde(rename = "groupTexts")]
        group_texts: Vec<String>,
        data: Vec<XyChartPathData>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XyChartDiagramLayout {
    pub width: f64,
    pub height: f64,
    #[serde(rename = "chartOrientation")]
    pub chart_orientation: String,
    #[serde(rename = "showDataLabel")]
    pub show_data_label: bool,
    #[serde(rename = "showDataLabelOutsideBar", default)]
    pub show_data_label_outside_bar: bool,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
    #[serde(rename = "labelData")]
    pub label_data: Vec<String>,
    #[serde(default)]
    pub drawables: Vec<XyChartDrawableElem>,
}
