use super::{Bounds, Group, record_crossings};
use crate::WorkError;
use crate::compound::PairSet;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use std::ops::Range;

const NO_ROW: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Extrema {
    end: f64,
    high: f64,
}

impl Extrema {
    const EMPTY: Self = Self {
        end: f64::INFINITY,
        high: f64::NEG_INFINITY,
    };

    fn combined(self, other: Self) -> Self {
        Self {
            end: self.end.min(other.end),
            high: self.high.max(other.high),
        }
    }
}

struct Row<'a> {
    group: Group<'a>,
    leaf: usize,
    previous: usize,
    next: usize,
}

// Leaves follow second-axis lows. The minimum sweep-axis end finds expired entries,
// and the maximum second-axis high prunes intervals in the queried prefix of lows.
// Live rows also have sweep-order links for queries where scanning is cheaper.
pub(super) struct ActiveIntervals<'a> {
    rows: Vec<Row<'a>>,
    by_low: Vec<usize>,
    tree: Vec<Extrema>,
    base: usize,
    first: usize,
    last: usize,
    len: usize,
}

impl<'a> ActiveIntervals<'a> {
    pub(super) fn new(
        groups: impl Iterator<Item = Group<'a>>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let mut rows = Vec::new();
        work.check()?;
        for group in groups {
            work.visit(1)?;
            let bounds = group.bounds;
            // The numeric expiry and overlap comparisons need ordered, finite bounds.
            if ![bounds.start, bounds.end, bounds.low, bounds.high]
                .into_iter()
                .all(f64::is_finite)
            {
                return Ok(None);
            }
            push(
                &mut rows,
                Row {
                    group,
                    leaf: 0,
                    previous: NO_ROW,
                    next: NO_ROW,
                },
            )?;
        }
        let base = rows
            .len()
            .checked_next_power_of_two()
            .ok_or(WorkError::ArithmeticOverflow)?;
        let tree_len = base.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(rows.len())?;
        let mut by_low = reserved(rows.len())?;
        by_low.extend(0..rows.len());
        work.sort(&mut by_low, |&a, &b| {
            rows[a]
                .group
                .bounds
                .low
                .total_cmp(&rows[b].group.bounds.low)
        })?;
        work.check()?;
        for (slot, &row) in by_low.iter().enumerate() {
            work.visit(1)?;
            rows[row].leaf = base + slot;
        }

        work.charge(tree_len)?;
        let tree = filled(tree_len, Extrema::EMPTY)?;
        work.check()?;
        Ok(Some(Self {
            rows,
            by_low,
            tree,
            base,
            first: NO_ROW,
            last: NO_ROW,
            len: 0,
        }))
    }

    pub(super) fn sweep(
        mut self,
        carried: usize,
        pairs: &mut PairSet,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let mut hits = Vec::new();
        let mut misses_before_retry = 0;
        let height = self.base.ilog2() as usize + 1;
        for row in 0..carried {
            work.visit(height)?;
            self.insert(row);
        }
        for row in carried..self.rows.len() {
            let group = self.rows[row].group;
            if row != carried {
                work.visit(group.owners.len())?;
            }
            self.expire(group.bounds.start, height, work)?;
            self.visit_overlaps(
                group.bounds,
                &mut hits,
                &mut misses_before_retry,
                work,
                |other, work| record_crossings(group, self.rows[other].group, pairs, work),
            )?;
            work.visit(height)?;
            self.insert(row);
        }
        Ok(())
    }

    fn insert(&mut self, row: usize) {
        self.rows[row].previous = self.last;
        if self.last == NO_ROW {
            self.first = row;
        } else {
            self.rows[self.last].next = row;
        }
        self.last = row;
        self.len += 1;
        let Bounds { end, high, .. } = self.rows[row].group.bounds;
        self.set_leaf(self.rows[row].leaf, Extrema { end, high });
    }

    fn expire(&mut self, start: f64, height: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        work.visit(1)?;
        while self.tree[1].end <= start {
            work.visit(2 * height)?;
            let mut node = 1;
            while node < self.base {
                node *= 2;
                if self.tree[node].end > start {
                    node += 1;
                }
            }
            let row = self.by_low[node - self.base];
            let Row { previous, next, .. } = self.rows[row];
            if previous == NO_ROW {
                self.first = next;
            } else {
                self.rows[previous].next = next;
            }
            if next == NO_ROW {
                self.last = previous;
            } else {
                self.rows[next].previous = previous;
            }
            self.len -= 1;
            self.set_leaf(node, Extrema::EMPTY);
        }
        Ok(())
    }

    fn set_leaf(&mut self, mut node: usize, extrema: Extrema) {
        self.tree[node] = extrema;
        while node > 1 {
            node /= 2;
            self.tree[node] = self.tree[node * 2].combined(self.tree[node * 2 + 1]);
        }
    }

