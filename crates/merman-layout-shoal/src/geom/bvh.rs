use super::Bounds;
use crate::work::{LayoutResult, Work, push};
use crate::{LayoutError, NoopWorkControl};
use std::ops::{ControlFlow, Range};

const LEAF_SIZE: usize = 8;

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
        if !items.is_empty() {
            let estimate = items
                .len()
                .div_ceil(LEAF_SIZE)
                .checked_mul(2)
                .ok_or(crate::WorkError::ArithmeticOverflow)?;
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
            work.charge(items.len())?;
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
