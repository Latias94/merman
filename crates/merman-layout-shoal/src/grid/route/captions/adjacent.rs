use super::{LayoutResult, Work, push};

const MERGE_DISTANCE: f64 = 0.5;
const LINES_PER_SIDE: usize = 12;
// One extra bucket per line is enough for the final merge across bucket boundaries.
const BUCKETS_PER_SIDE: usize = 2 * LINES_PER_SIDE;

#[derive(Clone, Copy, Default)]
struct Candidate {
    cross: f64,
    offset: f64,
    bucket: f64,
}

#[derive(Default)]
struct SideCandidates {
    nearest: [Candidate; BUCKETS_PER_SIDE],
    len: usize,
}

impl SideCandidates {
    fn insert(&mut self, candidate: Candidate, work: &mut Work<'_>) -> LayoutResult<()> {
        if self.len == BUCKETS_PER_SIDE && candidate.bucket > self.nearest[self.len - 1].bucket {
            return Ok(());
        }
        work.visit(self.len.checked_ilog2().map_or(0, |n| n as usize + 1))?;
        let at = self.nearest[..self.len].partition_point(|entry| entry.bucket < candidate.bucket);
        if at < self.len && self.nearest[at].bucket == candidate.bucket {
            let old = self.nearest[at];
            if (candidate.offset, candidate.cross) < (old.offset, old.cross) {
                self.nearest[at] = candidate;
            }
        } else {
            let end = self.len.min(BUCKETS_PER_SIDE - 1);
            work.visit(end - at + 1)?;
            self.nearest.copy_within(at..end, at + 1);
            self.nearest[at] = candidate;
            self.len = (self.len + 1).min(BUCKETS_PER_SIDE);
        }
        Ok(())
    }

    fn merge(&mut self, work: &mut Work<'_>) -> LayoutResult<()> {
        let mut len = 0;
        for i in 0..self.len {
            work.visit(1)?;
            let candidate = self.nearest[i];
            if len == 0 || candidate.offset - self.nearest[len - 1].offset >= MERGE_DISTANCE {
                self.nearest[len] = candidate;
                len += 1;
                if len == LINES_PER_SIDE {
                    break;
                }
            }
        }
        self.len = len;
        Ok(())
    }
}

/// Keep only nearby boundary lines, irrespective of the order in which an index found them.
pub(super) struct AdjacentLines {
    anchor: f64,
    range: [f64; 2],
    sides: [SideCandidates; 2],
}

impl AdjacentLines {
    pub(super) fn new(anchor: f64, range: [f64; 2]) -> Self {
        Self {
            anchor,
            range,
            sides: Default::default(),
        }
    }

    pub(super) fn insert(&mut self, cross: f64, work: &mut Work<'_>) -> LayoutResult<()> {
        work.visit(1)?;
        if !(self.range[0]..=self.range[1]).contains(&cross) {
            return Ok(());
        }
        let offset = (cross - self.anchor).abs();
        let bucket = (offset / MERGE_DISTANCE).floor();
        if !bucket.is_finite() {
            return Ok(());
        }
        self.sides[usize::from(cross > self.anchor)].insert(
            Candidate {
                cross,
                offset,
                bucket,
            },
            work,
        )
    }

    pub(super) fn collect(mut self, seats: &mut Vec<f64>, work: &mut Work<'_>) -> LayoutResult<()> {
        let [left, right] = &mut self.sides;
        left.merge(work)?;
        right.merge(work)?;
        let mut left = left.nearest[..left.len].iter().peekable();
        let mut right = right.nearest[..right.len].iter().peekable();
        loop {
            work.visit(1)?;
            let side = match (left.peek(), right.peek()) {
                (None, None) => break,
                (Some(a), Some(b)) if a.offset > b.offset => &mut right,
                (None, _) => &mut right,
                _ => &mut left,
            };
            if let Some(candidate) = side.next() {
                push(seats, candidate.cross)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn nonfinite_offsets_do_not_enter_adjacent_lines() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let mut lines = AdjacentLines::new(0.0, [f64::NEG_INFINITY, f64::INFINITY]);
        for at in [f64::NAN, -2.0, f64::INFINITY, f64::MAX, 3.0] {
            lines.insert(at, &mut work).unwrap();
        }
        let mut seats = Vec::new();
        lines.collect(&mut seats, &mut work).unwrap();
        assert_eq!(seats, [-2.0, 3.0]);
    }

    #[test]
    fn lines_are_bounded_merged_and_independent_of_obstacle_order() {
        let collect = |input: &mut dyn Iterator<Item = f64>| {
            let mut noop = NoopWorkControl;
            let mut work = Work::new(&mut noop);
            let mut lines = AdjacentLines::new(0.0, [-100.0, 100.0]);
            for at in input {
                lines.insert(at, &mut work).unwrap();
            }
            let mut found = Vec::new();
            lines.collect(&mut found, &mut work).unwrap();
            found
        };
        let input = (-300..=300).map(|i| i as f64 * 0.13);
        let found = collect(&mut input.clone());
        assert_eq!(found, collect(&mut input.rev()));
        assert_eq!(found.len(), 2 * LINES_PER_SIDE);
        assert_eq!(found[0], 0.0);
        assert!(found.windows(2).all(|v| v[0].abs() <= v[1].abs()));
        for positive in [false, true] {
            let mut last = f64::NEG_INFINITY;
            for at in found.iter().copied().filter(|at| (*at > 0.0) == positive) {
                assert!(at.abs() - last >= MERGE_DISTANCE);
                last = at.abs();
            }
        }
    }
}
