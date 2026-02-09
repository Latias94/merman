//! Add border segments for compound graphs.
//!
//! Dagre materializes per-rank `"border"` dummy nodes along each cluster so the ordering and
//! positioning steps can route edges around clusters. This mirrors upstream `add-border-segments.js`.

use crate::graphlib::Graph;
use crate::{EdgeLabel, GraphLabel, NodeLabel};

pub fn add_border_segments(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    if !g.options().compound {
        return;
    }

    fn dfs(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str) {
        let children: Vec<String> = g.children(v).into_iter().map(|s| s.to_string()).collect();
        for c in children {
            dfs(g, &c);
        }

        let Some((min_rank, max_rank)) = g.node(v).and_then(|n| Some((n.min_rank?, n.max_rank?)))
        else {
            return;
        };

        let max_rank_usize: usize = max_rank.max(0) as usize;
        if let Some(n) = g.node_mut(v) {
            n.border_left = vec![None; max_rank_usize + 1];
            n.border_right = vec![None; max_rank_usize + 1];
        }

        for rank in min_rank..=max_rank {
            add_border_node(g, "borderLeft", "_bl", v, rank, true);
            add_border_node(g, "borderRight", "_br", v, rank, false);
        }
    }

    let roots: Vec<String> = g
        .children_root()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for v in roots {
        dfs(g, &v);
    }
}

fn add_border_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    prop: &str,
    prefix: &str,
    sg: &str,
    rank: i32,
    is_left: bool,
) {
    let prev = g
        .node(sg)
        .and_then(|n| {
            let idx = (rank - 1) as usize;
            if is_left {
                n.border_left.get(idx).and_then(|v| v.clone())
            } else {
                n.border_right.get(idx).and_then(|v| v.clone())
            }
        })
        .unwrap_or_default();

    let curr = add_dummy_node(
        g,
        NodeLabel {
            width: 0.0,
            height: 0.0,
            rank: Some(rank),
            dummy: Some("border".to_string()),
            border_type: Some(prop.to_string()),
            ..Default::default()
        },
        prefix,
    );

    if let Some(n) = g.node_mut(sg) {
        let idx = rank.max(0) as usize;
        if is_left {
            if idx >= n.border_left.len() {
                n.border_left.resize(idx + 1, None);
            }
            n.border_left[idx] = Some(curr.clone());
        } else {
            if idx >= n.border_right.len() {
                n.border_right.resize(idx + 1, None);
            }
            n.border_right[idx] = Some(curr.clone());
        }
    }

    g.set_parent(curr.clone(), sg.to_string());
    if !prev.is_empty() {
        g.set_edge_with_label(
            prev,
            curr,
            EdgeLabel {
                weight: 1.0,
                ..Default::default()
            },
        );
    }
}

fn add_dummy_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    label: NodeLabel,
    prefix: &str,
) -> String {
    if !g.has_node(prefix) {
        g.set_node(prefix, label);
        return prefix.to_string();
    }
    for i in 1usize.. {
        let v = format!("{prefix}{i}");
        if !g.has_node(&v) {
            g.set_node(&v, label.clone());
            return v;
        }
    }
    unreachable!()
}
