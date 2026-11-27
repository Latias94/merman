use super::barycenter::{BarycenterEntryIx, SortEntryIx, sort_ix};
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship};
use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul, checked_n_log_n};
use crate::{WorkControl, WorkError};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

mod assembly;
use assembly::{SortAssembly, SubgraphOrder};

#[derive(Debug, Clone, Copy)]
struct WeightedNeighbor {
    original_ix: usize,
    weight: f64,
}

#[derive(Debug)]
struct LayerNode {
    original_ix: Option<usize>,
    parent: Option<usize>,
    children: Vec<usize>,
    use_original_order: bool,
    border_left: Option<usize>,
    border_right: Option<usize>,
}

impl LayerNode {
    fn root() -> Self {
        Self {
            original_ix: None,
            parent: None,
            children: Vec::new(),
            use_original_order: false,
            border_left: None,
            border_right: None,
        }
    }

    fn original(original_ix: usize) -> Self {
        Self {
            original_ix: Some(original_ix),
            parent: None,
            children: Vec::new(),
            use_original_order: true,
            border_left: None,
            border_right: None,
        }
    }
}

#[derive(Debug)]
struct LayerRank {
    nodes: Vec<LayerNode>,
    local_by_original: HashMap<usize, usize>,
    root: usize,
    hierarchy_link_visits: usize,
    child_sort_work: usize,
}

impl LayerRank {
    fn build<N, E, G>(
        g: &Graph<N, E, G>,
        rank: i32,
        nodes_with_rank: &[usize],
    ) -> Result<Self, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let mut layer = Self {
            nodes: vec![LayerNode::root()],
            local_by_original: HashMap::default(),
            root: 0,
            hierarchy_link_visits: 0,
            child_sort_work: 0,
        };
        layer.local_by_original.reserve(nodes_with_rank.len());

        for &original_ix in nodes_with_rank {
            let Some(id) = g.node_id_by_ix(original_ix) else {
                continue;
            };
            let Some(label) = g.node_label_by_ix(original_ix) else {
                continue;
            };

            let has_min_rank = label.has_min_rank();
            let border_left = label
                .border_left_at(rank)
                .as_deref()
                .and_then(|border| g.node_ix(border));
            let border_right = label
                .border_right_at(rank)
                .as_deref()
                .and_then(|border| g.node_ix(border));
            let parent_original = g.parent(id).and_then(|parent| g.node_ix(parent));

            let local_ix = layer.ensure_original(original_ix);
            let parent_local = parent_original
                .map(|parent_ix| layer.ensure_original(parent_ix))
                .unwrap_or(layer.root);
            let node = &mut layer.nodes[local_ix];
            node.use_original_order = !has_min_rank;
            node.border_left = border_left;
            node.border_right = border_right;
            layer.set_parent(local_ix, parent_local);
        }

        let mut visited = vec![false; layer.nodes.len()];
        let mut stack = vec![(layer.root, 0usize)];
        while let Some((local_ix, depth)) = stack.pop() {
            if local_ix >= visited.len() || visited[local_ix] {
                continue;
            }
            visited[local_ix] = true;
            layer.hierarchy_link_visits = checked_add(layer.hierarchy_link_visits, depth)?;
            let children = layer.children(local_ix).to_vec();
            layer.child_sort_work =
                checked_add(layer.child_sort_work, checked_n_log_n(children.len())?)?;
            let child_depth = checked_add(depth, 1)?;
            stack.extend(children.into_iter().rev().map(|child| (child, child_depth)));
        }

