//! Paint a boundary-to-boundary axis route in the coordinate space supplied by its caller.

mod clearance;
pub use clearance::{Clearance, EdgeClearance, ObstacleOwner, TerminalContact};

use crate::number_format::{canonical_number as number, canonicalize_number};
use crate::route_types::{
    Axis, BorderStitch, KnotKind, Marker, RoutePoint, SVG_LENGTH_ACCURACY, Side,
    sweep_handle_fraction,
};
use crate::{Bounds, LayoutPoint};
use kurbo::{Arc, CubicBez, Line, ParamCurveArclen, PathSeg, Point, Shape, SvgArc, Vec2};
use std::fmt::Write as _;

/// Straight shaft beyond the marker setback, in layout units (v13 `shared/svg/route.ts`).
#[cfg(feature = "layout-shoal")]
pub use merman_layout_shoal::MIN_MARKER_LEAD as LEAD;
/// Straight shaft beyond the marker setback for standalone route painting.
#[cfg(not(feature = "layout-shoal"))]
pub const LEAD: f64 = 6.0;
const STEP_INSET: f64 = 12.0;
const TURN_HANDLE: f64 = 0.62;
const TERMINAL_TURN_RADIUS: f64 = 24.0;
const MAX_CORNER_ARM: f64 = TERMINAL_TURN_RADIUS;
const MAX_CORNER_ASPECT: f64 = 3.0;
const QUARTER_CIRCLE_HANDLE: f64 = 0.552_284_749_830_793_6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteStyle {
    Curved,
    Rounded(f64),
    Step,
    StepBefore,
    StepAfter,
    Linear,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum CurveTerminals {
    #[default]
    Fixed,
    Smooth,
}

impl RouteStyle {
    /// Selects the base shaft. Families that support sketch strokes paint them over a polyline.
    #[cfg(feature = "theme-svg")]
    pub fn from_theme(style: merman_theme_contract::v2::EdgeStyle) -> Self {
        use merman_theme_contract::v2::EdgeStyle;

        match style {
            EdgeStyle::Curved => Self::Curved,
            EdgeStyle::Orthogonal => Self::Step,
            EdgeStyle::Straight | EdgeStyle::Sketch => Self::Linear,
        }
    }

    pub fn from_interpolate(name: &str, rounded_radius: f64) -> Self {
        match name {
            "rounded" => Self::Rounded(rounded_radius),
            "step" => Self::Step,
            "stepBefore" => Self::StepBefore,
            "stepAfter" => Self::StepAfter,
            "linear" => Self::Linear,
            _ => Self::Curved,
        }
    }

    #[cfg(feature = "layout-shoal")]
    pub fn routing_style(self) -> merman_layout_shoal::RoutingStyle {
        use merman_layout_shoal::RoutingStyle;

        match self {
            Self::Curved => RoutingStyle::Curved,
            Self::Rounded(_) | Self::Step | Self::StepBefore | Self::StepAfter => {
                RoutingStyle::Orthogonal
            }
            Self::Linear => RoutingStyle::Polyline,
        }
    }

    fn stepped(self) -> bool {
        matches!(self, Self::Step | Self::StepBefore | Self::StepAfter)
    }
}

#[derive(Debug, Clone)]
pub struct PaintedRoute {
    pub d: String,
    /// Tight geometric bounds of `d`, excluding stroke width, markers and effects.
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RoutePaintError {
    InvalidGeometry,
    NoClearDetour,
    DetourBudgetExceeded,
    ClearanceAllocationFailed,
    ShaftAllocationFailed,
}

impl std::fmt::Display for RoutePaintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidGeometry => "invalid canonical route geometry",
            Self::NoClearDetour => "no clear path between fixed route points",
            Self::DetourBudgetExceeded => "canonical route detour budget exceeded",
            Self::ClearanceAllocationFailed => "could not allocate the route clearance index",
            Self::ShaftAllocationFailed => "could not allocate the painted route segments",
        })
    }
}

fn outward(side: Side) -> Vec2 {
    match side {
        Side::Top => Vec2::new(0.0, -1.0),
        Side::Right => Vec2::new(1.0, 0.0),
        Side::Bottom => Vec2::new(0.0, 1.0),
        Side::Left => Vec2::new(-1.0, 0.0),
    }
}

pub fn group_lead_side(side: Side, port: RoutePoint, next: RoutePoint) -> Side {
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
    RoutePoint::new(p.x, p.y, axis)
}

const ALIGN_EPS: f64 = 1e-9;

fn aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= ALIGN_EPS || (a.y - b.y).abs() <= ALIGN_EPS
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
    if (b - c).hypot() <= ALIGN_EPS {
        if p.kind == KnotKind::GapBoundary {
            knots.last_mut().unwrap().kind = p.kind;
        }
        return;
    }
    if last.kind == KnotKind::Free
        && let [.., a, _] = knots.as_slice()
    {
        let a = position(*a);
        if (((a.x - b.x).abs() <= ALIGN_EPS && (b.x - c.x).abs() <= ALIGN_EPS)
            || ((a.y - b.y).abs() <= ALIGN_EPS && (b.y - c.y).abs() <= ALIGN_EPS))
            && (b - a).dot(c - b) > 0.0
        {
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
    let reserved = |m: Option<Marker>| m.map_or(0.0, |m| m.trim + m.lead(false));
    let lo = reserved(markers.0);
    let hi = delta.abs() - reserved(markers.1);
    from + sign
        * if lo <= hi {
            offset.clamp(lo, hi)
        } else {
            delta.abs() * 0.5
        }
}

/// v13 moves an existing four-point step only when its crossbar lies between the ends,
/// both endpoints are real terminals (not a compound border stitch), and the relocated
/// segments remain clear of obstacles.
fn move_step(
    style: RouteStyle,
    knots: &mut [RoutePoint],
    markers: (Marker, Marker),
    clearance: Option<EdgeClearance<'_>>,
) {
    let [a, b, c, d] = *knots else { return };
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
    let (mut next_b, mut next_c) = (b, c);
    if vertical {
        next_b.y = level;
        next_c.y = level;
    } else {
        next_b.x = level;
        next_c.x = level;
    }
    if let Some(clearance) = clearance {
        let candidate = [a, next_b, next_c, d];
        if candidate
            .windows(2)
            .any(|seg| clearance.blocks_line(position(seg[0]), position(seg[1])))
        {
            return;
        }
    }
    knots[1] = next_b;
    knots[2] = next_c;
}

fn steps(
    out: &mut Vec<RoutePoint>,
    style: RouteStyle,
    route: &[RoutePoint],
    markers: (Option<Marker>, Option<Marker>),
    clearance: Option<EdgeClearance<'_>>,
) {
    out.clear();
    out.reserve(route.len());
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
            for candidate in std::iter::once(style).chain(
                [
                    RouteStyle::Step,
                    RouteStyle::StepBefore,
                    RouteStyle::StepAfter,
                ]
                .into_iter()
                .filter(|candidate| *candidate != style),
            ) {
                let (p, q) = match b.axis {
                    Axis::Vertical => {
                        let y = step_level(candidate, a.y, b.y, ends);
                        (RoutePoint { y, ..a }, RoutePoint { y, ..b })
                    }
                    Axis::Horizontal => {
                        let x = step_level(candidate, a.x, b.x, ends);
                        (RoutePoint { x, ..a }, RoutePoint { x, ..b })
                    }
                };
                if clearance.is_none_or(|guard| {
                    [a, p, q, b]
                        .windows(2)
                        .all(|leg| !guard.blocks_line(position(leg[0]), position(leg[1])))
                }) {
                    push_knot(out, p);
                    push_knot(out, q);
                    break;
                }
            }
            // An offset leg without a clear crossbar reaches the bounded rectilinear detour.
        }
        push_knot(out, b);
    }
}

