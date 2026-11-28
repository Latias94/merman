use super::RoutePaintError;
use crate::route_types::{Endpoint, RoutePoint, Side};
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, Rect, Vec2};
#[cfg(feature = "layout-shoal")]
use merman_layout_shoal::geom::{Bounds, Bvh, BvhEntry};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::ops::ControlFlow::{Break, Continue};
use std::sync::Arc;

const DETOUR_WORK_LIMIT: usize = 4096;

pub(super) struct DetourBudget {
    remaining: usize,
    allow_shortcuts: bool,
}

impl Default for DetourBudget {
    fn default() -> Self {
        Self::new(false)
    }
}

impl DetourBudget {
    pub(super) fn new(allow_shortcuts: bool) -> Self {
        Self {
            remaining: DETOUR_WORK_LIMIT,
            allow_shortcuts,
        }
    }

    pub(super) fn can_try_shortcut(&self) -> bool {
        // Leave most of the shared work for recovering a blocked solved leg.
        self.allow_shortcuts && self.remaining > DETOUR_WORK_LIMIT * 7 / 8
    }

    fn visit_shortcut(&mut self) -> Result<(), RoutePaintError> {
        if !self.can_try_shortcut() {
            return Err(RoutePaintError::DetourBudgetExceeded);
        }
        self.visit()
    }

    fn visit(&mut self) -> Result<(), RoutePaintError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(RoutePaintError::DetourBudgetExceeded)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObstacleOwner {
    Node(u32),
    GroupTitle(u32),
    EdgeLabel(usize),
    Marker(usize, TerminalContact),
}

/// The exact layout-space point where a route meets its node or group contour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalContact {
    endpoint: Endpoint,
    coordinates: [u64; 2],
    side: Option<Side>,
}

impl TerminalContact {
    pub fn new(endpoint: Endpoint, point: RoutePoint) -> Self {
        let coordinate_key = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
        Self {
            endpoint,
            coordinates: [point.x, point.y].map(coordinate_key),
            side: None,
        }
    }

    pub(super) fn on_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    fn same_position(self, other: Self) -> bool {
        self.endpoint == other.endpoint && self.coordinates == other.coordinates
    }

    fn point(self) -> Point {
        let [x, y] = self.coordinates.map(f64::from_bits);
        Point::new(x, y)
    }
}

impl Hash for TerminalContact {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(&self.endpoint).hash(state);
        match self.endpoint {
            Endpoint::Node(id) | Endpoint::Group(id) => id.hash(state),
        }
        self.coordinates.hash(state);
        self.side.map(|side| side as u8).hash(state);
    }
}

#[cfg(feature = "layout-shoal")]
type Obstacles = Bvh<(Rect, ObstacleOwner)>;
#[cfg(not(feature = "layout-shoal"))]
type Obstacles = Vec<(Rect, ObstacleOwner)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clearance {
    obstacles: Option<Arc<Obstacles>>,
}

impl Clearance {
    pub fn new(mut boxes: Vec<(Rect, ObstacleOwner)>) -> Result<Self, RoutePaintError> {
        boxes.retain(|(r, _)| r.is_finite() && r.x0 < r.x1 && r.y0 < r.y1);
        if boxes.is_empty() {
            return Ok(Self::default());
        }
        #[cfg(feature = "layout-shoal")]
        let boxes = Bvh::build(boxes, |(rect, _)| Bounds {
            x: [rect.x0, rect.x1],
            y: [rect.y0, rect.y1],
        })
        .map_err(|_| RoutePaintError::ClearanceAllocationFailed)?;
        Ok(Self {
            obstacles: Some(Arc::new(boxes)),
        })
    }

