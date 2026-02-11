//! Brandes & Köpf (BK) horizontal compaction.
//!
//! This module is a parity-oriented port of Dagre's `position/bk` helpers.

use crate::graphlib::{EdgeKey, Graph, GraphOptions};
use crate::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};
use rustc_hash::FxHashMap as HashMap;
use std::collections::{BTreeMap, BTreeSet};

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
                    for u in g.predecessors(scan_node) {
                        let Some(u_label) = g.node(u) else {
                            continue;
                        };
                        let u_pos = u_label.order.unwrap_or(0);
                        let scan_dummy = g
                            .node(scan_node)
                            .map(|n| n.dummy.is_some())
                            .unwrap_or(false);
                        let u_dummy = u_label.dummy.is_some();

                        if (u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy) {
                            add_conflict(&mut conflicts, u, scan_node);
                        }
                    }
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
                for u in g.predecessors(v) {
                    let Some(u_node) = g.node(u) else {
                        continue;
                    };
                    if u_node.dummy.is_some() {
                        let u_order = u_node.order.unwrap_or(0) as isize;
                        if u_order < prev_north_border || u_order > next_north_border {
                            add_conflict(conflicts, u, v);
                        }
                    }
                }
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
                let predecessors = g.predecessors(v);
                if let Some(u) = predecessors.first() {
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
        return g
            .predecessors(v)
            .into_iter()
            .find(|u| g.node(u).map(|n| n.dummy.is_some()).unwrap_or(false))
            .map(|u| u.to_string());
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

pub fn horizontal_compaction(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let mut xs: HashMap<String, f64> = HashMap::default();
    let block_g = build_block_graph(g, layering, root, reverse_sep);
    let border_type = if reverse_sep {
        "borderLeft"
    } else {
        "borderRight"
    };

    fn iterate<F, N>(block_g: &Graph<(), f64, ()>, mut set_xs: F, mut next_nodes: N)
    where
        F: FnMut(&str),
        N: FnMut(&str) -> Vec<String>,
    {
        let mut stack: Vec<String> = block_g.nodes().map(|n| n.to_string()).collect();
        let mut visited: HashMap<String, bool> = HashMap::default();

        while let Some(elem) = stack.pop() {
            if visited.get(&elem).copied().unwrap_or(false) {
                set_xs(&elem);
                continue;
            }

            visited.insert(elem.clone(), true);
            stack.push(elem.clone());
            for next in next_nodes(&elem) {
                stack.push(next);
            }
        }
    }

    // First pass: assign smallest coordinates
    {
        let mut set = |elem: &str| {
            let mut best: f64 = 0.0;
            for e in block_g.in_edges(elem, None) {
                let w = *block_g.edge_by_key(&e).unwrap_or(&0.0);
                let x_v = xs.get(&e.v).copied().unwrap_or(0.0);
                best = best.max(x_v + w);
            }
            xs.insert(elem.to_string(), best);
        };
        let next = |elem: &str| {
            block_g
                .predecessors(elem)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        };
        iterate(&block_g, &mut set, next);
    }

    // Second pass: assign greatest coordinates
    {
        let mut set = |elem: &str| {
            let mut min: f64 = f64::INFINITY;
            for e in block_g.out_edges(elem, None) {
                let w = *block_g.edge_by_key(&e).unwrap_or(&0.0);
                let x_w = xs.get(&e.w).copied().unwrap_or(0.0);
                min = min.min(x_w - w);
            }

            let node = g.node(elem);
            let Some(node) = node else {
                return;
            };
            if min.is_finite() && node.border_type.as_deref() != Some(border_type) {
                let cur = xs.get(elem).copied().unwrap_or(0.0);
                xs.insert(elem.to_string(), cur.max(min));
            }
        };
        let next = |elem: &str| {
            block_g
                .successors(elem)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        };
        iterate(&block_g, &mut set, next);
    }

    // Assign x coordinates to all nodes based on their block root.
    let mut out: HashMap<String, f64> = HashMap::default();
    for (v, r) in align {
        let x = xs.get(root.get(v).unwrap_or(r)).copied().unwrap_or(0.0);
        out.insert(v.clone(), x);
    }
    out
}

fn build_block_graph(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    reverse_sep: bool,
) -> Graph<(), f64, ()> {
    let mut block_graph: Graph<(), f64, ()> = Graph::new(GraphOptions::default());
    for layer in layering {
        let mut u: Option<&str> = None;
        for v in layer {
            let v_root = root.get(v).cloned().unwrap_or_else(|| v.clone());
            block_graph.ensure_node(v_root.clone());

            if let Some(u) = u {
                let u_root = root.get(u).cloned().unwrap_or_else(|| u.to_string());
                let prev_max = block_graph
                    .edge(&u_root, &v_root, None)
                    .copied()
                    .unwrap_or(0.0);
                let sep = sep(g, v, u, reverse_sep);
                block_graph.set_edge_with_label(u_root, v_root, sep.max(prev_max));
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
    let mut conflicts = find_type1_conflicts(g, &layering);
    let type2 = find_type2_conflicts(g, &layering);
    for (v, ws) in type2 {
        for w in ws {
            add_conflict(&mut conflicts, &v, &w);
        }
    }

    let mut xss: HashMap<String, HashMap<String, f64>> = HashMap::default();

    for vert in ["u", "d"] {
        let mut adjusted_layering = if vert == "u" {
            layering.clone()
        } else {
            layering.iter().cloned().rev().collect::<Vec<_>>()
        };

        for horiz in ["l", "r"] {
            if horiz == "r" {
                adjusted_layering = adjusted_layering
                    .iter()
                    .map(|inner| inner.iter().cloned().rev().collect())
                    .collect();
            }

            let neighbor_fn = |v: &str| {
                if vert == "u" {
                    g.predecessors(v)
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    g.successors(v).into_iter().map(|s| s.to_string()).collect()
                }
            };

            let align = vertical_alignment(g, &adjusted_layering, &conflicts, neighbor_fn);
            let mut xs = horizontal_compaction(
                g,
                &adjusted_layering,
                &align.root,
                &align.align,
                horiz == "r",
            );
            if horiz == "r" {
                for v in xs.values_mut() {
                    *v = -*v;
                }
            }

            xss.insert(format!("{vert}{horiz}"), xs);
        }
    }

    let smallest = find_smallest_width_alignment(g, &xss);
    align_coordinates(&mut xss, &smallest);
    balance(&xss, g.graph().align.as_deref())
}

fn sep(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str, w: &str, reverse_sep: bool) -> f64 {
    let v_label = g.node(v).cloned().unwrap_or_default();
    let w_label = g.node(w).cloned().unwrap_or_default();

    let mut sum: f64 = 0.0;
    let mut delta: f64 = 0.0;

    sum += v_label.width / 2.0;
    if let Some(labelpos) = v_label.labelpos {
        delta = match labelpos {
            LabelPos::L => -v_label.width / 2.0,
            LabelPos::R => v_label.width / 2.0,
            LabelPos::C => 0.0,
        };
    }
    if delta != 0.0 {
        sum += if reverse_sep { delta } else { -delta };
    }
    delta = 0.0;

    let node_sep = g.graph().nodesep;
    let edge_sep = g.graph().edgesep;

    sum += if v_label.dummy.is_some() {
        edge_sep
    } else {
        node_sep
    } / 2.0;
    sum += if w_label.dummy.is_some() {
        edge_sep
    } else {
        node_sep
    } / 2.0;

    sum += w_label.width / 2.0;
    if let Some(labelpos) = w_label.labelpos {
        delta = match labelpos {
            LabelPos::L => w_label.width / 2.0,
            LabelPos::R => -w_label.width / 2.0,
            LabelPos::C => 0.0,
        };
    }
    if delta != 0.0 {
        sum += if reverse_sep { delta } else { -delta };
    }

    sum
}

fn width(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str) -> f64 {
    g.node(v).map(|n| n.width).unwrap_or(0.0)
}

#[allow(dead_code)]
fn edge_key(v: &str, w: &str) -> EdgeKey {
    EdgeKey::new(v, w, None::<String>)
}
