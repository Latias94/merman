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
use std::convert::Infallible;
use std::fmt::Write as _;

/// Straight shaft beyond the marker setback, in layout units (v13 `shared/svg/route.ts`).
pub use crate::route_types::MIN_MARKER_LEAD as LEAD;
const STEP_INSET: f64 = 12.0;
const TURN_HANDLE: f64 = 0.62;
const TERMINAL_TURN_RADIUS: f64 = 24.0;
const MAX_CORNER_ARM: f64 = TERMINAL_TURN_RADIUS;
const MAX_CORNER_ASPECT: f64 = 3.0;
const QUARTER_CIRCLE_HANDLE: f64 = 0.552_284_749_830_793_6;
#[cfg(feature = "layout-shoal")]
use merman_layout_shoal::STEEP_SWEEP_RATIO;
#[cfg(not(feature = "layout-shoal"))]
const STEEP_SWEEP_RATIO: f64 = 1.6;
const STEEP_SWEEP_MIN_ACROSS: f64 = 48.0;
// Long fan spans keep their distributed curves instead of sharing a flat crossbar.
const STEEP_SWEEP_MAX_ACROSS: f64 = 960.0;
const MIN_TURN_ARM: f64 = 8.0;
const MICRO_JOG_LIMIT: f64 = 8.0;

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
    /// Centerline bounds for the parity SVG, excluding stroke width, markers and effects.
    pub bounds: Bounds,
}