    fn visit_overlaps(
        &self,
        bounds: Bounds,
        hits: &mut Vec<usize>,
        misses_before_retry: &mut usize,
        work: &mut Work<'_>,
        mut visit: impl FnMut(usize, &mut Work<'_>) -> LayoutResult<()>,
    ) -> LayoutResult<()> {
        if self.len == 0 {
            return Ok(());
        }
        let probing = *misses_before_retry == 0;
        if probing {
            hits.clear();
            work.visit(self.base.ilog2() as usize + 1)?;
            let stop = self
                .by_low
                .partition_point(|&row| self.rows[row].group.bounds.low < bounds.high);
            let mut query = Overlaps {
                low: bounds.low,
                stop,
                remaining: self.len,
                hits,
            };
            if self.collect(1, 0..self.base, &mut query, work)? {
                // The charge for a crossing depends on the set size at the attempted
                // insertion. Report overlaps in exactly the original sweep order.
                work.sort(hits, usize::cmp)?;
                for &row in hits.iter() {
                    work.visit(1)?;
                    visit(row, work)?;
                }
                return Ok(());
            }
        }

        let mut misses = 0;
        let mut row = self.first;
        while row != NO_ROW {
            work.visit(1)?;
            let entry = &self.rows[row];
            if bounds.overlaps_other_axis(entry.group.bounds) {
                visit(row, work)?;
            } else {
                misses += 1;
            }
            row = entry.next;
        }
        // An abandoned walk can visit len nodes. Before retrying, scan enough
        // subsequent second-axis rejections to cover that walk.
        *misses_before_retry = if probing {
            self.len
        } else {
            misses_before_retry.saturating_sub(misses)
        };
        Ok(())
    }

    fn collect(
        &self,
        node: usize,
        range: Range<usize>,
        query: &mut Overlaps<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if query.remaining == 0 {
            return Ok(false);
        }
        query.remaining -= 1;
        work.visit(1)?;
        if range.start >= query.stop || self.tree[node].high <= query.low {
            return Ok(true);
        }
        if node >= self.base {
            push(query.hits, self.by_low[range.start])?;
            return Ok(order_work(query.hits.len()) <= self.len);
        }
        let mid = range.start + range.len() / 2;
        Ok(self.collect(node * 2, range.start..mid, query, work)?
            && self.collect(node * 2 + 1, mid..range.end, query, work)?)
    }
}

fn order_work(count: usize) -> usize {
    count.saturating_mul(count.checked_ilog2().unwrap_or(0) as usize + 1)
}

struct Overlaps<'a> {
    low: f64,
    stop: usize,
    remaining: usize,
    hits: &'a mut Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flat::crossings::Segment;
    use crate::{Axis, RoutePoint, WorkControl};

    #[test]
    fn sparse_index_visits_only_the_relevant_rows_in_sweep_order() {
        #[derive(Default)]
        struct Meter(usize);

        impl WorkControl for Meter {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }

            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.0 += units;
                Ok(())
            }
        }

        let segment = |edge, (x1, y1), (x2, y2)| {
            Segment::new(
                RoutePoint::new(x1, y1, Axis::Vertical),
                RoutePoint::new(x2, y2, Axis::Vertical),
                edge,
            )
        };
        let mut rows: Vec<_> = (0..128)
            .map(|i| {
                let y = 1000. + f64::from(i);
                segment(100 + i, (-1., y), (10., y))
            })
            .collect();
        rows.push(segment(1, (0., 2.), (10., 2.)));
        rows.push(segment(2, (1., 0.), (10., 0.)));
        let carried = rows.len();
        rows.push(segment(3, (5., 3.), (7., -1.)));
        let query = Group::new(&rows[carried..], true, false);
        let mut index = ActiveIntervals::new(
            rows.chunks(1).map(|row| Group::new(row, true, false)),
            &mut Work::new(&mut crate::NoopWorkControl),
        )
        .unwrap()
        .unwrap();
        for row in 0..carried {
            index.insert(row);
        }

        let mut pairs = PairSet::new();
        let mut noop = crate::NoopWorkControl;
        let mut setup = Work::new(&mut noop);
        for pair in [(1, 3), (4, 5), (6, 7)] {
            pairs.insert(pair, &mut setup).unwrap();
        }
        let mut hits = Vec::new();
        let mut metered = Meter::default();
        let mut work = Work::new(&mut metered);
        let mut misses_before_retry = 0;
        index
            .visit_overlaps(
                query.bounds,
                &mut hits,
                &mut misses_before_retry,
                &mut work,
                |other, work| record_crossings(query, index.rows[other].group, &mut pairs, work),
            )
            .unwrap();
        work.finish().unwrap();
        assert_eq!(misses_before_retry, 0);
        assert_eq!(hits, [128, 129]);
        assert_eq!(
            pairs.to_btree_set(),
            std::collections::BTreeSet::from([(1, 3), (2, 3), (4, 5), (6, 7)])
        );
        assert!(metered.0 < carried, "{} visits", metered.0);
    }
}
