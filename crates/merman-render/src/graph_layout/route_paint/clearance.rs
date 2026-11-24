use super::super::ir::Endpoint;
use super::RoutePaintError;
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, Rect, Vec2};
use std::collections::VecDeque;
use std::sync::Arc;

const DETOUR_WORK_LIMIT: usize = 4096;

pub(super) struct DetourBudget(usize);

impl Default for DetourBudget {
    fn default() -> Self {
        Self(DETOUR_WORK_LIMIT)
    }
}

impl DetourBudget {
    fn visit(&mut self) -> Result<(), RoutePaintError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(RoutePaintError::DetourBudgetExceeded)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObstacleOwner {
    Node(u32),
    GroupTitle(u32),
    EdgeLabel(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Clearance {
    tree: Arc<Tree>,
}

#[derive(Debug, Default, PartialEq)]
struct Tree {
    boxes: Vec<(Rect, ObstacleOwner)>,
    branches: Vec<Branch>,
}

#[derive(Debug, PartialEq)]
struct Branch {
    bounds: Rect,
    end: usize,
    boxes: std::ops::Range<usize>,
}

impl Tree {
    fn partition(&mut self, first: usize, last: usize) {
        let entries = &mut self.boxes[first..last];
        let bounds = entries
            .iter()
            .fold(entries[0].0, |bounds, (rect, _)| bounds.union(*rect));
        let branch = self.branches.len();
        self.branches.push(Branch {
            bounds,
            end: branch + 1,
            boxes: first..last,
        });
        if entries.len() > 8 {
            let horizontal = bounds.width() >= bounds.height();
            let half = entries.len() / 2;
            let key = |rect: Rect| {
                if horizontal {
                    rect.x0.midpoint(rect.x1)
                } else {
                    rect.y0.midpoint(rect.y1)
                }
            };
            entries.select_nth_unstable_by(half, |a, b| key(a.0).total_cmp(&key(b.0)));
            self.branches[branch].boxes = first..first;
            self.partition(first, first + half);
            self.partition(first + half, last);
            self.branches[branch].end = self.branches.len();
        }
    }
}

impl Clearance {
    pub(crate) fn new(mut boxes: Vec<(Rect, ObstacleOwner)>) -> Self {
        boxes.retain(|(r, _)| r.is_finite() && r.x0 < r.x1 && r.y0 < r.y1);
        let mut tree = Tree {
            boxes,
            branches: Vec::new(),
        };
        if !tree.boxes.is_empty() {
            tree.partition(0, tree.boxes.len());
        }
        Self {
            tree: Arc::new(tree),
        }
    }

    pub(crate) fn for_edge(
        &self,
        edge: usize,
        endpoints: (Endpoint, Endpoint),
        paint_to_layout: Vec2,
        stroke_outset: f64,
    ) -> EdgeClearance<'_> {
        EdgeClearance {
            tree: &self.tree,
            edge,
            endpoints,
            paint_to_layout,
            stroke_outset,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeClearance<'a> {
    tree: &'a Tree,
    edge: usize,
    endpoints: (Endpoint, Endpoint),
    paint_to_layout: Vec2,
    stroke_outset: f64,
}

impl EdgeClearance<'_> {
    fn blocked(self, bounds: Rect, enters: impl FnMut(Rect) -> bool) -> bool {
        self.query(bounds, None, |_| true, enters)
            .expect("no detour budget is used for a clearance check")
    }

    fn query(
        self,
        bounds: Rect,
        mut budget: Option<&mut DetourBudget>,
        mut near_curve: impl FnMut(Rect) -> bool,
        mut enters: impl FnMut(Rect) -> bool,
    ) -> Result<bool, RoutePaintError> {
        let outset = self.stroke_outset;
        let query = (bounds + self.paint_to_layout).inflate(outset, outset);
        let mut at = 0;
        while let Some(branch) = self.tree.branches.get(at) {
            if let Some(budget) = budget.as_deref_mut() {
                budget.visit()?;
            }
            if !overlaps(query, branch.bounds) || !near_curve(branch.bounds) {
                at = branch.end;
                continue;
            }
            for &(rect, owner) in &self.tree.boxes[branch.boxes.clone()] {
                if let Some(budget) = budget.as_deref_mut() {
                    budget.visit()?;
                }
                let own = match owner {
                    ObstacleOwner::Node(id) => {
                        self.endpoints.0 == Endpoint::Node(id)
                            || self.endpoints.1 == Endpoint::Node(id)
                    }
                    ObstacleOwner::EdgeLabel(edge) => edge == self.edge,
                    ObstacleOwner::GroupTitle(_) => false,
                };
                if !own
                    && overlaps(rect, query)
                    && near_curve(rect)
                    && enters((rect - self.paint_to_layout).inflate(outset, outset))
                {
                    return Ok(true);
                }
            }
            at += 1;
        }
        Ok(false)
    }

    pub(super) fn blocks_cubic(self, curve: CubicBez) -> bool {
        self.query_cubic(curve, |rect| cubic_enters(curve, rect))
    }

    fn query_cubic(self, curve: CubicBez, enters: impl FnMut(Rect) -> bool) -> bool {
        let bounds = control_bounds(curve);
        let mut sections = None;
        self.query(
            bounds,
            None,
            |rect| {
                sections
                    .get_or_insert_with(|| self.cubic_hulls(curve, bounds))
                    .iter()
                    .any(|&hull| !hull.is_finite() || overlaps(hull, rect))
            },
            enters,
        )
        .expect("no detour budget is used for a clearance check")
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
        [0.0..0.0625, 0.0625..0.9375, 0.9375..1.0].map(|range| {
            (control_bounds(curve.subsegment(range)) + self.paint_to_layout)
                .inflate(padding, padding)
        })
    }

    pub(super) fn blocks_line(self, from: Point, to: Point) -> bool {
        self.blocked(Rect::from_points(from, to), |rect| {
            line_enters(from, to, rect)
        })
    }

    fn blocks_detour_leg(
        self,
        from: Point,
        to: Point,
        budget: &mut DetourBudget,
    ) -> Result<bool, RoutePaintError> {
        budget.visit()?;
        self.query(
            Rect::from_points(from, to),
            Some(budget),
            |_| true,
            |rect| line_enters(from, to, rect),
        )
    }

    pub(super) fn detour(
        self,
        from: Point,
        to: Point,
        budget: &mut DetourBudget,
    ) -> Result<Vec<Point>, RoutePaintError> {
        for elbow in [Point::new(from.x, to.y), Point::new(to.x, from.y)] {
            if !self.blocks_detour_leg(from, elbow, budget)?
                && !self.blocks_detour_leg(elbow, to, budget)?
            {
                return Ok(vec![elbow, to]);
            }
        }
        let mut bounds = Rect::from_points(from, to);
        let mut points = Vec::new();
        loop {
            points.clear();
            points.push(from);
            let mut expanded = bounds;
            let occupied = self.query(
                bounds,
                Some(budget),
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
            if let Some(path) = self.visible_path(&points, to, budget)? {
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
        budget: &mut DetourBudget,
    ) -> Result<Option<Vec<Point>>, RoutePaintError> {
        let mut pending = VecDeque::from([0]);
        let mut parents = vec![None; points.len()];
        parents[0] = Some(0);
        while let Some(index) = pending.pop_front() {
            let from = points[index];
            if !self.blocks_detour_leg(from, to, budget)? {
                let mut path = vec![to];
                let mut current = index;
                while current != 0 {
                    path.push(points[current]);
                    current = parents[current].expect("queued detour points have a parent");
                }
                path.reverse();
                return Ok(Some(path));
            }
            for (next, parent) in parents.iter_mut().enumerate().skip(1) {
                budget.visit()?;
                if parent.is_none() && !self.blocks_detour_leg(from, points[next], budget)? {
                    *parent = Some(index);
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

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

fn control_bounds(curve: CubicBez) -> Rect {
    Rect::from_points(curve.p0, curve.p3)
        .union_pt(curve.p1)
        .union_pt(curve.p2)
}

fn line_enters(from: Point, to: Point, rect: Rect) -> bool {
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

fn cubic_enters(curve: CubicBez, rect: Rect) -> bool {
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
        );
        let guard = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), Vec2::ZERO, 1.);
        let mut budget = DetourBudget::default();
        let from = Point::new(20., 0.);
        let to = Point::new(120., 60.);
        assert_eq!(
            guard.detour(from, to, &mut budget),
            Err(RoutePaintError::DetourBudgetExceeded),
        );
        assert_eq!(budget.0, 0);
        assert_eq!(
            guard.detour(from, to, &mut budget),
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
        let tree = Clearance::new(boxes.clone());
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
        let tree = Clearance::new(nodes.clone());
        let route = tree.for_edge(0, (Endpoint::Node(0), Endpoint::Node(1)), offset, 1.);
        let mut broad = 0;
        let mut narrow = 0;
        route.blocked(control_bounds(curve), |_| {
            broad += 1;
            false
        });
        route.query_cubic(curve, |_| {
            narrow += 1;
            false
        });
        assert!(
            broad > 200 && narrow < 10,
            "candidate boxes {broad} → {narrow}"
        );

        for curve in [
            curve,
            CubicBez::new(curve.p3, curve.p2, curve.p1, curve.p0),
            CubicBez::new((0., 47.), (1_000., 31.), (400., 92.), (1_200., 48.)),
        ] {
            for (rect, owner) in
                nodes
                    .iter()
                    .copied()
                    .chain([0.0, 0.03, 0.0625, 0.5, 0.9375, 0.97, 1.0].map(|t| {
                        let point = curve.eval(t) + offset;
                        (
                            Rect::from_center_size(point, (1., 1.)),
                            ObstacleOwner::GroupTitle(0),
                        )
                    }))
            {
                let tree = Clearance::new(vec![(rect, owner)]);
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
