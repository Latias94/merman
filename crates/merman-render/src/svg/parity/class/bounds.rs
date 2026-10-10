use crate::model::{Bounds, LayoutPoint};

use super::super::path_bounds::{SvgPathBounds, svg_path_bounds_from_d};
use super::defs::ClassMarkerPaintSpec;

pub(super) fn include_rect(
    bounds: &mut Option<Bounds>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) {
    // Match Chromium's `getBBox()` behavior: ignore placeholder boxes that should not affect
    // the measured diagram bounds.
    let w = (max_x - min_x).abs();
    let h = (max_y - min_y).abs();
    if (w < 1e-9 && h < 1e-9) || (w <= 0.1 + 1e-9 && h <= 0.1 + 1e-9) {
        return;
    }
    if let Some(cur) = bounds.as_mut() {
        cur.min_x = cur.min_x.min(min_x);
        cur.min_y = cur.min_y.min(min_y);
        cur.max_x = cur.max_x.max(max_x);
        cur.max_y = cur.max_y.max(max_y);
    } else {
        *bounds = Some(Bounds {
            min_x,
            min_y,
            max_x,
            max_y,
        });
    }
}

pub(super) fn include_xywh(bounds: &mut Option<Bounds>, x: f64, y: f64, w: f64, h: f64) {
    include_rect(bounds, x, y, x + w, y + h);
}

pub(super) fn include_path_d(bounds: &mut Option<Bounds>, d: &str, dx: f64, dy: f64) {
    include_path_d_with_outset(bounds, d, dx, dy, 0.0);
}

pub(super) fn include_path_d_with_outset(
    bounds: &mut Option<Bounds>,
    d: &str,
    dx: f64,
    dy: f64,
    outset: f64,
) {
    if let Some(pb) = svg_path_bounds_from_d(d) {
        include_path_bounds_with_outset(bounds, &pb, dx, dy, outset);
    }
}

pub(super) fn include_path_bounds_with_outset(
    bounds: &mut Option<Bounds>,
    pb: &SvgPathBounds,
    dx: f64,
    dy: f64,
    outset: f64,
) {
    let outset = outset.max(0.0);
    include_rect(
        bounds,
        pb.min_x + dx - outset,
        pb.min_y + dy - outset,
        pb.max_x + dx + outset,
        pb.max_y + dy + outset,
    );
}

pub(super) fn include_class_marker_paint_bounds(
    bounds: &mut Option<Bounds>,
    path_points: &[LayoutPoint],
    is_start: bool,
    marker: ClassMarkerPaintSpec,
    relation_stroke_width: f64,
    dx: f64,
    dy: f64,
) {
    let Some(endpoint) = (if is_start {
        path_points.first()
    } else {
        path_points.last()
    }) else {
        return;
    };

    let adjacent = if is_start {
        path_points
            .iter()
            .skip(1)
            .find(|point| (point.x - endpoint.x).hypot(point.y - endpoint.y) > f64::EPSILON)
    } else {
        path_points
            .iter()
            .rev()
            .skip(1)
            .find(|point| (point.x - endpoint.x).hypot(point.y - endpoint.y) > f64::EPSILON)
    };
    let (tangent_x, tangent_y) = adjacent.map_or((1.0, 0.0), |adjacent| {
        if is_start {
            (adjacent.x - endpoint.x, adjacent.y - endpoint.y)
        } else {
            (endpoint.x - adjacent.x, endpoint.y - adjacent.y)
        }
    });
    let tangent_length = tangent_x.hypot(tangent_y);
    let (cos_angle, sin_angle) = if tangent_length > f64::EPSILON {
        (tangent_x / tangent_length, tangent_y / tangent_length)
    } else {
        (1.0, 0.0)
    };

    let local_bounds = marker.local_paint_bounds();
    let (ref_x, ref_y) = marker.reference_point();
    let scale = marker.coordinate_scale(relation_stroke_width);
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (x, y) in [
        (local_bounds.min_x, local_bounds.min_y),
        (local_bounds.min_x, local_bounds.max_y),
        (local_bounds.max_x, local_bounds.min_y),
        (local_bounds.max_x, local_bounds.max_y),
    ] {
        let local_x = (x - ref_x) * scale;
        let local_y = (y - ref_y) * scale;
        let world_x = endpoint.x + local_x * cos_angle - local_y * sin_angle + dx;
        let world_y = endpoint.y + local_x * sin_angle + local_y * cos_angle + dy;
        min_x = min_x.min(world_x);
        min_y = min_y.min(world_y);
        max_x = max_x.max(world_x);
        max_y = max_y.max(world_y);
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        include_rect(bounds, min_x, min_y, max_x, max_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::parity::class::defs::class_marker_paint_spec;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual={actual} expected={expected}"
        );
    }

    #[test]
    fn horizontal_relation_bounds_keep_markers_in_user_space() {
        let points = vec![
            LayoutPoint { x: 10.0, y: 20.0 },
            LayoutPoint { x: 110.0, y: 20.0 },
        ];
        let mut bounds = None;

        include_class_marker_paint_bounds(
            &mut bounds,
            &points,
            true,
            class_marker_paint_spec(0, true, false).expect("aggregation start marker"),
            10.0,
            0.0,
            0.0,
        );
        include_class_marker_paint_bounds(
            &mut bounds,
            &points,
            false,
            class_marker_paint_spec(2, false, false).expect("composition end marker"),
            10.0,
            0.0,
            0.0,
        );

        let bounds = bounds.expect("marker paint bounds");
        assert_close(bounds.min_x, -11.0);
        assert_close(bounds.min_y, 10.0);
        assert_close(bounds.max_x, 131.0);
        assert_close(bounds.max_y, 30.0);
    }

    #[test]
    fn vertical_relation_respects_marker_coordinate_units() {
        let points = vec![
            LayoutPoint { x: 50.0, y: 100.0 },
            LayoutPoint { x: 50.0, y: 200.0 },
        ];
        let mut bounds = None;

        include_class_marker_paint_bounds(
            &mut bounds,
            &points,
            true,
            class_marker_paint_spec(1, true, false).expect("extension start marker"),
            10.0,
            0.0,
            0.0,
        );
        include_class_marker_paint_bounds(
            &mut bounds,
            &points,
            false,
            class_marker_paint_spec(3, false, false).expect("dependency end marker"),
            10.0,
            0.0,
            0.0,
        );

        let bounds = bounds.expect("marker paint bounds");
        assert_close(bounds.min_x, 40.0);
        assert_close(bounds.min_y, 79.0);
        assert_close(bounds.max_x, 60.0);
        assert_close(bounds.max_y, 209.0);
    }

    #[test]
    fn lollipop_marker_radius_is_independent_of_relation_stroke() {
        let marker = class_marker_paint_spec(4, true, false).expect("lollipop start marker");
        let radius = marker.conservative_radius(4.0);

        assert!(radius.is_finite());
        assert!(radius > 14.0 && radius < 15.0, "radius={radius}");
        assert_close(radius, marker.conservative_radius(1.0));
    }
}
