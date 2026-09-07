//! Renderer-neutral geometry shared by canonical documents and target serializers.
//!
//! Geometry in this module is expressed with the public DrawingList primitives so SVG, native
//! targets, and future exporters can consume one typed result instead of reimplementing curves.

use crate::model::LayoutPoint;
use merman_display_list::{PathSegment, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartCurveKind {
    Linear,
    Basis,
    Step,
    StepBefore,
    StepAfter,
    Rounded,
}

impl FlowchartCurveKind {
    pub(crate) fn from_mermaid_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "linear" => Some(Self::Linear),
            "basis" => Some(Self::Basis),
            "step" => Some(Self::Step),
            "stepbefore" => Some(Self::StepBefore),
            "stepafter" => Some(Self::StepAfter),
            "rounded" => Some(Self::Rounded),
            _ => None,
        }
    }
}

pub(crate) fn flowchart_curve_segments(
    points: &[LayoutPoint],
    curve: FlowchartCurveKind,
    rounded_radius: f64,
    compact_rounded_corners: bool,
    rounded_corner_mask: Option<&[bool]>,
) -> Vec<PathSegment> {
    let mut out = Vec::new();
    let result: std::result::Result<(), std::convert::Infallible> = emit_flowchart_curve_segments(
        points,
        curve,
        rounded_radius,
        compact_rounded_corners,
        rounded_corner_mask,
        |segment| {
            out.push(segment);
            Ok(())
        },
    );
    match result {
        Ok(()) => out,
        Err(never) => match never {},
    }
}

/// Emits curve geometry incrementally, stopping immediately when the sink rejects a segment.
pub(crate) fn emit_flowchart_curve_segments<E>(
    points: &[LayoutPoint],
    curve: FlowchartCurveKind,
    rounded_radius: f64,
    compact_rounded_corners: bool,
    rounded_corner_mask: Option<&[bool]>,
    emit: impl FnMut(PathSegment) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    match curve {
        FlowchartCurveKind::Linear => emit_linear_segments(points, emit),
        FlowchartCurveKind::Basis => emit_basis_segments(points, emit),
        FlowchartCurveKind::Step => emit_step_segments(points, StepCurve::Midpoint, emit),
        FlowchartCurveKind::StepBefore => emit_step_segments(points, StepCurve::Before, emit),
        FlowchartCurveKind::StepAfter => emit_step_segments(points, StepCurve::After, emit),
        FlowchartCurveKind::Rounded => emit_rounded_segments(
            points,
            rounded_radius,
            compact_rounded_corners,
            rounded_corner_mask,
            emit,
        ),
    }
}

