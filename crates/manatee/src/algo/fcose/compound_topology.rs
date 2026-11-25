use super::{SimEdge, SimNode};
use crate::error::WorkFailure;

/// The static compound graph projection used by the CoSE kernel.
///
/// Cytoscape/layout-base computes each edge's lowest common ancestor and the two immediate
/// children below that ancestor before the spring embedder starts.  The previous Rust path
/// rebuilt a temporary mark array for every edge on every rerun and independently rescanned all
/// owner graphs to decide where gravity applies.  Keep those source-backed facts together so the
/// hierarchy is built once and reused by both runs and every iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EdgeProjection {
    pub(super) lca_owner_idx: usize,
    pub(super) source_in_lca: usize,
    pub(super) target_in_lca: usize,
}

#[derive(Debug, Clone)]
pub(super) struct CompoundTopology {
    pub(super) edge_projections: Vec<EdgeProjection>,
    pub(super) owner_connected: Vec<bool>,
}

impl CompoundTopology {
    /// Return a conservative setup tranche for the temporary binary-lifting table, edge
    /// projections, and one global connectivity union-find.
    ///
    /// This count-only entry point assumes the deepest possible compound chain. Callers that know
    /// the compound cardinality should use [`Self::work_units_with_compound_count`] to avoid
    /// charging a flat graph for a `log V` ancestry table.
    #[cfg(test)]
    pub(super) fn work_units(
        node_count: usize,
        edge_count: usize,
    ) -> std::result::Result<usize, WorkFailure> {
        Self::work_units_with_compound_count(node_count, edge_count, node_count)
    }

    /// Return a conservative topology tranche for a known compound cardinality.
    ///
    /// Ancestry projection is O((V + E) log D), where `D <= compounds + 1`, while union-find
    /// parent scans retain an independent O((V + E) log V) worst-case bound. Peak temporary space
    /// is O(V log D + E), and retained topology is O(V + E) after the ancestry table is dropped.
    pub(super) fn work_units_with_compound_count(
        node_count: usize,
        edge_count: usize,
        compound_count: usize,
    ) -> std::result::Result<usize, WorkFailure> {
        if node_count == 0 {
            return Ok(0);
        }

        let owner_count = node_count
            .checked_add(1)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let max_inclusion_depth = if compound_count >= node_count {
            node_count
        } else {
            compound_count + 1
        };
        let ancestry_levels = Self::lifting_level_count(max_inclusion_depth);
        let lifting_slots = owner_count
            .checked_mul(ancestry_levels)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let edge_projection_units = edge_count
            .checked_mul(
                ancestry_levels
                    .checked_mul(4)
                    .and_then(|units| units.checked_add(4))
                    .ok_or(WorkFailure::ArithmeticOverflow)?,
            )
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let find_work_units = Self::dsu_find_work_units(node_count, edge_count)?;
        let connectivity_units = owner_count
            .checked_add(node_count)
            .and_then(|units| units.checked_add(find_work_units))
            .ok_or(WorkFailure::ArithmeticOverflow)?;

        // Parent/depth initialization and the lifting table itself are both charged.  The
        // connectivity term covers DSU initialization, owner scans, and `levels(V)` parent-slot
        // visits for each of at most `2E + V` find calls. Union-by-size bounds the uncompressed
        // parent height; path halving can only reduce the executed work.
        owner_count
            .checked_mul(2)
            .and_then(|units| units.checked_add(lifting_slots))
            .and_then(|units| units.checked_add(edge_projection_units))
            .and_then(|units| units.checked_add(connectivity_units))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    pub(super) fn dsu_find_work_units(
        node_count: usize,
        edge_count: usize,
    ) -> std::result::Result<usize, WorkFailure> {
        let find_call_count = edge_count
            .checked_mul(2)
            .and_then(|calls| calls.checked_add(node_count))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        find_call_count
            .checked_mul(Self::lifting_level_count(node_count))
            .ok_or(WorkFailure::ArithmeticOverflow)
    }

    pub(super) fn lifting_level_count(cardinality: usize) -> usize {
        if cardinality <= 1 {
            1
        } else {
            (usize::BITS - cardinality.leading_zeros()) as usize
        }
    }

    pub(super) fn build(
        nodes: &[SimNode],
        edges: &[SimEdge],
        root_owner_idx: usize,
        children_by_owner: &[Vec<usize>],
        inclusion_depth: &[usize],
    ) -> Self {
        let node_count = nodes.len();
        if node_count == 0 {
            return Self {
                edge_projections: Vec::new(),
                owner_connected: vec![true; children_by_owner.len()],
            };
        }

        let max_inclusion_depth = inclusion_depth.iter().copied().max().unwrap_or(1);
        let levels = Self::lifting_level_count(max_inclusion_depth);
        let owner_count = node_count + 1;
        let mut depth = vec![0usize; owner_count];
        for (idx, value) in inclusion_depth.iter().copied().enumerate().take(node_count) {
            depth[idx] = value;
        }

        let mut first_ancestor = vec![root_owner_idx; owner_count];
        for (idx, node) in nodes.iter().enumerate() {
            first_ancestor[idx] = node.owner_idx.min(root_owner_idx);
        }
        first_ancestor[root_owner_idx] = root_owner_idx;
        let mut ancestors = Vec::with_capacity(levels);
        ancestors.push(first_ancestor);
        for level in 1..levels {
            let previous = &ancestors[level - 1];
            let mut current = vec![root_owner_idx; owner_count];
            for idx in 0..owner_count {
                let parent = previous[idx];
                current[idx] = previous.get(parent).copied().unwrap_or(root_owner_idx);
            }
            ancestors.push(current);
        }

        let ancestry = CompoundAncestry {
            root_owner_idx,
            depth,
            ancestors,
        };
        let edge_projections = edges
            .iter()
            .map(|edge| ancestry.project_edge(edge.a, edge.b))
            .collect::<Vec<_>>();
        let owner_connected = Self::connected_owners(nodes, &edge_projections, children_by_owner);

        Self {
            edge_projections,
            owner_connected,
        }
    }

    fn connected_owners(
        nodes: &[SimNode],
        edge_projections: &[EdgeProjection],
        children_by_owner: &[Vec<usize>],
    ) -> Vec<bool> {
        let mut dsu = DisjointSet::new(nodes.len());
        for projection in edge_projections {
            let source = projection.source_in_lca;
            let target = projection.target_in_lca;
            if source >= nodes.len() || target >= nodes.len() || source == target {
                continue;
            }
            // An LCA projection is always made of immediate children of that owner graph.  The
            // guard keeps malformed private test fixtures fail-closed without changing public
            // graph validation behavior.
            if nodes[source].owner_idx == projection.lca_owner_idx
                && nodes[target].owner_idx == projection.lca_owner_idx
            {
                dsu.union(source, target);
            }
        }

        let mut connected = vec![true; children_by_owner.len()];
        for (owner, children) in children_by_owner.iter().enumerate() {
            let Some((&first, rest)) = children.split_first() else {
                continue;
            };
            let representative = dsu.find(first);
            for &child in rest {
                if dsu.find(child) != representative {
                    connected[owner] = false;
                    break;
                }
            }
        }
        connected
    }
}

#[derive(Debug, Clone)]
struct CompoundAncestry {
    root_owner_idx: usize,
    depth: Vec<usize>,
    ancestors: Vec<Vec<usize>>,
}

impl CompoundAncestry {
    fn parent_of(&self, node_idx: usize) -> usize {
        self.ancestors
            .first()
            .and_then(|level| level.get(node_idx).copied())
            .unwrap_or(self.root_owner_idx)
    }

