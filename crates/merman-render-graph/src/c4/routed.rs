use euclid::default::Size2D;
use kurbo::Rect;
use merman_model::families::c4::C4DiagramModel;
use merman_render_core::{Bounds, LayoutPoint};

use super::{
    C4BoundaryLayout, C4DiagramLayout, C4LayoutSettings, C4RelLayout, C4ShapeLayout,
    C4TextBlockLayout,
};

/// The placed C4 facts used by SVG emission when the facade selects Shoal.
#[derive(Debug)]
pub struct C4RoutedGeometry {
    pub boundary_title_x: Vec<Option<f64>>,
    pub relations: Vec<C4RoutedRelation>,
}

#[derive(Debug)]
pub struct C4RoutedRelation {
    pub id: String,
    pub path: String,
    pub points: Vec<LayoutPoint>,
    pub label_origin: Option<LayoutPoint>,
    /// SVG-only endpoint reference for a caption placed outside the drawing.
    pub label_reference: Option<Box<C4TextBlockLayout>>,
    pub shaft_cutout: Option<Rect>,
}

impl C4DiagramLayout {
    /// Projects placed geometry into the existing public layout shape.
    #[allow(
        clippy::too_many_arguments,
        reason = "C4 layout keeps its measured and placed inputs explicit."
    )]
    pub fn from_routed(
        model: &C4DiagramModel,
        config: &C4LayoutSettings,
        bounds: Bounds,
        ink_size: Size2D<f64>,
        container: Size2D<f64>,
        screen_available_width: Option<f64>,
        boundaries: Vec<C4BoundaryLayout>,
        shapes: Vec<C4ShapeLayout>,
        rels: Vec<C4RelLayout>,
    ) -> Self {
        Self {
            bounds: Some(bounds),
            width: ink_size.width + 2.0 * config.diagram_margin_x,
            height: ink_size.height + 2.0 * config.diagram_margin_y,
            container_width: container.width,
            container_height: container.height,
            screen_available_width,
            c4_type: model.c4_type.clone(),
            title: model.title.clone(),
            use_max_width: config.use_max_width,
            boundaries,
            shapes,
            rels,
        }
    }
}
