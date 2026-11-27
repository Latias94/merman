use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttAxisTickLayout {
    pub time_ms: i64,
    pub x: f64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttExcludeRangeLayout {
    pub id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttSectionTitleLayout {
    pub section: String,
    pub index: i64,
    pub x: f64,
    pub y: f64,
    pub dy_em: f64,
    pub lines: Vec<String>,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttRowLayout {
    pub index: i64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttTaskLabelLayout {
    pub id: String,
    pub text: String,
    pub font_size: f64,
    pub width: f64,
    pub x: f64,
    pub y: f64,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttTaskBarLayout {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rx: f64,
    pub ry: f64,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttTaskLayout {
    pub id: String,
    pub task: String,
    pub section: String,
    pub task_type: String,
    pub order: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    #[serde(default)]
    pub render_end_ms: Option<i64>,
    pub milestone: bool,
    pub vert: bool,
    pub bar: GanttTaskBarLayout,
    pub label: GanttTaskLabelLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub left_padding: f64,
    pub right_padding: f64,
    pub top_padding: f64,
    pub grid_line_start_padding: f64,
    pub bar_height: f64,
    pub bar_gap: f64,
    pub title_top_margin: f64,
    pub font_size: f64,
    pub section_font_size: f64,
    pub number_section_styles: i64,
    pub display_mode: String,
    pub date_format: String,
    pub axis_format: String,
    #[serde(default)]
    pub tick_interval: Option<String>,
    pub top_axis: bool,
    pub today_marker: String,
    pub categories: Vec<String>,
    pub rows: Vec<GanttRowLayout>,
    pub section_titles: Vec<GanttSectionTitleLayout>,
    pub tasks: Vec<GanttTaskLayout>,
    pub excludes: Vec<GanttExcludeRangeLayout>,
    #[serde(default)]
    pub has_excludes_layer: bool,
    pub bottom_ticks: Vec<GanttAxisTickLayout>,
    #[serde(default)]
    pub top_ticks: Vec<GanttAxisTickLayout>,
    pub title: Option<String>,
    pub title_x: f64,
    pub title_y: f64,
}
