//! Swimlane endpoint docking from Mermaid 12.3 rendering-elements/edges.js.

use crate::model::LayoutPoint;

const AXIS_TOLERANCE: f64 = 0.5;

fn shares_column(first: &LayoutPoint, second: &LayoutPoint) -> bool {
    (first.x - second.x).abs() < AXIS_TOLERANCE
}

fn shares_row(first: &LayoutPoint, second: &LayoutPoint) -> bool {
    (first.y - second.y).abs() < AXIS_TOLERANCE
}

fn coincides(first: &LayoutPoint, second: &LayoutPoint) -> bool {
    shares_column(first, second) && shares_row(first, second)
}

fn dock_final_point(
    original: &LayoutPoint,
    inner: &LayoutPoint,
    before: &LayoutPoint,
    candidate: LayoutPoint,
) -> LayoutPoint {
    let column = shares_column(original, inner);
    let row = shares_row(original, inner);
    let keeps_axis = if column && !row {
        shares_column(&candidate, inner)
    } else if row && !column {
        shares_row(&candidate, inner)
    } else {
        shares_column(&candidate, inner) || shares_row(&candidate, inner)
    };
    if (column || row) && !keeps_axis {
        return original.clone();
    }
    let (heading, from) = if coincides(original, inner) {
        (inner, before)
    } else {
        (original, inner)
    };
    let dot = (heading.x - from.x) * (candidate.x - inner.x)
        + (heading.y - from.y) * (candidate.y - inner.y);
    if dot < 0.0 {
        return inner.clone();
    }
    if shares_row(heading, from) && !shares_column(heading, from) {
        LayoutPoint {
            x: candidate.x,
            y: inner.y,
        }
    } else if shares_column(heading, from) && !shares_row(heading, from) {
        LayoutPoint {
            x: inner.x,
            y: candidate.y,
        }
    } else {
        candidate
    }
}

fn marker_pull_back(marker: Option<&str>) -> f64 {
    match marker {
        Some("arrow_barb_neo") => 5.5,
        _ => crate::svg::parity::edge_path::marker_offset_for(marker).unwrap_or(0.0),
    }
}

pub(in crate::svg::parity::flowchart) fn clip_swimlane_endpoints(
    points: &[LayoutPoint],
    mut tail_intersect: impl FnMut(&LayoutPoint) -> LayoutPoint,
    mut head_intersect: impl FnMut(&LayoutPoint) -> LayoutPoint,
    arrow_type_end: Option<&str>,
    out: &mut Vec<LayoutPoint>,
) {
    out.clear();
    out.reserve(points.len());
    if points.len() < 2 {
        out.extend_from_slice(points);
        return;
    }
    if points.len() == 2 {
        let (first, last) = (&points[0], &points[1]);
        if shares_column(first, last) || shares_row(first, last) {
            out.push(tail_intersect(first));
            out.push(head_intersect(last));
        } else {
            out.push(tail_intersect(last));
            out.push(head_intersect(first));
        }
        return;
    }
    let inner = &points[1..points.len() - 1];
    let first_inner = &inner[0];
    let last_inner = &inner[inner.len() - 1];
    let original_last = &points[points.len() - 1];
    let before = inner
        .len()
        .checked_sub(2)
        .map_or(&points[0], |index| &inner[index]);
    let first = tail_intersect(first_inner);
    let candidate = head_intersect(last_inner);
    let last = dock_final_point(original_last, last_inner, before, candidate);
    let length = (last.x - last_inner.x).hypot(last.y - last_inner.y);
    let final_point = if length >= AXIS_TOLERANCE.max(marker_pull_back(arrow_type_end)) {
        Some(last)
    } else if !coincides(original_last, last_inner) && length >= AXIS_TOLERANCE {
        Some(original_last.clone())
    } else {
        None
    };
    if !coincides(&first, first_inner) {
        out.push(first);
    }
    out.extend_from_slice(inner);
    if let Some(last) = final_point {
        out.push(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn coordinates(points: &[LayoutPoint]) -> Vec<(f64, f64)> {
        points.iter().map(|point| (point.x, point.y)).collect()
    }

    #[test]
    fn slanted_two_point_routes_intersect_the_far_endpoint() {
        let points = [point(0.0, 0.0), point(100.0, 50.0)];
        let mut output = Vec::new();
        clip_swimlane_endpoints(
            &points,
            |far| {
                assert_eq!((far.x, far.y), (100.0, 50.0));
                point(20.0, 10.0)
            },
            |far| {
                assert_eq!((far.x, far.y), (0.0, 0.0));
                point(80.0, 40.0)
            },
            Some("arrow_point"),
            &mut output,
        );
        assert_eq!(coordinates(&output), [(20.0, 10.0), (80.0, 40.0)]);
    }

    #[test]
    fn axis_aligned_two_point_routes_preserve_each_offset_port() {
        let points = [point(4.0, 10.0), point(4.0, 90.0)];
        let mut output = Vec::new();
        clip_swimlane_endpoints(
            &points,
            |own| {
                assert_eq!((own.x, own.y), (4.0, 10.0));
                own.clone()
            },
            |own| {
                assert_eq!((own.x, own.y), (4.0, 90.0));
                own.clone()
            },
            Some("arrow_point"),
            &mut output,
        );
        assert_eq!(coordinates(&output), [(4.0, 10.0), (4.0, 90.0)]);
    }

    #[test]
    fn final_intersection_keeps_arrival_axis_and_heading() {
        for (original, candidate, expected) in [
            (point(40.0, 100.0), point(55.0, 60.0), point(40.0, 100.0)),
            (point(40.0, 100.0), point(40.0, 50.0), point(40.0, 60.0)),
            (point(40.0, 100.0), point(40.2, 90.0), point(40.0, 90.0)),
        ] {
            let points = [
                point(40.0, 0.0),
                point(40.0, 20.0),
                point(40.0, 60.0),
                original,
            ];
            let mut output = Vec::new();
            clip_swimlane_endpoints(
                &points,
                Clone::clone,
                |_| candidate.clone(),
                Some("arrow_point"),
                &mut output,
            );
            assert_eq!(
                (output.last().unwrap().x, output.last().unwrap().y),
                (expected.x, expected.y)
            );
        }
    }

    #[test]
    fn snapped_short_stubs_keep_turns_and_drop_backwards_marker_tails() {
        let mut output = Vec::new();
        let points = [point(0.0, 0.0), point(0.0, 100.0), point(10.0, 100.0)];
        clip_swimlane_endpoints(
            &points,
            Clone::clone,
            |_| point(5.0, 100.0),
            Some("aggregation"),
            &mut output,
        );
        assert_eq!(coordinates(&output), [(0.0, 100.0), (10.0, 100.0)]);
        let pinned = [
            point(0.0, 0.0),
            point(0.0, 100.0),
            point(300.0, 100.0),
            point(300.0, 100.0),
        ];
        clip_swimlane_endpoints(
            &pinned,
            Clone::clone,
            |_| point(301.0, 100.1),
            Some("arrow_point"),
            &mut output,
        );
        assert_eq!(coordinates(&output), [(0.0, 100.0), (300.0, 100.0)]);
        for marker in ["constructor", "__proto__", "arrow_open"] {
            let tiny = [point(0.0, 0.0), point(0.0, 100.0), point(0.0, 100.3)];
            clip_swimlane_endpoints(
                &tiny,
                Clone::clone,
                |_| point(0.0, 100.15),
                Some(marker),
                &mut output,
            );
            assert_eq!(coordinates(&output), [(0.0, 100.0)]);
        }
    }
}
