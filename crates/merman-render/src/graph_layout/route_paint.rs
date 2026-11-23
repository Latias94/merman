//! Paint a boundary-to-boundary axis route in the coordinate space supplied by its caller.

mod clearance;
pub(crate) use clearance::{Clearance, EdgeClearance, ObstacleOwner};

use super::{
    Marker,
    ir::{Axis, BorderStitch, RoutePoint, Side},
};
use crate::model::Bounds;
use kurbo::{CubicBez, Point, Shape, Vec2};
use std::fmt::Write as _;

/// Straight shaft beyond the marker setback, in layout units (v13 `shared/svg/route.ts`).
pub(crate) const LEAD: f64 = 6.0;
const STEP_INSET: f64 = 12.0;
const TURN_HANDLE: f64 = 0.62;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RouteStyle {
    Curved,
    Rounded(f64),
    Step,
    StepBefore,
    StepAfter,
    Linear,
}

impl RouteStyle {
    pub(crate) fn from_interpolate(name: &str, rounded_radius: f64) -> Self {
        match name {
            "rounded" => Self::Rounded(rounded_radius),
            "step" => Self::Step,
            "stepBefore" => Self::StepBefore,
            "stepAfter" => Self::StepAfter,
            "linear" => Self::Linear,
            _ => Self::Curved,
        }
    }

