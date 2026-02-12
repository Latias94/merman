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
    use super::{tree, util};
    use crate::graphlib::{EdgeKey, Graph, GraphOptions};
    use crate::{EdgeLabel, GraphLabel, NodeLabel};

    pub fn feasible_tree(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
        let mut t: Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> = Graph::new(GraphOptions {
            directed: false,
            ..Default::default()
        });

        let Some(start) = g.nodes().next().map(|s| s.to_string()) else {
            return t;
        };
        let size = g.node_count();
        t.set_node(start, tree::TreeNodeLabel::default());

        while tight_tree(&mut t, g) < size {
            let Some(edge) = find_min_slack_edge(&t, g) else {
                // Disconnected graphs can occur in downstream usage. Dagre effectively works
                // per component; here we create a forest by starting a new component root.
                let Some(next_root) = g.nodes().find(|v| !t.has_node(v)).map(|s| s.to_string())
                else {
                    break;
                };
                t.set_node(next_root, tree::TreeNodeLabel::default());
                continue;
            };
            let slack = util::slack(g, &edge);
            let delta = if t.has_node(&edge.v) { slack } else { -slack };
            shift_ranks(&t, g, delta);
        }

        t
    }

    fn tight_tree(
        t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> usize {
        fn dfs(
            v: &str,
            t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
            g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        ) {
            let edges: Vec<EdgeKey> = g.node_edges(v);
            for e in edges {
                let w = if v == e.v { e.w.as_str() } else { e.v.as_str() };
                if !t.has_node(w) && util::slack(g, &e) == 0 {
                    t.set_node(w.to_string(), tree::TreeNodeLabel::default());
                    t.set_edge(v.to_string(), w.to_string());
                    dfs(w, t, g);
                }
            }
        }

        let roots: Vec<String> = t.node_ids();
        for v in roots {
            dfs(&v, t, g);
        }
        t.node_count()
    }

    fn find_min_slack_edge(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Option<EdgeKey> {
        let mut best: Option<(i32, EdgeKey)> = None;
        for e in g.edges() {
            let in_v = t.has_node(&e.v);
            let in_w = t.has_node(&e.w);
            if in_v == in_w {
                continue;
            }
            let edge_slack = util::slack(g, e);
            match &best {
                Some((best_slack, _)) if edge_slack >= *best_slack => {}
                _ => best = Some((edge_slack, e.clone())),
            }
        }
        best.map(|(_, e)| e)
    }

    fn shift_ranks(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        delta: i32,
    ) {
        for v in t.node_ids() {
            let Some(label) = g.node_mut(&v) else {
                continue;
            };
            let Some(rank) = label.rank else {
                continue;
            };
            label.rank = Some(rank + delta);
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

        let mut ranks: HashMap<&str, i32> = HashMap::default();
        for id in g.nodes() {
            let rank = g.node(id).and_then(|lbl| lbl.rank).unwrap_or(0);
            ranks.insert(id, rank);
        }

        let mut best: Option<(i32, EdgeKey)> = None;
        for e in g.edges() {
            let Some(&(_, v_lim)) = t_labels.get(e.v.as_str()) else {
                continue;
            };
            let Some(&(_, w_lim)) = t_labels.get(e.w.as_str()) else {
                continue;
            };
            let v_desc = tail_low <= v_lim && v_lim <= tail_lim;
            let w_desc = tail_low <= w_lim && w_lim <= tail_lim;

            if flip == v_desc && flip != w_desc {
                let v_rank = ranks.get(e.v.as_str()).copied().unwrap_or(0);
                let w_rank = ranks.get(e.w.as_str()).copied().unwrap_or(0);
                let minlen: i32 = g
                    .edge_by_key(e)
                    .map(|lbl| lbl.minlen.max(1) as i32)
                    .unwrap_or(1);
                let slack = w_rank - v_rank - minlen;

                match &best {
                    Some((best_slack, _)) if slack >= *best_slack => {}
                    _ => best = Some((slack, e.clone())),
                }
            }
        }

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
