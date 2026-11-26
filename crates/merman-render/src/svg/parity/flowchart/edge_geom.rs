//! Flowchart edge geometry helpers.
//!
//! This module is a façade to keep the flowchart renderer organized.

use super::*;

mod basis;
mod boundary;
#[cfg(any(feature = "layout-shoal", test))]
mod canonical;
mod compute;
mod curve_path;
mod elk_points;
mod intersect;
mod line_with_offset;
mod rect_clip;
mod swimlane_points;
mod terminal_jogs;
mod trace;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", test)
))]
pub(super) use compute::{DagreLinearPaintedRoute, dagre_linear_painted_route};
pub(super) use compute::{finish_edge_route, prepare_edge_route};
pub(super) use terminal_jogs::{separate_edge_labels, straighten_edge_terminals};

pub(super) use crate::svg::parity::edge_path::maybe_fix_corners;
pub(super) use basis::maybe_remove_redundant_cluster_run_point;
pub(super) use boundary::{BoundaryNode, boundary_for_cluster, boundary_for_node};
use curve_path::curve_path_geometry;
pub(super) use elk_points::{
    apply_flowchart_elk_endpoint_cutter, missing_section_label_position, missing_section_points,
};
pub(super) use intersect::{force_intersect_for_layout_shape, intersect_for_layout_shape};
pub(super) use line_with_offset::{
    arrow_types_for_edge, line_with_offset_for_edge_type,
    rounded_line_with_marker_offsets_for_edge_type,
};
use rect_clip::{cut_path_at_intersect_into, dedup_consecutive_points};
pub(super) use trace::{FlowchartEdgeTraceInput, record_flowchart_edge_trace};

pub(super) struct ClippedEdgeRoute {
    base_points: Vec<crate::model::LayoutPoint>,
    points: Vec<crate::model::LayoutPoint>,
    origin_x: f64,
    origin_y: f64,
    label: Option<crate::model::LayoutLabel>,
}

#[derive(Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgePathGeomRequest<'a> {
    pub(super) ctx: &'a FlowchartRenderCtx<'a>,
    pub(super) key: crate::flowchart::FlowchartEdgeKey,
    pub(super) edge: &'a crate::flowchart::FlowEdge,
    pub(super) origin_x: f64,
    pub(super) origin_y: f64,
    pub(super) trace_enabled: bool,
}

pub(in crate::svg::parity::flowchart) fn flowchart_compute_edge_path_geom(
    request: FlowchartEdgePathGeomRequest<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Result<Option<FlowchartEdgePathGeom>> {
    #[cfg(any(feature = "layout-shoal", test))]
    if let crate::graph_layout::RouteContract::CanonicalAxis(axis) =
        &request.ctx.layout.route_contract
    {
        let occurrence = request.key.semantic_index();
        let first = axis
            .border_stitches
            .partition_point(|stitch| stitch.edge < occurrence);
        let tail = &axis.border_stitches[first..];
        let count = tail.partition_point(|stitch| stitch.edge == occurrence);
        let Some(route) = axis.routes.get(occurrence).and_then(Option::as_ref) else {
            return Ok(None);
        };
        return canonical::paint(
            request,
            &axis.nodes,
            route,
            &tail[..count],
            &axis.clearance,
            &mut scratch.canonical_points,
        )
        .map(Some);
    }
    Ok(prepare_edge_route(request, scratch)
        .and_then(|route| finish_edge_route(request, route, scratch)))
}
