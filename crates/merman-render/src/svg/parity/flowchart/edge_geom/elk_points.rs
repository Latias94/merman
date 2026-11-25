//! ELK edge point post-processing for flowchart SVG parity.
//!
//! Source port boundary:
//! - Mermaid `packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts` center-point injection, `cutter2`,
//!   endpoint replacement, invalid-point fallback, consecutive deduplication, and rounded curve
//!   selection.
//! - Mermaid `packages/mermaid/src/rendering-util/layout-algorithms/elk/geometry.ts` `outsideNode` / `replaceEndpoint`
//!   endpoint semantics.

use super::super::*;
use super::{BoundaryNode, boundary_for_node, intersect_for_layout_shape};

pub(in crate::svg::parity::flowchart) fn missing_section_points(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
) -> Option<Vec<crate::model::LayoutPoint>> {
    let start = boundary_for_node(ctx, &edge.from, origin_x, origin_y)?;
    let end = boundary_for_node(ctx, &edge.to, origin_x, origin_y)?;
    let centers = [
        crate::model::LayoutPoint {
            x: start.x,
            y: start.y,
        },
        crate::model::LayoutPoint { x: end.x, y: end.y },
    ];
    let mut clipped = Vec::new();
    sanitize_flowchart_elk_points(ctx, edge, origin_x, origin_y, &centers, false, &mut clipped);
    Some(clipped)
}

pub(in crate::svg::parity::flowchart) fn missing_section_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
    edge: &crate::model::LayoutEdge,
    origin_x: f64,
    origin_y: f64,
) -> Option<crate::model::LayoutPoint> {
    if !ctx.uses_elk_adapter_dom || !edge.points.is_empty() {
        return None;
    }
    let source = ctx.edges_by_key.get(&key)?;
    let points = missing_section_points(ctx, source, origin_x, origin_y)?;
    let first = points.first()?;
    let last = points.last()?;
    Some(crate::model::LayoutPoint {
        x: (first.x + last.x) / 2.0,
        y: (first.y + last.y) / 2.0,
    })
}

pub(in crate::svg::parity::flowchart) fn apply_flowchart_elk_endpoint_cutter(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    base_points: &[crate::model::LayoutPoint],
    out: &mut Vec<crate::model::LayoutPoint>,
) {
    sanitize_flowchart_elk_points(ctx, edge, origin_x, origin_y, base_points, true, out);
}

// Adapt family-owned outlines to the shared source port; no provider points are mutated.
#[allow(clippy::too_many_arguments)]
fn sanitize_flowchart_elk_points(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    base_points: &[crate::model::LayoutPoint],
    has_section: bool,
    out: &mut Vec<crate::model::LayoutPoint>,
) {
    use crate::elk_edge_geometry::{self as geometry, Outline, Shape};
    out.clear();
    out.extend_from_slice(base_points);
    let Some(start_bounds) = boundary_for_node(ctx, &edge.from, origin_x, origin_y) else {
        return;
    };
    let Some(end_bounds) = boundary_for_node(ctx, &edge.to, origin_x, origin_y) else {
        return;
    };
    let layout_node = |id: &str, bounds: &BoundaryNode| crate::model::LayoutNode {
        id: id.to_owned(),
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        is_cluster: ctx.layout_clusters_by_id.contains_key(id),
        label_width: None,
        label_height: None,
    };
    let start_node = layout_node(&edge.from, &start_bounds);
    let end_node = layout_node(&edge.to, &end_bounds);
    let start_shape = ctx
        .nodes_by_id
        .get(edge.from.as_str())
        .and_then(|node| node.layout_shape.as_deref());
    let end_shape = ctx
        .nodes_by_id
        .get(edge.to.as_str())
        .and_then(|node| node.layout_shape.as_deref());
    let start_intersection = |point: &crate::model::LayoutPoint| {
        intersect_for_layout_shape(ctx, &edge.from, &start_bounds, start_shape, point)
    };
    let end_intersection = |point: &crate::model::LayoutPoint| {
        intersect_for_layout_shape(ctx, &edge.to, &end_bounds, end_shape, point)
    };
    let start = Shape {
        node: &start_node,
        outline: Outline::Rect,
        intersection: Some(&start_intersection),
    };
    let end = Shape {
        node: &end_node,
        outline: Outline::Rect,
        intersection: Some(&end_intersection),
    };
    let mut input = Vec::with_capacity(base_points.len() + 2);
    if has_section && start_shape != Some("rect33") {
        input.push(crate::model::LayoutPoint {
            x: start_bounds.x,
            y: start_bounds.y,
        });
    }
    input.extend_from_slice(base_points);
    if has_section && end_shape != Some("rect33") {
        input.push(crate::model::LayoutPoint {
            x: end_bounds.x,
            y: end_bounds.y,
        });
    }
    *out = geometry::sanitize(&input, start, end);
    if has_section {
        adjust_marker_segments(out, start, end, edge.edge_type.as_deref());
    }
}

