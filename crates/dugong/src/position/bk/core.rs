//! Brandes & Köpf (BK) horizontal compaction.
//!
//! This module is a parity-oriented port of Dagre's `position/bk` helpers.
//!
//! Note: this file is being split into submodules to keep individual algorithms focused.

use crate::graphlib::{Graph, GraphOptions};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;
use std::collections::{BTreeMap, BTreeSet};

use super::util::{SepNodeMetrics, sep, sep_metrics, width};

pub type Conflicts = BTreeMap<String, BTreeSet<String>>;

pub fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (v, w) = if v <= w { (v, w) } else { (w, v) };
    conflicts
        .entry(v.to_string())
        .or_default()
        .insert(w.to_string());
}

pub fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (v, w) = if v <= w { (v, w) } else { (w, v) };
    conflicts.get(v).map(|m| m.contains(w)).unwrap_or(false)
}

fn first_dummy_predecessor<'a>(
    g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
) -> Option<&'a str> {
    let mut out: Option<&'a str> = None;
    g.for_each_predecessor(v, |u| {
        if out.is_some() {
            return;
        }
        if g.node(u).map(|n| n.dummy.is_some()).unwrap_or(false) {
            out = Some(u);
        }
    });
    out
}

pub fn find_type1_conflicts(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> Conflicts {
    let mut conflicts: Conflicts = BTreeMap::new();
    if layering.is_empty() {
        return conflicts;
    }

    for i in 1..layering.len() {
        let prev_layer = &layering[i - 1];
        let layer = &layering[i];

        let mut k0: usize = 0;
        let mut scan_pos: usize = 0;
        let prev_layer_len = prev_layer.len();
        let last_node = layer.last().map(|s| s.as_str());

        for (idx, v) in layer.iter().enumerate() {
            let w = find_other_inner_segment_node(g, v);
            let k1 = w
                .as_deref()
                .and_then(|w| g.node(w))
                .and_then(|n| n.order)
                .unwrap_or(prev_layer_len);

            if w.is_some() || last_node == Some(v.as_str()) {
                for scan_node in layer.iter().skip(scan_pos).take(idx + 1 - scan_pos) {
                    let scan_dummy = g
                        .node(scan_node)
                        .map(|n| n.dummy.is_some())
                        .unwrap_or(false);
                    g.for_each_predecessor(scan_node, |u| {
                        let Some(u_label) = g.node(u) else {
                            return;
                        };
                        let u_pos = u_label.order.unwrap_or(0);
                        let u_dummy = u_label.dummy.is_some();

                        if (u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy) {
                            add_conflict(&mut conflicts, u, scan_node);
                        }
                    });
                }
                scan_pos = idx + 1;
                k0 = k1;
            }
        }
    }

    conflicts
}

pub fn find_type2_conflicts(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> Conflicts {
    let mut conflicts: Conflicts = BTreeMap::new();
    if layering.is_empty() {
        return conflicts;
    }

    fn scan(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        conflicts: &mut Conflicts,
        south: &[String],
        south_pos: usize,
        south_end: usize,
        prev_north_border: isize,
        next_north_border: isize,
    ) {
        for v in south.iter().take(south_end).skip(south_pos) {
            let v_dummy = g.node(v).and_then(|n| n.dummy.as_deref());
            if v_dummy.is_some() {
                g.for_each_predecessor(v, |u| {
                    let Some(u_node) = g.node(u) else {
                        return;
                    };
                    if u_node.dummy.is_some() {
                        let u_order = u_node.order.unwrap_or(0) as isize;
                        if u_order < prev_north_border || u_order > next_north_border {
                            add_conflict(conflicts, u, v);
                        }
                    }
                });
            }
        }
    }

    for i in 1..layering.len() {
        let north = &layering[i - 1];
        let south = &layering[i];

        let mut prev_north_pos: isize = -1;
        let mut next_north_pos: Option<isize> = None;
        let mut south_pos: usize = 0;

        for (south_lookahead, v) in south.iter().enumerate() {
            let is_border = g
                .node(v)
                .and_then(|n| n.dummy.as_deref())
                .is_some_and(|d| d == "border");
            if is_border {
                let mut first: Option<&str> = None;
                g.for_each_predecessor(v, |u| {
                    if first.is_none() {
                        first = Some(u);
                    }
                });
                if let Some(u) = first {
                    next_north_pos = g.node(u).and_then(|n| n.order).map(|n| n as isize);
                    scan(
                        g,
                        &mut conflicts,
                        south,
                        south_pos,
                        south_lookahead,
                        prev_north_pos,
                        next_north_pos.unwrap_or(-1),
                    );
                    south_pos = south_lookahead;
                    prev_north_pos = next_north_pos.unwrap_or(prev_north_pos);
                }
            }

            scan(
                g,
                &mut conflicts,
                south,
                south_pos,
                south.len(),
                next_north_pos.unwrap_or(-1),
                north.len() as isize,
            );
        }
    }

    conflicts
}

fn find_other_inner_segment_node(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
) -> Option<String> {
    if g.node(v).map(|n| n.dummy.is_some()).unwrap_or(false) {
        return first_dummy_predecessor(g, v).map(|u| u.to_string());
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    pub root: HashMap<String, String>,
    pub align: HashMap<String, String>,
}

pub fn vertical_alignment<F>(
    _g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    neighbor_fn: F,
) -> Alignment
where
    F: Fn(&str) -> Vec<String>,
{
    let mut root: HashMap<String, String> = HashMap::default();
    let mut align: HashMap<String, String> = HashMap::default();
    let mut pos: HashMap<String, usize> = HashMap::default();

    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: isize = -1;
        for v in layer {
            let mut ws = neighbor_fn(v);
            if ws.is_empty() {
                continue;
            }
            ws.sort_by_key(|w| pos.get(w).copied().unwrap_or(usize::MAX));

            let mp = (ws.len() - 1) as f64 / 2.0;
            let i0 = mp.floor() as usize;
            let i1 = mp.ceil() as usize;

            for w in ws.iter().take(i1 + 1).skip(i0) {
                let v_align = align.get(v).cloned().unwrap_or_else(|| v.clone());
                let w_pos = pos.get(w).copied().unwrap_or(usize::MAX) as isize;
                if v_align == *v && prev_idx < w_pos && !has_conflict(conflicts, v, w) {
                    align.insert(w.clone(), v.clone());
                    let w_root = root.get(w).cloned().unwrap_or_else(|| w.clone());
                    align.insert(v.clone(), w_root.clone());
                    root.insert(v.clone(), w_root);
                    prev_idx = w_pos;
                }
            }
        }
    }

    Alignment { root, align }
}

const NONE: usize = usize::MAX;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BorderType {
    Other,
    Left,
    Right,
}

impl BorderType {
    fn from_label(label: &NodeLabel) -> Self {
        match label.border_type.as_deref() {
            Some("borderLeft") => Self::Left,
            Some("borderRight") => Self::Right,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy)]
struct BkNode {
    graph_ix: usize,
    order: Option<usize>,
    metrics: SepNodeMetrics,
    is_border: bool,
    border_type: BorderType,
}

#[derive(Clone, Copy)]
struct Orientation {
    index: usize,
    reverse_layers: bool,
    reverse_inner: bool,
}

const ORIENTATIONS: [Orientation; 4] = [
    Orientation {
        index: 0,
        reverse_layers: false,
        reverse_inner: false,
    },
    Orientation {
        index: 1,
        reverse_layers: false,
        reverse_inner: true,
    },
    Orientation {
        index: 2,
        reverse_layers: true,
        reverse_inner: false,
    },
    Orientation {
        index: 3,
        reverse_layers: true,
        reverse_inner: true,
    },
];

struct BkWorkspace<'a> {
    g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: Vec<Vec<usize>>,
    nodes: Vec<BkNode>,
    predecessors: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    first_predecessor: Vec<Option<usize>>,
    first_dummy_predecessor: Vec<Option<usize>>,
    conflicts: Vec<(usize, usize)>,
    conflicts_seen: HashSet<(usize, usize)>,
    root: Vec<usize>,
    align: Vec<usize>,
    pos: Vec<usize>,
    coords: [Vec<f64>; 4],
    neighbor_scratch: Vec<usize>,
    root_to_block: Vec<usize>,
    block_roots: Vec<usize>,
    block_edges: HashMap<(usize, usize), f64>,
    block_edge_order: Vec<(usize, usize)>,
    block_predecessors: Vec<Vec<(usize, f64)>>,
    block_successors: Vec<Vec<(usize, f64)>>,
    block_x: Vec<f64>,
    scheduled: Vec<bool>,
    stack: Vec<(usize, bool)>,
}

impl<'a> BkWorkspace<'a> {
    fn new(
        g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        source_layering: &[Vec<String>],
    ) -> Self {
        let mut layering: Vec<Vec<usize>> = Vec::with_capacity(source_layering.len());
        let mut nodes: Vec<BkNode> = Vec::new();
        let mut graph_to_local: Vec<usize> = Vec::new();

        for source_layer in source_layering {
            let mut layer: Vec<usize> = Vec::with_capacity(source_layer.len());
            for id in source_layer {
                let Some(graph_ix) = g.node_ix(id) else {
                    continue;
                };
                if graph_ix >= graph_to_local.len() {
                    graph_to_local.resize(graph_ix + 1, NONE);
                }

                let local_ix = graph_to_local[graph_ix];
                if local_ix != NONE {
                    layer.push(local_ix);
                    continue;
                }

                let Some(label) = g.node_label_by_ix(graph_ix) else {
                    continue;
                };

                let local_ix = nodes.len();
                graph_to_local[graph_ix] = local_ix;
                nodes.push(BkNode {
                    graph_ix,
                    order: label.order,
                    metrics: SepNodeMetrics::from(label),
                    is_border: label.dummy.as_deref() == Some("border"),
                    border_type: BorderType::from_label(label),
                });
                layer.push(local_ix);
            }
            layering.push(layer);
        }

        let node_count = nodes.len();
        let mut predecessors = vec![Vec::new(); node_count];
        let mut successors = vec![Vec::new(); node_count];
        let mut first_predecessor = vec![None; node_count];
        let mut first_dummy_predecessor = vec![None; node_count];
        let mut saw_predecessor = vec![false; node_count];
        let mut saw_dummy_predecessor = vec![false; node_count];

        if g.is_directed() {
            g.for_each_edge_ix(|from_graph_ix, to_graph_ix, _, _| {
                let to_local = graph_to_local
                    .get(to_graph_ix)
                    .copied()
                    .filter(|&ix| ix < node_count);
                let Some(to_local) = to_local else {
                    return;
                };
                let from_local = graph_to_local
                    .get(from_graph_ix)
                    .copied()
                    .filter(|&ix| ix < node_count);

                if !saw_predecessor[to_local] {
                    saw_predecessor[to_local] = true;
                    first_predecessor[to_local] = from_local;
                }

                let from_is_dummy = from_local
                    .map(|from_local| nodes[from_local].metrics.is_dummy)
                    .unwrap_or_else(|| {
                        g.node_label_by_ix(from_graph_ix)
                            .is_some_and(|label| label.dummy.is_some())
                    });
                if from_is_dummy && !saw_dummy_predecessor[to_local] {
                    saw_dummy_predecessor[to_local] = true;
                    first_dummy_predecessor[to_local] = from_local;
                }

                if let Some(from_local) = from_local {
                    predecessors[to_local].push(from_local);
                    successors[from_local].push(to_local);
                }
            });
        } else {
            let mut seen_neighbors: Vec<HashSet<usize>> =
                (0..node_count).map(|_| HashSet::default()).collect();
            g.for_each_edge_ix(|from_graph_ix, to_graph_ix, _, _| {
                for (node_graph_ix, neighbor_graph_ix) in
                    [(from_graph_ix, to_graph_ix), (to_graph_ix, from_graph_ix)]
                {
                    let node_local = graph_to_local
                        .get(node_graph_ix)
                        .copied()
                        .filter(|&ix| ix < node_count);
                    let Some(node_local) = node_local else {
                        continue;
                    };
                    if !seen_neighbors[node_local].insert(neighbor_graph_ix) {
                        continue;
                    }

                    let neighbor_local = graph_to_local
                        .get(neighbor_graph_ix)
                        .copied()
                        .filter(|&ix| ix < node_count);
                    if !saw_predecessor[node_local] {
                        saw_predecessor[node_local] = true;
                        first_predecessor[node_local] = neighbor_local;
                    }

                    let neighbor_is_dummy = neighbor_local
                        .map(|neighbor_local| nodes[neighbor_local].metrics.is_dummy)
                        .unwrap_or_else(|| {
                            g.node_label_by_ix(neighbor_graph_ix)
                                .is_some_and(|label| label.dummy.is_some())
                        });
                    if neighbor_is_dummy && !saw_dummy_predecessor[node_local] {
                        saw_dummy_predecessor[node_local] = true;
                        first_dummy_predecessor[node_local] = neighbor_local;
                    }

                    if let Some(neighbor_local) = neighbor_local {
                        predecessors[node_local].push(neighbor_local);
                        successors[node_local].push(neighbor_local);
                    }
                }
            });
        }

        Self {
            g,
            layering,
            nodes,
            predecessors,
            successors,
            first_predecessor,
            first_dummy_predecessor,
            conflicts: Vec::new(),
            conflicts_seen: HashSet::default(),
            root: (0..node_count).collect(),
            align: (0..node_count).collect(),
            pos: vec![NONE; node_count],
            coords: std::array::from_fn(|_| vec![0.0; node_count]),
            neighbor_scratch: Vec::new(),
            root_to_block: vec![NONE; node_count],
            block_roots: Vec::new(),
            block_edges: HashMap::default(),
            block_edge_order: Vec::new(),
            block_predecessors: vec![Vec::new(); node_count],
            block_successors: vec![Vec::new(); node_count],
            block_x: vec![0.0; node_count],
            scheduled: vec![false; node_count],
            stack: Vec::new(),
        }
    }

    fn run(mut self) -> HashMap<String, f64> {
        if self.nodes.is_empty() {
            return HashMap::default();
        }

        self.find_conflicts();
        for orientation in ORIENTATIONS {
            self.vertical_alignment(orientation);
            self.horizontal_compaction(orientation);
        }

        let smallest = self.find_smallest_width_alignment();
        self.align_coordinates(smallest);
        self.balance()
    }

    fn find_conflicts(&mut self) {
        self.find_type1_conflicts();
        self.find_type2_conflicts();
        self.conflicts.sort_unstable();
        self.conflicts_seen = HashSet::default();
    }

    fn add_conflict(&mut self, v: usize, w: usize) {
        let conflict = if v <= w { (v, w) } else { (w, v) };
        if self.conflicts_seen.insert(conflict) {
            self.conflicts.push(conflict);
        }
    }

    fn has_conflict(&self, v: usize, w: usize) -> bool {
        let conflict = if v <= w { (v, w) } else { (w, v) };
        self.conflicts.binary_search(&conflict).is_ok()
    }

    fn find_type1_conflicts(&mut self) {
        for layer_index in 1..self.layering.len() {
            let prev_layer_len = self.layering[layer_index - 1].len();
            let layer_len = self.layering[layer_index].len();
            if layer_len == 0 {
                continue;
            }

            let mut k0 = 0;
            let mut scan_pos = 0;
            for index in 0..layer_len {
                let v = self.layering[layer_index][index];
                let inner_segment = self.nodes[v]
                    .metrics
                    .is_dummy
                    .then_some(self.first_dummy_predecessor[v])
                    .flatten();
                let k1 = inner_segment
                    .and_then(|w| self.nodes[w].order)
                    .unwrap_or(prev_layer_len);

                if inner_segment.is_some() || index + 1 == layer_len {
                    for scan_index in scan_pos..=index {
                        let scan_node = self.layering[layer_index][scan_index];
                        let scan_dummy = self.nodes[scan_node].metrics.is_dummy;
                        for predecessor_index in 0..self.predecessors[scan_node].len() {
                            let u = self.predecessors[scan_node][predecessor_index];
                            let u_pos = self.nodes[u].order.unwrap_or(0);
                            let u_dummy = self.nodes[u].metrics.is_dummy;
                            if (u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy) {
                                self.add_conflict(u, scan_node);
                            }
                        }
                    }
                    scan_pos = index + 1;
                    k0 = k1;
                }
            }
        }
    }

    fn find_type2_conflicts(&mut self) {
        for layer_index in 1..self.layering.len() {
            if self.type2_requires_fallback(layer_index) {
                self.find_type2_conflicts_fallback(layer_index);
                continue;
            }

            let north_len = self.layering[layer_index - 1].len() as isize;
            let south_len = self.layering[layer_index].len();
            let mut prev_north_pos = -1;
            let mut south_pos = 0;

            for south_lookahead in 0..south_len {
                let v = self.layering[layer_index][south_lookahead];
                if !self.nodes[v].is_border {
                    continue;
                }
                let Some(u) = self.first_predecessor[v] else {
                    continue;
                };
                let Some(next_north_pos) = self.nodes[u].order.map(|order| order as isize) else {
                    continue;
                };

                self.scan_type2_segment(
                    layer_index,
                    south_pos,
                    south_lookahead,
                    prev_north_pos,
                    next_north_pos,
                );
                south_pos = south_lookahead;
                prev_north_pos = next_north_pos;
            }

            self.scan_type2_segment(layer_index, south_pos, south_len, prev_north_pos, north_len);
        }
    }

    fn type2_requires_fallback(&self, layer_index: usize) -> bool {
        let mut previous_order: Option<isize> = None;
        for &v in &self.layering[layer_index] {
            if !self.nodes[v].is_border {
                continue;
            }
            let Some(u) = self.first_predecessor[v] else {
                continue;
            };
            let Some(order) = self.nodes[u].order.map(|order| order as isize) else {
                return true;
            };
            if previous_order.is_some_and(|previous_order| order < previous_order) {
                return true;
            }
            previous_order = Some(order);
        }
        false
    }

    fn find_type2_conflicts_fallback(&mut self, layer_index: usize) {
        let north_len = self.layering[layer_index - 1].len() as isize;
        let south_len = self.layering[layer_index].len();
        let mut prev_north_pos = -1;
        let mut next_north_pos: Option<isize> = None;
        let mut south_pos = 0;

        for south_lookahead in 0..south_len {
            let v = self.layering[layer_index][south_lookahead];
            if self.nodes[v].is_border
                && let Some(u) = self.first_predecessor[v]
            {
                next_north_pos = self.nodes[u].order.map(|order| order as isize);
                self.scan_type2_segment(
                    layer_index,
                    south_pos,
                    south_lookahead,
                    prev_north_pos,
                    next_north_pos.unwrap_or(-1),
                );
                south_pos = south_lookahead;
                prev_north_pos = next_north_pos.unwrap_or(prev_north_pos);
            }

            self.scan_type2_segment(
                layer_index,
                south_pos,
                south_len,
                next_north_pos.unwrap_or(-1),
                north_len,
            );
        }
    }

    fn scan_type2_segment(
        &mut self,
        layer_index: usize,
        start: usize,
        end: usize,
        prev_north_border: isize,
        next_north_border: isize,
    ) {
        for index in start..end {
            let v = self.layering[layer_index][index];
            if !self.nodes[v].metrics.is_dummy {
                continue;
            }
            for predecessor_index in 0..self.predecessors[v].len() {
                let u = self.predecessors[v][predecessor_index];
                if self.nodes[u].metrics.is_dummy {
                    let u_order = self.nodes[u].order.unwrap_or(0) as isize;
                    if u_order < prev_north_border || u_order > next_north_border {
                        self.add_conflict(u, v);
                    }
                }
            }
        }
    }

    fn oriented_layer_index(&self, orientation: Orientation, index: usize) -> usize {
        if orientation.reverse_layers {
            self.layering.len() - 1 - index
        } else {
            index
        }
    }

    fn oriented_node(&self, orientation: Orientation, layer_index: usize, index: usize) -> usize {
        let layer = &self.layering[layer_index];
        if orientation.reverse_inner {
            layer[layer.len() - 1 - index]
        } else {
            layer[index]
        }
    }

    fn vertical_alignment(&mut self, orientation: Orientation) {
        for node in 0..self.nodes.len() {
            self.root[node] = node;
            self.align[node] = node;
            self.pos[node] = NONE;
        }

        for index in 0..self.layering.len() {
            let layer_index = self.oriented_layer_index(orientation, index);
            for order in 0..self.layering[layer_index].len() {
                let v = self.oriented_node(orientation, layer_index, order);
                self.pos[v] = order;
            }
        }

        for index in 0..self.layering.len() {
            let layer_index = self.oriented_layer_index(orientation, index);
            let mut prev_idx = -1;
            for order in 0..self.layering[layer_index].len() {
                let v = self.oriented_node(orientation, layer_index, order);
                self.neighbor_scratch.clear();
                let neighbors = if orientation.reverse_layers {
                    &self.successors[v]
                } else {
                    &self.predecessors[v]
                };
                self.neighbor_scratch.extend_from_slice(neighbors);
                if self.neighbor_scratch.is_empty() {
                    continue;
                }

                let pos = &self.pos;
                self.neighbor_scratch.sort_by_key(|&w| pos[w]);
                let i0 = (self.neighbor_scratch.len() - 1) / 2;
                let i1 = self.neighbor_scratch.len() / 2;
                for neighbor_index in i0..=i1 {
                    let w = self.neighbor_scratch[neighbor_index];
                    let w_pos = self.pos[w] as isize;
                    if self.align[v] == v && prev_idx < w_pos && !self.has_conflict(v, w) {
                        self.align[w] = v;
                        self.align[v] = self.root[w];
                        self.root[v] = self.root[w];
                        prev_idx = w_pos;
                    }
                }
            }
        }
    }

    fn ensure_block(&mut self, root: usize) -> usize {
        let block = self.root_to_block[root];
        if block != NONE {
            return block;
        }

        let block = self.block_roots.len();
        self.root_to_block[root] = block;
        self.block_roots.push(root);
        block
    }

    fn horizontal_compaction(&mut self, orientation: Orientation) {
        self.root_to_block.fill(NONE);
        self.block_roots.clear();
        self.block_edges.clear();
        self.block_edge_order.clear();

        let node_sep = self.g.graph().nodesep;
        let edge_sep = self.g.graph().edgesep;
        for index in 0..self.layering.len() {
            let layer_index = self.oriented_layer_index(orientation, index);
            let mut previous: Option<usize> = None;
            for order in 0..self.layering[layer_index].len() {
                let v = self.oriented_node(orientation, layer_index, order);
                let v_root = self.root[v];
                let v_block = self.ensure_block(v_root);
                if let Some(u) = previous {
                    let u_root = self.root[u];
                    let u_block = self.ensure_block(u_root);
                    let separation = sep_metrics(
                        self.nodes[v].metrics,
                        self.nodes[u].metrics,
                        node_sep,
                        edge_sep,
                        orientation.reverse_inner,
                    );
                    if let Some(weight) = self.block_edges.get_mut(&(u_block, v_block)) {
                        if separation > *weight {
                            *weight = separation;
                        }
                    } else {
                        self.block_edges.insert((u_block, v_block), separation);
                        self.block_edge_order.push((u_block, v_block));
                    }
                }
                previous = Some(v);
            }
        }

        let block_count = self.block_roots.len();
        for predecessors in &mut self.block_predecessors[..block_count] {
            predecessors.clear();
        }
        for successors in &mut self.block_successors[..block_count] {
            successors.clear();
        }
        for &(u, v) in &self.block_edge_order {
            let weight = self.block_edges[&(u, v)];
            self.block_predecessors[v].push((u, weight));
            self.block_successors[u].push((v, weight));
        }
        self.block_x[..block_count].fill(0.0);

        self.scheduled[..block_count].fill(false);
        self.stack.clear();
        self.stack
            .extend((0..block_count).map(|block| (block, false)));
        while let Some((block, expanded)) = self.stack.pop() {
            if expanded {
                let mut best: f64 = 0.0;
                for &(predecessor, weight) in &self.block_predecessors[block] {
                    best = best.max(self.block_x[predecessor] + weight);
                }
                self.block_x[block] = best;
                continue;
            }
            if self.scheduled[block] {
                continue;
            }
            self.scheduled[block] = true;
            self.stack.push((block, true));
            for &(predecessor, _) in &self.block_predecessors[block] {
                self.stack.push((predecessor, false));
            }
        }

        let excluded_border = if orientation.reverse_inner {
            BorderType::Left
        } else {
            BorderType::Right
        };
        self.scheduled[..block_count].fill(false);
        self.stack.clear();
        self.stack
            .extend((0..block_count).map(|block| (block, false)));
        while let Some((block, expanded)) = self.stack.pop() {
            if expanded {
                let mut min: f64 = f64::INFINITY;
                for &(successor, weight) in &self.block_successors[block] {
                    min = min.min(self.block_x[successor] - weight);
                }
                let root = self.block_roots[block];
                if min.is_finite() && self.nodes[root].border_type != excluded_border {
                    self.block_x[block] = self.block_x[block].max(min);
                }
                continue;
            }
            if self.scheduled[block] {
                continue;
            }
            self.scheduled[block] = true;
            self.stack.push((block, true));
            for &(successor, _) in &self.block_successors[block] {
                self.stack.push((successor, false));
            }
        }

        let xs = &mut self.coords[orientation.index];
        for (node, x) in xs.iter_mut().enumerate() {
            let block = self.root_to_block[self.root[node]];
            *x = if block == NONE {
                0.0
            } else {
                self.block_x[block]
            };
        }
        if orientation.reverse_inner {
            for x in xs {
                *x = -*x;
            }
        }
    }

    fn find_smallest_width_alignment(&self) -> usize {
        let mut best = 0;
        let mut best_width = f64::INFINITY;
        for orientation in ORIENTATIONS {
            let mut max = f64::NEG_INFINITY;
            let mut min = f64::INFINITY;
            for (node, &x) in self.coords[orientation.index].iter().enumerate() {
                let half_width = self.nodes[node].metrics.width / 2.0;
                max = max.max(x + half_width);
                min = min.min(x - half_width);
            }
            let width = max - min;
            if width < best_width {
                best_width = width;
                best = orientation.index;
            }
        }
        best
    }

    fn align_coordinates(&mut self, align_to: usize) {
        let align_to_min = self.coords[align_to]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let align_to_max = self.coords[align_to]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        for orientation in ORIENTATIONS {
            let xs_min = self.coords[orientation.index]
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let xs_max = self.coords[orientation.index]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let delta = if orientation.reverse_inner {
                align_to_max - xs_max
            } else {
                align_to_min - xs_min
            };
            if delta != 0.0 {
                for x in &mut self.coords[orientation.index] {
                    *x += delta;
                }
            }
        }
    }

    fn balance(&self) -> HashMap<String, f64> {
        let align = self.g.graph().align.as_deref().map(str::to_ascii_lowercase);
        let selected = match align.as_deref() {
            Some("ul") => Some(0),
            Some("ur") => Some(1),
            Some("dl") => Some(2),
            Some("dr") => Some(3),
            Some(_) => None,
            None => None,
        };
        let invalid_alignment = align.is_some() && selected.is_none();

        let mut out: HashMap<String, f64> = HashMap::default();
        out.reserve(self.nodes.len());
        for node in 0..self.nodes.len() {
            let x = if invalid_alignment {
                0.0
            } else if let Some(selected) = selected {
                self.coords[selected][node]
            } else {
                let mut values = [
                    self.coords[0][node],
                    self.coords[1][node],
                    self.coords[2][node],
                    self.coords[3][node],
                ];
                values.sort_by(f64::total_cmp);
                (values[1] + values[2]) / 2.0
            };
            if let Some(id) = self.g.node_id_by_ix(self.nodes[node].graph_ix) {
                out.insert(id.to_string(), x);
            }
        }
        out
    }
}

pub fn position_x_with_layering(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> HashMap<String, f64> {
    BkWorkspace::new(g, layering).run()
}

pub fn horizontal_compaction(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let root_ref: HashMap<&'_ str, &'_ str> =
        root.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let align_ref: HashMap<&'_ str, &'_ str> = align
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    horizontal_compaction_ref(g, layering, &root_ref, &align_ref, reverse_sep)
}

fn horizontal_compaction_ref<'a>(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<&'a str, &'a str>,
    align: &HashMap<&'a str, &'a str>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let mut xs: HashMap<String, f64> = HashMap::default();
    let block_g = build_block_graph_ref(g, layering, root, reverse_sep);
    let border_type = if reverse_sep {
        "borderLeft"
    } else {
        "borderRight"
    };

    fn iterate_predecessors<'a, F>(block_g: &'a Graph<(), f64, ()>, mut set_xs: F)
    where
        F: FnMut(&'a str),
    {
        let mut stack: Vec<&'a str> = block_g.nodes().collect();
        let mut entered: HashSet<&'a str> = HashSet::default();
        let mut scratch: Vec<&'a str> = Vec::new();

        while let Some(elem) = stack.pop() {
            if entered.contains(elem) {
                set_xs(elem);
                continue;
            }

            entered.insert(elem);
            stack.push(elem);

            scratch.clear();
            block_g.extend_predecessors(elem, &mut scratch);
            stack.extend(scratch.iter().copied());
        }
    }

    fn iterate_successors<'a, F>(block_g: &'a Graph<(), f64, ()>, mut set_xs: F)
    where
        F: FnMut(&'a str),
    {
        let mut stack: Vec<&'a str> = block_g.nodes().collect();
        let mut entered: HashSet<&'a str> = HashSet::default();
        let mut scratch: Vec<&'a str> = Vec::new();

        while let Some(elem) = stack.pop() {
            if entered.contains(elem) {
                set_xs(elem);
                continue;
            }

            entered.insert(elem);
            stack.push(elem);

            scratch.clear();
            block_g.extend_successors(elem, &mut scratch);
            stack.extend(scratch.iter().copied());
        }
    }

    // First pass: assign smallest coordinates
    {
        let mut set = |elem: &str| {
            let mut best: f64 = 0.0;
            block_g.for_each_in_edge(elem, None, |ek, w| {
                let x_v = xs.get(&ek.v).copied().unwrap_or(0.0);
                best = best.max(x_v + *w);
            });
            xs.insert(elem.to_string(), best);
        };
        iterate_predecessors(&block_g, &mut set);
    }

    // Second pass: assign greatest coordinates
    {
        let mut set = |elem: &str| {
            let mut min: f64 = f64::INFINITY;
            block_g.for_each_out_edge(elem, None, |ek, w| {
                let x_w = xs.get(&ek.w).copied().unwrap_or(0.0);
                min = min.min(x_w - *w);
            });

            let node = g.node(elem);
            let Some(node) = node else {
                return;
            };
            if min.is_finite() && node.border_type.as_deref() != Some(border_type) {
                let cur = xs.get(elem).copied().unwrap_or(0.0);
                xs.insert(elem.to_string(), cur.max(min));
            }
        };
        iterate_successors(&block_g, &mut set);
    }

    // Assign x coordinates to all nodes based on their block root.
    let mut out: HashMap<String, f64> = HashMap::default();
    for (&v, &r) in align {
        let x = xs
            .get(root.get(v).copied().unwrap_or(r))
            .copied()
            .unwrap_or(0.0);
        out.insert(v.to_string(), x);
    }
    out
}

fn build_block_graph_ref<'a>(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<&'a str, &'a str>,
    reverse_sep: bool,
) -> Graph<(), f64, ()> {
    let mut block_graph: Graph<(), f64, ()> = Graph::new(GraphOptions::default());
    for layer in layering {
        let mut u: Option<&str> = None;
        for v in layer {
            let v = v.as_str();
            let v_root = root.get(v).copied().unwrap_or(v);
            block_graph.ensure_node(v_root.to_string());

            if let Some(u) = u {
                let u_root = root.get(u).copied().unwrap_or(u);
                let prev_max = block_graph
                    .edge(u_root, v_root, None)
                    .copied()
                    .unwrap_or(0.0);
                let sep = sep(g, v, u, reverse_sep);
                block_graph.set_edge_with_label(
                    u_root.to_string(),
                    v_root.to_string(),
                    sep.max(prev_max),
                );
            }

            u = Some(v);
        }
    }
    block_graph
}

pub fn find_smallest_width_alignment(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    xss: &HashMap<String, HashMap<String, f64>>,
) -> HashMap<String, f64> {
    let mut best_width: f64 = f64::INFINITY;
    let mut best: HashMap<String, f64> = HashMap::default();

    // Match upstream dagre: ties are resolved by a stable iteration order over alignments.
    // The canonical order is: `ul`, `ur`, `dl`, `dr` (insertion order in upstream).
    for key in ["ul", "ur", "dl", "dr"] {
        let Some(xs) = xss.get(key) else {
            continue;
        };
        let mut max: f64 = f64::NEG_INFINITY;
        let mut min: f64 = f64::INFINITY;
        for (v, x) in xs {
            let half_w = width(g, v) / 2.0;
            max = max.max(x + half_w);
            min = min.min(x - half_w);
        }
        let w = max - min;
        if w < best_width {
            best_width = w;
            best = xs.clone();
        }
    }

    best
}

pub fn align_coordinates(
    xss: &mut HashMap<String, HashMap<String, f64>>,
    align_to: &HashMap<String, f64>,
) {
    let align_to_min = align_to.values().copied().fold(f64::INFINITY, f64::min);
    let align_to_max = align_to.values().copied().fold(f64::NEG_INFINITY, f64::max);

    for (vert, horiz) in [("u", "l"), ("u", "r"), ("d", "l"), ("d", "r")] {
        let key = format!("{vert}{horiz}");
        let Some(xs) = xss.get(&key).cloned() else {
            continue;
        };

        let xs_min = xs.values().copied().fold(f64::INFINITY, f64::min);
        let xs_max = xs.values().copied().fold(f64::NEG_INFINITY, f64::max);

        let mut delta = align_to_min - xs_min;
        if horiz != "l" {
            delta = align_to_max - xs_max;
        }

        if delta != 0.0 {
            xss.insert(key, xs.into_iter().map(|(v, x)| (v, x + delta)).collect());
        }
    }
}

pub fn balance(
    xss: &HashMap<String, HashMap<String, f64>>,
    align: Option<&str>,
) -> HashMap<String, f64> {
    let Some(xs_ul) = xss.get("ul") else {
        return HashMap::default();
    };

    let align_key = align.map(|a| a.to_ascii_lowercase());

    let mut out: HashMap<String, f64> = HashMap::default();
    for v in xs_ul.keys() {
        if let Some(key) = align_key.as_deref() {
            let x = xss
                .get(key)
                .and_then(|xs| xs.get(v))
                .copied()
                .unwrap_or(0.0);
            out.insert(v.clone(), x);
            continue;
        }

        let mut vals: Vec<f64> = xss.values().filter_map(|xs| xs.get(v).copied()).collect();
        vals.sort_by(|a, b| a.total_cmp(b));
        if vals.len() >= 4 {
            out.insert(v.clone(), (vals[1] + vals[2]) / 2.0);
        }
    }
    out
}

pub fn position_x(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> HashMap<String, f64> {
    let layering = crate::util::build_layer_matrix(g);
    position_x_with_layering(g, &layering)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fallback_graph(
        intermediate_count: usize,
    ) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());

        let north = vec![
            "north-low".to_string(),
            "north-middle".to_string(),
            "north-high".to_string(),
        ];
        for (order, id) in north.iter().enumerate() {
            g.set_node(
                id.clone(),
                NodeLabel {
                    rank: Some(0),
                    order: Some(order),
                    dummy: Some("dummy".to_string()),
                    ..Default::default()
                },
            );
        }

        let mut south = Vec::with_capacity(intermediate_count + 2);
        south.push("border-high".to_string());
        south.extend((0..intermediate_count).map(|index| format!("dummy-{index}")));
        south.push("border-low".to_string());

        for (order, id) in south.iter().enumerate() {
            let dummy = if id.starts_with("border-") {
                "border"
            } else {
                "dummy"
            };
            g.set_node(
                id.clone(),
                NodeLabel {
                    rank: Some(1),
                    order: Some(order),
                    dummy: Some(dummy.to_string()),
                    ..Default::default()
                },
            );
        }

        g.set_edge("north-high", "border-high");
        for id in south.iter().skip(1) {
            g.set_edge("north-low", id);
        }

        (g, vec![north, south])
    }

    #[test]
    fn indexed_type2_fallback_retains_only_unique_conflicts() {
        for intermediate_count in [8, 64] {
            let (g, layering) = fallback_graph(intermediate_count);
            let expected = find_type2_conflicts(&g, &layering);
            let expected_count: usize = expected.values().map(BTreeSet::len).sum();
            assert_eq!(expected_count, intermediate_count + 2);

            let mut workspace = BkWorkspace::new(&g, &layering);
            assert!(workspace.type2_requires_fallback(1));
            workspace.find_type2_conflicts();

            assert_eq!(workspace.conflicts.len(), expected_count);
            assert_eq!(workspace.conflicts_seen.len(), expected_count);
        }
    }
}
