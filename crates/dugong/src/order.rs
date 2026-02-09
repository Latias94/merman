//! Node ordering / crossing minimization.
//!
//! Ported from Dagre's `order` pipeline: barycenters, conflict resolution, and a sweep heuristic
//! that attempts to minimize edge crossings.

use crate::graphlib::{Graph, GraphOptions};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    InEdges,
    OutEdges,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerGraphLabel {
    pub root: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeightLabel {
    pub weight: f64,
}

pub trait OrderEdgeWeight {
    fn weight(&self) -> f64;
}

impl OrderEdgeWeight for WeightLabel {
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl OrderEdgeWeight for crate::EdgeLabel {
    fn weight(&self) -> f64 {
        self.weight
    }
}

pub trait OrderNodeRange {
    fn rank(&self) -> Option<i32>;
    fn min_rank(&self) -> Option<i32>;
    fn max_rank(&self) -> Option<i32>;
    fn has_min_rank(&self) -> bool {
        self.min_rank().is_some()
    }
    fn border_left_at(&self, _rank: i32) -> Option<String> {
        None
    }
    fn border_right_at(&self, _rank: i32) -> Option<String> {
        None
    }
    fn subgraph_layer_label(&self, _rank: i32) -> Self
    where
        Self: Sized,
    {
        unreachable!("subgraph_layer_label not implemented for this node label type")
    }
}

impl OrderNodeRange for crate::NodeLabel {
    fn rank(&self) -> Option<i32> {
        self.rank
    }

    fn min_rank(&self) -> Option<i32> {
        self.min_rank
    }

    fn max_rank(&self) -> Option<i32> {
        self.max_rank
    }

    fn has_min_rank(&self) -> bool {
        self.min_rank.is_some()
    }

    fn border_left_at(&self, rank: i32) -> Option<String> {
        self.border_left.get(rank as usize).cloned().unwrap_or(None)
    }

    fn border_right_at(&self, rank: i32) -> Option<String> {
        self.border_right
            .get(rank as usize)
            .cloned()
            .unwrap_or(None)
    }

    fn subgraph_layer_label(&self, rank: i32) -> Self {
        let left = self.border_left_at(rank);
        let right = self.border_right_at(rank);

        Self {
            border_left: vec![left],
            border_right: vec![right],
            ..Default::default()
        }
    }
}

pub trait OrderNodeLabel: OrderNodeRange {
    fn order(&self) -> Option<usize>;
    fn set_order(&mut self, order: usize);

    fn border_left(&self) -> Option<&str> {
        None
    }

    fn border_right(&self) -> Option<&str> {
        None
    }
}

impl OrderNodeLabel for crate::NodeLabel {
    fn order(&self) -> Option<usize> {
        self.order
    }

    fn set_order(&mut self, order: usize) {
        self.order = Some(order);
    }

    fn border_left(&self) -> Option<&str> {
        self.border_left.first().and_then(|v| v.as_deref())
    }

    fn border_right(&self) -> Option<&str> {
        self.border_right.first().and_then(|v| v.as_deref())
    }
}

pub fn build_layer_graph<N, E, G>(
    g: &Graph<N, E, G>,
    rank: i32,
    relationship: Relationship,
    nodes_with_rank: Option<&[String]>,
) -> Graph<N, WeightLabel, LayerGraphLabel>
where
    N: Default + Clone + 'static + OrderNodeRange,
    E: Default + 'static + OrderEdgeWeight,
    G: Default,
{
    let root = create_root_node(g);
    let mut result: Graph<N, WeightLabel, LayerGraphLabel> = Graph::new(GraphOptions {
        compound: true,
        multigraph: false,
        ..Default::default()
    });
    result.set_graph(LayerGraphLabel { root: root.clone() });
    result.set_node(root.clone(), N::default());

    let nodes: Vec<String> = match nodes_with_rank {
        Some(vs) => vs.to_vec(),
        None => g.nodes().map(|v| v.to_string()).collect(),
    };

    for v in nodes {
        let node = g.node(&v).cloned().unwrap_or_default();
        let parent = g.parent(&v).map(|p| p.to_string());

        let in_range = node.rank() == Some(rank)
            || (node.min_rank().is_some()
                && node.max_rank().is_some()
                && node.min_rank().is_some_and(|min| min <= rank)
                && node.max_rank().is_some_and(|max| rank <= max));

        if !in_range {
            continue;
        }

        result.set_node(v.clone(), node.clone());
        result.set_parent(v.clone(), parent.unwrap_or_else(|| root.clone()));

        let incident_edges = match relationship {
            Relationship::InEdges => g.in_edges(&v, None),
            Relationship::OutEdges => g.out_edges(&v, None),
        };

        for e in incident_edges {
            let u = if e.v == v { e.w.clone() } else { e.v.clone() };

            if !result.has_node(&u) {
                let lbl = g.node(&u).cloned().unwrap_or_default();
                result.set_node(u.clone(), lbl);
            }

            let existing_weight = result.edge(&u, &v, None).map(|l| l.weight).unwrap_or(0.0);
            let weight = g.edge_by_key(&e).map(|l| l.weight()).unwrap_or(0.0);
            result.set_edge_with_label(
                u,
                v.clone(),
                WeightLabel {
                    weight: weight + existing_weight,
                },
            );
        }

        if node.has_min_rank() {
            let override_label = node.subgraph_layer_label(rank);
            result.set_node(v, override_label);
        }
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct BarycenterEntry {
    pub v: String,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

pub fn barycenter<N, E, G>(g: &Graph<N, E, G>, movable: &[String]) -> Vec<BarycenterEntry>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    movable
        .iter()
        .map(|v| {
            let in_edges = g.in_edges(v, None);
            if in_edges.is_empty() {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            }

            let mut sum: f64 = 0.0;
            let mut weight: f64 = 0.0;
            for e in in_edges {
                let edge_weight = g.edge_by_key(&e).map(|e| e.weight()).unwrap_or(0.0);
                let u_order = g
                    .node(&e.v)
                    .and_then(|n| n.order())
                    .map(|n| n as f64)
                    .unwrap_or(0.0);
                sum += edge_weight * u_order;
                weight += edge_weight;
            }

            BarycenterEntry {
                v: v.clone(),
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortEntry {
    pub vs: Vec<String>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
struct ConflictEntry {
    indegree: usize,
    ins: Vec<String>,
    outs: Vec<String>,
    vs: Vec<String>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

pub fn resolve_conflicts<N, E, G>(
    entries: &[BarycenterEntry],
    cg: &Graph<N, E, G>,
) -> Vec<SortEntry>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut mapped: HashMap<String, ConflictEntry> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        mapped.insert(
            entry.v.clone(),
            ConflictEntry {
                indegree: 0,
                ins: Vec::new(),
                outs: Vec::new(),
                vs: vec![entry.v.clone()],
                i,
                barycenter: entry.barycenter,
                weight: entry.weight,
                merged: false,
            },
        );
    }

    for e in cg.edges() {
        let Some(_) = mapped.get(&e.v) else {
            continue;
        };
        let Some(_) = mapped.get(&e.w) else {
            continue;
        };

        if let Some(w_entry) = mapped.get_mut(&e.w) {
            w_entry.indegree += 1;
        }
        if let Some(v_entry) = mapped.get_mut(&e.v) {
            v_entry.outs.push(e.w.clone());
        }
    }

    let mut source_set: Vec<String> = mapped
        .iter()
        .filter_map(|(k, v)| {
            if v.indegree == 0 {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect();

    let mut processed: Vec<String> = Vec::new();
    while let Some(v) = source_set.pop() {
        processed.push(v.clone());

        let ins = mapped.get(&v).map(|e| e.ins.clone()).unwrap_or_default();

        // Match upstream `.reverse().forEach(...)` on the "in" list.
        for u in ins.into_iter().rev() {
            if mapped.get(&u).map(|e| e.merged).unwrap_or(true) {
                continue;
            }
            let (u_bary, v_bary) = {
                let Some(u_entry) = mapped.get(&u) else {
                    continue;
                };
                let Some(v_entry) = mapped.get(&v) else {
                    continue;
                };
                (u_entry.barycenter, v_entry.barycenter)
            };
            let should_merge = match (u_bary, v_bary) {
                (None, _) => true,
                (_, None) => true,
                (Some(ub), Some(vb)) => ub >= vb,
            };
            if should_merge {
                merge_conflict_entries(&mut mapped, &v, &u);
            }
        }

        let outs = mapped.get(&v).map(|e| e.outs.clone()).unwrap_or_default();
        for w in outs {
            if let Some(w_entry) = mapped.get_mut(&w) {
                w_entry.ins.push(v.clone());
            }
            let w_indegree = {
                let Some(w_entry) = mapped.get_mut(&w) else {
                    continue;
                };
                w_entry.indegree = w_entry.indegree.saturating_sub(1);
                w_entry.indegree
            };
            if w_indegree == 0 {
                source_set.push(w);
            }
        }
    }

    let mut out: Vec<SortEntry> = Vec::new();
    for id in processed {
        let Some(entry) = mapped.get(&id) else {
            continue;
        };
        if entry.merged {
            continue;
        }
        out.push(SortEntry {
            vs: entry.vs.clone(),
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    out
}

// The conflict resolution algorithm needs a helper that can mutate two entries in-place.
// We keep it as a standalone function to make the port easy to review.
fn merge_conflict_entries(mapped: &mut HashMap<String, ConflictEntry>, target: &str, source: &str) {
    let (target_bary, target_weight, source_bary, source_weight, source_vs, source_i) = {
        let (Some(t), Some(s)) = (mapped.get(target), mapped.get(source)) else {
            return;
        };
        (
            t.barycenter,
            t.weight,
            s.barycenter,
            s.weight,
            s.vs.clone(),
            s.i,
        )
    };

    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    if let (Some(b), Some(w)) = (target_bary, target_weight) {
        if w != 0.0 {
            sum += b * w;
            weight += w;
        }
    }
    if let (Some(b), Some(w)) = (source_bary, source_weight) {
        if w != 0.0 {
            sum += b * w;
            weight += w;
        }
    }

    let Some(t) = mapped.get_mut(target) else {
        return;
    };
    t.vs = source_vs.into_iter().chain(t.vs.drain(..)).collect();
    if weight != 0.0 {
        t.barycenter = Some(sum / weight);
        t.weight = Some(weight);
    }
    t.i = t.i.min(source_i);

    if let Some(s) = mapped.get_mut(source) {
        s.merged = true;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

pub fn sort(entries: &[SortEntry], bias_right: bool) -> SortResult {
    let mut sortable: Vec<SortEntry> = Vec::new();
    let mut unsortable: Vec<SortEntry> = Vec::new();

    for entry in entries {
        if entry.barycenter.is_some() {
            sortable.push(entry.clone());
        } else {
            unsortable.push(entry.clone());
        }
    }

    unsortable.sort_by(|a, b| b.i.cmp(&a.i));

    sortable.sort_by(|a, b| {
        let a_bc = a.barycenter.unwrap_or(0.0);
        let b_bc = b.barycenter.unwrap_or(0.0);
        if a_bc < b_bc {
            std::cmp::Ordering::Less
        } else if a_bc > b_bc {
            std::cmp::Ordering::Greater
        } else if !bias_right {
            a.i.cmp(&b.i)
        } else {
            b.i.cmp(&a.i)
        }
    });

    let mut parts: Vec<Vec<String>> = Vec::new();
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        parts: &mut Vec<Vec<String>>,
        unsortable: &mut Vec<SortEntry>,
        mut index: usize,
    ) -> usize {
        while let Some(last) = unsortable.last() {
            if last.i > index {
                break;
            }
            let Some(last) = unsortable.pop() else {
                break;
            };
            parts.push(last.vs);
            index += 1;
        }
        index
    }

    vs_index = consume_unsortable(&mut parts, &mut unsortable, vs_index);

    for entry in sortable {
        vs_index += entry.vs.len();
        parts.push(entry.vs.clone());
        if let (Some(bc), Some(w)) = (entry.barycenter, entry.weight) {
            sum += bc * w;
            weight += w;
        }
        vs_index = consume_unsortable(&mut parts, &mut unsortable, vs_index);
    }

    let vs: Vec<String> = parts.into_iter().flatten().collect();
    if weight != 0.0 {
        SortResult {
            vs,
            barycenter: Some(sum / weight),
            weight: Some(weight),
        }
    } else {
        SortResult {
            vs,
            barycenter: None,
            weight: None,
        }
    }
}

pub fn sort_subgraph<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
) -> SortResult
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut movable: Vec<String> = g.children(v).into_iter().map(|s| s.to_string()).collect();

    let (border_left, border_right) = g.node(v).map_or((None, None), |node| {
        (
            node.border_left().map(|s| s.to_string()),
            node.border_right().map(|s| s.to_string()),
        )
    });

    if let (Some(bl), Some(br)) = (border_left.as_deref(), border_right.as_deref()) {
        movable.retain(|w| w != bl && w != br);
    }

    let mut subgraphs: HashMap<String, SortResult> = HashMap::new();

    let mut barycenters = barycenter(g, &movable);
    for entry in &mut barycenters {
        if !g.children(&entry.v).is_empty() {
            let subgraph_result = sort_subgraph(g, &entry.v, cg, bias_right);
            if subgraph_result.barycenter.is_some() {
                merge_barycenters(entry, &subgraph_result);
            }
            subgraphs.insert(entry.v.clone(), subgraph_result);
        }
    }

    let mut entries = resolve_conflicts(&barycenters, cg);
    expand_subgraphs(&mut entries, &subgraphs);

    let mut result = sort(&entries, bias_right);

    if let (Some(bl), Some(br)) = (border_left, border_right) {
        let mut out: Vec<String> = Vec::with_capacity(result.vs.len() + 2);
        out.push(bl.clone());
        out.extend(result.vs);
        out.push(br.clone());
        result.vs = out;

        if !g.predecessors(&bl).is_empty() {
            let bl_pred = g.predecessors(&bl)[0];
            let br_pred = g.predecessors(&br)[0];
            let bl_order = g.node(bl_pred).and_then(|n| n.order()).unwrap_or(0) as f64;
            let br_order = g.node(br_pred).and_then(|n| n.order()).unwrap_or(0) as f64;

            let bc = result.barycenter.unwrap_or(0.0);
            let w = result.weight.unwrap_or(0.0);
            let denom = w + 2.0;
            result.barycenter = Some((bc * w + bl_order + br_order) / denom);
            result.weight = Some(denom);
        }
    }

    result
}

fn expand_subgraphs(entries: &mut [SortEntry], subgraphs: &HashMap<String, SortResult>) {
    for entry in entries {
        let mut out: Vec<String> = Vec::new();
        for v in &entry.vs {
            if let Some(sg) = subgraphs.get(v) {
                out.extend(sg.vs.iter().cloned());
            } else {
                out.push(v.clone());
            }
        }
        entry.vs = out;
    }
}

fn merge_barycenters(target: &mut BarycenterEntry, other: &SortResult) {
    let Some(other_bc) = other.barycenter else {
        return;
    };
    let other_w = other.weight.unwrap_or(0.0);

    if let (Some(bc), Some(w)) = (target.barycenter, target.weight) {
        let denom = w + other_w;
        target.barycenter = Some((bc * w + other_bc * other_w) / denom);
        target.weight = Some(denom);
    } else {
        target.barycenter = Some(other_bc);
        target.weight = Some(other_w);
    }
}

pub fn add_subgraph_constraints<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    cg: &mut Graph<CN, CE, CG>,
    vs: &[String],
) where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut root_prev: Option<String> = None;

    for v in vs {
        let mut child = g.parent(v).map(|s| s.to_string());
        while let Some(c) = child.clone() {
            let parent = g.parent(&c).map(|s| s.to_string());

            let prev_child = if let Some(p) = parent.as_deref() {
                prev.insert(p.to_string(), c.clone())
            } else {
                root_prev.replace(c.clone())
            };

            if let Some(prev_child) = prev_child {
                if prev_child != c {
                    cg.set_edge(prev_child, c);
                    break;
                }
            }

            child = parent;
        }
    }
}

pub fn init_order<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
where
    N: Default + OrderNodeRange + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut visited: HashMap<String, bool> = HashMap::new();

    let simple_nodes: Vec<String> = g
        .nodes()
        .filter(|v| g.children(v).is_empty())
        .map(|v| v.to_string())
        .collect();

    let mut max_rank: i32 = i32::MIN;
    for v in &simple_nodes {
        let Some(rank) = g.node(v).and_then(|n| n.rank()) else {
            continue;
        };
        max_rank = max_rank.max(rank);
    }

    if max_rank == i32::MIN {
        return Vec::new();
    }

    let mut layers: Vec<Vec<String>> = vec![Vec::new(); (max_rank + 1).max(0) as usize];

    fn dfs<N, E, G>(
        g: &Graph<N, E, G>,
        v: &str,
        visited: &mut HashMap<String, bool>,
        layers: &mut [Vec<String>],
    ) where
        N: Default + OrderNodeRange + 'static,
        E: Default + 'static,
        G: Default,
    {
        if visited.get(v).copied().unwrap_or(false) {
            return;
        }
        visited.insert(v.to_string(), true);

        let Some(rank) = g.node(v).and_then(|n| n.rank()) else {
            return;
        };
        let idx = rank.max(0) as usize;
        if let Some(layer) = layers.get_mut(idx) {
            layer.push(v.to_string());
        }

        let successors: Vec<String> = g.successors(v).into_iter().map(|s| s.to_string()).collect();
        for w in successors {
            dfs(g, &w, visited, layers);
        }
    }

    let mut ordered_vs = simple_nodes.clone();

    let mut insertion_idx: HashMap<String, usize> = HashMap::new();
    for (idx, v) in simple_nodes.iter().enumerate() {
        insertion_idx.insert(v.to_string(), idx);
    }

    // Dagre's `initOrder` is effectively stable for nodes within the same rank (Graphlib/JS
    // preserves insertion order in `g.nodes()`). Rust's `sort_by_key` is unstable, so we must
    // include insertion order as a tie-breaker to avoid mirrored / drifted layouts on graphs
    // with symmetric constraints.
    ordered_vs.sort_by_key(|v| {
        let rank = g.node(v).and_then(|n| n.rank()).unwrap_or(i32::MAX);
        let idx = insertion_idx.get(v).copied().unwrap_or(usize::MAX);
        (rank, idx)
    });
    for v in ordered_vs {
        dfs(g, &v, &mut visited, &mut layers);
    }

    layers
}

pub fn cross_count<N, E, G>(g: &Graph<N, E, G>, layering: &[Vec<String>]) -> f64
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut cc: f64 = 0.0;
    for i in 1..layering.len() {
        cc += two_layer_cross_count(g, &layering[i - 1], &layering[i]);
    }
    cc
}

fn two_layer_cross_count<N, E, G>(g: &Graph<N, E, G>, north: &[String], south: &[String]) -> f64
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    if south.is_empty() {
        return 0.0;
    }

    let mut south_pos: HashMap<&str, usize> = HashMap::new();
    for (i, v) in south.iter().enumerate() {
        south_pos.insert(v.as_str(), i);
    }

    #[derive(Debug, Clone)]
    struct SouthEntry {
        pos: usize,
        weight: f64,
    }

    let mut south_entries: Vec<SouthEntry> = Vec::new();
    for v in north {
        let mut entries: Vec<SouthEntry> = g
            .out_edges(v, None)
            .into_iter()
            .filter_map(|e| {
                let pos = *south_pos.get(e.w.as_str())?;
                let weight = g.edge_by_key(&e).map(|e| e.weight()).unwrap_or(0.0);
                Some(SouthEntry { pos, weight })
            })
            .collect();
        entries.sort_by_key(|e| e.pos);
        south_entries.extend(entries);
    }

    let mut first_index: usize = 1;
    while first_index < south.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    first_index -= 1;
    let mut tree: Vec<f64> = vec![0.0; tree_size];

    let mut cc: f64 = 0.0;
    for entry in south_entries {
        let mut index = entry.pos + first_index;
        tree[index] += entry.weight;
        let mut weight_sum: f64 = 0.0;
        while index > 0 {
            if index % 2 == 1 {
                weight_sum += tree[index + 1];
            }
            index = (index - 1) >> 1;
            tree[index] += entry.weight;
        }
        cc += entry.weight * weight_sum;
    }

    cc
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderOptions {
    pub disable_optimal_order_heuristic: bool,
}

pub fn order<N, E, G>(g: &mut Graph<N, E, G>, opts: OrderOptions)
where
    N: Default + Clone + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut max_rank: i32 = i32::MIN;
    let mut nodes_by_rank: BTreeMap<i32, Vec<String>> = BTreeMap::new();

    for v in g.nodes() {
        let Some(node) = g.node(v) else {
            continue;
        };
        if let Some(rank) = node.rank() {
            max_rank = max_rank.max(rank);
            nodes_by_rank.entry(rank).or_default().push(v.to_string());
        }
        if let (Some(min_rank), Some(max_rank_node)) = (node.min_rank(), node.max_rank()) {
            for r in min_rank..=max_rank_node {
                if node.rank() == Some(r) {
                    continue;
                }
                nodes_by_rank.entry(r).or_default().push(v.to_string());
            }
        }
    }

    if max_rank == i32::MIN {
        return;
    }

    let layering = init_order(g);
    assign_order(g, &layering);

    if opts.disable_optimal_order_heuristic {
        return;
    }

    let mut best_cc: f64 = f64::INFINITY;
    let mut best_layering: Option<Vec<Vec<String>>> = None;

    let mut i: usize = 0;
    let mut last_best: usize = 0;
    while last_best < 4 {
        let use_down = i % 2 == 1;
        let bias_right = i % 4 >= 2;

        if use_down {
            let ranks: Vec<i32> = (1..=max_rank).collect();
            sweep(g, &nodes_by_rank, &ranks, Relationship::InEdges, bias_right);
        } else {
            let ranks: Vec<i32> = if max_rank >= 1 {
                (0..=(max_rank - 1)).rev().collect()
            } else {
                Vec::new()
            };
            sweep(
                g,
                &nodes_by_rank,
                &ranks,
                Relationship::OutEdges,
                bias_right,
            );
        }

        let layering_now = build_layer_matrix(g, max_rank);
        let cc = cross_count(g, &layering_now);
        if cc < best_cc {
            last_best = 0;
            best_cc = cc;
            best_layering = Some(layering_now);
        }

        i += 1;
        last_best += 1;
    }

    if let Some(best) = best_layering {
        assign_order(g, &best);
    }
}

fn assign_order<N, E, G>(g: &mut Graph<N, E, G>, layering: &[Vec<String>])
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    for layer in layering {
        for (i, v) in layer.iter().enumerate() {
            if let Some(node) = g.node_mut(v) {
                node.set_order(i);
            }
        }
    }
}

fn sweep<N, E, G>(
    g: &mut Graph<N, E, G>,
    nodes_by_rank: &BTreeMap<i32, Vec<String>>,
    ranks: &[i32],
    relationship: Relationship,
    bias_right: bool,
) where
    N: Default + Clone + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut cg: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    for &rank in ranks {
        let nodes = nodes_by_rank
            .get(&rank)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let lg = build_layer_graph(g, rank, relationship, Some(nodes));
        let root = lg.graph().root.clone();

        let sorted = sort_subgraph(&lg, &root, &cg, bias_right);
        for (i, v) in sorted.vs.iter().enumerate() {
            if let Some(n) = g.node_mut(v) {
                n.set_order(i);
            }
        }

        add_subgraph_constraints(&lg, &mut cg, &sorted.vs);
    }
}

fn build_layer_matrix<N, E, G>(g: &Graph<N, E, G>, max_rank: i32) -> Vec<Vec<String>>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut layers: Vec<Vec<(usize, String)>> = vec![Vec::new(); (max_rank + 1) as usize];
    for v in g.nodes() {
        let Some(node) = g.node(v) else {
            continue;
        };
        let Some(rank) = node.rank() else {
            continue;
        };
        let Some(order) = node.order() else {
            continue;
        };
        layers[rank.max(0) as usize].push((order, v.to_string()));
    }
    let mut out: Vec<Vec<String>> = Vec::with_capacity(layers.len());
    for mut layer in layers {
        layer.sort_by_key(|(o, _)| *o);
        out.push(layer.into_iter().map(|(_, v)| v).collect());
    }
    out
}

fn create_root_node<N, E, G>(g: &Graph<N, E, G>) -> String
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    loop {
        let v = crate::util::unique_id("_root");
        if !g.has_node(&v) {
            return v;
        }
    }
}