fn adjust_marker_segments(
    points: &mut Vec<crate::model::LayoutPoint>,
    start: crate::elk_edge_geometry::Shape<'_>,
    end: crate::elk_edge_geometry::Shape<'_>,
    edge_type: Option<&str>,
) {
    let (start_marker, end_marker) = super::arrow_types_for_edge(edge_type);
    // Mermaid applies the end adjustment before the start adjustment.
    crate::elk_edge_geometry::marker_segment(points, end, end_marker, false);
    crate::elk_edge_geometry::marker_segment(points, start, start_marker, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elk_edge_geometry::{Outline, Shape, sanitize};
    use crate::model::{LayoutNode, LayoutPoint as P};

    fn circle(x: f64) -> LayoutNode {
        LayoutNode {
            id: "circle".into(),
            x,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        }
    }

    #[test]
    fn marker_adjustment_keeps_elk_cutter_contacts() {
        let source_node = circle(0.0);
        let target_node = circle(60.0);
        let source = Shape {
            node: &source_node,
            outline: Outline::Ellipse,
            intersection: None,
        };
        let target = Shape {
            node: &target_node,
            outline: Outline::Ellipse,
            intersection: None,
        };
        let provider = vec![
            P { x: 0.0, y: 0.0 },
            P { x: 10.0, y: 3.0 },
            P { x: 20.0, y: 8.0 },
            P { x: 40.0, y: 8.0 },
            P { x: 50.0, y: 3.0 },
            P { x: 60.0, y: 0.0 },
        ];
        let mut points = sanitize(&provider, source, target);
        // Mermaid cutter2 pops the final intersection when its distance to the target port
        // is below 2. Marker adjustment keeps that surviving terminal contact.
        assert_eq!(points.len(), 5);
        adjust_marker_segments(&mut points, source, target, Some("double_arrow_point"));
        let radius_scale = 10.0 / 109.0_f64.sqrt();
        assert_eq!(points.len(), 4);
        assert!(
            (points[0].x - 10.0 * radius_scale).abs() < 2e-6,
            "{points:?}"
        );
        assert!(
            (points[0].y - 3.0 * radius_scale).abs() < 2e-6,
            "{points:?}"
        );
        assert_eq!((points[3].x, points[3].y), (50.0, 3.0));
        assert_eq!((points[1].x, points[1].y), (20.0, 8.0));
        assert_eq!((points[2].x, points[2].y), (40.0, 8.0));
        assert_eq!(provider.len(), 6);
        assert_eq!((provider[1].x, provider[1].y), (10.0, 3.0));
    }

    #[test]
    fn marker_adjustment_removes_short_ports_at_both_ends() {
        let source_node = circle(0.0);
        let target_node = circle(40.0);
        let source = Shape {
            node: &source_node,
            outline: Outline::Ellipse,
            intersection: None,
        };
        let target = Shape {
            node: &target_node,
            outline: Outline::Ellipse,
            intersection: None,
        };
        let mut points = vec![
            P { x: 9.6, y: 2.8 },
            P { x: 10.0, y: 3.0 },
            P { x: 30.0, y: 3.0 },
            P { x: 30.4, y: 2.8 },
        ];
        adjust_marker_segments(&mut points, source, target, Some("double_arrow_point"));
        assert_eq!(points.len(), 2);
        assert_eq!((points[0].x, points[0].y), (9.6, 2.8));
        assert_eq!((points[1].x, points[1].y), (30.4, 2.8));
    }
}
