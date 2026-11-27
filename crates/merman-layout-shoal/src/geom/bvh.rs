mod index;

pub use index::BoundsIndex;

use super::Bounds;
use crate::work::{LayoutResult, Work, push, reserved};
use crate::{LayoutError, NoopWorkControl, WorkControl, WorkError};
use std::ops::{ControlFlow, Range};

const LEAF_SIZE: usize = 8;

/// Non-generic BVH partitioner over `(Bounds, original_index)` pairs so `select_nth_unstable_by`
/// is monomorphized once across the crate instead of once per `Bvh<T>` element type.
fn partition_indexed(
    entries: &mut [(Bounds, usize)],
    first: usize,
    nodes: &mut Vec<Node>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.visit(1)?;
    let mut bounds = entries[0].0;
    for &(entry_bounds, _) in &entries[1..] {
        work.visit(1)?;
        bounds = bounds.union(entry_bounds);
    }
    let index = nodes.len();
    push(
        nodes,
        Node {
            bounds,
            end: index + 1,
            items: first..first + entries.len(),
        },
    )?;
    if entries.len() > LEAF_SIZE {
        let horizontal = bounds.x[1] - bounds.x[0] >= bounds.y[1] - bounds.y[0];
        let midpoint = |bounds: Bounds| {
            let [lo, hi] = if horizontal { bounds.x } else { bounds.y };
            lo.midpoint(hi)
        };
        let half = entries.len() / 2;
        work.check()?;
        entries.select_nth_unstable_by(half, |a, b| {
            midpoint(a.0)
                .total_cmp(&midpoint(b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        work.check()?;
        let (left, right) = entries.split_at_mut(half);
        // Halving the item count bounds the build's recursion by usize::BITS.
        partition_indexed(left, first, nodes, work)?;
        partition_indexed(right, first + half, nodes, work)?;
        nodes[index].items = first..first;
        nodes[index].end = nodes.len();
    }
    Ok(())
}

/// Applies the permutation `new_items[i] = old_items[entries[i].1]` in place in `O(N)` swaps
/// without allocating a second `Vec<T>`.
fn permute_in_place<T>(items: &mut [T], entries: &mut [(Bounds, usize)]) {
    for start in 0..items.len() {
        let first_src = entries[start].1;
        if first_src == usize::MAX || first_src == start {
            entries[start].1 = usize::MAX;
            continue;
        }
        let mut cur = start;
        loop {
            let src = entries[cur].1;
            entries[cur].1 = usize::MAX;
            if src == start {
                break;
            }
            items.swap(cur, src);
            cur = src;
        }
    }
}

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

    /// Builds the same tree with its sorting and traversal charged to the caller.
    pub fn build_controlled(
        items: Vec<T>,
        bounds: impl Fn(&T) -> Bounds,
        control: &mut impl WorkControl,
    ) -> Result<Self, LayoutError> {
        let mut work = Work::new(control);
        work.check()?;
        let tree = Self::build_with_work(items, bounds, &mut work)?;
        work.finish()?;
        Ok(tree)
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
            if len <= LEAF_SIZE {
                work.visit(1)?;
                let mut union = bounds(&items[0]);
                for item in &items[1..] {
                    work.visit(1)?;
                    union = union.union(bounds(item));
                }
                push(
                    &mut nodes,
                    Node {
                        bounds: union,
                        end: 1,
                        items: 0..len,
                    },
                )?;
            } else {
                let mut entries: Vec<(Bounds, usize)> = reserved(len)?;
                for (index, item) in items.iter().enumerate() {
                    entries.push((bounds(item), index));
                }
                partition_indexed(&mut entries, 0, &mut nodes, work)?;
                permute_in_place(&mut items, &mut entries);
            }
        }
        Ok(Self { items, nodes })
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

    #[test]
    fn non_generic_partition_and_in_place_permutation_preserve_items_and_enclosing_bounds() {
        #[derive(Debug, PartialEq)]
        struct Tagged {
            id: usize,
            bounds: Bounds,
        }
        for len in [0, 1, 7, 8, 9, 17, 33, 129] {
            let items: Vec<Tagged> = (0..len)
                .map(|i| {
                    let x = ((i * 37) % 101) as f64;
                    let y = ((i * 53) % 89) as f64;
                    Tagged {
                        id: i,
                        bounds: Bounds {
                            x: [x, x + 5.0],
                            y: [y, y + 3.0],
                        },
                    }
                })
                .collect();
            let tree = Bvh::build(items, |item| item.bounds).unwrap();
            assert_eq!(tree.items().len(), len);
            let mut seen = vec![false; len];
            let mut leaf_items = 0usize;
            let _ = tree
                .visit(|entry| -> Result<ControlFlow<(), bool>, ()> {
                    match entry {
                        BvhEntry::Node(_) => Ok(ControlFlow::Continue(true)),
                        BvhEntry::Item(item) => {
                            assert!(!seen[item.id], "duplicate item {}", item.id);
                            seen[item.id] = true;
                            leaf_items += 1;
                            Ok(ControlFlow::Continue(true))
                        }
                    }
                })
                .unwrap();
            assert_eq!(leaf_items, len);
            assert!(seen.into_iter().all(|v| v));
            // Verify every query point finds all overlapping items.
            if len > 0 {
                let query = Bounds {
                    x: [20.0, 45.0],
                    y: [15.0, 40.0],
                };
                let mut found = Vec::new();
                let _ = tree
                    .visit(|entry| -> Result<ControlFlow<(), bool>, ()> {
                        Ok(match entry {
                            BvhEntry::Node(b) => ControlFlow::Continue(b.overlaps(query)),
                            BvhEntry::Item(item) => {
                                if item.bounds.overlaps(query) {
                                    found.push(item.id);
                                }
                                ControlFlow::Continue(true)
                            }
                        })
                    })
                    .unwrap();
                let mut expected: Vec<usize> = tree
                    .items()
                    .iter()
                    .filter(|item| item.bounds.overlaps(query))
                    .map(|item| item.id)
                    .collect();
                found.sort_unstable();
                expected.sort_unstable();
                assert_eq!(found, expected, "mismatch for len={len}");
            }
        }
    }
}
