//! Count distinct pairs of crossing routing centerlines before interpolation by a painter.
use super::*;
use crate::RoutePoint;
use std::collections::BTreeSet;

mod index;

const MIN_INDEX_FRONT: usize = 128;

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

#[derive(Clone, Copy)]
struct Bounds {
    start: f64,
    end: f64,
    low: f64,
    high: f64,
}

impl Bounds {
    fn of(segment: &Segment, horizontal: bool) -> Self {
        let (start, end) = segment.interval(horizontal);
        let (low, high) = segment.interval(!horizontal);
        Self {
            start,
            end,
            low,
            high,
        }
    }

    fn overlaps_other_axis(self, low: f64, high: f64) -> bool {
        low <= self.high && high >= self.low
    }
}

pub(super) fn pairs(
    edges: &[(u32, EdgeGeometry)],
    compress_shared: bool,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    Ok(edge_pairs(edges, compress_shared, work)?.len() as u64)
}

fn edge_pairs(
    edges: &[(u32, EdgeGeometry)],
    compress_shared: bool,
    work: &mut Work<'_>,
) -> LayoutResult<BTreeSet<(u32, u32)>> {
    if edges.len() < 2 {
        return Ok(BTreeSet::new());
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
        return sweep(
            segments.chunk_by(|a, b| a.coordinates() == b.coordinates()),
            segments.len(),
            horizontal,
            true,
            work,
        );
    }

    segments.sort_unstable_by(|a, b| {
        a.interval(horizontal)
            .0
            .total_cmp(&b.interval(horizontal).0)
            .then(a.edge.cmp(&b.edge))
    });
    sweep(segments.chunks(1), segments.len(), horizontal, false, work)
}

fn sweep<'a>(
    mut groups: impl Iterator<Item = &'a [Segment]> + Clone,
    segment_count: usize,
    horizontal: bool,
    shared: bool,
    work: &mut Work<'_>,
) -> LayoutResult<BTreeSet<(u32, u32)>> {
    let mut pairs = BTreeSet::new();
    let mut active: Vec<&[Segment]> = Vec::new();
    let mut previous_start = None;
    // Build only at a separately admitted work batch and after enough second-axis
    // rejections to cover ordering, tree setup, and maintenance. Narrow or mostly
    // overlapping fronts keep scanning.
    let setup_bound = segment_count
        .saturating_mul(segment_count.checked_ilog2().unwrap_or(0) as usize + 1)
        .saturating_mul(2);
    let mut misses = 0usize;
    let mut index_allowed = true;
    let end = |neighbors: &[Segment]| neighbors[0].interval(horizontal).1;
    while let Some(owners) = groups.next() {
        work.visit(active.len() + owners.len())?;
        let bounds = Bounds::of(&owners[0], horizontal);
        if index_allowed && misses >= setup_bound && active.len() >= MIN_INDEX_FRONT {
            // Admission uses the old width. Decide whether to build using the rows
            // that can actually meet this segment after expiry.
            if previous_start != Some(bounds.start) {
                visit_active(&mut active, &mut previous_start, bounds.start, end, |_| {
                    Ok(())
                })?;
            }
            if active.len() >= MIN_INDEX_FRONT {
                index_allowed = false;
                // Bound the remaining scans even if no more rows expire.
                let tail = groups
                    .size_hint()
                    .1
                    .unwrap_or(segment_count)
                    .saturating_add(1);
                let possible_scans = tail.saturating_mul(active.len().saturating_add(tail / 2));
                if possible_scans >= setup_bound.saturating_mul(2) {
                    let remaining = active
                        .iter()
                        .copied()
                        .chain(std::iter::once(owners))
                        .chain(groups.clone());
                    if let Some(index) = index::ActiveIntervals::new(remaining, horizontal, work)? {
                        index.sweep(active.len(), shared, &mut pairs, work)?;
                        break;
                    }
                }
            }
        }
        visit_active(
            &mut active,
            &mut previous_start,
            bounds.start,
            end,
            |neighbors| {
                let (low, high) = neighbors[0].interval(!horizontal);
                if bounds.overlaps_other_axis(low, high) {
                    record_crossings(owners, neighbors, shared, &mut pairs, work)?;
                } else {
                    misses = misses.saturating_add(1);
                }
                Ok(())
            },
        )?;
        crate::work::push(&mut active, owners)?;
    }
    Ok(pairs)
}

