//! Venn geometry helpers and renderer presentation bindings.

use crate::model::{VennAreaLayout, VennTextNodeLayout};
#[cfg(test)]
use crate::{
    Result,
    model::VennDiagramLayout,
    resources::{OperationWorkMeter, RenderResourcePolicy},
};
#[cfg(test)]
use merman_model::families::venn::VennDiagramModel;

pub(crate) use merman_render_charts::venn::layout_venn_diagram_typed_with_work_meter;
pub use merman_render_charts::venn::{
    VennArc, VennArea, VennCircle, VennLayoutArea, VennLayoutError, VennLayoutOptions,
    VennLayoutResult, VennPoint, VennSolution, VennTextPoint, circle_area,
    circle_circle_intersection, circle_overlap, compute_text_centre, compute_venn_layout,
    disjoint_cluster, distance, distance_from_intersect_area, greedy_layout, intersection_area,
    intersection_area_path, loss_function, normalize_solution, scale_solution, venn,
};

mod native_theme;
mod theme;

pub(crate) use native_theme::native_font_size;
pub(crate) use theme::{VennAreaPaintBinding, VennTypographyThemePlan};

pub(crate) const VENN_TITLE_CLASS: &str = "venn-title";
pub(crate) const VENN_CIRCLE_CLASS: &str = "venn-circle";
pub(crate) const VENN_INTERSECTION_CLASS: &str = "venn-intersection";
pub(crate) const VENN_TEXT_NODE_CLASS: &str = "venn-text-node";

pub(crate) fn rendered_area_label(area: &VennAreaLayout) -> &str {
    if let Some(label) = area.label.as_deref().filter(|label| !label.is_empty()) {
        label
    } else if area.sets.len() == 1 {
        area.sets[0].as_str()
    } else {
        ""
    }
}

pub(crate) fn rendered_text_node_label(node: &VennTextNodeLayout) -> &str {
    node.label.as_deref().unwrap_or(node.id.as_str())
}

#[cfg(test)]
pub(crate) fn layout_venn_diagram_typed(
    model: &VennDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    resource_limits: RenderResourcePolicy,
) -> Result<VennDiagramLayout> {
    let work_meter = OperationWorkMeter::new(resource_limits);
    layout_venn_diagram_typed_with_work_meter(
        model,
        diagram_title,
        effective_config,
        None,
        &work_meter,
    )
}
