//! Axis-tagged route paint from Mermaid 12.4.0's `routePath.ts`.

use crate::resources::OperationWorkMeter;
use crate::route_types::{Axis, MIN_MARKER_LEAD, RoutePoint};
use crate::svg_output::{BoundedSvgOutput, SvgOutput};
use crate::{Error, Result};
use kurbo::{Point, Size, Vec2};
use merman_model::OperationPhase;
use std::fmt::Write as _;

const EPSILON: f64 = 0.01;
const STEP_CORNER: f64 = 12.0;
const RADIUS: f64 = 5.0;
const CORNER_HANDLE: f64 = 0.62;

#[derive(Clone, Copy)]
struct Knot {
    point: Point,
    axis: Axis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Curved,
    Linear,
    Rounded,
    Step,
    StepBefore,
    StepAfter,
}

impl Curve {
    pub fn from_mermaid(curve: &str) -> Self {
        match curve {
            "linear" => Self::Linear,
            "rounded" => Self::Rounded,
            "step" => Self::Step,
            "stepBefore" => Self::StepBefore,
            "stepAfter" => Self::StepAfter,
            _ => Self::Curved,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Curved => "curved",
            Self::Linear => "linear",
            Self::Rounded => "rounded",
            Self::Step => "step",
            Self::StepBefore => "stepBefore",
            Self::StepAfter => "stepAfter",
        }
    }

    fn step_coordinate(self, from: f64, to: f64) -> f64 {
        match self {
            Self::StepBefore => from,
            Self::StepAfter => to,
            _ => (from + to) / 2.0,
        }
    }

    pub const fn is_stepped(self) -> bool {
        matches!(self, Self::Step | Self::StepBefore | Self::StepAfter)
    }
}

/// Marker setback for `routePath`. Neo's dash mask has its own separate offset table.
pub fn marker_trim(arrow_type: Option<&str>) -> f64 {
    if arrow_type == Some("arrow_barb_neo") {
        5.5
    } else {
        super::edge_path::marker_offset_for(arrow_type).unwrap_or(0.0)
    }
}

/// Move a box contact onto a leaf's contour without changing its side or axis tag. The callback
/// receives a target and an optional line origin. An origin-aware shape can find the first hit
/// even on a non-convex outline. A ray-only shape falls back to the same inward search as Mermaid.
pub fn clip_endpoint(
    knot: RoutePoint,
    center: Point,
    size: Size,
    intersect: impl Fn(Point, Option<Point>) -> Result<Option<Point>>,
    work: &OperationWorkMeter,
) -> Result<RoutePoint> {
    work.checkpoint(OperationPhase::Emit)?;
    let end = Point::new(knot.x, knot.y);
    let offset = end - center;
    let on_horizontal_side = (offset.y.abs() - size.height / 2.0).abs() < 0.5;
    let on_vertical_side = (offset.x.abs() - size.width / 2.0).abs() < 0.5;
    if !on_horizontal_side && !on_vertical_side {
        return Ok(knot);
    }
    let with_position = |at: Point| RoutePoint {
        x: at.x,
        y: at.y,
        ..knot
    };
    let (far, inner) = if on_horizontal_side {
        (
            Point::new(end.x, 2.0 * center.y - end.y),
            Point::new(end.x, center.y),
        )
    } else {
        (
            Point::new(2.0 * center.x - end.x, end.y),
            Point::new(center.x, end.y),
        )
    };
    let query = |target, origin| -> Result<Option<Point>> {
        work.charge_emit_work(1)?;
        Ok(intersect(target, origin)?.filter(|hit| hit.x.is_finite() && hit.y.is_finite()))
    };
    if let Some(hit) = query(end, Some(far))? {
        let on_route = if on_horizontal_side {
            (hit.x - end.x).abs() < 0.5
        } else {
            (hit.y - end.y).abs() < 0.5
        };
        let local = hit - center;
        let in_box =
            local.x.abs() <= size.width / 2.0 + 0.5 && local.y.abs() <= size.height / 2.0 + 0.5;
        if on_route && in_box && hit != center {
            let at = if on_horizontal_side {
                Point::new(end.x, hit.y)
            } else {
                Point::new(hit.x, end.y)
            };
            return Ok(with_position(at));
        }
    }

    let inside = |at: Point| -> Result<bool> {
        let distance = (at - center).hypot();
        if distance < 1e-6 {
            return Ok(true);
        }
        Ok(query(at, None)?.is_some_and(|outline| distance <= (outline - center).hypot() + 1e-6))
    };
    if inside(end)? {
        return Ok(knot);
    }
    if !inside(inner)? {
        return Ok(query(end, None)?.map_or(knot, with_position));
    }
    let (mut outside, mut within) = (0.0, 1.0);
    let inward = inner - end;
    for _ in 0..40 {
        work.checkpoint(OperationPhase::Emit)?;
        let halfway = (outside + within) / 2.0;
        if inside(end + inward * halfway)? {
            within = halfway;
        } else {
            outside = halfway;
        }
    }
    Ok(with_position(end + inward * within))
}