    fn stepped(self) -> bool {
        matches!(self, Self::Step | Self::StepBefore | Self::StepAfter)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PaintedRoute {
    pub(crate) d: String,
    /// Tight geometric bounds of `d`, excluding stroke width, markers and effects.
    pub(crate) bounds: Bounds,
}

fn outward(side: Side) -> Vec2 {
    match side {
        Side::Top => Vec2::new(0.0, -1.0),
        Side::Right => Vec2::new(1.0, 0.0),
        Side::Bottom => Vec2::new(0.0, 1.0),
        Side::Left => Vec2::new(-1.0, 0.0),
    }
}

pub(crate) fn group_lead_side(side: Side, port: RoutePoint, next: RoutePoint) -> Side {
    match side {
        Side::Top if next.y > port.y => Side::Bottom,
        Side::Right if next.x < port.x => Side::Left,
        Side::Bottom if next.y < port.y => Side::Top,
        Side::Left if next.x > port.x => Side::Right,
        _ => side,
    }
}

fn position(p: RoutePoint) -> Point {
    Point::new(p.x, p.y)
}

fn at(p: Point, axis: Axis) -> RoutePoint {
    RoutePoint {
        x: p.x,
        y: p.y,
        axis,
    }
}

fn aligned(a: Point, b: Point) -> bool {
    a.x == b.x || a.y == b.y
}

fn has_offset_leg(route: &[RoutePoint]) -> bool {
    route
        .windows(2)
        .any(|leg| !aligned(position(leg[0]), position(leg[1])))
}

/// Drop only redundant orthogonal knots. In particular a U-turn or a shelf between two
/// diagonal legs carries geometry even if two of its coordinates match nearby points.
fn push_knot(knots: &mut Vec<RoutePoint>, p: RoutePoint) {
    let Some(&last) = knots.last() else {
        knots.push(p);
        return;
    };
    let (b, c) = (position(last), position(p));
    if b == c {
        return;
    }
    if let [.., a, _] = knots.as_slice() {
        let a = position(*a);
        if ((a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y)) && (b - a).dot(c - b) > 0.0 {
            knots.pop();
        }
    }
    knots.push(p);
}

fn step_level(
    style: RouteStyle,
    from: f64,
    to: f64,
    markers: (Option<Marker>, Option<Marker>),
) -> f64 {
    let delta = to - from;
    let sign = delta.signum();
    let trims = (
        markers.0.map_or(0.0, |m| m.trim),
        markers.1.map_or(0.0, |m| m.trim),
    );
    let available = (delta.abs() - trims.0 - trims.1).max(0.0);
    let inset = STEP_INSET.min(available / 4.0);
    let offset = match style {
        RouteStyle::StepBefore => trims.0 + inset,
        RouteStyle::StepAfter => delta.abs() - trims.1 - inset,
        _ => delta.abs() * 0.5,
    };
    let reserved = |m: Option<Marker>| m.map_or(0.0, |m| m.trim + LEAD.max(m.depth));
    let lo = reserved(markers.0);
    let hi = delta.abs() - reserved(markers.1);
    from + sign
        * if lo <= hi {
            offset.clamp(lo, hi)
        } else {
            delta.abs() * 0.5
        }
}

/// v13 moves an existing four-point step only when its crossbar lies between the ends.
/// A same-side loop's crossbar has a different job and must stay in its exterior lane.
fn move_step(style: RouteStyle, knots: &mut [RoutePoint], markers: (Marker, Marker)) {
    let [a, b, c, d] = knots else { return };
    if style == RouteStyle::Step {
        return;
    }
    let (from, level, to, vertical) = if a.x == b.x && b.y == c.y && c.x == d.x {
        (a.y, b.y, d.y, true)
    } else if a.y == b.y && b.x == c.x && c.y == d.y {
        (a.x, b.x, d.x, false)
    } else {
        return;
    };
    let sign = (to - from).signum();
    if sign * (level - from) <= markers.0.trim || sign * (to - level) <= markers.1.trim {
        return;
    }
    let level = step_level(style, from, to, (Some(markers.0), Some(markers.1)));
    if vertical {
        b.y = level;
        c.y = level;
    } else {
        b.x = level;
        c.x = level;
    }
}

fn steps(
    style: RouteStyle,
    route: &[RoutePoint],
    markers: (Option<Marker>, Option<Marker>),
) -> Vec<RoutePoint> {
    let mut out = Vec::with_capacity(route.len());
    out.push(route[0]);
    for (i, pair) in route.windows(2).enumerate() {
        let [a, b] = [pair[0], pair[1]];
        if !aligned(position(a), position(b)) {
            let ends = (
                if i == 0 { markers.0 } else { None },
                if i + 2 == route.len() {
                    markers.1
                } else {
                    None
                },
            );
            let (p, q) = match b.axis {
                Axis::Vertical => {
                    let y = step_level(style, a.y, b.y, ends);
                    (RoutePoint { y, ..a }, RoutePoint { y, ..b })
                }
                Axis::Horizontal => {
                    let x = step_level(style, a.x, b.x, ends);
                    (RoutePoint { x, ..a }, RoutePoint { x, ..b })
                }
            };
            push_knot(&mut out, p);
            push_knot(&mut out, q);
        }
        push_knot(&mut out, b);
    }
    out
}

struct Pen {
    d: String,
    bounds: Bounds,
    at: Point,
    finite: bool,
}

impl Pen {
    fn new(p: Point) -> Self {
        Self {
            d: format!("M{},{}", p.x, p.y),
            bounds: Bounds {
                min_x: p.x,
                min_y: p.y,
                max_x: p.x,
                max_y: p.y,
            },
            at: p,
            finite: p.is_finite(),
        }
    }

    fn include(&mut self, p: Point) {
        self.finite &= p.is_finite();
        self.bounds.min_x = self.bounds.min_x.min(p.x);
        self.bounds.min_y = self.bounds.min_y.min(p.y);
        self.bounds.max_x = self.bounds.max_x.max(p.x);
        self.bounds.max_y = self.bounds.max_y.max(p.y);
    }

    fn line(&mut self, p: Point) {
        if p != self.at {
            let _ = write!(self.d, "L{},{}", p.x, p.y);
            self.include(p);
            self.at = p;
        }
    }

    fn cubic(&mut self, a: Point, b: Point, end: Point) {
        if !a.is_finite() || !b.is_finite() || !end.is_finite() {
            self.finite = false;
            return;
        }
        let _ = write!(
            self.d,
            "C{},{} {},{} {},{}",
            a.x, a.y, b.x, b.y, end.x, end.y
        );
        let bounds = CubicBez::new(self.at, a, b, end).bounding_box();
        self.include(Point::new(bounds.x0, bounds.y0));
        self.include(Point::new(bounds.x1, bounds.y1));
        self.at = end;
    }

    fn sweep(&mut self, end: RoutePoint, clearance: Option<EdgeClearance<'_>>) {
        let delta = position(end) - self.at;
        let (run, across) = match end.axis {
            Axis::Vertical => (delta.y, delta.x),
            Axis::Horizontal => (delta.x, delta.y),
        };
        let ease = (4.0 * run.abs() / across.abs().max(1.0)).clamp(0.25, 0.5);
        let handle = match end.axis {
            Axis::Vertical => Vec2::new(0.0, run * ease),
            Axis::Horizontal => Vec2::new(run * ease, 0.0),
        };
        let target = position(end);
        let curve = |scale| {
            CubicBez::new(
                self.at,
                self.at + handle * scale,
                target - handle * scale,
                target,
            )
        };
        let scale = clearance.map_or(1., |guard| {
            smoothing_scale(|scale| guard.blocks_cubic(curve(scale)))
        });
        if scale == 0. {
            self.line(target);
        } else {
            let safe = curve(scale);
            self.cubic(safe.p1, safe.p2, safe.p3);
        }
    }

    fn quarter_arc(&mut self, radius: f64, clockwise: bool, end: Point) {
        let _ = write!(
            self.d,
            "A{radius},{radius} 0 0,{} {},{}",
            u8::from(clockwise),
            end.x,
            end.y
        );
        // Each coordinate is monotone on an axis-aligned quarter circle.
        self.include(end);
        self.at = end;
    }

    fn finish(self) -> Option<PaintedRoute> {
        self.finite.then_some(PaintedRoute {
            d: self.d,
            bounds: self.bounds,
        })
    }
}

fn right_angle(route: &[RoutePoint], i: usize) -> bool {
    if i == 0 || i + 1 >= route.len() {
        return false;
    }
    let a = position(route[i - 1]);
    let b = position(route[i]);
    let c = position(route[i + 1]);
    (a.x == b.x && b.y == c.y) || (a.y == b.y && b.x == c.x)
}

fn smoothing_scale(blocked: impl Fn(f64) -> bool) -> f64 {
    if !blocked(1.0) {
        return 1.0;
    }
    let mut safe: f64 = 0.0;
    let mut limit: f64 = 1.0;
    for _ in 0..12 {
        let scale = safe.midpoint(limit);
        if blocked(scale) {
            limit = scale;
        } else {
            safe = scale;
        }
    }
    safe
}

fn draw_body(
    pen: &mut Pen,
    body: &[RoutePoint],
    style: RouteStyle,
    clearance: Option<EdgeClearance<'_>>,
) {
    for i in 1..body.len() {
        let p = position(body[i]);
        if matches!(style, RouteStyle::Curved | RouteStyle::Rounded(_)) && right_angle(body, i) {
            let incoming = p - position(body[i - 1]);
            let outgoing = position(body[i + 1]) - p;
            let mut before = incoming.hypot();
            let mut after = outgoing.hypot();
            let u = incoming / before;
            let v = outgoing / after;
            if right_angle(body, i - 1) {
                before *= 0.5;
            }
            if right_angle(body, i + 1) {
                after *= 0.5;
            }
            if let RouteStyle::Rounded(radius) = style {
                let r = radius.min(before).min(after);
                let r = r * clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| {
                        guard.blocks_quarter(p - u * (r * scale), p, p + v * (r * scale))
                    })
                });
                if r > 0.0 {
                    pen.line(p - u * r);
                    pen.quarter_arc(r, u.cross(v) > 0.0, p + v * r);
                } else {
                    pen.line(p);
                }
            } else {
                let curve = |scale| {
                    let incoming = before * scale;
                    let outgoing = after * scale;
                    let start = p - u * incoming;
                    let end = p + v * outgoing;
                    CubicBez::new(
                        start,
                        start + u * (TURN_HANDLE * incoming),
                        end - v * (TURN_HANDLE * outgoing),
                        end,
                    )
                };
                let scale = clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| guard.blocks_cubic(curve(scale)))
                });
                if scale == 0. {
                    pen.line(p);
                    continue;
                }
                let safe = curve(scale);
                pen.line(safe.p0);
                pen.cubic(safe.p1, safe.p2, safe.p3);
            }
        } else if style == RouteStyle::Curved && !aligned(position(body[i - 1]), p) {
            pen.sweep(body[i], clearance);
        } else {
            pen.line(p);
        }
    }
}

