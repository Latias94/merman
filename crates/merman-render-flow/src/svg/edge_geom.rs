//! Flowchart edge geometry helpers.
//!
//! This module is a façade to keep the flowchart renderer organized.

use super::*;

#[cfg(feature = "canonical-edge-paint")]
fn edge_paint_error(
    edge_id: &str,
    error: merman_render_core::route_paint::RoutePaintError,
) -> Error {
    Error::InvalidModel {
        message: format!("Could not paint canonical edge `{edge_id}`: {error}"),
    }
}

mod boundary;
#[cfg(feature = "canonical-edge-paint")]
mod canonical;
mod compute;
mod elk_points;
mod intersect;
#[cfg(test)]
mod line_with_offset;
mod terminal_jogs;
mod trace;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-elk",
    feature = "native-compact"
))]
pub(super) use compute::elk_straight_painted_route;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", feature = "test-support")
))]
pub(super) use compute::{LinearPaintedRoute, dagre_linear_painted_route};
pub(super) use compute::{finish_edge_route, prepare_edge_route};
pub(super) use terminal_jogs::{separate_edge_labels, straighten_edge_terminals};

pub(super) use crate::edge_svg::arrow_types_for_edge;
pub(super) use boundary::{BoundaryNode, boundary_for_cluster, boundary_for_node};
pub(super) use elk_points::{
    apply_flowchart_elk_endpoint_cutter, missing_section_label_position, missing_section_points,
};
pub(super) use intersect::{force_intersect_for_layout_shape, intersect_for_layout_shape};
pub(super) use trace::{FlowchartEdgeTraceInput, record_flowchart_edge_trace};

pub(super) struct ClippedEdgeRoute {
    base_points: Vec<merman_render_core::LayoutPoint>,
    points: Vec<merman_render_core::LayoutPoint>,
    origin_x: f64,
    origin_y: f64,
    label: Option<merman_render_core::LayoutLabel>,
}

#[derive(Clone, Copy)]
pub(in crate::svg) struct FlowchartEdgePathGeomRequest<'a> {
    pub(super) ctx: &'a FlowchartRenderCtx<'a>,
    pub(super) key: crate::projection::FlowchartEdgeKey,
    pub(super) edge: &'a merman_model::families::flowchart::FlowEdge,
    pub(super) origin_x: f64,
    pub(super) origin_y: f64,
    pub(super) trace_enabled: bool,
}

pub(in crate::svg) fn flowchart_compute_edge_path_geom(
    request: FlowchartEdgePathGeomRequest<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    measure_neo_length: bool,
) -> Result<Option<FlowchartEdgePathGeom>> {
    #[cfg(feature = "canonical-edge-paint")]
    if let FlowchartSvgRoute::CanonicalAxis(axis) = request.ctx.layout.route_contract {
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
            scratch,
        )
        .map(Some);
    }
    Ok(prepare_edge_route(request, scratch)
        .and_then(|route| finish_edge_route(request, route, scratch, measure_neo_length)))
}
