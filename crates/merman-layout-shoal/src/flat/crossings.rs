//! Count distinct pairs of crossing routing centerlines before interpolation by a painter.
use super::*;
use crate::RoutePoint;
use std::collections::BTreeSet;

struct Segment {
    a: RoutePoint,
    b: RoutePoint,
    edge: u32,
}
impl Segment {
    fn interval(&self, horizontal: bool) -> (f64, f64) {
        let (a, b) = if horizontal {
            (self.a.x, self.b.x)
        } else {
            (self.a.y, self.b.y)
        };
        (a.min(b), a.max(b))
    }
    fn straddles(&self, other: &Segment) -> bool {
        let cross = |p: RoutePoint| {
            (self.b.x - self.a.x) * (p.y - self.a.y) - (self.b.y - self.a.y) * (p.x - self.a.x)
        };
        let a = cross(other.a);
        let b = cross(other.b);
        (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0)
    }
}

pub(super) fn pairs(edges: &[(u32, EdgeGeometry)], work: &mut Work<'_>) -> LayoutResult<u64> {
    if edges.len() < 2 {
        return Ok(0);
    }
    let count = edges
        .iter()
        .try_fold(0usize, |n, (_, e)| {
            n.checked_add(e.points.len().saturating_sub(1))
        })
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count)?;
    let mut segments = crate::work::reserved(count)?;
    for (id, edge) in edges {
        for p in edge.points.windows(2) {
            if p[0].x != p[1].x || p[0].y != p[1].y {
                segments.push(Segment {
                    a: p[0],
                    b: p[1],
                    edge: *id,
                });
            }
        }
    }
    // Pick the axis with the smaller average relative interval width. Chains and wide fans
    // then have small active fronts in their natural drawing orientation.
    let crowding = |axis| {
        let mut length = 0.0;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for s in &segments {
            let (l, r) = s.interval(axis);
            length += r - l;
            min = min.min(l);
            max = max.max(r);
        }
        if max > min {
            length / (max - min)
        } else {
            f64::INFINITY
        }
    };
    let horizontal = crowding(true) < crowding(false);
    validate::charge_sort(segments.len(), work)?;
    segments.sort_unstable_by(|a, b| {
        a.interval(horizontal)
            .0
            .total_cmp(&b.interval(horizontal).0)
            .then(a.edge.cmp(&b.edge))
    });
    let mut pairs = BTreeSet::new();
    let mut active: Vec<usize> = Vec::new();
    for (index, s) in segments.iter().enumerate() {
        work.visit(active.len() + 1)?;
        let start = s.interval(horizontal).0;
        active.retain(|&i| segments[i].interval(horizontal).1 >= start);
        let (low, high) = s.interval(!horizontal);
        for &i in &active {
            let t = &segments[i];
            let (l, h) = t.interval(!horizontal);
            if s.edge != t.edge && l <= high && h >= low && s.straddles(t) && t.straddles(s) {
                let key = (s.edge.min(t.edge), s.edge.max(t.edge));
                work.visit(1 + pairs.len().checked_ilog2().unwrap_or(0) as usize)?;
                pairs.insert(key);
            }
        }
        crate::work::push(&mut active, index)?;
    }
    Ok(pairs.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, NoopWorkControl};
    fn route(points: &[(f64, f64)]) -> EdgeGeometry {
        EdgeGeometry {
            points: points
                .iter()
                .map(|&(x, y)| RoutePoint {
                    x,
                    y,
                    axis: Axis::Vertical,
                })
                .collect(),
            ports: (Side::Bottom, Side::Top),
            label_center: None,
            start_label_center: None,
            end_label_center: None,
            reversed_for_ranking: false,
        }
    }
    #[test]
    fn counts_each_crossing_pair_once_and_ignores_common_endpoints() {
        let g = [
            (0, route(&[(0.0, 3.0), (8.0, 3.0)])),
            (1, route(&[(2.0, 5.0), (3.0, 0.0), (5.0, 5.0)])),
            (2, route(&[(0.0, 3.0), (-1.0, 6.0)])),
        ];
        assert_eq!(pairs(&g, &mut Work::new(&mut NoopWorkControl)).unwrap(), 1);
    }
}