/// Reduce multiple same-direction offsets between fixed borders to one caption crossbar.
fn caption_crossbar(route: &[RoutePoint], guard: EdgeClearance<'_>) -> Option<[RoutePoint; 4]> {
    let &[first, second, .., penultimate, last] = route else {
        return None;
    };
    let label = guard.label_bounds?;
    let vertical = (first.x - second.x).abs() <= ALIGN_EPS;
    let axes = |p: Point| if vertical { (p.y, p.x) } else { (p.x, p.y) };
    let (start_run, start_lane) = axes(position(first));
    let (end_run, end_lane) = axes(position(last));
    let (run, lane) = axes(label.center());
    let inside = |from: f64, value: f64, to: f64| from.min(to) < value && value < from.max(to);
    let nearest_lane = lane.clamp(start_lane.min(end_lane), start_lane.max(end_lane));
    let caption_half_span = if vertical {
        label.width()
    } else {
        label.height()
    } / 2.0;
    if !inside(start_run, run, end_run)
        || start_lane == end_lane
        || (nearest_lane - lane).abs() >= caption_half_span
        || (axes(position(second)).1 - start_lane).abs() > ALIGN_EPS
        || (axes(position(penultimate)).1 - end_lane).abs() > ALIGN_EPS
    {
        return None;
    }
    let mut changes = 0;
    let mut first_turn = None;
    let mut last_turn = start_run;
    let mut lateral_reversal = 0.0;
    for pair in route.windows(2) {
        let (a, b) = (axes(position(pair[0])), axes(position(pair[1])));
        if (b.0 - a.0) * (end_run - start_run).signum() < -ALIGN_EPS {
            return None;
        }
        if (b.1 - a.1).abs() > ALIGN_EPS {
            changes += 1;
            first_turn.get_or_insert(a.0);
            last_turn = b.0;
            lateral_reversal += ((a.1 - b.1) * (end_lane - start_lane).signum()).max(0.0);
        }
    }
    let first_turn = first_turn?;
    if changes < 2 || lateral_reversal > STEP_INSET || !inside(first_turn, run, last_turn) {
        return None;
    }
    let axis = if vertical {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let on_plane = |p: RoutePoint| {
        if vertical {
            RoutePoint { y: run, axis, ..p }
        } else {
            RoutePoint { x: run, axis, ..p }
        }
    };
    let candidate = [first, on_plane(first), on_plane(last), last];
    candidate
        .windows(2)
        .all(|pair| !guard.blocks_line(position(pair[0]), position(pair[1])))
        .then_some(candidate)
}

fn sweep_curve(from: Point, end: RoutePoint, contour_departure: bool, scale: f64) -> CubicBez {
    let to = position(end);
    let delta = to - from;
    let (run, across) = match end.axis {
        Axis::Vertical => (delta.y, delta.x),
        Axis::Horizontal => (delta.x, delta.y),
    };
    let length = run * sweep_handle_fraction(across, run) * scale;
    let across_handle = if contour_departure {
        (0.35 * length.abs())
            .min(0.25 * across.abs())
            .copysign(across)
    } else {
        0.0
    };
    let (departure, arrival) = match end.axis {
        Axis::Vertical => (Vec2::new(across_handle, length), Vec2::new(0.0, length)),
        Axis::Horizontal => (Vec2::new(length, across_handle), Vec2::new(length, 0.0)),
    };
    CubicBez::new(from, from + departure, to - arrival, to)
}

struct Pen<'a> {
    d: String,
    bounds: Bounds,
    at: Point,
    finite: bool,
    compact_lines: bool,
    line_start: Option<Point>,
    length: Option<f64>,
    segments: Option<&'a mut Vec<PathSeg>>,
    shaft_allocation_failed: bool,
    detour_budget: clearance::DetourBudget,
}

fn svg_point(point: Point) -> Point {
    Point::new(canonicalize_number(point.x), canonicalize_number(point.y))
}

impl<'a> Pen<'a> {
    fn new(
        p: Point,
        compact_lines: bool,
        measure_length: bool,
        segments: Option<&'a mut Vec<PathSeg>>,
    ) -> Self {
        Self {
            d: String::new(),
            bounds: Bounds {
                min_x: p.x,
                min_y: p.y,
                max_x: p.x,
                max_y: p.y,
            },
            at: p,
            finite: p.is_finite(),
            compact_lines,
            line_start: None,
            length: measure_length.then_some(0.0),
            segments,
            shaft_allocation_failed: false,
            detour_budget: clearance::DetourBudget::default(),
        }
    }

    fn start_path(&mut self, points: usize, style: RouteStyle) {
        if !self.d.is_empty() {
            return;
        }
        // Size the first allocation from the simplified section, with room for controls on
        // curves. Later sections and unusually long coordinates can grow it as they are emitted.
        const MAX_INITIAL_BYTES: usize = 384;
        let bytes_per_point = match style {
            RouteStyle::Curved | RouteStyle::Rounded(_) => 48,
            _ => 24,
        };
        let capacity = points
            .checked_mul(bytes_per_point)
            .unwrap_or(MAX_INITIAL_BYTES)
            .min(MAX_INITIAL_BYTES);
        let _ = self.d.try_reserve_exact(capacity);
        let p = svg_point(self.at);
        let _ = write!(self.d, "M{},{}", p.x, p.y);
    }

    fn include(&mut self, p: Point) {
        self.finite &= p.is_finite();
        self.bounds.min_x = self.bounds.min_x.min(p.x);
        self.bounds.min_y = self.bounds.min_y.min(p.y);
        self.bounds.max_x = self.bounds.max_x.max(p.x);
        self.bounds.max_y = self.bounds.max_y.max(p.y);
    }

    fn line(&mut self, p: Point) {
        if (p - self.at).hypot() > ALIGN_EPS {
            if let Some(start) = self.line_start {
                let (previous, next) = (self.at - start, p - self.at);
                let scale = previous.hypot().max(next.hypot());
                if previous.cross(next).abs() > ALIGN_EPS * scale || previous.dot(next) <= 0.0 {
                    self.flush_line();
                }
            }
            self.line_start.get_or_insert(self.at);
            self.include(p);
            self.at = p;
            if !self.compact_lines {
                self.flush_line();
            }
        }
    }

    fn flush_line(&mut self) {
        if let Some(start) = self.line_start.take() {
            let (start, end) = (svg_point(start), svg_point(self.at));
            if start == end {
                return;
            }
            let _ = write!(self.d, "L{},{}", end.x, end.y);
            self.record_segment(PathSeg::Line(Line::new(start, end)));
            if let Some(length) = &mut self.length {
                *length += (end - start).hypot();
            }
        }
    }

    fn record_segment(&mut self, segment: PathSeg) {
        if let Some(segments) = &mut self.segments
            && !self.shaft_allocation_failed
        {
            if segments.try_reserve(1).is_ok() {
                segments.push(segment);
            } else {
                self.shaft_allocation_failed = true;
            }
        }
    }

    fn cubic(&mut self, a: Point, b: Point, end: Point) {
        if !a.is_finite() || !b.is_finite() || !end.is_finite() {
            self.finite = false;
            return;
        }
        self.flush_line();
        let [from, a, b, emitted_end] = [self.at, a, b, end].map(svg_point);
        let _ = write!(
            self.d,
            "C{},{} {},{} {},{}",
            a.x, a.y, b.x, b.y, emitted_end.x, emitted_end.y
        );
        let curve = CubicBez::new(from, a, b, emitted_end);
        self.record_segment(PathSeg::Cubic(curve));
        if let Some(length) = &mut self.length {
            *length += curve.arclen(SVG_LENGTH_ACCURACY);
        }
        let bounds = curve.bounding_box();
        self.include(Point::new(bounds.x0, bounds.y0));
        self.include(Point::new(bounds.x1, bounds.y1));
        self.at = end;
    }

