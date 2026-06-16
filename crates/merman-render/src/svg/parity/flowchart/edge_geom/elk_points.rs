//! ELK edge point post-processing for flowchart SVG parity.
//!
//! Source port boundary:
//! - Mermaid `packages/mermaid-layout-elk/src/render.ts` center-point injection, `cutter2`,
//!   endpoint replacement, invalid-point fallback, consecutive deduplication, and rounded curve
//!   selection.
//! - Mermaid `packages/mermaid-layout-elk/src/geometry.ts` `outsideNode` / `replaceEndpoint`
//!   endpoint semantics.

use super::super::*;
use super::{BoundaryNode, boundary_for_node, intersect_for_layout_shape};

pub(in crate::svg::parity::flowchart) fn apply_flowchart_elk_endpoint_cutter(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    normalize_cyclic_special: bool,
    base_points: &[crate::model::LayoutPoint],
    out: &mut Vec<crate::model::LayoutPoint>,
) {
    out.clear();
    out.extend_from_slice(base_points);
    if base_points.len() < 2 {
        return;
    }

    let Some(start_bounds) = boundary_for_node(
        ctx,
        edge.from.as_str(),
        origin_x,
        origin_y,
        normalize_cyclic_special,
    ) else {
        return;
    };
    let Some(end_bounds) = boundary_for_node(
        ctx,
        edge.to.as_str(),
        origin_x,
        origin_y,
        normalize_cyclic_special,
    ) else {
        return;
    };

    let start_shape = ctx
        .nodes_by_id
        .get(edge.from.as_str())
        .and_then(|node| node.layout_shape.as_deref());
    let end_shape = ctx
        .nodes_by_id
        .get(edge.to.as_str())
        .and_then(|node| node.layout_shape.as_deref());
    let start_center = crate::model::LayoutPoint {
        x: start_bounds.x,
        y: start_bounds.y,
    };
    let end_center = crate::model::LayoutPoint {
        x: end_bounds.x,
        y: end_bounds.y,
    };

    out.clear();
    if !point_close(
        base_points.first().unwrap_or(&start_center),
        &start_center,
        1e-6,
    ) {
        out.push(start_center.clone());
    }
    out.extend_from_slice(base_points);
    if !point_close(base_points.last().unwrap_or(&end_center), &end_center, 1e-6) {
        out.push(end_center.clone());
    }

    let prev_points = out.clone();
    apply_start_intersection(ctx, edge.from.as_str(), start_shape, &start_bounds, out);
    apply_end_intersection(ctx, edge.to.as_str(), end_shape, &end_bounds, out);
    trim_too_close_tail(out);
    dedup_consecutive_points_in_place(out);

    if out.len() < 2 || out.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        out.clear();
        out.extend(prev_points);
        dedup_consecutive_points_in_place(out);
    }
}

fn apply_start_intersection(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    shape: Option<&str>,
    bounds: &BoundaryNode,
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    let Some(first_outside) = points.iter().position(|point| outside_node(bounds, point)) else {
        return;
    };
    let outside = points[first_outside].clone();
    let value = node_intersection(ctx, node_id, shape, bounds, &outside);
    replace_endpoint(points, Endpoint::Start, value);
}

fn apply_end_intersection(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    shape: Option<&str>,
    bounds: &BoundaryNode,
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    let outside = points
        .iter()
        .rposition(|point| outside_node(bounds, point))
        .or_else(|| (points.len() > 1).then_some(points.len() - 2));
    let Some(outside) = outside else {
        return;
    };
    let outside = points[outside].clone();
    let value = node_intersection(ctx, node_id, shape, bounds, &outside);
    replace_endpoint(points, Endpoint::End, value);
}

fn node_intersection(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    shape: Option<&str>,
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    intersect_for_layout_shape(ctx, node_id, bounds, shape, outside)
}

#[derive(Debug, Clone, Copy)]
enum Endpoint {
    Start,
    End,
}

fn replace_endpoint(
    points: &mut Vec<crate::model::LayoutPoint>,
    endpoint: Endpoint,
    value: crate::model::LayoutPoint,
) {
    if points.is_empty() {
        return;
    }

    match endpoint {
        Endpoint::Start => {
            if point_close(&points[0], &value, 0.1) {
                points.remove(0);
            } else {
                points[0] = value;
            }
        }
        Endpoint::End => {
            let last = points.len() - 1;
            if point_close(&points[last], &value, 0.1) {
                points.pop();
            } else {
                points[last] = value;
            }
        }
    }
}

fn outside_node(bounds: &BoundaryNode, point: &crate::model::LayoutPoint) -> bool {
    let dx = (point.x - bounds.x).abs();
    let dy = (point.y - bounds.y).abs();
    dx >= bounds.width / 2.0 || dy >= bounds.height / 2.0
}

fn trim_too_close_tail(points: &mut Vec<crate::model::LayoutPoint>) {
    if points.len() <= 1 {
        return;
    }
    let last = points[points.len() - 1].clone();
    let prev = points[points.len() - 2].clone();
    if (last.x - prev.x).hypot(last.y - prev.y) < 2.0 {
        points.pop();
    }
}

fn dedup_consecutive_points_in_place(points: &mut Vec<crate::model::LayoutPoint>) {
    if points.len() < 2 {
        return;
    }

    let mut write = 1usize;
    for read in 1..points.len() {
        if point_close(&points[read], &points[write - 1], 1e-6) {
            continue;
        }
        if write != read {
            points[write] = points[read].clone();
        }
        write += 1;
    }
    points.truncate(write);
}

fn point_close(a: &crate::model::LayoutPoint, b: &crate::model::LayoutPoint, eps: f64) -> bool {
    (a.x - b.x).abs() <= eps && (a.y - b.y).abs() <= eps
}
