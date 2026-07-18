//! Pure Mermaid line-hop geometry.
//!
//! This is a DOM-free port of Mermaid's `lineJump.ts`. Callers retain ownership
//! of curve/path eligibility and can consume the returned SVG `d` strings
//! directly once the caller has applied the source-backed curve eligibility rule.

use crate::model::LayoutPoint;
use std::cmp::Ordering;
use std::collections::HashMap;

// Kept in sync with Mermaid's `generateRoundedPath` radius.
pub(in crate::svg::parity::flowchart) const ROUNDED_CORNER_RADIUS: f64 = 5.0;
const CORNER_EPSILON: f64 = 1e-5;
const ENDPOINT_EPSILON: f64 = 1e-6;
const MIN_JUMP_RADIUS: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::svg::parity::flowchart) enum LineHopStyle {
    Arc,
    Gap,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct LineHopConfig {
    pub(in crate::svg::parity::flowchart) enabled: bool,
    pub(in crate::svg::parity::flowchart) jump_radius: f64,
    pub(in crate::svg::parity::flowchart) jump_style: LineHopStyle,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct LineHopEdge<'a> {
    pub(in crate::svg::parity::flowchart) id: &'a str,
    pub(in crate::svg::parity::flowchart) points: &'a [LayoutPoint],
    pub(in crate::svg::parity::flowchart) curve: Option<&'a str>,
    pub(in crate::svg::parity::flowchart) arrow_type_start: Option<&'a str>,
    pub(in crate::svg::parity::flowchart) arrow_type_end: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(in crate::svg::parity::flowchart) struct LineHopCrossing<'a> {
    pub(in crate::svg::parity::flowchart) jump_edge_id: &'a str,
    pub(in crate::svg::parity::flowchart) other_edge_id: &'a str,
    pub(in crate::svg::parity::flowchart) segment_index: usize,
    pub(in crate::svg::parity::flowchart) t: f64,
    pub(in crate::svg::parity::flowchart) point: LayoutPoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::svg::parity::flowchart) struct LineHopPath<'a> {
    pub(in crate::svg::parity::flowchart) edge_id: &'a str,
    pub(in crate::svg::parity::flowchart) path: String,
    pub(in crate::svg::parity::flowchart) has_hops: bool,
}

#[derive(Debug, Clone, Copy)]
struct Segment<'a> {
    start: &'a LayoutPoint,
    end: &'a LayoutPoint,
}

#[derive(Debug, Clone)]
struct SegmentIntersection {
    point: LayoutPoint,
    t_a: f64,
    t_b: f64,
}

#[derive(Debug, Clone)]
struct JumpOnSegment {
    t: f64,
    point: LayoutPoint,
    distance: f64,
    radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct RoundedCorner {
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    control_x: f64,
    control_y: f64,
    cut_length: f64,
}

pub(in crate::svg::parity::flowchart) fn find_edge_intersections<'a>(
    edges: &[LineHopEdge<'a>],
) -> Vec<LineHopCrossing<'a>> {
    let mut crossings = Vec::new();

    for (edge_a_index, edge_a) in edges.iter().enumerate() {
        for edge_b in &edges[edge_a_index + 1..] {
            for (segment_a_index, pair_a) in edge_a.points.windows(2).enumerate() {
                let segment_a = Segment {
                    start: &pair_a[0],
                    end: &pair_a[1],
                };
                for (segment_b_index, pair_b) in edge_b.points.windows(2).enumerate() {
                    let segment_b = Segment {
                        start: &pair_b[0],
                        end: &pair_b[1],
                    };
                    let Some(intersection) = segment_intersection(segment_a, segment_b) else {
                        continue;
                    };

                    let a_is_horizontal = is_horizontally_dominant(segment_a);
                    let b_is_horizontal = is_horizontally_dominant(segment_b);
                    let jump_on_a = a_is_horizontal != b_is_horizontal && a_is_horizontal;

                    if jump_on_a {
                        crossings.push(LineHopCrossing {
                            jump_edge_id: edge_a.id,
                            other_edge_id: edge_b.id,
                            segment_index: segment_a_index,
                            t: intersection.t_a,
                            point: intersection.point,
                        });
                    } else {
                        crossings.push(LineHopCrossing {
                            jump_edge_id: edge_b.id,
                            other_edge_id: edge_a.id,
                            segment_index: segment_b_index,
                            t: intersection.t_b,
                            point: intersection.point,
                        });
                    }
                }
            }
        }
    }