    fn sweep(
        &mut self,
        end: RoutePoint,
        contour_departure: bool,
        clearance: Option<EdgeClearance<'_>>,
    ) -> Result<(), RoutePaintError> {
        let target = position(end);
        let curve = |scale| sweep_curve(self.at, end, contour_departure, scale);
        let full = curve(1.0);
        let sweep = if let Some(guard) = clearance
            && guard.blocks_cubic(full)
        {
            let Some(scale) = shrink_smoothing_scale(|scale| {
                if scale == 0. {
                    guard.blocks_line(self.at, target)
                } else {
                    guard.blocks_cubic(curve(scale))
                }
            }) else {
                return self.detour_to(target, guard, false);
            };
            if scale == 0.0 {
                self.line(target);
                return Ok(());
            }
            curve(scale)
        } else {
            full
        };
        self.cubic(sweep.p1, sweep.p2, sweep.p3);
        Ok(())
    }

    fn detour_to(
        &mut self,
        target: Point,
        clearance: EdgeClearance<'_>,
        orthogonal: bool,
    ) -> Result<(), RoutePaintError> {
        for point in clearance.detour(self.at, target, orthogonal, &mut self.detour_budget)? {
            self.line(point);
        }
        Ok(())
    }

    fn quarter_arc(&mut self, radius: f64, clockwise: bool, end: Point) {
        self.flush_line();
        // The SVG arc parser measures a cubic approximation of its rounded parameters.
        self.length = None;
        let emitted_end = svg_point(end);
        let _ = write!(
            self.d,
            "A{0},{0} 0 0,{1} {2},{3}",
            number(radius),
            u8::from(clockwise),
            emitted_end.x,
            emitted_end.y
        );
        if self.segments.is_some() {
            let mut from = svg_point(self.at);
            let svg_arc = SvgArc {
                from,
                to: emitted_end,
                radii: Vec2::splat(canonicalize_number(radius)),
                x_rotation: 0.0,
                large_arc: false,
                sweep: clockwise,
            };
            if let Some(arc) = Arc::from_svg_arc(&svg_arc) {
                // Use the same cubic conversion as kurbo's SVG path reader.
                arc.to_cubic_beziers(0.1, |a, b, to| {
                    self.record_segment(PathSeg::Cubic(CubicBez::new(from, a, b, to)));
                    from = to;
                });
            } else {
                self.record_segment(PathSeg::Line(Line::new(from, emitted_end)));
            }
        }
        // Each coordinate is monotone on an axis-aligned quarter circle.
        self.include(end);
        self.at = end;
    }

    fn finish(self) -> Result<PaintedRoute, RoutePaintError> {
        if self.shaft_allocation_failed {
            return Err(RoutePaintError::ShaftAllocationFailed);
        }
        if !self.finite {
            return Err(RoutePaintError::InvalidGeometry);
        }
        Ok(PaintedRoute {
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
    let ab_vert = (a.x - b.x).abs() <= ALIGN_EPS && (a.y - b.y).abs() > ALIGN_EPS;
    let ab_horiz = (a.y - b.y).abs() <= ALIGN_EPS && (a.x - b.x).abs() > ALIGN_EPS;
    let bc_vert = (b.x - c.x).abs() <= ALIGN_EPS && (b.y - c.y).abs() > ALIGN_EPS;
    let bc_horiz = (b.y - c.y).abs() <= ALIGN_EPS && (b.x - c.x).abs() > ALIGN_EPS;
    (ab_vert && bc_horiz) || (ab_horiz && bc_vert)
}

fn smoothing_scale(blocked: impl Fn(f64) -> bool) -> Option<f64> {
    if !blocked(1.0) {
        return Some(1.0);
    }
    shrink_smoothing_scale(blocked)
}

fn shrink_smoothing_scale(blocked: impl Fn(f64) -> bool) -> Option<f64> {
    // The caller has already checked that scale 1.0 is blocked.
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
    (safe > 0.0 || !blocked(0.0)).then_some(safe)
}

fn draw_body(
    pen: &mut Pen,
    body: &[RoutePoint],
    style: RouteStyle,
    terminals: (Option<&Terminal>, Option<&Terminal>),
    clearance: Option<EdgeClearance<'_>>,
) -> Result<(), RoutePaintError> {
    if let Some(guard) = clearance
        && (style.stepped() || style == RouteStyle::Linear)
    {
        for (i, &knot) in body.iter().enumerate().skip(1) {
            let p = position(knot);
            if (style.stepped() && !aligned(pen.at, p)) || guard.blocks_line(pen.at, p) {
                // A bend from the placement style can lie inside an obstacle after switching
                // styles. The section ends capture the fixed terminals and border contacts.
                if i + 1 < body.len() && guard.blocks_line(p, p) {
                    continue;
                }
                pen.detour_to(p, guard, style.stepped())?;
            } else {
                pen.line(p);
            }
        }
        return Ok(());
    }
    for i in 1..body.len() {
        let p = position(body[i]);
        if matches!(style, RouteStyle::Curved | RouteStyle::Rounded(_)) && right_angle(body, i) {
            let incoming = p - position(body[i - 1]);
            let outgoing = position(body[i + 1]) - p;
            let mut before = incoming.hypot();
            let mut after = outgoing.hypot();
            let u = incoming / before;
            let v = outgoing / after;
            let previous_turn = right_angle(body, i - 1);
            let next_turn = right_angle(body, i + 1);
            if previous_turn {
                before *= 0.5;
            }
            if next_turn {
                after *= 0.5;
            }
            if let RouteStyle::Rounded(radius) = style {
                let r = radius.min(before).min(after);
                let r = r * clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| {
                        guard.blocks_quarter(p - u * (r * scale), p, p + v * (r * scale))
                    })
                    .unwrap_or(0.)
                });
                if r > 1e-6 {
                    pen.line(p - u * r);
                    pen.quarter_arc(r, u.cross(v) > 0.0, p + v * r);
                } else {
                    pen.line(p);
                }
            } else {
                // A side terminal can turn onto a rail before it leaves the rank. Use
                // equal arms so a long horizontal lead doesn't stretch the corner.
                let on_normal = |terminal: &Terminal| {
                    let delta = p - terminal.tip;
                    delta.cross(terminal.normal) == 0.0 && delta.dot(terminal.normal) > 0.0
                };
                let terminal_turn = !previous_turn
                    && !next_turn
                    && ((i == 1
                        && body[i + 1].kind == KnotKind::GapBoundary
                        && terminals.0.is_some_and(on_normal))
                        || (i + 2 == body.len()
                            && body[i - 1].kind == KnotKind::GapBoundary
                            && terminals.1.is_some_and(on_normal)));
                let track_shelf = body[i - 1].kind == KnotKind::GapBoundary
                    || body[i + 1].kind == KnotKind::GapBoundary;
                let (before, after, handle) = if terminal_turn {
                    let arm = before.min(after).min(TERMINAL_TURN_RADIUS);
                    (arm, arm, QUARTER_CIRCLE_HANDLE)
                } else {
                    // Bound right-angle corner asymmetry so a long straight corridor
                    // meeting a short jog stays straight instead of bowing into a banana arc.
                    // Free corners outside explicit GapBoundary shelves also cap arm length.
                    let cap = before.min(after) * MAX_CORNER_ASPECT;
                    let (b, a) = (before.min(cap), after.min(cap));
                    if track_shelf {
                        (b, a, TURN_HANDLE)
                    } else {
                        (b.min(MAX_CORNER_ARM), a.min(MAX_CORNER_ARM), TURN_HANDLE)
                    }
                };
                let curve = |scale| {
                    let incoming = before * scale;
                    let outgoing = after * scale;
                    let start = p - u * incoming;
                    let end = p + v * outgoing;
                    CubicBez::new(
                        start,
                        start + u * (handle * incoming),
                        end - v * (handle * outgoing),
                        end,
                    )
                };
                let scale = clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| guard.blocks_cubic(curve(scale))).unwrap_or(0.)
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
            pen.sweep(
                body[i],
                body[i - 1].kind == KnotKind::ContourDeparture,
                clearance,
            )?;
        } else {
            pen.line(p);
        }
    }
    Ok(())
}

