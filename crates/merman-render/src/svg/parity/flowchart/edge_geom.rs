//! Flowchart edge geometry helpers.
//!
//! This module is a façade to keep the flowchart renderer organized. The current implementation
//! lives in `flowchart.rs` and can be migrated here incrementally.

use super::*;

mod basis;
mod boundary;
mod cyclic_special;
mod data_points;
mod fix_corners;
mod intersect;
mod rect_clip;
mod trace;

pub(super) use basis::{
    maybe_collapse_straight_except_one_endpoint, maybe_insert_midpoint_for_basis,
    maybe_pad_cyclic_special_basis_route, maybe_remove_redundant_cluster_run_point,
};
pub(super) use boundary::{
    BoundaryNode, boundary_for_cluster, boundary_for_node, maybe_normalize_selfedge_loop_points,
};
pub(super) use cyclic_special::normalize_cyclic_special_data_points;
pub(super) use data_points::{maybe_snap_data_point_to_f32, maybe_truncate_data_point};
pub(super) use fix_corners::maybe_fix_corners;
pub(super) use intersect::{
    force_intersect_for_layout_shape, intersect_for_layout_shape, is_rounded_intersect_shift_shape,
};
pub(super) use rect_clip::{cut_path_at_intersect_into, dedup_consecutive_points_into};
pub(super) use trace::{TraceEndpointIntersection, tb, tp, write_flowchart_edge_trace};

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
