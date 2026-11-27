//! Count distinct pairs of crossing routing centerlines before interpolation by a painter.
use super::*;
use crate::RoutePoint;
use crate::compound::PairSet;

mod index;

const MIN_INDEX_FRONT: usize = 128;
const MIN_FAN_SIZE: usize = 32;
const NO_FAN: usize = usize::MAX;

struct Segment {
    a: RoutePoint,
    b: RoutePoint,
    edge: u32,
    fan: usize,
}
impl Segment {
    fn new(a: RoutePoint, b: RoutePoint, edge: u32) -> Self {
        Self {
            a,
            b,
            edge,
            fan: NO_FAN,
        }
    }

    fn interval(&self, horizontal: bool) -> (f64, f64) {
        let (a, b) = if horizontal {
            (self.a.x, self.b.x)
        } else {
            (self.a.y, self.b.y)
        };
        (zero(a.min(b)), zero(a.max(b)))
    }

    fn coordinates(&self) -> [[u64; 2]; 2] {
        let (a, b) = (point_key(self.a), point_key(self.b));
        [a.min(b), a.max(b)]
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

fn zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn point_key(point: RoutePoint) -> [u64; 2] {
    [zero(point.x).to_bits(), zero(point.y).to_bits()]
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

    fn union(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            low: self.low.min(other.low),
            high: self.high.max(other.high),
        }
    }

    fn overlaps_other_axis(self, other: Self) -> bool {
        // Contact at a projection's end cannot be a proper intersection. This also
        // discards parallel pairs that both have zero extent on this axis.
        other.low < self.high && other.high > self.low
    }
}

#[derive(Clone, Copy)]
struct Group<'a> {
    owners: &'a [Segment],
    bounds: Bounds,
    coincident: bool,
}

impl<'a> Group<'a> {
    fn new(owners: &'a [Segment], horizontal: bool, compress_shared: bool) -> Self {
        let first = Bounds::of(&owners[0], horizontal);
        // With shared compression, the same coordinates at both ends of a sorted
        // run identify a single shaft even if it also belongs to a large fan.
        let coincident = owners[0].fan == NO_FAN
            || compress_shared && owners[0].coordinates() == owners.last().unwrap().coordinates();
        let bounds = if coincident {
            first
        } else {
            owners[1..].iter().fold(first, |bounds, segment| {
                bounds.union(Bounds::of(segment, horizontal))
            })
        };
        Self {
            owners,
            bounds,
            coincident,
        }
    }

    fn runs(self) -> impl Iterator<Item = &'a [Segment]> {
        let size = if self.coincident {
            self.owners.len()
        } else {
            1
        };
        self.owners.chunks(size)
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
) -> LayoutResult<PairSet> {
    if edges.len() < 2 {
        return Ok(PairSet::new());
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
                segments.push(Segment::new(p[0], p[1], *id));
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
    let fans = assign_fans(&mut segments, work)?;
    work.charge_sort(segments.len())?;
    segments.sort_unstable_by(|a, b| {
        a.fan
            .cmp(&b.fan)
            .then_with(|| {
                a.interval(horizontal)
                    .0
                    .total_cmp(&b.interval(horizontal).0)
            })
            .then_with(|| {
                if compress_shared {
                    a.coordinates().cmp(&b.coordinates())
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then(a.edge.cmp(&b.edge))
    });
    if compress_shared {
        work.charge(segments.len())?;
        segments.dedup_by(|a, b| a.edge == b.edge && a.coordinates() == b.coordinates());
    }
    let groups = segments
        .chunk_by(|a, b| {
            a.fan == b.fan
                && (a.fan != NO_FAN || compress_shared && a.coordinates() == b.coordinates())
        })
        .map(|owners| Group::new(owners, horizontal, compress_shared));
    if fans {
        work.charge(segments.len())?;
        let mut ordered = crate::work::reserved(segments.len())?;
        ordered.extend(groups.enumerate());
        work.charge_sort(ordered.len())?;
        ordered.sort_unstable_by(|(ia, a), (ib, b)| {
            a.bounds
                .start
                .total_cmp(&b.bounds.start)
                .then(a.owners[0].edge.cmp(&b.owners[0].edge))
                .then(ia.cmp(ib))
        });
        let len = ordered.len();
        fn group_of<'a>((_, group): (usize, Group<'a>)) -> Group<'a> {
            group
        }
        sweep(ordered.into_iter().map(group_of), len, work)
    } else {
        sweep(groups, segments.len(), work)
    }
}

/// All members of a fan share an exact endpoint, so its internal pairs cannot cross.
/// Compare the fan's envelope to other groups and expand it only for potential foreign hits.
fn assign_fans(segments: &mut [Segment], work: &mut Work<'_>) -> LayoutResult<bool> {
    if segments.len() < MIN_INDEX_FRONT {
        return Ok(false);
    }
    let count = segments
        .len()
        .checked_mul(2)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count)?;
    let mut ends = crate::work::reserved(count)?;
    for (index, segment) in segments.iter().enumerate() {
        ends.push((point_key(segment.a), index));
        ends.push((point_key(segment.b), index));
    }
    work.charge_sort(ends.len())?;
    ends.sort_unstable();

    let mut degrees = crate::work::reserved(ends.len() / MIN_FAN_SIZE)?;
    for cohort in ends.chunk_by(|a, b| a.0 == b.0) {
        work.visit(cohort.len())?;
        let degree = cohort.len();
        if degree < MIN_FAN_SIZE {
            continue;
        }
        let fan = degrees.len();
        degrees.push(degree);
        work.visit(degree)?;
        for &(_, index) in cohort {
            let owner = &mut segments[index].fan;
            if *owner == NO_FAN || degrees[*owner] < degree {
                *owner = fan;
            }
        }
    }
    Ok(!degrees.is_empty())
}