struct Terminal {
    tip: Point,
    normal: Vec2,
    marker: Marker,
}

impl Terminal {
    fn shaft_end(&self) -> Point {
        self.tip + self.normal * self.marker.trim
    }

    fn lead_end(&self) -> Point {
        self.shaft_end() + self.normal * LEAD.max(self.marker.depth)
    }

    fn covers(&self, knot: RoutePoint) -> bool {
        let delta = position(knot) - self.tip;
        delta.cross(self.normal) == 0.0
            && (0.0..=self.marker.trim + LEAD.max(self.marker.depth))
                .contains(&delta.dot(self.normal))
    }
}

#[cfg(all(
    feature = "layout-shoal",
    any(
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-state"
    )
))]
pub(super) fn marker_lead(
    port: RoutePoint,
    side: Side,
    marker: Marker,
) -> (crate::model::LayoutPoint, crate::model::LayoutPoint) {
    let terminal = Terminal {
        tip: position(port),
        normal: outward(side),
        marker,
    };
    let point = |p: Point| crate::model::LayoutPoint { x: p.x, y: p.y };
    (point(terminal.shaft_end()), point(terminal.lead_end()))
}

/// The endpoints are already on the contours. The two sides are in semantic source/target
/// order, including on reverse-ranked edges. Trim distances refer to the SVG marker reference
/// points, measured outward from the supplied contour contacts.
pub(crate) fn build_svg_path(
    points: &[RoutePoint],
    sides: (Side, Side),
    style: RouteStyle,
    markers: (Marker, Marker),
    stitches: &[BorderStitch],
    clearance: Option<EdgeClearance<'_>>,
) -> Option<PaintedRoute> {
    if points.len() < 2
        || [markers.0, markers.1]
            .iter()
            .any(|m| !m.trim.is_finite() || m.trim < 0.0 || !m.depth.is_finite() || m.depth < 0.0)
        || matches!(style, RouteStyle::Rounded(radius) if !radius.is_finite())
    {
        return None;
    }
    let last = points.len() - 1;
    let start = Terminal {
        tip: position(points[0]),
        normal: outward(sides.0),
        marker: markers.0,
    };
    let end = Terminal {
        tip: position(points[last]),
        normal: outward(sides.1),
        marker: markers.1,
    };
    let mut pen = Pen::new(start.shaft_end());
    let mut knots = Vec::with_capacity(points.len());
    let mut body = Vec::with_capacity(points.len());
    let mut begin = 0;
    for finish in stitches
        .iter()
        .map(|stitch| stitch.point)
        .chain(std::iter::once(last))
    {
        if finish == begin {
            continue;
        }
        draw_section(
            &mut pen,
            points.get(begin..=finish)?,
            (
                (begin == 0).then_some(&start),
                (finish == last).then_some(&end),
            ),
            style,
            &mut knots,
            &mut body,
            clearance,
        )?;
        begin = finish;
    }
    pen.finish()
}

