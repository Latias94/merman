use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyActorLegendLineLayout {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub tspan_x: f64,
    pub text_margin: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyActorLegendItemLayout {
    pub actor: String,
    pub pos: i64,
    pub color: String,
    pub circle_cx: f64,
    pub circle_cy: f64,
    pub circle_r: f64,
    pub label_lines: Vec<JourneyActorLegendLineLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JourneyMouthKind {
    Smile,
    Sad,
    Ambivalent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyTaskActorCircleLayout {
    pub actor: String,
    pub pos: i64,
    pub color: String,
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyTaskLayout {
    pub index: i64,
    pub section: String,
    pub task: String,
    /// JavaScript Number score, including signed infinities.
    ///
    /// Invalid authored Number input retains zero with NaN geometry in `face_cy`.
    pub score: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub num: i64,
    pub people: Vec<String>,
    pub actor_circles: Vec<JourneyTaskActorCircleLayout>,
    pub line_id: String,
    pub line_x1: f64,
    pub line_y1: f64,
    pub line_x2: f64,
    pub line_y2: f64,
    pub face_cx: f64,
    /// Face center y coordinate; `None` represents a NaN score.
    ///
    /// `Some` retains signed infinity from either the score or coordinate arithmetic overflow.
    /// SVG emits its JavaScript spelling; JSON serialization projects it as null.
    pub face_cy: Option<f64>,
    pub mouth: JourneyMouthKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneySectionLayout {
    pub section: String,
    pub num: i64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyLineLayout {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyDiagramLayout {
    pub bounds: Option<Bounds>,
    pub left_margin: f64,
    pub max_actor_label_width: f64,
    pub width: f64,
    pub height: f64,
    pub svg_height: f64,
    #[serde(default = "super::config::default_use_max_width")]
    pub use_max_width: bool,
    pub title: Option<String>,
    pub title_x: f64,
    pub title_y: f64,
    pub actor_legend: Vec<JourneyActorLegendItemLayout>,
    pub sections: Vec<JourneySectionLayout>,
    pub tasks: Vec<JourneyTaskLayout>,
    pub activity_line: JourneyLineLayout,
}
