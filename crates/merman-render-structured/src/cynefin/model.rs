use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CynefinDomainLayout {
    pub name: String,
    pub cx: f64,
    pub cy: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CynefinItemLayout {
    pub domain: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text_x: f64,
    pub text_y: f64,
    pub overflow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CynefinTransitionLayout {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub cpx: f64,
    pub cpy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CynefinDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub total_width: f64,
    pub total_height: f64,
    pub use_max_width: bool,
    pub show_domain_descriptions: bool,
    pub boundary_amplitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<f64>,
    pub domain_layouts: Vec<CynefinDomainLayout>,
    pub items: Vec<CynefinItemLayout>,
    pub transitions: Vec<CynefinTransitionLayout>,
}
