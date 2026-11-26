use super::Bounds;
use crate::work::{LayoutResult, Work, push};
use crate::{LayoutError, NoopWorkControl, WorkError};
use std::ops::{ControlFlow, Range};

const LEAF_SIZE: usize = 8;

fn selection_work(len: usize) -> Result<usize, WorkError> {
    // Above the last level with any splits, each level selects across all items.
    // On that last level partition sizes differ by at most one; only the larger
    // partitions split if the smaller ones already fit in a leaf.
    let levels = (len / LEAF_SIZE).ilog2() as usize;
    let partitions = 1 << levels;
    let extra = len - partitions * LEAF_SIZE;
    let last = if extra < partitions {
        extra * (LEAF_SIZE + 1)
    } else {
        len
    };
    len.checked_mul(levels)
        .and_then(|units| units.checked_add(last))
        .ok_or(WorkError::ArithmeticOverflow)
}

/// Read-only index of bounds with inline items and nodes in preorder.
#[derive(Debug, PartialEq)]
pub struct Bvh<T> {
    items: Vec<T>,
    nodes: Vec<Node>,
}

#[derive(Debug, PartialEq)]
struct Node {
    bounds: Bounds,
    end: usize,
    items: Range<usize>,
}

pub enum BvhEntry<'a, T> {
    /// The bounds enclose all items in this subtree.
    Node(Bounds),
    Item(&'a T),
}

impl<'a, T> IntoIterator for &'a Bvh<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl Bvh<Bounds> {
    pub(crate) fn from_bounds(bounds: Vec<Bounds>, work: &mut Work<'_>) -> LayoutResult<Self> {
        Self::build_with_work(bounds, |entry| *entry, work)
    }
}

impl<T> Bvh<T> {
    /// Partitions items in place, using the midpoint of their bounds on the longest axis.
    pub fn build(items: Vec<T>, bounds: impl Fn(&T) -> Bounds) -> Result<Self, LayoutError> {
        Self::build_with_work(items, bounds, &mut Work::new(&mut NoopWorkControl))
    }

    pub(crate) fn build_with_work(
        mut items: Vec<T>,
        bounds: impl Fn(&T) -> Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let mut nodes = Vec::new();
        let len = items.len();
        if len > LEAF_SIZE {
            work.charge(selection_work(len)?)?;
        }
        if len != 0 {
            let estimate = len
                .div_ceil(LEAF_SIZE)
                .checked_mul(2)
                .ok_or(WorkError::ArithmeticOverflow)?;
            nodes
                .try_reserve(estimate)
                .map_err(|_| LayoutError::AllocationLimit)?;
            Self::partition(&mut items, 0, &mut nodes, &bounds, work)?;
        }
        Ok(Self { items, nodes })
    }

    fn partition(
        items: &mut [T],
        first: usize,
        nodes: &mut Vec<Node>,
        bounds_of: &impl Fn(&T) -> Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.visit(1)?;
        let mut bounds = bounds_of(&items[0]);
        for item in &items[1..] {
            work.visit(1)?;
            bounds = bounds.union(bounds_of(item));
        }
        let index = nodes.len();
        push(
            nodes,
            Node {
                bounds,
                end: index + 1,
                items: first..first + items.len(),
            },
        )?;
        if items.len() > LEAF_SIZE {
            let horizontal = bounds.x[1] - bounds.x[0] >= bounds.y[1] - bounds.y[0];
            let midpoint = |item: &T| {
                let bounds = bounds_of(item);
                let [lo, hi] = if horizontal { bounds.x } else { bounds.y };
                lo.midpoint(hi)
            };
            let half = items.len() / 2;
            work.check()?;
            items.select_nth_unstable_by(half, |a, b| midpoint(a).total_cmp(&midpoint(b)));
            work.check()?;
            let (left, right) = items.split_at_mut(half);
            // Halving the item count bounds the build's recursion by usize::BITS.
            Self::partition(left, first, nodes, bounds_of, work)?;
            Self::partition(right, first + half, nodes, bounds_of, work)?;
            nodes[index].items = first..first;
            nodes[index].end = nodes.len();
        }
        Ok(())
    }

    pub(crate) fn root_bounds(&self) -> Option<Bounds> {
        self.nodes.first().map(|node| node.bounds)
    }

    pub(crate) fn items(&self) -> &[T] {
        &self.items
    }

    /// Visits nodes and leaf items in preorder, passing both to the same callback so
    /// it can meter work and update its query as it goes. `Continue(false)` skips a
    /// node's entire subtree and has no effect for items. `Break(())` stops the walk.
    pub fn visit<E>(
        &self,
        mut visit: impl FnMut(BvhEntry<'_, T>) -> Result<ControlFlow<(), bool>, E>,
    ) -> Result<ControlFlow<()>, E> {
        let mut at = 0;
        while let Some(node) = self.nodes.get(at) {
            match visit(BvhEntry::Node(node.bounds))? {
                ControlFlow::Continue(true) => {}
                ControlFlow::Continue(false) => {
                    at = node.end;
                    continue;
                }
                ControlFlow::Break(()) => return Ok(ControlFlow::Break(())),
            }
            for item in &self.items[node.items.clone()] {
                if visit(BvhEntry::Item(item))?.is_break() {
                    return Ok(ControlFlow::Break(()));
                }
            }
            at += 1;
        }
        Ok(ControlFlow::Continue(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkControl;

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

    #[test]
    fn all_median_work_is_admitted_before_reading_bounds() {
        let result = Bvh::build_with_work(
            vec![(); 17],
            |_| panic!("unadmitted build read its bounds"),
            &mut Work::new(&mut Budget(17 + 9 - 1)),
        );
        assert_eq!(result, Err(WorkError::Interrupted.into()));
    }

    #[test]
    fn an_uneven_tree_consumes_the_same_total_scan_and_selection_budget() {
        fn expected(len: usize) -> usize {
            if len <= LEAF_SIZE {
                return len;
            }
            2 * len + expected(len / 2) + expected(len - len / 2)
        }

        let bounds = Bounds {
            x: [0.0, 1.0],
            y: [0.0, 1.0],
        };
        for len in (0..=2 * LEAF_SIZE * LEAF_SIZE).chain([511, 512, 513, 4097]) {
            let mut meter = Budget(expected(len));
            let mut work = Work::new(&mut meter);
            let tree = Bvh::from_bounds(vec![bounds; len], &mut work).unwrap();
            assert_eq!(tree.root_bounds(), (len > 0).then_some(bounds));
            work.finish().unwrap();
            assert_eq!(meter.0, 0, "item count {len}");
        }
    }
}
