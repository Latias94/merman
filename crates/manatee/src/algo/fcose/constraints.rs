use indexmap::IndexSet;

use super::{FcoseRandom, IndexedFcoseOptions, SimGraph, SimNode, WorkControl};
use crate::error::{Error, Result, WorkFailure};
use crate::work::admit_dynamic_work;

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
    pub(super) fn from_indexed_opts(node_count: usize, opts: &IndexedFcoseOptions) -> Self {
        let mut align_horizontal: Vec<Vec<usize>> = Vec::new();
        let mut align_vertical: Vec<Vec<usize>> = Vec::new();

        if let Some(a) = opts.alignment_constraint.as_ref() {
            align_horizontal = map_indexed_align_lists(&a.horizontal, node_count);
            align_vertical = map_indexed_align_lists(&a.vertical, node_count);
        }

        let mut relative: Vec<RelConstraint> = Vec::new();
        for r in &opts.relative_placement_constraint {
            relative.push(RelConstraint {
                left: r.left.filter(|idx| *idx < node_count),
                right: r.right.filter(|idx| *idx < node_count),
                top: r.top.filter(|idx| *idx < node_count),
                bottom: r.bottom.filter(|idx| *idx < node_count),
                gap: r.gap.max(0.0),
            });
        }

        Self {
            align_horizontal,
            align_vertical,
            relative,
        }
    }

    pub(super) fn normalize<W: WorkControl + ?Sized>(
        &mut self,
        node_count: usize,
        work_control: &mut W,
    ) -> Result<bool> {
        let horizontal = normalize_axis_constraint_graph(
            node_count,
            &mut self.align_vertical,
            self.relative.iter().filter_map(|constraint| {
                Some((constraint.left?, constraint.right?, constraint.gap))
            }),
            self.relative.len(),
            "horizontal",
            work_control,
        )?;
        let vertical = normalize_axis_constraint_graph(
            node_count,
            &mut self.align_horizontal,
            self.relative.iter().filter_map(|constraint| {
                Some((constraint.top?, constraint.bottom?, constraint.gap))
            }),
            self.relative.len(),
            "vertical",
            work_control,
        )?;
        if horizontal.is_none() && vertical.is_none() {
            return Ok(false);
        }

        self.relative.retain_mut(|constraint| {
            let remove_horizontal =
                same_component(horizontal.as_deref(), constraint.left.zip(constraint.right));
            let remove_vertical =
                same_component(vertical.as_deref(), constraint.top.zip(constraint.bottom));
            if remove_horizontal {
                constraint.left = None;
                constraint.right = None;
            }
            if remove_vertical {
                constraint.top = None;
                constraint.bottom = None;
            }
            !(remove_horizontal || remove_vertical)
                || constraint.left.zip(constraint.right).is_some()
                || constraint.top.zip(constraint.bottom).is_some()
        });
        Ok(true)
    }

    pub(super) fn projection_node_count(&self, sim: &SimGraph) -> Result<usize> {
        if sim.leaf_count == sim.nodes.len() {
            return Ok(sim.leaf_count);
        }
        let aligned = self
            .align_horizontal
            .iter()
            .chain(&self.align_vertical)
            .flatten()
            .copied();
        let relative = self.relative.iter().flat_map(|constraint| {
            [
                constraint.left.zip(constraint.right),
                constraint.top.zip(constraint.bottom),
            ]
            .into_iter()
            .flatten()
            .flat_map(|(first, second)| [first, second])
        });
        let mut has_compound_target = false;
        for index in aligned
            .chain(relative)
            .filter(|&node| node >= sim.leaf_count)
        {
            if !sim.children_by_owner[index].is_empty() {
                return Err(Error::UnsupportedCompoundConstraint { index });
            }
            has_compound_target = true;
        }
        Ok(if has_compound_target {
            sim.nodes.len()
        } else {
            sim.leaf_count
        })
    }
}

fn same_component(components: Option<&[usize]>, pair: Option<(usize, usize)>) -> bool {
    components
        .zip(pair)
        .is_some_and(|(components, (from, to))| components[from] == components[to])
}

