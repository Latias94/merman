//! Break cycles by reversing a feedback arc set (FAS).
//!
//! This mirrors Dagre's `acyclic.js`. Mermaid uses the default (DFS-based) variant by default,
//! but can opt into the greedy strategy.

use crate::graphlib::{EdgeKey, Graph};
use crate::work::{ceil_log2, checked_add, checked_mul, checked_n_log_n};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

#[derive(Debug, Default)]
struct ReverseNameGen {
    next_by_endpoints: HashMap<(String, String), usize>,
}

impl ReverseNameGen {
    fn next(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        v: &str,
        w: &str,
    ) -> Result<String, WorkError> {
        let endpoints = (v.to_string(), w.to_string());
        let mut next = self.next_by_endpoints.get(&endpoints).copied().unwrap_or(1);
        loop {
            let candidate = format!("rev{next}");
            if !g.has_edge(v, w, Some(&candidate)) {
                self.next_by_endpoints
                    .insert(endpoints, checked_add(next, 1)?);
                return Ok(candidate);
            }
            next = checked_add(next, 1)?;
        }
    }
}

pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    run_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject acyclic work");
}

pub(crate) fn run_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    let fas = if g
        .graph()
        .acyclicer
        .as_deref()
        .is_some_and(|s| s == "greedy")
    {
        work_control.charge(greedy_work_units(g)?)?;
        crate::greedy_fas::greedy_fas_with_weight(g, |lbl: &EdgeLabel| {
            if !lbl.weight.is_finite() {
                return 0;
            }
            lbl.weight.round() as i64
        })
    } else {
        let plan = dfs_work_plan(g)?;
        work_control.charge(plan.work_units)?;
        if plan.prepare_adjacency_cache {
            // `out_edges` rebuilds the CSR cache through interior mutability. Materialize it only
            // after the complete slot-backed rebuild has been admitted.
            g.prepare_adjacency_cache();
        }
        dfs_fas(g)
    };

    let mut reverse_names = ReverseNameGen::default();
    for e in fas.into_iter().filter(|e| e.v != e.w) {
        let Some(label) = g.edge_by_key(&e).cloned() else {
            continue;
        };
        let _ = g.remove_edge_key(&e);

        let mut label = label;
        label.forward_name = e.name.clone();
        label.reversed = true;

        let name = reverse_names.next(g, &e.w, &e.v)?;
        g.set_edge_named(e.w, e.v, Some(name), Some(label));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct DfsWorkPlan {
    work_units: usize,
    prepare_adjacency_cache: bool,
}

fn dfs_work_plan(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Result<DfsWorkPlan, WorkError> {
    let prepare_adjacency_cache = !g.is_adjacency_cache_current();
    let edge_tombstones = g
        .edge_slot_count()
        .checked_sub(g.edge_count())
        .ok_or(WorkError::ArithmeticOverflow)?;
    let tombstone_rebuild_work = if prepare_adjacency_cache {
        // The existing two live-edge units cover CSR materialization plus the DFS edge visit.
        // A stale cache additionally scans every tombstone in both CSR construction passes.
        checked_mul(edge_tombstones, 2)?
    } else {
        0
    };
    let work_units = checked_add(
        checked_add(g.node_order_slot_count(), checked_mul(g.edge_count(), 2)?)?,
        tombstone_rebuild_work,
    )?;
    Ok(DfsWorkPlan {
        work_units,
        prepare_adjacency_cache,
    })
}

fn greedy_work_units(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Result<usize, WorkError> {
    let nodes = g.node_count();
    let edges = g.edge_count();
    let node_work = checked_mul(checked_n_log_n(nodes)?, 3)?;
    let edge_steps = checked_add(ceil_log2(nodes).max(1), 6)?;
    let live_work = checked_add(node_work, checked_mul(edges, edge_steps)?)?;
    let node_order_tombstones = g
        .node_order_slot_count()
        .checked_sub(nodes)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let edge_tombstones = g
        .edge_slot_count()
        .checked_sub(edges)
        .ok_or(WorkError::ArithmeticOverflow)?;
    checked_add(
        live_work,
        checked_add(node_order_tombstones, edge_tombstones)?,
    )
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

fn dfs_fas(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<EdgeKey> {
    // Ported from `dagre-d3-es 7.0.14`, the Dagre companion pinned by Mermaid 11.16.0.
    let mut fas: Vec<EdgeKey> = Vec::new();
    let mut stack: HashSet<String> = HashSet::default();
    let mut visited: HashSet<String> = HashSet::default();

    struct DfsFrame {
        v: String,
        edges: Vec<EdgeKey>,
        next_edge: usize,
    }

    fn push_frame(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        v: String,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        frames: &mut Vec<DfsFrame>,
    ) {
        visited.insert(v.clone());
        stack.insert(v.clone());
        frames.push(DfsFrame {
            edges: g.out_edges(&v, None),
            v,
            next_edge: 0,
        });
    }

    fn dfs_iterative(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root: &str,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        fas: &mut Vec<EdgeKey>,
    ) {
        if visited.contains(root) {
            return;
        }

        let mut frames: Vec<DfsFrame> = Vec::new();
        push_frame(g, root.to_string(), visited, stack, &mut frames);

        while !frames.is_empty() {
            let next = {
                let frame = match frames.last_mut() {
                    Some(frame) => frame,
                    None => break,
                };
                if frame.next_edge < frame.edges.len() {
                    let edge = frame.edges[frame.next_edge].clone();
                    frame.next_edge += 1;
                    Some(edge)
                } else {
                    None
                }
            };

            let Some(e) = next else {
                let Some(frame) = frames.pop() else {
                    break;
                };
                stack.remove(&frame.v);
                continue;
            };

            if e.v == e.w {
                continue;
            }
            if stack.contains(&e.w) {
                fas.push(e);
            } else if !visited.contains(&e.w) {
                push_frame(g, e.w.clone(), visited, stack, &mut frames);
            }
        }
    }

    // Dagre's `dfsFAS` iterates nodes in pinned Graphlib `g.nodes()` order. Graphlib delegates to
    // JavaScript object-key enumeration, so array-index IDs precede ordinary creation-order IDs.
    for v in g.nodes() {
        dfs_iterative(g, v, &mut visited, &mut stack, &mut fas);
    }
    fas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;

    struct RecordingWorkControl {
        used: usize,
        max: usize,
    }

    impl RecordingWorkControl {
        fn unlimited() -> Self {
            Self {
                used: 0,
                max: usize::MAX,
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            let next = self
                .used
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            if next > self.max {
                return Err(WorkError::Interrupted);
            }
            self.used = next;
            Ok(())
        }
    }

    fn cycle_graph(
        acyclicer: &str,
        edge_tombstones: usize,
    ) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel {
            acyclicer: Some(acyclicer.to_string()),
            ..Default::default()
        });
        for id in ["a", "b", "c"] {
            graph.set_node(id, NodeLabel::default());
        }
        for (from, to) in [("a", "b"), ("b", "c")] {
            graph.set_edge_with_label(
                from,
                to,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                },
            );
        }
        let mut dead_edges = Vec::with_capacity(edge_tombstones);
        for index in 0..edge_tombstones {
            let name = format!("dead-{index}");
            let key = EdgeKey::new("a", "a", Some(name.clone()));
            graph.set_edge_named("a", "a", Some(name), Some(EdgeLabel::default()));
            dead_edges.push(key);
        }
        graph.set_edge_with_label(
            "c",
            "a",
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            },
        );
        for key in dead_edges {
            assert!(graph.remove_edge_key(&key));
        }
        assert_eq!(
            graph.edge_slot_count(),
            graph.edge_count() + edge_tombstones
        );
        graph
    }

    fn run_and_measure(
        mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> (usize, Vec<EdgeKey>) {
        let mut control = RecordingWorkControl::unlimited();
        run_controlled(&mut graph, &mut control).unwrap();
        (control.used, graph.edge_keys())
    }

    #[test]
    fn dfs_precharges_both_stale_csr_tombstone_scans() {
        let tombstones = 7;
        let dense = cycle_graph("dfs", 0);
        let sparse = cycle_graph("dfs", tombstones);
        assert!(!dense.is_adjacency_cache_current());
        assert!(!sparse.is_adjacency_cache_current());

        let (dense_work, dense_edges) = run_and_measure(dense);
        let (sparse_work, sparse_edges) = run_and_measure(sparse);

        assert_eq!(sparse_work, dense_work + 2 * tombstones);
        assert_eq!(sparse_edges, dense_edges);
    }

    #[test]
    fn dfs_does_not_recharge_an_already_materialized_sparse_cache() {
        let dense = cycle_graph("dfs", 0);
        let sparse = cycle_graph("dfs", 7);
        dense.prepare_adjacency_cache();
        sparse.prepare_adjacency_cache();

        let (dense_work, dense_edges) = run_and_measure(dense);
        let (sparse_work, sparse_edges) = run_and_measure(sparse);

        assert_eq!(sparse_work, dense_work);
        assert_eq!(sparse_edges, dense_edges);
    }

    #[test]
    fn dfs_rejection_precedes_cache_materialization_and_graph_mutation() {
        let mut graph = cycle_graph("dfs", 3);
        let initial_edges = graph.edge_keys();
        let plan = dfs_work_plan(&graph).unwrap();
        let mut control = RecordingWorkControl {
            used: 0,
            max: plan.work_units - 1,
        };

        assert_eq!(
            run_controlled(&mut graph, &mut control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(graph.edge_keys(), initial_edges);
        assert!(!graph.is_adjacency_cache_current());
        assert_eq!(control.used, 0);
    }

    #[test]
    fn greedy_precharges_the_complete_edge_slot_scan() {
        let tombstones = 7;
        let (dense_work, dense_edges) = run_and_measure(cycle_graph("greedy", 0));
        let (sparse_work, sparse_edges) = run_and_measure(cycle_graph("greedy", tombstones));

        assert_eq!(sparse_work, dense_work + tombstones);
        assert_eq!(sparse_edges, dense_edges);
    }
}
