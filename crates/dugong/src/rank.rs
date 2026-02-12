//! Ranking algorithms (network simplex, tight tree, longest path).
//!
//! Ported from Dagre's `rank.js` and related helpers. The implementation here is parity-oriented
//! (deterministic and defensive) to support headless diagram rendering.

pub fn rank(g: &mut crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>) {
    let ranker = g.graph().ranker.clone();
    match ranker.as_deref() {
        Some("network-simplex") => network_simplex::network_simplex(g),
        Some("tight-tree") => {
            util::longest_path(g);
            let _ = feasible_tree::feasible_tree(g);
        }
        Some("longest-path") => util::longest_path(g),
        Some("none") => {}
        _ => network_simplex::network_simplex(g),
    }
}

pub mod util {
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
}

pub mod tree {
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct TreeNodeLabel {
        pub low: i32,
        pub lim: i32,
        pub parent: Option<String>,
    }

    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct TreeEdgeLabel {
        pub cutvalue: f64,
    }
}

pub mod feasible_tree {
    use super::tree;
    use crate::graphlib::{EdgeKey, Graph, GraphOptions};
    use crate::{EdgeLabel, GraphLabel, NodeLabel};

    pub fn feasible_tree(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
        let mut rank_by_ix: Vec<i32> = Vec::new();
        g.for_each_node_ix(|ix, _id, lbl| {
            if ix >= rank_by_ix.len() {
                rank_by_ix.resize(ix + 1, 0);
            }
            rank_by_ix[ix] = lbl.rank.unwrap_or(0);
        });
        let mut in_tree_by_ix: Vec<bool> = vec![false; rank_by_ix.len()];

        let mut t: Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> = Graph::new(GraphOptions {
            directed: false,
            ..Default::default()
        });

        let Some(start) = g.nodes().next().map(|s| s.to_string()) else {
            return t;
        };
        let size = g.node_count();
        t.set_node(start, tree::TreeNodeLabel::default());
        if let Some(ix) = g.node_ix(t.nodes().next().expect("start node should exist")) {
            if ix >= in_tree_by_ix.len() {
                in_tree_by_ix.resize(ix + 1, false);
                rank_by_ix.resize(ix + 1, 0);
            }
            in_tree_by_ix[ix] = true;
        }

        while tight_tree(&mut t, g, &rank_by_ix, &mut in_tree_by_ix) < size {
            let Some((slack, in_v)) = find_min_slack_edge(g, &rank_by_ix, &in_tree_by_ix) else {
                // Disconnected graphs can occur in downstream usage. Dagre effectively works
                // per component; here we create a forest by starting a new component root.
                let Some(next_root) = g.nodes().find(|v| !t.has_node(v)).map(|s| s.to_string())
                else {
                    break;
                };
                if let Some(ix) = g.node_ix(&next_root) {
                    if ix >= in_tree_by_ix.len() {
                        in_tree_by_ix.resize(ix + 1, false);
                        rank_by_ix.resize(ix + 1, 0);
                    }
                    in_tree_by_ix[ix] = true;
                }
                t.set_node(next_root, tree::TreeNodeLabel::default());
                continue;
            };
            let delta = if in_v { slack } else { -slack };
            shift_ranks(&t, g, &mut rank_by_ix, delta);
        }

        t
    }

    fn tight_tree(
        t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        rank_by_ix: &[i32],
        in_tree_by_ix: &mut Vec<bool>,
    ) -> usize {
        let roots: Vec<String> = t.node_ids();
        for root in roots {
            let mut stack: Vec<String> = vec![root];

            while let Some(v) = stack.pop() {
                let mut handle_incident_edge = |ek: &EdgeKey, lbl: &EdgeLabel| {
                    let w = if v == ek.v {
                        ek.w.as_str()
                    } else {
                        ek.v.as_str()
                    };
                    if t.has_node(w) {
                        return;
                    }

                    let minlen: i32 = lbl.minlen.max(1) as i32;

                    let Some(tail_ix) = g.node_ix(&ek.v) else {
                        return;
                    };
                    let Some(head_ix) = g.node_ix(&ek.w) else {
                        return;
                    };
                    let tail_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                    let head_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);

                    // Compute slack for the directed edge `(ek.v -> ek.w)`.
                    let slack = head_rank - tail_rank - minlen;
                    if slack == 0 {
                        let w = w.to_string();
                        stack.push(w.clone());
                        if let Some(w_ix) = g.node_ix(&w) {
                            if w_ix >= in_tree_by_ix.len() {
                                in_tree_by_ix.resize(w_ix + 1, false);
                            }
                            in_tree_by_ix[w_ix] = true;
                        }
                        t.set_edge(v.clone(), w);
                    }
                };

                g.for_each_out_edge(&v, None, |ek, lbl| handle_incident_edge(ek, lbl));
                if g.is_directed() {
                    g.for_each_in_edge(&v, None, |ek, lbl| handle_incident_edge(ek, lbl));
                }
            }
        }
        t.node_count()
    }

    fn find_min_slack_edge(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        rank_by_ix: &[i32],
        in_tree_by_ix: &[bool],
    ) -> Option<(i32, bool)> {
        let mut best: Option<(i32, bool)> = None;
        g.for_each_edge_ix(|v_ix, w_ix, _key, lbl| {
            let in_v = in_tree_by_ix.get(v_ix).copied().unwrap_or(false);
            let in_w = in_tree_by_ix.get(w_ix).copied().unwrap_or(false);
            if in_v == in_w {
                return;
            }

            let v_rank = rank_by_ix.get(v_ix).copied().unwrap_or(0);
            let w_rank = rank_by_ix.get(w_ix).copied().unwrap_or(0);
            let minlen: i32 = lbl.minlen.max(1) as i32;
            let slack = w_rank - v_rank - minlen;

            match &best {
                Some((best_slack, _)) if slack >= *best_slack => {}
                _ => best = Some((slack, in_v)),
            }
        });
        best
    }

    fn shift_ranks(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        rank_by_ix: &mut Vec<i32>,
        delta: i32,
    ) {
        for v in t.nodes() {
            let Some(label) = g.node_mut(v) else {
                continue;
            };
            let Some(rank) = label.rank else {
                continue;
            };
            let new_rank = rank + delta;
            label.rank = Some(new_rank);
            if let Some(ix) = g.node_ix(v) {
                if ix >= rank_by_ix.len() {
                    rank_by_ix.resize(ix + 1, 0);
                }
                rank_by_ix[ix] = new_rank;
            }
        }
    }
}