/// Applies Mermaid State's renderer-owned curve preparation to layout points.
///
/// State layout owns endpoint intersection. The renderer then rounds orthogonal corners and, for
/// the neo barb marker, shortens the terminal point. Keeping this transformation here gives SVG
/// and DrawingList one typed geometry source.
pub(crate) fn state_curve_points(
    points: &[LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<LayoutPoint> {
    let finite_points = points
        .iter()
        .filter(|point| !point.y.is_nan())
        .cloned()
        .collect::<Vec<_>>();
    let rounded = state_fix_corners(&finite_points);
    state_line_with_end_marker_offset_points(&rounded, arrow_type_end)
}

/// Builds one Mermaid Pie slice around the local origin.
///
/// Angles use Mermaid's convention: zero points upward and positive angles advance clockwise in
/// the y-down coordinate system. Donut holes are represented by a counter-clockwise inner
/// subpath so the default non-zero fill rule preserves the hole.
pub(crate) fn pie_slice_segments(
    radius: f64,
    inner_radius: f64,
    start_angle: f64,
    end_angle: f64,
    is_full_circle: bool,
) -> Option<Vec<PathSegment>> {
    if ![radius, inner_radius, start_angle, end_angle]
        .into_iter()
        .all(f64::is_finite)
        || radius <= 0.0
        || inner_radius < 0.0
        || inner_radius >= radius
        || (!is_full_circle && end_angle <= start_angle)
    {
        return None;
    }

    if is_full_circle {
        let outer_top = Point::new(0.0, -radius);
        let outer_bottom = Point::new(0.0, radius);
        let mut segments = vec![
            PathSegment::MoveTo { to: outer_top },
            PathSegment::ArcTo {
                radius_x: radius,
                radius_y: radius,
                x_axis_rotation_degrees: 0.0,
                large_arc: true,
                sweep_clockwise: true,
                to: outer_bottom,
            },
            PathSegment::ArcTo {
                radius_x: radius,
                radius_y: radius,
                x_axis_rotation_degrees: 0.0,
                large_arc: true,
                sweep_clockwise: true,
                to: outer_top,
            },
        ];
        if inner_radius > 0.0 {
            let inner_top = Point::new(0.0, -inner_radius);
            let inner_bottom = Point::new(0.0, inner_radius);
            segments.extend([
                PathSegment::MoveTo { to: inner_top },
                PathSegment::ArcTo {
                    radius_x: inner_radius,
                    radius_y: inner_radius,
                    x_axis_rotation_degrees: 0.0,
                    large_arc: true,
                    sweep_clockwise: false,
                    to: inner_bottom,
                },
                PathSegment::ArcTo {
                    radius_x: inner_radius,
                    radius_y: inner_radius,
                    x_axis_rotation_degrees: 0.0,
                    large_arc: true,
                    sweep_clockwise: false,
                    to: inner_top,
                },
                PathSegment::Close,
            ]);
        } else {
            segments.push(PathSegment::Close);
        }
        return Some(segments);
    }

    let outer_start = pie_polar_point(radius, start_angle);
    let outer_end = pie_polar_point(radius, end_angle);
    let large_arc = end_angle - start_angle > std::f64::consts::PI;
    let mut segments = vec![
        PathSegment::MoveTo { to: outer_start },
        PathSegment::ArcTo {
            radius_x: radius,
            radius_y: radius,
            x_axis_rotation_degrees: 0.0,
            large_arc,
            sweep_clockwise: true,
            to: outer_end,
        },
    ];
    if inner_radius > 0.0 {
        let inner_end = pie_polar_point(inner_radius, end_angle);
        let inner_start = pie_polar_point(inner_radius, start_angle);
        segments.extend([
            PathSegment::LineTo { to: inner_end },
            PathSegment::ArcTo {
                radius_x: inner_radius,
                radius_y: inner_radius,
                x_axis_rotation_degrees: 0.0,
                large_arc,
                sweep_clockwise: false,
                to: inner_start,
            },
            PathSegment::Close,
        ]);
    } else {
        segments.extend([
            PathSegment::LineTo {
                to: Point::new(0.0, 0.0),
            },
            PathSegment::Close,
        ]);
    }
    Some(segments)
}

fn pie_polar_point(radius: f64, angle: f64) -> Point {
    Point::new(radius * angle.sin(), -radius * angle.cos())
}

fn state_find_adjacent_point(
    point_a: &LayoutPoint,
    point_b: &LayoutPoint,
    distance: f64,
) -> LayoutPoint {
    let x_diff = point_b.x - point_a.x;
    let y_diff = point_b.y - point_a.y;
    let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
    let ratio = distance / length;
    LayoutPoint {
        x: point_b.x - ratio * x_diff,
        y: point_b.y - ratio * y_diff,
    }
}

fn state_is_corner_point(prev: &LayoutPoint, curr: &LayoutPoint, next: &LayoutPoint) -> bool {
    (prev.x == curr.x
        && curr.y == next.y
        && (curr.x - next.x).abs() > 5.0
        && (curr.y - prev.y).abs() > 5.0)
        || (prev.y == curr.y
            && curr.x == next.x
            && (curr.x - prev.x).abs() > 5.0
            && (curr.y - next.y).abs() > 5.0)
}

fn state_fix_corners(line_data: &[LayoutPoint]) -> Vec<LayoutPoint> {
    if line_data.len() < 3 {
        return line_data.to_vec();
    }

    let mut out = Vec::with_capacity(line_data.len());
    for (index, point) in line_data.iter().enumerate() {
        let is_corner = index > 0
            && index + 1 < line_data.len()
            && state_is_corner_point(&line_data[index - 1], point, &line_data[index + 1]);
        if !is_corner {
            out.push(point.clone());
            continue;
        }

        let previous = &line_data[index - 1];
        let next = &line_data[index + 1];
        let new_previous = state_find_adjacent_point(previous, point, 5.0);
        let new_next = state_find_adjacent_point(next, point, 5.0);
        let x_diff = new_next.x - new_previous.x;
        let y_diff = new_next.y - new_previous.y;
        out.push(new_previous.clone());

        let mut rounded_corner = point.clone();
        if (next.x - previous.x).abs() > 10.0 && (next.y - previous.y).abs() >= 10.0 {
            let diagonal = std::f64::consts::SQRT_2 * 2.0;
            let radius = 5.0;
            rounded_corner = if point.x == new_previous.x {
                LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_previous.x - radius + diagonal
                    } else {
                        new_previous.x + radius - diagonal
                    },
                    y: if y_diff < 0.0 {
                        new_previous.y - diagonal
                    } else {
                        new_previous.y + diagonal
                    },
                }
            } else {
                LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_previous.x - diagonal
                    } else {
                        new_previous.x + diagonal
                    },
                    y: if y_diff < 0.0 {
                        new_previous.y - radius + diagonal
                    } else {
                        new_previous.y + radius - diagonal
                    },
                }
            };
        }

        out.push(rounded_corner);
        out.push(new_next);
    }
    out
}