/// Write the path of an already clipped route. The caller retains the untrimmed knots for
/// metadata and crossings. Only the SVG shaft receives marker setbacks and approach segments.
pub fn path(
    points: &[RoutePoint],
    curve: Curve,
    start_trim: f64,
    end_trim: f64,
    work: &OperationWorkMeter,
) -> Result<String> {
    let capacity = points
        .len()
        .checked_mul(if curve.is_stepped() { 3 } else { 1 })
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge_emit_work(capacity)?;
    if points.len() < 2
        || !start_trim.is_finite()
        || !end_trim.is_finite()
        || start_trim < 0.0
        || end_trim < 0.0
    {
        return Err(invalid_geometry());
    }
    let mut route = Vec::<Knot>::new();
    route
        .try_reserve_exact(capacity)
        .map_err(|_| Error::LayoutAllocationFailed { backend: "layered" })?;
    for &knot in points {
        work.checkpoint(OperationPhase::Emit)?;
        if !knot.x.is_finite() || !knot.y.is_finite() {
            return Err(invalid_geometry());
        }
        let at = Point::new(knot.x, knot.y);
        if curve.is_stepped()
            && let Some(previous) = route.last().map(|knot| knot.point)
            && !aligned(previous, at)
        {
            let (cross, corners) = match knot.axis {
                Axis::Vertical => {
                    let y = curve.step_coordinate(previous.y, at.y);
                    (y, [Point::new(previous.x, y), Point::new(at.x, y)])
                }
                Axis::Horizontal => {
                    let x = curve.step_coordinate(previous.x, at.x);
                    (x, [Point::new(x, previous.y), Point::new(x, at.y)])
                }
            };
            if !cross.is_finite() {
                return Err(invalid_geometry());
            }
            route.extend(
                corners
                    .into_iter()
                    .filter(|&corner| !near(corner, previous) && !near(corner, at))
                    .map(|point| Knot {
                        point,
                        axis: knot.axis,
                    }),
            );
        }
        route.push(Knot {
            point: at,
            axis: knot.axis,
        });
    }
    prepare_marker_leads(&mut route, curve, start_trim, end_trim);
    write_path(&route, curve, work)
}

fn prepare_marker_leads(route: &mut Vec<Knot>, curve: Curve, start_trim: f64, end_trim: f64) {
    let last = route.len() - 1;
    let start_dir = direction(curve, route[0], route[1]);
    let end_dir = direction(curve, route[last - 1], route[last]);
    route[0].point += start_dir * start_trim;
    route[last].point -= end_dir * end_trim;
    let lead = |a: Point, b: Point, dir: Vec2| {
        let delta = b - a;
        MIN_MARKER_LEAD.min((delta.x.abs() * dir.x.abs() + delta.y.abs() * dir.y.abs()) / 4.0)
    };
    let (first, second) = (route[0].point, route[1].point);
    let (penultimate, tip) = (route[last - 1].point, route[last].point);
    let enters =
        end_trim > 0.0 && (!straight(curve, penultimate, tip) || turns(route, last - 1, curve));
    let exits = start_trim > 0.0 && (!straight(curve, first, second) || turns(route, 1, curve));
    let end_lead = if enters {
        lead(penultimate, tip, end_dir)
    } else {
        0.0
    };
    let start_lead = if exits {
        lead(first, second, start_dir)
    } else {
        0.0
    };
    if end_lead >= 1.0 {
        route.insert(
            last,
            Knot {
                point: tip - end_dir * end_lead,
                axis: route[last].axis,
            },
        );
    }
    if start_lead >= 1.0 {
        route.insert(
            1,
            Knot {
                point: first + start_dir * start_lead,
                axis: route[1].axis,
            },
        );
    }
}