pub mod network_simplex {
    use super::{feasible_tree, tree, util};
    use crate::graphlib::{EdgeKey, Graph, alg};
    use crate::{EdgeLabel, GraphLabel, NodeLabel};
    use rustc_hash::FxHashMap as HashMap;
    use rustc_hash::FxHashSet as HashSet;

    pub fn network_simplex(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
        let mut simplified = crate::util::simplify(g);
        util::longest_path(&mut simplified);
        let mut t = feasible_tree::feasible_tree(&mut simplified);
        init_low_lim_values(&mut t, None);
        init_cut_values(&mut t, &simplified);

        while let Some(e) = leave_edge(&t) {
            let f = enter_edge(&t, &simplified, &e);
            exchange_edges(&mut t, &mut simplified, &e, &f);
        }

        for v in g.node_ids() {
            if let Some(rank) = simplified.node(&v).and_then(|n| n.rank) {
                if let Some(lbl) = g.node_mut(&v) {
                    lbl.rank = Some(rank);
                }
            }
        }
    }

    pub fn init_low_lim_values(
        tree: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        root: Option<&str>,
    ) {
        let Some(root) = root
            .map(|s| s.to_string())
            .or_else(|| tree.nodes().next().map(|s| s.to_string()))
        else {
            return;
        };

        let mut visited: HashSet<String> = HashSet::default();
        let _ = dfs_assign_low_lim(tree, &mut visited, 1, &root, None);
    }

    fn dfs_assign_low_lim(
        tree: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        visited: &mut HashSet<String>,
        next_lim: i32,
        v: &str,
        parent: Option<&str>,
    ) -> i32 {
        let low = next_lim;
        visited.insert(v.to_string());

        let neighbors: Vec<String> = tree
            .neighbors(v)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let mut next_lim = next_lim;
        for w in neighbors {
            if !visited.contains(&w) {
                next_lim = dfs_assign_low_lim(tree, visited, next_lim, &w, Some(v));
            }
        }

        if let Some(label) = tree.node_mut(v) {
            label.low = low;
            label.lim = next_lim;
            label.parent = parent.map(|p| p.to_string());
        }
        next_lim + 1
    }

    pub fn init_cut_values(
        t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        let mut vs: Vec<String> = {
            let roots: Vec<&str> = t.nodes().collect();
            alg::postorder(t, &roots)
        };
        let _ = vs.pop();
        for v in vs {
            assign_cut_value(t, g, &v);
        }
    }

    fn assign_cut_value(
        t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        child: &str,
    ) {
        let Some(parent) = t.node(child).and_then(|lbl| lbl.parent.clone()) else {
            return;
        };
        let cutvalue = calc_cut_value(t, g, child);
        if let Some(edge) = t.edge_mut(child, &parent, None) {
            edge.cutvalue = cutvalue;
        }
    }