        Ok(layer)
    }

    fn ensure_original(&mut self, original_ix: usize) -> usize {
        if let Some(&local_ix) = self.local_by_original.get(&original_ix) {
            return local_ix;
        }
        let local_ix = self.nodes.len();
        self.nodes.push(LayerNode::original(original_ix));
        self.local_by_original.insert(original_ix, local_ix);
        local_ix
    }

    fn set_parent(&mut self, child: usize, parent: usize) {
        let previous = self.nodes.get(child).and_then(|node| node.parent);
        if previous == Some(parent) {
            return;
        }
        if let Some(previous) = previous
            && let Some(children) = self.nodes.get_mut(previous).map(|node| &mut node.children)
        {
            children.retain(|&candidate| candidate != child);
        }
        if let Some(node) = self.nodes.get_mut(child) {
            node.parent = Some(parent);
        }
        if let Some(children) = self.nodes.get_mut(parent).map(|node| &mut node.children) {
            children.push(child);
        }
    }

    fn original_ix(&self, local_ix: usize) -> Option<usize> {
        self.nodes.get(local_ix)?.original_ix
    }

    fn local_ix(&self, original_ix: usize) -> Option<usize> {
        self.local_by_original.get(&original_ix).copied()
    }

    fn children(&self, local_ix: usize) -> &[usize] {
        self.nodes
            .get(local_ix)
            .map(|node| node.children.as_slice())
            .unwrap_or(&[])
    }

    fn parent(&self, local_ix: usize) -> Option<usize> {
        self.nodes.get(local_ix).and_then(|node| node.parent)
    }

    fn order<N, E, G>(&self, g: &Graph<N, E, G>, local_ix: usize) -> Option<usize>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let node = self.nodes.get(local_ix)?;
        if !node.use_original_order {
            return None;
        }
        g.node_label_by_ix(node.original_ix?)?.order()
    }

    fn order_by_original<N, E, G>(
        &self,
        g: &Graph<N, E, G>,
        tracked_orders: &[bool],
        original_ix: usize,
    ) -> Option<usize>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        if !tracked_orders.get(original_ix).copied().unwrap_or(false) {
            return None;
        }
        if let Some(local_ix) = self.local_ix(original_ix) {
            self.order(g, local_ix)
        } else {
            g.node_label_by_ix(original_ix)?.order()
        }
    }
}

#[derive(Debug, Default)]
struct ConstraintGraph {
    outgoing: HashMap<usize, Vec<usize>>,
    seen: HashSet<(usize, usize)>,
}

impl ConstraintGraph {
    fn clear(&mut self) {
        self.outgoing.clear();
        self.seen.clear();
    }

    fn insert(&mut self, from: usize, to: usize) {
        if self.seen.insert((from, to)) {
            self.outgoing.entry(from).or_default().push(to);
        }
    }
}

#[derive(Debug, Default)]
struct SortScratch {
    entry_by_original: Vec<Option<usize>>,
    touched_originals: Vec<usize>,
    resolved: Vec<SortEntryIx>,
    group_members: Vec<usize>,
    sortable: Vec<usize>,
    unsortable: Vec<usize>,
    assembly: SortAssembly,
}

impl SortScratch {
    fn prepare_entries(&mut self, node_slots: usize) {
        if self.entry_by_original.len() < node_slots {
            self.entry_by_original.resize(node_slots, None);
        }
        for original_ix in self.touched_originals.drain(..) {
            self.entry_by_original[original_ix] = None;
        }
    }

    fn clear_entries(&mut self) {
        for original_ix in self.touched_originals.drain(..) {
            self.entry_by_original[original_ix] = None;
        }
    }
}

pub(super) struct OrderWorkspace {
    layers: Vec<LayerRank>,
    in_neighbors: Vec<Vec<WeightedNeighbor>>,
    out_neighbors: Vec<Vec<WeightedNeighbor>>,
    tracked_orders: Vec<bool>,
    constraints: ConstraintGraph,
    scratch: SortScratch,
    in_iteration_work_prefix: Vec<usize>,
    out_iteration_work_prefix: Vec<usize>,
}

