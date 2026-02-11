//! Break cycles by reversing a feedback arc set (FAS).
//!
//! This mirrors Dagre's `acyclic.js`. Mermaid uses the default (DFS-based) variant by default,
//! but can opt into the greedy strategy.

use crate::graphlib::{EdgeKey, Graph};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use std::collections::BTreeSet;

pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let fas = if g
        .graph()
        .acyclicer
        .as_deref()
        .is_some_and(|s| s == "greedy")
    {
        crate::greedy_fas::greedy_fas_with_weight(g, |lbl: &EdgeLabel| {
            if !lbl.weight.is_finite() {
                return 0;
            }
            lbl.weight.round() as i64
        })
    } else {
        dfs_fas(g)
    };

    for e in fas.into_iter().filter(|e| e.v != e.w) {
        let Some(label) = g.edge_by_key(&e).cloned() else {
            continue;
        };
        let _ = g.remove_edge_key(&e);

        let mut label = label;
        label.forward_name = e.name.clone();
        label.reversed = true;

        let name = unique_rev_name(g, &e.w, &e.v);
        g.set_edge_named(e.w, e.v, Some(name), Some(label));
    }
}

pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let edge_keys = g.edge_keys();
    for e in edge_keys {
        let Some(label) = g.edge_by_key(&e).cloned() else {
            continue;
        };
        if !label.reversed {
            continue;
        }
        let _ = g.remove_edge_key(&e);

        let mut label = label;
        let forward_name = label.forward_name.take();
        label.reversed = false;
        label.points.reverse();
        g.set_edge_named(e.w, e.v, forward_name, Some(label));
    }
}

fn unique_rev_name(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str, w: &str) -> String {
    for i in 1usize.. {
        let candidate = format!("rev{i}");
        if !g.has_edge(v, w, Some(&candidate)) {
            return candidate;
        }
    }
    unreachable!()
}

fn dfs_fas(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<EdgeKey> {
    // Ported from Dagre `lib/acyclic.js` (dfsFAS) as used by Mermaid `@11.12.2`.
    let mut fas: Vec<EdgeKey> = Vec::new();
    let mut stack: BTreeSet<String> = BTreeSet::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();

    fn dfs(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        v: &str,
        visited: &mut BTreeSet<String>,
        stack: &mut BTreeSet<String>,
        fas: &mut Vec<EdgeKey>,
    ) {
        if !visited.insert(v.to_string()) {
            return;
        }
        stack.insert(v.to_string());
        for e in g.out_edges(v, None) {
            if e.v == e.w {
                continue;
            }
            if stack.contains(&e.w) {
                fas.push(e);
            } else {
                dfs(g, &e.w, visited, stack, fas);
            }
        }
        stack.remove(v);
    }

    // Dagre's `dfsFAS` iterates nodes in `g.nodes()` order (insertion order).
    for v in g.nodes() {
        dfs(g, v, &mut visited, &mut stack, &mut fas);
    }
    fas
}