/// A final shaft command in the coordinate space supplied to the painter.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteCommand {
    MoveTo(Point),
    LineTo(Point),
    CubicTo {
        control1: Point,
        control2: Point,
        end: Point,
    },
    QuarterArcTo {
        radius: f64,
        clockwise: bool,
        end: Point,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteCommandError<E> {
    Geometry(RoutePaintError),
    Sink(E),
}

impl<E> From<RoutePaintError> for RouteCommandError<E> {
    fn from(error: RoutePaintError) -> Self {
        Self::Geometry(error)
    }
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

/// Reduce multiple same-direction offsets between fixed borders to one clear turn near the caption.
fn caption_crossbar(
    route: &[RoutePoint],
    style: RouteStyle,
    guard: EdgeClearance<'_>,
) -> Option<[RoutePoint; 4]> {
    let &[first, second, .., penultimate, last] = route else {
        return None;
    };
    let label = guard.label_bounds?;
    if !label.is_finite() || label.width() <= 0.0 || label.height() <= 0.0 {
        return None;
    }
    let vertical = (first.x - second.x).abs() <= ALIGN_EPS;
    let axes = |p: Point| if vertical { (p.y, p.x) } else { (p.x, p.y) };
    let (start_run, start_lane) = axes(position(first));
    let (end_run, end_lane) = axes(position(last));
    let (run, lane) = axes(label.center());
    let inside = |from: f64, value: f64, to: f64| from.min(to) < value && value < from.max(to);
    let caption_half_span = if vertical {
        label.width()
    } else {
        label.height()
    } / 2.0;
    let gap = LEAD + guard.stroke_outset();
    let attaches = |shaft_lane: f64| (shaft_lane - lane).abs() + gap < caption_half_span;
    if !inside(start_run, run, end_run)
        || start_lane == end_lane
        || !(attaches(start_lane) || attaches(end_lane))
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
    let (lo, hi) = if vertical {
        (label.y0, label.y1)
    } else {
        (label.x0, label.x1)
    };
    // Keep a visible lead between the label backplate and the bend. The parallel arm may
    // still meet the caption so its backplate interrupts the shaft across its short axis.
    let levels = if (end_run > start_run) == (style == RouteStyle::StepBefore) {
        [lo - gap, hi + gap]
    } else {
        [hi + gap, lo - gap]
    };
    levels.into_iter().find_map(|level| {
        if !inside(first_turn, level, last_turn) {
            return None;
        }
        let lane_at_caption = if inside(start_run, run, level) {
            start_lane
        } else {
            end_lane
        };
        if !attaches(lane_at_caption) {
            return None;
        }
        let on_plane = |p: RoutePoint| {
            if vertical {
                RoutePoint {
                    y: level,
                    axis,
                    ..p
                }
            } else {
                RoutePoint {
                    x: level,
                    axis,
                    ..p
                }
            }
        };
        let candidate = [first, on_plane(first), on_plane(last), last];
        candidate
            .windows(2)
            .all(|pair| !guard.blocks_line(position(pair[0]), position(pair[1])))
            .then_some(candidate)
    })
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

fn corner_curve(
    corner: Point,
    incoming: Vec2,
    outgoing: Vec2,
    before: f64,
    after: f64,
    handle: f64,
) -> CubicBez {
    let start = corner - incoming * before;
    let end = corner + outgoing * after;
    CubicBez::new(
        start,
        start + incoming * (handle * before),
        end - outgoing * (handle * after),
        end,
    )
}

fn steep_offset(from: Point, end: RoutePoint) -> Option<(f64, f64)> {
    let (run, across) = match end.axis {
        Axis::Vertical => (end.y - from.y, end.x - from.x),
        Axis::Horizontal => (end.x - from.x, end.y - from.y),
    };
    (run.abs() >= 2.0 * MIN_TURN_ARM
        && across.abs() > STEEP_SWEEP_MIN_ACROSS
        && across.abs() <= STEEP_SWEEP_MAX_ACROSS
        && across.abs() > run.abs() * STEEP_SWEEP_RATIO)
        .then_some((run, across))
}

/// A cubic corridor certificate cannot cover an alternate crossbar. Layout bridges use this
/// inexpensive preflight to retain obstacle queries when one could be substituted.
#[doc(hidden)]
pub fn may_use_rounded_crossbar(
    points: &[RoutePoint],
    sides: (Side, Side),
    markers: (Marker, Marker),
    scratch: &mut RoutePaintScratch,
) -> bool {
    let [first, .., last] = points else {
        return false;
    };
    let start = Terminal::new(*first, sides.0, markers.0, true);
    let end = Terminal::new(*last, sides.1, markers.1, true);
    let route = &mut scratch.knots;
    route.clear();
    route.reserve(points.len());
    for &point in points {
        push_knot(route, point);
    }
    if route.len() < 2 {
        return false;
    }
    prepare_body(route, &mut scratch.body, (Some(&start), Some(&end)));
    scratch.body.windows(2).any(|pair| {
        pair[0].kind != KnotKind::ContourDeparture
            && steep_offset(position(pair[0]), pair[1]).is_some()
    })
}

fn steep_crossbar(from: Point, end: RoutePoint, guard: EdgeClearance<'_>) -> Option<[CubicBez; 2]> {
    let to = position(end);
    let (run, span) = steep_offset(from, end)?;
    let (u, v) = match end.axis {
        Axis::Vertical => (Vec2::new(0.0, run.signum()), Vec2::new(span.signum(), 0.0)),
        Axis::Horizontal => (Vec2::new(run.signum(), 0.0), Vec2::new(0.0, span.signum())),
    };
    let cubic_meets_label = |curve, label| {
        clearance::overlaps(clearance::control_bounds(curve), label)
            && clearance::cubic_enters(curve, label)
    };
    let attached_label = guard
        .label_bounds
        .filter(|label| label.is_finite() && label.width() > 0.0 && label.height() > 0.0)
        .filter(|&label| cubic_meets_label(sweep_curve(from, end, false, 1.0), label));
    [0.5_f64, 0.35, 0.65].into_iter().find_map(|fraction| {
        let arm = (run.abs() * fraction.min(1.0 - fraction))
            .min(span.abs() / 2.0)
            .min(MAX_CORNER_ARM);
        let [first, last] = match end.axis {
            Axis::Vertical => [from, to].map(|p| Point::new(p.x, from.y + run * fraction)),
            Axis::Horizontal => [from, to].map(|p| Point::new(from.x + run * fraction, p.y)),
        };
        if arm < MIN_TURN_ARM
            || [from, first, last, to]
                .windows(2)
                .any(|leg| guard.blocks_line(leg[0], leg[1]))
        {
            return None;
        }
        let turns = |scale| {
            let arm = arm * scale;
            [
                corner_curve(first, u, v, arm, arm, QUARTER_CIRCLE_HANDLE),
                corner_curve(last, v, u, arm, arm, QUARTER_CIRCLE_HANDLE),
            ]
        };
        let scale =
            smoothing_scale(|scale| turns(scale).into_iter().any(|c| guard.blocks_cubic(c)))?;
        if arm * scale < MIN_TURN_ARM {
            return None;
        }
        let [a, b] = turns(scale);
        // Off-center candidates can clear the blockers by leaving the host caption behind.
        if attached_label.is_some_and(|label| {
            ![
                Line::new(from, a.p0),
                Line::new(a.p3, b.p0),
                Line::new(b.p3, to),
            ]
            .into_iter()
            .any(|leg| clearance::line_enters(leg.p0, leg.p1, label))
                && !cubic_meets_label(a, label)
                && !cubic_meets_label(b, label)
        }) {
            return None;
        }
        Some([a, b])
    })
}

enum RouteOutput<'a, E> {
    Svg(&'a mut String),
    Commands(&'a mut dyn FnMut(RouteCommand) -> Result<(), E>),
}

impl<E> RouteOutput<'_, E> {
    fn reserve_start(&mut self, points: usize, style: RouteStyle) {
        let Self::Svg(d) = self else {
            return;
        };
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
        let _ = d.try_reserve_exact(capacity);
    }

    fn write_command(&mut self, command: RouteCommand) -> Result<(), RouteCommandError<E>> {
        match self {
            Self::Svg(d) => {
                let _ = match command {
                    RouteCommand::MoveTo(p) => write!(d, "M{},{}", p.x, p.y),
                    RouteCommand::LineTo(p) => write!(d, "L{},{}", p.x, p.y),
                    RouteCommand::CubicTo {
                        control1: a,
                        control2: b,
                        end,
                    } => write!(d, "C{},{} {},{} {},{}", a.x, a.y, b.x, b.y, end.x, end.y),
                    RouteCommand::QuarterArcTo {
                        radius,
                        clockwise,
                        end,
                    } => write!(
                        d,
                        "A{0},{0} 0 0,{1} {2},{3}",
                        number(radius),
                        u8::from(clockwise),
                        end.x,
                        end.y
                    ),
                };
                Ok(())
            }
            Self::Commands(write) => write(command).map_err(RouteCommandError::Sink),
        }
    }
}

struct Pen<'a, 'b, E> {
    output: RouteOutput<'a, E>,
    started: bool,
    bounds: Bounds,
    at: Point,
    finite: bool,
    compact_lines: bool,
    line_start: Option<Point>,
    length: Option<f64>,
    segments: Option<&'a mut Vec<PathSeg>>,
    shaft_allocation_failed: bool,
    detour_budget: &'b mut clearance::DetourBudget,
}

fn svg_point(point: Point) -> Point {
    Point::new(canonicalize_number(point.x), canonicalize_number(point.y))
}

impl<'a, 'b, E> Pen<'a, 'b, E> {
    fn new(
        p: Point,
        compact_lines: bool,
        measure_length: bool,
        output: RouteOutput<'a, E>,
        segments: Option<&'a mut Vec<PathSeg>>,
        detour_budget: &'b mut clearance::DetourBudget,
    ) -> Self {
        let bounds_start = if matches!(&output, RouteOutput::Commands(_)) {
            svg_point(p)
        } else {
            p
        };
        Self {
            output,
            started: false,
            bounds: Bounds {
                min_x: bounds_start.x,
                min_y: bounds_start.y,
                max_x: bounds_start.x,
                max_y: bounds_start.y,
            },
            at: p,
            finite: p.is_finite(),
            compact_lines,
            line_start: None,
            length: measure_length.then_some(0.0),
            segments,
            shaft_allocation_failed: false,
            detour_budget,
        }
    }

    fn start_path(&mut self, points: usize, style: RouteStyle) -> Result<(), RouteCommandError<E>> {
        if !self.started {
            self.output.reserve_start(points, style);
            self.output
                .write_command(RouteCommand::MoveTo(svg_point(self.at)))?;
            self.started = true;
        }
        Ok(())
    }

    fn include(&mut self, p: Point) {
        self.finite &= p.is_finite();
        self.bounds.min_x = self.bounds.min_x.min(p.x);
        self.bounds.min_y = self.bounds.min_y.min(p.y);
        self.bounds.max_x = self.bounds.max_x.max(p.x);
        self.bounds.max_y = self.bounds.max_y.max(p.y);
    }

    fn line(&mut self, p: Point) -> Result<(), RouteCommandError<E>> {
        if (p - self.at).hypot() > ALIGN_EPS {
            if let Some(start) = self.line_start {
                let (previous, next) = (self.at - start, p - self.at);
                let scale = previous.hypot().max(next.hypot());
                if previous.cross(next).abs() > ALIGN_EPS * scale || previous.dot(next) <= 0.0 {
                    self.flush_line()?;
                }
            }
            self.line_start.get_or_insert(self.at);
            // The parity root still uses layout bounds. Typed scenes use the emitted shaft.
            if matches!(&self.output, RouteOutput::Svg(_)) {
                self.include(p);
            } else {
                self.finite &= p.is_finite();
            }
            self.at = p;
            if !self.compact_lines {
                self.flush_line()?;
            }
        }
        Ok(())
    }

    fn flush_line(&mut self) -> Result<(), RouteCommandError<E>> {
        if let Some(start) = self.line_start.take() {
            let (start, end) = (svg_point(start), svg_point(self.at));
            if start == end {
                return Ok(());
            }
            self.output.write_command(RouteCommand::LineTo(end))?;
            self.record_segment(PathSeg::Line(Line::new(start, end)));
            if matches!(&self.output, RouteOutput::Commands(_)) {
                self.include(end);
            }
            if let Some(length) = &mut self.length {
                *length += (end - start).hypot();
            }
        }
        Ok(())
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

    fn cubic(&mut self, a: Point, b: Point, end: Point) -> Result<(), RouteCommandError<E>> {
        if !a.is_finite() || !b.is_finite() || !end.is_finite() {
            self.finite = false;
            return Ok(());
        }
        self.flush_line()?;
        let [from, a, b, emitted_end] = [self.at, a, b, end].map(svg_point);
        self.output.write_command(RouteCommand::CubicTo {
            control1: a,
            control2: b,
            end: emitted_end,
        })?;
        let curve = CubicBez::new(from, a, b, emitted_end);
        self.record_segment(PathSeg::Cubic(curve));
        if let Some(length) = &mut self.length {
            *length += curve.arclen(SVG_LENGTH_ACCURACY);
        }
        let bounds = curve.bounding_box();
        self.include(Point::new(bounds.x0, bounds.y0));
        self.include(Point::new(bounds.x1, bounds.y1));
        self.at = end;
        Ok(())
    }

    fn sweep(
        &mut self,
        end: RoutePoint,
        contour_departure: bool,
        clearance: Option<EdgeClearance<'_>>,
    ) -> Result<(), RouteCommandError<E>> {
        // A layout-certified cubic corridor alone cannot prove an alternative crossbar clear.
        if !contour_departure
            && let Some(turns) = clearance.and_then(|guard| steep_crossbar(self.at, end, guard))
        {
            for turn in turns {
                self.line(turn.p0)?;
                self.cubic(turn.p1, turn.p2, turn.p3)?;
            }
            return self.line(position(end));
        }
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
                return self.line(target);
            }
            curve(scale)
        } else {
            full
        };
        self.cubic(sweep.p1, sweep.p2, sweep.p3)
    }

    fn detour_to(
        &mut self,
        target: Point,
        clearance: EdgeClearance<'_>,
        orthogonal: bool,
    ) -> Result<(), RouteCommandError<E>> {
        for point in clearance.detour(self.at, target, orthogonal, self.detour_budget)? {
            self.line(point)?;
        }
        Ok(())
    }

    fn quarter_arc(
        &mut self,
        radius: f64,
        clockwise: bool,
        end: Point,
    ) -> Result<(), RouteCommandError<E>> {
        self.flush_line()?;
        // The SVG arc parser measures a cubic approximation of its rounded parameters.
        self.length = None;
        let emitted_end = svg_point(end);
        let radius = canonicalize_number(radius);
        self.output.write_command(RouteCommand::QuarterArcTo {
            radius,
            clockwise,
            end: emitted_end,
        })?;
        if self.segments.is_some() {
            let mut from = svg_point(self.at);
            let svg_arc = SvgArc {
                from,
                to: emitted_end,
                radii: Vec2::splat(radius),
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
        self.finite &= end.is_finite();
        self.include(if matches!(&self.output, RouteOutput::Commands(_)) {
            emitted_end
        } else {
            end
        });
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<Bounds, RoutePaintError> {
        if self.shaft_allocation_failed {
            return Err(RoutePaintError::ShaftAllocationFailed);
        }
        if !self.finite {
            return Err(RoutePaintError::InvalidGeometry);
        }
        Ok(self.bounds)
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

struct RightAngle {
    knot: Point,
    incoming: Vec2,
    outgoing: Vec2,
    before: f64,
    after: f64,
    previous_turn: bool,
    next_turn: bool,
}

impl RightAngle {
    fn at(body: &[RoutePoint], i: usize) -> Option<Self> {
        if !right_angle(body, i) {
            return None;
        }
        let knot = position(body[i]);
        let incoming = knot - position(body[i - 1]);
        let outgoing = position(body[i + 1]) - knot;
        let before = incoming.hypot();
        let after = outgoing.hypot();
        let previous_turn = right_angle(body, i - 1);
        let next_turn = right_angle(body, i + 1);
        Some(Self {
            knot,
            incoming: incoming / before,
            outgoing: outgoing / after,
            before: if previous_turn { before * 0.5 } else { before },
            after: if next_turn { after * 0.5 } else { after },
            previous_turn,
            next_turn,
        })
    }

    fn cubic(
        self,
        body: &[RoutePoint],
        i: usize,
        terminals: (Option<&Terminal>, Option<&Terminal>),
    ) -> impl Fn(f64) -> CubicBez {
        let on_normal = |terminal: &Terminal| {
            let delta = self.knot - terminal.tip;
            delta.cross(terminal.normal) == 0.0 && delta.dot(terminal.normal) > 0.0
        };
        let previous_boundary = body[i - 1].kind == KnotKind::GapBoundary;
        let next_boundary = body[i + 1].kind == KnotKind::GapBoundary;
        // Use equal arms when a side terminal turns onto a rail before it leaves the rank.
        let terminal_turn = !self.previous_turn
            && !self.next_turn
            && ((i == 1 && next_boundary && terminals.0.is_some_and(on_normal))
                || (i + 2 == body.len()
                    && previous_boundary
                    && terminals.1.is_some_and(on_normal)));
        let (before, after, handle) = if terminal_turn {
            let arm = self.before.min(self.after).min(TERMINAL_TURN_RADIUS);
            (arm, arm, QUARTER_CIRCLE_HANDLE)
        } else {
            // A long straight corridor meeting a short jog needs similarly sized arms.
            let aspect_cap = self.before.min(self.after) * MAX_CORNER_ASPECT;
            let cap = if previous_boundary || next_boundary {
                aspect_cap
            } else {
                aspect_cap.min(MAX_CORNER_ARM)
            };
            (self.before.min(cap), self.after.min(cap), TURN_HANDLE)
        };
        move |scale| {
            corner_curve(
                self.knot,
                self.incoming,
                self.outgoing,
                before * scale,
                after * scale,
                handle,
            )
        }
    }
}

fn sweep_shortcut(
    from: Point,
    body: &[RoutePoint],
    i: usize,
    terminals: (Option<&Terminal>, Option<&Terminal>),
) -> Option<(CubicBez, usize)> {
    let [join, next, ..] = body.get(i..)? else {
        return None;
    };
    if (from - position(body[i - 1])).hypot() > ALIGN_EPS
        || body[i - 1].kind == KnotKind::ContourDeparture
        || join.kind == KnotKind::ContourDeparture
        || steep_offset(from, *join).is_some()
    {
        return None;
    }
    let along = |delta: Vec2| match join.axis {
        Axis::Horizontal => delta.x,
        Axis::Vertical => delta.y,
    };
    let (end, elbow_join, through) = if let Some(first) = RightAngle::at(body, i + 1) {
        let last = RightAngle::at(body, i + 2)?;
        if next.kind != KnotKind::Free
            || body[i + 2].kind != KnotKind::Free
            || along(first.incoming).abs() < 1.0 - ALIGN_EPS
            || (first.incoming - last.outgoing).hypot() > ALIGN_EPS
        {
            return None;
        }
        let first = first.cubic(body, i + 1, terminals)(1.0);
        let last = last.cubic(body, i + 2, terminals)(1.0);
        // The elbows must consume the entire shelf. This can release a gap boundary
        // at the first join because there is no straight track left to turn onto.
        if (first.p0 - position(*join)).hypot() > ALIGN_EPS
            || (first.p3 - last.p0).hypot() > ALIGN_EPS
        {
            return None;
        }
        (at(last.p3, join.axis), Some(first.p3), i + 2)
    } else {
        if join.kind != KnotKind::Free
            || join.axis != next.axis
            || aligned(position(*join), position(*next))
            || steep_offset(position(*join), *next).is_some()
        {
            return None;
        }
        (*next, None, i + 1)
    };
    let chord = position(end) - from;
    let length = chord.hypot();
    let run = along(chord);
    let mut previous = 0.0;
    let near_chord = |point: Point| {
        let delta = point - from;
        let advance = along(delta) * run.signum();
        let ordered = previous < advance && advance < run.abs();
        previous = advance;
        ordered && chord.cross(delta).abs() < MICRO_JOG_LIMIT * length
    };
    if length.is_finite()
        && std::iter::once(position(*join))
            .chain(elbow_join)
            .all(near_chord)
    {
        Some((sweep_curve(from, end, false, 1.0), through))
    } else {
        None
    }
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

fn draw_body<E>(
    pen: &mut Pen<'_, '_, E>,
    body: &[RoutePoint],
    style: RouteStyle,
    terminals: (Option<&Terminal>, Option<&Terminal>),
    clearance: Option<EdgeClearance<'_>>,
) -> Result<(), RouteCommandError<E>> {
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
                pen.line(p)?;
            }
        }
        return Ok(());
    }
    let shortcut_clearance = clearance.filter(|guard| guard.label_bounds.is_none());
    let mut merged_through = 0;
    for i in 1..body.len() {
        if i <= merged_through {
            continue;
        }
        let p = position(body[i]);
        if matches!(style, RouteStyle::Curved | RouteStyle::Rounded(_))
            && let Some(turn) = RightAngle::at(body, i)
        {
            if let RouteStyle::Rounded(radius) = style {
                let r = radius.min(turn.before).min(turn.after);
                let (u, v) = (turn.incoming, turn.outgoing);
                let r = r * clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| {
                        guard.blocks_quarter(p - u * (r * scale), p, p + v * (r * scale))
                    })
                    .unwrap_or(0.)
                });
                if r > 1e-6 {
                    pen.line(p - u * r)?;
                    pen.quarter_arc(r, u.cross(v) > 0.0, p + v * r)?;
                } else {
                    pen.line(p)?;
                }
            } else {
                let curve = turn.cubic(body, i, terminals);
                let scale = clearance.map_or(1., |guard| {
                    smoothing_scale(|scale| guard.blocks_cubic(curve(scale))).unwrap_or(0.)
                });
                if scale == 0. {
                    pen.line(p)?;
                    continue;
                }
                let safe = curve(scale);
                pen.line(safe.p0)?;
                pen.cubic(safe.p1, safe.p2, safe.p3)?;
            }
        } else if style == RouteStyle::Curved && !aligned(position(body[i - 1]), p) {
            if let Some(guard) = shortcut_clearance
                && pen.detour_budget.can_try_shortcut()
                && let Some((curve, through)) = sweep_shortcut(pen.at, body, i, terminals)
                && guard.allows_sweep_shortcut(
                    CubicBez::new(
                        svg_point(curve.p0),
                        svg_point(curve.p1),
                        svg_point(curve.p2),
                        svg_point(curve.p3),
                    ),
                    pen.detour_budget,
                )
            {
                if aligned(curve.p0, curve.p3) {
                    pen.line(curve.p3)?;
                } else {
                    pen.cubic(curve.p1, curve.p2, curve.p3)?;
                }
                merged_through = through;
            } else {
                pen.sweep(
                    body[i],
                    body[i - 1].kind == KnotKind::ContourDeparture,
                    clearance,
                )?;
            }
        } else {
            pen.line(p)?;
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
    fn new(knot: RoutePoint, side: Side, marker: Marker, smooth: bool) -> Self {
        Self {
            tip: position(knot),
            normal: outward(side),
            marker,
            smooth,
        }
    }

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
    let terminal = Terminal::new(port, side, marker, false);
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
        ObstacleOwner::Marker(edge, TerminalContact::new(endpoint, port).on_side(side)),
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
    RoutePaintRequest {
        points,
        sides,
        style,
        markers,
        curve_terminals,
        stitches,
        clearance,
    }
    .paint_svg(scratch)
}

/// Bound route inputs shared by the SVG formatter and typed command sink.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RoutePaintRequest<'a> {
    pub points: &'a [RoutePoint],
    pub sides: (Side, Side),
    pub style: RouteStyle,
    pub markers: (Marker, Marker),
    pub curve_terminals: CurveTerminals,
    pub stitches: &'a [BorderStitch],
    pub clearance: Option<EdgeClearance<'a>>,
}

impl RoutePaintRequest<'_> {
    pub fn paint_svg(
        self,
        scratch: &mut RoutePaintScratch,
    ) -> Result<PaintedRoute, RoutePaintError> {
        let mut d = String::new();
        let bounds = self
            .paint(scratch, RouteOutput::<Infallible>::Svg(&mut d))
            .map_err(|error| match error {
                RouteCommandError::Geometry(error) => error,
                RouteCommandError::Sink(never) => match never {},
            })?;
        Ok(PaintedRoute { d, bounds })
    }

    /// Paints an already solved route, recovering from an obstructed detour.
    pub fn paint_solved_svg(
        self,
        scratch: &mut RoutePaintScratch,
    ) -> Result<PaintedRoute, RoutePaintError> {
        let mut d = String::new();
        let bounds = self
            .paint_solved(scratch, |request, scratch, smooth, budget| {
                d.clear();
                request.paint_with(
                    scratch,
                    RouteOutput::<Infallible>::Svg(&mut d),
                    smooth,
                    budget,
                )
            })
            .map_err(|error| match error {
                RouteCommandError::Geometry(error) => error,
                RouteCommandError::Sink(never) => match never {},
            })?;
        Ok(PaintedRoute { d, bounds })
    }

    /// Appends one solved route atomically, preserving earlier commands if the sink fails.
    pub fn paint_solved_commands<T, E>(
        self,
        scratch: &mut RoutePaintScratch,
        commands: &mut Vec<T>,
        mut write: impl FnMut(RouteCommand, &mut Vec<T>) -> Result<(), E>,
    ) -> Result<Bounds, RouteCommandError<E>> {
        let start = commands.len();
        let result = self.paint_solved(scratch, |request, scratch, smooth, budget| {
            commands.truncate(start);
            request.paint_with(
                scratch,
                RouteOutput::Commands(&mut |command| write(command, commands)),
                smooth,
                budget,
            )
        });
        if result.is_err() {
            commands.truncate(start);
        }
        result
    }

    fn paint_solved<E>(
        self,
        scratch: &mut RoutePaintScratch,
        mut paint: impl FnMut(
            Self,
            &mut RoutePaintScratch,
            bool,
            &mut clearance::DetourBudget,
        ) -> Result<Bounds, RouteCommandError<E>>,
    ) -> Result<Bounds, RouteCommandError<E>> {
        let measure_length = scratch.measure_length;
        let record_segments = scratch.record_segments;
        let smooth_terminals = self.smooth_terminals();
        let mut budget =
            clearance::DetourBudget::new(cfg!(feature = "layout-shoal") && smooth_terminals);
        let mut attempt = |request| {
            scratch.measure_length = measure_length;
            scratch.record_segments = record_segments;
            paint(request, scratch, smooth_terminals, &mut budget)
        };
        match attempt(self) {
            Err(RouteCommandError::Geometry(RoutePaintError::NoClearDetour)) => {}
            result => return result,
        }
        if let Some(guard) = self.clearance {
            let style = if self.style.stepped() || matches!(self.style, RouteStyle::Rounded(_)) {
                RouteStyle::Step
            } else {
                RouteStyle::Linear
            };
            // The guarded polyline bridges obstructed interior knots. The original terminal
            // runways and border stitches stay fixed, and detours share the original work budget.
            for guard in [
                guard.with_terminal_recovery(self.sides),
                guard.hard_obstacles_only(),
            ] {
                match attempt(Self {
                    style,
                    clearance: Some(guard),
                    ..self
                }) {
                    Err(RouteCommandError::Geometry(RoutePaintError::NoClearDetour)) => {}
                    result => return result,
                }
            }
        }
        attempt(Self {
            style: RouteStyle::Linear,
            clearance: None,
            ..self
        })
    }

    /// Writes the final shaft commands and returns their tight centerline bounds.
    /// The sink may receive a prefix on error. Publish the route only on success.
    pub fn paint_commands<E>(
        self,
        scratch: &mut RoutePaintScratch,
        mut write: impl FnMut(RouteCommand) -> Result<(), E>,
    ) -> Result<Bounds, RouteCommandError<E>> {
        self.paint(scratch, RouteOutput::Commands(&mut write))
    }

    fn smooth_terminals(&self) -> bool {
        self.style == RouteStyle::Curved && matches!(self.curve_terminals, CurveTerminals::Smooth)
    }

    fn paint<'a, E>(
        self,
        scratch: &'a mut RoutePaintScratch,
        output: RouteOutput<'a, E>,
    ) -> Result<Bounds, RouteCommandError<E>> {
        self.paint_with(
            scratch,
            output,
            self.smooth_terminals(),
            &mut clearance::DetourBudget::default(),
        )
    }

    fn paint_with<'a, E>(
        self,
        scratch: &'a mut RoutePaintScratch,
        output: RouteOutput<'a, E>,
        smooth_terminals: bool,
        budget: &mut clearance::DetourBudget,
    ) -> Result<Bounds, RouteCommandError<E>> {
        let Self {
            points,
            sides,
            style,
            markers,
            stitches,
            clearance,
            ..
        } = self;
        let measure_length = std::mem::take(&mut scratch.measure_length);
        scratch.measured_length = None;
        scratch.segments.clear();
        let record_segments = std::mem::take(&mut scratch.record_segments);
        if points.len() < 2
            || [markers.0, markers.1].iter().any(|m| {
                !m.trim.is_finite() || m.trim < 0.0 || !m.depth.is_finite() || m.depth < 0.0
            })
            || matches!(style, RouteStyle::Rounded(radius) if !radius.is_finite())
        {
            return Err(RoutePaintError::InvalidGeometry.into());
        }
        let last = points.len() - 1;
        let start = Terminal::new(points[0], sides.0, markers.0, smooth_terminals);
        let end = Terminal::new(points[last], sides.1, markers.1, smooth_terminals);
        let mut pen = Pen::new(
            start.shaft_end(),
            smooth_terminals && style == RouteStyle::Curved,
            measure_length && !matches!(style, RouteStyle::Rounded(_)),
            output,
            record_segments.then_some(&mut scratch.segments),
            budget,
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
            pen.flush_line()?;
            begin = finish;
        }
        let length = pen.length.filter(|length| length.is_finite());
        let bounds = pen.finish()?;
        scratch.measured_length = length;
        Ok(bounds)
    }
}

