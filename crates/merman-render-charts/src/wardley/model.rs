use merman_model::families::wardley::WardleySourceStrategy;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WardleyPointLayout {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WardleyLineLayout {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl WardleyLineLayout {
    pub(super) fn between(start: WardleyPointLayout, end: WardleyPointLayout) -> Self {
        Self {
            x1: start.x,
            y1: start.y,
            x2: end.x,
            y2: end.y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WardleyCircleLayout {
    pub center: WardleyPointLayout,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WardleyRectLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub corner_radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WardleyRotationLayout {
    pub degrees: f64,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleyTextAnchor {
    Start,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleyFontWeight {
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleyDominantBaseline {
    Auto,
    Middle,
    Central,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyTextLayout {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub font_size: f64,
    pub font_weight: WardleyFontWeight,
    pub text_anchor: WardleyTextAnchor,
    pub dominant_baseline: Option<WardleyDominantBaseline>,
    pub rotation: Option<WardleyRotationLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyAxesLayout {
    pub x_axis: WardleyLineLayout,
    pub y_axis: WardleyLineLayout,
    pub x_label: WardleyTextLayout,
    pub y_label: WardleyTextLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyStageLayout {
    pub name: String,
    pub start_x: f64,
    pub end_x: f64,
    pub label: WardleyTextLayout,
    pub divider: Option<WardleyLineLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyGridLayout {
    pub ratio: f64,
    pub vertical: WardleyLineLayout,
    pub horizontal: WardleyLineLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyPipelineBoxLayout {
    pub node_id: String,
    pub component_ids: Vec<String>,
    pub rect: WardleyRectLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyPipelineLinkLayout {
    pub source: String,
    pub target: String,
    pub line: WardleyLineLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WardleyFlowMarkersLayout {
    pub start: bool,
    pub end: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyLinkLayout {
    pub source: String,
    pub target: String,
    pub line: WardleyLineLayout,
    pub dashed: bool,
    pub markers: WardleyFlowMarkersLayout,
    pub label: Option<WardleyTextLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyTrendLayout {
    pub node_id: String,
    pub origin: WardleyPointLayout,
    pub target: WardleyPointLayout,
    pub line: WardleyLineLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WardleyNodeShapeLayout {
    Anchor,
    None,
    Circle { circle: WardleyCircleLayout },
    PipelineSquare { rect: WardleyRectLayout },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WardleySourceOverlayLayout {
    Build {
        circle: WardleyCircleLayout,
    },
    Buy {
        circle: WardleyCircleLayout,
    },
    Outsource {
        circle: WardleyCircleLayout,
    },
    Market {
        outer_circle: WardleyCircleLayout,
        connectors: Vec<WardleyLineLayout>,
        dots: Vec<WardleyCircleLayout>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyNodeLayout {
    pub id: String,
    pub label: String,
    pub class_name: Option<String>,
    pub position: WardleyPointLayout,
    pub in_pipeline: bool,
    pub is_pipeline_parent: bool,
    pub source_strategy: Option<WardleySourceStrategy>,
    pub shape: WardleyNodeShapeLayout,
    pub source_overlay: Option<WardleySourceOverlayLayout>,
    pub inertia: Option<WardleyLineLayout>,
    pub label_layout: WardleyTextLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyAnnotationPointLayout {
    pub center: WardleyPointLayout,
    pub radius: f64,
    pub label: WardleyTextLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyAnnotationLayout {
    pub number: f64,
    pub points: Vec<WardleyAnnotationPointLayout>,
    pub segments: Vec<WardleyLineLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyAnnotationsBoxLayout {
    pub rect: Option<WardleyRectLayout>,
    pub lines: Vec<WardleyTextLayout>,
    pub padding: f64,
    pub line_height: f64,
    pub font_size: f64,
    pub max_text_width: f64,
    pub max_text_height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyNoteLayout {
    pub text: WardleyTextLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleyArrowDirection {
    Right,
    Left,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyArrowLayout {
    pub name: String,
    pub direction: WardleyArrowDirection,
    pub origin: WardleyPointLayout,
    pub width: f64,
    pub height: f64,
    pub head_width: f64,
    pub path: Vec<WardleyPointLayout>,
    pub label: WardleyTextLayout,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WardleyDiagramLayout {
    pub width: f64,
    pub height: f64,
    pub use_max_width: bool,
    pub padding: f64,
    pub chart_width: f64,
    pub chart_height: f64,
    pub node_radius: f64,
    pub node_label_offset: f64,
    pub axis_font_size: f64,
    pub label_font_size: f64,
    pub square_size: f64,
    pub title: Option<WardleyTextLayout>,
    pub axes: WardleyAxesLayout,
    pub stages: Vec<WardleyStageLayout>,
    pub grid: Vec<WardleyGridLayout>,
    pub pipeline_boxes: Vec<WardleyPipelineBoxLayout>,
    pub pipeline_links: Vec<WardleyPipelineLinkLayout>,
    pub links: Vec<WardleyLinkLayout>,
    pub trends: Vec<WardleyTrendLayout>,
    pub nodes: Vec<WardleyNodeLayout>,
    pub annotations: Vec<WardleyAnnotationLayout>,
    pub annotations_box: Option<WardleyAnnotationsBoxLayout>,
    pub notes: Vec<WardleyNoteLayout>,
    pub accelerators: Vec<WardleyArrowLayout>,
    pub deaccelerators: Vec<WardleyArrowLayout>,
}
