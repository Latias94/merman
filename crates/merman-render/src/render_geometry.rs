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
    match curve {
        FlowchartCurveKind::Linear => linear_segments(points),
        FlowchartCurveKind::Basis => basis_segments(points),
        FlowchartCurveKind::Step => step_segments(points, StepCurve::Midpoint),
        FlowchartCurveKind::StepBefore => step_segments(points, StepCurve::Before),
        FlowchartCurveKind::StepAfter => step_segments(points, StepCurve::After),
        FlowchartCurveKind::Rounded => rounded_segments(
            points,
            rounded_radius,
            compact_rounded_corners,
            rounded_corner_mask,
        ),
    }
}

#[derive(Clone, Copy)]
enum StepCurve {
    Before,
    Midpoint,
    After,
}

fn step_segments(points: &[LayoutPoint], curve: StepCurve) -> Vec<PathSegment> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(points.len().saturating_mul(3));
    out.push(PathSegment::MoveTo {
        to: Point::new(first.x, first.y),
    });
    let mut previous = first;
    for point in points.iter().skip(1) {
        match curve {
            StepCurve::Before => out.push(PathSegment::LineTo {
                to: Point::new(previous.x, point.y),
            }),
            StepCurve::Midpoint => {
                let mid_x = (previous.x + point.x) / 2.0;
                out.push(PathSegment::LineTo {
                    to: Point::new(mid_x, previous.y),
                });
                out.push(PathSegment::LineTo {
                    to: Point::new(mid_x, point.y),
                });
            }
            StepCurve::After => out.push(PathSegment::LineTo {
                to: Point::new(point.x, previous.y),
            }),
        }
        out.push(PathSegment::LineTo {
            to: Point::new(point.x, point.y),
        });
        previous = point;
    }
    out
}

fn linear_segments(points: &[LayoutPoint]) -> Vec<PathSegment> {
    let mut out = Vec::with_capacity(points.len());
    if let Some(first) = points.first() {
        out.push(PathSegment::MoveTo {
            to: Point::new(first.x, first.y),
        });
        out.extend(points.iter().skip(1).map(|point| PathSegment::LineTo {
            to: Point::new(point.x, point.y),
        }));
    }
    out
}

fn basis_segments(points: &[LayoutPoint]) -> Vec<PathSegment> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() < 3 {
        return linear_segments(points);
    }
    let point = |index: usize| Point::new(points[index].x, points[index].y);
    let mut out = vec![PathSegment::MoveTo { to: point(0) }];
    out.push(PathSegment::LineTo {
        to: Point::new(
            (5.0 * points[0].x + points[1].x) / 6.0,
            (5.0 * points[0].y + points[1].y) / 6.0,
        ),
    });
    for index in 1..points.len() - 1 {
        let previous = point(index - 1);
        let current = point(index);
        let next = point(index + 1);
        out.push(PathSegment::CubicTo {
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
        });
    }
    let previous = point(points.len() - 2);
    let current = point(points.len() - 1);
    out.push(PathSegment::CubicTo {
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
    });
    out.push(PathSegment::LineTo { to: current });
    out
}

fn rounded_segments(
    points: &[LayoutPoint],
    radius: f64,
    compact: bool,
    rounded_corner_mask: Option<&[bool]>,
) -> Vec<PathSegment> {
    if points.len() < 2 {
        return linear_segments(points);
    }
    let mut out = vec![PathSegment::MoveTo {
        to: Point::new(points[0].x, points[0].y),
    }];
    for index in 1..points.len() {
        let current = Point::new(points[index].x, points[index].y);
        if index + 1 == points.len()
            || rounded_corner_mask.is_some_and(|mask| mask.get(index) == Some(&false))
        {
            out.push(PathSegment::LineTo { to: current });
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
            out.push(PathSegment::LineTo { to: current });
            continue;
        }
        let nx1 = dx1 / len1;
        let ny1 = dy1 / len1;
        let nx2 = dx2 / len2;
        let ny2 = dy2 / len2;
        let angle = (nx1 * nx2 + ny1 * ny2).clamp(-1.0, 1.0).acos();
        if angle < 1e-5 || (std::f64::consts::PI - angle).abs() < 1e-5 {
            out.push(PathSegment::LineTo { to: current });
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
        out.push(PathSegment::LineTo { to: start });
        out.push(PathSegment::QuadTo {
            control: current,
            to: end,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
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
}