impl OrderWorkspace {
    pub(super) fn new<N, E, G>(
        g: &Graph<N, E, G>,
        nodes_by_rank: &[Vec<usize>],
        max_rank: i32,
    ) -> Result<Self, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + OrderEdgeWeight + 'static,
        G: Default,
    {
        let mut node_slots = 0usize;
        g.for_each_node_ix(|node_ix, _id, _node| {
            node_slots = node_slots.max(node_ix.saturating_add(1));
        });

        let mut tracked_orders = vec![false; node_slots];
        g.for_each_node_ix(|node_ix, _id, node| {
            tracked_orders[node_ix] = node.order().is_some();
        });
        let in_neighbors = build_weighted_neighbors(g, node_slots, Relationship::InEdges);
        let out_neighbors = build_weighted_neighbors(g, node_slots, Relationship::OutEdges);
        let rank_count =
            usize::try_from(i64::from(max_rank) + 1).map_err(|_| WorkError::ArithmeticOverflow)?;
        let mut layers = Vec::with_capacity(rank_count);
        for rank_index in 0..rank_count {
            let rank = i32::try_from(rank_index).map_err(|_| WorkError::ArithmeticOverflow)?;
            let nodes = nodes_by_rank
                .get(rank_index)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            layers.push(LayerRank::build(g, rank, nodes)?);
        }

        let mut in_iteration_work_prefix = Vec::with_capacity(layers.len() + 1);
        let mut out_iteration_work_prefix = Vec::with_capacity(layers.len() + 1);
        in_iteration_work_prefix.push(0usize);
        out_iteration_work_prefix.push(0usize);
        for layer in &layers {
            let mut in_neighbor_visits = 0usize;
            let mut out_neighbor_visits = 0usize;
            for node in &layer.nodes {
                let Some(original_ix) = node.original_ix else {
                    continue;
                };
                in_neighbor_visits = checked_add(
                    in_neighbor_visits,
                    in_neighbors.get(original_ix).map_or(0, Vec::len),
                )?;
                out_neighbor_visits = checked_add(
                    out_neighbor_visits,
                    out_neighbors.get(original_ix).map_or(0, Vec::len),
                )?;
            }
            let hierarchy_work = checked_mul(layer.hierarchy_link_visits, 3)?;
            let common_work = checked_add(
                layer.nodes.len(),
                checked_add(hierarchy_work, layer.child_sort_work)?,
            )?;
            let in_work = checked_add(common_work, in_neighbor_visits)?;
            let out_work = checked_add(common_work, out_neighbor_visits)?;
            in_iteration_work_prefix.push(checked_add(
                *in_iteration_work_prefix.last().unwrap_or(&0),
                in_work,
            )?);
            out_iteration_work_prefix.push(checked_add(
                *out_iteration_work_prefix.last().unwrap_or(&0),
                out_work,
            )?);
        }

        Ok(Self {
            layers,
            in_neighbors,
            out_neighbors,
            tracked_orders,
            constraints: ConstraintGraph::default(),
            scratch: SortScratch::default(),
            in_iteration_work_prefix,
            out_iteration_work_prefix,
        })
    }

    pub(super) fn begin_sweep(&mut self) {
        self.constraints.clear();
    }

    pub(super) fn iteration_work_units(
        &self,
        ranks: &[i32],
        relationship: Relationship,
    ) -> Result<usize, WorkError> {
        let Some((&first, &last)) = ranks.first().zip(ranks.last()) else {
            return Ok(0);
        };
        let start = usize::try_from(first.min(last)).map_err(|_| WorkError::ArithmeticOverflow)?;
        let end = usize::try_from(first.max(last))
            .map_err(|_| WorkError::ArithmeticOverflow)?
            .checked_add(1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        let prefix = match relationship {
            Relationship::InEdges => &self.in_iteration_work_prefix,
            Relationship::OutEdges => &self.out_iteration_work_prefix,
        };
        let Some((&before, &after)) = prefix.get(start).zip(prefix.get(end)) else {
            return Err(WorkError::ArithmeticOverflow);
        };
        after
            .checked_sub(before)
            .ok_or(WorkError::ArithmeticOverflow)
    }

    pub(super) fn sort_rank<N, E, G>(
        &mut self,
        g: &Graph<N, E, G>,
        rank: usize,
        relationship: Relationship,
        bias_right: bool,
        work_control: &mut dyn WorkControl,
    ) -> Result<Vec<usize>, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let Some(layer) = self.layers.get(rank) else {
            return Ok(Vec::new());
        };
        let neighbors = match relationship {
            Relationship::InEdges => &self.in_neighbors,
            Relationship::OutEdges => &self.out_neighbors,
        };
        sort_subgraph(
            SortSubgraphContext {
                graph: g,
                layer,
                neighbors,
                tracked_orders: &self.tracked_orders,
                constraints: &self.constraints,
            },
            &mut self.scratch,
            bias_right,
            work_control,
        )
    }

