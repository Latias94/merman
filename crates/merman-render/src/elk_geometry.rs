//! Mermaid 12 missing-section geometry shared by Class/ER label placement and SVG paint.

use crate::model::{LayoutNode, LayoutPoint};

/// Mirrors the two-center path through `sanitizeElkEdgePoints` in the pinned ELK renderer.
/// This derived geometry must not replace the raw provider route: empty sections differ
/// from two real points and select different curve and label behavior in the renderer.
pub(crate) fn missing_rect_section_points(
    start: &LayoutNode,
    end: &LayoutNode,
) -> Vec<LayoutPoint> {
    fn intersect(node: &LayoutNode, point: &LayoutPoint) -> LayoutPoint {
        let dx = point.x - node.x;
        let dy = point.y - node.y;
        let w = node.width / 2.0;
        let h = node.height / 2.0;
        let (sx, sy) = if dy.abs() * w > dx.abs() * h {
            let h = if dy < 0.0 { -h } else { h };
            (if dy == 0.0 { 0.0 } else { h * dx / dy }, h)
        } else {
            let w = if dx < 0.0 { -w } else { w };
            (w, if dx == 0.0 { 0.0 } else { w * dy / dx })
        };
        LayoutPoint {
            x: node.x + sx,
            y: node.y + sy,
        }
    }

    let centers = [
        LayoutPoint {
            x: start.x,
            y: start.y,
        },
        LayoutPoint { x: end.x, y: end.y },
    ];
    let mut points = centers.clone();
    if (end.x - start.x).abs() >= start.width / 2.0 || (end.y - start.y).abs() >= start.height / 2.0
    {
        points[0] = intersect(start, &centers[1]);
    }
    points[1] = intersect(end, &points[0]);
    // The source removes a tail closer than 2px, then restores the original centers if
    // fewer than two valid points survive. Consecutive deduplication runs afterwards.
    if (points[1].x - points[0].x).hypot(points[1].y - points[0].y) < 2.0
        || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        points = centers;
    }
    if (points[1].x - points[0].x).abs() <= 1e-6 && (points[1].y - points[0].y).abs() <= 1e-6 {
        vec![points[0].clone()]
    } else {
        points.to_vec()
    }
}
