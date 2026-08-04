//! Feasible tree construction used by the network simplex ranker.

use super::tree;
use crate::graphlib::{Graph, GraphOptions};
use crate::work::{checked_add, checked_mul, checked_ordered_key_updates};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};

pub fn feasible_tree(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
    let mut work_control = NoopWorkControl;
    feasible_tree_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject feasible-tree work")
}

pub(crate) fn feasible_tree_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>, WorkError> {
    work_control.charge(feasible_tree_setup_work_units(g)?)?;
    crate::rank::validate_rank_arithmetic(g)?;
    let mut rank_by_ix: Vec<i128> = Vec::new();
    g.for_each_node_ix(|ix, _id, lbl| {
        if ix >= rank_by_ix.len() {
            rank_by_ix.resize(ix + 1, 0);
        }
        rank_by_ix[ix] = i128::from(lbl.rank.unwrap_or(0));
    });
    let mut in_tree_by_ix: Vec<bool> = vec![false; rank_by_ix.len()];
    let mut tree_g_ixs: Vec<usize> = Vec::new();

    let mut t: Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });

    let Some(start) = g.nodes().next().map(|s| s.to_string()) else {
        return Ok(t);
    };
    let size = g.node_count();
    t.set_node(start.clone(), tree::TreeNodeLabel::default());
    if let Some(ix) = g.node_ix(&start) {
        if ix >= in_tree_by_ix.len() {
            in_tree_by_ix.resize(ix + 1, false);
            rank_by_ix.resize(ix + 1, 0);
        }
        in_tree_by_ix[ix] = true;
        tree_g_ixs.push(ix);
    }

    loop {
        work_control.charge(feasible_tree_iteration_work_units(g)?)?;
        if tight_tree(&mut t, g, &rank_by_ix, &mut in_tree_by_ix, &mut tree_g_ixs) >= size {
            break;
        }
        let Some((slack, in_v)) = find_min_slack_edge(g, &rank_by_ix, &in_tree_by_ix) else {
            // Disconnected graphs can occur in downstream usage. Dagre effectively works
            // per component; here we create a forest by starting a new component root.
            let mut next_root: Option<(usize, String)> = None;
            g.for_each_node_ix(|ix, id, _lbl| {
                if next_root.is_some() {
                    return;
                }
                if in_tree_by_ix.get(ix).copied().unwrap_or(false) {
                    return;
                }
                next_root = Some((ix, id.to_string()));
            });
            let Some((ix, next_root)) = next_root else {
                break;
            };
            if ix >= in_tree_by_ix.len() {
                in_tree_by_ix.resize(ix + 1, false);
                rank_by_ix.resize(ix + 1, 0);
            }
            in_tree_by_ix[ix] = true;
            tree_g_ixs.push(ix);
            t.set_node(next_root, tree::TreeNodeLabel::default());
            continue;
        };
        let delta = if in_v { slack } else { -slack };
        shift_ranks(g, &mut rank_by_ix, &tree_g_ixs, delta)?;
    }

    Ok(t)
}

fn feasible_tree_setup_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    let linear_work = checked_add(
        checked_mul(g.node_count(), 4)?,
        checked_mul(g.edge_count(), 2)?,
    )?;
    let numeric_node_work =
        checked_ordered_key_updates(g.node_count(), g.array_index_node_count())?;
    checked_add(linear_work, numeric_node_work)
}

fn feasible_tree_iteration_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    let node_work = checked_mul(g.node_count(), 4)?;
    let edge_work = checked_mul(g.edge_count(), 3)?;
    checked_add(node_work, edge_work)
}

