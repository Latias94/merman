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

    fn coordinates(&self) -> [u64; 4] {
        [
            self.a.x.to_bits(),
            self.a.y.to_bits(),
            self.b.x.to_bits(),
            self.b.y.to_bits(),
        ]
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

pub(super) fn pairs(
    edges: &[(u32, EdgeGeometry)],
    compress_shared: bool,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
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
    if compress_shared {
        segments.sort_unstable_by(|a, b| {
            a.interval(horizontal)
                .0
                .total_cmp(&b.interval(horizontal).0)
                .then_with(|| a.coordinates().cmp(&b.coordinates()))
                .then(a.edge.cmp(&b.edge))
        });
        work.charge(segments.len())?;
        segments.dedup_by(|a, b| a.edge == b.edge && a.coordinates() == b.coordinates());
        return shared_pairs(&segments, horizontal, work);
    }

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
        let (start, _) = s.interval(horizontal);
        let (low, high) = s.interval(!horizontal);
        retain_with(&mut active, |i| {
            let t = &segments[i];
            let live = t.interval(horizontal).1 >= start;
            if !live {
                return Ok(false);
            }
            let (l, h) = t.interval(!horizontal);
            if s.edge != t.edge && l <= high && h >= low && s.straddles(t) && t.straddles(s) {
                let key = (s.edge.min(t.edge), s.edge.max(t.edge));
                work.visit(1 + pairs.len().checked_ilog2().unwrap_or(0) as usize)?;
                pairs.insert(key);
            }
            Ok(true)
        })?;
        crate::work::push(&mut active, index)?;
    }
    Ok(pairs.len() as u64)
}

fn shared_pairs(segments: &[Segment], horizontal: bool, work: &mut Work<'_>) -> LayoutResult<u64> {
    let mut pairs = BTreeSet::new();
    let mut active: Vec<&[Segment]> = Vec::new();
    for owners in segments.chunk_by(|a, b| a.coordinates() == b.coordinates()) {
        let s = &owners[0];
        work.visit(active.len() + owners.len())?;
        let (start, _) = s.interval(horizontal);
        let (low, high) = s.interval(!horizontal);
        retain_with(&mut active, |neighbors| {
            let t = &neighbors[0];
            let live = t.interval(horizontal).1 >= start;
            if !live {
                return Ok(false);
            }
            let (l, h) = t.interval(!horizontal);
            if !(l <= high && h >= low && s.straddles(t) && t.straddles(s)) {
                return Ok(true);
            }
            for first in owners {
                for second in neighbors {
                    work.visit(1 + pairs.len().checked_ilog2().unwrap_or(0) as usize)?;
                    if first.edge != second.edge {
                        pairs.insert((first.edge.min(second.edge), first.edge.max(second.edge)));
                    }
                }
            }
            Ok(true)
        })?;
        crate::work::push(&mut active, owners)?;
    }
    Ok(pairs.len() as u64)
}

/// Evict expired entries while visiting surviving candidates in their original order.
/// The sweep owns the list and discards it if a visitor's work charge fails.
fn retain_with<T: Copy>(
    active: &mut Vec<T>,
    mut visit: impl FnMut(T) -> LayoutResult<bool>,
) -> LayoutResult<()> {
    let mut kept = 0;
    for index in 0..active.len() {
        let entry = active[index];
        if visit(entry)? {
            if kept != index {
                active[kept] = entry;
            }
            kept += 1;
        }
    }
    active.truncate(kept);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, NoopWorkControl};
    fn route(points: &[(f64, f64)]) -> EdgeGeometry {
        EdgeGeometry {
            points: points
                .iter()
                .map(|&(x, y)| RoutePoint::new(x, y, Axis::Vertical))
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
        for compress_shared in [false, true] {
            assert_eq!(
                pairs(&g, compress_shared, &mut Work::new(&mut NoopWorkControl)).unwrap(),
                1
            );
        }
    }

    #[test]
    fn shared_segments_credit_each_crossing_edge_pair_once() {
        let edges = [
            (0, route(&[(0., 0.), (0., 10.), (0., 0.), (0., 10.)])),
            (1, route(&[(0., 0.), (0., 10.)])),
            (2, route(&[(0., 5.), (0., 10.)])),
            (3, route(&[(-4., 2.), (4., 2.), (4., 7.), (-4., 7.)])),
            (4, route(&[(4., 7.), (-4., 7.)])),
            (5, route(&[(0., 10.), (2., 12.)])),
        ];
        for compress_shared in [false, true] {
            assert_eq!(
                pairs(
                    &edges,
                    compress_shared,
                    &mut Work::new(&mut NoopWorkControl)
                )
                .unwrap(),
                6
            );
        }
    }

    #[test]
    fn shared_trunk_avoids_pairwise_work_and_still_crosses_a_foreign_edge() {
        #[derive(Default)]
        struct Meter(usize);

        impl crate::WorkControl for Meter {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }

            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.0 += units;
                Ok(())
            }
        }

        let count = 512;
        let mut edges: Vec<_> = (0..count)
            .map(|edge| (edge, route(&[(0., 0.), (0., 10.)])))
            .collect();
        edges.push((count, route(&[(-10., 5.), (10., 5.)])));
        let mut legacy = Meter::default();
        let mut shared = Meter::default();
        for (compress_shared, meter) in [(false, &mut legacy), (true, &mut shared)] {
            let mut work = Work::new(meter);
            assert_eq!(
                pairs(&edges, compress_shared, &mut work),
                Ok(u64::from(count))
            );
            work.finish().unwrap();
        }
        assert!(shared.0 * 4 < legacy.0, "{}/{}", shared.0, legacy.0);
    }
}
