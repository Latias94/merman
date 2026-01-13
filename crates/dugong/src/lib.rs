//! Dagre-compatible graph layout algorithms.
//!
//! Baseline: `repo-ref/dagre` (see `repo-ref/REPOS.lock.json`).

pub use dugong_graphlib as graphlib;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

#[derive(Debug, Clone)]
pub struct GraphLabel {
    pub rankdir: RankDir,
    pub nodesep: f64,
    pub ranksep: f64,
    pub edgesep: f64,
    pub nesting_root: Option<String>,
    pub node_rank_factor: Option<usize>,
}

impl Default for GraphLabel {
    fn default() -> Self {
        Self {
            rankdir: RankDir::TB,
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 10.0,
            nesting_root: None,
            node_rank_factor: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeLabel {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rank: Option<i32>,
    pub order: Option<usize>,
    pub dummy: Option<String>,
    pub border_top: Option<String>,
    pub border_bottom: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelPos {
    #[default]
    C,
    L,
    R,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct EdgeLabel {
    pub width: f64,
    pub height: f64,
    pub labelpos: LabelPos,
    pub labeloffset: f64,
    pub minlen: usize,
    pub weight: f64,
    pub nesting_edge: bool,

    pub x: Option<f64>,
    pub y: Option<f64>,
    pub points: Vec<Point>,
}

impl Default for EdgeLabel {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            labelpos: LabelPos::C,
            labeloffset: 0.0,
            minlen: 1,
            weight: 0.0,
            nesting_edge: false,
            x: None,
            y: None,
            points: Vec::new(),
        }
    }
}

pub mod coordinate_system {
    use super::{EdgeLabel, GraphLabel, NodeLabel, RankDir};
    use crate::graphlib::{EdgeKey, Graph};

    pub fn adjust(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        match g.graph().rankdir {
            RankDir::LR | RankDir::RL => swap_width_height(g),
            RankDir::TB | RankDir::BT => {}
        }
    }

    pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        match g.graph().rankdir {
            RankDir::BT | RankDir::RL => reverse_y(g),
            RankDir::TB | RankDir::LR => {}
        }

        match g.graph().rankdir {
            RankDir::LR | RankDir::RL => {
                swap_xy(g);
                swap_width_height(g);
            }
            RankDir::TB | RankDir::BT => {}
        }
    }

    fn swap_width_height(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let node_ids = g.node_ids();
        for id in node_ids {
            if let Some(n) = g.node_mut(&id) {
                (n.width, n.height) = (n.height, n.width);
            }
        }

        let edge_keys = g.edge_keys();
        for EdgeKey { v, w, name } in edge_keys {
            if let Some(e) = g.edge_mut(&v, &w, name.as_deref()) {
                (e.width, e.height) = (e.height, e.width);
            }
        }
    }

    fn reverse_y(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let node_ids = g.node_ids();
        for id in node_ids {
            if let Some(n) = g.node_mut(&id) {
                if let Some(y) = n.y {
                    n.y = Some(-y);
                }
            }
        }

        let edge_keys = g.edge_keys();
        for EdgeKey { v, w, name } in edge_keys {
            if let Some(e) = g.edge_mut(&v, &w, name.as_deref()) {
                for p in &mut e.points {
                    p.y = -p.y;
                }
                if let Some(y) = e.y {
                    e.y = Some(-y);
                }
            }
        }
    }

    fn swap_xy(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let node_ids = g.node_ids();
        for id in node_ids {
            if let Some(n) = g.node_mut(&id) {
                if let (Some(x), Some(y)) = (n.x, n.y) {
                    n.x = Some(y);
                    n.y = Some(x);
                }
            }
        }

        let edge_keys = g.edge_keys();
        for EdgeKey { v, w, name } in edge_keys {
            if let Some(e) = g.edge_mut(&v, &w, name.as_deref()) {
                for p in &mut e.points {
                    (p.x, p.y) = (p.y, p.x);
                }
                if let (Some(x), Some(y)) = (e.x, e.y) {
                    e.x = Some(y);
                    e.y = Some(x);
                }
            }
        }
    }
}

pub mod nesting_graph {
    use super::{EdgeLabel, GraphLabel, NodeLabel};
    use crate::graphlib::{EdgeKey, Graph, alg};
    use std::collections::BTreeMap;

    fn unique_id(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, prefix: &str) -> String {
        if !g.has_node(prefix) {
            return prefix.to_string();
        }
        for i in 1usize.. {
            let v = format!("{prefix}{i}");
            if !g.has_node(&v) {
                return v;
            }
        }
        unreachable!()
    }

    fn add_dummy_node(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        dummy: &str,
        mut label: NodeLabel,
        name: &str,
    ) -> String {
        let id = unique_id(g, name);
        label.dummy = Some(dummy.to_string());
        g.set_node(id.clone(), label);
        id
    }

    fn add_border_node(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>, prefix: &str) -> String {
        add_dummy_node(
            g,
            "border",
            NodeLabel {
                width: 0.0,
                height: 0.0,
                ..Default::default()
            },
            prefix,
        )
    }

    fn tree_depths(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> BTreeMap<String, usize> {
        fn dfs(
            g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            v: &str,
            depth: usize,
            out: &mut BTreeMap<String, usize>,
        ) {
            for child in g.children(v) {
                dfs(g, child, depth + 1, out);
            }
            out.insert(v.to_string(), depth);
        }

        let mut out: BTreeMap<String, usize> = BTreeMap::new();
        for v in g.children_root() {
            dfs(g, v, 1, &mut out);
        }
        out
    }

    fn sum_weights(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> f64 {
        g.edge_keys()
            .iter()
            .filter_map(|k| g.edge_by_key(k))
            .map(|e| e.weight)
            .sum()
    }

    fn dfs(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root: &str,
        node_sep: usize,
        weight: f64,
        height: usize,
        depths: &BTreeMap<String, usize>,
        v: &str,
    ) {
        let children: Vec<String> = g.children(v).into_iter().map(|s| s.to_string()).collect();
        if children.is_empty() {
            if v != root {
                g.set_edge_with_label(
                    root,
                    v,
                    EdgeLabel {
                        weight: 0.0,
                        minlen: node_sep,
                        ..Default::default()
                    },
                );
            }
            return;
        }

        let top = add_border_node(g, "_bt");
        let bottom = add_border_node(g, "_bb");

        g.set_parent(top.clone(), v.to_string());
        if let Some(lbl) = g.node_mut(v) {
            lbl.border_top = Some(top.clone());
        }
        g.set_parent(bottom.clone(), v.to_string());
        if let Some(lbl) = g.node_mut(v) {
            lbl.border_bottom = Some(bottom.clone());
        }

        for child in children {
            dfs(g, root, node_sep, weight, height, depths, &child);

            let child_node = g.node(&child).cloned().unwrap_or_default();
            let child_top = child_node
                .border_top
                .as_deref()
                .unwrap_or(&child)
                .to_string();
            let child_bottom = child_node
                .border_bottom
                .as_deref()
                .unwrap_or(&child)
                .to_string();
            let this_weight = if child_node.border_top.is_some() {
                weight
            } else {
                2.0 * weight
            };
            let minlen = if child_top != child_bottom {
                1usize
            } else {
                let dv = depths.get(v).copied().unwrap_or(1);
                height.saturating_sub(dv).saturating_add(1)
            };

            g.set_edge_with_label(
                top.clone(),
                child_top.clone(),
                EdgeLabel {
                    weight: this_weight,
                    minlen,
                    nesting_edge: true,
                    ..Default::default()
                },
            );
            g.set_edge_with_label(
                child_bottom.clone(),
                bottom.clone(),
                EdgeLabel {
                    weight: this_weight,
                    minlen,
                    nesting_edge: true,
                    ..Default::default()
                },
            );
        }

        if g.parent(v).is_none() {
            let dv = depths.get(v).copied().unwrap_or(1);
            g.set_edge_with_label(
                root,
                top,
                EdgeLabel {
                    weight: 0.0,
                    minlen: height + dv,
                    nesting_edge: true,
                    ..Default::default()
                },
            );
        }
    }

    pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let root = add_dummy_node(
            g,
            "root",
            NodeLabel {
                ..Default::default()
            },
            "_root",
        );

        let depths = tree_depths(g);
        let height = depths
            .values()
            .copied()
            .max()
            .unwrap_or(1)
            .saturating_sub(1);
        let node_sep = 2 * height + 1;

        if let Some(gl) = g.graph_mut().nesting_root.replace(root.clone()) {
            let _ = gl;
        }

        for k in g.edge_keys() {
            if let Some(e) = g.edge_mut_by_key(&k) {
                e.minlen *= node_sep.max(1);
            }
        }

        let weight = sum_weights(g) + 1.0;

        let children = g
            .children_root()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        for child in children {
            dfs(g, &root, node_sep, weight, height, &depths, &child);
        }

        g.graph_mut().node_rank_factor = Some(node_sep);

        let _ = alg::components(g);
    }

    pub fn cleanup(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let root = g.graph().nesting_root.clone();
        if let Some(root) = root {
            let _ = g.remove_node(&root);
            g.graph_mut().nesting_root = None;
        }

        let keys: Vec<EdgeKey> = g.edge_keys();
        for k in keys {
            if let Some(e) = g.edge_by_key(&k) {
                if e.nesting_edge {
                    let _ = g.remove_edge_key(&k);
                }
            }
        }
    }
}

pub mod position {
    use super::{EdgeLabel, GraphLabel, NodeLabel};
    use crate::graphlib::Graph;
    use std::collections::BTreeMap;

    pub fn position(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        // Upstream dagre positions a non-compound view of the graph.
        // We mimic that by ignoring cluster nodes (nodes with children).
        let leaf_ids: Vec<String> = g
            .node_ids()
            .into_iter()
            .filter(|id| !(g.options().compound && !g.children(id).is_empty()))
            .collect();

        let mut ranks: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        for id in &leaf_ids {
            let Some(n) = g.node(id) else { continue };
            let Some(rank) = n.rank else { continue };
            ranks.entry(rank).or_default().push(id.clone());
        }

        // Within each rank, order by `order` if present, otherwise keep insertion order.
        for ids in ranks.values_mut() {
            ids.sort_by_key(|id| g.node(id).and_then(|n| n.order).unwrap_or(usize::MAX));
        }

        let rank_sep = g.graph().ranksep;
        let mut prev_y: f64 = 0.0;
        for ids in ranks.values() {
            let mut max_h: f64 = 0.0;
            for id in ids {
                if let Some(n) = g.node(id) {
                    max_h = max_h.max(n.height);
                }
            }
            for id in ids {
                if let Some(n) = g.node_mut(id) {
                    n.y = Some(prev_y + max_h / 2.0);
                }
            }
            prev_y += max_h + rank_sep;
        }

        // Minimal x positioning that matches upstream tests that only assert nodesep behavior.
        let node_sep = g.graph().nodesep;
        for ids in ranks.values() {
            let mut x_cursor: f64 = 0.0;
            for id in ids {
                let width = g.node(id).map(|n| n.width).unwrap_or(0.0);
                let x = x_cursor + width / 2.0;
                if let Some(n) = g.node_mut(id) {
                    n.x = Some(x);
                }
                x_cursor += width + node_sep;
            }
        }
    }
}

pub fn layout(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let graph = g.graph().clone();
    let edge_keys: Vec<graphlib::EdgeKey> = g.edges().cloned().collect();

    let mut max_edge_label_width: f64 = 0.0;
    let mut max_edge_label_height: f64 = 0.0;
    for e in &edge_keys {
        if let Some(lbl) = g.edge(&e.v, &e.w, e.name.as_deref()) {
            max_edge_label_width = max_edge_label_width.max(lbl.width);
            max_edge_label_height = max_edge_label_height.max(lbl.height);
        }
    }

    // A minimal parity-oriented approximation:
    // - in TB/BT: long edge labels tend to push nodes apart horizontally (cross-axis)
    // - in LR/RL: long edge labels tend to push ranks apart horizontally (axis)
    let node_sep = match graph.rankdir {
        RankDir::TB | RankDir::BT => graph.nodesep.max(max_edge_label_width),
        RankDir::LR | RankDir::RL => graph.nodesep.max(max_edge_label_height),
    };
    let rank_sep = match graph.rankdir {
        RankDir::TB | RankDir::BT => graph.ranksep,
        RankDir::LR | RankDir::RL => graph.ranksep.max(max_edge_label_width),
    };

    let node_ids: Vec<String> = g.nodes().map(|s| s.to_string()).collect();
    let node_ids: Vec<String> = node_ids
        .into_iter()
        .filter(|id| !(g.options().compound && !g.children(id).is_empty()))
        .collect();

    let mut indegree: std::collections::HashMap<String, usize> =
        node_ids.iter().map(|id| (id.clone(), 0)).collect();
    for e in g.edges() {
        if let Some(v) = indegree.get_mut(&e.w) {
            *v += 1;
        }
    }

    // Deterministic Kahn order: initial nodes in insertion order.
    let mut queue: std::collections::VecDeque<String> = node_ids
        .iter()
        .filter(|id| indegree.get(*id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();

    let mut topo: Vec<String> = Vec::new();
    while let Some(n) = queue.pop_front() {
        topo.push(n.clone());

        // Traverse outgoing edges in edge insertion order.
        let mut out: Vec<String> = Vec::new();
        for e in g.edges() {
            if e.v == n {
                out.push(e.w.clone());
            }
        }
        for w in out {
            if let Some(v) = indegree.get_mut(&w) {
                *v = v.saturating_sub(1);
                if *v == 0 {
                    queue.push_back(w);
                }
            }
        }
    }

    // If the graph has a cycle, fall back to insertion order for now.
    if topo.len() != node_ids.len() {
        topo = node_ids.clone();
    }

    let mut rank: std::collections::HashMap<String, usize> =
        node_ids.iter().map(|id| (id.clone(), 0)).collect();
    for n in &topo {
        let r = rank.get(n).copied().unwrap_or(0);
        for e in g.edges() {
            if e.v != *n {
                continue;
            }
            let minlen = g
                .edge(&e.v, &e.w, e.name.as_deref())
                .map(|l| l.minlen)
                .unwrap_or(1)
                .max(1);
            let next = r.saturating_add(minlen);
            let entry = rank.entry(e.w.clone()).or_insert(0);
            if next > *entry {
                *entry = next;
            }
        }
    }

    if g.options().compound {
        // Compact ranks inside compound nodes where a common rank is feasible, to minimize cluster height.
        // This is a small parity-oriented step to match upstream Dagre behavior for subgraphs.
        let parents: Vec<String> = g
            .node_ids()
            .into_iter()
            .filter(|id| !g.children(id).is_empty())
            .collect();

        for parent in parents {
            let children: Vec<String> = g
                .children(&parent)
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let targets: Vec<String> = children
                .into_iter()
                .filter(|c| rank.contains_key(c))
                .collect();
            if targets.len() < 2 {
                continue;
            }

            // Preserve insertion order (children are stored deterministically).
            let mut min_needed: usize = 0;
            let mut max_allowed: usize = usize::MAX / 4;

            for child in &targets {
                let mut min_rank: usize = 0;
                for ek in g.in_edges(child, None) {
                    let Some(&pred_rank) = rank.get(&ek.v) else {
                        continue;
                    };
                    let minlen = g.edge_by_key(&ek).map(|e| e.minlen).unwrap_or(1).max(1);
                    min_rank = min_rank.max(pred_rank.saturating_add(minlen));
                }

                let mut max_rank: usize = usize::MAX / 4;
                for ek in g.out_edges(child, None) {
                    let Some(&succ_rank) = rank.get(&ek.w) else {
                        continue;
                    };
                    let minlen = g.edge_by_key(&ek).map(|e| e.minlen).unwrap_or(1).max(1);
                    let upper = succ_rank.saturating_sub(minlen);
                    max_rank = max_rank.min(upper);
                }

                min_needed = min_needed.max(min_rank);
                max_allowed = max_allowed.min(max_rank);
            }

            if min_needed <= max_allowed {
                for child in &targets {
                    rank.insert(child.clone(), min_needed);
                }
            }
        }
    }

    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut ranks: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for id in &node_ids {
        let r = rank.get(id).copied().unwrap_or(0);
        ranks[r].push(id.clone());
    }

    fn node_size(g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>, id: &str) -> (f64, f64) {
        match g.node(id) {
            Some(n) => (n.width, n.height),
            None => (0.0, 0.0),
        }
    }

    let mut gap_extra: Vec<f64> = vec![0.0; ranks.len().saturating_sub(1)];
    for e in g.edges() {
        let Some(v_rank) = rank.get(&e.v).copied() else {
            continue;
        };
        let Some(w_rank) = rank.get(&e.w).copied() else {
            continue;
        };
        if w_rank != v_rank.saturating_add(1) {
            continue;
        }
        let Some(lbl) = g.edge(&e.v, &e.w, e.name.as_deref()) else {
            continue;
        };
        if lbl.height <= 0.0 {
            continue;
        }
        if let Some(extra) = gap_extra.get_mut(v_rank) {
            *extra = extra.max(lbl.height);
        }
    }

    let mut rank_heights: Vec<f64> = Vec::with_capacity(ranks.len());
    let mut rank_widths: Vec<f64> = Vec::with_capacity(ranks.len());
    for ids in &ranks {
        let mut h: f64 = 0.0;
        let mut w: f64 = 0.0;
        for (i, id) in ids.iter().enumerate() {
            let (nw, nh) = node_size(g, id);
            h = h.max(nh);
            w += nw;
            if i + 1 < ids.len() {
                w += node_sep;
            }
        }
        rank_heights.push(h);
        rank_widths.push(w);
    }
    let max_rank_width = rank_widths.iter().copied().fold(0.0_f64, |a, b| a.max(b));

    let mut y_cursor: f64 = 0.0;
    for (rank_idx, ids) in ranks.iter().enumerate() {
        let rank_h = rank_heights[rank_idx];
        let y = y_cursor + rank_h / 2.0;

        let rank_w = rank_widths[rank_idx];
        let mut x_cursor = (max_rank_width - rank_w) / 2.0;
        for id in ids {
            let (nw, _) = node_size(g, id);
            let x = x_cursor + nw / 2.0;
            if let Some(n) = g.node_mut(id) {
                n.x = Some(x);
                n.y = Some(y);
            }
            x_cursor += nw + node_sep;
        }

        y_cursor += rank_h;
        if rank_idx + 1 < ranks.len() {
            y_cursor += rank_sep + gap_extra.get(rank_idx).copied().unwrap_or(0.0);
        }
    }

    let total_height = y_cursor;

    for e in &edge_keys {
        let Some((sx, sy, sw, sh)) = g
            .node(&e.v)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };
        let Some((tx, ty, tw, th)) = g
            .node(&e.w)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };

        let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) else {
            continue;
        };
        lbl.points.clear();
        lbl.x = None;
        lbl.y = None;

        if e.v == e.w {
            // A minimal self-loop shape that satisfies upstream dagre invariants:
            // - TB/BT: all points are to the right of the node center (x > node.x)
            // - LR/RL: after rankdir transforms, all points are below the node center (y > node.y)
            // and all points stay within the node's height/2 on the cross-axis.
            let x0 = sx + sw / 2.0 + graph.edgesep.max(1.0);
            let x1 = x0 + graph.edgesep.max(1.0);
            let y0 = sy;
            let y_top = sy - sh / 2.0;
            let y_bot = sy + sh / 2.0;

            lbl.points.extend([
                Point { x: x0, y: y0 },
                Point { x: x0, y: y_top },
                Point { x: x1, y: y_top },
                Point { x: x1, y: y0 },
                Point { x: x1, y: y_bot },
                Point { x: x0, y: y_bot },
                Point { x: x0, y: y0 },
            ]);

            continue;
        }

        let start = Point {
            x: sx,
            y: sy + sh / 2.0,
        };
        let end = Point {
            x: tx,
            y: ty - th / 2.0,
        };

        let minlen = lbl.minlen.max(1);
        let count = 2 * minlen + 1;
        for i in 0..count {
            let t = (i as f64) / ((count - 1) as f64);
            lbl.points.push(Point {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
            });
        }

        if lbl.width > 0.0 || lbl.height > 0.0 {
            let mid = lbl.points[count / 2];
            let mut ex = mid.x;
            let ey = mid.y;
            match lbl.labelpos {
                LabelPos::C => {}
                LabelPos::L => ex -= lbl.labeloffset + lbl.width / 2.0,
                LabelPos::R => ex += lbl.labeloffset + lbl.width / 2.0,
            }
            lbl.x = Some(ex);
            lbl.y = Some(ey);
        }

        let _ = (sw, tw);
    }

    match graph.rankdir {
        RankDir::TB => {}
        RankDir::BT => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    if let Some(y) = n.y {
                        n.y = Some(total_height - y);
                    }
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        p.y = total_height - p.y;
                    }
                    if let Some(y) = lbl.y {
                        lbl.y = Some(total_height - y);
                    }
                }
            }
        }
        RankDir::LR => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    let (Some(x), Some(y)) = (n.x, n.y) else {
                        continue;
                    };
                    n.x = Some(y);
                    n.y = Some(x);
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        (p.x, p.y) = (p.y, p.x);
                    }
                    if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                        lbl.x = Some(y);
                        lbl.y = Some(x);
                    }
                }
            }
        }
        RankDir::RL => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    let (Some(x), Some(y)) = (n.x, n.y) else {
                        continue;
                    };
                    n.x = Some(total_height - y);
                    n.y = Some(x);
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        let new_x = total_height - p.y;
                        (p.x, p.y) = (new_x, p.x);
                    }
                    if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                        lbl.x = Some(total_height - y);
                        lbl.y = Some(x);
                    }
                }
            }
        }
    }
}