    fn lift(&self, mut node_idx: usize, mut distance: usize) -> usize {
        let mut level = 0usize;
        while distance != 0 {
            if distance & 1 == 1 {
                node_idx = self
                    .ancestors
                    .get(level)
                    .and_then(|table| table.get(node_idx).copied())
                    .unwrap_or(self.root_owner_idx);
            }
            distance >>= 1;
            level = level.saturating_add(1);
        }
        node_idx
    }

    fn lca(&self, mut first: usize, mut second: usize) -> usize {
        first = first.min(self.root_owner_idx);
        second = second.min(self.root_owner_idx);
        if self.depth[first] < self.depth[second] {
            std::mem::swap(&mut first, &mut second);
        }
        first = self.lift(first, self.depth[first].saturating_sub(self.depth[second]));
        if first == second {
            return first;
        }
        for table in self.ancestors.iter().rev() {
            let first_parent = table.get(first).copied().unwrap_or(self.root_owner_idx);
            let second_parent = table.get(second).copied().unwrap_or(self.root_owner_idx);
            if first_parent != second_parent {
                first = first_parent;
                second = second_parent;
            }
        }
        self.parent_of(first)
    }

    fn child_below(&self, node_idx: usize, ancestor: usize) -> usize {
        let node_idx = node_idx.min(self.root_owner_idx);
        if self.parent_of(node_idx) == ancestor {
            return node_idx;
        }
        let desired_depth = self.depth[ancestor].saturating_add(1);
        let distance = self.depth[node_idx].saturating_sub(desired_depth);
        self.lift(node_idx, distance)
    }

    fn project_edge(&self, source: usize, target: usize) -> EdgeProjection {
        let source = source.min(self.root_owner_idx);
        let target = target.min(self.root_owner_idx);
        let source_owner = self.parent_of(source);
        let target_owner = self.parent_of(target);
        let lca_owner_idx = self.lca(source_owner, target_owner);
        let source_in_lca = if source_owner == lca_owner_idx {
            source
        } else {
            self.child_below(source, lca_owner_idx)
        };
        let target_in_lca = if target_owner == lca_owner_idx {
            target
        } else {
            self.child_below(target, lca_owner_idx)
        };
        EdgeProjection {
            lca_owner_idx,
            source_in_lca,
            target_in_lca,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DisjointSet {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl DisjointSet {
    pub(super) fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
            size: vec![1; len],
        }
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent.get(node).copied().unwrap_or(node) != node {
            let parent = self.parent[node];
            let grandparent = self.parent.get(parent).copied().unwrap_or(parent);
            self.parent[node] = grandparent;
            node = grandparent;
        }
        node
    }

    pub(super) fn union(&mut self, first: usize, second: usize) {
        if first >= self.parent.len() || second >= self.parent.len() {
            return;
        }
        let mut first_root = self.find(first);
        let mut second_root = self.find(second);
        if first_root == second_root {
            return;
        }
        if self.size[first_root] < self.size[second_root] {
            std::mem::swap(&mut first_root, &mut second_root);
        }
        self.parent[second_root] = first_root;
        self.size[first_root] = self.size[first_root].saturating_add(self.size[second_root]);
    }

    #[cfg(test)]
    pub(super) fn parent_depth(&self, mut node: usize) -> usize {
        let mut depth = 0usize;
        while self
            .parent
            .get(node)
            .copied()
            .is_some_and(|parent| parent != node)
        {
            node = self.parent[node];
            depth += 1;
            assert!(depth <= self.parent.len(), "disjoint-set parent cycle");
        }
        depth
    }
}