fn draw_section<E>(
    pen: &mut Pen<'_, '_, E>,
    points: &[RoutePoint],
    terminals: (Option<&Terminal>, Option<&Terminal>),
    style: RouteStyle,
    route: &mut Vec<RoutePoint>,
    body: &mut Vec<RoutePoint>,
    clearance: Option<EdgeClearance<'_>>,
) -> Result<(), RouteCommandError<E>> {
    route.clear();
    route.reserve(points.len());
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(RoutePaintError::InvalidGeometry.into());
        }
        push_knot(route, point);
    }
    if route.len() < 2 {
        return (terminals.0.is_none() && terminals.1.is_none())
            .then_some(())
            .ok_or(RoutePaintError::InvalidGeometry.into());
    }
    if style.stepped() {
        if style != RouteStyle::Step
            && terminals.0.is_none()
            && terminals.1.is_none()
            && let Some(path) = clearance.and_then(|guard| caption_crossbar(route, style, guard))
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
            return Err(RoutePaintError::InvalidGeometry.into());
        }
        if aligned(start.tip, end.tip)
            && (end.tip - start.tip).dot(start.normal) > 0.0
            && (!(style.stepped() || style == RouteStyle::Linear)
                || clearance.is_none_or(|guard| !guard.blocks_line(pen.at, end.shaft_end())))
        {
            // Opposed markers can share this one straight segment up to its full length.
            if gap < start.lead().max(end.lead()) {
                return Err(RoutePaintError::InvalidGeometry.into());
            }
            pen.start_path(2, RouteStyle::Linear)?;
            return pen.line(end.shaft_end());
        }
    }

    prepare_body(route, body, terminals);
    // With different endpoint sides, inserting a normal shaft can create an offset leg even
    // on a previously orthogonal route. Step renderers must still emit only axis-aligned legs.
    if style.stepped() && has_offset_leg(body) {
        steps(route, style, body, (None, None), clearance);
        std::mem::swap(route, body);
    }
    pen.start_path(body.len(), style)?;
    pen.line(position(body[0]))?;
    draw_body(pen, body, style, terminals, clearance)?;
    if let Some(end) = terminals.1 {
        pen.line(end.shaft_end())?;
    }
    Ok(())
}

