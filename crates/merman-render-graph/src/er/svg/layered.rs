//! Paint ER's axis-tagged Layered routes without a second rectangular clipping pass.

use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman_render_core::{
    LayoutEdge, LayoutPoint, Result,
    config::{ConfigLookup, value_at},
    graph_layout::SolvedEdge,
    resources::OperationWorkMeter,
    route_types::Axis,
    svg_edge_parity::{
        layered_route::{self, Curve},
        line_hops::{LineHopEdge, LineHopStyle},
        path_numbers::json_stringify_axis_points_into,
    },
};
use serde_json::Value;

use super::super::layered::{ErLayeredRoutes, allocation_error, invalid_projection};

pub(super) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    routes: &ErLayeredRoutes,
    config: &(impl ConfigLookup + ?Sized),
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    let style = match value_at(config, &["layered", "lineHops"]) {
        Some(Value::Bool(false)) => return Ok(HashMap::new()),
        Some(Value::String(style)) if style == "gap" => LineHopStyle::Gap,
        _ => LineHopStyle::Arc,
    };
    work.charge_emit_work(edges.len())?;
    let mut hop_edges = Vec::new();
    hop_edges
        .try_reserve_exact(edges.len())
        .map_err(|_| allocation_error())?;
    hop_edges.extend(edges.iter().map(|edge| LineHopEdge {
        id: &edge.id,
        points: &edge.points,
        curve: Some(routes.curve.as_str()),
        // ER's crow's-foot markers have no route setback in Mermaid.
        arrow_type_start: None,
        arrow_type_end: None,
    }));
    let paths = layered_route::line_hops(&hop_edges, style, work)?;
    let mut by_id = HashMap::new();
    by_id
        .try_reserve(paths.len())
        .map_err(|_| allocation_error())?;
    by_id.extend(
        paths
            .into_iter()
            .map(|(index, path)| (edges[index].id.as_str(), path)),
    );
    Ok(by_id)
}

pub(super) fn shaft(route: &SolvedEdge, curve: Curve, work: &OperationWorkMeter) -> Result<String> {
    layered_route::path(&route.points, curve, 0.0, 0.0, work)
}

#[derive(Default)]
pub(super) struct AxisPointJson {
    axes: Vec<Axis>,
    json: String,
    number: ryu_js::Buffer,
}

impl AxisPointJson {
    pub(super) fn encode(
        &mut self,
        output: &mut String,
        points: &[LayoutPoint],
        route: &SolvedEdge,
        work: &OperationWorkMeter,
    ) -> Result<()> {
        if points.len() != route.points.len() {
            return Err(invalid_projection(
                "shaft and data points occupy different slots",
            ));
        }
        work.charge_emit_work(points.len())?;
        self.axes.clear();
        self.axes
            .try_reserve(points.len())
            .map_err(|_| allocation_error())?;
        self.axes
            .extend(route.points.iter().map(|point| point.axis));
        self.json.clear();
        json_stringify_axis_points_into(&mut self.json, points, &self.axes, &mut self.number);
        work.charge_emit_work(self.json.len())?;
        STANDARD.encode_string(&self.json, output);
        Ok(())
    }
}