fn write_path(route: &[Knot], curve: Curve, work: &OperationWorkMeter) -> Result<String> {
    let mut out = BoundedSvgOutput::new(work);
    let mut at = route[0].point;
    out.push('M');
    point(&mut out, at)?;
    for (i, &Knot { point: to, axis }) in route.iter().enumerate().skip(1) {
        work.checkpoint(OperationPhase::Emit)?;
        let from = route[i - 1].point;
        if !straight(curve, from, to) {
            let (across, along) = match axis {
                Axis::Vertical => (to.x - at.x, to.y - at.y),
                Axis::Horizontal => (to.y - at.y, to.x - at.x),
            };
            let reach = along * (4.0 * along.abs() / nonzero(across.abs())).clamp(0.25, 0.5);
            let handle = match axis {
                Axis::Vertical => Vec2::new(0.0, reach),
                Axis::Horizontal => Vec2::new(reach, 0.0),
            };
            cubic(&mut out, at + handle, to - handle, to)?;
            at = to;
        } else if turns(route, i, curve) {
            let next = route[i + 1].point;
            let mut before = length(from, to) / if turns(route, i - 1, curve) { 2.0 } else { 1.0 };
            let mut after = length(to, next) / if turns(route, i + 1, curve) { 2.0 } else { 1.0 };
            if curve.is_stepped() || curve == Curve::Rounded {
                let limit: f64 = if curve == Curve::Rounded {
                    RADIUS
                } else {
                    STEP_CORNER
                };
                before = limit.min(before).min(after);
                after = before;
            }
            let incoming = unit(to - from);
            let outgoing = unit(next - to);
            line_to(&mut out, &mut at, to - incoming * before)?;
            at = to + outgoing * after;
            if curve == Curve::Rounded {
                out.push('A');
                point(&mut out, Point::new(before, before))?;
                out.push_str(if incoming.cross(outgoing) > 0.0 {
                    " 0 0,1 "
                } else {
                    " 0 0,0 "
                });
                point(&mut out, at)?;
            } else if curve.is_stepped() {
                out.push('Q');
                point(&mut out, to)?;
                out.push(' ');
                point(&mut out, at)?;
            } else {
                cubic(
                    &mut out,
                    to - incoming * before * (1.0 - CORNER_HANDLE),
                    to + outgoing * after * (1.0 - CORNER_HANDLE),
                    at,
                )?;
            }
        } else if !route.get(i + 1).is_some_and(|next| {
            aligned(from, to) && aligned(to, next.point) && (to - from).dot(next.point - to) > 0.0
        }) {
            line_to(&mut out, &mut at, to)?;
        }
        out.checkpoint()?;
    }
    out.finish()
}

fn invalid_geometry() -> Error {
    Error::InvalidModel {
        message: "Layered route has invalid SVG geometry".to_owned(),
    }
}

fn length(from: Point, to: Point) -> f64 {
    (to.x - from.x).abs() + (to.y - from.y).abs()
}

fn turns(route: &[Knot], i: usize, curve: Curve) -> bool {
    if curve == Curve::Linear || i == 0 || i + 1 >= route.len() {
        return false;
    }
    let (from, at, to) = (route[i - 1].point, route[i].point, route[i + 1].point);
    aligned(from, at)
        && aligned(at, to)
        && length(from, at) >= EPSILON
        && length(at, to) >= EPSILON
        && ((at.x - from.x).abs() < EPSILON) != ((to.x - at.x).abs() < EPSILON)
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < EPSILON && (a.y - b.y).abs() < EPSILON
}

fn aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < EPSILON || (a.y - b.y).abs() < EPSILON
}

fn straight(curve: Curve, a: Point, b: Point) -> bool {
    matches!(curve, Curve::Linear | Curve::Rounded) || aligned(a, b)
}

fn nonzero(value: f64) -> f64 {
    if value == 0.0 { 1.0 } else { value }
}

fn unit(delta: Vec2) -> Vec2 {
    let length = nonzero(delta.hypot());
    Vec2::new(delta.x / length, delta.y / length)
}

fn direction(curve: Curve, from: Knot, to: Knot) -> Vec2 {
    let delta = to.point - from.point;
    if straight(curve, from.point, to.point) {
        return unit(delta);
    }
    let sign = |value: f64| if value < 0.0 { -1.0 } else { 1.0 };
    match to.axis {
        Axis::Vertical => Vec2::new(0.0, sign(delta.y)),
        Axis::Horizontal => Vec2::new(sign(delta.x), 0.0),
    }
}

fn point(out: &mut BoundedSvgOutput<'_>, point: Point) -> Result<()> {
    use super::edge_label_geometry::js_round;
    let x = js_round(point.x, 2).ok_or_else(invalid_geometry)?;
    let y = js_round(point.y, 2).ok_or_else(invalid_geometry)?;
    let x = if x == 0.0 { 0.0 } else { x };
    let y = if y == 0.0 { 0.0 } else { y };
    let mut x_buf = ryu_js::Buffer::new();
    let mut y_buf = ryu_js::Buffer::new();
    let _ = write!(out, "{},{}", x_buf.format_finite(x), y_buf.format_finite(y));
    Ok(())
}

fn line_to(out: &mut BoundedSvgOutput<'_>, at: &mut Point, to: Point) -> Result<()> {
    if !near(*at, to) {
        out.push('L');
        point(out, to)?;
        *at = to;
    }
    Ok(())
}

