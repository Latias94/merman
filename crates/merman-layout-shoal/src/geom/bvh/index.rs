use super::{Bounds, Bvh, BvhEntry, LEAF_SIZE};
use crate::work::{LayoutResult, Work, push, reserved};
use crate::{LayoutError, WorkControl, WorkError};
use std::ops::ControlFlow::{Break, Continue};

/// An append-only index of occupied rectangles. Full batches are merged like a binary
/// counter, so each query checks at most one balanced tree per size and one small tail.
#[derive(Default)]
pub struct BoundsIndex {
    levels: Vec<Option<Bvh<Bounds>>>,
    tail: Vec<Bounds>,
}

impl BoundsIndex {
    pub fn insert(
        &mut self,
        bounds: Bounds,
        control: &mut impl WorkControl,
    ) -> Result<(), LayoutError> {
        self.insert_with_work(bounds, &mut Work::new(control))
    }

    /// Reports an intersection with positive area. Touching edges and empty boxes are allowed.
    pub fn any_overlap(
        &self,
        area: Bounds,
        control: &mut impl WorkControl,
    ) -> Result<bool, LayoutError> {
        let mut work = Work::new(control);
        let overlapping = area.has_area()
            && self.visit_overlaps_with_work(area, &mut work, |bounds, _| {
                Ok(bounds.has_area() && area.overlaps_strictly(bounds))
            })?;
        work.finish()?;
        Ok(overlapping)
    }

    /// Sums the intersection area with each occupied box, including their shared areas.
    pub fn overlap_area(
        &self,
        area: Bounds,
        control: &mut impl WorkControl,
    ) -> Result<f64, LayoutError> {
        let mut work = Work::new(control);
        let mut total = 0.0;
        if area.has_area() {
            self.visit_overlaps_with_work(area, &mut work, |bounds, _| {
                let width = area.x[1].min(bounds.x[1]) - area.x[0].max(bounds.x[0]);
                let height = area.y[1].min(bounds.y[1]) - area.y[0].max(bounds.y[0]);
                if width > 0.0 && height > 0.0 {
                    total += width * height;
                }
                Ok(false)
            })?;
        }
        work.finish()?;
        Ok(total)
    }

    pub(crate) fn insert_with_work(
        &mut self,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if !bounds.finite() || bounds.x[0] > bounds.x[1] || bounds.y[0] > bounds.y[1] {
            return Err(LayoutError::InvalidInput("invalid occupied bounds"));
        }
        if self.tail.len() + 1 < LEAF_SIZE {
            work.charge(1)?;
            return push(&mut self.tail, bounds);
        }

        let mut level = 0;
        let mut count = LEAF_SIZE;
        while self.levels.get(level).is_some_and(Option::is_some) {
            work.visit(1)?;
            count = count.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
            level += 1;
        }
        work.charge(count)?;
        let mut items = reserved(count)?;
        items.extend_from_slice(&self.tail);
        items.push(bounds);
        for tree in self.levels[..level].iter().flatten() {
            work.visit(1)?;
            items.extend_from_slice(tree.items());
        }
        let tree = Bvh::from_bounds(items, work)?;
        // Keep the old index usable if building or admitting the merged tree fails.
        work.charge(level + 1)?;
        if let Some(slot) = self.levels.get_mut(level) {
            *slot = Some(tree);
        } else {
            push(&mut self.levels, Some(tree))?;
        }
        self.levels[..level].fill_with(|| None);
        self.tail.clear();
        Ok(())
    }

    pub(crate) fn visit_overlaps_with_work(
        &self,
        area: Bounds,
        work: &mut Work<'_>,
        mut found: impl FnMut(Bounds, &mut Work<'_>) -> LayoutResult<bool>,
    ) -> LayoutResult<bool> {
        for &bounds in &self.tail {
            work.visit(1)?;
            if area.overlaps(bounds) && found(bounds, work)? {
                return Ok(true);
            }
        }
        for tree in self.levels.iter().rev() {
            work.visit(1)?;
            if let Some(tree) = tree
                && tree
                    .visit(|entry| -> LayoutResult<_> {
                        work.visit(1)?;
                        Ok(match entry {
                            BvhEntry::Node(bounds) => Continue(area.overlaps(bounds)),
                            BvhEntry::Item(&bounds)
                                if area.overlaps(bounds) && found(bounds, work)? =>
                            {
                                Break(())
                            }
                            BvhEntry::Item(_) => Continue(true),
                        })
                    })?
                    .is_break()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    fn square(x: f64, y: f64) -> Bounds {
        Bounds {
            x: [x, x + 4.0],
            y: [y, y + 4.0],
        }
    }

    #[test]
    fn occupied_boxes_survive_merges_and_allow_boundary_contact() {
        let mut index = BoundsIndex::default();
        let mut control = NoopWorkControl;
        for i in 0..73 {
            let x = 10.0 * (i % 8) as f64;
            let y = 10.0 * (i / 8) as f64;
            index.insert(square(x, y), &mut control).unwrap();
        }
        for i in 0..80 {
            let x = 10.0 * (i % 8) as f64;
            let y = 10.0 * (i / 8) as f64;
            assert_eq!(
                index.any_overlap(square(x + 3.0, y), &mut control),
                Ok(i < 73)
            );
            assert_eq!(
                index.any_overlap(square(x + 4.0, y), &mut control),
                Ok(false)
            );
        }
    }

    #[test]
    fn overlap_area_sums_each_occupied_box_across_batches() {
        let mut index = BoundsIndex::default();
        let mut control = NoopWorkControl;
        for i in 1..8 {
            index
                .insert(square(10.0 * i as f64, 0.0), &mut control)
                .unwrap();
        }
        index.insert(square(0.0, 0.0), &mut control).unwrap();
        index.insert(square(2.0, 2.0), &mut control).unwrap();
        index.insert(square(5.0, 1.0), &mut control).unwrap();
        assert_eq!(index.overlap_area(square(1.0, 1.0), &mut control), Ok(18.0));
        assert_eq!(index.overlap_area(square(0.0, 9.0), &mut control), Ok(0.0));
    }

    #[test]
    fn interrupted_merge_keeps_the_old_boxes_and_can_retry() {
        struct Budget(usize);
        impl WorkControl for Budget {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }
            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.0 = self.0.checked_sub(units).ok_or(WorkError::Interrupted)?;
                Ok(())
            }
        }

        let mut index = BoundsIndex::default();
        let mut control = NoopWorkControl;
        for i in 1..LEAF_SIZE {
            index
                .insert(square(i as f64 * 10.0, 0.0), &mut control)
                .unwrap();
        }
        let next = square(0.0, 0.0);
        assert_eq!(
            index.insert(next, &mut Budget(LEAF_SIZE)),
            Err(WorkError::Interrupted.into())
        );
        assert_eq!(index.any_overlap(next, &mut control), Ok(false));
        assert_eq!(index.any_overlap(square(10.0, 0.0), &mut control), Ok(true));
        index.insert(next, &mut control).unwrap();
        assert_eq!(index.any_overlap(next, &mut control), Ok(true));
    }
}