    pub(super) fn original_ix(&self, rank: usize, local_ix: usize) -> Option<usize> {
        self.layers.get(rank)?.original_ix(local_ix)
    }

    pub(super) fn add_constraints(&mut self, rank: usize, sorted: &[usize]) {
        let Some(layer) = self.layers.get(rank) else {
            return;
        };
        add_subgraph_constraints(layer, &mut self.constraints, sorted);
    }
}

fn build_weighted_neighbors<N, E, G>(
    g: &Graph<N, E, G>,
    node_slots: usize,
    relationship: Relationship,
) -> Vec<Vec<WeightedNeighbor>>
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut result: Vec<Vec<WeightedNeighbor>> = (0..node_slots).map(|_| Vec::new()).collect();
    let mut position_by_neighbor = vec![usize::MAX; node_slots];
    let mut touched_neighbors = Vec::new();

    g.for_each_node_ix(|node_ix, id, _node| {
        for neighbor_ix in touched_neighbors.drain(..) {
            position_by_neighbor[neighbor_ix] = usize::MAX;
        }
        let mut add = |neighbor_ix: usize, weight: f64| {
            if neighbor_ix >= node_slots {
                return;
            }
            let position = if position_by_neighbor[neighbor_ix] != usize::MAX {
                position_by_neighbor[neighbor_ix]
            } else {
                let position = result[node_ix].len();
                result[node_ix].push(WeightedNeighbor {
                    original_ix: neighbor_ix,
                    weight: 0.0,
                });
                position_by_neighbor[neighbor_ix] = position;
                touched_neighbors.push(neighbor_ix);
                position
            };
            result[node_ix][position].weight += weight;
        };

        match (g.is_directed(), relationship) {
            (true, Relationship::InEdges) => {
                g.for_each_in_edge_ix(node_ix, None, |from_ix, _to_ix, _key, label| {
                    add(from_ix, label.weight());
                });
            }
            (true, Relationship::OutEdges) => {
                g.for_each_out_edge_ix(node_ix, None, |_from_ix, to_ix, _key, label| {
                    add(to_ix, label.weight());
                });
            }
            (false, Relationship::InEdges) => {
                g.for_each_in_edge(id, None, |key, label| {
                    if let Some(neighbor_ix) = g.node_ix(&key.v) {
                        add(neighbor_ix, label.weight());
                    }
                });
            }
            (false, Relationship::OutEdges) => {
                g.for_each_out_edge(id, None, |key, label| {
                    if let Some(neighbor_ix) = g.node_ix(&key.w) {
                        add(neighbor_ix, label.weight());
                    }
                });
            }
        }
    });

    result
}

fn add_subgraph_constraints(
    layer: &LayerRank,
    constraints: &mut ConstraintGraph,
    sorted: &[usize],
) {
    let mut previous_by_parent: HashMap<usize, usize> = HashMap::default();
    let mut previous_root: Option<usize> = None;

    for &local_ix in sorted {
        let mut child = layer.parent(local_ix);
        while let Some(current) = child {
            let parent = layer.parent(current);
            let previous = if let Some(parent) = parent {
                previous_by_parent.insert(parent, current)
            } else {
                previous_root.replace(current)
            };

            if let Some(previous) = previous
                && previous != current
            {
                if let (Some(from), Some(to)) =
                    (layer.original_ix(previous), layer.original_ix(current))
                {
                    constraints.insert(from, to);
                }
                break;
            }
            child = parent;
        }
    }
}

