//! Layout DTO projection for Usecase backends.

use super::{LayoutEdge, LayoutNode, LayoutPoint, UsecaseEdgePlan, UsecaseNodePlan};
use merman_render_core::LayoutLabel;

pub fn node(plan: &UsecaseNodePlan, x: f64, y: f64, width: f64, height: f64) -> LayoutNode {
    LayoutNode {
        id: plan.id.clone(),
        x,
        y,
        width,
        height,
        is_cluster: plan.is_boundary,
        label_width: Some(plan.label.metrics.width),
        label_height: Some(plan.label.metrics.height),
    }
}

pub fn edge(
    plan: &UsecaseEdgePlan,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
) -> LayoutEdge {
    LayoutEdge {
        id: plan.id.clone(),
        from: plan.source.clone(),
        to: plan.target.clone(),
        from_cluster: None,
        to_cluster: None,
        points,
        label,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: plan.start_marker.clone(),
        end_marker: plan.end_marker.clone(),
        stroke_dasharray: plan.dotted.then(|| "3,3".to_owned()),
    }
}