fn state_line_with_end_marker_offset_points(
    input: &[LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<LayoutPoint> {
    let Some(end_marker_height) = (arrow_type_end == Some("arrow_barb_neo")).then_some(5.5) else {
        return input.to_vec();
    };
    if input.len() < 2 {
        return input.to_vec();
    }

    let start = &input[0];
    let end = &input[input.len() - 1];
    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;
    let mut out = Vec::with_capacity(input.len());

    for (index, point) in input.iter().enumerate() {
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;
        if index == input.len() - 1 {
            let adjacent = &input[input.len() - 2];
            let delta_x = adjacent.x - end.x;
            let delta_y = adjacent.y - end.y;
            let angle = (delta_y / delta_x).atan();
            offset_x = end_marker_height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
            offset_y =
                end_marker_height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
        }

        let diff_x = (point.x - end.x).abs();
        let diff_y = (point.y - end.y).abs();
        if diff_x < end_marker_height && diff_x > 0.0 && diff_y < end_marker_height {
            let adjustment = (end_marker_height + extra_room - diff_x)
                * if !x_direction_is_left { -1.0 } else { 1.0 };
            offset_x -= adjustment;
        }
        if diff_y < end_marker_height && diff_y > 0.0 && diff_x < end_marker_height {
            let adjustment = (end_marker_height + extra_room - diff_y)
                * if !y_direction_is_down { -1.0 } else { 1.0 };
            offset_y -= adjustment;
        }

        out.push(LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }
    out
}

#[derive(Clone, Copy)]
enum StepCurve {
    Before,
    Midpoint,
    After,
}

fn emit_step_segments<E>(
    points: &[LayoutPoint],
    curve: StepCurve,
    mut emit: impl FnMut(PathSegment) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    let Some(first) = points.first() else {
        return Ok(());
    };
    emit(PathSegment::MoveTo {
        to: Point::new(first.x, first.y),
    })?;
    let mut previous = first;
    for point in points.iter().skip(1) {
        match curve {
            StepCurve::Before => emit(PathSegment::LineTo {
                to: Point::new(previous.x, point.y),
            })?,
            StepCurve::Midpoint => {
                let mid_x = (previous.x + point.x) / 2.0;
                emit(PathSegment::LineTo {
                    to: Point::new(mid_x, previous.y),
                })?;
                emit(PathSegment::LineTo {
                    to: Point::new(mid_x, point.y),
                })?;
            }
            StepCurve::After => emit(PathSegment::LineTo {
                to: Point::new(point.x, previous.y),
            })?,
        }
        emit(PathSegment::LineTo {
            to: Point::new(point.x, point.y),
        })?;
        previous = point;
    }
    Ok(())
}

fn emit_linear_segments<E>(
    points: &[LayoutPoint],
    mut emit: impl FnMut(PathSegment) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    if let Some(first) = points.first() {
        emit(PathSegment::MoveTo {
            to: Point::new(first.x, first.y),
        })?;
        for point in points.iter().skip(1) {
            emit(PathSegment::LineTo {
                to: Point::new(point.x, point.y),
            })?;
        }
    }
    Ok(())
}

pub(crate) fn emit_basis_segments<E>(
    points: &[LayoutPoint],
    mut emit: impl FnMut(PathSegment) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    let Some(first) = points.first() else {
        return Ok(());
    };
    emit(PathSegment::MoveTo {
        to: Point::new(first.x, first.y),
    })?;
    if points.len() < 3 {
        for point in points.iter().skip(1) {
            emit(PathSegment::LineTo {
                to: Point::new(point.x, point.y),
            })?;
        }
        return Ok(());
    }
    let point = |index: usize| Point::new(points[index].x, points[index].y);
    emit(PathSegment::LineTo {
        to: Point::new(
            (5.0 * points[0].x + points[1].x) / 6.0,
            (5.0 * points[0].y + points[1].y) / 6.0,
        ),
    })?;
    for index in 1..points.len() - 1 {
        let previous = point(index - 1);
        let current = point(index);
        let next = point(index + 1);
        emit(PathSegment::CubicTo {
            control1: Point::new(
                (2.0 * previous.x + current.x) / 3.0,
                (2.0 * previous.y + current.y) / 3.0,
            ),
            control2: Point::new(
                (previous.x + 2.0 * current.x) / 3.0,
                (previous.y + 2.0 * current.y) / 3.0,
            ),
            to: Point::new(
                (previous.x + 4.0 * current.x + next.x) / 6.0,
                (previous.y + 4.0 * current.y + next.y) / 6.0,
            ),
        })?;
    }
    let previous = point(points.len() - 2);
    let current = point(points.len() - 1);
    emit(PathSegment::CubicTo {
        control1: Point::new(
            (2.0 * previous.x + current.x) / 3.0,
            (2.0 * previous.y + current.y) / 3.0,
        ),
        control2: Point::new(
            (previous.x + 2.0 * current.x) / 3.0,
            (previous.y + 2.0 * current.y) / 3.0,
        ),
        to: Point::new(
            (previous.x + 5.0 * current.x) / 6.0,
            (previous.y + 5.0 * current.y) / 6.0,
        ),
    })?;
    emit(PathSegment::LineTo { to: current })
}

fn emit_rounded_segments<E>(
    points: &[LayoutPoint],
    radius: f64,
    compact: bool,
    rounded_corner_mask: Option<&[bool]>,
    mut emit: impl FnMut(PathSegment) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    if points.len() < 2 {
        return emit_linear_segments(points, emit);
    }
    emit(PathSegment::MoveTo {
        to: Point::new(points[0].x, points[0].y),
    })?;
    for index in 1..points.len() {
        let current = Point::new(points[index].x, points[index].y);
        if index + 1 == points.len()
            || rounded_corner_mask.is_some_and(|mask| mask.get(index) == Some(&false))
        {
            emit(PathSegment::LineTo { to: current })?;
            continue;
        }
        let previous = Point::new(points[index - 1].x, points[index - 1].y);
        let next = Point::new(points[index + 1].x, points[index + 1].y);
        let dx1 = current.x - previous.x;
        let dy1 = current.y - previous.y;
        let dx2 = next.x - current.x;
        let dy2 = next.y - current.y;
        let len1 = dx1.hypot(dy1);
        let len2 = dx2.hypot(dy2);
        if len1 < 1e-5 || len2 < 1e-5 {
            emit(PathSegment::LineTo { to: current })?;
            continue;
        }
        let nx1 = dx1 / len1;
        let ny1 = dy1 / len1;
        let nx2 = dx2 / len2;
        let ny2 = dy2 / len2;
        let angle = (nx1 * nx2 + ny1 * ny2).clamp(-1.0, 1.0).acos();
        if angle < 1e-5 || (std::f64::consts::PI - angle).abs() < 1e-5 {
            emit(PathSegment::LineTo { to: current })?;
            continue;
        }
        let mermaid_cut = radius / (angle / 2.0).sin();
        let radius_cut = if compact {
            mermaid_cut.min(radius * (angle / 2.0).tan())
        } else {
            mermaid_cut
        };
        let cut = radius_cut.min(len1 / 2.0).min(len2 / 2.0);
        let start = Point::new(current.x - nx1 * cut, current.y - ny1 * cut);
        let end = Point::new(current.x + nx2 * cut, current.y + ny2 * cut);
        emit(PathSegment::LineTo { to: start })?;
        emit(PathSegment::QuadTo {
            control: current,
            to: end,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    #[test]
    fn basis_emission_preserves_geometry_and_propagates_rejection() {
        let points = [point(0.0, 0.0), point(6.0, 12.0), point(12.0, 0.0)];
        assert_eq!(
            flowchart_curve_segments(&points, FlowchartCurveKind::Basis, 0.0, false, None),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0),
                },
                PathSegment::LineTo {
                    to: Point::new(1.0, 2.0),
                },
                PathSegment::CubicTo {
                    control1: Point::new(2.0, 4.0),
                    control2: Point::new(4.0, 8.0),
                    to: Point::new(6.0, 8.0),
                },
                PathSegment::CubicTo {
                    control1: Point::new(8.0, 8.0),
                    control2: Point::new(10.0, 4.0),
                    to: Point::new(11.0, 2.0),
                },
                PathSegment::LineTo {
                    to: Point::new(12.0, 0.0),
                },
            ]
        );
        for length in 0..3 {
            assert_eq!(
                flowchart_curve_segments(
                    &points[..length],
                    FlowchartCurveKind::Basis,
                    0.0,
                    false,
                    None,
                ),
                flowchart_curve_segments(
                    &points[..length],
                    FlowchartCurveKind::Linear,
                    0.0,
                    false,
                    None,
                )
            );
        }
        let mut attempted = 0;
        let result = emit_basis_segments(&points, |_| {
            attempted += 1;
            if attempted == 3 { Err("stop") } else { Ok(()) }
        });
        assert_eq!(result, Err("stop"));
        assert_eq!(attempted, 3);
    }

    #[test]
    fn flowchart_curve_emission_stops_at_the_rejected_segment() {
        let points = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 20.0),
            point(20.0, 20.0),
        ];
        for curve in [
            FlowchartCurveKind::Linear,
            FlowchartCurveKind::Basis,
            FlowchartCurveKind::Step,
            FlowchartCurveKind::StepBefore,
            FlowchartCurveKind::StepAfter,
            FlowchartCurveKind::Rounded,
        ] {
            for rejected_at in 1..=3 {
                let mut attempted = 0;
                let result =
                    emit_flowchart_curve_segments(&points, curve, 2.0, false, None, |_| {
                        attempted += 1;
                        if attempted == rejected_at {
                            Err("stop")
                        } else {
                            Ok(())
                        }
                    });
                assert_eq!(result, Err("stop"), "{curve:?}");
                assert_eq!(attempted, rejected_at, "{curve:?}");
            }
        }
    }

    #[test]
    fn linear_curves_preserve_duplicate_points_and_empty_input() {
        assert!(
            flowchart_curve_segments(&[], FlowchartCurveKind::Linear, 0.0, false, None).is_empty()
        );
        assert_eq!(
            flowchart_curve_segments(
                &[point(2.0, 3.0), point(2.0, 3.0), point(5.0, 7.0)],
                FlowchartCurveKind::Linear,
                0.0,
                false,
                None,
            ),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(2.0, 3.0),
                },
                PathSegment::LineTo {
                    to: Point::new(2.0, 3.0),
                },
                PathSegment::LineTo {
                    to: Point::new(5.0, 7.0),
                },
            ]
        );
    }

    #[test]
    fn rounded_curves_clip_the_cut_and_respect_the_corner_mask() {
        let points = [point(0.0, 0.0), point(10.0, 0.0), point(10.0, 20.0)];
        for compact in [false, true] {
            assert_eq!(
                flowchart_curve_segments(
                    &points,
                    FlowchartCurveKind::Rounded,
                    100.0,
                    compact,
                    None,
                ),
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(0.0, 0.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(5.0, 0.0),
                    },
                    PathSegment::QuadTo {
                        control: Point::new(10.0, 0.0),
                        to: Point::new(10.0, 5.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(10.0, 20.0),
                    },
                ]
            );
        }
        assert_eq!(
            flowchart_curve_segments(
                &points,
                FlowchartCurveKind::Rounded,
                100.0,
                false,
                Some(&[true, false, true]),
            ),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0),
                },
                PathSegment::LineTo {
                    to: Point::new(10.0, 0.0),
                },
                PathSegment::LineTo {
                    to: Point::new(10.0, 20.0),
                },
            ]
        );
    }

    #[test]
    fn step_curves_keep_their_d3_axis_order() {
        let points = [point(0.0, 0.0), point(10.0, 20.0)];
        assert_eq!(
            flowchart_curve_segments(&points, FlowchartCurveKind::StepBefore, 0.0, false, None),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0)
                },
                PathSegment::LineTo {
                    to: Point::new(0.0, 20.0)
                },
                PathSegment::LineTo {
                    to: Point::new(10.0, 20.0)
                },
            ]
        );
        assert_eq!(
            flowchart_curve_segments(&points, FlowchartCurveKind::StepAfter, 0.0, false, None),
            vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0)
                },
                PathSegment::LineTo {
                    to: Point::new(10.0, 0.0)
                },
                PathSegment::LineTo {
                    to: Point::new(10.0, 20.0)
                },
            ]
        );
    }

    #[test]
    fn state_curve_points_shorten_the_neo_barb_terminal_point() {
        let input = [point(0.0, 0.0), point(10.0, 0.0)];
        let output = state_curve_points(&input, Some("arrow_barb_neo"));

        assert_eq!(output.len(), 2);
        assert!((output[0].x - 0.0).abs() <= 1e-9);
        assert!((output[0].y - 0.0).abs() <= 1e-9);
        assert!((output[1].x - 4.5).abs() <= 1e-9);
        assert!((output[1].y - 0.0).abs() <= 1e-9);
    }
}