struct Terminal {
    tip: Point,
    normal: Vec2,
    marker: Marker,
    smooth: bool,
}

impl Marker {
    fn lead(self, smooth: bool) -> f64 {
        if smooth {
            if self.trim == 0.0 && self.depth == 0.0 && self.width == 0.0 {
                return 0.0;
            }
            LEAD.max(self.depth).max(self.trim)
        } else {
            LEAD.max(self.depth)
        }
    }

    #[cfg(feature = "shoal-bridge")]
    pub(crate) fn smooth_runway(self) -> f64 {
        self.trim + self.lead(true)
    }
}

impl Terminal {
    fn shaft_end(&self) -> Point {
        self.tip + self.normal * self.marker.trim
    }

    fn lead_end(&self) -> Point {
        self.shaft_end() + self.normal * self.lead()
    }

    fn lead(&self) -> f64 {
        self.marker.lead(self.smooth)
    }

    fn covers(&self, knot: RoutePoint) -> bool {
        let delta = position(knot) - self.tip;
        delta.cross(self.normal).abs() <= ALIGN_EPS
            && (0.0..=self.marker.trim + self.lead() + ALIGN_EPS).contains(&delta.dot(self.normal))
    }
}

pub fn marker_lead(port: RoutePoint, side: Side, marker: Marker) -> (LayoutPoint, LayoutPoint) {
    let terminal = Terminal {
        tip: position(port),
        normal: outward(side),
        marker,
        smooth: false,
    };
    let point = |p: Point| LayoutPoint { x: p.x, y: p.y };
    (point(terminal.shaft_end()), point(terminal.lead_end()))
}

/// Marker ink envelope at a canonical contour contact, before the edge's stroke outset.
pub fn marker_bounds(port: RoutePoint, side: Side, marker: Marker) -> Option<kurbo::Rect> {
    if marker.depth == 0.0 || marker.width == 0.0 {
        return None;
    }
    let start = position(port);
    let outward = outward(side);
    let end = start + outward * (marker.trim + marker.depth);
    let width = marker.width / 2.0;
    Some(
        kurbo::Rect::from_points(start, end)
            .inflate(outward.y.abs() * width, outward.x.abs() * width),
    )
}

pub fn marker_obstacle(
    edge: usize,
    endpoint: crate::route_types::Endpoint,
    port: RoutePoint,
    side: Side,
    marker: Marker,
) -> Option<(kurbo::Rect, ObstacleOwner)> {
    let rect = marker_bounds(port, side, marker)?;
    Some((
        rect,
        ObstacleOwner::Marker(edge, TerminalContact::new(endpoint, port)),
    ))
}

/// Temporary route points reusable across the edges of one SVG operation.
#[derive(Default)]
pub struct RoutePaintScratch {
    knots: Vec<RoutePoint>,
    body: Vec<RoutePoint>,
    measure_length: bool,
    measured_length: Option<f64>,
    record_segments: bool,
    segments: Vec<PathSeg>,
}

/// Per-operation buffers shared by a canonical route adapter and its SVG writer.
#[doc(hidden)]
#[derive(Default)]
pub struct CanonicalPaintScratch {
    pub points: Vec<RoutePoint>,
    pub route: RoutePaintScratch,
    pub data_points: Vec<LayoutPoint>,
}

impl RoutePaintScratch {
    /// Measures the next route through this scratch using the same segments written to SVG.
    pub fn measure_length(&mut self) {
        self.measure_length = true;
    }

    /// Returns the requested centerline length of the last successful paint, when measured.
    pub fn measured_length(&self) -> Option<f64> {
        self.measured_length
    }

    /// Retains the shaft segments of the next route alongside its SVG path.
    pub fn record_segments(&mut self) {
        self.record_segments = true;
    }

    /// The last painted shaft, in the same coordinate space as its path data.
    pub fn segments(&self) -> &[PathSeg] {
        &self.segments
    }
}

/// The endpoints are already on the contours. The two sides are in semantic source/target
/// order, including on reverse-ranked edges. Trim distances refer to the SVG marker reference
/// points, measured outward from the supplied contour contacts.
pub fn build_svg_path(
    points: &[RoutePoint],
    sides: (Side, Side),
    style: RouteStyle,
    markers: (Marker, Marker),
    curve_terminals: CurveTerminals,
    stitches: &[BorderStitch],
    clearance: Option<EdgeClearance<'_>>,
) -> Result<PaintedRoute, RoutePaintError> {
    build_svg_path_with_scratch(
        points,
        sides,
        style,
        markers,
        curve_terminals,
        stitches,
        clearance,
        &mut RoutePaintScratch::default(),
    )
}

/// Paints a route as in [`build_svg_path`], reusing scratch after success or failure.
#[expect(
    clippy::too_many_arguments,
    reason = "The reusable entry point takes the same route inputs as the standalone entry point."
)]
pub fn build_svg_path_with_scratch(
    points: &[RoutePoint],
    sides: (Side, Side),
    style: RouteStyle,
    markers: (Marker, Marker),
    curve_terminals: CurveTerminals,
    stitches: &[BorderStitch],
    clearance: Option<EdgeClearance<'_>>,
    scratch: &mut RoutePaintScratch,
) -> Result<PaintedRoute, RoutePaintError> {
    let measure_length = std::mem::take(&mut scratch.measure_length);
    scratch.measured_length = None;
    scratch.segments.clear();
    let record_segments = std::mem::take(&mut scratch.record_segments);
    if points.len() < 2
        || [markers.0, markers.1]
            .iter()
            .any(|m| !m.trim.is_finite() || m.trim < 0.0 || !m.depth.is_finite() || m.depth < 0.0)
        || matches!(style, RouteStyle::Rounded(radius) if !radius.is_finite())
    {
        return Err(RoutePaintError::InvalidGeometry);
    }
    let last = points.len() - 1;
    let smooth = style == RouteStyle::Curved && matches!(curve_terminals, CurveTerminals::Smooth);
    let start = Terminal {
        tip: position(points[0]),
        normal: outward(sides.0),
        marker: markers.0,
        smooth,
    };
    let end = Terminal {
        tip: position(points[last]),
        normal: outward(sides.1),
        marker: markers.1,
        smooth,
    };
    let mut pen = Pen::new(
        start.shaft_end(),
        smooth,
        measure_length && !matches!(style, RouteStyle::Rounded(_)),
        record_segments.then_some(&mut scratch.segments),
    );
    let (knots, body) = (&mut scratch.knots, &mut scratch.body);
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
            points
                .get(begin..=finish)
                .ok_or(RoutePaintError::InvalidGeometry)?,
            (
                (begin == 0).then_some(&start),
                (finish == last).then_some(&end),
            ),
            style,
            knots,
            body,
            clearance,
        )?;
        pen.flush_line();
        begin = finish;
    }
    let length = pen.length.filter(|length| length.is_finite());
    let painted = pen.finish()?;
    scratch.measured_length = length;
    Ok(painted)
}

