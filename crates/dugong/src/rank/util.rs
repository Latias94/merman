//! Rank helpers (longest-path, slack).

use crate::graphlib::{EdgeKey, Graph};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use rustc_hash::FxHashMap as HashMap;

pub fn longest_path(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    fn dfs(
        v: &str,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        visited: &mut HashMap<String, i32>,
    ) -> i32 {
        if let Some(&rank) = visited.get(v) {
            return rank;
        }

        let mut rank: Option<i32> = None;
        for e in g.out_edges(v, None) {
            let minlen: i32 = g.edge_by_key(&e).map(|lbl| lbl.minlen as i32).unwrap_or(1);
            let candidate = dfs(&e.w, g, visited) - minlen;
            rank = Some(match rank {
                Some(current) => current.min(candidate),
                None => candidate,
            });
        }

        let rank = rank.unwrap_or(0);
        if let Some(label) = g.node_mut(v) {
            label.rank = Some(rank);
        }
        visited.insert(v.to_string(), rank);
        rank
    }

    let sources: Vec<String> = g.sources().into_iter().map(|s| s.to_string()).collect();
    let mut visited: HashMap<String, i32> = HashMap::default();
    for v in sources {
        dfs(&v, g, &mut visited);
    }
}

pub fn slack(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, e: &EdgeKey) -> i32 {
    // Be defensive: callers can provide arbitrary graphs. Missing nodes/ranks are treated
    // as `0` so layout can degrade gracefully instead of panicking.
    let w_rank = g.node(&e.w).and_then(|n| n.rank).unwrap_or(0);
    let v_rank = g.node(&e.v).and_then(|n| n.rank).unwrap_or(0);
    let minlen: i32 = g.edge_by_key(e).map(|lbl| lbl.minlen as i32).unwrap_or(1);
    w_rank - v_rank - minlen
}