fn sweep<'a>(
    mut groups: impl Iterator<Item = Group<'a>> + Clone,
    group_count: usize,
    work: &mut Work<'_>,
) -> LayoutResult<PairSet> {
    let mut pairs = PairSet::new();
    let mut active: Vec<Group<'_>> = Vec::new();
    let mut previous_start = None;
    // Build only at a separately admitted work batch and after enough second-axis
    // rejections to cover ordering, tree setup, and maintenance. Narrow or mostly
    // overlapping fronts keep scanning.
    let setup_bound = group_count
        .saturating_mul(group_count.checked_ilog2().unwrap_or(0) as usize + 1)
        .saturating_mul(2);
    let mut misses = 0usize;
    let mut index_allowed = true;
    let end = |group: Group<'_>| group.bounds.end;
    while let Some(group) = groups.next() {
        work.visit(group.owners.len())?;
        let bounds = group.bounds;
        if index_allowed && misses >= setup_bound && active.len() >= MIN_INDEX_FRONT {
            if previous_start != Some(bounds.start) {
                work.visit(active.len())?;
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
                    .unwrap_or(group_count)
                    .saturating_add(1);
                let possible_scans = tail.saturating_mul(active.len().saturating_add(tail / 2));
                if possible_scans >= setup_bound.saturating_mul(2) {
                    let remaining = active
                        .iter()
                        .copied()
                        .chain(std::iter::once(group))
                        .chain(groups.clone());
                    if let Some(index) = index::ActiveIntervals::new(remaining, work)? {
                        index.sweep(active.len(), &mut pairs, work)?;
                        break;
                    }
                }
            }
        }
        work.visit(active.len())?;
        visit_active(
            &mut active,
            &mut previous_start,
            bounds.start,
            end,
            |neighbors| {
                if bounds.overlaps_other_axis(neighbors.bounds) {
                    record_crossings(group, neighbors, &mut pairs, work)?;
                } else {
                    misses = misses.saturating_add(1);
                }
                Ok(())
            },
        )?;
        crate::work::push(&mut active, group)?;
    }
    Ok(pairs)
}