    crossings
}

pub(in crate::svg::parity::flowchart) fn process_edges_with_line_hops<'a>(
    edges: &[LineHopEdge<'a>],
    config: LineHopConfig,
) -> Vec<LineHopPath<'a>> {
    if !config.enabled {
        return edges
            .iter()
            .map(|edge| LineHopPath {
                edge_id: edge.id,
                path: plain_path(edge.points),
                has_hops: false,
            })
            .collect();
    }

    let mut jumps_by_edge: HashMap<&'a str, Vec<LineHopCrossing<'a>>> = HashMap::new();
    for crossing in find_edge_intersections(edges) {
        debug_assert_ne!(crossing.jump_edge_id, crossing.other_edge_id);
        jumps_by_edge
            .entry(crossing.jump_edge_id)
            .or_default()
            .push(crossing);
    }

    edges
        .iter()
        .map(|edge| {
            let Some(jumps) = jumps_by_edge.get(edge.id) else {
                return LineHopPath {
                    edge_id: edge.id,
                    path: plain_path(edge.points),
                    has_hops: false,
                };
            };
            let (path, has_hops) = rewrite_edge_path(*edge, jumps, config);
            LineHopPath {
                edge_id: edge.id,
                path,
                has_hops,
            }
        })
        .collect()
}

pub(in crate::svg::parity::flowchart) fn curve_supports_line_hops(curve: Option<&str>) -> bool {
    matches!(
        curve,
        None | Some("linear" | "rounded" | "step" | "stepBefore" | "stepAfter")
    )
}

#[cfg(test)]
fn is_straight_path(path: &str) -> bool {
    path.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_whitespace()
            || character == '\u{feff}'
            || matches!(
                character,
                '+' | ',' | '.' | 'L' | 'M' | 'e' | 'l' | 'm' | '-'
            )
    })
}

fn build_segments(points: &[LayoutPoint]) -> Vec<Segment<'_>> {
    points
        .windows(2)
        .map(|pair| Segment {
            start: &pair[0],
            end: &pair[1],
        })
        .collect()
}

fn segment_intersection(
    segment_a: Segment<'_>,
    segment_b: Segment<'_>,
) -> Option<SegmentIntersection> {
    let dx_a = segment_a.end.x - segment_a.start.x;
    let dy_a = segment_a.end.y - segment_a.start.y;
    let dx_b = segment_b.end.x - segment_b.start.x;
    let dy_b = segment_b.end.y - segment_b.start.y;
    let denominator = dx_a * dy_b - dy_a * dx_b;
    if denominator == 0.0 {
        return None;
    }

    let dx = segment_b.start.x - segment_a.start.x;
    let dy = segment_b.start.y - segment_a.start.y;
    let t_a = (dx * dy_b - dy * dx_b) / denominator;
    let t_b = (dx * dy_a - dy * dx_a) / denominator;

    if t_a <= ENDPOINT_EPSILON
        || t_a >= 1.0 - ENDPOINT_EPSILON
        || t_b <= ENDPOINT_EPSILON
        || t_b >= 1.0 - ENDPOINT_EPSILON
    {
        return None;
    }

    Some(SegmentIntersection {
        point: LayoutPoint {
            x: segment_a.start.x + t_a * dx_a,
            y: segment_a.start.y + t_a * dy_a,
        },
        t_a,
        t_b,
    })
}

fn is_horizontally_dominant(segment: Segment<'_>) -> bool {
    (segment.end.x - segment.start.x).abs() >= (segment.end.y - segment.start.y).abs()
}

