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

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::svg::parity::flowchart) struct ElkEndpointAdapterCorners {
    pub source: bool,
    pub target: bool,
}

pub(in crate::svg::parity::flowchart) fn apply_flowchart_elk_endpoint_cutter(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    normalize_cyclic_special: bool,
    base_points: &[crate::model::LayoutPoint],
    out: &mut Vec<crate::model::LayoutPoint>,
) -> ElkEndpointAdapterCorners {
    out.clear();
    out.extend_from_slice(base_points);
    if base_points.len() < 2 {
        return ElkEndpointAdapterCorners::default();
    }

    let Some(start_bounds) = boundary_for_node(
        ctx,
        edge.from.as_str(),
        origin_x,
        origin_y,
        normalize_cyclic_special,
    ) else {
        return ElkEndpointAdapterCorners::default();
    };
    let Some(end_bounds) = boundary_for_node(
        ctx,
        edge.to.as_str(),
        origin_x,
        origin_y,
        normalize_cyclic_special,
    ) else {
        return ElkEndpointAdapterCorners::default();
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

    let inserted_start_center = !point_close(
        base_points.first().unwrap_or(&start_center),
        &start_center,
        1e-6,
    );
    let inserted_end_center =
        !point_close(base_points.last().unwrap_or(&end_center), &end_center, 1e-6);

    out.clear();
    if inserted_start_center {
        out.push(start_center.clone());
    }
    out.extend_from_slice(base_points);
    if inserted_end_center {
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
        return ElkEndpointAdapterCorners::default();
    }

    ElkEndpointAdapterCorners {
        source: inserted_start_center
            && out.len() > 2
            && point_close(&out[1], &base_points[0], 1e-6),
        target: inserted_end_center
            && out.len() > 2
            && point_close(
                &out[out.len() - 2],
                &base_points[base_points.len() - 1],
                1e-6,
            ),
    }
}

pub(in crate::svg::parity::flowchart) fn align_elk_endpoint_adapters_to_route(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    normalize_cyclic_special: bool,
    adapters: &mut ElkEndpointAdapterCorners,
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    fn route_intersection(
        ctx: &FlowchartRenderCtx<'_>,
        node_id: &str,
        shape: Option<&str>,
        bounds: &BoundaryNode,
        port: &crate::model::LayoutPoint,
        route_neighbor: &crate::model::LayoutPoint,
    ) -> Option<crate::model::LayoutPoint> {
        let mut dx = route_neighbor.x - port.x;
        let mut dy = route_neighbor.y - port.y;
        let len = dx.hypot(dy);
        if !len.is_finite() || len <= 1e-9 {
            return None;
        }
        dx /= len;
        dy /= len;
        // For either endpoint, the neighboring route point lies away from the node.
        let inward = -1.0;

        let is_inside = |point: &crate::model::LayoutPoint| {
            let boundary = intersect_for_layout_shape(ctx, node_id, bounds, shape, point);
            let point_distance = (point.x - bounds.x).hypot(point.y - bounds.y);
            let boundary_distance = (boundary.x - bounds.x).hypot(boundary.y - bounds.y);
            if boundary_distance <= 1e-9 {
                return (point.x - bounds.x).abs() <= bounds.width / 2.0
                    && (point.y - bounds.y).abs() <= bounds.height / 2.0;
            }
            point_distance <= boundary_distance + 1e-6
        };

        if is_inside(port) {
            return Some(port.clone());
        }

        let max_distance = (bounds.width + bounds.height).max(1.0) * 2.0;
        let mut outside_distance = 0.0;
        let mut inside_distance = 1.0;
        while inside_distance <= max_distance {
            let candidate = crate::model::LayoutPoint {
                x: port.x + inward * dx * inside_distance,
                y: port.y + inward * dy * inside_distance,
            };
            if is_inside(&candidate) {
                for _ in 0..40 {
                    let mid = (outside_distance + inside_distance) / 2.0;
                    let candidate = crate::model::LayoutPoint {
                        x: port.x + inward * dx * mid,
                        y: port.y + inward * dy * mid,
                    };
                    if is_inside(&candidate) {
                        inside_distance = mid;
                    } else {
                        outside_distance = mid;
                    }
                }
                return Some(crate::model::LayoutPoint {
                    x: port.x + inward * dx * inside_distance,
                    y: port.y + inward * dy * inside_distance,
                });
            }
            outside_distance = inside_distance;
            inside_distance *= 2.0;
        }
        None
    }

    if adapters.source && points.len() >= 3 {
        let bounds = boundary_for_node(
            ctx,
            edge.from.as_str(),
            origin_x,
            origin_y,
            normalize_cyclic_special,
        );
        let shape = ctx
            .nodes_by_id
            .get(edge.from.as_str())
            .and_then(|node| node.layout_shape.as_deref());
        if let Some(bounds) = bounds
            && let Some(intersection) = route_intersection(
                ctx,
                edge.from.as_str(),
                shape,
                &bounds,
                &points[1],
                &points[2],
            )
        {
            points[0] = intersection;
            points.remove(1);
            adapters.source = false;
        }
    }

    if adapters.target && points.len() >= 3 {
        let bounds = boundary_for_node(
            ctx,
            edge.to.as_str(),
            origin_x,
            origin_y,
            normalize_cyclic_special,
        );
        let shape = ctx
            .nodes_by_id
            .get(edge.to.as_str())
            .and_then(|node| node.layout_shape.as_deref());
        let n = points.len();
        if let Some(bounds) = bounds
            && let Some(intersection) = route_intersection(
                ctx,
                edge.to.as_str(),
                shape,
                &bounds,
                &points[n - 2],
                &points[n - 3],
            )
        {
            points[n - 1] = intersection;
            points.remove(n - 2);
            adapters.target = false;
        }
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
    let center = points[0].clone();
    let value = node_intersection(ctx, node_id, shape, bounds, &outside, &center);
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
    let center = points[points.len() - 1].clone();
    let value = node_intersection(ctx, node_id, shape, bounds, &outside, &center);
    replace_endpoint(points, Endpoint::End, value);
}

fn node_intersection(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    shape: Option<&str>,
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
    center: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let outside = ensure_truly_outside(bounds, outside);
    let candidate = intersect_for_layout_shape(ctx, node_id, bounds, shape, &outside);
    if node_intersection_is_usable(bounds, &outside, &candidate) {
        candidate
    } else {
        fallback_intersection(bounds, &outside, center)
    }
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

fn ensure_truly_outside(
    bounds: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    const EPS: f64 = 1.0;
    const PUSH_OUT: f64 = 10.0;

    let dx = (point.x - bounds.x).abs();
    let dy = (point.y - bounds.y).abs();
    let w = bounds.width / 2.0;
    let h = bounds.height / 2.0;
    if (dx - w).abs() < EPS || (dy - h).abs() < EPS {
        let dir_x = point.x - bounds.x;
        let dir_y = point.y - bounds.y;
        let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if len > 0.0 {
            return crate::model::LayoutPoint {
                x: bounds.x + (dir_x / len) * (len + PUSH_OUT),
                y: bounds.y + (dir_y / len) * (len + PUSH_OUT),
            };
        }
    }
    point.clone()
}

fn node_intersection_is_usable(
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
    candidate: &crate::model::LayoutPoint,
) -> bool {
    const EPS: f64 = 1.0;

    let wrong_side = (outside.x < bounds.x && candidate.x > bounds.x)
        || (outside.x > bounds.x && candidate.x < bounds.x);
    if wrong_side {
        return false;
    }

    let dx = outside.x - candidate.x;
    let dy = outside.y - candidate.y;
    (dx * dx + dy * dy).sqrt() > EPS
}

fn fallback_intersection(
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
    center: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let inside = make_inside_point(bounds, outside, center);
    rect_intersection(bounds, outside, &inside)
}

fn make_inside_point(
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
    center: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    const EPS: f64 = 1.0;

    let is_vertical = (outside.x - bounds.x).abs() < EPS;
    let is_horizontal = (outside.y - bounds.y).abs() < EPS;
    crate::model::LayoutPoint {
        x: if is_vertical {
            outside.x
        } else if outside.x < bounds.x {
            bounds.x - bounds.width / 4.0
        } else {
            bounds.x + bounds.width / 4.0
        },
        y: if is_horizontal { outside.y } else { center.y },
    }
}

fn rect_intersection(
    bounds: &BoundaryNode,
    outside: &crate::model::LayoutPoint,
    inside: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width / 2.0;
    let h = bounds.height / 2.0;

    let q_total = (outside.y - inside.y).abs();
    let r_total = (outside.x - inside.x).abs();

    if (y - outside.y).abs() * w > (x - outside.x).abs() * h {
        let q = if inside.y < outside.y {
            outside.y - h - y
        } else {
            y - h - outside.y
        };
        let r = (r_total * q) / q_total;
        let mut res = crate::model::LayoutPoint {
            x: if inside.x < outside.x {
                inside.x + r
            } else {
                inside.x - r_total + r
            },
            y: if inside.y < outside.y {
                inside.y + q_total - q
            } else {
                inside.y - q_total + q
            },
        };
        if r_total == 0.0 {
            res.x = outside.x;
        }
        if q_total == 0.0 {
            res.y = outside.y;
        }
        res
    } else {
        let r = if inside.x < outside.x {
            outside.x - w - x
        } else {
            x - w - outside.x
        };
        let q = (q_total * r) / r_total;
        let mut res = crate::model::LayoutPoint {
            x: if inside.x < outside.x {
                inside.x + r_total - r
            } else {
                inside.x - r_total + r
            },
            y: if inside.y < outside.y {
                inside.y + q
            } else {
                inside.y - q
            },
        };
        if r_total == 0.0 {
            res.x = outside.x;
        }
        if q_total == 0.0 {
            res.y = outside.y;
        }
        res
    }
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