fn draw_section(
    pen: &mut Pen,
    points: &[RoutePoint],
    terminals: (Option<&Terminal>, Option<&Terminal>),
    style: RouteStyle,
    route: &mut Vec<RoutePoint>,
    body: &mut Vec<RoutePoint>,
    clearance: Option<EdgeClearance<'_>>,
) -> Option<()> {
    route.clear();
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        push_knot(route, point);
    }
    if route.len() < 2 {
        return (terminals.0.is_none() && terminals.1.is_none()).then_some(());
    }
    if style.stepped() {
        let markers = (
            terminals.0.map(|end| end.marker),
            terminals.1.map(|end| end.marker),
        );
        move_step(
            style,
            route,
            (markers.0.unwrap_or_default(), markers.1.unwrap_or_default()),
        );
        if has_offset_leg(route) {
            *route = steps(style, route, markers);
        }
    }

    let last = route.len() - 1;
    if last == 1
        && let (Some(start), Some(end)) = terminals
        && start.normal == -end.normal
    {
        let gap = (end.shaft_end() - start.shaft_end()).dot(start.normal);
        // The two marker footprints consume the same runway. A reversed shaft would put both
        // SVG markers on the wrong side of their reference points.
        if gap < 0.0 {
            return None;
        }
        if aligned(start.tip, end.tip) && (end.tip - start.tip).dot(start.normal) > 0.0 {
            // Opposed markers can share this one straight segment up to its full length.
            if gap < start.marker.depth.max(end.marker.depth).max(LEAD) {
                return None;
            }
            pen.line(end.shaft_end());
            return Some(());
        }
    }

    body.clear();
    push_knot(
        body,
        terminals
            .0
            .map_or(route[0], |start| at(start.lead_end(), route[0].axis)),
    );
    for (i, &p) in route[1..last].iter().enumerate() {
        if (i == 0 && terminals.0.is_some_and(|start| start.covers(p)))
            || (i + 2 == last && terminals.1.is_some_and(|end| end.covers(p)))
        {
            continue;
        }
        push_knot(body, p);
    }
    push_knot(
        body,
        terminals
            .1
            .map_or(route[last], |end| at(end.lead_end(), route[last].axis)),
    );
    pen.line(position(body[0]));
    // With different endpoint sides, inserting a normal shaft can create an offset leg even
    // on a previously orthogonal route. Step renderers must still emit only axis-aligned legs.
    if style.stepped() && has_offset_leg(body) {
        *body = steps(style, body, (None, None));
    }
    draw_body(pen, body, style, clearance);
    if let Some(end) = terminals.1 {
        pen.line(end.shaft_end());
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathEl, PathSeg};

    fn v(x: f64, y: f64) -> RoutePoint {
        RoutePoint {
            x,
            y,
            axis: Axis::Vertical,
        }
    }
    fn h(x: f64, y: f64) -> RoutePoint {
        RoutePoint {
            x,
            y,
            axis: Axis::Horizontal,
        }
    }
    fn marker(trim: f64, depth: f64) -> Marker {
        Marker {
            trim,
            depth,
            width: 10.0,
        }
    }

    fn segments(p: &PaintedRoute) -> Vec<PathSeg> {
        let path = BezPath::from_svg(&p.d).unwrap();
        let b = path.bounding_box();
        for (actual, expected) in [
            (p.bounds.min_x, b.x0),
            (p.bounds.min_y, b.y0),
            (p.bounds.max_x, b.x1),
            (p.bounds.max_y, b.y1),
        ] {
            assert!(
                (actual - expected).abs() < 1e-8,
                "{actual} vs {expected}, {}",
                p.d
            );
        }
        path.segments().collect()
    }

    fn samples_inside(path: &PaintedRoute, obstacle: kurbo::Rect) -> bool {
        BezPath::from_svg(&path.d)
            .unwrap()
            .segments()
            .any(|segment| {
                (0..=4096).any(|i| {
                    let p = segment.eval(i as f64 / 4096.);
                    p.x > obstacle.x0 + 1e-6
                        && p.x < obstacle.x1 - 1e-6
                        && p.y > obstacle.y0 + 1e-6
                        && p.y < obstacle.y1 - 1e-6
                })
            })
    }

    #[test]
    fn wide_corners_clear_foreign_labels_without_changing_the_other_turn_or_stitches() {
        use super::super::ir::Endpoint;
        let points = [
            h(0., 5.),
            h(10., 5.),
            v(10., 80.),
            h(190., 80.),
            v(190., 10.),
            h(200., 10.),
        ];
        let stitches = [
            BorderStitch {
                edge: 0,
                group: 0,
                point: 1,
                side: Side::Right,
            },
            BorderStitch {
                edge: 0,
                group: 1,
                point: 4,
                side: Side::Left,
            },
        ];
        let obstacle = kurbo::Rect::new(158., 20., 188., 71.);
        let delta = Vec2::new(732., -127.);
        let markers = (Marker::default(), Marker::default());
        let endpoints = (Endpoint::Node(3), Endpoint::Node(7));
        let tree = Clearance::new(vec![(obstacle + delta, ObstacleOwner::EdgeLabel(1))]);
        for style in [RouteStyle::Curved, RouteStyle::Rounded(40.)] {
            let paint = |guard| {
                build_svg_path(
                    &points,
                    (Side::Right, Side::Left),
                    style,
                    markers,
                    &stitches,
                    guard,
                )
                .unwrap()
            };
            let original = paint(None);
            assert!(samples_inside(&original, obstacle));
            let result = paint(Some(tree.for_edge(0, endpoints, delta, 1.5)));
            assert!(
                !samples_inside(&result, obstacle.inflate(1.5, 1.5)),
                "{}",
                result.d
            );
            let curves = |path: &PaintedRoute| -> Vec<_> {
                segments(path)
                    .into_iter()
                    .filter(|s| !matches!(s, PathSeg::Line(_)))
                    .collect()
            };
            assert_eq!(curves(&original)[0], curves(&result)[0]);
            assert!(curves(&result).len() >= 2);
            for stitch in &stitches {
                assert!(
                    segments(&result)
                        .iter()
                        .any(|leg| leg.end() == position(points[stitch.point]))
                );
            }
            assert_eq!(
                original.d,
                paint(Some(tree.for_edge(1, endpoints, delta, 1.5))).d
            );
        }
    }

    #[test]
    fn only_the_unsafe_sweep_changes_and_foreign_node_and_group_title_ownership_are_distinct() {
        use super::super::ir::Endpoint;
        let points = [h(0., 0.), h(10., 0.), h(110., 50.), h(120., 50.)];
        let obstacle = kurbo::Rect::new(25., 0.75, 35., 5.);
        fn paint(points: &[RoutePoint], guard: Option<EdgeClearance<'_>>) -> PaintedRoute {
            build_svg_path(
                points,
                (Side::Right, Side::Left),
                RouteStyle::Curved,
                (Marker::default(), Marker::default()),
                &[],
                guard,
            )
            .unwrap()
        }
        let original = paint(&points, None);
        assert!(samples_inside(&original, obstacle));
        for (owner, endpoints, protected) in [
            (
                ObstacleOwner::Node(4),
                (Endpoint::Node(4), Endpoint::Node(5)),
                false,
            ),
            (
                ObstacleOwner::Node(4),
                (Endpoint::Node(3), Endpoint::Node(5)),
                true,
            ),
            (
                ObstacleOwner::GroupTitle(4),
                (Endpoint::Group(4), Endpoint::Node(5)),
                true,
            ),
        ] {
            let tree = Clearance::new(vec![(obstacle, owner)]);
            let result = paint(&points, Some(tree.for_edge(1, endpoints, Vec2::ZERO, 0.)));
            assert_eq!(samples_inside(&result, obstacle), !protected);
            assert_eq!(result.d == original.d, !protected);
            assert!(result.d.contains('C'));
            assert_eq!(segments(&result).first(), segments(&original).first());
            assert_eq!(segments(&result).last(), segments(&original).last());
        }
    }

    #[test]
    fn both_markers_keep_their_full_depth_on_all_sides_and_styles() {
        let markers = (marker(3.0, 18.0), marker(7.0, 23.0));
        for (s, t) in [
            (Side::Bottom, Side::Top),
            (Side::Top, Side::Bottom),
            (Side::Right, Side::Left),
            (Side::Left, Side::Right),
        ] {
            let n = outward(s);
            let axis = if n.x == 0.0 {
                Axis::Vertical
            } else {
                Axis::Horizontal
            };
            let a = Point::new(-24.25, 7.75);
            let b = a + n * 120.0 + Vec2::new(-n.y, n.x) * 80.0;
            for style in [
                RouteStyle::Curved,
                RouteStyle::Rounded(15.0),
                RouteStyle::Linear,
                RouteStyle::StepBefore,
                RouteStyle::StepAfter,
                RouteStyle::Step,
            ] {
                let path = build_svg_path(
                    &[at(a, axis), at(b, axis)],
                    (s, t),
                    style,
                    markers,
                    &[],
                    None,
                )
                .unwrap();
                let segs = segments(&path);
                let (start, end) = (segs[0], *segs.last().unwrap());
                assert_eq!(start.start(), a + n * 3.0);
                assert_eq!(end.end(), b - n * 7.0);
                assert!(
                    matches!((start, end), (PathSeg::Line(_), PathSeg::Line(_))),
                    "{}",
                    path.d
                );
                for (seg, depth) in [(start, 18.0), (end, 23.0)] {
                    let shaft = seg.end() - seg.start();
                    assert!(shaft.dot(n) >= depth && shaft.cross(n) == 0.0, "{}", path.d);
                }
            }
        }
    }

    #[test]
    fn vertical_and_horizontal_sweeps_use_the_arrival_axis_and_adaptive_handles() {
        let m = Marker::default();
        for (pts, sides, expected) in [
            (
                [v(0.0, 0.0), v(2000.0, 60.0)],
                (Side::Bottom, Side::Top),
                "M0,0L0,6C0,18 2000,42 2000,54L2000,60",
            ),
            (
                [h(0.0, 0.0), h(80.0, 40.0)],
                (Side::Right, Side::Left),
                "M0,0L6,0C40,0 40,40 74,40L80,40",
            ),
        ] {
            let path = build_svg_path(&pts, sides, RouteStyle::Curved, (m, m), &[], None).unwrap();
            assert_eq!(path.d, expected);
            segments(&path);
        }
    }

    #[test]
    fn rounded_quarters_share_short_legs_and_wide_turns_use_both_handles() {
        let points = [v(0.0, 0.0), v(0.0, 30.0), h(16.0, 30.0), v(16.0, 90.0)];
        let m = Marker::default();
        let paint =
            |s| build_svg_path(&points, (Side::Bottom, Side::Top), s, (m, m), &[], None).unwrap();
        let rounded = paint(RouteStyle::Rounded(50.0));
        assert!(
            rounded.d.contains("L0,22A8,8 0 0,0 8,30A8,8 0 0,1 16,38"),
            "{}",
            rounded.d
        );
        segments(&rounded);
        let wide = paint(RouteStyle::Curved);
        let curves: Vec<_> = BezPath::from_svg(&wide.d)
            .unwrap()
            .elements()
            .iter()
            .filter_map(|e| {
                if let PathEl::CurveTo(a, b, c) = *e {
                    Some((a, b, c))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(curves.len(), 2);
        assert_eq!(curves[0].2, Point::new(8.0, 30.0));
        assert!((curves[0].0.y - (6.0 + 0.62 * 24.0)).abs() < 1e-12);
        assert!((curves[0].1.x - (8.0 - 0.62 * 8.0)).abs() < 1e-12);
        segments(&wide);
    }

    #[test]
    fn steps_move_only_the_internal_crossbar_and_keep_u_turns_outside() {
        let m = marker(4.0, 10.0);
        let paint = |p: &[RoutePoint], sides, style| {
            build_svg_path(p, sides, style, (m, m), &[], None).unwrap()
        };
        let sides = (Side::Bottom, Side::Top);
        let z = [v(0.0, 0.0), v(0.0, 50.0), h(100.0, 50.0), v(100.0, 100.0)];
        let u = [v(0.0, 0.0), v(0.0, 140.0), h(100.0, 140.0), v(100.0, 100.0)];
        for (style, y) in [
            (RouteStyle::Step, 50.0),
            (RouteStyle::StepBefore, 16.0),
            (RouteStyle::StepAfter, 84.0),
        ] {
            for p in [&z[..], &[z[0], z[3]]] {
                let path = paint(p, sides, style);
                assert!(segments(&path).into_iter().any(|s| s.start()==Point::new(0.0,y) && s.end()==Point::new(100.0,y)),"{}",path.d);
            }
            let path = paint(&u, (Side::Bottom, Side::Bottom), style);
            assert!(segments(&path).into_iter().any(
                |s| s.start() == Point::new(0.0, 140.0) && s.end() == Point::new(100.0, 140.0)
            ));
        }
        let internal = [v(0.0, 0.0), v(0.0, 20.0), v(60.0, 60.0), v(60.0, 100.0)];
        assert!(
            paint(&internal, sides, RouteStyle::StepBefore)
                .d
                .contains("L60,30")
        );
    }

    #[test]
    fn linear_preserves_an_interior_diagonal_and_discards_duplicate_contacts() {
        let p = [
            v(0.0, 0.0),
            v(0.0, 0.0),
            v(0.0, 20.0),
            h(70.0, 45.0),
            h(120.0, 45.0),
            h(120.0, 45.0),
        ];
        let path = build_svg_path(
            &p,
            (Side::Bottom, Side::Left),
            RouteStyle::Linear,
            (marker(4.0, 12.0), marker(2.0, 8.0)),
            &[],
            None,
        )
        .unwrap();
        let segs = segments(&path);
        assert!(
            segs.iter()
                .all(|s| matches!(s, PathSeg::Line(_)) && s.arclen(1e-6) > 0.0)
        );
        assert!(
            segs.iter()
                .any(|s| s.start() == Point::new(0.0, 20.0) && s.end() == Point::new(70.0, 45.0))
        );
        assert_eq!(segs[0].start(), Point::new(0.0, 4.0));
        assert_eq!(segs.last().unwrap().end(), Point::new(118.0, 45.0));
    }

    #[test]
    fn straight_routes_share_the_runway_and_empty_routes_have_no_paint_bounds() {
        let m = marker(3.0, 14.0);
        let paint = |p: &[RoutePoint]| {
            build_svg_path(
                p,
                (Side::Right, Side::Left),
                RouteStyle::Curved,
                (m, m),
                &[],
                None,
            )
        };
        let path = paint(&[h(0.0, 0.0), h(18.0, 0.0), h(26.0, 0.0)]).unwrap();
        assert_eq!(path.d, "M3,0L23,0");
        assert!(paint(&[]).is_none());
        assert!(paint(&[h(0.0, 0.0); 2]).is_none());
        assert!(paint(&[h(0.0, 0.0), h(f64::NAN, 1.0)]).is_none());
        assert!(paint(&[h(0.0, 0.0), h(5.0, 0.0)]).is_none());
        assert!(paint(&[h(0.0, 0.0), h(19.0, 0.0)]).is_none());
        assert!(paint(&[h(-f64::MAX, 2.0), h(f64::MAX, 1.0)]).is_none());
    }

    #[test]
    fn every_curve_passes_through_the_shared_group_port_in_one_subpath() {
        let route = [
            h(0., 0.),
            h(32., 0.),
            v(32., 20.),
            v(32., 92.),
            h(160., 92.),
        ];
        let stitch = BorderStitch {
            edge: 0,
            point: 2,
            group: 0,
            side: Side::Top,
        };
        let port = position(route[stitch.point]);
        for style in [
            RouteStyle::Curved,
            RouteStyle::Rounded(80.),
            RouteStyle::StepBefore,
            RouteStyle::StepAfter,
            RouteStyle::Linear,
        ] {
            let markers = (marker(3., 12.), marker(3., 12.));
            let path = build_svg_path(
                &route,
                (Side::Right, Side::Left),
                style,
                markers,
                &[stitch],
                None,
            )
            .unwrap();
            assert_eq!(path.d.matches('M').count(), 1, "{style:?}");
            let segments = segments(&path);
            assert!(
                segments
                    .windows(2)
                    .any(|pair| pair[0].end() == port && pair[1].start() == port),
                "{style:?}: {}",
                path.d
            );
        }
    }

    #[test]
    fn tight_bounds_use_bezier_extrema_not_control_points() {
        let mut p = Pen::new(Point::new(-1.0, 0.0));
        p.cubic(
            Point::new(-1.0, 10.0),
            Point::new(3.0, 10.0),
            Point::new(3.0, 0.0),
        );
        assert_eq!(
            p.bounds,
            Bounds {
                min_x: -1.0,
                min_y: 0.0,
                max_x: 3.0,
                max_y: 7.5
            }
        );
    }
}
