use indexmap::IndexSet;

use super::{FcoseRandom, IndexedFcoseOptions, SimGraph, SimNode};
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub(super) struct Constraints {
    pub(super) align_horizontal: Vec<Vec<usize>>,
    pub(super) align_vertical: Vec<Vec<usize>>,
    pub(super) relative: Vec<RelConstraint>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RelConstraint {
    pub(super) left: Option<usize>,
    pub(super) right: Option<usize>,
    pub(super) top: Option<usize>,
    pub(super) bottom: Option<usize>,
    pub(super) gap: f64,
}

impl Constraints {
    pub(super) fn from_indexed_opts(sim: &SimGraph, opts: &IndexedFcoseOptions) -> Self {
        let mut align_horizontal: Vec<Vec<usize>> = Vec::new();
        let mut align_vertical: Vec<Vec<usize>> = Vec::new();

        if let Some(a) = opts.alignment_constraint.as_ref() {
            align_horizontal = map_indexed_align_lists(&a.horizontal, sim.nodes.len());
            align_vertical = map_indexed_align_lists(&a.vertical, sim.nodes.len());
        }

        let mut relative: Vec<RelConstraint> = Vec::new();
        for r in &opts.relative_placement_constraint {
            relative.push(RelConstraint {
                left: r.left.filter(|idx| *idx < sim.nodes.len()),
                right: r.right.filter(|idx| *idx < sim.nodes.len()),
                top: r.top.filter(|idx| *idx < sim.nodes.len()),
                bottom: r.bottom.filter(|idx| *idx < sim.nodes.len()),
                gap: r.gap.max(0.0),
            });
        }

        Self {
            align_horizontal,
            align_vertical,
            relative,
        }
    }

    pub(super) fn validate(&self, node_count: usize) -> Result<()> {
        validate_axis_constraint_graph(
            node_count,
            &self.align_vertical,
            self.relative.iter().filter_map(|constraint| {
                Some((constraint.left?, constraint.right?, constraint.gap))
            }),
            "horizontal",
        )?;
        validate_axis_constraint_graph(
            node_count,
            &self.align_horizontal,
            self.relative.iter().filter_map(|constraint| {
                Some((constraint.top?, constraint.bottom?, constraint.gap))
            }),
            "vertical",
        )
    }
}

fn validate_axis_constraint_graph(
    node_count: usize,
    alignment_groups: &[Vec<usize>],
    relative: impl Iterator<Item = (usize, usize, f64)> + Clone,
    axis: &'static str,
) -> Result<()> {
    let mut parent: Vec<usize> = (0..node_count).collect();

    fn find(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }

    for group in alignment_groups {
        let Some((&first, rest)) = group.split_first() else {
            continue;
        };
        if first >= node_count {
            continue;
        }
        for &node in rest {
            if node >= node_count {
                continue;
            }
            let first_root = find(&mut parent, first);
            let node_root = find(&mut parent, node);
            if first_root != node_root {
                parent[node_root] = first_root;
            }
        }
    }
    for node in 0..node_count {
        let root = find(&mut parent, node);
        parent[node] = root;
    }

    // The original constraints remain available for the final positive-gap check.
    // Build the SCC adjacency directly from a copy of the borrowed iterator.
    let mut edges = relative
        .filter(|(from, to, _)| *from < node_count && *to < node_count)
        .map(|(from, to, gap)| (parent[from], parent[to], gap.max(0.0)));
    let mut outgoing = vec![Vec::new(); node_count];
    let mut incoming = vec![Vec::new(); node_count];
    for (from, to, _) in edges.clone() {
        outgoing[from].push(to);
        incoming[to].push(from);
    }

    let mut visited = vec![false; node_count];
    let mut finish_order = Vec::with_capacity(node_count);
    for start in 0..node_count {
        if parent[start] != start || visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0usize)];
        while let Some((node, next_edge)) = stack.pop() {
            if let Some(&next) = outgoing[node].get(next_edge) {
                stack.push((node, next_edge + 1));
                if !visited[next] {
                    visited[next] = true;
                    stack.push((next, 0));
                }
            } else {
                finish_order.push(node);
            }
        }
    }

    let mut component = vec![usize::MAX; node_count];
    let mut component_id = 0usize;
    for &start in finish_order.iter().rev() {
        if component[start] != usize::MAX {
            continue;
        }
        component[start] = component_id;
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            for &next in &incoming[node] {
                if component[next] == usize::MAX {
                    component[next] = component_id;
                    stack.push(next);
                }
            }
        }
        component_id += 1;
    }

    if edges.any(|(from, to, gap)| {
        gap > 0.0 && component[from] != usize::MAX && component[from] == component[to]
    }) {
        return Err(Error::InfeasibleConstraints { axis });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub(super) struct ConstraintRuntime {
    horizontal: AxisConstraintRuntime,
    vertical: AxisConstraintRuntime,
}

#[derive(Debug, Clone)]
struct AxisConstraintRuntime {
    node_count: usize,
    dummy_to_nodes: Vec<Vec<usize>>,
    nodes_in_relative: Vec<usize>,
    rel_map: Vec<Vec<AxisRelAdj>>,
    temp_pos: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
enum AxisRelAdj {
    Right { node: usize, gap: f64 },
    Left { node: usize, gap: f64 },
    Bottom { node: usize, gap: f64 },
    Top { node: usize, gap: f64 },
}

impl ConstraintRuntime {
    pub(super) fn new(nodes: &[SimNode], c: &Constraints) -> Option<Self> {
        if c.relative.is_empty() {
            return None;
        }
        Some(Self {
            horizontal: AxisConstraintRuntime::new_axis(
                nodes,
                &c.align_vertical,
                &c.relative,
                Axis::Horizontal,
            ),
            vertical: AxisConstraintRuntime::new_axis(
                nodes,
                &c.align_horizontal,
                &c.relative,
                Axis::Vertical,
            ),
        })
    }

    pub(super) fn update_displacements(
        &mut self,
        c: &Constraints,
        disps: &mut [(f64, f64)],
        total_iterations: usize,
        rng: &mut FcoseRandom,
    ) {
        // Alignments (match `cose-base` updateDisplacements): average displacements per group.
        for group in &c.align_vertical {
            if group.len() <= 1 {
                continue;
            }
            let mut sum = 0.0;
            for &idx in group {
                sum += disps[idx].0;
            }
            let avg = sum / (group.len() as f64);
            for &idx in group {
                disps[idx].0 = avg;
            }
        }
        for group in &c.align_horizontal {
            if group.len() <= 1 {
                continue;
            }
            let mut sum = 0.0;
            for &idx in group {
                sum += disps[idx].1;
            }
            let avg = sum / (group.len() as f64);
            for &idx in group {
                disps[idx].1 = avg;
            }
        }

        // Relative placements (match `cose-base` relax-movement mode).
        // Upstream keeps `nodeToTempPositionMap*` as a persistent accumulator across iterations:
        // it starts from node centers and is advanced by the chosen displacements each tick.
        // Do not re-seed from node centers here, or the relaxation order differs.
        if total_iterations.is_multiple_of(10) {
            self.horizontal.shuffle_tail_third(rng);
            self.vertical.shuffle_tail_third(rng);
        }

        self.horizontal
            .apply_relative_relaxation(disps, Axis::Horizontal);
        self.vertical
            .apply_relative_relaxation(disps, Axis::Vertical);
    }
}

impl AxisConstraintRuntime {
    fn new_axis(
        nodes: &[SimNode],
        axis_alignment_groups: &[Vec<usize>],
        rel: &[RelConstraint],
        axis: Axis,
    ) -> Self {
        let n = nodes.len();
        let d = axis_alignment_groups.len();

        let mut node_to_dummy: Vec<Option<usize>> = vec![None; n];
        let mut dummy_to_nodes: Vec<Vec<usize>> = Vec::with_capacity(d);
        for (i, group) in axis_alignment_groups.iter().enumerate() {
            let dummy_key = n + i;
            dummy_to_nodes.push(group.clone());
            for &idx in group {
                if idx < n {
                    node_to_dummy[idx] = Some(dummy_key);
                }
            }
        }

        let key_count = n + d;
        let mut rel_map: Vec<Vec<AxisRelAdj>> = vec![Vec::new(); key_count];
        let mut nodes_in_relative_set: IndexSet<usize> = IndexSet::new();

        for r in rel {
            match axis {
                Axis::Horizontal => {
                    let (Some(left), Some(right)) = (r.left, r.right) else {
                        continue;
                    };
                    let lk = node_to_dummy.get(left).copied().flatten().unwrap_or(left);
                    let rk = node_to_dummy.get(right).copied().flatten().unwrap_or(right);
                    nodes_in_relative_set.insert(lk);
                    nodes_in_relative_set.insert(rk);
                    rel_map[lk].push(AxisRelAdj::Right {
                        node: rk,
                        gap: r.gap,
                    });
                    rel_map[rk].push(AxisRelAdj::Left {
                        node: lk,
                        gap: r.gap,
                    });
                }
                Axis::Vertical => {
                    let (Some(top), Some(bottom)) = (r.top, r.bottom) else {
                        continue;
                    };
                    let tk = node_to_dummy.get(top).copied().flatten().unwrap_or(top);
                    let bk = node_to_dummy
                        .get(bottom)
                        .copied()
                        .flatten()
                        .unwrap_or(bottom);
                    nodes_in_relative_set.insert(tk);
                    nodes_in_relative_set.insert(bk);
                    rel_map[tk].push(AxisRelAdj::Bottom {
                        node: bk,
                        gap: r.gap,
                    });
                    rel_map[bk].push(AxisRelAdj::Top {
                        node: tk,
                        gap: r.gap,
                    });
                }
            }
        }

        let mut rt = Self {
            node_count: n,
            dummy_to_nodes,
            nodes_in_relative: nodes_in_relative_set.into_iter().collect(),
            rel_map,
            temp_pos: vec![0.0; key_count],
        };
        rt.refresh_temp_positions(nodes, axis);
        rt
    }

    fn refresh_temp_positions(&mut self, nodes: &[SimNode], axis: Axis) {
        let n = self.node_count;
        for key in 0..self.temp_pos.len() {
            let v = if key < n {
                match axis {
                    Axis::Horizontal => nodes[key].center_x(),
                    Axis::Vertical => nodes[key].center_y(),
                }
            } else {
                let dummy_idx = key - n;
                let first = self.dummy_to_nodes[dummy_idx]
                    .first()
                    .copied()
                    .unwrap_or(0)
                    .min(n.saturating_sub(1));
                match axis {
                    Axis::Horizontal => nodes[first].center_x(),
                    Axis::Vertical => nodes[first].center_y(),
                }
            };
            self.temp_pos[key] = v;
        }
    }

    fn shuffle_tail_third(&mut self, rng: &mut FcoseRandom) {
        let len = self.nodes_in_relative.len();
        if len <= 1 {
            return;
        }
        // Upstream (`cose-base`) uses:
        //
        // `for (i = len - 1; i >= (2 * len / 3); i--)`
        //
        // where `(2 * len / 3)` is a JS Number (not integer division). Therefore the effective
        // lower bound is `ceil(2 * len / 3)`.
        let start = (2 * len).div_ceil(3);
        for i in (start..len).rev() {
            let j = rng.next_usize(i + 1);
            self.nodes_in_relative.swap(i, j);
        }
    }

    fn apply_relative_relaxation(&mut self, disps: &mut [(f64, f64)], axis: Axis) {
        let n = self.node_count;

        for &key in &self.nodes_in_relative {
            let mut displacement = if key < n {
                match axis {
                    Axis::Horizontal => disps[key].0,
                    Axis::Vertical => disps[key].1,
                }
            } else {
                let dummy_idx = key - n;
                let first = self.dummy_to_nodes[dummy_idx]
                    .first()
                    .copied()
                    .unwrap_or(0)
                    .min(n.saturating_sub(1));
                match axis {
                    Axis::Horizontal => disps[first].0,
                    Axis::Vertical => disps[first].1,
                }
            };

            for adj in &self.rel_map[key] {
                match (*adj, axis) {
                    (AxisRelAdj::Right { node, gap }, Axis::Horizontal) => {
                        let diff = (self.temp_pos[node] - self.temp_pos[key]) - displacement;
                        if diff < gap {
                            displacement -= gap - diff;
                        }
                    }
                    (AxisRelAdj::Left { node, gap }, Axis::Horizontal) => {
                        let diff = (self.temp_pos[key] - self.temp_pos[node]) + displacement;
                        if diff < gap {
                            displacement += gap - diff;
                        }
                    }
                    (AxisRelAdj::Bottom { node, gap }, Axis::Vertical) => {
                        let diff = (self.temp_pos[node] - self.temp_pos[key]) - displacement;
                        if diff < gap {
                            displacement -= gap - diff;
                        }
                    }
                    (AxisRelAdj::Top { node, gap }, Axis::Vertical) => {
                        let diff = (self.temp_pos[key] - self.temp_pos[node]) + displacement;
                        if diff < gap {
                            displacement += gap - diff;
                        }
                    }
                    _ => {}
                }
            }

            self.temp_pos[key] += displacement;

            if key < n {
                match axis {
                    Axis::Horizontal => disps[key].0 = displacement,
                    Axis::Vertical => disps[key].1 = displacement,
                }
            } else {
                let dummy_idx = key - n;
                for &idx in &self.dummy_to_nodes[dummy_idx] {
                    if idx >= disps.len() {
                        continue;
                    }
                    match axis {
                        Axis::Horizontal => disps[idx].0 = displacement,
                        Axis::Vertical => disps[idx].1 = displacement,
                    }
                }
            }
        }
    }
}

fn map_indexed_align_lists(groups: &[Vec<usize>], node_count: usize) -> Vec<Vec<usize>> {
    // Preserve Mermaid/Cytoscape ordering (and duplicates) for alignment arrays.
    //
    // Upstream `ConstraintHandler` uses the *first* node id in each alignment group as the seed
    // for dummy-node positions in the relative-placement enforcement phase. Sorting/deduping
    // here changes that seed and can shift the entire layout in parity-root mode.
    let mut out: Vec<Vec<usize>> = Vec::new();
    for g in groups {
        let idxs: Vec<usize> = g.iter().copied().filter(|idx| *idx < node_count).collect();
        if idxs.len() > 1 {
            out.push(idxs);
        }
    }
    out
}
