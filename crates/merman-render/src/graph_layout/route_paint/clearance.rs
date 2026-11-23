use super::super::ir::Endpoint;
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, Rect, Vec2};
use std::sync::Arc;

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
    fn blocked(self, bounds: Rect, mut enters: impl FnMut(Rect) -> bool) -> bool {
        let outset = self.stroke_outset;
        let query = (bounds + self.paint_to_layout).inflate(outset, outset);
        let mut at = 0;
        while let Some(branch) = self.tree.branches.get(at) {
            if !overlaps(query, branch.bounds) {
                at = branch.end;
                continue;
            }
            for &(rect, owner) in &self.tree.boxes[branch.boxes.clone()] {
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
                    && enters((rect - self.paint_to_layout).inflate(outset, outset))
                {
                    return true;
                }
            }
            at += 1;
        }
        false
    }

    pub(super) fn blocks_cubic(self, curve: CubicBez) -> bool {
        let bounds = Rect::from_points(curve.p0, curve.p3)
            .union_pt(curve.p1)
            .union_pt(curve.p2);
        self.blocked(bounds, |rect| cubic_enters(curve, rect))
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
}