fn prepare_body(
    route: &[RoutePoint],
    body: &mut Vec<RoutePoint>,
    terminals: (Option<&Terminal>, Option<&Terminal>),
) {
    let last = route.len() - 1;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_types::Endpoint;
    use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Rect};

    #[test]
    fn serialized_lines_skip_knots_that_round_to_the_same_point() {
        let mut svg = String::new();
        let mut segments = Vec::new();
        let mut budget = clearance::DetourBudget::default();
        let mut pen = Pen::new(
            Point::ORIGIN,
            false,
            true,
            RouteOutput::<Infallible>::Svg(&mut svg),
            Some(&mut segments),
            &mut budget,
        );
        pen.start_path(4, RouteStyle::Step).unwrap();
        pen.line(Point::new(3.0, 0.0)).unwrap();
        pen.line(Point::new(3.0, 0.000_000_5)).unwrap();
        pen.line(Point::new(3.0, 4.0)).unwrap();
        assert_eq!(pen.length, Some(7.0));
        pen.finish().unwrap();
        assert_eq!(svg, "M0,0L3,0L3,4");
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn typed_shaft_bounds_include_the_emitted_line_endpoint() {
        let points = [h(0.0, 0.0), h(15.999_999_5, 0.0)];
        let request = || RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Linear,
            markers: (Marker::default(), Marker::default()),
            curve_terminals: CurveTerminals::Fixed,
            stitches: &[],
            clearance: None,
        };
        let mut scratch = RoutePaintScratch::default();
        scratch.record_segments();
        let mut commands = Vec::new();
        let bounds = request()
            .paint_commands(&mut scratch, |command| {
                commands.push(command);
                Ok::<_, Infallible>(())
            })
            .unwrap();
        let end = Point::new(16.0, 0.0);
        assert_eq!(commands.last(), Some(&RouteCommand::LineTo(end)));
        assert_eq!(scratch.segments().last().unwrap().end(), end);
        assert_eq!(bounds.max_x, end.x);
        let svg = request().paint_svg(&mut scratch).unwrap();
        assert_eq!(svg.d, "M0,0L16,0");
        assert_eq!(svg.bounds.max_x, points[1].x);
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

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn solved_exterior_sweep_keeps_its_stitch_and_terminal_shaft() {
        let points = [
            v(68., 0.),
            v(68., 34.),
            v(68., 41.),
            RoutePoint::gap_boundary(34., 82., Axis::Vertical),
            v(34., 110.),
            h(0., 110.),
            v(0., 152.),
            v(0., 164.),
        ];
        let stitches = [BorderStitch {
            edge: 0,
            point: 1,
            group: 0,
            side: Side::Bottom,
        }];
        let tree = Clearance::default();
        let endpoints = (Endpoint::Node(0), Endpoint::Node(1));
        let offset = Vec2::new(800., -200.);
        let request = RoutePaintRequest {
            points: &points,
            sides: (Side::Bottom, Side::Top),
            style: RouteStyle::Curved,
            markers: (Marker::default(), marker(4., 8.)),
            curve_terminals: CurveTerminals::Smooth,
            stitches: &stitches,
            clearance: Some(tree.for_edge(0, endpoints, offset, 1.)),
        };
        let mut scratch = RoutePaintScratch::default();
        let original = request.paint_svg(&mut scratch).unwrap();
        scratch.measure_length();
        scratch.record_segments();
        let simplified = request.paint_solved_svg(&mut scratch).unwrap();
        let pieces = segments(&simplified);
        assert_eq!(pieces, scratch.segments());
        assert_eq!(
            scratch.measured_length(),
            Some(
                BezPath::from_svg(&simplified.d)
                    .unwrap()
                    .perimeter(SVG_LENGTH_ACCURACY)
            )
        );
        let [
            PathSeg::Line(through_border),
            PathSeg::Line(guard),
            PathSeg::Cubic(sweep),
            PathSeg::Line(shaft),
        ] = pieces.as_slice()
        else {
            panic!("{}", simplified.d);
        };
        assert_eq!(through_border.p1, position(points[1]));
        assert_eq!(guard.p1, position(points[2]));
        assert_eq!(sweep.p0.x, sweep.p1.x);
        assert_eq!(sweep.p2.x, sweep.p3.x);
        assert_eq!(shaft.p0.x, shaft.p1.x);
        assert_eq!(shaft.p1, Point::new(0., 160.));
        assert!(shaft.length() >= 8.);

        for obstacle in [Rect::new(50., 64., 51., 66.), Rect::new(56., 64., 57., 68.)] {
            let tree =
                Clearance::new(vec![(obstacle + offset, ObstacleOwner::GroupTitle(1))]).unwrap();
            let guarded = RoutePaintRequest {
                clearance: Some(tree.for_edge(0, endpoints, offset, 1.)),
                ..request
            }
            .paint_solved_svg(&mut scratch)
            .unwrap();
            assert_eq!(guarded.d, original.d);
            assert!(!samples_inside(&guarded, obstacle.inflate(1., 1.)));
        }
        assert_eq!(
            RoutePaintRequest {
                clearance: None,
                ..request
            }
            .paint_solved_svg(&mut scratch)
            .unwrap()
            .d,
            original.d
        );

        let crowded = Clearance::new(vec![
            (
                Rect::new(30., 70., 39., 100.) + offset,
                ObstacleOwner::EdgeLabel(0),
            );
            600
        ])
        .unwrap();
        let budget_limited = RoutePaintRequest {
            clearance: Some(crowded.for_edge(0, endpoints, offset, 1.)),
            ..request
        };
        assert_eq!(
            budget_limited.paint_solved_svg(&mut scratch).unwrap().d,
            original.d
        );
    }

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn solved_curves_drop_only_small_free_jogs() {
        let tree = Clearance::default();
        let guard = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.);
        let mut scratch = RoutePaintScratch::default();
        for (join, cubics) in [
            (v(6., 50.), 0),
            (v(8., 50.), 2),
            (RoutePoint::gap_boundary(6., 50., Axis::Vertical), 2),
        ] {
            let route = RoutePaintRequest {
                points: &[v(0., 0.), v(0., 20.), join, v(0., 80.), v(0., 100.)],
                sides: (Side::Bottom, Side::Top),
                style: RouteStyle::Curved,
                markers: (marker(3., 8.), marker(4., 8.)),
                curve_terminals: CurveTerminals::Smooth,
                stitches: &[],
                clearance: Some(guard),
            }
            .paint_solved_svg(&mut scratch)
            .unwrap();
            assert_eq!(route.d.matches('C').count(), cubics, "{}", route.d);
            if cubics == 0 {
                assert_eq!(route.d, "M0,3L0,96");
            }
        }
    }

    #[test]
    fn solved_route_recovers_from_a_same_face_sibling_at_its_contact() {
        let points = [h(0., 0.), h(36., 22.), h(100., 22.)];
        let endpoints = (Endpoint::Node(0), Endpoint::Node(1));
        let sibling = Marker {
            trim: 2.,
            depth: 10.,
            width: 14.,
        };
        let body = Rect::new(18., 5., 30., 16.);
        let tree = Clearance::new(vec![
            marker_obstacle(1, endpoints.0, h(0., 3.), Side::Right, sibling).unwrap(),
            (body, ObstacleOwner::Node(2)),
        ])
        .unwrap();
        let request = RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Curved,
            markers: (Marker::default(), marker(2., 9.)),
            curve_terminals: CurveTerminals::Smooth,
            stitches: &[],
            clearance: Some(
                tree.for_edge(0, endpoints, Vec2::ZERO, 1.)
                    .with_terminal_contacts((points[0], points[2])),
            ),
        };
        let mut scratch = RoutePaintScratch::default();
        assert!(matches!(
            request.paint_svg(&mut scratch),
            Err(RoutePaintError::NoClearDetour)
        ));
        scratch.record_segments();
        scratch.measure_length();
        let path = request.paint_solved_svg(&mut scratch).unwrap();
        assert!(!samples_inside(&path, body.inflate(1., 1.)), "{}", path.d);
        assert_eq!(scratch.segments()[0].start(), Point::ORIGIN);
        let last = scratch.segments().last().unwrap();
        assert_eq!(last.start(), Point::new(89., 22.));
        assert_eq!(last.end(), Point::new(98., 22.));
        assert!(scratch.measured_length().is_some_and(|n| n > 98.));
    }

    #[test]
    fn solved_route_skips_a_trapped_knot_and_preserves_the_stitch_and_command_prefix() {
        let points = [h(0., 0.), h(20., 8.), h(60., 25.), h(130., 25.)];
        let stitches = [BorderStitch {
            edge: 0,
            point: 2,
            group: 0,
            side: Side::Right,
        }];
        let body = Rect::new(17., 4., 23., 12.);
        let tree = Clearance::new(vec![(body, ObstacleOwner::Node(2))]).unwrap();
        let request = RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Curved,
            markers: (marker(2., 8.), marker(3., 9.)),
            curve_terminals: CurveTerminals::Smooth,
            stitches: &stitches,
            clearance: Some(tree.for_edge(
                0,
                (Endpoint::Node(0), Endpoint::Node(1)),
                Vec2::ZERO,
                1.,
            )),
        };
        let mut scratch = RoutePaintScratch::default();
        assert!(matches!(
            request.paint_svg(&mut scratch),
            Err(RoutePaintError::NoClearDetour)
        ));
        let path = request.paint_solved_svg(&mut scratch).unwrap();
        assert!(!samples_inside(&path, body.inflate(1., 1.)), "{}", path.d);
        let prefix = RouteCommand::MoveTo(Point::new(-7., -9.));
        let mut commands = vec![prefix];
        request
            .paint_solved_commands(&mut scratch, &mut commands, |command, commands| {
                commands.push(command);
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(commands[0], prefix);
        assert_eq!(commands[1], RouteCommand::MoveTo(Point::new(2., 0.)));
        assert!(commands[2..].contains(&RouteCommand::LineTo(position(points[2]))));
        assert_eq!(
            commands[2..]
                .iter()
                .filter(|c| matches!(c, RouteCommand::MoveTo(_)))
                .count(),
            0
        );
        assert_eq!(
            commands.last(),
            Some(&RouteCommand::LineTo(Point::new(127., 25.)))
        );

        let mut commands = vec![prefix];
        let error =
            request.paint_solved_commands(&mut scratch, &mut commands, |command, commands| {
                commands.push(command);
                Err("output limit")
            });
        assert_eq!(error.unwrap_err(), RouteCommandError::Sink("output limit"));
        assert_eq!(commands, [prefix]);
    }

    #[test]
    fn solved_route_propagates_an_exhausted_detour_budget() {
        let points = [h(0., 0.), h(240., 60.)];
        let obstacle = Rect::new(80., -2., 170., 62.);
        let tree = Clearance::new(vec![(obstacle, ObstacleOwner::EdgeLabel(1)); 5000]).unwrap();
        let request = RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Curved,
            markers: (Marker::default(), Marker::default()),
            curve_terminals: CurveTerminals::Smooth,
            stitches: &[],
            clearance: Some(tree.for_edge(
                0,
                (Endpoint::Node(0), Endpoint::Node(1)),
                Vec2::ZERO,
                1.,
            )),
        };
        assert!(matches!(
            request.paint_solved_svg(&mut RoutePaintScratch::default()),
            Err(RoutePaintError::DetourBudgetExceeded)
        ));
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
    fn a_steep_sweep_uses_a_clear_rounded_crossbar() {
        let points = [h(0., 0.), h(112., 200.)];
        let obstacle = kurbo::Rect::new(50., 80., 62., 120.);
        let clearance = Clearance::new(vec![(obstacle, ObstacleOwner::GroupTitle(0))]).unwrap();
        let paint = |clearance: Option<&Clearance>| {
            build_svg_path(
                &points,
                (Side::Right, Side::Left),
                RouteStyle::Curved,
                (Marker::default(), Marker::default()),
                CurveTerminals::Smooth,
                &[],
                clearance.map(|tree| {
                    tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.)
                }),
            )
            .unwrap()
        };
        let painted = paint(Some(&clearance));
        let [
            PathSeg::Line(start),
            PathSeg::Cubic(first),
            PathSeg::Line(crossbar),
            PathSeg::Cubic(second),
            PathSeg::Line(end),
        ] = segments(&painted)[..]
        else {
            panic!("{}", painted.d);
        };
        assert_eq!(start.p0, Point::ORIGIN);
        assert_eq!(end.p1, Point::new(112., 200.));
        assert_eq!(crossbar.p0.x, crossbar.p1.x);
        assert_eq!((first.p0, first.p3), (start.p1, crossbar.p0));
        assert_eq!((second.p0, second.p3), (crossbar.p1, end.p0));
        assert!(!samples_inside(&painted, obstacle.inflate(1., 1.)));

        let outside = kurbo::Rect::new(15., -2., 20., 2.);
        let blocked = Clearance::new(vec![(outside, ObstacleOwner::GroupTitle(0))]).unwrap();
        let fallback = paint(Some(&blocked));
        assert_eq!(fallback.d, paint(None).d);
        assert!(!samples_inside(&fallback, outside.inflate(1., 1.)));
    }

    #[test]
    fn crossbar_preflight_checks_the_sweep_after_marker_trimming() {
        let sides = (Side::Right, Side::Left);
        let mut scratch = RoutePaintScratch::default();
        let plain = (Marker::default(), Marker::default());
        assert!(!may_use_rounded_crossbar(
            &[h(0., 0.), h(200., 60.)],
            sides,
            plain,
            &mut scratch,
        ));

        let mut points = [h(0., 0.), h(140., 200.)];
        assert!(!may_use_rounded_crossbar(
            &points,
            sides,
            plain,
            &mut scratch
        ));
        let marker = Marker {
            depth: 10.,
            trim: 5.,
            width: 5.,
        };
        assert!(may_use_rounded_crossbar(
            &points,
            sides,
            (marker, marker),
            &mut scratch,
        ));
        points[0].kind = KnotKind::ContourDeparture;
        assert!(!may_use_rounded_crossbar(
            &points,
            sides,
            (marker, marker),
            &mut scratch,
        ));
    }

    #[test]
    fn a_steep_crossbar_keeps_an_attached_caption() {
        let end = h(112., 200.);
        let label = kurbo::Rect::new(50., 96., 62., 104.);
        let title = kurbo::Rect::new(52., 130., 60., 140.);
        let tree = Clearance::new(vec![(title, ObstacleOwner::GroupTitle(0))]).unwrap();
        let guard = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.);
        let sweep = sweep_curve(Point::ORIGIN, end, false, 1.0);
        assert!(clearance::cubic_enters(sweep, label));
        assert!(!guard.blocks_cubic(sweep));
        assert!(steep_crossbar(Point::ORIGIN, end, guard).is_some());
        let captioned = guard.with_label_bounds(Some(label));
        assert!(steep_crossbar(Point::ORIGIN, end, captioned).is_none());
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
        let neighboring_label = (Rect::new(4., 39., 12., 45.), ObstacleOwner::EdgeLabel(1));
        let one_side_blocked = Clearance::new(vec![neighboring_label]).unwrap();
        let both_sides_blocked = Clearance::new(vec![
            neighboring_label,
            (Rect::new(4., 19., 12., 25.), ObstacleOwner::GroupTitle(4)),
        ])
        .unwrap();
        fn guard(clearance: &Clearance) -> EdgeClearance<'_> {
            clearance
                .for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 0.)
                .with_label_bounds(Some(Rect::new(-12., 28., 48., 36.)))
        }
        let mut svg = String::new();
        let mut budget = clearance::DetourBudget::default();
        let mut pen = Pen::new(
            position(points[0]),
            true,
            false,
            RouteOutput::<Infallible>::Svg(&mut svg),
            None,
            &mut budget,
        );
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
        pen.flush_line().unwrap();
        pen.finish().unwrap();
        assert_eq!(svg, "M0,0L0,42L36,42L36,90");
        let fallback =
            caption_crossbar(&points, RouteStyle::StepAfter, guard(&one_side_blocked)).unwrap();
        assert_eq!((fallback[1].y, fallback[2].y), (22.0, 22.0));
        assert!(
            caption_crossbar(&points, RouteStyle::StepBefore, guard(&both_sides_blocked)).is_none()
        );
        let target_lane_only =
            guard(&unblocked).with_label_bounds(Some(Rect::new(29., 28., 43., 36.)));
        let attached = caption_crossbar(&points, RouteStyle::StepAfter, target_lane_only).unwrap();
        assert_eq!((attached[1].y, attached[2].y), (22.0, 22.0));

        for label in [
            Rect::new(f64::NAN, 28., 43., 36.),
            Rect::new(16., 28., 24., 36.),
            Rect::new(22., 28., 37., 36.),
        ] {
            assert!(
                caption_crossbar(
                    &points,
                    RouteStyle::StepAfter,
                    guard(&unblocked).with_label_bounds(Some(label)),
                )
                .is_none()
            );
        }
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
        let points = [
            v(0.0, 0.0),
            v(0.0, 0.0),
            v(0.0, 20.0),
            h(70.0, 45.0),
            h(120.0, 45.0),
            h(120.0, 45.0),
        ];
        let request = || RoutePaintRequest {
            points: &points,
            sides: (Side::Bottom, Side::Left),
            style: RouteStyle::Linear,
            markers: (marker(4.0, 12.0), marker(2.0, 8.0)),
            curve_terminals: CurveTerminals::Fixed,
            stitches: &[],
            clearance: None,
        };
        let mut scratch = RoutePaintScratch::default();
        let mut commands = Vec::new();
        let bounds = request()
            .paint_commands(&mut scratch, |command| {
                commands.push(command);
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(
            commands,
            [
                RouteCommand::MoveTo(Point::new(0.0, 4.0)),
                RouteCommand::LineTo(Point::new(0.0, 16.0)),
                RouteCommand::LineTo(Point::new(0.0, 20.0)),
                RouteCommand::LineTo(Point::new(70.0, 45.0)),
                RouteCommand::LineTo(Point::new(110.0, 45.0)),
                RouteCommand::LineTo(Point::new(118.0, 45.0)),
            ]
        );
        assert_eq!(
            bounds,
            Bounds {
                min_x: 0.0,
                min_y: 4.0,
                max_x: 118.0,
                max_y: 45.0,
            }
        );
        let svg = request().paint_svg(&mut scratch).unwrap();
        assert_eq!(svg.d, "M0,4L0,16L0,20L70,45L110,45L118,45");
        assert_eq!(svg.bounds, bounds);
    }

    #[test]
    fn command_sink_failure_stops_at_the_failed_command_and_resets_scratch() {
        let points = [h(0.0, 0.0), h(20.0, 0.0), h(80.0, 40.0)];
        let request = || RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Linear,
            markers: (Marker::default(), Marker::default()),
            curve_terminals: CurveTerminals::Fixed,
            stitches: &[],
            clearance: None,
        };
        let mut scratch = RoutePaintScratch::default();
        scratch.measure_length();
        let expected = request().paint_svg(&mut scratch).unwrap();
        assert!(scratch.measured_length().is_some());

        scratch.measure_length();
        let mut offered = Vec::new();
        let result = request().paint_commands(&mut scratch, |command| {
            offered.push(command);
            match command {
                RouteCommand::MoveTo(_) => Ok(()),
                _ => Err(command),
            }
        });
        let failed = RouteCommand::LineTo(Point::new(LEAD, 0.0));
        assert_eq!(result, Err(RouteCommandError::Sink(failed)));
        assert_eq!(offered, [RouteCommand::MoveTo(Point::ORIGIN), failed]);
        assert_eq!(scratch.measured_length(), None);

        let recovered = request().paint_svg(&mut scratch).unwrap();
        assert_eq!(recovered.d, expected.d);
        assert_eq!(recovered.bounds, expected.bounds);
        assert_eq!(scratch.measured_length(), None);
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
        let mut write = |_| Ok::<_, Infallible>(());
        let mut budget = clearance::DetourBudget::default();
        let mut p = Pen::new(
            Point::new(-1.0, 0.0),
            false,
            false,
            RouteOutput::Commands(&mut write),
            None,
            &mut budget,
        );
        p.start_path(2, RouteStyle::Curved).unwrap();
        p.cubic(
            Point::new(-1.0, 10.0),
            Point::new(3.0, 10.0),
            Point::new(3.0, 0.0),
        )
        .unwrap();
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
