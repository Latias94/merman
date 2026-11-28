use merman_render_core::{Bounds, LayoutPoint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub(crate) struct C4TextRowLayout {
    /// Source words emitted into one Mermaid `createText` outer tspan.
    ///
    /// Keeping the source words (rather than only a joined string) preserves the same
    /// whitespace/token boundaries for layout and SVG emission.
    pub(crate) words: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct C4UnifiedTextRows {
    /// Wrapped source-word rows consumed by the C4 SVG emitter.
    pub(crate) rows: Vec<C4TextRowLayout>,
    /// The section bbox x offset relative to the centered outer tspan anchor.
    pub(crate) bbox_x: f64,
    /// The section bbox y offset produced by Mermaid's createText DOM shape.
    pub(crate) bbox_y: f64,
}

#[derive(Debug, Clone)]
pub(crate) enum C4TextRenderPlan {
    Unified(C4UnifiedTextRows),
    /// Measured relation text bounds relative to the block's baseline.
    #[cfg(feature = "layout-shoal")]
    RelationInk(kurbo::Rect),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4TextBlockLayout {
    pub text: String,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
    /// In-memory paint geometry for unified text and routed captions. Legacy text and the
    /// serialized C4 layout retain their historical representation.
    #[serde(skip)]
    pub(crate) render_plan: Option<C4TextRenderPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4ImageLayout {
    pub width: f64,
    pub height: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4ShapeLayout {
    pub alias: String,
    pub parent_boundary: String,
    pub type_c4_shape: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub margin: f64,
    pub image: C4ImageLayout,
    pub type_block: C4TextBlockLayout,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub ty: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub techn: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4BoundaryLayout {
    pub alias: String,
    pub parent_boundary: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub image: C4ImageLayout,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub ty: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4RelLayout {
    pub from: String,
    pub to: String,
    pub rel_type: String,
    pub start_point: LayoutPoint,
    pub end_point: LayoutPoint,
    #[serde(default)]
    pub offset_x: Option<merman_model::families::c4::C4RelationOffset>,
    #[serde(default)]
    pub offset_y: Option<merman_model::families::c4::C4RelationOffset>,
    pub label: C4TextBlockLayout,
    #[serde(default)]
    pub techn: Option<C4TextBlockLayout>,
    #[serde(default)]
    pub descr: Option<C4TextBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct C4DiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    #[serde(default = "super::config::default_use_max_width")]
    pub use_max_width: bool,
    pub container_width: f64,
    pub container_height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_available_width: Option<f64>,
    pub c4_type: String,
    pub title: Option<String>,
    pub boundaries: Vec<C4BoundaryLayout>,
    pub shapes: Vec<C4ShapeLayout>,
    pub rels: Vec<C4RelLayout>,
}
