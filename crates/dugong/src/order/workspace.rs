use super::barycenter::{BarycenterEntryIx, SortEntryIx, SortResultIx, sort_ix};
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship};
use crate::graphlib::Graph;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

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
}

impl LayerRank {
    fn build<N, E, G>(g: &Graph<N, E, G>, rank: i32, nodes_with_rank: &[usize]) -> Self
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let mut layer = Self {
            nodes: vec![LayerNode::root()],
            local_by_original: HashMap::default(),
            root: 0,
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

        layer
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
    edges: Vec<(usize, usize)>,
    seen: HashSet<(usize, usize)>,
}

impl ConstraintGraph {
    fn clear(&mut self) {
        self.edges.clear();
        self.seen.clear();
    }

    fn insert(&mut self, from: usize, to: usize) {
        if self.seen.insert((from, to)) {
            self.edges.push((from, to));
        }
    }
}

#[derive(Debug, Default)]
struct SortScratch {
    entry_by_original: Vec<Option<usize>>,
    touched_originals: Vec<usize>,
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
}

impl OrderWorkspace {
    pub(super) fn new<N, E, G>(
        g: &Graph<N, E, G>,
        nodes_by_rank: &[Vec<usize>],
        max_rank: i32,
    ) -> Self
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
        let mut layers = Vec::with_capacity((max_rank + 1).max(0) as usize);
        for rank in 0..=max_rank {
            let nodes = nodes_by_rank
                .get(rank as usize)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            layers.push(LayerRank::build(g, rank, nodes));
        }

        Self {
            layers,
            in_neighbors,
            out_neighbors,
            tracked_orders,
            constraints: ConstraintGraph::default(),
            scratch: SortScratch::default(),
        }
    }

    pub(super) fn begin_sweep(&mut self) {
        self.constraints.clear();
    }

    pub(super) fn sort_rank<N, E, G>(
        &mut self,
        g: &Graph<N, E, G>,
        rank: usize,
        relationship: Relationship,
        bias_right: bool,
    ) -> SortResultIx
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let Some(layer) = self.layers.get(rank) else {
            return SortResultIx {
                vs: Vec::new(),
                barycenter: None,
                weight: None,
            };
        };
        let neighbors = match relationship {
            Relationship::InEdges => &self.in_neighbors,
            Relationship::OutEdges => &self.out_neighbors,
        };
        sort_subgraph(
            g,
            layer,
            neighbors,
            &self.tracked_orders,
            &self.constraints,
            &mut self.scratch,
            bias_right,
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

fn sort_subgraph<N, E, G>(
    g: &Graph<N, E, G>,
    layer: &LayerRank,
    neighbors: &[Vec<WeightedNeighbor>],
    tracked_orders: &[bool],
    constraints: &ConstraintGraph,
    scratch: &mut SortScratch,
    bias_right: bool,
) -> SortResultIx
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

    let mut results: Vec<Option<SortResultIx>> = (0..layer.nodes.len()).map(|_| None).collect();
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

                let barycenters = barycenters(g, layer, neighbors, tracked_orders, &movable);
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
                    let Some(subgraph) = results[entry.v_ix].as_ref() else {
                        continue;
                    };
                    merge_barycenters(entry, subgraph);
                }

                let mut entries = resolve_conflicts(
                    layer,
                    &frame.barycenters,
                    constraints,
                    scratch,
                    neighbors.len(),
                );
                expand_subgraphs(&mut entries, &results);
                for entry in &frame.barycenters {
                    results[entry.v_ix].take();
                }
                let mut result = sort_ix(&entries, bias_right);

                if let (Some(border_left), Some(border_right)) =
                    (frame.border_left, frame.border_right)
                {
                    let mut ordered = Vec::with_capacity(result.vs.len() + 2);
                    ordered.push(border_left);
                    ordered.extend(result.vs);
                    ordered.push(border_right);
                    result.vs = ordered;

                    let left_predecessor = first_neighbor_original(layer, neighbors, border_left);
                    let right_predecessor = first_neighbor_original(layer, neighbors, border_right);
                    if let (Some(left), Some(right)) = (left_predecessor, right_predecessor) {
                        let left_order = layer
                            .order_by_original(g, tracked_orders, left)
                            .unwrap_or(0) as f64;
                        let right_order = layer
                            .order_by_original(g, tracked_orders, right)
                            .unwrap_or(0) as f64;
                        let barycenter = result.barycenter.unwrap_or(0.0);
                        let weight = result.weight.unwrap_or(0.0);
                        let denominator = weight + 2.0;
                        result.barycenter =
                            Some((barycenter * weight + left_order + right_order) / denominator);
                        result.weight = Some(denominator);
                    }
                }

                results[frame.local_ix] = Some(result);
            }
        }
    }

    results[layer.root].take().unwrap_or(SortResultIx {
        vs: Vec::new(),
        barycenter: None,
        weight: None,
    })
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

fn merge_barycenters(target: &mut BarycenterEntryIx, other: &SortResultIx) {
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

fn expand_subgraphs(entries: &mut [SortEntryIx], subgraphs: &[Option<SortResultIx>]) {
    for entry in entries {
        let mut expanded = Vec::new();
        for local_ix in std::mem::take(&mut entry.vs) {
            if let Some(Some(subgraph)) = subgraphs.get(local_ix) {
                expanded.extend(subgraph.vs.iter().copied());
            } else {
                expanded.push(local_ix);
            }
        }
        entry.vs = expanded;
    }
}

#[derive(Debug)]
struct ConflictEntry {
    indegree: usize,
    ins: Vec<usize>,
    outs: Vec<usize>,
    vs: Vec<usize>,
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
) -> Vec<SortEntryIx> {
    scratch.prepare_entries(node_slots);
    let mut conflicts = Vec::with_capacity(entries.len());
    for (entry_ix, entry) in entries.iter().enumerate() {
        if let Some(original_ix) = layer.original_ix(entry.v_ix) {
            scratch.entry_by_original[original_ix] = Some(entry_ix);
            scratch.touched_originals.push(original_ix);
        }
        conflicts.push(ConflictEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            vs: vec![entry_ix],
            i: entry_ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    for &(from, to) in &constraints.edges {
        let Some(Some(from_entry)) = scratch.entry_by_original.get(from).copied() else {
            continue;
        };
        let Some(Some(to_entry)) = scratch.entry_by_original.get(to).copied() else {
            continue;
        };
        conflicts[to_entry].indegree += 1;
        conflicts[from_entry].outs.push(to_entry);
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
                merge_conflict_entries(&mut conflicts, entry_ix, incoming);
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

    let mut resolved = Vec::new();
    for entry_ix in processed {
        let entry = &conflicts[entry_ix];
        if entry.merged {
            continue;
        }
        resolved.push(SortEntryIx {
            vs: entry.vs.iter().map(|&ix| entries[ix].v_ix).collect(),
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    resolved
}

fn merge_conflict_entries(entries: &mut [ConflictEntry], target: usize, source: usize) {
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

    let mut merged = std::mem::take(&mut source_entry.vs);
    merged.extend(std::mem::take(&mut target_entry.vs));
    target_entry.vs = merged;
    if weight != 0.0 {
        target_entry.barycenter = Some(sum / weight);
        target_entry.weight = Some(weight);
    }
    target_entry.i = target_entry.i.min(source_entry.i);
    source_entry.merged = true;
}