fn normalize_axis_constraint_graph<W: WorkControl + ?Sized>(
    node_count: usize,
    alignment_groups: &mut Vec<Vec<usize>>,
    relative: impl Iterator<Item = (usize, usize, f64)> + Clone,
    relative_count: usize,
    axis: &'static str,
    work_control: &mut W,
) -> Result<Option<Vec<usize>>> {
    let mut edges = relative.peekable();
    if edges.peek().is_none() {
        return Ok(None);
    }
    let mut parent: Vec<usize> = (0..node_count).collect();

    fn find(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }

    for group in alignment_groups.iter() {
        let Some((&first, rest)) = group.split_first() else {
            continue;
        };
        for &node in rest {
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

    let mut outgoing = vec![Vec::new(); node_count];
    let mut incoming = vec![Vec::new(); node_count];
    for (from, to, _) in edges.clone() {
        let (from, to) = (parent[from], parent[to]);
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

    let mut has_internal_edges = false;
    let mut has_new_alignments = false;
    let mut has_aliased_endpoints = false;
    for (from, to, gap) in edges.clone() {
        if component[parent[from]] != component[parent[to]] {
            continue;
        }
        if gap > 0.0 {
            return Err(Error::InfeasibleConstraints { axis });
        }
        has_internal_edges = true;
        has_new_alignments |= parent[from] != parent[to];
        has_aliased_endpoints |= from != to && parent[from] == parent[to];
    }
    if !has_internal_edges {
        return Ok(None);
    }

    const NO_GROUP: usize = usize::MAX;
    const OVERLAPPING_GROUPS: usize = usize::MAX - 1;
    if has_aliased_endpoints {
        let comparison_work = alignment_groups
            .iter()
            .try_fold(alignment_groups.len(), |work, group| {
                work.checked_add(group.len())
            })
            .and_then(|work| work.checked_add(node_count))
            .and_then(|work| work.checked_add(relative_count))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        admit_dynamic_work(work_control, comparison_work)?;
        finish_order.clear();
        finish_order.resize(node_count, NO_GROUP);
        for (group_index, group) in alignment_groups.iter().enumerate() {
            let Some(&first) = group.first() else {
                continue;
            };
            let authored = &mut finish_order[parent[first]];
            match *authored {
                NO_GROUP => *authored = group_index,
                OVERLAPPING_GROUPS => {}
                previous if alignment_groups[previous] != *group => {
                    *authored = OVERLAPPING_GROUPS;
                }
                _ => {}
            }
        }
    }
    let requires_merge = |from: usize, to: usize| {
        component[parent[from]] == component[parent[to]]
            && from != to
            && (parent[from] != parent[to]
                || (has_aliased_endpoints && finish_order[parent[from]] == OVERLAPPING_GROUPS))
    };
    if has_aliased_endpoints && !has_new_alignments {
        has_new_alignments = edges.clone().any(|(from, to, _)| requires_merge(from, to));
    }

    let mut normalization_work = node_count
        .checked_add(relative_count)
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    if has_new_alignments {
        normalization_work = normalization_work
            .checked_add(component_id)
            .and_then(|work| work.checked_add(node_count))
            .and_then(|work| work.checked_add(relative_count))
            .and_then(|work| work.checked_add(alignment_groups.len()))
            .ok_or(WorkFailure::ArithmeticOverflow)?;
    }
    admit_dynamic_work(work_control, normalization_work)?;

    if has_new_alignments {
        let mut merged = vec![None; component_id];
        for (from, to, _) in edges {
            if requires_merge(from, to) {
                merged[component[parent[from]]].get_or_insert_with(Vec::new);
            }
        }
        for (node, &root) in parent.iter().enumerate() {
            if let Some(group) = &mut merged[component[root]] {
                group.push(node);
            }
        }
        alignment_groups.retain(|group| {
            group
                .first()
                .is_none_or(|&first| merged[component[parent[first]]].is_none())
        });
        alignment_groups.extend(merged.into_iter().flatten());
    }
    for (node, root) in parent.into_iter().enumerate() {
        component[node] = component[root];
    }
    Ok(Some(component))
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
