//! Flowchart edge geometry helpers.
//!
//! This module is a façade to keep the flowchart renderer organized. The current implementation
//! lives in `flowchart.rs` and can be migrated here incrementally.

use super::*;

pub(super) fn flowchart_compute_edge_path_geom(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    abs_top_transform: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    trace_enabled: bool,
    viewbox_current_bounds: Option<(f64, f64, f64, f64)>,
) -> Option<FlowchartEdgePathGeom> {
    super::flowchart_compute_edge_path_geom_impl(
        ctx,
        edge,
        origin_x,
        origin_y,
        abs_top_transform,
        scratch,
        trace_enabled,
        viewbox_current_bounds,
    )
}