fn cubic(out: &mut BoundedSvgOutput<'_>, first: Point, second: Point, to: Point) -> Result<()> {
    out.push('C');
    point(out, first)?;
    out.push(' ');
    point(out, second)?;
    out.push(' ');
    point(out, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    fn work() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    #[test]
    fn route_path_matches_mermaid_axis_curves_and_marker_leads() {
        use Axis::{Horizontal, Vertical};
        let diagonal = [
            RoutePoint::new(0.0, 0.0, Vertical),
            RoutePoint::new(40.0, 100.0, Vertical),
        ];
        let bend = [
            RoutePoint::new(0.0, 0.0, Vertical),
            RoutePoint::new(0.0, 50.0, Vertical),
            RoutePoint::new(50.0, 50.0, Horizontal),
        ];
        for (curve, start, end, expected) in [
            (Curve::Linear, 0.0, 0.0, "M0,0L40,100"),
            (Curve::Curved, 0.0, 0.0, "M0,0C0,50 40,50 40,100"),
            (Curve::Curved, 0.0, 4.0, "M0,0C0,45 40,45 40,90L40,96"),
            (Curve::Curved, 4.0, 0.0, "M0,4L0,10C0,55 40,55 40,100"),
            (Curve::Curved, 4.0, 4.0, "M0,4L0,10C0,50 40,50 40,90L40,96"),
            (
                Curve::Step,
                0.0,
                0.0,
                "M0,0L0,38Q0,50 12,50L28,50Q40,50 40,62L40,100",
            ),
            (Curve::StepBefore, 0.0, 0.0, "M0,0L28,0Q40,0 40,12L40,100"),
            (Curve::StepAfter, 0.0, 0.0, "M0,0L0,88Q0,100 12,100L40,100"),
        ] {
            assert_eq!(
                path(&diagonal, curve, start, end, &work()).unwrap(),
                expected
            );
        }
        assert_eq!(
            path(&bend, Curve::Rounded, 0.0, 0.0, &work()).unwrap(),
            "M0,0L0,45A5,5 0 0,0 5,50L50,50"
        );
        assert_eq!(
            path(&bend, Curve::Curved, 0.0, 0.0, &work()).unwrap(),
            "M0,0C0,31 19,50 50,50"
        );
    }

    #[test]
    fn route_path_retains_shared_emit_admission() {
        let points = [
            RoutePoint::new(-0.125, -0.005, Axis::Vertical),
            RoutePoint::new(-0.125, 23.125, Axis::Vertical),
        ];
        let render = |work: &OperationWorkMeter| path(&points, Curve::Linear, 0.0, 0.0, work);
        let baseline = work();
        let d = render(&baseline).unwrap();
        assert_eq!(d, "M-0.12,0L-0.12,23.13");
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let limit = |id, cap| OperationWorkMeter::new(policy.with_limit(id, cap).unwrap());
        assert_eq!(
            render(&limit(ResourceLimitId::MaxSvgBytes, d.len())).unwrap(),
            d
        );
        assert!(matches!(
            render(&limit(ResourceLimitId::MaxSvgBytes, d.len() - 1)),
            Err(Error::ResourceLimitExceeded(_))
        ));
        let short = limit(ResourceLimitId::MaxLayoutWorkUnits, baseline.used());
        short.charge(1).unwrap();
        assert!(matches!(
            render(&short),
            Err(Error::ResourceLimitExceeded(_))
        ));
        let cancelled = work();
        cancelled.control().cancel();
        assert!(
            matches!(render(&cancelled), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit)
        );
    }

    #[test]
    fn off_center_port_uses_the_line_origin_or_the_same_inward_search() {
        let center = Point::ORIGIN;
        let size = Size::new(20.0, 20.0);
        let top = RoutePoint::new(8.0, -10.0, Axis::Vertical);
        let expected = Point::new(8.0, -6.0);
        let exact = clip_endpoint(
            top,
            center,
            size,
            |point, origin| {
                assert_eq!(point, Point::new(8.0, -10.0));
                assert_eq!(origin, Some(Point::new(8.0, 10.0)));
                Ok(Some(expected))
            },
            &work(),
        )
        .unwrap();
        assert_eq!(
            exact,
            RoutePoint::new(expected.x, expected.y, Axis::Vertical)
        );

        let approximate = clip_endpoint(
            top,
            center,
            size,
            |point, _| Ok(Some(center + unit(point - center) * 10.0)),
            &work(),
        )
        .unwrap();
        assert_eq!(approximate.x, expected.x);
        assert!((approximate.y - expected.y).abs() < 2e-6);
        assert_eq!(approximate.axis, top.axis);
    }
}