fn tight_tree(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i128],
    in_tree_by_ix: &mut Vec<bool>,
    tree_g_ixs: &mut Vec<usize>,
) -> usize {
    if g.is_directed() {
        let mut stack_ix: Vec<usize> = Vec::new();
        stack_ix.extend(tree_g_ixs.iter().copied());
        while let Some(v_ix) = stack_ix.pop() {
            let Some(v_id) = g.node_id_by_ix(v_ix) else {
                continue;
            };
            let v = v_id.to_string();

            g.for_each_out_edge_ix(v_ix, None, |tail_ix, head_ix, _ek, lbl| {
                if in_tree_by_ix.get(head_ix).copied().unwrap_or(false) {
                    return;
                }

                let tail_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                let head_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);
                let minlen = lbl.minlen.max(1) as i128;
                let slack = head_rank - tail_rank - minlen;
                if slack == 0 {
                    let Some(w_id) = g.node_id_by_ix(head_ix) else {
                        return;
                    };
                    let w = w_id.to_string();

                    stack_ix.push(head_ix);
                    if head_ix >= in_tree_by_ix.len() {
                        in_tree_by_ix.resize(head_ix + 1, false);
                    }
                    in_tree_by_ix[head_ix] = true;
                    tree_g_ixs.push(head_ix);
                    t.set_edge(v.clone(), w);
                }
            });

            g.for_each_in_edge_ix(v_ix, None, |tail_ix, head_ix, _ek, lbl| {
                debug_assert_eq!(head_ix, v_ix);
                if in_tree_by_ix.get(tail_ix).copied().unwrap_or(false) {
                    return;
                }

                let tail_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                let head_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);
                let minlen = lbl.minlen.max(1) as i128;
                let slack = head_rank - tail_rank - minlen;
                if slack == 0 {
                    let Some(w_id) = g.node_id_by_ix(tail_ix) else {
                        return;
                    };
                    let w = w_id.to_string();

                    stack_ix.push(tail_ix);
                    if tail_ix >= in_tree_by_ix.len() {
                        in_tree_by_ix.resize(tail_ix + 1, false);
                    }
                    in_tree_by_ix[tail_ix] = true;
                    tree_g_ixs.push(tail_ix);
                    t.set_edge(v.clone(), w);
                }
            });
        }
    } else {
        let roots: Vec<String> = t.node_ids();
        for root in roots {
            let mut stack: Vec<String> = vec![root];

            while let Some(v) = stack.pop() {
                g.for_each_out_edge(&v, None, |ek, lbl| {
                    let w = if v == ek.v {
                        ek.w.as_str()
                    } else {
                        ek.v.as_str()
                    };
                    if t.has_node(w) {
                        return;
                    }

                    let minlen = lbl.minlen.max(1) as i128;

                    let Some(tail_ix) = g.node_ix(&ek.v) else {
                        return;
                    };
                    let Some(head_ix) = g.node_ix(&ek.w) else {
                        return;
                    };
                    let tail_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                    let head_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);
                    let slack = head_rank - tail_rank - minlen;
                    if slack == 0 {
                        let w = w.to_string();
                        stack.push(w.clone());
                        if let Some(w_ix) = g.node_ix(&w) {
                            if w_ix >= in_tree_by_ix.len() {
                                in_tree_by_ix.resize(w_ix + 1, false);
                            }
                            in_tree_by_ix[w_ix] = true;
                            tree_g_ixs.push(w_ix);
                        }
                        t.set_edge(v.clone(), w);
                    }
                });
            }
        }
    }
    t.node_count()
}

fn find_min_slack_edge(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i128],
    in_tree_by_ix: &[bool],
) -> Option<(i128, bool)> {
    let mut best: Option<(i128, bool)> = None;
    g.for_each_edge_ix(|v_ix, w_ix, _key, lbl| {
        let in_v = in_tree_by_ix.get(v_ix).copied().unwrap_or(false);
        let in_w = in_tree_by_ix.get(w_ix).copied().unwrap_or(false);
        if in_v == in_w {
            return;
        }

        let v_rank = rank_by_ix.get(v_ix).copied().unwrap_or(0);
        let w_rank = rank_by_ix.get(w_ix).copied().unwrap_or(0);
        let minlen = lbl.minlen.max(1) as i128;
        let slack = w_rank - v_rank - minlen;

        match &best {
            Some((best_slack, _)) if slack >= *best_slack => {}
            _ => best = Some((slack, in_v)),
        }
    });
    best
}