fn fmt_number(value: f64) -> String {
    let scaled = value * 1000.0;
    let rounded = if scaled.is_finite() {
        (scaled + 0.5).floor() / 1000.0
    } else {
        value
    };

    if rounded == 0.0 {
        "0".to_owned()
    } else if rounded == f64::INFINITY {
        "Infinity".to_owned()
    } else if rounded == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        rounded.to_string()
    }
}

fn point_to_string(point: &LayoutPoint) -> String {
    format!("{},{}", fmt_number(point.x), fmt_number(point.y))
}

fn arc_sweep_flag(segment: Segment<'_>) -> u8 {
    let dx = segment.end.x - segment.start.x;
    let dy = segment.end.y - segment.start.y;
    if dx.abs() >= dy.abs() {
        u8::from(dx >= 0.0)
    } else {
        u8::from(dy >= 0.0)
    }
}

// `lineJump.ts` imports this exact table from `utils/lineWithOffset.ts`.
// It is mirrored here because the existing flowchart offset helper is private
// to a sibling module and this geometry port must remain independently usable.
fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
    match arrow_type {
        Some("aggregation" | "extension" | "composition") => Some(17.25),
        Some("dependency") => Some(6.0),
        Some("lollipop") => Some(13.5),
        Some("arrow_point") => Some(4.0),
        Some("arrow_barb") => Some(0.0),
        Some("arrow_barb_neo") => Some(5.5),
        _ => None,
    }
}

fn apply_marker_offsets(points: &[LayoutPoint], edge: LineHopEdge<'_>) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut offset_points = points.to_vec();
    if let Some(offset) = marker_offset_for(edge.arrow_type_start).filter(|offset| *offset != 0.0) {
        let start = &points[0];
        let next = &points[1];
        let angle = (next.y - start.y).atan2(next.x - start.x);
        offset_points[0].x = start.x + offset * angle.cos();
        offset_points[0].y = start.y + offset * angle.sin();
    }

    if let Some(offset) = marker_offset_for(edge.arrow_type_end).filter(|offset| *offset != 0.0) {
        let last_index = points.len() - 1;
        let previous = &points[last_index - 1];
        let end = &points[last_index];
        let angle = (end.y - previous.y).atan2(end.x - previous.x);
        offset_points[last_index].x = end.x - offset * angle.cos();
        offset_points[last_index].y = end.y - offset * angle.sin();
    }

    offset_points
}

fn compute_rounded_corner(
    previous: &LayoutPoint,
    current: &LayoutPoint,
    next: &LayoutPoint,
) -> Option<RoundedCorner> {
    let incoming_dx = current.x - previous.x;
    let incoming_dy = current.y - previous.y;
    let outgoing_dx = next.x - current.x;
    let outgoing_dy = next.y - current.y;
    let incoming_length = incoming_dx.hypot(incoming_dy);
    let outgoing_length = outgoing_dx.hypot(outgoing_dy);
    if incoming_length < CORNER_EPSILON || outgoing_length < CORNER_EPSILON {
        return None;
    }

    let incoming_x = incoming_dx / incoming_length;
    let incoming_y = incoming_dy / incoming_length;
    let outgoing_x = outgoing_dx / outgoing_length;
    let outgoing_y = outgoing_dy / outgoing_length;
    let dot_product = incoming_x * outgoing_x + incoming_y * outgoing_y;
    let clamped_dot = dot_product.clamp(-1.0, 1.0);
    let angle = clamped_dot.acos();
    if angle < CORNER_EPSILON || (std::f64::consts::PI - angle).abs() < CORNER_EPSILON {
        return None;
    }

    let cut_length = (ROUNDED_CORNER_RADIUS / (angle / 2.0).sin())
        .min(incoming_length / 2.0)
        .min(outgoing_length / 2.0);
    Some(RoundedCorner {
        start_x: current.x - incoming_x * cut_length,
        start_y: current.y - incoming_y * cut_length,
        end_x: current.x + outgoing_x * cut_length,
        end_y: current.y + outgoing_y * cut_length,
        control_x: current.x,
        control_y: current.y,
        cut_length,
    })
}