    pub fn for_edge(
        &self,
        edge: usize,
        endpoints: (Endpoint, Endpoint),
        paint_to_layout: Vec2,
        stroke_outset: f64,
    ) -> EdgeClearance<'_> {
        EdgeClearance {
            obstacles: self.obstacles.as_deref(),
            edge,
            endpoints,
            terminal_contacts: None,
            label_bounds: None,
            has_terminal_labels: false,
            paint_to_layout,
            stroke_outset,
            recovery: Recovery::None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct EdgeClearance<'a> {
    obstacles: Option<&'a Obstacles>,
    edge: usize,
    endpoints: (Endpoint, Endpoint),
    terminal_contacts: Option<[TerminalContact; 2]>,
    pub(super) label_bounds: Option<Rect>,
    has_terminal_labels: bool,
    paint_to_layout: Vec2,
    stroke_outset: f64,
    recovery: Recovery,
}

#[derive(Clone, Copy)]
enum Recovery {
    None,
    TerminalSiblings,
    HardObstacles,
}

impl EdgeClearance<'_> {
    /// Supplies the painted caption envelope for alternate route segments.
    pub fn with_label_bounds(mut self, bounds: Option<Rect>) -> Self {
        self.label_bounds = bounds;
        self
    }

    /// Keeps route shortcuts from moving the shaft away from its solver-placed end captions.
    pub fn with_terminal_labels(mut self, present: bool) -> Self {
        self.has_terminal_labels = present;
        self
    }

    pub(super) fn can_shortcut_sweep(self) -> bool {
        self.label_bounds.is_none() && !self.has_terminal_labels
    }

    pub(super) fn with_terminal_recovery(mut self, sides: (Side, Side)) -> Self {
        self.terminal_contacts = self
            .terminal_contacts
            .map(|[a, b]| [a.on_side(sides.0), b.on_side(sides.1)]);
        self.recovery = Recovery::TerminalSiblings;
        self
    }

    pub(super) fn hard_obstacles_only(mut self) -> Self {
        self.recovery = Recovery::HardObstacles;
        self
    }

    pub(super) fn stroke_outset(self) -> f64 {
        self.stroke_outset
    }

    /// Records the route's contour contacts in the same space as the obstacle inventory.
    pub fn with_terminal_contacts(mut self, contacts: (RoutePoint, RoutePoint)) -> Self {
        self.terminal_contacts = Some([
            TerminalContact::new(self.endpoints.0, contacts.0),
            TerminalContact::new(self.endpoints.1, contacts.1),
        ]);
        self
    }

    fn blocked(self, bounds: Rect, enters: impl FnMut(Rect) -> bool) -> bool {
        self.query(bounds, || Ok(()), |_| true, enters)
            .expect("no detour budget is used for a clearance check")
    }