fn shift_ranks(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut Vec<i128>,
    tree_g_ixs: &[usize],
    delta: i128,
) -> Result<(), WorkError> {
    let mut updates = Vec::with_capacity(tree_g_ixs.len());
    for &ix in tree_g_ixs {
        if ix >= rank_by_ix.len() {
            rank_by_ix.resize(ix + 1, 0);
        }
        let new_rank = rank_by_ix[ix] + delta;
        let new_rank_i32 = i32::try_from(new_rank).map_err(|_| WorkError::ArithmeticOverflow)?;
        updates.push((ix, new_rank, new_rank_i32));
    }
    for (ix, new_rank, new_rank_i32) in updates {
        rank_by_ix[ix] = new_rank;
        if let Some(label) = g.node_label_mut_by_ix(ix) {
            label.rank = Some(new_rank_i32);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        remaining: Option<usize>,
    }

    impl RecordingWorkControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                remaining: Some(limit),
                ..Self::default()
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            let Some(remaining) = self.remaining else {
                return Ok(());
            };
            let Some(next) = remaining.checked_sub(units) else {
                return Err(WorkError::Interrupted);
            };
            self.remaining = Some(next);
            Ok(())
        }
    }

    fn tight_chain(numeric: bool) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        let ids = if numeric {
            ["0", "1", "2", "3"]
        } else {
            ["node-0", "node-1", "node-2", "node-3"]
        };
        for (rank, id) in ids.into_iter().enumerate() {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(i32::try_from(rank).unwrap()),
                    ..NodeLabel::default()
                },
            );
        }
        for pair in ids.windows(2) {
            graph.set_edge_with_label(
                pair[0],
                pair[1],
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
        graph
    }

    #[test]
    fn feasible_tree_precharges_numeric_node_order_without_directed_adjacency() {
        let mut numeric = tight_chain(true);
        let mut ordinary = tight_chain(false);
        let numeric_order_work =
            checked_ordered_key_updates(numeric.node_count(), numeric.array_index_node_count())
                .unwrap();
        let numeric_setup = feasible_tree_setup_work_units(&numeric).unwrap();
        let ordinary_setup = feasible_tree_setup_work_units(&ordinary).unwrap();
        assert_eq!(numeric_setup, ordinary_setup + numeric_order_work);
        assert!(numeric.directed_array_index_adjacency_entry_count() > 0);

        let iteration_work = feasible_tree_iteration_work_units(&numeric).unwrap();
        let exact = checked_add(numeric_setup, iteration_work).unwrap();
        let source_nodes = numeric.node_ids();
        let source_edges = numeric.edge_keys();
        let source_ranks = source_nodes
            .iter()
            .map(|id| numeric.node(id).and_then(|node| node.rank))
            .collect::<Vec<_>>();

        let mut measured = RecordingWorkControl::default();
        let tree = feasible_tree_controlled(&mut numeric, &mut measured)
            .expect("the unbounded control admits the numeric feasible tree");
        assert_eq!(measured.charges, [numeric_setup, iteration_work]);
        assert_eq!(tree.node_count(), numeric.node_count());
        assert_eq!(
            tree.array_index_node_count(),
            numeric.array_index_node_count()
        );
        assert!(!tree.is_directed());
        assert_eq!(tree.directed_array_index_adjacency_entry_count(), 0);

        let mut ordinary_measured = RecordingWorkControl::default();
        let ordinary_tree = feasible_tree_controlled(&mut ordinary, &mut ordinary_measured)
            .expect("the unbounded control admits the ordinary feasible tree");
        assert_eq!(
            ordinary_measured.charges,
            [
                ordinary_setup,
                feasible_tree_iteration_work_units(&ordinary).unwrap()
            ]
        );
        assert_eq!(ordinary_tree.array_index_node_count(), 0);

        let mut rejected_graph = tight_chain(true);
        let mut rejected = RecordingWorkControl::with_limit(numeric_setup - 1);
        assert!(matches!(
            feasible_tree_controlled(&mut rejected_graph, &mut rejected),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(rejected.charges, [numeric_setup]);
        assert_eq!(rejected.remaining, Some(numeric_setup - 1));
        assert_eq!(rejected_graph.node_ids(), source_nodes);
        assert_eq!(rejected_graph.edge_keys(), source_edges);
        assert_eq!(
            source_nodes
                .iter()
                .map(|id| rejected_graph.node(id).and_then(|node| node.rank))
                .collect::<Vec<_>>(),
            source_ranks
        );

        let mut below_graph = tight_chain(true);
        let mut below = RecordingWorkControl::with_limit(exact - 1);
        assert!(matches!(
            feasible_tree_controlled(&mut below_graph, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [numeric_setup, iteration_work]);
        assert_eq!(below.remaining, Some(iteration_work - 1));

        for limit in [exact, exact + 1] {
            let mut admitted_graph = tight_chain(true);
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let tree = feasible_tree_controlled(&mut admitted_graph, &mut admitted)
                .expect("equal and above numeric feasible-tree budgets succeed");
            assert_eq!(tree.node_count(), admitted_graph.node_count());
        }
    }
}