    pub fn calc_cut_value(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        child: &str,
    ) -> f64 {
        let Some(parent) = t.node(child).and_then(|lbl| lbl.parent.as_deref()) else {
            return 0.0;
        };

        let mut child_is_tail = true;
        let mut graph_edge = g.edge(child, parent, None);
        if graph_edge.is_none() {
            child_is_tail = false;
            graph_edge = g.edge(parent, child, None);
        }
        let Some(graph_edge) = graph_edge else {
            return 0.0;
        };

        let mut cut_value = graph_edge.weight;

        g.for_each_out_edge(child, None, |ek, lbl| {
            let other = ek.w.as_str();
            if other == parent {
                return;
            }

            let points_to_head = child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if is_tree_edge(t, child, other) {
                if let Some(other_edge) = t.edge(child, other, None) {
                    let other_cut_value = other_edge.cutvalue;
                    cut_value += if points_to_head {
                        -other_cut_value
                    } else {
                        other_cut_value
                    };
                }
            }
        });

        g.for_each_in_edge(child, None, |ek, lbl| {
            let other = ek.v.as_str();
            if other == parent {
                return;
            }

            let points_to_head = !child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if is_tree_edge(t, child, other) {
                if let Some(other_edge) = t.edge(child, other, None) {
                    let other_cut_value = other_edge.cutvalue;
                    cut_value += if points_to_head {
                        -other_cut_value
                    } else {
                        other_cut_value
                    };
                }
            }
        });

        cut_value
    }

    pub fn leave_edge(t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>) -> Option<EdgeKey> {
        t.edges()
            .find(|e| {
                t.edge_by_key(e)
                    .map(|lbl| lbl.cutvalue < 0.0)
                    .unwrap_or(false)
            })
            .cloned()
    }

    pub fn enter_edge(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        edge: &EdgeKey,
    ) -> EdgeKey {
        let (v, w) = if g.has_edge(&edge.v, &edge.w, None) {
            (edge.v.as_str(), edge.w.as_str())
        } else {
            (edge.w.as_str(), edge.v.as_str())
        };

        let mut t_labels: HashMap<&str, (i32, i32)> = HashMap::default(); // id -> (low, lim)
        for id in t.nodes() {
            let Some(lbl) = t.node(id) else {
                continue;
            };
            t_labels.insert(id, (lbl.low, lbl.lim));
        }

        let Some(&(v_low, v_lim)) = t_labels.get(v) else {
            return edge.clone();
        };
        let Some(&(w_low, w_lim)) = t_labels.get(w) else {
            return edge.clone();
        };
        let ((tail_low, tail_lim), flip) = if v_lim > w_lim {
            ((w_low, w_lim), true)
        } else {
            ((v_low, v_lim), false)
        };

        let mut g_rank: Vec<i32> = Vec::new();
        g.for_each_node_ix(|g_ix, _, lbl| {
            if g_ix >= g_rank.len() {
                g_rank.resize(g_ix + 1, 0);
            }
            g_rank[g_ix] = lbl.rank.unwrap_or(0);
        });

        let mut best: Option<(i32, EdgeKey)> = None;
        g.for_each_edge_ix(|g_v_ix, g_w_ix, key, lbl| {
            let Some(&(_, v_lim)) = t_labels.get(key.v.as_str()) else {
                return;
            };
            let Some(&(_, w_lim)) = t_labels.get(key.w.as_str()) else {
                return;
            };
            let v_desc = tail_low <= v_lim && v_lim <= tail_lim;
            let w_desc = tail_low <= w_lim && w_lim <= tail_lim;

            if flip == v_desc && flip != w_desc {
                let v_rank = g_rank.get(g_v_ix).copied().unwrap_or(0);
                let w_rank = g_rank.get(g_w_ix).copied().unwrap_or(0);
                let minlen: i32 = (lbl.minlen.max(1)) as i32;
                let slack = w_rank - v_rank - minlen;

                match &best {
                    Some((best_slack, _)) if slack >= *best_slack => {}
                    _ => best = Some((slack, key.clone())),
                }
            }
        });

        best.map(|(_, e)| e).unwrap_or_else(|| edge.clone())
    }

    pub fn exchange_edges(
        t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        e: &EdgeKey,
        f: &EdgeKey,
    ) {
        let _ = t.remove_edge(&e.v, &e.w, None);
        t.set_edge(f.v.clone(), f.w.clone());
        init_low_lim_values(t, None);
        init_cut_values(t, g);
        update_ranks(t, g);
    }

    fn update_ranks(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        let Some(root) = t
            .nodes()
            .find(|v| t.node(v).map(|lbl| lbl.parent.is_none()).unwrap_or(false))
            .or_else(|| t.nodes().next())
        else {
            return;
        };

        let vs = alg::preorder(t, &[root]);
        for v in vs.into_iter().skip(1) {
            let Some(parent) = t.node(&v).and_then(|lbl| lbl.parent.clone()) else {
                continue;
            };

            let (minlen, flipped) = match g.edge(&v, &parent, None) {
                Some(e) => (e.minlen as i32, false),
                None => {
                    let Some(e) = g.edge(&parent, &v, None) else {
                        continue;
                    };
                    (e.minlen as i32, true)
                }
            };

            let Some(parent_rank) = g.node(&parent).and_then(|n| n.rank) else {
                continue;
            };
            let rank = if flipped {
                parent_rank + minlen
            } else {
                parent_rank - minlen
            };
            if let Some(node) = g.node_mut(&v) {
                node.rank = Some(rank);
            }
        }
    }

    fn is_tree_edge(
        tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        u: &str,
        v: &str,
    ) -> bool {
        tree.has_edge(u, v, None)
    }
}