struct SortSubgraphContext<'a, N: Default + 'static, E: Default + 'static, G: Default> {
    graph: &'a Graph<N, E, G>,
    layer: &'a LayerRank,
    neighbors: &'a [Vec<WeightedNeighbor>],
    tracked_orders: &'a [bool],
    constraints: &'a ConstraintGraph,
}

fn sort_subgraph<N, E, G>(
    context: SortSubgraphContext<'_, N, E, G>,
    scratch: &mut SortScratch,
    bias_right: bool,
    work_control: &mut dyn WorkControl,
) -> Result<Vec<usize>, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    struct Frame {
        local_ix: usize,
        barycenters: Vec<BarycenterEntryIx>,
        border_left: Option<usize>,
        border_right: Option<usize>,
    }

    enum Step {
        Enter(usize),
        Exit(Frame),
    }

    let SortSubgraphContext {
        graph,
        layer,
        neighbors,
        tracked_orders,
        constraints,
    } = context;
    scratch.assembly.begin_rank(layer.nodes.len());
    let mut stack = vec![Step::Enter(layer.root)];

    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(local_ix) => {
                let node = &layer.nodes[local_ix];
                let border_left = node.border_left.and_then(|ix| layer.local_ix(ix));
                let border_right = node.border_right.and_then(|ix| layer.local_ix(ix));
                let mut movable = layer.children(local_ix).to_vec();
                if border_left.is_some() && border_right.is_some() {
                    movable.retain(|candidate| {
                        Some(*candidate) != border_left && Some(*candidate) != border_right
                    });
                }

                let barycenters = barycenters(graph, layer, neighbors, tracked_orders, &movable);
                let mut nested = Vec::new();
                for entry in &barycenters {
                    if !layer.children(entry.v_ix).is_empty() {
                        nested.push(entry.v_ix);
                    }
                }

                stack.push(Step::Exit(Frame {
                    local_ix,
                    barycenters,
                    border_left,
                    border_right,
                }));
                for child in nested.into_iter().rev() {
                    stack.push(Step::Enter(child));
                }
            }
            Step::Exit(mut frame) => {
                for entry in &mut frame.barycenters {
                    let Some(subgraph) = scratch.assembly.subgraph(entry.v_ix) else {
                        continue;
                    };
                    merge_barycenters(entry, subgraph);
                }

                resolve_conflicts(
                    layer,
                    &frame.barycenters,
                    constraints,
                    scratch,
                    neighbors.len(),
                    work_control,
                )?;
                let assembly = &mut scratch.assembly;
                let group_members = &scratch.group_members;
                let borders = frame.border_left.zip(frame.border_right);
                let start = assembly.item_count();
                let mut len = 0;
                if let Some((border_left, _)) = borders {
                    assembly.append_node(border_left);
                    len += 1;
                }
                let (mut barycenter, mut weight) = sort_ix(
                    &scratch.resolved,
                    bias_right,
                    &mut scratch.sortable,
                    &mut scratch.unsortable,
                    |entry| {
                        let added = assembly.append_members(&group_members[entry.members.clone()]);
                        len += added;
                        added
                    },
                );

                if let Some((border_left, border_right)) = borders {
                    assembly.append_node(border_right);
                    len += 1;
                    let left_predecessor = first_neighbor_original(layer, neighbors, border_left);
                    let right_predecessor = first_neighbor_original(layer, neighbors, border_right);
                    if let (Some(left), Some(right)) = (left_predecessor, right_predecessor) {
                        let left_order = layer
                            .order_by_original(graph, tracked_orders, left)
                            .unwrap_or(0) as f64;
                        let right_order = layer
                            .order_by_original(graph, tracked_orders, right)
                            .unwrap_or(0) as f64;
                        let prior_barycenter = barycenter.unwrap_or(0.0);
                        let prior_weight = weight.unwrap_or(0.0);
                        let denominator = prior_weight + 2.0;
                        barycenter = Some(
                            (prior_barycenter * prior_weight + left_order + right_order)
                                / denominator,
                        );
                        weight = Some(denominator);
                    }
                }

                assembly.finish_subgraph(frame.local_ix, start, len, barycenter, weight);
            }
        }
    }

    Ok(scratch.assembly.flatten(layer.root))
}

