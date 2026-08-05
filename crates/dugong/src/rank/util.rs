//! Rank helpers (longest-path, slack).

use crate::graphlib::{EdgeKey, Graph};
use crate::work::{checked_add, checked_mul};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use rustc_hash::FxHashMap as HashMap;

pub fn longest_path(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = crate::NoopWorkControl;
    longest_path_controlled(g, &mut work_control)
        .expect("rank arithmetic must fit the public longest-path compatibility API");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LongestPathWorkPlan {
    work_units: usize,
    prepare_adjacency_cache: bool,
}

fn longest_path_work_plan(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<LongestPathWorkPlan, crate::WorkError> {
    let prepare_adjacency_cache = !g.is_adjacency_cache_current();
    // The admitted algorithm scans Graphlib node order for sources, validates every edge slot,
    // then visits each live node and outgoing edge. A stale CSR additionally initializes and
    // clones its node-slot offset/cursor arrays, scans edge slots twice, and materializes both
    // directed endpoint arrays. Keep that cold-only work distinct so warm callers are not charged
    // for historical node tombstones that they do not inspect.
    let base_work = checked_add(
        checked_add(g.node_order_slot_count(), g.node_count())?,
        checked_add(g.edge_slot_count(), checked_mul(g.edge_count(), 2)?)?,
    )?;
    let cold_csr_work = if prepare_adjacency_cache {
        checked_add(
            checked_mul(g.node_slot_count(), 6)?,
            checked_add(
                checked_mul(g.edge_slot_count(), 2)?,
                checked_mul(g.edge_count(), 2)?,
            )?,
        )?
    } else {
        0
    };
    Ok(LongestPathWorkPlan {
        work_units: checked_add(base_work, cold_csr_work)?,
        prepare_adjacency_cache,
    })
}

pub(crate) fn longest_path_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn crate::WorkControl,
) -> Result<(), crate::WorkError> {
    let plan = longest_path_work_plan(g)?;
    work_control.charge(plan.work_units)?;
    crate::rank::validate_rank_arithmetic(g)?;
    if plan.prepare_adjacency_cache {
        g.prepare_adjacency_cache();
    }
    struct Frame {
        v: String,
        edges: Vec<EdgeKey>,
        next_edge: usize,
        rank: Option<i128>,
        incoming_minlen: Option<i128>,
    }

    fn apply_candidate(rank: &mut Option<i128>, candidate: i128) {
        *rank = Some(match *rank {
            Some(current) => current.min(candidate),
            None => candidate,
        });
    }

    let sources: Vec<String> = g.sources().into_iter().map(|s| s.to_string()).collect();
    let mut visited: HashMap<String, i128> = HashMap::default();
    for v in sources {
        if visited.contains_key(&v) {
            continue;
        }

        let mut stack = vec![Frame {
            edges: g.out_edges(&v, None),
            v,
            next_edge: 0,
            rank: None,
            incoming_minlen: None,
        }];

        while let Some(frame) = stack.last_mut() {
            if let Some(rank) = visited.get(frame.v.as_str()).copied() {
                let incoming_minlen = frame.incoming_minlen;
                let _ = stack.pop();
                if let (Some(parent), Some(minlen)) = (stack.last_mut(), incoming_minlen) {
                    apply_candidate(&mut parent.rank, rank - minlen);
                }
                continue;
            }

            if frame.next_edge < frame.edges.len() {
                let edge = frame.edges[frame.next_edge].clone();
                frame.next_edge += 1;
                let minlen = g
                    .edge_by_key(&edge)
                    .map(|lbl| lbl.minlen as i128)
                    .unwrap_or(1);
                if let Some(child_rank) = visited.get(edge.w.as_str()).copied() {
                    apply_candidate(&mut frame.rank, child_rank - minlen);
                } else {
                    stack.push(Frame {
                        edges: g.out_edges(&edge.w, None),
                        v: edge.w,
                        next_edge: 0,
                        rank: None,
                        incoming_minlen: Some(minlen),
                    });
                }
                continue;
            }

            let Some(frame) = stack.pop() else {
                break;
            };
            let rank = frame.rank.unwrap_or(0);
            let rank_i32 = i32::try_from(rank).map_err(|_| crate::WorkError::ArithmeticOverflow)?;
            if let Some(label) = g.node_mut(&frame.v) {
                label.rank = Some(rank_i32);
            }
            visited.insert(frame.v, rank);
            if let (Some(parent), Some(minlen)) = (stack.last_mut(), frame.incoming_minlen) {
                apply_candidate(&mut parent.rank, rank - minlen);
            }
        }
    }
    Ok(())
}

pub fn slack(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, e: &EdgeKey) -> i32 {
    slack_checked(g, e).expect("rank slack must fit the public compatibility API")
}

pub(crate) fn slack_checked(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    e: &EdgeKey,
) -> Result<i32, crate::WorkError> {
    // Be defensive: callers can provide arbitrary graphs. Missing nodes/ranks are treated
    // as `0` so layout can degrade gracefully instead of panicking.
    let w_rank = g.node(&e.w).and_then(|n| n.rank).unwrap_or(0);
    let v_rank = g.node(&e.v).and_then(|n| n.rank).unwrap_or(0);
    let minlen = g.edge_by_key(e).map_or(1, |lbl| lbl.minlen) as i128;
    let slack = i128::from(w_rank) - i128::from(v_rank) - minlen;
    i32::try_from(slack).map_err(|_| crate::WorkError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;
    use crate::{WorkControl, WorkError};

    #[derive(Debug, Default)]
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

    fn graph_with_nodes(nodes: &[&str]) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        });
        for node in nodes {
            graph.set_node(*node, NodeLabel::default());
        }
        graph
    }

    fn chain_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = graph_with_nodes(&["a", "b", "c"]);
        graph.set_edge("a", "b");
        graph.set_edge("b", "c");
        graph
    }

    fn sparse_chain_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = graph_with_nodes(&["a", "b", "c"]);
        graph.set_edge_named("a", "c", Some("removed"), None);
        graph.set_edge("a", "b");
        graph.set_edge("b", "c");
        assert!(graph.remove_edge("a", "c", Some("removed")));
        graph
    }

    fn rank_snapshot(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Vec<(String, Option<i32>)> {
        graph
            .nodes()
            .map(|id| (id.to_string(), graph.node(id).and_then(|node| node.rank)))
            .collect()
    }

    #[test]
    fn longest_path_rejects_before_cold_csr_or_rank_mutation() {
        let mut rejected_graph = chain_graph();
        rejected_graph.for_each_node_mut(|_id, node| node.rank = Some(17));
        let source_ranks = rank_snapshot(&rejected_graph);
        let plan = longest_path_work_plan(&rejected_graph).unwrap();
        assert!(plan.prepare_adjacency_cache);
        assert!(!rejected_graph.is_adjacency_cache_current());

        let mut rejected = RecordingWorkControl::with_limit(plan.work_units - 1);
        assert_eq!(
            longest_path_controlled(&mut rejected_graph, &mut rejected),
            Err(WorkError::Interrupted)
        );
        assert_eq!(rejected.charges, [plan.work_units]);
        assert_eq!(rejected.remaining, Some(plan.work_units - 1));
        assert_eq!(rank_snapshot(&rejected_graph), source_ranks);
        assert!(!rejected_graph.is_adjacency_cache_current());

        for limit in [plan.work_units, plan.work_units + 1] {
            let mut graph = chain_graph();
            let mut admitted = RecordingWorkControl::with_limit(limit);
            longest_path_controlled(&mut graph, &mut admitted)
                .expect("equal and above longest-path budgets succeed");
            assert_eq!(admitted.charges, [plan.work_units]);
            assert_eq!(admitted.remaining, Some(limit - plan.work_units));
            assert!(graph.is_adjacency_cache_current());
            assert_eq!(
                rank_snapshot(&graph),
                [
                    ("a".to_string(), Some(-2)),
                    ("b".to_string(), Some(-1)),
                    ("c".to_string(), Some(0)),
                ]
            );
        }
    }

    #[test]
    fn longest_path_distinguishes_cold_and_warm_csr() {
        let mut cold_graph = sparse_chain_graph();
        let mut warm_graph = sparse_chain_graph();
        warm_graph.prepare_adjacency_cache();

        let cold_plan = longest_path_work_plan(&cold_graph).unwrap();
        let warm_plan = longest_path_work_plan(&warm_graph).unwrap();
        let expected_cold_delta = checked_add(
            checked_mul(cold_graph.node_slot_count(), 6).unwrap(),
            checked_add(
                checked_mul(cold_graph.edge_slot_count(), 2).unwrap(),
                checked_mul(cold_graph.edge_count(), 2).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            cold_plan.work_units,
            checked_add(warm_plan.work_units, expected_cold_delta).unwrap()
        );

        let mut cold_control = RecordingWorkControl::default();
        longest_path_controlled(&mut cold_graph, &mut cold_control)
            .expect("the cold longest-path run succeeds");
        let mut warm_control = RecordingWorkControl::default();
        longest_path_controlled(&mut warm_graph, &mut warm_control)
            .expect("the warm longest-path run succeeds");
        assert_eq!(cold_control.charges, [cold_plan.work_units]);
        assert_eq!(warm_control.charges, [warm_plan.work_units]);
        assert_eq!(rank_snapshot(&cold_graph), rank_snapshot(&warm_graph));
    }

    #[test]
    fn longest_path_accounts_for_each_slot_domain() {
        let dense_edges = chain_graph();
        let sparse_edges = sparse_chain_graph();
        assert_eq!(
            sparse_edges.edge_slot_count(),
            dense_edges.edge_slot_count() + 1
        );
        let dense_edges_cold = longest_path_work_plan(&dense_edges).unwrap().work_units;
        let sparse_edges_cold = longest_path_work_plan(&sparse_edges).unwrap().work_units;
        assert_eq!(sparse_edges_cold, dense_edges_cold + 3);
        dense_edges.prepare_adjacency_cache();
        sparse_edges.prepare_adjacency_cache();
        assert_eq!(
            longest_path_work_plan(&sparse_edges).unwrap().work_units,
            longest_path_work_plan(&dense_edges).unwrap().work_units + 1
        );

        let mut dense_node_slots = graph_with_nodes(&["0", "2", "3"]);
        dense_node_slots.set_edge("0", "2");
        dense_node_slots.set_edge("2", "3");
        let mut sparse_node_slots = graph_with_nodes(&["0", "1", "2", "3"]);
        assert!(sparse_node_slots.remove_node("1"));
        sparse_node_slots.set_edge("0", "2");
        sparse_node_slots.set_edge("2", "3");
        assert_eq!(
            sparse_node_slots.node_slot_count(),
            dense_node_slots.node_slot_count() + 1
        );
        assert_eq!(
            sparse_node_slots.node_order_slot_count(),
            dense_node_slots.node_order_slot_count()
        );
        assert_eq!(
            longest_path_work_plan(&sparse_node_slots)
                .unwrap()
                .work_units,
            longest_path_work_plan(&dense_node_slots)
                .unwrap()
                .work_units
                + 6
        );
        dense_node_slots.prepare_adjacency_cache();
        sparse_node_slots.prepare_adjacency_cache();
        assert_eq!(
            longest_path_work_plan(&sparse_node_slots)
                .unwrap()
                .work_units,
            longest_path_work_plan(&dense_node_slots)
                .unwrap()
                .work_units
        );

        let mut sparse_order = graph_with_nodes(&["a", "b", "c", "d"]);
        assert!(sparse_order.remove_node("b"));
        sparse_order.set_edge("a", "c");
        sparse_order.set_edge("c", "d");
        sparse_order.prepare_adjacency_cache();
        assert_eq!(
            sparse_order.node_slot_count(),
            sparse_node_slots.node_slot_count()
        );
        assert_eq!(
            sparse_order.node_order_slot_count(),
            sparse_node_slots.node_order_slot_count() + 1
        );
        assert_eq!(
            longest_path_work_plan(&sparse_order).unwrap().work_units,
            longest_path_work_plan(&sparse_node_slots)
                .unwrap()
                .work_units
                + 1
        );
    }
}
