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
}

impl Default for GraphLabel {
    fn default() -> Self {
        Self {
            rankdir: RankDir::TB,
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 10.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeLabel {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct EdgeLabel {}

pub fn layout(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let graph = g.graph().clone();

    let node_ids: Vec<String> = g.nodes().map(|s| s.to_string()).collect();

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
            let next = r.saturating_add(1);
            let entry = rank.entry(e.w.clone()).or_insert(0);
            if next > *entry {
                *entry = next;
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
                w += graph.nodesep;
            }
        }
        rank_heights.push(h);
        rank_widths.push(w);
    }
    let max_rank_width = rank_widths.iter().copied().fold(0.0_f64, |a, b| a.max(b));

    let mut y_cursor = 0.0;
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
            x_cursor += nw + graph.nodesep;
        }

        y_cursor += rank_h + graph.ranksep;
    }
}