    fn query(
        self,
        bounds: Rect,
        mut charge_visit: impl FnMut() -> Result<(), RoutePaintError>,
        mut near_curve: impl FnMut(Rect) -> bool,
        mut enters: impl FnMut(Rect) -> bool,
    ) -> Result<bool, RoutePaintError> {
        let Some(obstacles) = self.obstacles else {
            return Ok(false);
        };
        let outset = self.stroke_outset;
        let query = (bounds + self.paint_to_layout).inflate(outset, outset);
        let mut visit = |rect: Rect, owner: Option<ObstacleOwner>| -> Result<_, RoutePaintError> {
            charge_visit()?;
            if !overlaps(rect, query) {
                return Ok(Continue(false));
            }
            let own = owner.is_some_and(|owner| match owner {
                ObstacleOwner::Node(id) => {
                    self.endpoints.0 == Endpoint::Node(id) || self.endpoints.1 == Endpoint::Node(id)
                }
                ObstacleOwner::EdgeLabel(edge) => {
                    edge == self.edge || matches!(self.recovery, Recovery::HardObstacles)
                }
                ObstacleOwner::Marker(edge, contact) => {
                    edge == self.edge
                        || matches!(self.recovery, Recovery::HardObstacles)
                        || self.terminal_contacts.is_some_and(|ends| {
                            ends.into_iter().any(|end| {
                                end.same_position(contact)
                                    || (matches!(self.recovery, Recovery::TerminalSiblings)
                                        && end.endpoint == contact.endpoint
                                        && end.side == contact.side
                                        && contact.side.is_some()
                                        && rect.inflate(outset, outset).contains(end.point()))
                            })
                        })
                }
                ObstacleOwner::GroupTitle(_) => false,
            });
            if own || !near_curve(rect) {
                return Ok(Continue(false));
            }
            if owner.is_some() && enters((rect - self.paint_to_layout).inflate(outset, outset)) {
                return Ok(Break(()));
            }
            Ok(Continue(true))
        };

        #[cfg(feature = "layout-shoal")]
        {
            let result = obstacles.visit(|entry| match entry {
                BvhEntry::Node(bounds) => {
                    let [x0, x1] = bounds.x;
                    let [y0, y1] = bounds.y;
                    visit(Rect::new(x0, y0, x1, y1), None)
                }
                BvhEntry::Item(&(rect, owner)) => visit(rect, Some(owner)),
            })?;
            Ok(result.is_break())
        }
        #[cfg(not(feature = "layout-shoal"))]
        {
            for &(rect, owner) in obstacles {
                if visit(rect, Some(owner))?.is_break() {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }

    pub(super) fn blocks_cubic(self, curve: CubicBez) -> bool {
        self.query_cubic(curve, || Ok(()), |rect| cubic_enters(curve, rect))
            .expect("no detour budget is used for a clearance check")
    }

    fn query_cubic(
        self,
        curve: CubicBez,
        charge_visit: impl FnMut() -> Result<(), RoutePaintError>,
        enters: impl FnMut(Rect) -> bool,
    ) -> Result<bool, RoutePaintError> {
        let bounds = control_bounds(curve);
        let mut sections = None;
        self.query(
            bounds,
            charge_visit,
            |rect| {
                sections
                    .get_or_insert_with(|| self.cubic_hulls(curve, bounds))
                    .iter()
                    .any(|&hull| !hull.is_finite() || overlaps(hull, rect))
            },
            enters,
        )
    }

    pub(super) fn allows_sweep_shortcut(self, curve: CubicBez, budget: &mut DetourBudget) -> bool {
        let (from, to) = (curve.p0, curve.p3);
        let mut charge_visit = || budget.visit_shortcut();
        charge_visit().is_ok()
            && self.query_line(from, to, &mut charge_visit) == Ok(false)
            && (super::aligned(from, to)
                || self.query_cubic(curve, charge_visit, |rect| cubic_enters(curve, rect))
                    == Ok(false))
    }

    fn cubic_hulls(self, curve: CubicBez, bounds: Rect) -> [Rect; 3] {
        // Short end sections separate the terminal rows from the broad middle of a fan curve.
        // Each subcurve stays in its control hull. Pad the hulls for subdivision and translation
        // roundoff so obstacles close to a boundary still reach the exact intersection test.
        let magnitude = bounds
            .x0
            .abs()
            .max(bounds.x1.abs())
            .max(bounds.y0.abs())
            .max(bounds.y1.abs())
            .max(self.paint_to_layout.x.abs())
            .max(self.paint_to_layout.y.abs())
            .max(self.stroke_outset.abs())
            .max(1.0);
        let padding = self.stroke_outset + 64.0 * f64::EPSILON * magnitude;
        let padded = |hull: Rect| (hull + self.paint_to_layout).inflate(padding, padding);
        let monotone = |a, b, c, d| (a <= b && b <= c && c <= d) || (a >= b && b >= c && c >= d);
        if self.stroke_outset >= 0.0
            && bounds.is_finite()
            && monotone(curve.p0.x, curve.p1.x, curve.p2.x, curve.p3.x)
            && monotone(curve.p0.y, curve.p1.y, curve.p2.y, curve.p3.y)
        {
            // Each coordinate derivative is a Bernstein sum of differences with one sign.
            // Endpoints bound each section. Use a shorter end section when its inner
            // cut advances more than the stroke outset on both axes.
            let cut = |endpoint, t, fallback| {
                let point = curve.eval(t);
                let advance: Vec2 = point - endpoint;
                if advance.x.abs().min(advance.y.abs()) > self.stroke_outset {
                    point
                } else {
                    curve.eval(fallback)
                }
            };
            let a = cut(curve.p0, 1.0 / 32.0, 1.0 / 16.0);
            let b = cut(curve.p3, 31.0 / 32.0, 15.0 / 16.0);
            if a.is_finite() && b.is_finite() {
                let hull = |start, end| padded(Rect::from_points(start, end));
                let hulls = [hull(curve.p0, a), hull(a, b), hull(b, curve.p3)];
                if hulls.iter().all(|hull| hull.is_finite()) {
                    return hulls;
                }
            }
        }
        [0.0..0.0625, 0.0625..0.9375, 0.9375..1.0]
            .map(|range| padded(control_bounds(curve.subsegment(range))))
    }

    pub(super) fn blocks_line(self, from: Point, to: Point) -> bool {
        self.query_line(from, to, || Ok(()))
            .expect("no detour budget is used for a clearance check")
    }

    fn query_line(
        self,
        from: Point,
        to: Point,
        charge_visit: impl FnMut() -> Result<(), RoutePaintError>,
    ) -> Result<bool, RoutePaintError> {
        self.query(
            Rect::from_points(from, to),
            charge_visit,
            |_| true,
            |rect| line_enters(from, to, rect),
        )
    }

    fn blocks_detour_leg(
        self,
        from: Point,
        to: Point,
        budget: &mut DetourBudget,
    ) -> Result<bool, RoutePaintError> {
        budget.visit()?;
        self.query_line(from, to, || budget.visit())
    }

    fn clear_elbow(
        self,
        from: Point,
        to: Point,
        budget: &mut DetourBudget,
    ) -> Result<Option<Point>, RoutePaintError> {
        if super::aligned(from, to) {
            return Ok(None);
        }
        for elbow in [Point::new(from.x, to.y), Point::new(to.x, from.y)] {
            if !self.blocks_detour_leg(from, elbow, budget)?
                && !self.blocks_detour_leg(elbow, to, budget)?
            {
                return Ok(Some(elbow));
            }
        }
        Ok(None)
    }

    pub(super) fn detour(
        self,
        from: Point,
        to: Point,
        orthogonal: bool,
        budget: &mut DetourBudget,
    ) -> Result<Vec<Point>, RoutePaintError> {
        if let Some(elbow) = self.clear_elbow(from, to, budget)? {
            return Ok(vec![elbow, to]);
        }
        let mut bounds = Rect::from_points(from, to);
        let mut points = Vec::new();
        loop {
            points.clear();
            points.push(from);
            let mut expanded = bounds;
            let occupied = self.query(
                bounds,
                || budget.visit(),
                |_| true,
                |rect| {
                    if line_enters(from, from, rect) || line_enters(to, to, rect) {
                        return true;
                    }
                    expanded = expanded.union(rect);
                    points.extend([
                        Point::new(rect.x0, rect.y0),
                        Point::new(rect.x0, rect.y1),
                        Point::new(rect.x1, rect.y0),
                        Point::new(rect.x1, rect.y1),
                    ]);
                    false
                },
            )?;
            if occupied {
                return Err(RoutePaintError::NoClearDetour);
            }
            if let Some(path) = self.visible_path(&points, to, orthogonal, budget)? {
                return Ok(path);
            }
            if expanded == bounds {
                return Err(RoutePaintError::NoClearDetour);
            }
            bounds = expanded;
        }
    }

    fn visible_path(
        self,
        points: &[Point],
        to: Point,
        orthogonal: bool,
        budget: &mut DetourBudget,
    ) -> Result<Option<Vec<Point>>, RoutePaintError> {
        // Rectilinear visibility can use either elbow between obstacle corners. Store
        // the chosen elbow with the parent so the final path uses the checked legs.
        let connect = |from, to, budget: &mut _| {
            if orthogonal && !super::aligned(from, to) {
                self.clear_elbow(from, to, budget)
            } else {
                Ok((!self.blocks_detour_leg(from, to, budget)?).then_some(to))
            }
        };
        let mut pending = VecDeque::from([0]);
        let mut parents = vec![None; points.len()];
        while let Some(index) = pending.pop_front() {
            let from = points[index];
            if let Some(elbow) = connect(from, to, budget)? {
                let mut path = vec![to];
                if elbow != to {
                    path.push(elbow);
                }
                let mut current = index;
                while let Some((parent, elbow)) = parents[current] {
                    let point = points[current];
                    path.push(point);
                    if elbow != point {
                        path.push(elbow);
                    }
                    current = parent;
                }
                path.reverse();
                return Ok(Some(path));
            }
            for (next, parent) in parents.iter_mut().enumerate().skip(1) {
                budget.visit()?;
                if parent.is_none()
                    && let Some(elbow) = connect(from, points[next], budget)?
                {
                    *parent = Some((index, elbow));
                    pending.push_back(next);
                }
            }
        }
        Ok(None)
    }

    pub(super) fn blocks_quarter(self, start: Point, corner: Point, end: Point) -> bool {
        let arc = Rect::from_points(start, end);
        let center = start + (end - corner);
        let radius = (start - corner).hypot();
        self.blocked(arc, |obstacle| {
            let clipped = obstacle.intersect(arc);
            if clipped.width() <= 0.0 || clipped.height() <= 0.0 {
                return false;
            }
            let near = Point::new(
                center.x.clamp(clipped.x0, clipped.x1),
                center.y.clamp(clipped.y0, clipped.y1),
            );
            let far = Vec2::new(
                (center.x - clipped.x0)
                    .abs()
                    .max((center.x - clipped.x1).abs()),
                (center.y - clipped.y0)
                    .abs()
                    .max((center.y - clipped.y1).abs()),
            );
            (near - center).hypot() < radius && far.hypot() > radius
        })
    }
}

pub(super) fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

pub(super) fn control_bounds(curve: CubicBez) -> Rect {
    Rect::from_points(curve.p0, curve.p3)
        .union_pt(curve.p1)
        .union_pt(curve.p2)
}

pub(super) fn line_enters(from: Point, to: Point, rect: Rect) -> bool {
    let mut near: f64 = 0.0;
    let mut far: f64 = 1.0;
    for (start, delta, min, max) in [
        (from.x, to.x - from.x, rect.x0, rect.x1),
        (from.y, to.y - from.y, rect.y0, rect.y1),
    ] {
        if delta == 0.0 {
            if start <= min || start >= max {
                return false;
            }
        } else {
            let a = (min - start) / delta;
            let b = (max - start) / delta;
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    near < far
}

pub(super) fn cubic_enters(curve: CubicBez, rect: Rect) -> bool {
    let inside = |p: Point| rect.x0 < p.x && p.x < rect.x1 && rect.y0 < p.y && p.y < rect.y1;
    if inside(curve.p0) || inside(curve.p3) {
        return true;
    }
    let corners = [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ];
    let segment = PathSeg::Cubic(curve);
    let mut breaks = [0.; 14];
    breaks[1] = 1.;
    let mut count = 2;
    for (first, next) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        for hit in segment.intersect_line(Line::new(*first, *next)) {
            if 0. < hit.segment_t && hit.segment_t < 1. {
                breaks[count] = hit.segment_t;
                count += 1;
            }
        }
    }
    breaks[..count].sort_unstable_by(f64::total_cmp);
    breaks[..count]
        .windows(2)
        .any(|t| inside(curve.eval(t[0].midpoint(t[1]))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detour_search_stops_at_the_shared_visit_limit() {
        let middle = Rect::new(45., 10., 95., 50.);
        let walls = [
            Rect::new(116., -2., 124., 6.),
            Rect::new(16., 54., 24., 66.),
        ];
        let tree = Clearance::new(
            std::iter::repeat_n(middle, DETOUR_WORK_LIMIT)
                .chain(walls)
                .map(|rect| (rect, ObstacleOwner::EdgeLabel(1)))
                .collect(),
        )
        .unwrap();
        let guard = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.);
        let mut budget = DetourBudget::default();
        let from = Point::new(20., 0.);
        let to = Point::new(120., 60.);
        assert_eq!(
            guard.detour(from, to, false, &mut budget),
            Err(RoutePaintError::DetourBudgetExceeded),
        );
        assert_eq!(budget.remaining, 0);
        assert_eq!(
            guard.detour(from, to, false, &mut budget),
            Err(RoutePaintError::DetourBudgetExceeded),
        );
    }

    #[test]
    fn nested_spatial_branches_match_brute_force_and_prune_distant_leaves() {
        let boxes: Vec<_> = (0..512)
            .map(|index| {
                let x = (index % 32) as f64 * 30.0;
                let y = (index / 32) as f64 * 20.0;
                (
                    Rect::new(x, y, x + 9., y + 5.),
                    ObstacleOwner::GroupTitle(index),
                )
            })
            .collect();
        let tree = Clearance::new(boxes.clone()).unwrap();
        let route = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 0.);
        for query in [
            Rect::new(5., 0., 8., 3.),
            Rect::new(94., 22., 131., 49.),
            Rect::new(392., 72., 779., 74.),
            Rect::new(1000., 10., 1010., 70.),
        ] {
            let mut actual = 0;
            assert!(!route.blocked(query, |_| {
                actual += 1;
                false
            }));
            assert_eq!(
                actual,
                boxes.iter().filter(|(r, _)| overlaps(query, *r)).count()
            );
        }
        let obstacle = boxes[0].0;
        let distant = CubicBez::new((0., 20.), (0., 12.), (12., 0.), (20., 0.));
        assert!(overlaps(
            obstacle,
            Rect::from_points(distant.p0, distant.p3)
        ));
        assert!(!route.blocks_cubic(distant));
    }

    #[test]
    fn cubic_hulls_prune_wide_fans_without_missing_contacts() {
        let offset = Vec2::new(-20., 300.);
        let curve = CubicBez::new((0., 45.), (0., 60.), (50_000., 63.), (50_000., 78.));
        let nodes: Vec<_> = (0..250)
            .map(|index| {
                let x = f64::from(index) * 200. + 120.;
                (
                    Rect::new(x, 0., x + 152., 45.) + offset,
                    ObstacleOwner::Node(index as u32 + 2),
                )
            })
            .collect();
        let tree = Clearance::new(nodes.clone()).unwrap();
        let route = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), offset, 1.);
        let mut broad = 0;
        let mut narrow = 0;
        route.blocked(control_bounds(curve), |_| {
            broad += 1;
            false
        });
        route
            .query_cubic(
                curve,
                || Ok(()),
                |_| {
                    narrow += 1;
                    false
                },
            )
            .unwrap();
        assert!(
            broad > 200 && narrow < 10,
            "candidate boxes {broad} → {narrow}"
        );
        #[cfg(feature = "layout-shoal")]
        {
            let bounds = control_bounds(curve);
            let hulls = route.cubic_hulls(curve, bounds);
            let mut budget = DetourBudget {
                remaining: 64,
                ..DetourBudget::default()
            };
            assert_eq!(
                route.query(
                    bounds,
                    || budget.visit(),
                    |rect| hulls.iter().any(|&hull| overlaps(hull, rect)),
                    |_| false,
                ),
                Ok(false),
            );
        }

        for curve in [
            curve,
            CubicBez::new(curve.p3, curve.p2, curve.p1, curve.p0),
            CubicBez::new((0., 47.), (1_000., 31.), (400., 92.), (1_200., 48.)),
        ] {
            for (rect, owner) in nodes.iter().copied().chain(
                [0.0, 0.03, 0.03125, 0.0625, 0.5, 0.9375, 0.96875, 0.97, 1.0].map(|t| {
                    let point = curve.eval(t) + offset;
                    (
                        Rect::from_center_size(point, (1., 1.)),
                        ObstacleOwner::GroupTitle(0),
                    )
                }),
            ) {
                let tree = Clearance::new(vec![(rect, owner)]).unwrap();
                let route = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), offset, 1.);
                assert_eq!(
                    route.blocks_cubic(curve),
                    route.blocked(control_bounds(curve), |rect| cubic_enters(curve, rect)),
                    "{curve:?} near {rect:?}",
                );
            }
        }
    }
}
