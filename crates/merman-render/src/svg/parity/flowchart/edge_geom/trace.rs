//! Trace payload structures for debugging flowchart edge geometry.
//!
//! These types are emitted only when tracing is enabled through [`SvgDebugOptions`].

use super::super::*;

#[derive(serde::Serialize)]
pub(in crate::svg::parity::flowchart) struct TracePoint {
    pub(in crate::svg::parity::flowchart) x: f64,
    pub(in crate::svg::parity::flowchart) y: f64,
}

pub(in crate::svg::parity::flowchart) fn tp(p: &crate::model::LayoutPoint) -> TracePoint {
    TracePoint { x: p.x, y: p.y }
}

#[derive(serde::Serialize)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgeTrace {
    fixture_diagram_id: String,
    edge_id: String,
    from: String,
    to: String,
    layout_from: String,
    layout_to: String,
    from_cluster: Option<String>,
    to_cluster: Option<String>,
    origin_x: f64,
    origin_y: f64,
    tx: f64,
    ty: f64,
    base_points: Vec<TracePoint>,
    points_after_intersect: Vec<TracePoint>,
    points_for_render: Vec<TracePoint>,
    points_for_data_points: Vec<TracePoint>,
}

pub(in crate::svg::parity::flowchart) struct FlowchartEdgeTraceInput<'a> {
    pub(in crate::svg::parity::flowchart) ctx: &'a FlowchartRenderCtx<'a>,
    pub(in crate::svg::parity::flowchart) edge: &'a crate::flowchart::FlowEdge,
    pub(in crate::svg::parity::flowchart) layout_edge: &'a crate::model::LayoutEdge,
    pub(in crate::svg::parity::flowchart) origin_x: f64,
    pub(in crate::svg::parity::flowchart) origin_y: f64,
    pub(in crate::svg::parity::flowchart) base_points: &'a [crate::model::LayoutPoint],
    pub(in crate::svg::parity::flowchart) points_after_intersect_for_trace:
        Option<&'a [crate::model::LayoutPoint]>,
    pub(in crate::svg::parity::flowchart) points_for_render: &'a [crate::model::LayoutPoint],
    pub(in crate::svg::parity::flowchart) points_for_data_points: &'a [crate::model::LayoutPoint],
}

pub(in crate::svg::parity::flowchart) fn write_flowchart_edge_trace(
    input: FlowchartEdgeTraceInput<'_>,
) {
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
        fixture_diagram_id: ctx.diagram_id.to_string(),
        edge_id: edge.id.clone(),
        from: edge.from.clone(),
        to: edge.to.clone(),
        layout_from: layout_edge.from.clone(),
        layout_to: layout_edge.to.clone(),
        from_cluster: layout_edge.from_cluster.clone(),
        to_cluster: layout_edge.to_cluster.clone(),
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

    let default_path =
        std::path::PathBuf::from(format!("merman_flowchart_edge_trace_{}.json", edge.id));
    let out_path = ctx.trace_output_path.unwrap_or(default_path.as_path());
    if let Ok(json) = serde_json::to_string_pretty(&trace) {
        let _ = std::fs::write(out_path, json);
    }
}
