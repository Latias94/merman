//! Trace payload structures for debugging flowchart edge geometry.
//!
//! These records are emitted only when tracing is enabled through [`SvgDebugOptions`].

use super::super::*;
use merman_render_core::svg_edge_parity::trace::{FlowchartEdgeTrace, FlowchartEdgeTracePoint};

pub(in crate::svg) fn tp(p: &merman_render_core::LayoutPoint) -> FlowchartEdgeTracePoint {
    FlowchartEdgeTracePoint { x: p.x, y: p.y }
}

pub(in crate::svg) struct FlowchartEdgeTraceInput<'a> {
    pub(in crate::svg) ctx: &'a FlowchartRenderCtx<'a>,
    pub(in crate::svg) edge: &'a merman_model::families::flowchart::FlowEdge,
    pub(in crate::svg) layout_edge: Option<&'a merman_render_core::LayoutEdge>,
    pub(in crate::svg) origin_x: f64,
    pub(in crate::svg) origin_y: f64,
    pub(in crate::svg) base_points: &'a [merman_render_core::LayoutPoint],
    pub(in crate::svg) points_after_intersect_for_trace:
        Option<&'a [merman_render_core::LayoutPoint]>,
    pub(in crate::svg) points_for_render: &'a [merman_render_core::LayoutPoint],
    pub(in crate::svg) points_for_data_points: &'a [merman_render_core::LayoutPoint],
}

pub(in crate::svg) fn record_flowchart_edge_trace(input: FlowchartEdgeTraceInput<'_>) {
    let FlowchartEdgeTraceInput {
        ctx,
        edge,
        layout_edge,
        origin_x,
        origin_y,
        base_points,
        points_after_intersect_for_trace,
        points_for_render,
        points_for_data_points,
    } = input;

    let trace = FlowchartEdgeTrace {
        fixture_diagram_id: ctx.diagram_id.semantic_str().to_string(),
        edge_id: edge.id.clone(),
        from: edge.from.clone(),
        to: edge.to.clone(),
        layout_from: layout_edge
            .map_or(&edge.from, |layout| &layout.from)
            .clone(),
        layout_to: layout_edge.map_or(&edge.to, |layout| &layout.to).clone(),
        from_cluster: layout_edge.and_then(|layout| layout.from_cluster.clone()),
        to_cluster: layout_edge.and_then(|layout| layout.to_cluster.clone()),
        origin_x,
        origin_y,
        tx: ctx.tx,
        ty: ctx.ty,
        base_points: base_points.iter().map(tp).collect(),
        points_after_intersect: points_after_intersect_for_trace
            .unwrap_or(points_for_data_points)
            .iter()
            .map(tp)
            .collect(),
        points_for_render: points_for_render.iter().map(tp).collect(),
        points_for_data_points: points_for_data_points.iter().map(tp).collect(),
    };

    if let Some(collector) = ctx.trace_collector {
        collector.record(trace);
    }
}