fn barycenters<N, E, G>(
    g: &Graph<N, E, G>,
    layer: &LayerRank,
    neighbors: &[Vec<WeightedNeighbor>],
    tracked_orders: &[bool],
    movable: &[usize],
) -> Vec<BarycenterEntryIx>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    movable
        .iter()
        .map(|&local_ix| {
            let Some(original_ix) = layer.original_ix(local_ix) else {
                return BarycenterEntryIx {
                    v_ix: local_ix,
                    barycenter: None,
                    weight: None,
                };
            };
            let Some(node_neighbors) = neighbors.get(original_ix) else {
                return BarycenterEntryIx {
                    v_ix: local_ix,
                    barycenter: None,
                    weight: None,
                };
            };
            if node_neighbors.is_empty() {
                return BarycenterEntryIx {
                    v_ix: local_ix,
                    barycenter: None,
                    weight: None,
                };
            }

            let mut sum = 0.0;
            let mut weight = 0.0;
            for neighbor in node_neighbors {
                let order = layer
                    .order_by_original(g, tracked_orders, neighbor.original_ix)
                    .unwrap_or(0) as f64;
                sum += neighbor.weight * order;
                weight += neighbor.weight;
            }
            BarycenterEntryIx {
                v_ix: local_ix,
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

fn first_neighbor_original(
    layer: &LayerRank,
    neighbors: &[Vec<WeightedNeighbor>],
    local_ix: usize,
) -> Option<usize> {
    let original_ix = layer.original_ix(local_ix)?;
    neighbors
        .get(original_ix)?
        .first()
        .map(|edge| edge.original_ix)
}

fn merge_barycenters(target: &mut BarycenterEntryIx, other: &SubgraphOrder) {
    let Some(other_barycenter) = other.barycenter else {
        return;
    };
    let other_weight = other.weight.unwrap_or(0.0);
    if let (Some(barycenter), Some(weight)) = (target.barycenter, target.weight) {
        let denominator = weight + other_weight;
        target.barycenter =
            Some((barycenter * weight + other_barycenter * other_weight) / denominator);
        target.weight = Some(denominator);
    } else {
        target.barycenter = Some(other_barycenter);
        target.weight = Some(other_weight);
    }
}

#[derive(Debug)]
struct ConflictEntry {
    indegree: usize,
    ins: Vec<usize>,
    outs: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

fn resolve_conflicts(
    layer: &LayerRank,
    entries: &[BarycenterEntryIx],
    constraints: &ConstraintGraph,
    scratch: &mut SortScratch,
    node_slots: usize,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    scratch.group_members.clear();
    scratch.resolved.clear();
    scratch.prepare_entries(node_slots);
    let mut conflicts = Vec::with_capacity(entries.len());
    let mut next_in_group = vec![None; entries.len()];
    for (entry_ix, entry) in entries.iter().enumerate() {
        if let Some(original_ix) = layer.original_ix(entry.v_ix) {
            scratch.entry_by_original[original_ix] = Some(entry_ix);
            scratch.touched_originals.push(original_ix);
        }
        conflicts.push(ConflictEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            head: Some(entry_ix),
            tail: Some(entry_ix),
            i: entry_ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    // Pinned Graphlib's global edge scan appends every constraint to its source entry's `out`
    // array. Interleaving edges from different sources cannot affect those per-source arrays, so
    // replay sources in entry order while preserving each source's outgoing insertion order. This
    // is equivalent to Dagre's adjacency construction without collecting and sorting global edge
    // ordinals.
    for (from_entry, entry) in entries.iter().enumerate() {
        let Some(from) = layer.original_ix(entry.v_ix) else {
            continue;
        };
        let Some(outgoing) = constraints.outgoing.get(&from) else {
            continue;
        };
        work_control.charge(outgoing.len())?;
        for &to in outgoing {
            let Some(Some(to_entry)) = scratch.entry_by_original.get(to).copied() else {
                continue;
            };
            conflicts[to_entry].indegree += 1;
            conflicts[from_entry].outs.push(to_entry);
        }
    }
    scratch.clear_entries();

    let mut sources = Vec::new();
    for (entry_ix, entry) in conflicts.iter().enumerate() {
        if entry.indegree == 0 {
            sources.push(entry_ix);
        }
    }

    let mut processed = Vec::new();
    while let Some(entry_ix) = sources.pop() {
        processed.push(entry_ix);
        let ins = std::mem::take(&mut conflicts[entry_ix].ins);
        for incoming in ins.into_iter().rev() {
            if conflicts[incoming].merged {
                continue;
            }
            let should_merge = match (
                conflicts[incoming].barycenter,
                conflicts[entry_ix].barycenter,
            ) {
                (None, _) | (_, None) => true,
                (Some(incoming), Some(current)) => incoming >= current,
            };
            if should_merge {
                merge_conflict_entries(&mut conflicts, &mut next_in_group, entry_ix, incoming);
            }
        }

        let outs = std::mem::take(&mut conflicts[entry_ix].outs);
        for outgoing in outs {
            conflicts[outgoing].ins.push(entry_ix);
            conflicts[outgoing].indegree = conflicts[outgoing].indegree.saturating_sub(1);
            if conflicts[outgoing].indegree == 0 {
                sources.push(outgoing);
            }
        }
    }

    for entry_ix in processed {
        let entry = &conflicts[entry_ix];
        if entry.merged {
            continue;
        }
        let start = scratch.group_members.len();
        let mut current = entry.head;
        while let Some(entry_ix) = current {
            scratch.group_members.push(entries[entry_ix].v_ix);
            current = next_in_group[entry_ix];
        }
        scratch.resolved.push(SortEntryIx {
            members: start..scratch.group_members.len(),
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    Ok(())
}

fn merge_conflict_entries(
    entries: &mut [ConflictEntry],
    next_in_group: &mut [Option<usize>],
    target: usize,
    source: usize,
) {
    if target == source {
        return;
    }
    let (target_entry, source_entry) = if target < source {
        let (left, right) = entries.split_at_mut(source);
        (&mut left[target], &mut right[0])
    } else {
        let (left, right) = entries.split_at_mut(target);
        (&mut right[0], &mut left[source])
    };

    let mut sum = 0.0;
    let mut weight = 0.0;
    if let (Some(barycenter), Some(entry_weight)) = (target_entry.barycenter, target_entry.weight)
        && entry_weight != 0.0
    {
        sum += barycenter * entry_weight;
        weight += entry_weight;
    }
    if let (Some(barycenter), Some(entry_weight)) = (source_entry.barycenter, source_entry.weight)
        && entry_weight != 0.0
    {
        sum += barycenter * entry_weight;
        weight += entry_weight;
    }

    if let Some(source_tail) = source_entry.tail
        && source_tail < next_in_group.len()
    {
        next_in_group[source_tail] = target_entry.head;
    }
    target_entry.head = source_entry.head.or(target_entry.head);
    if target_entry.tail.is_none() {
        target_entry.tail = source_entry.tail;
    }
    source_entry.head = None;
    source_entry.tail = None;
    if weight != 0.0 {
        target_entry.barycenter = Some(sum / weight);
        target_entry.weight = Some(weight);
    }
    target_entry.i = target_entry.i.min(source_entry.i);
    source_entry.merged = true;
}

#[cfg(test)]
mod tests;
