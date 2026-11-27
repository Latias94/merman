use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineNodeLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Width used for wrapping (excluding padding).
    pub content_width: f64,
    /// Padding used to compute `width` (Mermaid 11.12.2 uses 20).
    pub padding: f64,
    pub section_class: String,
    pub label: String,
    /// Wrapped lines as rendered into `<tspan>` nodes.
    pub label_lines: Vec<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineLineLayout {
    pub kind: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineTaskLayout {
    pub node: TimelineNodeLayout,
    #[serde(default)]
    pub connectors: Vec<TimelineLineLayout>,
    pub events: Vec<TimelineNodeLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSectionLayout {
    pub node: TimelineNodeLayout,
    pub tasks: Vec<TimelineTaskLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineDiagramLayout {
    pub direction: merman_model::families::timeline::TimelineDirection,
    pub bounds: Option<Bounds>,
    pub left_margin: f64,
    pub base_x: f64,
    pub base_y: f64,
    /// `svg.node().getBBox().width` computed *before* title/activity line are inserted.
    pub pre_title_box_width: f64,
    pub sections: Vec<TimelineSectionLayout>,
    #[serde(default)]
    pub orphan_tasks: Vec<TimelineTaskLayout>,
    pub activity_line: TimelineLineLayout,
    pub title: Option<String>,
    pub title_x: f64,
    pub title_y: f64,
    #[serde(default = "default_use_max_width")]
    pub use_max_width: bool,
}

fn default_use_max_width() -> bool {
    super::config::DEFAULT_USE_MAX_WIDTH
}