fn record_crossings(
    owners: &[Segment],
    neighbors: &[Segment],
    shared: bool,
    pairs: &mut BTreeSet<(u32, u32)>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let (s, t) = (&owners[0], &neighbors[0]);
    if (!shared && s.edge == t.edge) || !s.straddles(t) || !t.straddles(s) {
        return Ok(());
    }
    for first in owners {
        for second in neighbors {
            work.visit(1 + pairs.len().checked_ilog2().unwrap_or(0) as usize)?;
            if first.edge != second.edge {
                pairs.insert((first.edge.min(second.edge), first.edge.max(second.edge)));
            }
        }
    }
    Ok(())
}

/// Evict expired entries at each new start while visiting live candidates in sweep order.
fn visit_active<T: Copy>(
    active: &mut Vec<T>,
    previous_start: &mut Option<f64>,
    start: f64,
    end: impl Fn(T) -> f64,
    mut visit: impl FnMut(T) -> LayoutResult<()>,
) -> LayoutResult<()> {
    // Retained entries already passed this start. The newly appended entry ends at
    // or after its own start. Numeric equality matches the inclusive expiry test.
    if previous_start.replace(start) == Some(start) {
        return active.iter().copied().try_for_each(visit);
    }

    let mut kept = 0;
    for index in 0..active.len() {
        let entry = active[index];
        if end(entry) >= start {
            visit(entry)?;
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

    #[test]
    fn indexed_grid_keeps_only_crossings_in_the_same_band() {
        let count = 192u32;
        for transpose in [false, true] {
            let oriented = |points: &[(f64, f64)]| {
                let points: Vec<_> = points
                    .iter()
                    .map(|&(x, y)| if transpose { (y, x) } else { (x, y) })
                    .collect();
                route(&points)
            };
            let width = f64::from(count) * 4.;
            let mut edges = Vec::new();
            for band in 0..count {
                let i = f64::from(band);
                let y = i * 4. - 1.;
                let x = width / 2. + i;
                let middle = if band == 0 { -0.0 } else { y + 1. };
                edges.push((band * 2, oriented(&[(i * 2., middle), (width, middle)])));
                edges.push((band * 2 + 1, oriented(&[(x, y), (x, y + 2.)])));
            }
            // Widen the sweep domain without raising the active front or adding a crossing.
            let far = width.powi(3);
            edges.push((count * 2, oriented(&[(far, 0.), (far, 2.)])));
            let expected = (0..count).map(|i| (i * 2, i * 2 + 1)).collect();

            for shared in [false, true] {
                let mut control = NoopWorkControl;
                let mut work = Work::new(&mut control);
                assert_eq!(edge_pairs(&edges, shared, &mut work).unwrap(), expected);
                work.finish().unwrap();
            }
        }
    }

    #[test]
    fn indexed_fans_keep_distinct_owners_at_equal_numeric_starts() {
        let count = 128u32;
        let mut edges = Vec::new();
        let mut expected = BTreeSet::new();
        for band in 0..count {
            let id = band * 5;
            let y = f64::from(band) * 4.;
            let a = (-0.0, y);
            let b = (256., y + 2.);
            edges.extend([
                (id, route(&[a, b, a, b])),
                (id + 1, route(&[a, b])),
                (id + 2, route(&[(0.0, y), b])),
                (id + 3, route(&[(0.0, y + 2.), (256., y)])),
                // This edge only touches the diagonals at their endpoints.
                (id + 4, route(&[(0.0, y), (-0.0, y), (256., y)])),
            ]);
            expected.extend((id..id + 3).map(|owner| (owner, id + 3)));
        }
        edges.push((count * 5, route(&[(1e9, 0.), (1e9, 2.)])));

        for shared in [false, true] {
            let mut control = NoopWorkControl;
            let mut work = Work::new(&mut control);
            assert_eq!(edge_pairs(&edges, shared, &mut work).unwrap(), expected);
            work.finish().unwrap();
        }
    }
}
