//! Flowchart edge bbox/path helpers.
//!
//! This is a small façade module to keep the flowchart module surface organized. The current
//! implementation lives in `flowchart.rs` and can be migrated here incrementally.

use super::*;

pub(super) fn flowchart_edge_path_d_for_bbox(
    layout_edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
    translate_x: f64,
    translate_y: f64,
    default_edge_interpolate: &str,
    edge_html_labels: bool,
    edge: &crate::flowchart::FlowEdge,
) -> Option<(String, super::super::path_bounds::SvgPathBounds)> {
    super::flowchart_edge_path_d_for_bbox_impl(
        layout_edges_by_id,
        layout_clusters_by_id,
        translate_x,
        translate_y,
        default_edge_interpolate,
        edge_html_labels,
        edge,
    )
}