fn draw_section(
    pen: &mut Pen,
    points: &[RoutePoint],
    terminals: (Option<&Terminal>, Option<&Terminal>),
    style: RouteStyle,
    route: &mut Vec<RoutePoint>,
    body: &mut Vec<RoutePoint>,
    clearance: Option<EdgeClearance<'_>>,
) -> Result<(), RoutePaintError> {
    route.clear();
    route.reserve(points.len());
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(RoutePaintError::InvalidGeometry);
        }
        push_knot(route, point);
    }
    if route.len() < 2 {
        return (terminals.0.is_none() && terminals.1.is_none())
            .then_some(())
            .ok_or(RoutePaintError::InvalidGeometry);
    }
    if style.stepped() {
        if style != RouteStyle::Step
            && terminals.0.is_none()
            && terminals.1.is_none()
            && let Some(path) = clearance.and_then(|guard| caption_crossbar(route, guard))
        {
            route.clear();
            route.extend(path);
        }
        let markers = (
            terminals.0.map(|end| end.marker),
            terminals.1.map(|end| end.marker),
        );
        if terminals.0.is_some() && terminals.1.is_some() {
            move_step(
                style,
                route,
                (markers.0.unwrap_or_default(), markers.1.unwrap_or_default()),
                clearance,
            );
        }
        if has_offset_leg(route) {
            steps(body, style, route, markers, clearance);
            std::mem::swap(route, body);
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
            return Err(RoutePaintError::InvalidGeometry);
        }
        if aligned(start.tip, end.tip)
            && (end.tip - start.tip).dot(start.normal) > 0.0
            && (!(style.stepped() || style == RouteStyle::Linear)
                || clearance.is_none_or(|guard| !guard.blocks_line(pen.at, end.shaft_end())))
        {
            // Opposed markers can share this one straight segment up to its full length.
            if gap < start.lead().max(end.lead()) {
                return Err(RoutePaintError::InvalidGeometry);
            }
            pen.start_path(2, RouteStyle::Linear);
            pen.line(end.shaft_end());
            return Ok(());
        }
    }

    body.clear();
    body.reserve(route.len());
    push_knot(
        body,
        terminals.0.map_or(route[0], |start| RoutePoint {
            kind: route[0].kind,
            ..at(start.lead_end(), route[0].axis)
        }),
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
    // With different endpoint sides, inserting a normal shaft can create an offset leg even
    // on a previously orthogonal route. Step renderers must still emit only axis-aligned legs.
    if style.stepped() && has_offset_leg(body) {
        steps(route, style, body, (None, None), clearance);
        std::mem::swap(route, body);
    }
    pen.start_path(body.len(), style);
    pen.line(position(body[0]));
    draw_body(pen, body, style, terminals, clearance)?;
    if let Some(end) = terminals.1 {
        pen.line(end.shaft_end());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_types::Endpoint;
    use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathEl, PathSeg};

    #[test]
    fn serialized_lines_skip_knots_that_round_to_the_same_point() {
        let mut segments = Vec::new();
        let mut pen = Pen::new(Point::ORIGIN, false, true, Some(&mut segments));
        pen.start_path(4, RouteStyle::Step);
        pen.line(Point::new(3.0, 0.0));
        pen.line(Point::new(3.0, 0.000_000_5));
        pen.line(Point::new(3.0, 4.0));
        assert_eq!(pen.length, Some(7.0));
        assert_eq!(pen.finish().unwrap().d, "M0,0L3,0L3,4");
        assert_eq!(segments.len(), 2);
    }

    fn v(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Vertical)
    }
    fn h(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Horizontal)
    }
    fn marker(trim: f64, depth: f64) -> Marker {
        Marker {
            trim,
            depth,
            width: 10.0,
        }
    }

    #[test]
    fn measured_centerline_matches_the_serialized_path_with_the_same_tolerance() {
        let points = [
            v(1024.1, 3.37),
            v(1024.1, 39.1),
            v(-13.41, 102.7),
            v(-13.41, 160.13),
        ];
        let sides = (Side::Bottom, Side::Top);
        let markers = (Marker::default(), marker(4.0, 10.0));
        let mut scratch = RoutePaintScratch::default();
        let mut paint = |style, measure| {
            if measure {
                scratch.measure_length();
            }
            let path = build_svg_path_with_scratch(
                &points,
                sides,
                style,
                markers,
                CurveTerminals::Smooth,
                &[],
                None,
                &mut scratch,
            )
            .unwrap();
            (path, scratch.measured_length())
        };
        for style in [RouteStyle::Curved, RouteStyle::Step, RouteStyle::Linear] {
            let (painted, length) = paint(style, true);
            let parsed = BezPath::from_svg(&painted.d).unwrap();
            assert_eq!(
                length,
                Some(parsed.perimeter(SVG_LENGTH_ACCURACY)),
                "{style:?}"
            );
            assert_eq!(paint(style, false).1, None);
        }
        assert_eq!(paint(RouteStyle::Rounded(6.0), true).1, None);
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
    fn track_curves_stay_between_pinned_gap_boundaries() {
        let pinned = |mut point: RoutePoint| {
            point.kind = KnotKind::GapBoundary;
            point
        };
        let route = [
            v(0., 0.),
            pinned(v(0., 24.)),
            v(0., 50.),
            h(100., 50.),
            pinned(v(100., 72.)),
            v(100., 140.),
        ];
        let path = build_svg_path(
            &route,
            (Side::Bottom, Side::Top),
            RouteStyle::Curved,
            (Marker::default(), marker(4., 10.)),
            CurveTerminals::Smooth,
            &[],
            None,
        )
        .unwrap();
        let segments = segments(&path);
        let curves: Vec<_> = segments
            .iter()
            .filter_map(|segment| match segment {
                PathSeg::Cubic(curve) => Some(*curve),
                _ => None,
            })
            .collect();
        assert_eq!(curves.len(), 2);
        assert_eq!(curves[0].p0, Point::new(0., 24.));
        assert_eq!(curves[0].p3, curves[1].p0);
        assert_eq!(curves[1].p3, Point::new(100., 72.));
        assert!(segments.last().unwrap().arclen(1e-8) >= 10.);
    }

    #[test]
    fn smooth_departure_preserves_the_solved_turn_start_and_marked_shaft() {
        let path = build_svg_path(
            &[v(0., 0.), v(0., 12.), v(90., 30.), v(90., 50.)],
            (Side::Bottom, Side::Top),
            RouteStyle::Curved,
            (Marker::default(), marker(4., 10.)),
            CurveTerminals::Smooth,
            &[],
            None,
        )
        .unwrap();
        let [
            PathSeg::Line(start),
            PathSeg::Cubic(curve),
            PathSeg::Line(end),
        ] = segments(&path)[..]
        else {
            panic!("{}", path.d);
        };
        assert_eq!((start.p0, start.p1), (Point::ZERO, Point::new(0., 12.)));
        assert_eq!(curve.p0, start.p1);
        assert_eq!(curve.p1.x, curve.p0.x);
        assert_eq!(curve.p3, Point::new(90., 30.));
        assert_eq!((end.p0, end.p1), (curve.p3, Point::new(90., 46.)));
    }

    #[test]
    fn a_long_straight_approach_stays_outside_the_terminal_curve() {
        let path = build_svg_path(
            &[v(100., 0.), v(100., 12.), v(0., 54.), v(0., 450.)],
            (Side::Bottom, Side::Top),
            RouteStyle::Curved,
            (Marker::default(), marker(4., 10.)),
            CurveTerminals::Smooth,
            &[],
            None,
        )
        .unwrap();
        let [
            PathSeg::Line(_),
            PathSeg::Cubic(curve),
            PathSeg::Line(shaft),
        ] = segments(&path)[..]
        else {
            panic!("{}", path.d);
        };
        assert_eq!(curve.p3, Point::new(0., 54.));
        assert_eq!(shaft.p1, Point::new(0., 446.));
    }

    #[test]
    fn wide_corners_clear_foreign_labels_without_changing_the_other_turn_or_stitches() {
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
        let tree = Clearance::new(vec![(obstacle + delta, ObstacleOwner::EdgeLabel(1))]).unwrap();
        for style in [RouteStyle::Curved, RouteStyle::Rounded(40.)] {
            let paint = |guard| {
                build_svg_path(
                    &points,
                    (Side::Right, Side::Left),
                    style,
                    markers,
                    CurveTerminals::Fixed,
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
        let points = [h(0., 0.), h(10., 0.), h(110., 50.), h(120., 50.)];
        let obstacle = kurbo::Rect::new(25., 0.75, 35., 5.);
        fn paint(points: &[RoutePoint], guard: Option<EdgeClearance<'_>>) -> PaintedRoute {
            build_svg_path(
                points,
                (Side::Right, Side::Left),
                RouteStyle::Curved,
                (Marker::default(), Marker::default()),
                CurveTerminals::Fixed,
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
            let tree = Clearance::new(vec![(obstacle, owner)]).unwrap();
            let result = paint(&points, Some(tree.for_edge(1, endpoints, Vec2::ZERO, 0.)));
            assert_eq!(samples_inside(&result, obstacle), !protected);
            assert_eq!(result.d == original.d, !protected);
            assert!(result.d.contains('C'));
            assert_eq!(segments(&result).first(), segments(&original).first());
            assert_eq!(segments(&result).last(), segments(&original).last());
        }
    }

    #[test]
    fn a_blocked_sweep_and_chord_detour_without_moving_stitches_or_markers() {
        let points = [h(0., 0.), h(20., 0.), h(120., 60.), h(140., 60.)];
        let stitches = [1, 2].map(|point| BorderStitch {
            edge: 0,
            group: point as u32,
            point,
            side: Side::Left,
        });
        let obstacles = [
            kurbo::Rect::new(45., 10., 95., 50.),
            kurbo::Rect::new(116., -2., 124., 6.),
            kurbo::Rect::new(16., 54., 24., 66.),
        ];
        let tree = Clearance::new(
            obstacles
                .into_iter()
                .map(|rect| (rect, ObstacleOwner::EdgeLabel(1)))
                .collect(),
        )
        .unwrap();
        let markers = (marker(3., 8.), marker(2., 9.));
        let path = build_svg_path(
            &points,
            (Side::Right, Side::Left),
            RouteStyle::Curved,
            markers,
            CurveTerminals::Fixed,
            &stitches,
            Some(tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.)),
        )
        .unwrap();
        assert_eq!(path.d.matches('M').count(), 1);
        for obstacle in obstacles {
            assert!(
                !samples_inside(&path, obstacle.inflate(1., 1.)),
                "{}",
                path.d
            );
        }
        let segments = segments(&path);
        assert_eq!(segments[0].start(), Point::new(3., 0.));
        assert_eq!(segments[0].end(), Point::new(11., 0.));
        for stitch in stitches {
            let port = position(points[stitch.point]);
            assert!(
                segments
                    .windows(2)
                    .any(|pair| pair[0].end() == port && pair[1].start() == port)
            );
        }
        assert_eq!(segments.last().unwrap().start(), Point::new(129., 60.));
        assert_eq!(segments.last().unwrap().end(), Point::new(138., 60.));
    }

    #[test]
    fn a_fixed_sweep_point_inside_a_foreign_box_returns_an_error() {
        let points = [h(0., 0.), h(20., 0.), h(120., 60.), h(140., 60.)];
        let tree = Clearance::new(vec![(
            kurbo::Rect::new(18., -2., 22., 2.),
            ObstacleOwner::GroupTitle(0),
        )])
        .unwrap();
        let result = build_svg_path(
            &points,
            (Side::Right, Side::Left),
            RouteStyle::Curved,
            (marker(3., 8.), marker(2., 9.)),
            CurveTerminals::Fixed,
            &[],
            Some(tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.)),
        );
        assert!(matches!(result, Err(RoutePaintError::NoClearDetour)));
    }

    #[test]
    fn coincident_marker_contacts_preserve_the_shaft_and_other_obstacles() {
        let points = [h(0., 0.), h(12., 0.), h(120., 30.), h(140., 30.)];
        let offset = Vec2::new(1024., -256.);
        let layout_contact = |p: RoutePoint| RoutePoint {
            x: p.x + offset.x,
            y: p.y + offset.y,
            ..p
        };
        let contacts = (layout_contact(points[0]), layout_contact(points[3]));
        let endpoints = (Endpoint::Node(4), Endpoint::Group(1));
        let marker = Marker {
            trim: 2.,
            depth: 10.,
            width: 16.,
        };
        let bound = |port, side| marker_bounds(port, side, marker).unwrap();
        let port = contacts.0;
        let marker_for =
            |endpoint, contact| ObstacleOwner::Marker(1, TerminalContact::new(endpoint, contact));
        let adjacent = RoutePoint {
            y: port.y + 1.,
            ..port
        };
        let co_terminal = Clearance::new(vec![
            (bound(port, Side::Right), marker_for(endpoints.0, port)),
            (
                bound(port, Side::Top),
                marker_for(endpoints.0, v(port.x, port.y)),
            ),
        ])
        .unwrap();
        for style in [RouteStyle::Curved, RouteStyle::Step] {
            let paint = |clearance: Option<&Clearance>| {
                build_svg_path(
                    &points,
                    (Side::Right, Side::Left),
                    style,
                    (Marker::default(), Marker::default()),
                    CurveTerminals::Smooth,
                    &[],
                    clearance.map(|tree| {
                        tree.for_edge(0, endpoints, offset, 1.)
                            .with_terminal_contacts(contacts)
                    }),
                )
            };
            for (rect, owner) in [
                (
                    bound(adjacent, Side::Right),
                    marker_for(endpoints.0, adjacent),
                ),
                (
                    bound(port, Side::Right),
                    marker_for(Endpoint::Node(5), port),
                ),
                (bound(port, Side::Right), ObstacleOwner::GroupTitle(1)),
            ] {
                let tree = Clearance::new(vec![(rect, owner)]).unwrap();
                assert!(
                    matches!(paint(Some(&tree)), Err(RoutePaintError::NoClearDetour)),
                    "{style:?} through {owner:?}"
                );
            }
            assert_eq!(paint(Some(&co_terminal)).unwrap().d, paint(None).unwrap().d);
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
                    CurveTerminals::Fixed,
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
        for (pts, sides, along, across) in [
            (
                [v(0.0, 0.0), v(2000.0, 60.0)],
                (Side::Bottom, Side::Top),
                Vec2::new(0., 48.),
                2000.,
            ),
            (
                [h(0.0, 0.0), h(80.0, 40.0)],
                (Side::Right, Side::Left),
                Vec2::new(68., 0.),
                40.,
            ),
        ] {
            let path = build_svg_path(
                &pts,
                sides,
                RouteStyle::Curved,
                (m, m),
                CurveTerminals::Fixed,
                &[],
                None,
            )
            .unwrap();
            let [
                PathSeg::Line(start),
                PathSeg::Cubic(curve),
                PathSeg::Line(end),
            ] = segments(&path)[..]
            else {
                panic!("{}", path.d);
            };
            let handle = along * sweep_handle_fraction(across, along.hypot());
            assert_eq!(curve.p0, start.p1);
            assert_eq!(curve.p3, end.p0);
            assert_eq!(curve.p1, curve.p0 + handle);
            assert_eq!(curve.p2, curve.p3 - handle);
        }
    }

    #[test]
    fn offset_cubics_have_ordered_handles_in_each_flow_direction() {
        for (axis, sides) in [
            (Axis::Vertical, (Side::Bottom, Side::Top)),
            (Axis::Vertical, (Side::Top, Side::Bottom)),
            (Axis::Horizontal, (Side::Right, Side::Left)),
            (Axis::Horizontal, (Side::Left, Side::Right)),
        ] {
            let sign = if matches!(sides.0, Side::Top | Side::Left) {
                -1.0
            } else {
                1.0
            };
            let point = |along, across| match axis {
                Axis::Vertical => v(across, sign * along),
                Axis::Horizontal => h(sign * along, across),
            };
            let progress = |p: Point| match axis {
                Axis::Vertical => sign * p.y,
                Axis::Horizontal => sign * p.x,
            };
            let mut previous = 0.48;
            for ratio in [0.5, 1.0, 2.0, 4.0, 8.0, 20.0] {
                let marker = Marker::default();
                let points = [point(0.0, 0.0), point(112.0, ratio * 100.0)];
                let painted = build_svg_path(
                    &points,
                    sides,
                    RouteStyle::Curved,
                    (marker, marker),
                    CurveTerminals::Fixed,
                    &[],
                    None,
                )
                .unwrap();
                let [PathSeg::Line(_), PathSeg::Cubic(curve), PathSeg::Line(_)] =
                    segments(&painted)[..]
                else {
                    panic!("{}", painted.d);
                };
                let [start, first, second, end] =
                    [curve.p0, curve.p1, curve.p2, curve.p3].map(progress);
                let fraction = (first - start) / (end - start);
                assert!(first <= second, "{sides:?}, ratio {ratio}: {}", painted.d);
                assert!((0.25 - 1e-9..=previous + 1e-9).contains(&fraction));
                previous = fraction;
            }
        }
    }

    #[test]
    fn rounded_quarters_share_short_legs_and_wide_turns_use_both_handles() {
        let points = [v(0.0, 0.0), v(0.0, 30.0), h(16.0, 30.0), v(16.0, 90.0)];
        let m = Marker::default();
        let paint = |style| {
            build_svg_path(
                &points,
                (Side::Bottom, Side::Top),
                style,
                (m, m),
                CurveTerminals::Fixed,
                &[],
                None,
            )
            .unwrap()
        };
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
    fn a_captioned_step_between_borders_uses_one_clear_crossbar() {
        use kurbo::Rect;

        let points = [
            v(0., 0.),
            v(0., 10.),
            h(20., 10.),
            v(20., 45.),
            v(36., 75.),
            v(36., 90.),
        ];
        let unblocked = Clearance::default();
        let blocked = Clearance::new(vec![(
            Rect::new(4., 28., 12., 36.),
            ObstacleOwner::GroupTitle(4),
        )])
        .unwrap();
        fn guard(clearance: &Clearance) -> EdgeClearance<'_> {
            clearance
                .for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 0.)
                .with_label_bounds(Some(Rect::new(16., 28., 24., 36.)))
        }
        let mut pen = Pen::new(position(points[0]), true, false, None);
        draw_section(
            &mut pen,
            &points,
            (None, None),
            RouteStyle::StepAfter,
            &mut Vec::new(),
            &mut Vec::new(),
            Some(guard(&unblocked)),
        )
        .unwrap();
        pen.flush_line();
        assert_eq!(pen.finish().unwrap().d, "M0,0L0,32L36,32L36,90");
        assert!(caption_crossbar(&points, guard(&blocked)).is_none());
    }

    #[test]
    fn steps_move_only_the_internal_crossbar_and_keep_u_turns_outside() {
        let m = marker(4.0, 10.0);
        let paint = |p: &[RoutePoint], sides, style| {
            build_svg_path(p, sides, style, (m, m), CurveTerminals::Fixed, &[], None).unwrap()
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

        // Moving a step crossbar must be declined when the relocated shaft would hit an obstacle
        // or when the section starts/ends at a compound border stitch.
        let obstacle = Clearance::new(vec![(
            kurbo::Rect::new(-10.0, 55.0, 10.0, 75.0),
            ObstacleOwner::Node(99),
        )])
        .unwrap();
        let blocked = build_svg_path(
            &z,
            sides,
            RouteStyle::StepAfter,
            (m, m),
            CurveTerminals::Fixed,
            &[],
            Some(obstacle.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.0)),
        )
        .unwrap();
        assert!(
            segments(&blocked)
                .into_iter()
                .any(|s| s.start() == Point::new(0.0, 50.0) && s.end() == Point::new(100.0, 50.0)),
            "StepAfter must keep Shoal's clear y=50 crossbar when y=84 hits an obstacle: {}",
            blocked.d
        );
        let stitched_route = [
            v(0.0, 0.0),
            v(0.0, 20.0),
            v(0.0, 40.0),
            h(100.0, 40.0),
            v(100.0, 100.0),
        ];
        let stitch = BorderStitch {
            edge: 0,
            point: 1,
            group: 0,
            side: Side::Bottom,
        };
        let stitched = build_svg_path(
            &stitched_route,
            sides,
            RouteStyle::StepAfter,
            (m, m),
            CurveTerminals::Fixed,
            &[stitch],
            None,
        )
        .unwrap();
        assert!(
            segments(&stitched)
                .into_iter()
                .any(|s| s.start() == Point::new(0.0, 40.0) && s.end() == Point::new(100.0, 40.0)),
            "StepAfter must preserve stitched compound corridor crossbars: {}",
            stitched.d
        );
    }

    #[test]
    fn straight_route_styles_detour_around_blocked_offset_legs() {
        let points = [v(0.0, 0.0), v(100.0, 100.0)];
        let tree = Clearance::new(vec![(
            kurbo::Rect::new(45.0, 12.0, 55.0, 88.0),
            ObstacleOwner::GroupTitle(0),
        )])
        .unwrap();
        let guard = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.0);
        for style in [
            RouteStyle::Step,
            RouteStyle::StepBefore,
            RouteStyle::StepAfter,
            RouteStyle::Linear,
        ] {
            let route = build_svg_path(
                &points,
                (Side::Bottom, Side::Top),
                style,
                (marker(4.0, 10.0), marker(4.0, 10.0)),
                CurveTerminals::Fixed,
                &[],
                Some(guard),
            )
            .unwrap();
            let segments = segments(&route);
            assert!(
                segments.iter().all(|segment| {
                    matches!(segment, PathSeg::Line(_))
                        && (!style.stepped() || aligned(segment.start(), segment.end()))
                        && !guard.blocks_line(segment.start(), segment.end())
                }),
                "{style:?}: {}",
                route.d
            );
            assert_eq!(segments[0].start(), Point::new(0.0, 4.0));
            assert_eq!(segments[0].end(), Point::new(0.0, 14.0));
            assert_eq!(segments.last().unwrap().start(), Point::new(100.0, 86.0));
            assert_eq!(segments.last().unwrap().end(), Point::new(100.0, 96.0));
        }
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
            CurveTerminals::Fixed,
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
                CurveTerminals::Fixed,
                &[],
                None,
            )
        };
        let path = paint(&[h(0.0, 0.0), h(18.0, 0.0), h(26.0, 0.0)]).unwrap();
        assert_eq!(path.d, "M3,0L23,0");
        assert!(paint(&[]).is_err());
        assert!(paint(&[h(0.0, 0.0); 2]).is_err());
        assert!(paint(&[h(0.0, 0.0), h(f64::NAN, 1.0)]).is_err());
        assert!(paint(&[h(0.0, 0.0), h(5.0, 0.0)]).is_err());
        assert!(paint(&[h(0.0, 0.0), h(19.0, 0.0)]).is_err());
        assert!(paint(&[h(-f64::MAX, 2.0), h(f64::MAX, 1.0)]).is_err());
    }

    #[test]
    fn route_paint_scratch_reuses_both_buffers_after_an_invalid_route() {
        let paint = |points: &[RoutePoint], scratch: &mut RoutePaintScratch| {
            build_svg_path_with_scratch(
                points,
                (Side::Bottom, Side::Top),
                RouteStyle::Curved,
                (marker(3.0, 12.0), marker(3.0, 12.0)),
                CurveTerminals::Smooth,
                &[],
                None,
                scratch,
            )
        };
        let mut scratch = RoutePaintScratch::default();
        let points = [v(0.0, 0.0), v(24.0, 70.0)];
        let expected = paint(&points, &mut scratch).unwrap();
        let buffers = (scratch.knots.as_ptr(), scratch.body.as_ptr());
        assert!(matches!(
            paint(&[points[0], v(f64::NAN, 70.0)], &mut scratch),
            Err(RoutePaintError::InvalidGeometry)
        ));
        let actual = paint(&points, &mut scratch).unwrap();
        assert_eq!(actual.d, expected.d);
        assert_eq!(actual.bounds, expected.bounds);
        assert_eq!(buffers, (scratch.knots.as_ptr(), scratch.body.as_ptr()));
    }

    #[test]
    fn straight_path_capacity_depends_on_simplified_points() {
        let paint = |points: &[RoutePoint]| {
            build_svg_path(
                points,
                (Side::Right, Side::Left),
                RouteStyle::Curved,
                (Marker::default(), Marker::default()),
                CurveTerminals::Smooth,
                &[],
                None,
            )
            .unwrap()
            .d
        };
        let points: Vec<_> = (0..4096).map(|x| h(f64::from(x), 0.0)).collect();
        let short = paint(&[points[0], points[4095]]);
        let long = paint(&points);
        assert_eq!(long, "M0,0L4095,0");
        assert_eq!(long.capacity(), short.capacity());
    }

    #[test]
    fn step_expansion_reuses_the_knot_and_body_buffers() {
        let points = [h(0.0, 0.0), h(100.0, 80.0)];
        let paint = |scratch: &mut RoutePaintScratch| {
            build_svg_path_with_scratch(
                &points,
                (Side::Right, Side::Top),
                RouteStyle::StepBefore,
                (marker(3.0, 12.0), marker(4.0, 12.0)),
                CurveTerminals::Fixed,
                &[],
                None,
                scratch,
            )
            .unwrap()
        };
        let buffers = |scratch: &RoutePaintScratch| {
            let mut pointers = [scratch.knots.as_ptr(), scratch.body.as_ptr()];
            pointers.sort_unstable();
            pointers
        };
        let mut scratch = RoutePaintScratch::default();
        let expected = paint(&mut scratch);
        paint(&mut scratch);
        let pointers = buffers(&scratch);
        let actual = paint(&mut scratch);
        assert_eq!(actual.d, expected.d);
        assert_eq!(actual.bounds, expected.bounds);
        assert_eq!(buffers(&scratch), pointers);
    }

    #[test]
    fn recorded_segments_match_the_serialized_shaft() {
        let route = [h(0., 0.), h(48., 0.), v(48., 64.), h(100., 64.)];
        let mut scratch = RoutePaintScratch::default();
        for style in [
            RouteStyle::Linear,
            RouteStyle::Curved,
            RouteStyle::Rounded(5.0),
            RouteStyle::StepAfter,
        ] {
            scratch.record_segments();
            let painted = build_svg_path_with_scratch(
                &route,
                (Side::Right, Side::Left),
                style,
                (marker(3., 12.), marker(3., 12.)),
                CurveTerminals::Fixed,
                &[],
                None,
                &mut scratch,
            )
            .unwrap();
            let parsed = segments(&painted);
            assert_eq!(parsed.len(), scratch.segments().len(), "{style:?}");
            for (expected, actual) in parsed.iter().zip(scratch.segments()) {
                for t in [0., 0.25, 0.5, 0.75, 1.] {
                    assert!(
                        (expected.eval(t) - actual.eval(t)).hypot() < 1e-9,
                        "{style:?}"
                    );
                }
            }
        }
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
                CurveTerminals::Fixed,
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
        let mut p = Pen::new(Point::new(-1.0, 0.0), false, false, None);
        p.start_path(2, RouteStyle::Curved);
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

    #[test]
    fn self_loop_with_one_ulp_drift_keeps_both_curved_and_rounded_corners() {
        let base_x = 37.78_f64;
        let drifted_x = base_x - 7.105_427_357_601_002e-15_f64;
        let points = [
            h(70.4, 104.25),
            h(base_x, 104.25),
            v(drifted_x, 126.75),
            h(70.4, 126.75),
        ];
        let markers = (Marker::default(), marker(4.0, 12.6194));

        let curved = build_svg_path(
            &points,
            (Side::Left, Side::Left),
            RouteStyle::Curved,
            markers,
            CurveTerminals::Smooth,
            &[],
            None,
        )
        .unwrap();
        let cubic_count = segments(&curved)
            .into_iter()
            .filter(|seg| matches!(seg, PathSeg::Cubic(_)))
            .count();
        assert!(
            cubic_count >= 2,
            "curved self-loop must curve both corners despite 1-ULP drift: {}",
            curved.d
        );

        let rounded = build_svg_path(
            &points,
            (Side::Left, Side::Left),
            RouteStyle::Rounded(6.0),
            markers,
            CurveTerminals::Fixed,
            &[],
            None,
        )
        .unwrap();
        assert!(
            rounded.d.matches('A').count() >= 2,
            "rounded self-loop must emit both corner arcs despite 1-ULP drift: {}",
            rounded.d
        );
    }

    #[test]
    fn shared_shelf_midpoint_emits_no_sub_epsilon_line_micro_segments() {
        let points = [
            v(92.985_f64, 40.0),
            v(92.985_f64, 89.61942946412395),
            h(60.419_f64, 89.61942946412395),
            v(60.419_f64, 166.61942946412395),
            h(92.985_f64, 166.61942946412395),
            v(92.985_f64, 210.0),
        ];
        for style in [RouteStyle::Curved, RouteStyle::Rounded(10.0)] {
            let painted = build_svg_path(
                &points,
                (Side::Bottom, Side::Top),
                style,
                (Marker::default(), marker(4.0, 12.6194)),
                CurveTerminals::Smooth,
                &[],
                None,
            )
            .unwrap();
            for seg in segments(&painted) {
                if let PathSeg::Line(line) = seg {
                    let len = (line.p1 - line.p0).hypot();
                    assert!(
                        len > 1e-6,
                        "emitted degenerate {len:.3e} line micro-segment in {style:?}: {}",
                        painted.d
                    );
                }
            }
        }
    }
}
