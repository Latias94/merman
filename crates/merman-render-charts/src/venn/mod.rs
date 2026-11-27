//! Venn geometry, preparation, theme binding, and SVG emission.

use merman_model::{families::venn::VennDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, resources::OperationWorkMeter};
use merman_theme_contract::v2::{Color, FamilyTheme};

mod config;
mod layout;
mod model;
mod native_theme;
mod solver;
mod svg;
mod theme;

pub use layout::{VennFontSizes, layout_venn_diagram_typed_with_work_meter};
pub use model::{
    VennAreaLayout, VennCircleLayout, VennDiagramLayout, VennTextAreaLayout,
    VennTextDebugCellLayout, VennTextNodeLayout,
};
pub use solver::{
    VennArc, VennArea, VennCircle, VennLayoutArea, VennLayoutError, VennLayoutOptions,
    VennLayoutResult, VennPoint, VennSolution, VennTextPoint, circle_area,
    circle_circle_intersection, circle_overlap, compute_text_centre, compute_venn_layout,
    disjoint_cluster, distance, distance_from_intersect_area, greedy_layout, intersection_area,
    intersection_area_path, loss_function, normalize_solution, scale_solution, venn,
};

pub use svg::{VennSvgContext, render_venn_svg};
pub use theme::VennStyle;

/// Prepares Venn geometry and presentation using the same operation work meter.
pub fn prepare_venn_diagram(
    model: &VennDiagramModel,
    diagram_title: Option<&str>,
    config: &PresentationConfig,
    theme: Option<&FamilyTheme>,
    canvas: Option<Color>,
    work_meter: &OperationWorkMeter,
) -> Result<(VennDiagramLayout, VennStyle)> {
    let layout = layout_venn_diagram_typed_with_work_meter(
        model,
        diagram_title,
        config.as_value(),
        native_theme::native_font_size(theme, config),
        work_meter,
    )?;
    let style = VennStyle::resolve(theme, config, &layout, model, canvas, work_meter)?;
    Ok((layout, style))
}

const VENN_TITLE_CLASS: &str = "venn-title";
const VENN_CIRCLE_CLASS: &str = "venn-circle";
const VENN_INTERSECTION_CLASS: &str = "venn-intersection";
const VENN_TEXT_NODE_CLASS: &str = "venn-text-node";

fn rendered_area_label(area: &VennAreaLayout) -> &str {
    if let Some(label) = area.label.as_deref().filter(|label| !label.is_empty()) {
        label
    } else if area.sets.len() == 1 {
        area.sets[0].as_str()
    } else {
        ""
    }
}

fn rendered_text_node_label(node: &VennTextNodeLayout) -> &str {
    node.label.as_deref().unwrap_or(node.id.as_str())
}
