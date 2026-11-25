use super::{Bounds, Segment, record_crossings};
use crate::WorkError;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use std::{collections::BTreeSet, ops::Range};

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
    owners: &'a [Segment],
    bounds: Bounds,
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
        groups: impl Iterator<Item = &'a [Segment]>,
        horizontal: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let mut rows = Vec::new();
        let mut poll = Poll::default();
        work.check()?;
        for owners in groups {
            poll.advance(owners.len(), work)?;
            let bounds = Bounds::of(&owners[0], horizontal);
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
                    owners,
                    bounds,
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
        let mut by_low = reserved(rows.len())?;
        for row in 0..rows.len() {
            by_low.push(row);
            poll.step(work)?;
        }
        work.check()?;
        by_low.sort_unstable_by(|&a, &b| rows[a].bounds.low.total_cmp(&rows[b].bounds.low));
        work.check()?;
        for (slot, &row) in by_low.iter().enumerate() {
            poll.step(work)?;
            rows[row].leaf = base + slot;
        }

        work.check()?;
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
        shared: bool,
        pairs: &mut BTreeSet<(u32, u32)>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let mut poll = Poll::default();
        let mut hits = Vec::new();
        let mut misses_before_retry = 0;
        for row in 0..carried {
            poll.step(work)?;
            self.insert(row);
        }
        for row in carried..self.rows.len() {
            let owners = self.rows[row].owners;
            let bounds = self.rows[row].bounds;
            // Charge the size before expiry, just as the linear sweep does. The first
            // row was already admitted by the caller when it switched to the index.
            if row != carried {
                work.visit(self.len + owners.len())?;
            }
            poll.step(work)?;
            self.expire(bounds.start, &mut poll, work)?;
            self.visit_overlaps(
                bounds,
                &mut hits,
                &mut misses_before_retry,
                &mut poll,
                work,
                |other, work| {
                    record_crossings(owners, self.rows[other].owners, shared, pairs, work)
                },
            )?;
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
        let Bounds { end, high, .. } = self.rows[row].bounds;
        self.set_leaf(self.rows[row].leaf, Extrema { end, high });
    }

    fn expire(&mut self, start: f64, poll: &mut Poll, work: &mut Work<'_>) -> LayoutResult<()> {
        while self.tree[1].end < start {
            poll.step(work)?;
            let mut node = 1;
            while node < self.base {
                node *= 2;
                if self.tree[node].end >= start {
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
        poll: &mut Poll,
        work: &mut Work<'_>,
        mut visit: impl FnMut(usize, &mut Work<'_>) -> LayoutResult<()>,
    ) -> LayoutResult<()> {
        if self.len == 0 {
            return Ok(());
        }
        let probing = *misses_before_retry == 0;
        if probing {
            hits.clear();
            let stop = self
                .by_low
                .partition_point(|&row| self.rows[row].bounds.low <= bounds.high);
            let mut query = Overlaps {
                low: bounds.low,
                stop,
                remaining: self.len,
                hits,
            };
            if self.collect(1, 0..self.base, &mut query, poll, work)? {
                // The charge for a crossing depends on the set size at the attempted
                // insertion. Report overlaps in exactly the original sweep order.
                let bulk = order_work(hits.len()) >= 128;
                if bulk {
                    work.check()?;
                }
                hits.sort_unstable();
                if bulk {
                    work.check()?;
                }
                for &row in hits.iter() {
                    poll.step(work)?;
                    visit(row, work)?;
                }
                return Ok(());
            }
        }

        let mut misses = 0;
        let mut row = self.first;
        while row != NO_ROW {
            poll.step(work)?;
            let entry = &self.rows[row];
            if bounds.overlaps_other_axis(entry.bounds.low, entry.bounds.high) {
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
        poll: &mut Poll,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if query.remaining == 0 {
            return Ok(false);
        }
        query.remaining -= 1;
        poll.step(work)?;
        if range.start >= query.stop || self.tree[node].high < query.low {
            return Ok(true);
        }
        if node >= self.base {
            push(query.hits, self.by_low[range.start])?;
            return Ok(order_work(query.hits.len()) <= self.len);
        }
        let mid = range.start + range.len() / 2;
        Ok(self.collect(node * 2, range.start..mid, query, poll, work)?
            && self.collect(node * 2 + 1, mid..range.end, query, poll, work)?)
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

// Index work uses the already-admitted sort and scan allowances. Keep interruptible
// walks responsive without changing the crossing charge sequence.
#[derive(Default)]
struct Poll(u8);

impl Poll {
    fn step(&mut self, work: &mut Work<'_>) -> LayoutResult<()> {
        self.advance(1, work)
    }

    fn advance(&mut self, units: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        if units >= usize::from(128 - self.0) {
            self.0 = 0;
            work.check()?;
        } else {
            self.0 += units as u8;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, RoutePoint, WorkControl};

    #[test]
    fn indexed_hits_debit_the_duplicate_before_the_new_pair() {
        #[derive(Default)]
        struct Meter(Vec<usize>);

        impl WorkControl for Meter {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }

            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.0.push(units);
                Ok(())
            }
        }

        let segment = |edge, (x1, y1), (x2, y2)| Segment {
            a: RoutePoint::new(x1, y1, Axis::Vertical),
            b: RoutePoint::new(x2, y2, Axis::Vertical),
            edge,
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
        let owners = &rows[carried..];
        let mut control = Meter::default();
        let mut work = Work::new(&mut control);
        let mut index = ActiveIntervals::new(rows.chunks(1), true, &mut work)
            .unwrap()
            .unwrap();
        for row in 0..carried {
            index.insert(row);
        }

        let mut pairs = BTreeSet::from([(1, 3), (4, 5), (6, 7)]);
        let mut hits = Vec::new();
        let mut misses_before_retry = 0;
        work.visit(carried + owners.len()).unwrap();
        index
            .visit_overlaps(
                Bounds::of(&owners[0], true),
                &mut hits,
                &mut misses_before_retry,
                &mut Poll::default(),
                &mut work,
                |other, work| {
                    record_crossings(owners, index.rows[other].owners, false, &mut pairs, work)
                },
            )
            .unwrap();
        work.finish().unwrap();
        assert_eq!(misses_before_retry, 0);
        assert_eq!(hits, [128, 129]);
        assert_eq!(pairs, BTreeSet::from([(1, 3), (2, 3), (4, 5), (6, 7)]));
        assert_eq!(control.0, [131, 4]);
    }
}
