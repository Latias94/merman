//! Heavy-light indexing for ancestry, depth, and scope queries on importer nodes.

use std::collections::HashMap;

use super::{ElkInputNode, ImportResult};
use crate::work::{WorkError, checked_sum};

pub(super) struct HierarchyQueryIndex {
    parents: Vec<Option<usize>>,
    depths: Vec<usize>,
    roots: Vec<usize>,
    chain_heads: Vec<usize>,
    chain_positions: Vec<usize>,
    nodes_by_chain_position: Vec<usize>,
    preorder_positions: Vec<usize>,
    subtree_ends: Vec<usize>,
}

impl HierarchyQueryIndex {
    pub(super) fn new(
        node_order: &HashMap<&str, usize>,
        root_children: &[&ElkInputNode],
        children_by_parent: &HashMap<&str, Vec<&ElkInputNode>>,
    ) -> ImportResult<Self> {
        let mut parents = vec![None; node_order.len()];
        let mut children = vec![Vec::new(); node_order.len()];
        for (parent, input_children) in children_by_parent {
            let parent = node_order[*parent];
            for child in input_children {
                let child_index = node_order[child.id.as_str()];
                parents[child_index] = Some(parent);
                children[parent].push(child_index);
            }
        }

        let mut depths = vec![0usize; node_order.len()];
        let mut roots = vec![0usize; node_order.len()];
        let mut preorder_positions = vec![0usize; node_order.len()];
        let mut preorder = Vec::with_capacity(node_order.len());
        let mut stack = root_children
            .iter()
            .rev()
            .map(|node| {
                let node = node_order[node.id.as_str()];
                (node, node)
            })
            .collect::<Vec<_>>();
        while let Some((node, root)) = stack.pop() {
            roots[node] = root;
            preorder_positions[node] = preorder.len();
            preorder.push(node);
            let child_depth = checked_sum([depths[node], 1])?;
            for &child in children[node].iter().rev() {
                depths[child] = child_depth;
                stack.push((child, root));
            }
        }

        let mut subtree_sizes = vec![1usize; node_order.len()];
        let mut heavy_children = vec![None; node_order.len()];
        for &node in preorder.iter().rev() {
            let mut largest = 0usize;
            for &child in &children[node] {
                subtree_sizes[node] = checked_sum([subtree_sizes[node], subtree_sizes[child]])?;
                if subtree_sizes[child] > largest {
                    largest = subtree_sizes[child];
                    heavy_children[node] = Some(child);
                }
            }
        }
        let mut subtree_ends = vec![0usize; node_order.len()];
        for node in 0..node_order.len() {
            subtree_ends[node] = checked_sum([preorder_positions[node], subtree_sizes[node]])?;
        }

        let mut chain_heads = vec![0usize; node_order.len()];
        let mut chain_positions = vec![0usize; node_order.len()];
        let mut nodes_by_chain_position = Vec::with_capacity(node_order.len());
        let mut chains = root_children
            .iter()
            .rev()
            .map(|node| {
                let node = node_order[node.id.as_str()];
                (node, node)
            })
            .collect::<Vec<_>>();
        while let Some((start, head)) = chains.pop() {
            let mut current = Some(start);
            while let Some(node) = current {
                chain_heads[node] = head;
                chain_positions[node] = nodes_by_chain_position.len();
                nodes_by_chain_position.push(node);
                for &child in children[node].iter().rev() {
                    if Some(child) != heavy_children[node] {
                        chains.push((child, child));
                    }
                }
                current = heavy_children[node];
            }
        }

        Ok(Self {
            parents,
            depths,
            roots,
            chain_heads,
            chain_positions,
            nodes_by_chain_position,
            preorder_positions,
            subtree_ends,
        })
    }

    pub(super) fn depth(&self, node: usize) -> usize {
        self.depths[node]
    }

    pub(super) fn lift(
        &self,
        mut node: usize,
        mut steps: usize,
    ) -> ImportResult<(Option<usize>, usize)> {
        let mut probes = 0usize;
        while steps > 0 {
            probes = checked_sum([probes, 1])?;
            let head = self.chain_heads[node];
            let distance = self
                .depth(node)
                .checked_sub(self.depth(head))
                .ok_or(WorkError::ArithmeticOverflow)?;
            if steps <= distance {
                let position = self.chain_positions[node]
                    .checked_sub(steps)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                return Ok((Some(self.nodes_by_chain_position[position]), probes));
            }
            steps = steps
                .checked_sub(checked_sum([distance, 1])?)
                .ok_or(WorkError::ArithmeticOverflow)?;
            let Some(parent) = self.parents[head] else {
                return Ok((None, probes));
            };
            node = parent;
        }
        Ok((Some(node), probes))
    }

    pub(super) fn is_ancestor(&self, ancestor: usize, node: usize) -> ImportResult<(bool, usize)> {
        let position = self.preorder_positions[node];
        Ok((
            self.preorder_positions[ancestor] <= position && position < self.subtree_ends[ancestor],
            1,
        ))
    }

    pub(super) fn lca(&self, left: usize, right: usize) -> ImportResult<(Option<usize>, usize)> {
        if self.roots[left] != self.roots[right] {
            return Ok((None, 1));
        }
        let mut left = left;
        let mut right = right;
        let mut probes = 0usize;
        while self.chain_heads[left] != self.chain_heads[right] {
            probes = checked_sum([probes, 1])?;
            let left_head = self.chain_heads[left];
            let right_head = self.chain_heads[right];
            if self.depth(left_head) > self.depth(right_head) {
                left = self.parents[left_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            } else {
                right = self.parents[right_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            }
        }
        probes = checked_sum([probes, 1])?;
        let common = if self.depth(left) <= self.depth(right) {
            left
        } else {
            right
        };
        Ok((Some(common), probes))
    }
}
