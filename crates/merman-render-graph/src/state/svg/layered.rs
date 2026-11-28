//! State outline and marker projection over the shared axis route painter.

use kurbo::{Point, Size};
use merman_model::OperationPhase;
use merman_render_core::{
    graph_layout::{Axis, RoutePoint},
    svg_edge_parity::{layered_route, line_hops::LineHopEdge},
};
use serde_json::Value;

use crate::state::{
    flat_input::buffer,
    layered::{StateLayeredRoutes, allocation_error, invalid_projection},
};

use super::*;

pub(super) type StateLayeredEdges<'a> = FxHashMap<&'a str, StateLayeredEdgePaint<'a>>;

pub(super) struct StateLayeredEdgePaint<'a> {
    pub(super) points: Vec<LayoutPoint>,
    pub(super) axes: &'a [Axis],
    pub(super) path: String,
    pub(super) hopped: Option<String>,
}

pub(super) fn prepare_edges<'a>(
    ctx: &StateRenderCtx<'a>,
    layout: &'a StateDiagramLayout,
    routes: &'a StateLayeredRoutes,
    config: &Value,
) -> Result<StateLayeredEdges<'a>> {
    let mut painted = StateLayeredEdges::default();
    if !ctx.include_edges {
        return Ok(painted);
    }
    let work = ctx.work_meter;
    work.charge_emit_work(routes.edges.len())?;
    if layout.edges.len() != routes.edges.len() {
        return Err(invalid_projection("route and paint slots differ"));
    }
    painted
        .try_reserve(routes.edges.len())
        .map_err(|_| allocation_error())?;
    let mut knots = Vec::new();
    for (edge, route) in layout.edges.iter().zip(&routes.edges) {
        work.charge_emit_work(edge.id.len().max(1))?;
        let semantic = ctx
            .edges
            .get(route.source_slot)
            .filter(|owner| owner.id == edge.id)
            .ok_or_else(|| invalid_projection("route lost its source transition"))?;
        if route.axes.len() != edge.points.len() || edge.points.len() < 2 {
            return Err(invalid_projection("route lost its axis points"));
        }
        work.charge_emit_work(edge.points.len())?;
        knots.clear();
        knots
            .try_reserve(edge.points.len())
            .map_err(|_| allocation_error())?;
        knots.extend(
            edge.points
                .iter()
                .zip(&route.axes)
                .map(|(point, &axis)| RoutePoint::new(point.x, point.y, axis)),
        );
        if let [start, .., end] = knots.as_mut_slice() {
            *start = clip(ctx, &edge.from, *start)?;
            *end = clip(ctx, &edge.to, *end)?;
        }
        let trim = layered_route::marker_trim(Some(&semantic.arrow_type_end));
        let path = layered_route::path(&knots, routes.curve, 0.0, trim, work)?;
        work.charge_emit_work(knots.len())?;
        let mut points = buffer(knots.len(), "layered")?;
        points.extend(knots.iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        painted.insert(
            edge.id.as_str(),
            StateLayeredEdgePaint {
                points,
                axes: &route.axes,
                path,
                hopped: None,
            },
        );
    }

    if let Some(style) = layered_route::line_hop_style(config.pointer("/layered/lineHops"))
        && ctx.style_plan.compatibility().native_edge_style()
            != Some(merman_theme_contract::v2::EdgeStyle::Sketch)
    {
        work.charge_emit_work(routes.edges.len())?;
        let mut hops = buffer(routes.edges.len(), "layered")?;
        for (edge, route) in layout.edges.iter().zip(&routes.edges) {
            let owner = &ctx.edges[route.source_slot];
            hops.push(LineHopEdge {
                id: &edge.id,
                points: &painted[edge.id.as_str()].points,
                curve: Some(routes.curve.as_str()),
                arrow_type_start: None,
                arrow_type_end: Some(&owner.arrow_type_end),
            });
        }
        let overrides = layered_route::line_hops(&hops, style, work)?;
        drop(hops);
        for (slot, path) in overrides {
            work.charge_emit_work(path.len())?;
            if let Some(paint) = painted.get_mut(layout.edges[slot].id.as_str()) {
                paint.hopped = Some(path);
            }
        }
    }
    work.checkpoint(OperationPhase::Emit)?;
    Ok(painted)
}

fn clip(ctx: &StateRenderCtx<'_>, id: &str, point: RoutePoint) -> Result<RoutePoint> {
    let Some(&semantic) = ctx.nodes_by_id.get(id) else {
        return Ok(point);
    };
    let diamond = match semantic.shape.as_str() {
        "stateStart" | "stateEnd" => false,
        "choice" => true,
        // The remaining State leaves are rectangles; the solver already owns that contact.
        _ => return Ok(point),
    };
    let Some(node) = ctx.layout_nodes_by_id.get(id) else {
        return Ok(point);
    };
    let center = Point::new(node.x, node.y);
    let size = Size::new(node.width, node.height);
    let radius = node.width / 2.0;
    let intersect = |target: Point, origin: Option<Point>| {
        let offset = target - center;
        let hit = if let Some(origin) = origin {
            let along = target - origin;
            let vertical = along.x.abs() <= along.y.abs();
            let (across, flow) = if vertical {
                (offset.x, along.y)
            } else {
                (offset.y, along.x)
            };
            let fraction = across.abs() / radius;
            if fraction > 1.0 || flow == 0.0 {
                return Ok(None);
            }
            let unit = if diamond {
                1.0 - fraction
            } else {
                (1.0 - fraction * fraction).sqrt()
            };
            let extent = (radius * unit).copysign(flow);
            if vertical {
                Point::new(target.x, center.y + extent)
            } else {
                Point::new(center.x + extent, target.y)
            }
        } else {
            let distance = if diamond {
                offset.x.abs() + offset.y.abs()
            } else {
                offset.hypot()
            };
            if distance == 0.0 {
                return Ok(None);
            }
            center + offset * (radius / distance)
        };
        Ok(Some(hit))
    };
    layered_route::clip_endpoint(point, center, size, intersect, ctx.work_meter)
}
