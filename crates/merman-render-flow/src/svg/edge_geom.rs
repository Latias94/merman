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
#[cfg(feature = "layout-grid")]
mod grid;
mod intersect;
#[cfg(feature = "layout-layered")]
mod layered;
#[cfg(test)]
mod line_with_offset;
mod terminal_jogs;
mod trace;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-shoal",
    feature = "native-compact"
))]
pub(super) use canonical::paint_commands as paint_canonical_edge_commands;
#[cfg(all(
    feature = "diagram-flowchart",
    any(
        all(
            feature = "layout-dagre",
            any(feature = "native-compact", feature = "test-support")
        ),
        all(feature = "layout-elk", feature = "native-compact")
    )
))]
pub(super) use compute::LinearPaintedRoute;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", feature = "test-support")
))]
pub(super) use compute::dagre_linear_painted_route;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-elk",
    feature = "native-compact"
))]
pub(super) use compute::elk_straight_painted_route;
pub(super) use compute::{finish_edge_route, prepare_edge_route};
#[cfg(feature = "layout-grid")]
pub(super) use grid::apply_grid_line_hops;
#[cfg(feature = "layout-layered")]
pub(super) use layered::apply_layered_line_hops;
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
        return canonical::paint(request, axis, scratch);
    }
    #[cfg(feature = "layout-grid")]
    if let Some(settings) = request.ctx.layout.route_contract.grid_paint() {
        return grid::paint(request, settings, scratch, measure_neo_length);
    }
    #[cfg(feature = "layout-layered")]
    if let FlowchartSvgRoute::Layered(routes) = request.ctx.layout.route_contract {
        return layered::paint(request, routes.routing, scratch, measure_neo_length);
    }
    Ok(prepare_edge_route(request, scratch)
        .and_then(|route| finish_edge_route(request, route, scratch, measure_neo_length)))
}
