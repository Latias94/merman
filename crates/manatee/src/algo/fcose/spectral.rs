#![allow(clippy::needless_range_loop)]

use super::{FcoseRandom, NoopWorkControl, SimEdge, SimNode, WorkControl};
use crate::WorkFailure;

#[cfg(test)]
use std::cell::Cell;

const INFINITY_HOPS: f64 = 100_000_000.0;
const SMALL: f64 = 1e-9;

const DEFAULT_SAMPLE_SIZE: usize = 25;
const DEFAULT_PI_TOL: f64 = 1e-7;

const MAX_POWER_ITERATIONS: usize = 10_000;
const MAX_SVD_QR_ITERATIONS_PER_VALUE: usize = 10_000;
const MAX_SVD_QR_PHASES_PER_VALUE: usize = 4;

#[cfg(test)]
thread_local! {
    static TOPOLOGY_BUILD_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn record_topology_build() {
    TOPOLOGY_BUILD_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn reset_topology_build_count() {
    TOPOLOGY_BUILD_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn topology_build_count() -> usize {
    TOPOLOGY_BUILD_COUNT.with(Cell::get)
}

/// Immutable hierarchy-derived graph used by every randomized run of one layout invocation.
///
/// It intentionally excludes mutable geometry and adjusted ideal lengths, so a rerun can reuse
/// the expensive transformed adjacency without observing stale spring state.
#[derive(Debug)]
pub(super) struct SpectralTopology {
    real_node_count: usize,
    adjacency: Vec<Vec<usize>>,
    adjacency_entries: usize,
}

impl SpectralTopology {
    pub(super) fn build<W: WorkControl + ?Sized>(
        nodes: &[SimNode],
        edges: &[SimEdge],
        compound_parent: &[Option<usize>],
        work_control: &mut W,
    ) -> Result<Self, WorkFailure> {
        work_control.charge(topology_build_work_units(
            nodes.len(),
            edges.len(),
            compound_parent.len(),
        )?)?;

        let adjacency = build_transformed_adjacency(nodes, edges, compound_parent)?;

        let adjacency_entries = adjacency.iter().try_fold(0usize, |total, neighbors| {
            total
                .checked_add(neighbors.len())
                .ok_or(WorkFailure::ArithmeticOverflow)
        })?;

        #[cfg(test)]
        record_topology_build();

        Ok(Self {
            real_node_count: nodes.len(),
            adjacency,
            adjacency_entries,
        })
    }

    fn node_size(&self) -> usize {
        self.adjacency.len()
    }
}

pub(super) fn apply_spectral_start_positions<W: WorkControl + ?Sized>(
    nodes: &mut [SimNode],
    edges: &[SimEdge],
    topology: &SpectralTopology,
    node_separation: f64,
    rng: &mut FcoseRandom,
    work_control: &mut W,
) -> Result<bool, WorkFailure> {
    if nodes.is_empty() {
        return Ok(false);
    }
    if nodes.len() != topology.real_node_count {
        return Ok(false);
    }

    let n_real = nodes.len();
    let adjacency = &topology.adjacency;
    let node_size = topology.node_size();
    if node_size <= 1 {
        return Ok(false);
    }

    // Upstream skips spectral when the transformed graph has 1 or 2 nodes.
    if node_size == 2 {
        work_control.charge(
            edges
                .len()
                .checked_add(nodes.len())
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        )?;
        if n_real != 2 {
            return Ok(false);
        }
        // Place the second node to the right of the first node using an ideal edge length.
        // This matches upstream spectral.js' fallback path.
        let ideal = edges
            .iter()
            .filter(|edge| edge.a < n_real && edge.b < n_real)
            .map(|e| e.ideal_length)
            .find(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(50.0);

        let (first, second) = (&nodes[0], &nodes[1]);
        let x1 = first.center_x();
        let y1 = first.center_y();
        let x2 = x1 + first.width / 2.0 + second.width / 2.0 + ideal;

        nodes[1].left = x2 - nodes[1].width / 2.0;
        nodes[1].top = y1 - nodes[1].height / 2.0;
        return Ok(true);
    }

    let sample_size = node_size.min(DEFAULT_SAMPLE_SIZE);
    if sample_size <= 1 {
        return Ok(false);
    }

    work_control.charge(spectral_runtime_setup_work_units(
        node_size,
        topology.adjacency_entries,
        sample_size,
        n_real,
    )?)?;

    // Column sampling matrix (squared shortest-path distances).
    // Keep this as a plain Vec-backed matrix to match upstream JS operation order more closely.
    let mut c: Vec<Vec<f64>> = vec![vec![0.0; sample_size]; node_size];
    let mut samples: Vec<usize> = vec![0; sample_size];
    let mut min_dist: Vec<f64> = vec![INFINITY_HOPS; node_size];

    // Greedy sampling (Mermaid default): pick a random first sample, then repeatedly pick the node
    // that maximizes the minimum distance to the already-sampled set.
    //
    // Note: any "seed offset" to match upstream Mermaid baseline RNG consumption should be
    // applied *outside* spectral, at the layout invocation level, so reruns (`layout.run()` twice)
    // do not double-advance the RNG stream.
    let mut sample = rng.next_usize(node_size);
    min_dist.fill(INFINITY_HOPS);
    for (col, slot) in samples.iter_mut().enumerate().take(sample_size) {
        *slot = sample;
        sample = bfs_fill_column(
            sample,
            col,
            &adjacency,
            node_separation,
            &mut c,
            Some(&mut min_dist),
        );
    }

    // Square distances for C.
    for i in 0..node_size {
        for j in 0..sample_size {
            let v = c[i][j];
            c[i][j] = v * v;
        }
    }

    // PHI is the intersection of sampled rows/columns.
    let mut phi: Vec<Vec<f64>> = vec![vec![0.0; sample_size]; sample_size];
    for i in 0..sample_size {
        for j in 0..sample_size {
            phi[i][j] = c[samples[j]][i];
        }
    }

    let inv = match regularized_inverse_from_svd(&phi, work_control)? {
        Some(m) => m,
        None => return Ok(false),
    };

    let (x_coords, y_coords) = match power_iteration(rng, &c, &inv, DEFAULT_PI_TOL, work_control)? {
        Some(v) => v,
        None => return Ok(false),
    };

    for i in 0..n_real {
        let x = x_coords[i];
        let y = y_coords[i];
        if !(x.is_finite() && y.is_finite()) {
            return Ok(false);
        }
        nodes[i].left = x - nodes[i].width / 2.0;
        nodes[i].top = y - nodes[i].height / 2.0;
    }

    Ok(true)
}

fn checked_ceil_log2(value: usize) -> usize {
    if value <= 1 {
        1
    } else {
        (usize::BITS - (value - 1).leading_zeros()) as usize
    }
}

fn checked_n_log_n(value: usize) -> Result<usize, WorkFailure> {
    value
        .checked_mul(checked_ceil_log2(value))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn topology_build_work_units(
    nodes: usize,
    edges: usize,
    compounds: usize,
) -> Result<usize, WorkFailure> {
    let non_root_elements = nodes
        .checked_add(compounds)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let elements = non_root_elements
        .checked_add(1)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let graph_items = elements
        .checked_add(edges)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let hierarchy_levels = checked_ceil_log2(elements);
    let hierarchy_work = elements
        .checked_mul(hierarchy_levels)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    // Per edge the indexed projection can perform one depth-alignment lift, one reverse LCA
    // scan, and one lift for each endpoint. Keep the fixed endpoint/index work explicit too.
    let edge_projection_per_edge = hierarchy_levels
        .checked_mul(4)
        .and_then(|work| work.checked_add(12))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let edge_projection_work = edges
        .checked_mul(edge_projection_per_edge)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let edge_entries = edges
        .checked_mul(2)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    // Each non-root hierarchy element can contribute at most one representative connection to a
    // scope dummy. The final adjacency sort therefore includes both original and dummy entries.
    let dummy_entries = non_root_elements
        .checked_mul(2)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let final_entries = edge_entries
        .checked_add(dummy_entries)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    // Original real adjacency and scope-local adjacency each sort the original edge entries; the
    // transformed graph then sorts the final original-plus-dummy adjacency once more.
    let original_ordering_work = checked_n_log_n(edge_entries)?
        .checked_mul(2)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let compound_ordering_work = checked_n_log_n(compounds)?;
    let adjacency_ordering_work = original_ordering_work
        .checked_add(checked_n_log_n(final_entries)?)
        .and_then(|work| work.checked_add(compound_ordering_work))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let linear_work = elements
        .checked_mul(12)
        .and_then(|work| work.checked_add(edges.checked_mul(8)?))
        .and_then(|work| work.checked_add(final_entries.checked_mul(4)?))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    graph_items
        .checked_add(hierarchy_work)
        .and_then(|work| work.checked_add(edge_projection_work))
        .and_then(|work| work.checked_add(adjacency_ordering_work))
        .and_then(|work| work.checked_add(linear_work))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn spectral_runtime_setup_work_units(
    nodes: usize,
    adjacency_entries: usize,
    sample_size: usize,
    real_nodes: usize,
) -> Result<usize, WorkFailure> {
    let matrix_cells = nodes
        .checked_mul(sample_size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let bfs_per_sample = nodes
        .checked_mul(3)
        .and_then(|work| work.checked_add(adjacency_entries))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let bfs_work = bfs_per_sample
        .checked_mul(sample_size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let phi_cells = sample_size
        .checked_mul(sample_size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let phi_work = phi_cells
        .checked_mul(2)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let eigensolver_setup = nodes
        .checked_mul(8)
        .and_then(|work| work.checked_add(real_nodes))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    matrix_cells
        .checked_add(bfs_work)
        .and_then(|work| work.checked_add(matrix_cells))
        .and_then(|work| work.checked_add(phi_work))
        .and_then(|work| work.checked_add(eigensolver_setup))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn power_iteration_work_units(
    nodes: usize,
    sample_size: usize,
    orthogonalize: bool,
) -> Result<usize, WorkFailure> {
    let rectangular = nodes
        .checked_mul(sample_size)
        .and_then(|work| work.checked_mul(2))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let square = sample_size
        .checked_mul(sample_size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let vector_factor = if orthogonalize { 14 } else { 12 };
    let vector_work = nodes
        .checked_mul(vector_factor)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let temporary_initialization = sample_size
        .checked_mul(2)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    rectangular
        .checked_add(square)
        .and_then(|work| work.checked_add(vector_work))
        .and_then(|work| work.checked_add(temporary_initialization))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn svd_setup_work_units(rows: usize, columns: usize) -> Result<usize, WorkFailure> {
    let rank = rows.min(columns);
    let matrix_cells = rows
        .checked_mul(columns)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let bidiagonalization = matrix_cells
        .checked_mul(rank)
        .and_then(|work| work.checked_mul(4))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let left_vectors = rows
        .checked_mul(rank)
        .and_then(|work| work.checked_mul(rank))
        .and_then(|work| work.checked_mul(2))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let right_vectors = columns
        .checked_mul(columns)
        .and_then(|work| work.checked_mul(columns))
        .and_then(|work| work.checked_mul(2))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let vector_storage = columns
        .checked_mul(3)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    matrix_cells
        .checked_add(bidiagonalization)
        .and_then(|work| work.checked_add(left_vectors))
        .and_then(|work| work.checked_add(right_vectors))
        .and_then(|work| work.checked_add(rows))
        .and_then(|work| work.checked_add(vector_storage))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn svd_qr_iteration_work_units(
    rows: usize,
    columns: usize,
    active_columns: usize,
) -> Result<usize, WorkFailure> {
    let scan_work = active_columns
        .checked_mul(8)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    rows.checked_add(columns)
        .and_then(|work| work.checked_mul(active_columns))
        .and_then(|work| work.checked_mul(2))
        .and_then(|work| work.checked_add(scan_work))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

fn regularized_inverse_work_units(size: usize) -> Result<usize, WorkFailure> {
    let square = size
        .checked_mul(size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let cube = square
        .checked_mul(size)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    cube.checked_add(
        square
            .checked_mul(2)
            .ok_or(WorkFailure::ArithmeticOverflow)?,
    )
    .and_then(|work| work.checked_add(size.checked_mul(3)?))
    .ok_or(WorkFailure::ArithmeticOverflow)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ElemKey {
    Leaf(usize),
    Compound(usize),
}

fn build_transformed_adjacency(
    nodes: &[SimNode],
    edges: &[SimEdge],
    compound_parent: &[Option<usize>],
) -> Result<Vec<Vec<usize>>, WorkFailure> {
    let n_real = nodes.len();
    let compound_count = compound_parent.len();
    let root_scope = compound_count;
    let root_element = n_real
        .checked_add(compound_count)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let element_count = root_element
        .checked_add(1)
        .ok_or(WorkFailure::ArithmeticOverflow)?;

    // Transformed graph starts with all real (childless) nodes, then adds dummy nodes created by
    // `aux.connectComponents(...)` (top-level first, then for each parent in insertion order).
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n_real];
    for e in edges {
        if e.a < n_real && e.b < n_real {
            adjacency[e.a].push(e.b);
            adjacency[e.b].push(e.a);
        }
    }
    for neigh in &mut adjacency {
        neigh.sort_unstable();
        neigh.dedup();
    }

    let leaf_deg: Vec<usize> = adjacency.iter().map(|v| v.len()).collect();

    let mut parent_element = vec![root_element; element_count];
    for (leaf, node) in nodes.iter().enumerate() {
        parent_element[leaf] = node
            .parent
            .filter(|parent| *parent < compound_count)
            .map(|parent| n_real + parent)
            .unwrap_or(root_element);
    }
    for (compound, parent) in compound_parent.iter().copied().enumerate() {
        parent_element[n_real + compound] = parent
            .filter(|parent| *parent < compound_count)
            .map(|parent| n_real + parent)
            .unwrap_or(root_element);
    }
    parent_element[root_element] = root_element;

    // Resolve every hierarchy depth once. Graph validation rejects cycles before this allocation.
    let mut depth = vec![usize::MAX; element_count];
    depth[root_element] = 0;
    let mut path: Vec<usize> = Vec::new();
    for start in 0..root_element {
        if depth[start] != usize::MAX {
            continue;
        }
        path.clear();
        let mut current = start;
        while depth[current] == usize::MAX {
            path.push(current);
            if path.len() > element_count {
                return Err(WorkFailure::ArithmeticOverflow);
            }
            current = parent_element[current];
        }
        let mut current_depth = depth[current];
        while let Some(element) = path.pop() {
            current_depth = current_depth
                .checked_add(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            depth[element] = current_depth;
        }
    }

    let hierarchy_levels = checked_ceil_log2(element_count);
    let mut ancestors: Vec<Vec<usize>> = Vec::with_capacity(hierarchy_levels);
    ancestors.push(parent_element.clone());
    for level in 1..hierarchy_levels {
        let previous = &ancestors[level - 1];
        let mut current = vec![root_element; element_count];
        for element in 0..element_count {
            current[element] = previous[previous[element]];
        }
        ancestors.push(current);
    }

    let mut compounds_deep_first: Vec<usize> = (0..compound_count).collect();
    compounds_deep_first
        .sort_by_key(|compound| std::cmp::Reverse(depth[n_real.saturating_add(*compound)]));

    // Compute the upstream parentChildMap representative in one bottom-up pass. Direct leaves
    // win; otherwise the first used compound child in insertion order supplies the representative.
    let mut compound_used = vec![false; compound_count];
    let mut direct_best_leaf: Vec<Option<usize>> = vec![None; compound_count];
    let mut direct_best_degree = vec![usize::MAX; compound_count];
    for (leaf, node) in nodes.iter().enumerate() {
        let Some(parent) = node.parent.filter(|parent| *parent < compound_count) else {
            continue;
        };
        compound_used[parent] = true;
        let degree = leaf_deg[leaf];
        if direct_best_leaf[parent].is_none() || degree < direct_best_degree[parent] {
            direct_best_leaf[parent] = Some(leaf);
            direct_best_degree[parent] = degree;
        }
    }
    for &compound in &compounds_deep_first {
        if compound_used[compound]
            && let Some(parent) = compound_parent[compound]
        {
            compound_used[parent] = true;
        }
    }
    let mut first_used_child: Vec<Option<usize>> = vec![None; compound_count];
    for compound in 0..compound_count {
        if !compound_used[compound] {
            continue;
        }
        if let Some(parent) = compound_parent[compound]
            && first_used_child[parent].is_none()
        {
            first_used_child[parent] = Some(compound);
        }
    }
    let mut compound_repr_leaf = vec![None; compound_count];
    for &compound in &compounds_deep_first {
        compound_repr_leaf[compound] = direct_best_leaf[compound]
            .or_else(|| first_used_child[compound].and_then(|child| compound_repr_leaf[child]));
    }

    // Stable scope members: compounds first in insertion order, then direct leaves in node order.
    let mut scope_elements: Vec<Vec<ElemKey>> = vec![Vec::new(); compound_count + 1];
    let mut element_local_index = vec![usize::MAX; element_count];
    for compound in 0..compound_count {
        if !compound_used[compound] {
            continue;
        }
        let scope = compound_parent[compound].unwrap_or(root_scope);
        element_local_index[n_real + compound] = scope_elements[scope].len();
        scope_elements[scope].push(ElemKey::Compound(compound));
    }
    for (leaf, node) in nodes.iter().enumerate() {
        let scope = node.parent.unwrap_or(root_scope);
        element_local_index[leaf] = scope_elements[scope].len();
        scope_elements[scope].push(ElemKey::Leaf(leaf));
    }

    // An original leaf edge connects different direct children in exactly one scope: the LCA of
    // its endpoints. Assigning it once removes the previous scope-by-edge rescans.
    let mut scope_edge_pairs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); compound_count + 1];
    for edge in edges {
        if edge.a >= n_real || edge.b >= n_real || edge.a == edge.b {
            continue;
        }
        let lca = lowest_common_ancestor(edge.a, edge.b, &depth, &ancestors);
        let scope = if lca == root_element {
            root_scope
        } else if lca >= n_real && lca < root_element {
            lca - n_real
        } else {
            continue;
        };
        let a_child = direct_child_below(lca, edge.a, &depth, &ancestors);
        let b_child = direct_child_below(lca, edge.b, &depth, &ancestors);
        if a_child == b_child {
            continue;
        }
        let a_index = element_local_index[a_child];
        let b_index = element_local_index[b_child];
        if a_index == usize::MAX || b_index == usize::MAX {
            continue;
        }
        scope_edge_pairs[scope].push((a_index, b_index));
    }

    // Upstream creates the root dummy first, then parent dummies in compound insertion order.
    add_dummy_for_scope(
        &mut adjacency,
        &scope_elements[root_scope],
        &scope_edge_pairs[root_scope],
        &leaf_deg,
        &compound_repr_leaf,
    );
    for scope in 0..compound_count {
        if compound_used[scope] {
            add_dummy_for_scope(
                &mut adjacency,
                &scope_elements[scope],
                &scope_edge_pairs[scope],
                &leaf_deg,
                &compound_repr_leaf,
            );
        }
    }

    for neigh in &mut adjacency {
        neigh.sort_unstable();
        neigh.dedup();
    }

    Ok(adjacency)
}

fn lift_element(mut element: usize, mut steps: usize, ancestors: &[Vec<usize>]) -> usize {
    let mut level = 0usize;
    while steps > 0 && level < ancestors.len() {
        if steps & 1 == 1 {
            element = ancestors[level][element];
        }
        steps >>= 1;
        level += 1;
    }
    element
}

fn lowest_common_ancestor(
    mut a: usize,
    mut b: usize,
    depth: &[usize],
    ancestors: &[Vec<usize>],
) -> usize {
    if depth[a] > depth[b] {
        a = lift_element(a, depth[a] - depth[b], ancestors);
    } else if depth[b] > depth[a] {
        b = lift_element(b, depth[b] - depth[a], ancestors);
    }
    if a == b {
        return a;
    }
    for level in (0..ancestors.len()).rev() {
        if ancestors[level][a] != ancestors[level][b] {
            a = ancestors[level][a];
            b = ancestors[level][b];
        }
    }
    ancestors[0][a]
}

fn direct_child_below(
    scope: usize,
    leaf: usize,
    depth: &[usize],
    ancestors: &[Vec<usize>],
) -> usize {
    let steps = depth[leaf].saturating_sub(depth[scope]).saturating_sub(1);
    lift_element(leaf, steps, ancestors)
}

fn add_dummy_for_scope(
    transformed_adj: &mut Vec<Vec<usize>>,
    top_most: &[ElemKey],
    edge_pairs: &[(usize, usize)],
    leaf_deg: &[usize],
    compound_repr_leaf: &[Option<usize>],
) {
    if top_most.len() <= 1 {
        return;
    }

    let mut elem_adj: Vec<Vec<usize>> = vec![Vec::new(); top_most.len()];
    for &(a, b) in edge_pairs {
        if a >= elem_adj.len() || b >= elem_adj.len() || a == b {
            continue;
        }
        elem_adj[a].push(b);
        elem_adj[b].push(a);
    }
    for neigh in &mut elem_adj {
        neigh.sort_unstable();
        neigh.dedup();
    }

    let components = connected_components(&elem_adj);
    if components.len() <= 1 {
        return;
    }

    let dummy_idx = transformed_adj.len();
    transformed_adj.push(Vec::new());

    for comp in components {
        // Mirror `aux.connectComponents(...)` selection: pick the minimum-degree top-most node
        // in the component, and keep the first one on ties (JS uses `<`, not `<=`).
        let mut best = top_most[comp[0]];
        let mut best_deg = match best {
            ElemKey::Leaf(leaf) => leaf_deg.get(leaf).copied().unwrap_or(0),
            ElemKey::Compound(_) => 0,
        };
        for &i in comp.iter().skip(1) {
            let e = top_most[i];
            let deg = match e {
                ElemKey::Leaf(leaf) => leaf_deg.get(leaf).copied().unwrap_or(0),
                ElemKey::Compound(_) => 0,
            };
            if deg < best_deg {
                best = e;
                best_deg = deg;
            }
        }

        let rep_leaf = match best {
            ElemKey::Leaf(leaf) => Some(leaf),
            ElemKey::Compound(compound) => compound_repr_leaf.get(compound).copied().flatten(),
        };
        let Some(rep_leaf) = rep_leaf else {
            continue;
        };
        if rep_leaf >= transformed_adj.len() {
            continue;
        }
        transformed_adj[dummy_idx].push(rep_leaf);
        transformed_adj[rep_leaf].push(dummy_idx);
    }
}

fn connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adjacency.len();
    let mut visited = vec![false; n];
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    for start in 0..n {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        q.push_back(start);
        let mut comp: Vec<usize> = Vec::new();

        while let Some(v) = q.pop_front() {
            comp.push(v);
            for &u in &adjacency[v] {
                if !visited[u] {
                    visited[u] = true;
                    q.push_back(u);
                }
            }
        }

        out.push(comp);
    }

    out
}

fn bfs_fill_column(
    pivot: usize,
    col: usize,
    adjacency: &[Vec<usize>],
    node_separation: f64,
    c: &mut [Vec<f64>],
    mut min_dist: Option<&mut [f64]>,
) -> usize {
    let node_size = adjacency.len();
    let mut dist: Vec<i32> = vec![-1; node_size];
    let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();

    dist[pivot] = 0;
    q.push_back(pivot);

    while let Some(v) = q.pop_front() {
        for &u in &adjacency[v] {
            if dist[u] == -1 {
                dist[u] = dist[v].saturating_add(1);
                q.push_back(u);
            }
        }
    }

    let mut max_dist = 0.0;
    let mut max_idx = 0usize;
    for i in 0..node_size {
        let d = if dist[i] == -1 {
            INFINITY_HOPS
        } else {
            (dist[i] as f64) * node_separation
        };
        c[i][col] = d;

        if let Some(min_dist) = min_dist.as_deref_mut() {
            if d < min_dist[i] {
                min_dist[i] = d;
            }
            if min_dist[i] > max_dist {
                max_dist = min_dist[i];
                max_idx = i;
            }
        }
    }

    if min_dist.is_some() { max_idx } else { pivot }
}

#[derive(Debug, Clone)]
pub(super) struct SvdResult {
    pub(super) u: Vec<Vec<f64>>,
    pub(super) v: Vec<Vec<f64>>,
    pub(super) s: Vec<f64>,
}

// Port of layout-base `util/SVD.js` (JamaJS-derived) + `spectral.js` regularized inverse.
// This avoids relying on external linear algebra implementations whose numeric behavior can
// diverge enough to change the spectral basis on symmetric graphs (which cascades into different
// FCoSE results and parity-root viewports).
fn regularized_inverse_from_svd<W: WorkControl + ?Sized>(
    phi: &[Vec<f64>],
    work_control: &mut W,
) -> Result<Option<Vec<Vec<f64>>>, WorkFailure> {
    let n = phi.len();
    if n == 0 {
        return Ok(None);
    }
    if phi.iter().any(|r| r.len() != n) {
        return Ok(None);
    }

    let Some(svd) = svd_jama_controlled(phi, work_control)? else {
        return Ok(None);
    };
    if svd.s.is_empty() {
        return Ok(None);
    }

    // The regularization and V * Sig * U^T multiply are separate from the SVD kernel. Charge the
    // complete local tranche before allocating its output so rejection cannot leave partial work.
    work_control.charge(regularized_inverse_work_units(n)?)?;

    // layout-base spectral.js:
    // max_s = q[0]^3 where q is sorted descending by the SVD routine.
    let q0 = svd.s[0];
    let max_s = q0 * q0 * q0;

    // Diagonal regularization values (a_Sig[i][i]).
    let mut sig_diag: Vec<f64> = vec![0.0; n];
    for i in 0..n {
        let qi = svd.s.get(i).copied().unwrap_or(0.0);
        let qi2 = qi * qi;
        if qi2 == 0.0 {
            sig_diag[i] = 0.0;
            continue;
        }
        sig_diag[i] = qi / (qi2 + (max_s / qi2));
    }

    // INV = V * Sig * U^T
    let mut inv: Vec<Vec<f64>> = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += svd.v[i][k] * sig_diag[k] * svd.u[j][k];
            }
            inv[i][j] = sum;
        }
    }
    Ok(Some(inv))
}

fn power_iteration<W: WorkControl + ?Sized>(
    rng: &mut FcoseRandom,
    c: &[Vec<f64>],
    inv: &[Vec<f64>],
    pi_tol: f64,
    work_control: &mut W,
) -> Result<Option<(Vec<f64>, Vec<f64>)>, WorkFailure> {
    let n = c.len();
    if n == 0 {
        return Ok(None);
    }
    let sample_size = c[0].len();
    if inv.len() != sample_size || inv.iter().any(|r| r.len() != sample_size) {
        return Ok(None);
    }

    // Match upstream `spectral.js` RNG consumption order:
    //
    // ```
    // for(i=0; i<nodeSize; i++){
    //   Y1[i] = Math.random();
    //   Y2[i] = Math.random();
    // }
    // ```
    //
    // Interleaving matters on symmetric graphs: consuming all `Y1` values first and then all
    // `Y2` values yields a different RNG stream split and can rotate/reflect the spectral basis,
    // which cascades into different FCoSE results.
    let mut y1: Vec<f64> = vec![0.0; n];
    let mut y2: Vec<f64> = vec![0.0; n];
    for i in 0..n {
        y1[i] = rng.next_f64_unit();
        y2[i] = rng.next_f64_unit();
    }
    normalize_in_place(&mut y1);
    normalize_in_place(&mut y2);

    let (v1, theta1) = match dominant_eigenvector(c, inv, y1, pi_tol, work_control)? {
        Some(result) => result,
        None => return Ok(None),
    };
    let (v2, theta2) = match second_eigenvector(c, inv, &v1, y2, pi_tol, work_control)? {
        Some(result) => result,
        None => return Ok(None),
    };

    let s1 = theta1.abs().sqrt();
    let s2 = theta2.abs().sqrt();
    let x: Vec<f64> = v1.iter().map(|v| v * s1).collect();
    let y: Vec<f64> = v2.iter().map(|v| v * s2).collect();
    Ok(Some((x, y)))
}

fn dominant_eigenvector<W: WorkControl + ?Sized>(
    c: &[Vec<f64>],
    inv: &[Vec<f64>],
    mut y: Vec<f64>,
    pi_tol: f64,
    work_control: &mut W,
) -> Result<Option<(Vec<f64>, f64)>, WorkFailure> {
    let mut previous = SMALL;
    let mut theta = 0.0;
    let iteration_work = power_iteration_work_units(c.len(), c[0].len(), false)?;

    for _ in 0..MAX_POWER_ITERATIONS {
        work_control.charge(iteration_work)?;
        let v = y.clone();
        let t = mult_gamma(&v);
        let t = mult_l(&t, c, inv);
        let mut next = mult_gamma(&t);
        theta = dot(&v, &next);
        normalize_in_place(&mut next);

        let current = dot(&v, &next);
        let ratio = (current / previous).abs();

        y = next;
        if ratio <= 1.0 + pi_tol && ratio >= 1.0 {
            return Ok(Some((y, theta)));
        }
        previous = current;
        if previous.abs() < SMALL {
            previous = SMALL;
        }
    }

    Ok(Some((y, theta)))
}

fn second_eigenvector<W: WorkControl + ?Sized>(
    c: &[Vec<f64>],
    inv: &[Vec<f64>],
    v1: &[f64],
    mut y: Vec<f64>,
    pi_tol: f64,
    work_control: &mut W,
) -> Result<Option<(Vec<f64>, f64)>, WorkFailure> {
    let mut previous = SMALL;
    let mut theta = 0.0;
    let iteration_work = power_iteration_work_units(c.len(), c[0].len(), true)?;

    for _ in 0..MAX_POWER_ITERATIONS {
        work_control.charge(iteration_work)?;
        let mut v = y.clone();
        let proj = dot(v1, &v);
        for i in 0..v.len() {
            v[i] -= v1[i] * proj;
        }

        let t = mult_gamma(&v);
        let t = mult_l(&t, c, inv);
        let mut next = mult_gamma(&t);
        theta = dot(&v, &next);
        normalize_in_place(&mut next);

        let current = dot(&v, &next);
        let ratio = (current / previous).abs();

        y = next;
        if ratio <= 1.0 + pi_tol && ratio >= 1.0 {
            return Ok(Some((y, theta)));
        }
        previous = current;
        if previous.abs() < SMALL {
            previous = SMALL;
        }
    }

    Ok(Some((y, theta)))
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut sum = 0.0;
    for i in 0..n {
        sum += a[i] * b[i];
    }
    sum
}

fn mult_gamma(v: &[f64]) -> Vec<f64> {
    let n = v.len();
    if n == 0 {
        return Vec::new();
    }
    let mut sum = 0.0;
    for &x in v {
        sum += x;
    }
    let mean = sum / (n as f64);
    let mut out = vec![0.0; n];
    for i in 0..n {
        out[i] = v[i] - mean;
    }
    out
}

fn mult_l(v: &[f64], c: &[Vec<f64>], inv: &[Vec<f64>]) -> Vec<f64> {
    // layout-base `Matrix.multL`:
    // result = -0.5 * C * INV * C^T * v
    let node_size = c.len();
    if node_size == 0 {
        return Vec::new();
    }
    let sample_size = c[0].len();

    let mut temp1 = vec![0.0; sample_size];
    for i in 0..sample_size {
        let mut sum = 0.0;
        for j in 0..node_size {
            sum += -0.5 * c[j][i] * v[j];
        }
        temp1[i] = sum;
    }

    let mut temp2 = vec![0.0; sample_size];
    for i in 0..sample_size {
        let mut sum = 0.0;
        for j in 0..sample_size {
            sum += inv[i][j] * temp1[j];
        }
        temp2[i] = sum;
    }

    let mut out = vec![0.0; node_size];
    for i in 0..node_size {
        let mut sum = 0.0;
        for j in 0..sample_size {
            sum += c[i][j] * temp2[j];
        }
        out[i] = sum;
    }
    out
}

fn normalize_in_place(v: &mut [f64]) {
    let mut sum_sq = 0.0;
    for &x in v.iter() {
        sum_sq += x * x;
    }
    let norm = sum_sq.sqrt();
    if norm.is_finite() && norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn svd_hypot(a: f64, b: f64) -> f64 {
    // layout-base `SVD.hypot`.
    if a.abs() > b.abs() {
        let r = b / a;
        a.abs() * (1.0 + r * r).sqrt()
    } else if b != 0.0 {
        let r = a / b;
        b.abs() * (1.0 + r * r).sqrt()
    } else {
        0.0
    }
}

pub(super) fn svd_jama(a_in: &[Vec<f64>]) -> Option<SvdResult> {
    let mut work_control = NoopWorkControl;
    svd_jama_controlled(a_in, &mut work_control).ok().flatten()
}

fn svd_jama_controlled<W: WorkControl + ?Sized>(
    a_in: &[Vec<f64>],
    work_control: &mut W,
) -> Result<Option<SvdResult>, WorkFailure> {
    let m = a_in.len();
    if m == 0 {
        return Ok(None);
    }
    let n = a_in[0].len();
    if n == 0 || a_in.iter().any(|r| r.len() != n) {
        return Ok(None);
    }

    // The bidiagonalization and U/V construction loops have fixed bounds derived from the input
    // shape. Charge them before cloning the matrix; the convergence-dependent QR phase below is
    // charged once per actual outer iteration.
    work_control.charge(svd_setup_work_units(m, n)?)?;

    let mut a: Vec<Vec<f64>> = a_in.to_vec();

    let nu = m.min(n);
    let mut s: Vec<f64> = vec![0.0; (m + 1).min(n)];
    let mut u: Vec<Vec<f64>> = vec![vec![0.0; nu]; m];
    let mut v: Vec<Vec<f64>> = vec![vec![0.0; n]; n];
    let mut e: Vec<f64> = vec![0.0; n];
    let mut work: Vec<f64> = vec![0.0; m];

    let wantu = true;
    let wantv = true;

    let nct = (m.saturating_sub(1)).min(n);
    let nrt = (n.saturating_sub(2)).min(m);

    let k_max = nct.max(nrt);
    for k in 0..k_max {
        if k < nct {
            s[k] = 0.0;
            for i in k..m {
                s[k] = svd_hypot(s[k], a[i][k]);
            }
            if s[k] != 0.0 {
                if a[k][k] < 0.0 {
                    s[k] = -s[k];
                }
                for i in k..m {
                    a[i][k] /= s[k];
                }
                a[k][k] += 1.0;
            }
            s[k] = -s[k];
        }

        for j in (k + 1)..n {
            if k < nct && s[k] != 0.0 {
                let mut t = 0.0;
                for i in k..m {
                    t += a[i][k] * a[i][j];
                }
                t = -t / a[k][k];
                for i in k..m {
                    a[i][j] += t * a[i][k];
                }
            }
            e[j] = a[k][j];
        }

        if wantu && k < nct {
            for i in k..m {
                u[i][k] = a[i][k];
            }
        }

        if k < nrt {
            e[k] = 0.0;
            for i in (k + 1)..n {
                e[k] = svd_hypot(e[k], e[i]);
            }
            if e[k] != 0.0 {
                if e[k + 1] < 0.0 {
                    e[k] = -e[k];
                }
                for i in (k + 1)..n {
                    e[i] /= e[k];
                }
                e[k + 1] += 1.0;
            }
            e[k] = -e[k];

            if (k + 1) < m && e[k] != 0.0 {
                for i in (k + 1)..m {
                    work[i] = 0.0;
                }
                for j in (k + 1)..n {
                    for i in (k + 1)..m {
                        work[i] += e[j] * a[i][j];
                    }
                }
                for j in (k + 1)..n {
                    let t = -e[j] / e[k + 1];
                    for i in (k + 1)..m {
                        a[i][j] += t * work[i];
                    }
                }
            }

            if wantv {
                for i in (k + 1)..n {
                    v[i][k] = e[i];
                }
            }
        }
    }

    let p = n.min(m + 1);
    if nct < n {
        s[nct] = a[nct][nct];
    }
    if m < p {
        s[p - 1] = 0.0;
    }
    if (nrt + 1) < p {
        e[nrt] = a[nrt][p - 1];
    }
    e[p - 1] = 0.0;

    if wantu {
        for j in nct..nu {
            for i in 0..m {
                u[i][j] = 0.0;
            }
            u[j][j] = 1.0;
        }

        let mut k = nct as i32 - 1;
        while k >= 0 {
            let kk = k as usize;
            if s[kk] != 0.0 {
                for j in (kk + 1)..nu {
                    let mut t = 0.0;
                    for i in kk..m {
                        t += u[i][kk] * u[i][j];
                    }
                    t = -t / u[kk][kk];
                    for i in kk..m {
                        u[i][j] += t * u[i][kk];
                    }
                }
                for i in kk..m {
                    u[i][kk] = -u[i][kk];
                }
                u[kk][kk] += 1.0;
                for i in 0..kk.saturating_sub(1) {
                    u[i][kk] = 0.0;
                }
            } else {
                for i in 0..m {
                    u[i][kk] = 0.0;
                }
                u[kk][kk] = 1.0;
            }
            k -= 1;
        }
    }

    if wantv {
        let mut k = n as i32 - 1;
        while k >= 0 {
            let kk = k as usize;
            if kk < nrt && e[kk] != 0.0 {
                for j in (kk + 1)..nu {
                    let mut t = 0.0;
                    for i in (kk + 1)..n {
                        t += v[i][kk] * v[i][j];
                    }
                    t = -t / v[kk + 1][kk];
                    for i in (kk + 1)..n {
                        v[i][j] += t * v[i][kk];
                    }
                }
            }
            for i in 0..n {
                v[i][kk] = 0.0;
            }
            v[kk][kk] = 1.0;
            k -= 1;
        }
    }

    let mut p_i32 = p as i32;
    let pp = (p - 1) as i32;
    let mut iter = 0i32;
    let max_outer_iterations = p
        .max(1)
        .checked_mul(MAX_SVD_QR_ITERATIONS_PER_VALUE)
        .and_then(|limit| limit.checked_mul(MAX_SVD_QR_PHASES_PER_VALUE))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    let mut outer_iterations = 0usize;
    let eps = 2f64.powi(-52);
    let tiny = 2f64.powi(-966);

    while p_i32 > 0 {
        if outer_iterations >= max_outer_iterations {
            return Ok(None);
        }
        outer_iterations += 1;
        work_control.charge(svd_qr_iteration_work_units(m, n, p_i32 as usize)?)?;

        let mut k: i32;
        let kase: i32;

        k = p_i32 - 2;
        while k >= -1 {
            if k == -1 {
                break;
            }
            let kk = k as usize;
            if e[kk].abs() <= tiny + eps * (s[kk].abs() + s[kk + 1].abs()) {
                e[kk] = 0.0;
                break;
            }
            k -= 1;
        }

        if k == p_i32 - 2 {
            kase = 4;
        } else {
            let mut ks = p_i32 - 1;
            while ks >= k {
                if ks == k {
                    break;
                }
                let ksu = ks as usize;
                let t = (if ks != p_i32 { e[ksu].abs() } else { 0.0 })
                    + (if ks != k + 1 { e[ksu - 1].abs() } else { 0.0 });
                if s[ksu].abs() <= tiny + eps * t {
                    s[ksu] = 0.0;
                    break;
                }
                ks -= 1;
            }

            if ks == k {
                kase = 3;
            } else if ks == p_i32 - 1 {
                kase = 1;
            } else {
                // kase = 2
                k = ks;
                kase = 2;
            }
        }

        k += 1;
        match kase {
            1 => {
                let mut f = e[(p_i32 - 2) as usize];
                e[(p_i32 - 2) as usize] = 0.0;
                let mut j = p_i32 - 2;
                while j >= k {
                    let ju = j as usize;
                    let t = svd_hypot(s[ju], f);
                    let cs = s[ju] / t;
                    let sn = f / t;
                    s[ju] = t;
                    if j != k {
                        f = -sn * e[(j - 1) as usize];
                        e[(j - 1) as usize] *= cs;
                    }
                    if wantv {
                        for i in 0..n {
                            let t2 = cs * v[i][ju] + sn * v[i][(p_i32 - 1) as usize];
                            v[i][(p_i32 - 1) as usize] =
                                -sn * v[i][ju] + cs * v[i][(p_i32 - 1) as usize];
                            v[i][ju] = t2;
                        }
                    }
                    j -= 1;
                }
            }
            2 => {
                let mut f = e[(k - 1) as usize];
                e[(k - 1) as usize] = 0.0;
                let mut j = k;
                while j < p_i32 {
                    let ju = j as usize;
                    let t = svd_hypot(s[ju], f);
                    let cs = s[ju] / t;
                    let sn = f / t;
                    s[ju] = t;
                    f = -sn * e[ju];
                    e[ju] *= cs;
                    if wantu {
                        for i in 0..m {
                            let t2 = cs * u[i][ju] + sn * u[i][(k - 1) as usize];
                            u[i][(k - 1) as usize] = -sn * u[i][ju] + cs * u[i][(k - 1) as usize];
                            u[i][ju] = t2;
                        }
                    }
                    j += 1;
                }
            }
            3 => {
                let scale = s[(p_i32 - 1) as usize]
                    .abs()
                    .max(s[(p_i32 - 2) as usize].abs())
                    .max(e[(p_i32 - 2) as usize].abs())
                    .max(s[k as usize].abs())
                    .max(e[k as usize].abs());
                let sp = s[(p_i32 - 1) as usize] / scale;
                let spm1 = s[(p_i32 - 2) as usize] / scale;
                let epm1 = e[(p_i32 - 2) as usize] / scale;
                let sk = s[k as usize] / scale;
                let ek = e[k as usize] / scale;
                let b = ((spm1 + sp) * (spm1 - sp) + epm1 * epm1) / 2.0;
                let c_val = (sp * epm1) * (sp * epm1);
                let mut shift = 0.0;
                if b != 0.0 || c_val != 0.0 {
                    shift = (b * b + c_val).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c_val / (b + shift);
                }
                let mut f = (sk + sp) * (sk - sp) + shift;
                let mut g = sk * ek;

                let mut j = k;
                while j < p_i32 - 1 {
                    let ju = j as usize;
                    let mut t = svd_hypot(f, g);
                    let mut cs = f / t;
                    let mut sn = g / t;
                    if j != k {
                        e[(j - 1) as usize] = t;
                    }
                    f = cs * s[ju] + sn * e[ju];
                    e[ju] = cs * e[ju] - sn * s[ju];
                    g = sn * s[ju + 1];
                    s[ju + 1] *= cs;
                    if wantv {
                        for i in 0..n {
                            t = cs * v[i][ju] + sn * v[i][ju + 1];
                            v[i][ju + 1] = -sn * v[i][ju] + cs * v[i][ju + 1];
                            v[i][ju] = t;
                        }
                    }

                    t = svd_hypot(f, g);
                    cs = f / t;
                    sn = g / t;
                    s[ju] = t;
                    f = cs * e[ju] + sn * s[ju + 1];
                    s[ju + 1] = -sn * e[ju] + cs * s[ju + 1];
                    g = sn * e[ju + 1];
                    e[ju + 1] *= cs;
                    if wantu && (j as usize) < m.saturating_sub(1) {
                        for i in 0..m {
                            t = cs * u[i][ju] + sn * u[i][ju + 1];
                            u[i][ju + 1] = -sn * u[i][ju] + cs * u[i][ju + 1];
                            u[i][ju] = t;
                        }
                    }
                    j += 1;
                }
                e[(p_i32 - 2) as usize] = f;
                iter += 1;
            }
            4 => {
                let ku = k as usize;
                if s[ku] <= 0.0 {
                    s[ku] = if s[ku] < 0.0 { -s[ku] } else { 0.0 };
                    if wantv {
                        for i in 0..=pp.max(0) as usize {
                            v[i][ku] = -v[i][ku];
                        }
                    }
                }
                while k < pp {
                    let ku = k as usize;
                    if s[ku] >= s[ku + 1] {
                        break;
                    }
                    s.swap(ku, ku + 1);
                    if wantv && (k as usize) < n.saturating_sub(1) {
                        for i in 0..n {
                            v[i].swap(ku + 1, ku);
                        }
                    }
                    if wantu && (k as usize) < m.saturating_sub(1) {
                        for i in 0..m {
                            u[i].swap(ku + 1, ku);
                        }
                    }
                    k += 1;
                }
                iter = 0;
                p_i32 -= 1;
            }
            _ => {}
        }

        // Prevent pathological infinite loops.
        if iter > 10_000 {
            return Ok(None);
        }
    }

    Ok(Some(SvdResult { u, v, s }))
}

#[cfg(test)]
mod tests {
    use super::{WorkControl, WorkFailure, svd_jama_controlled, svd_setup_work_units};

    #[derive(Default)]
    struct RejectAfter {
        accepted: usize,
        limit: usize,
        charges: Vec<usize>,
    }

    impl WorkControl for RejectAfter {
        fn charge(&mut self, units: usize) -> Result<(), WorkFailure> {
            if self.accepted >= self.limit {
                return Err(WorkFailure::Interrupted);
            }
            self.accepted += 1;
            self.charges.push(units);
            Ok(())
        }
    }

    #[test]
    fn controlled_svd_charges_setup_before_the_iterative_phase() {
        let matrix = vec![vec![4.0, 1.0], vec![2.0, 3.0]];
        let mut control = RejectAfter {
            limit: 1,
            ..RejectAfter::default()
        };

        let error = svd_jama_controlled(&matrix, &mut control)
            .expect_err("the first QR tranche must be rejectable");

        assert_eq!(error, WorkFailure::Interrupted);
        assert_eq!(control.charges, vec![svd_setup_work_units(2, 2).unwrap()]);
    }

    #[test]
    fn svd_work_units_fail_closed_on_overflow() {
        assert_eq!(
            svd_setup_work_units(usize::MAX, 2),
            Err(WorkFailure::ArithmeticOverflow)
        );
    }
}