fn record_crossings(
    group: Group<'_>,
    other: Group<'_>,
    pairs: &mut PairSet,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    for owners in group.runs() {
        for neighbors in other.runs() {
            work.visit(1)?;
            let (s, t) = (&owners[0], &neighbors[0]);
            if (owners.len() == 1 && neighbors.len() == 1 && s.edge == t.edge)
                || !s.straddles(t)
                || !t.straddles(s)
            {
                continue;
            }
            for first in owners {
                for second in neighbors {
                    work.visit(1)?;
                    if first.edge != second.edge {
                        pairs.insert(
                            (first.edge.min(second.edge), first.edge.max(second.edge)),
                            work,
                        )?;
                    }
                }
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
    // Retained entries already passed this start. An entry appended at this start can
    // only end here if it has no extent on the sweep axis. The exact test rejects it.
    if previous_start.replace(start) == Some(start) {
        return active.iter().copied().try_for_each(visit);
    }

    let mut kept = 0;
    for index in 0..active.len() {
        let entry = active[index];
        if end(entry) > start {
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
            shared_bus: false,
            corridor_clearance: 0.0,
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
    fn shared_segment_keys_ignore_direction_and_signed_zero() {
        let a = RoutePoint::new(-0.0, 0.0, Axis::Horizontal);
        let b = RoutePoint::new(10.0, 2.0, Axis::Horizontal);
        let forward = Segment::new(a, b, 0);
        let reverse = Segment::new(b, RoutePoint::new(0.0, -0.0, Axis::Vertical), 1);
        assert_eq!(forward.coordinates(), reverse.coordinates());
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
            .map(|edge| {
                let ends = if edge % 2 == 0 {
                    [(0., 0.), (0., 10.)]
                } else {
                    [(0., 10.), (0., 0.)]
                };
                (edge, route(&ends))
            })
            .collect();
        edges.push((count, route(&[(-10., 5.), (10., 5.)])));
        for compress_shared in [false, true] {
            let mut meter = Meter::default();
            let mut work = Work::new(&mut meter);
            assert_eq!(
                pairs(&edges, compress_shared, &mut work),
                Ok(u64::from(count))
            );
            work.finish().unwrap();
            assert!(meter.0 <= 32_000, "{}", meter.0);
        }
    }

    #[test]
    fn radial_groups_match_pairwise_crossings_with_foreign_edges() {
        let mut edges: Vec<_> = (0..500)
            .map(|id| {
                let leaf = (2.0 * f64::from(id), 4.0);
                let center = (500.0, 20.0);
                let points = if id % 2 == 0 {
                    [leaf, center]
                } else {
                    [center, leaf]
                };
                (id, route(&points))
            })
            .collect();
        edges.extend([
            (500, route(&[(0., 12.), (500., 12.)])),
            (501, route(&[(750., 2.), (700., 34.)])),
            (502, route(&[(500., 20.), (250., 12.)])),
        ]);
        let segments: Vec<_> = edges
            .iter()
            .map(|(id, edge)| Segment::new(edge.points[0], edge.points[1], *id))
            .collect();
        let mut expected = std::collections::BTreeSet::new();
        for (i, a) in segments.iter().enumerate() {
            for b in &segments[i + 1..] {
                if a.straddles(b) && b.straddles(a) {
                    expected.insert((a.edge, b.edge));
                }
            }
        }
        assert!(!expected.is_empty());
        for compress_shared in [false, true] {
            let mut control = NoopWorkControl;
            let mut work = Work::new(&mut control);
            assert_eq!(
                edge_pairs(&edges, compress_shared, &mut work)
                    .unwrap()
                    .to_btree_set(),
                expected
            );
            work.finish().unwrap();
        }
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
            let expected: std::collections::BTreeSet<_> =
                (0..count).map(|i| (i * 2, i * 2 + 1)).collect();

            for shared in [false, true] {
                let mut control = NoopWorkControl;
                let mut work = Work::new(&mut control);
                assert_eq!(
                    edge_pairs(&edges, shared, &mut work)
                        .unwrap()
                        .to_btree_set(),
                    expected
                );
                work.finish().unwrap();
            }
        }
    }

    #[test]
    fn indexed_fans_keep_distinct_owners_at_equal_numeric_starts() {
        let count = 128u32;
        let mut edges = Vec::new();
        let mut expected = std::collections::BTreeSet::new();
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
            assert_eq!(
                edge_pairs(&edges, shared, &mut work)
                    .unwrap()
                    .to_btree_set(),
                expected
            );
            work.finish().unwrap();
        }
    }
}