fn emit_jump(
    jump: &JumpOnSegment,
    unit_x: f64,
    unit_y: f64,
    sweep: u8,
    style: LineHopStyle,
    parts: &mut Vec<String>,
) {
    let before = LayoutPoint {
        x: jump.point.x - unit_x * jump.radius,
        y: jump.point.y - unit_y * jump.radius,
    };
    let after = LayoutPoint {
        x: jump.point.x + unit_x * jump.radius,
        y: jump.point.y + unit_y * jump.radius,
    };
    parts.push(format!("L{}", point_to_string(&before)));
    match style {
        LineHopStyle::Arc => parts.push(format!(
            "A{},{} 0 0 {} {}",
            fmt_number(jump.radius),
            fmt_number(jump.radius),
            sweep,
            point_to_string(&after)
        )),
        LineHopStyle::Gap => parts.push(format!("M{}", point_to_string(&after))),
    }
}

fn rewrite_edge_path(
    edge: LineHopEdge<'_>,
    jumps: &[LineHopCrossing<'_>],
    config: LineHopConfig,
) -> (String, bool) {
    if edge.points.len() < 2 {
        return (String::new(), false);
    }

    let points = apply_marker_offsets(edge.points, edge);
    let segments = build_segments(&points);
    let mut jumps_by_segment: HashMap<usize, Vec<JumpOnSegment>> = HashMap::new();
    for jump in jumps {
        let Some(segment) = segments.get(jump.segment_index) else {
            continue;
        };
        let segment_length =
            (segment.end.x - segment.start.x).hypot(segment.end.y - segment.start.y);
        jumps_by_segment
            .entry(jump.segment_index)
            .or_default()
            .push(JumpOnSegment {
                t: jump.t,
                point: jump.point.clone(),
                distance: jump.t * segment_length,
                radius: config.jump_radius,
            });
    }

    let rounded = edge.curve == Some("rounded");
    let mut parts = vec![format!("M{}", point_to_string(&points[0]))];
    let mut emitted_hop = false;

    for (segment_index, segment) in segments.iter().enumerate() {
        let segment_length =
            (segment.end.x - segment.start.x).hypot(segment.end.y - segment.start.y);
        let (unit_x, unit_y) = if segment_length == 0.0 {
            (0.0, 0.0)
        } else {
            (
                (segment.end.x - segment.start.x) / segment_length,
                (segment.end.y - segment.start.y) / segment_length,
            )
        };
        let sweep = arc_sweep_flag(*segment);

        let segment_start_consumed = if rounded && segment_index > 0 {
            compute_rounded_corner(
                &points[segment_index - 1],
                &points[segment_index],
                points
                    .get(segment_index + 1)
                    .unwrap_or(&points[segment_index]),
            )
            .map_or(0.0, |corner| corner.cut_length)
        } else {
            0.0
        };

        let upcoming_corner = if rounded && segment_index < segments.len() - 1 {
            compute_rounded_corner(
                &points[segment_index],
                &points[segment_index + 1],
                points
                    .get(segment_index + 2)
                    .unwrap_or(&points[segment_index + 1]),
            )
        } else {
            None
        };
        let segment_end_stop =
            upcoming_corner.map_or(segment_length, |corner| segment_length - corner.cut_length);

        let mut segment_jumps = jumps_by_segment.remove(&segment_index).unwrap_or_default();
        segment_jumps
            .sort_by(|left, right| left.t.partial_cmp(&right.t).unwrap_or(Ordering::Equal));
        for jump in &mut segment_jumps {
            jump.radius = jump
                .radius
                .min(jump.distance - segment_start_consumed)
                .min(segment_end_stop - jump.distance);
        }
        for index in 0..segment_jumps.len().saturating_sub(1) {
            let (before, after) = segment_jumps.split_at_mut(index + 1);
            let first = &mut before[index];
            let second = &mut after[0];
            let gap = second.distance - first.distance;
            if first.radius + second.radius > gap {
                let half_gap = gap / 2.0;
                first.radius = first.radius.min(half_gap);
                second.radius = second.radius.min(half_gap);
            }
        }

        for jump in &segment_jumps {
            if jump.radius < MIN_JUMP_RADIUS {
                continue;
            }
            emit_jump(jump, unit_x, unit_y, sweep, config.jump_style, &mut parts);
            emitted_hop = true;
        }

        if let Some(corner) = upcoming_corner {
            parts.push(format!(
                "L{},{}",
                fmt_number(corner.start_x),
                fmt_number(corner.start_y)
            ));
            parts.push(format!(
                "Q{},{} {},{}",
                fmt_number(corner.control_x),
                fmt_number(corner.control_y),
                fmt_number(corner.end_x),
                fmt_number(corner.end_y)
            ));
        } else {
            parts.push(format!("L{}", point_to_string(segment.end)));
        }
    }

    (parts.join(" "), emitted_hop)
}

fn plain_path(points: &[LayoutPoint]) -> String {
    let Some(first) = points.first() else {
        return String::new();
    };
    let mut parts = Vec::with_capacity(points.len());
    parts.push(format!("M{}", point_to_string(first)));
    parts.extend(
        points[1..]
            .iter()
            .map(|point| format!("L{}", point_to_string(point))),
    );
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn edge<'a>(id: &'a str, points: &'a [LayoutPoint]) -> LineHopEdge<'a> {
        LineHopEdge {
            id,
            points,
            curve: None,
            arrow_type_start: None,
            arrow_type_end: None,
        }
    }

    fn arc_config(radius: f64) -> LineHopConfig {
        LineHopConfig {
            enabled: true,
            jump_radius: radius,
            jump_style: LineHopStyle::Arc,
        }
    }

    fn path_for<'a>(paths: &'a [LineHopPath<'_>], id: &str) -> &'a str {
        &paths
            .iter()
            .find(|path| path.edge_id == id)
            .expect("edge path")
            .path
    }

    #[test]
    fn horizontal_segment_hops_over_vertical_regardless_of_edge_order() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];

        for edges in [
            [edge("horizontal", &horizontal), edge("vertical", &vertical)],
            [edge("vertical", &vertical), edge("horizontal", &horizontal)],
        ] {
            let crossings = find_edge_intersections(&edges);
            assert_eq!(crossings.len(), 1);
            assert_eq!(crossings[0].jump_edge_id, "horizontal");
            assert_eq!(crossings[0].other_edge_id, "vertical");
            assert_eq!(crossings[0].segment_index, 0);
            assert_eq!(crossings[0].t, 0.5);
            assert_eq!(crossings[0].point.x, 5.0);
            assert_eq!(crossings[0].point.y, 5.0);
        }
    }

    #[test]
    fn same_orientation_crossing_assigns_hop_to_later_edge() {
        let first = [point(0.0, 0.0), point(10.0, 2.0)];
        let second = [point(0.0, 2.0), point(10.0, 0.0)];
        let crossings = find_edge_intersections(&[edge("first", &first), edge("second", &second)]);

        assert_eq!(crossings.len(), 1);
        assert_eq!(crossings[0].jump_edge_id, "second");
        assert_eq!(crossings[0].other_edge_id, "first");
    }

    #[test]
    fn endpoint_touches_shared_endpoints_and_parallel_segments_are_not_crossings() {
        let backbone = [point(0.0, 5.0), point(10.0, 5.0)];
        let t_junction = [point(5.0, 0.0), point(5.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("backbone", &backbone), edge("touch", &t_junction),])
                .is_empty()
        );

        let fan_a = [point(0.0, 0.0), point(10.0, 5.0)];
        let fan_b = [point(0.0, 0.0), point(10.0, -5.0)];
        assert!(find_edge_intersections(&[edge("a", &fan_a), edge("b", &fan_b)]).is_empty());

        let converge_a = [point(0.0, 0.0), point(10.0, 5.0)];
        let converge_b = [point(0.0, 10.0), point(10.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("a", &converge_a), edge("b", &converge_b),]).is_empty()
        );

        let parallel_a = [point(0.0, 0.0), point(10.0, 0.0)];
        let parallel_b = [point(0.0, 5.0), point(10.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("a", &parallel_a), edge("b", &parallel_b),]).is_empty()
        );
    }

    #[test]
    fn emits_arc_and_gap_paths_without_rewriting_the_other_edge() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];
        let edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];

        let arc_paths = process_edges_with_line_hops(&edges, arc_config(1.0));
        assert_eq!(
            path_for(&arc_paths, "horizontal"),
            "M0,5 L4,5 A1,1 0 0 1 6,5 L10,5"
        );
        assert_eq!(path_for(&arc_paths, "vertical"), "M5,0 L5,10");
        assert!(arc_paths[0].has_hops);
        assert!(!arc_paths[1].has_hops);

        let gap_paths = process_edges_with_line_hops(
            &edges,
            LineHopConfig {
                jump_style: LineHopStyle::Gap,
                ..arc_config(1.0)
            },
        );
        assert_eq!(path_for(&gap_paths, "horizontal"), "M0,5 L4,5 M6,5 L10,5");
    }

    #[test]
    fn crossings_are_emitted_in_segment_order_and_adjacent_radii_are_clamped() {
        let vertical_a = [point(4.5, 0.0), point(4.5, 10.0)];
        let vertical_b = [point(5.5, 0.0), point(5.5, 10.0)];
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let edges = [
            edge("vertical-a", &vertical_a),
            edge("vertical-b", &vertical_b),
            edge("horizontal", &horizontal),
        ];

        assert_eq!(
            path_for(
                &process_edges_with_line_hops(&edges, arc_config(1.0)),
                "horizontal"
            ),
            "M0,5 L4,5 A0.5,0.5 0 0 1 5,5 L5,5 A0.5,0.5 0 0 1 6,5 L10,5"
        );
    }

    #[test]
    fn requested_radius_is_clamped_to_segment_boundaries_and_tiny_hops_are_omitted() {
        let close_vertical = [point(0.5, -1.0), point(0.5, 1.0)];
        let horizontal = [point(0.0, 0.0), point(10.0, 0.0)];
        let edges = [
            edge("vertical", &close_vertical),
            edge("horizontal", &horizontal),
        ];
        assert_eq!(
            path_for(
                &process_edges_with_line_hops(&edges, arc_config(2.0)),
                "horizontal"
            ),
            "M0,0 L0,0 A0.5,0.5 0 0 1 1,0 L10,0"
        );

        let almost_at_start = [point(0.0005, -1.0), point(0.0005, 1.0)];
        let tiny_edges = [
            edge("vertical", &almost_at_start),
            edge("horizontal", &horizontal),
        ];
        let paths = process_edges_with_line_hops(&tiny_edges, arc_config(2.0));
        assert_eq!(path_for(&paths, "horizontal"), "M0,0 L10,0");
        assert!(!paths[1].has_hops);
    }

    #[test]
    fn rounded_edges_preserve_five_pixel_corners_and_share_space_with_hops() {
        let vertical = [point(14.0, 0.0), point(14.0, 10.0)];
        let rounded_points = [point(0.0, 5.0), point(20.0, 5.0), point(20.0, 15.0)];
        let rounded = LineHopEdge {
            curve: Some("rounded"),
            ..edge("rounded", &rounded_points)
        };
        let paths =
            process_edges_with_line_hops(&[edge("vertical", &vertical), rounded], arc_config(5.0));

        assert_eq!(
            path_for(&paths, "rounded"),
            "M0,5 L13,5 A1,1 0 0 1 15,5 L15,5 Q20,5 20,10 L20,15"
        );
        assert_eq!(ROUNDED_CORNER_RADIUS, 5.0);
    }

    #[test]
    fn marker_offsets_match_upstream_table_and_shift_rewritten_endpoints() {
        let expected = [
            ("aggregation", 17.25),
            ("extension", 17.25),
            ("composition", 17.25),
            ("dependency", 6.0),
            ("lollipop", 13.5),
            ("arrow_point", 4.0),
            ("arrow_barb", 0.0),
            ("arrow_barb_neo", 5.5),
        ];
        for (name, offset) in expected {
            assert_eq!(marker_offset_for(Some(name)), Some(offset));
        }
        assert_eq!(marker_offset_for(Some("arrow_cross")), None);

        let horizontal_points = [point(0.0, 5.0), point(20.0, 5.0)];
        let horizontal = LineHopEdge {
            arrow_type_end: Some("arrow_point"),
            ..edge("horizontal", &horizontal_points)
        };
        let vertical = [point(10.0, 0.0), point(10.0, 20.0)];
        let paths = process_edges_with_line_hops(
            &[horizontal, edge("vertical", &vertical)],
            arc_config(1.0),
        );
        assert_eq!(
            path_for(&paths, "horizontal"),
            "M0,5 L9,5 A1,1 0 0 1 11,5 L16,5"
        );

        let long_horizontal_points = [point(0.0, 5.0), point(40.0, 5.0)];
        let both_markers = LineHopEdge {
            arrow_type_start: Some("arrow_point"),
            arrow_type_end: Some("dependency"),
            ..edge("both-markers", &long_horizontal_points)
        };
        let vertical = [point(20.0, 0.0), point(20.0, 20.0)];
        let paths = process_edges_with_line_hops(
            &[both_markers, edge("vertical", &vertical)],
            arc_config(1.0),
        );
        assert_eq!(
            path_for(&paths, "both-markers"),
            "M4,5 L19,5 A1,1 0 0 1 21,5 L34,5"
        );
    }

    #[test]
    fn reverse_segments_use_the_upward_or_rightward_arc_sweep() {
        let descending_horizontal = [point(10.0, 5.0), point(0.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];
        let paths = process_edges_with_line_hops(
            &[
                edge("horizontal", &descending_horizontal),
                edge("vertical", &vertical),
            ],
            arc_config(1.0),
        );
        assert_eq!(
            path_for(&paths, "horizontal"),
            "M10,5 L6,5 A1,1 0 0 0 4,5 L0,5"
        );

        let down_start = point(5.0, 0.0);
        let down_end = point(5.0, 10.0);
        let up_start = point(5.0, 10.0);
        let up_end = point(5.0, 0.0);
        assert_eq!(
            arc_sweep_flag(Segment {
                start: &down_start,
                end: &down_end,
            }),
            1
        );
        assert_eq!(
            arc_sweep_flag(Segment {
                start: &up_start,
                end: &up_end,
            }),
            0
        );
    }

    #[test]
    fn disabled_config_returns_plain_polylines() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];
        let paths = process_edges_with_line_hops(
            &[edge("horizontal", &horizontal), edge("vertical", &vertical)],
            LineHopConfig {
                enabled: false,
                ..arc_config(1.0)
            },
        );

        assert_eq!(path_for(&paths, "horizontal"), "M0,5 L10,5");
        assert_eq!(path_for(&paths, "vertical"), "M5,0 L5,10");
        assert!(paths.iter().all(|path| !path.has_hops));
    }

    #[test]
    fn curve_and_rendered_path_guards_match_upstream_whitelists() {
        for curve in [
            None,
            Some("linear"),
            Some("rounded"),
            Some("step"),
            Some("stepBefore"),
            Some("stepAfter"),
        ] {
            assert!(curve_supports_line_hops(curve), "curve={curve:?}");
        }
        for curve in [Some("basis"), Some("monotoneX"), Some("cardinal")] {
            assert!(!curve_supports_line_hops(curve), "curve={curve:?}");
        }

        for path in ["", "M0,5 L10,5", "M5,0 L5,4 L5,10", "m1e-3,+2 l.5,-4"] {
            assert!(is_straight_path(path), "path={path}");
        }
        for path in [
            "M0,0 C1,1 2,2 3,3",
            "M0,0 Q1,1 2,2",
            "M0,0 L1,1 A1,1 0 0 1 2,2",
            "M0,0 H2",
        ] {
            assert!(!is_straight_path(path), "path={path}");
        }
    }
}
