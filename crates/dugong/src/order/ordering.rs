use super::cross_count::{cross_count_ix_controlled, is_zero_crossing_ix_controlled};
use super::workspace::OrderWorkspace;
use super::{EMPTY_LAYER_SLOT, OrderEdgeWeight, OrderNodeLabel, Relationship, init_order};
use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul, checked_n_log_n};
use crate::{NoopWorkControl, WorkControl, WorkError};

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderOptions {
    pub disable_optimal_order_heuristic: bool,
}

pub fn order<N, E, G>(g: &mut Graph<N, E, G>, opts: OrderOptions)
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut work_control = NoopWorkControl;
    order_controlled(g, opts, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject ordering work");
}

pub(crate) fn order_controlled<N, E, G>(
    g: &mut Graph<N, E, G>,
    opts: OrderOptions,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    // The setup plan inspects node slots, rank spans, and leaf status before any of the matching
    // vectors or sort inputs are materialized. Admit that bounded planning scan first.
    work_control.charge(checked_mul(g.node_order_slot_count(), 2)?)?;
    let setup_plan = order_setup_plan(g)?;
    work_control.charge(setup_plan.work_units)?;
    if setup_plan.prepare_adjacency_cache {
        // Adjacency queries rebuild through interior mutability. Force that rebuild only after its
        // complete slot-backed work has been admitted, so later cardinality queries stay O(1).
        g.prepare_adjacency_cache();
    }
    work_control.charge(checked_mul(setup_plan.node_slots, 3)?)?;
    let mut max_rank: i32 = i32::MIN;
    let mut nodes_by_rank: Vec<Vec<usize>> = Vec::new();
    let mut sweep_nodes = vec![false; setup_plan.node_slots];

    g.for_each_node_ix(|v_ix, _id, node| {
        let mut participates_in_sweep = false;
        let mut push_rank = |rank: i32| {
            if rank < 0 {
                return;
            }
            participates_in_sweep = true;
            let idx = rank as usize;
            if nodes_by_rank.len() <= idx {
                nodes_by_rank.resize_with(idx + 1, Vec::new);
            }
            nodes_by_rank[idx].push(v_ix);
            max_rank = max_rank.max(rank);
        };

        if let Some(rank) = node.rank() {
            push_rank(rank);
        }
        if let (Some(min_rank), Some(max_rank_node)) = (node.min_rank(), node.max_rank()) {
            for r in min_rank..=max_rank_node {
                if node.rank() == Some(r) {
                    continue;
                }
                push_rank(r);
            }
        }
        if participates_in_sweep {
            sweep_nodes[v_ix] = true;
        }
    });
    if max_rank == i32::MIN {
        return Ok(());
    }
    let layering = init_order(g);
    assign_order(g, &layering);
    let mut initial_layering_nodes = vec![false; setup_plan.node_slots];
    for id in layering.iter().flatten() {
        if let Some(node_ix) = g.node_ix(id) {
            initial_layering_nodes[node_ix] = true;
        }
    }
    let initial_layering_covers_sweep_nodes = sweep_nodes
        .iter()
        .zip(&initial_layering_nodes)
        .all(|(&swept, &layered)| !swept || layered);

    // With at most one node in every rank there is no alternate ordering and therefore no
    // crossing-minimization decision to make. Preserve the stable initial order and skip the five
    // sweep/cross-count rounds entirely.
    if opts.disable_optimal_order_heuristic {
        return Ok(());
    }
    if initial_layering_covers_sweep_nodes && layering.iter().all(|layer| layer.len() <= 1) {
        return Ok(());
    }

    let mut workspace = OrderWorkspace::new(g, &nodes_by_rank, max_rank)?;
    let mut best_cc: f64 = f64::INFINITY;
    let mut best_layering: Option<Vec<Vec<usize>>> = None;

    let ranks_down: Vec<i32> = (1..=max_rank).collect();
    let ranks_up: Vec<i32> = if max_rank >= 1 {
        (0..=(max_rank - 1)).rev().collect()
    } else {
        Vec::new()
    };

    let mut i: usize = 0;
    let mut last_best: usize = 0;
    let mut have_scored_layering = false;
    let mut best_is_proven_minimum = false;
    while last_best < 4 {
        let primary_order_changed =
            run_sweep_round(g, &ranks_down, &ranks_up, i, &mut workspace, work_control)?;

        // Crossing count is a pure function of the primary layer matrix and edge weights. Keep
        // running every Dagre sweep so bias, constraints, and range-only node side effects remain
        // intact, but do not rebuild and recount a matrix that is byte-for-byte unchanged.
        if have_scored_layering && !primary_order_changed {
            i += 1;
            last_best += 1;
            continue;
        }

        let layering_now = build_layer_matrix_ix_controlled(g, max_rank, work_control)?;
        have_scored_layering = true;
        if best_is_proven_minimum && layering_now.has_unique_rank_orders {
            let is_zero = is_zero_crossing_ix_controlled(g, &layering_now.layers, work_control)?;
            if is_zero {
                // Pinned Dagre keeps the most recent layering when crossing scores tie. The full
                // weighted count is unnecessary once zero is proved to be the exact global lower
                // bound, but the latest zero-crossing layering remains observable.
                best_layering = Some(layering_now.layers);
            }
        } else {
            let cross_count = cross_count_ix_controlled(g, &layering_now.layers, work_control)?;
            let proves_minimum =
                layering_now.has_unique_rank_orders && cross_count.is_proven_minimum;
            if cross_count.value < best_cc {
                last_best = 0;
                best_cc = cross_count.value;
                best_is_proven_minimum = proves_minimum;
                best_layering = Some(layering_now.layers);
            } else if cross_count.value == best_cc {
                // Pinned Dagre keeps the most recent layering when crossing scores tie. Bias
                // alternates across sweeps, so replacing the previous best here is observable in
                // final node order.
                best_is_proven_minimum |= proves_minimum;
                best_layering = Some(layering_now.layers);
            }
        }

        i += 1;
        last_best += 1;
    }

    if let Some(best) = best_layering {
        assign_order_ix(g, &best);
    }
    Ok(())
}

fn run_sweep_round<N, E, G>(
    g: &mut Graph<N, E, G>,
    ranks_down: &[i32],
    ranks_up: &[i32],
    iteration: usize,
    workspace: &mut OrderWorkspace,
    work_control: &mut dyn WorkControl,
) -> Result<bool, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let use_down = iteration % 2 == 1;
    let bias_right = iteration % 4 >= 2;
    let relationship = if use_down {
        Relationship::InEdges
    } else {
        Relationship::OutEdges
    };
    work_control.charge(
        workspace
            .iteration_work_units(if use_down { ranks_down } else { ranks_up }, relationship)?,
    )?;

    if use_down {
        sweep(
            g,
            ranks_down,
            bias_right,
            relationship,
            workspace,
            work_control,
        )
    } else {
        sweep(
            g,
            ranks_up,
            bias_right,
            relationship,
            workspace,
            work_control,
        )
    }
}

struct OrderSetupPlan {
    work_units: usize,
    node_slots: usize,
    prepare_adjacency_cache: bool,
}

fn order_setup_plan<N, E, G>(g: &Graph<N, E, G>) -> Result<OrderSetupPlan, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut node_slots = 0usize;
    let mut simple_nodes = Ok(0usize);
    g.for_each_node_ix(|node_ix, id, _node| {
        node_slots = node_slots.max(node_ix.saturating_add(1));
        if g.children_iter(id).next().is_none() {
            simple_nodes = simple_nodes.and_then(|count| checked_add(count, 1));
        }
    });
    let node_work = checked_add(checked_mul(g.node_count(), 3)?, node_slots)?;
    let prepare_adjacency_cache = !g.is_adjacency_cache_current();
    let edge_tombstones = g
        .edge_slot_count()
        .checked_sub(g.edge_count())
        .ok_or(WorkError::ArithmeticOverflow)?;
    let tombstone_rebuild_work = if prepare_adjacency_cache {
        // A stale CSR cache scans every edge slot twice. The existing four live-edge units cover
        // live adjacency materialization plus the owner-local edge passes; add only the two scans
        // that are invisible when an edge slot is a tombstone.
        checked_mul(edge_tombstones, 2)?
    } else {
        0
    };
    let edge_work = checked_add(checked_mul(g.edge_count(), 4)?, tombstone_rebuild_work)?;
    let mut work = checked_add(node_work, edge_work);
    let mut rank_slots = 0usize;
    g.for_each_node(|_id, node| {
        let Ok(current) = work else {
            return;
        };
        for rank in [node.rank(), node.max_rank()].into_iter().flatten() {
            let contributes_slots = rank >= 0
                && (node.rank() == Some(rank)
                    || node.min_rank().is_some_and(|min_rank| min_rank <= rank));
            if contributes_slots {
                let slots =
                    usize::try_from(i64::from(rank) + 1).map_err(|_| WorkError::ArithmeticOverflow);
                match slots {
                    Ok(slots) => rank_slots = rank_slots.max(slots),
                    Err(error) => {
                        work = Err(error);
                        return;
                    }
                }
            }
        }
        let span = match (node.min_rank(), node.max_rank()) {
            (Some(min_rank), Some(max_rank)) if max_rank >= min_rank => {
                usize::try_from(i64::from(max_rank) - i64::from(min_rank) + 1)
                    .map_err(|_| WorkError::ArithmeticOverflow)
            }
            _ => Ok(0),
        };
        work = span.and_then(|span| checked_add(current, span));
    });
    let work = checked_add(work?, checked_mul(rank_slots, 4)?)?;
    let work_units = checked_add(work, checked_n_log_n(simple_nodes?)?)?;
    Ok(OrderSetupPlan {
        work_units,
        node_slots,
        prepare_adjacency_cache,
    })
}

fn assign_order<N, E, G>(g: &mut Graph<N, E, G>, layering: &[Vec<String>])
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    for layer in layering {
        for (i, v) in layer.iter().enumerate() {
            if let Some(node) = g.node_mut(v) {
                node.set_order(i);
            }
        }
    }
}

fn sweep<N, E, G>(
    g: &mut Graph<N, E, G>,
    ranks: &[i32],
    bias_right: bool,
    relationship: Relationship,
    workspace: &mut OrderWorkspace,
    work_control: &mut dyn WorkControl,
) -> Result<bool, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    workspace.begin_sweep();
    let mut primary_order_changed = false;

    for &rank in ranks {
        if rank < 0 {
            continue;
        }
        let rank = rank as usize;
        let sorted = workspace.sort_rank(g, rank, relationship, bias_right, work_control)?;

        for (i, &local_ix) in sorted.vs.iter().enumerate() {
            let Some(original_ix) = workspace.original_ix(rank, local_ix) else {
                continue;
            };
            if let Some(n) = g.node_label_mut_by_ix(original_ix) {
                if n.rank().is_some() && n.order() != Some(i) {
                    primary_order_changed = true;
                }
                n.set_order(i);
            }
        }
        workspace.add_constraints(rank, &sorted.vs);
    }
    Ok(primary_order_changed)
}

#[derive(Debug, PartialEq, Eq)]
struct BuiltLayerMatrix {
    layers: Vec<Vec<usize>>,
    has_unique_rank_orders: bool,
}

fn build_layer_matrix_ix_controlled<N, E, G>(
    g: &Graph<N, E, G>,
    max_rank: i32,
    work_control: &mut dyn WorkControl,
) -> Result<BuiltLayerMatrix, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let rank_slots = usize::try_from((i64::from(max_rank) + 1).max(0))
        .map_err(|_| WorkError::ArithmeticOverflow)?;
    let planning_work = checked_add(
        checked_mul(rank_slots, 2)?,
        checked_add(g.node_count(), g.node_order_slot_count())?,
    )?;
    work_control.charge(planning_work)?;

    let mut layer_slots = vec![0usize; rank_slots];
    let mut entries = Vec::with_capacity(g.node_count());
    let mut error = None;
    let mut has_unique_rank_orders = true;
    g.for_each_node_ix(|v_ix, _id, node| {
        if error.is_some() {
            return;
        }
        let Some(rank) = node.rank() else {
            return;
        };
        if rank < 0 {
            return;
        }
        let Some(order) = node.order() else {
            // A later sweep may assign this ranked node an order and change which edges
            // participate in the layer matrix. Do not reuse a proof from an incomplete matrix.
            has_unique_rank_orders = false;
            return;
        };
        let idx = rank as usize;
        let Some(slots) = layer_slots.get_mut(idx) else {
            has_unique_rank_orders = false;
            return;
        };
        let Some(required_slots) = order.checked_add(1) else {
            error = Some(WorkError::ArithmeticOverflow);
            return;
        };
        *slots = (*slots).max(required_slots);
        entries.push((idx, order, v_ix));
    });
    if let Some(error) = error {
        return Err(error);
    }

    let total_slots = layer_slots
        .iter()
        .try_fold(0usize, |total, &slots| checked_add(total, slots))?;
    work_control.charge(checked_add(
        rank_slots,
        checked_add(total_slots, entries.len())?,
    )?)?;

    let mut layers = Vec::with_capacity(rank_slots);
    for slots in layer_slots {
        layers.push(vec![EMPTY_LAYER_SLOT; slots]);
    }
    for (rank, order, v_ix) in entries {
        if let Some(slot) = layers.get_mut(rank).and_then(|layer| layer.get_mut(order)) {
            // Dagre assigns `layering[rank][order] = node`, so a later graph node replaces an
            // earlier duplicate order instead of introducing a second compacted entry.
            has_unique_rank_orders &= *slot == EMPTY_LAYER_SLOT;
            *slot = v_ix;
        }
    }
    Ok(BuiltLayerMatrix {
        layers,
        has_unique_rank_orders,
    })
}

fn assign_order_ix<N, E, G>(g: &mut Graph<N, E, G>, layering: &[Vec<usize>])
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    for layer in layering {
        for (i, &v_ix) in layer.iter().enumerate() {
            if v_ix == EMPTY_LAYER_SLOT {
                continue;
            }
            if let Some(node) = g.node_label_mut_by_ix(v_ix) {
                node.set_order(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;
    use crate::{EdgeLabel, GraphLabel, NodeLabel};

    #[derive(Default)]
    struct RecordingWorkControl {
        used: usize,
        max: usize,
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

    fn sparse_ranked_chain(node_count: usize) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for index in 0..node_count {
            graph.set_node(
                format!("n{index}"),
                NodeLabel {
                    rank: Some(i32::try_from(index).unwrap()),
                    ..Default::default()
                },
            );
            if index > 0 {
                graph.set_edge(format!("n{}", index - 1), format!("n{index}"));
            }
        }
        graph
    }

    #[test]
    fn sparse_ordering_work_tracks_n_log_n_instead_of_edge_pairs() {
        let mut graph = sparse_ranked_chain(256);
        let mut work_control = RecordingWorkControl {
            max: 125_000,
            ..Default::default()
        };

        order_controlled(&mut graph, OrderOptions::default(), &mut work_control)
            .expect("a representative sparse graph fits the constrained profile budget");

        assert!(work_control.used > graph.node_count() + graph.edge_count());
        assert!(work_control.used < 125_000);
    }

    #[test]
    fn setup_precharges_edge_tombstones_before_adjacency_rebuild() {
        fn stale_graph(directed: bool) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
            let mut graph = Graph::new(GraphOptions {
                directed,
                multigraph: true,
                ..GraphOptions::default()
            });
            graph.set_graph(GraphLabel::default());
            for (id, rank) in [("a", 0), ("b", 1), ("c", 1)] {
                graph.set_node(
                    id,
                    NodeLabel {
                        rank: Some(rank),
                        ..Default::default()
                    },
                );
            }
            graph.set_edge_named("a", "b", Some("first"), Some(EdgeLabel::default()));
            graph.set_edge_named("a", "c", Some("removed"), Some(EdgeLabel::default()));
            graph.set_edge_named("b", "c", Some("last"), Some(EdgeLabel::default()));
            graph.prepare_adjacency_cache();
            assert!(graph.remove_edge("a", "c", Some("removed")));
            assert_eq!(graph.edge_count(), 2);
            assert_eq!(graph.edge_slot_count(), 3);
            assert!(!graph.is_adjacency_cache_current());
            graph
        }

        for directed in [true, false] {
            let mut graph = stale_graph(directed);
            let planning_work = checked_mul(graph.node_order_slot_count(), 2).unwrap();
            let setup_plan = order_setup_plan(&graph).unwrap();
            assert!(setup_plan.prepare_adjacency_cache);
            let admitted_boundary = checked_add(planning_work, setup_plan.work_units).unwrap();

            let mut below = RecordingWorkControl {
                max: admitted_boundary - 1,
                ..Default::default()
            };
            assert_eq!(
                order_controlled(&mut graph, OrderOptions::default(), &mut below),
                Err(WorkError::Interrupted)
            );
            assert_eq!(below.used, planning_work);
            assert!(!graph.is_adjacency_cache_current());

            let mut exact = RecordingWorkControl {
                max: admitted_boundary,
                ..Default::default()
            };
            assert_eq!(
                order_controlled(&mut graph, OrderOptions::default(), &mut exact),
                Err(WorkError::Interrupted)
            );
            assert_eq!(exact.used, admitted_boundary);
            assert!(graph.is_adjacency_cache_current());
        }
    }

    #[test]
    fn wide_ordering_does_not_charge_a_layer_matrix_sort() {
        const NODE_COUNT: usize = 256;
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for index in (0..NODE_COUNT).rev() {
            graph.set_node(
                format!("n{index}"),
                NodeLabel {
                    rank: Some(0),
                    order: Some(index),
                    ..Default::default()
                },
            );
        }
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, 0, &mut work_control).unwrap();

        let planning_work = 2 + 2 * NODE_COUNT;
        let materialization_work = 1 + NODE_COUNT * 2;
        assert_eq!(work_control.used, planning_work + materialization_work);
        assert!(layering.has_unique_rank_orders);
        assert_eq!(layering.layers[0].len(), NODE_COUNT);
        assert!(work_control.used < checked_n_log_n(NODE_COUNT).unwrap());
    }

    #[test]
    fn sparse_high_rank_layer_matrix_charges_every_rank_wide_pass() {
        const MAX_RANK: i32 = 1_024;
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for (id, rank) in [("low", 0), ("high", MAX_RANK)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    order: Some(0),
                    ..Default::default()
                },
            );
        }
        let rank_slots = usize::try_from(MAX_RANK + 1).unwrap();
        let planning_work = 2 * rank_slots + 2 * graph.node_count();
        let materialization_work = rank_slots + 2 + graph.node_count();
        let exact_work = planning_work + materialization_work;
        let mut exact = RecordingWorkControl {
            max: exact_work,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, MAX_RANK, &mut exact).unwrap();

        assert_eq!(exact.used, exact_work);
        assert!(layering.has_unique_rank_orders);
        assert_eq!(layering.layers.len(), rank_slots);
        assert_eq!(layering.layers[0], vec![graph.node_ix("low").unwrap()]);
        assert_eq!(
            layering.layers[rank_slots - 1],
            vec![graph.node_ix("high").unwrap()]
        );

        let mut below = RecordingWorkControl {
            max: exact_work - 1,
            ..Default::default()
        };
        assert_eq!(
            build_layer_matrix_ix_controlled(&graph, MAX_RANK, &mut below),
            Err(WorkError::Interrupted)
        );
        assert_eq!(below.used, planning_work);
    }

    #[test]
    fn layer_matrix_charges_node_order_tombstones_before_scanning() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for (id, order) in [("a", 0), ("removed", 1), ("b", 2)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(0),
                    order: Some(order),
                    ..Default::default()
                },
            );
        }
        assert!(graph.remove_node("removed"));
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.node_order_slot_count(), 3);

        let planning_work = 2 + graph.node_count() + graph.node_order_slot_count();
        let materialization_work = 1 + 3 + graph.node_count();
        let exact_work = planning_work + materialization_work;
        let mut exact = RecordingWorkControl {
            max: exact_work,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, 0, &mut exact).unwrap();

        assert_eq!(exact.used, exact_work);
        assert_eq!(
            layering.layers,
            vec![vec![
                graph.node_ix("a").unwrap(),
                EMPTY_LAYER_SLOT,
                graph.node_ix("b").unwrap(),
            ]]
        );

        let mut below = RecordingWorkControl {
            max: planning_work - 1,
            ..Default::default()
        };
        assert_eq!(
            build_layer_matrix_ix_controlled(&graph, 0, &mut below),
            Err(WorkError::Interrupted)
        );
        assert_eq!(below.used, 0);
    }

    #[test]
    fn single_node_layers_do_not_skip_range_only_ordering() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "negative",
            NodeLabel {
                rank: Some(-1),
                ..Default::default()
            },
        );
        graph.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                ..Default::default()
            },
        );
        graph.set_node(
            "x",
            NodeLabel {
                min_rank: Some(0),
                max_rank: Some(1),
                ..Default::default()
            },
        );
        graph.set_node(
            "b",
            NodeLabel {
                rank: Some(1),
                ..Default::default()
            },
        );
        graph.set_edge("a", "b");
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        order_controlled(&mut graph, OrderOptions::default(), &mut work_control).unwrap();

        assert_eq!(graph.node("x").and_then(|node| node.order), Some(0));
    }

    #[test]
    fn equal_crossing_sweeps_keep_mermaid_dagres_last_layering() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                ..Default::default()
            },
        );
        for id in ["b", "c"] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            graph.set_edge_with_label(
                "a",
                id,
                EdgeLabel {
                    // Explicit zero weights exercise the structural proof's ignored-edge path;
                    // the sweep must still preserve Dagre's observable tie-last replacement.
                    weight: 0.0,
                    ..Default::default()
                },
            );
        }
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        order_controlled(&mut graph, OrderOptions::default(), &mut work_control).unwrap();

        // Pinned Dagre replaces the best layering on an equal crossing score. The fourth sweep
        // uses right bias, so the equal-barycenter siblings finish in reverse insertion order.
        assert_eq!(graph.node("b").and_then(|node| node.order), Some(1));
        assert_eq!(graph.node("c").and_then(|node| node.order), Some(0));
    }

    #[test]
    fn zero_crossing_sweep_does_not_short_circuit_dagres_tie_last_search() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for (id, rank) in [
            ("anchor", 0),
            ("b0", 1),
            ("b1", 1),
            ("b2", 1),
            ("c0", 2),
            ("c1", 2),
        ] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    ..Default::default()
                },
            );
        }
        for (from, to) in [("b0", "c0"), ("b1", "c1"), ("b2", "c0")] {
            graph.set_edge_with_label(
                from,
                to,
                EdgeLabel {
                    weight: 1.0,
                    ..Default::default()
                },
            );
        }
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        order_controlled(&mut graph, OrderOptions::default(), &mut work_control).unwrap();

        // Pinned Dagre scores these sweeps 0, 1, 0, 1. A zero global minimum therefore permits a
        // cheaper zero/non-zero proof, but it must still preserve the later equal minimum from the
        // right-biased third sweep rather than the first zero or the final working order.
        for (id, expected) in [
            ("anchor", 0),
            ("b2", 0),
            ("b0", 1),
            ("b1", 2),
            ("c0", 0),
            ("c1", 1),
        ] {
            assert_eq!(
                graph.node(id).and_then(|node| node.order),
                Some(expected),
                "unexpected final order for {id}"
            );
        }
    }

    #[test]
    fn layer_matrix_preserves_mermaid_dagre_order_holes() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                order: Some(0),
                ..Default::default()
            },
        );
        graph.set_node(
            "b",
            NodeLabel {
                rank: Some(0),
                order: Some(2),
                ..Default::default()
            },
        );
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, 0, &mut work_control).unwrap();

        assert!(layering.has_unique_rank_orders);
        assert_eq!(
            layering.layers,
            vec![vec![
                graph.node_ix("a").unwrap(),
                EMPTY_LAYER_SLOT,
                graph.node_ix("b").unwrap(),
            ]]
        );
    }

    #[test]
    fn layer_matrix_matches_mermaid_dagres_last_write_for_duplicate_orders() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for id in ["a", "b"] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    ..Default::default()
                },
            );
        }
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, 0, &mut work_control).unwrap();

        assert!(!layering.has_unique_rank_orders);
        assert_eq!(layering.layers, vec![vec![graph.node_ix("b").unwrap()]]);
    }

    #[test]
    fn layer_matrix_disables_cached_proofs_when_a_ranked_node_has_no_order() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "ordered",
            NodeLabel {
                rank: Some(0),
                order: Some(0),
                ..Default::default()
            },
        );
        graph.set_node(
            "missing",
            NodeLabel {
                rank: Some(0),
                order: None,
                ..Default::default()
            },
        );
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        let layering = build_layer_matrix_ix_controlled(&graph, 0, &mut work_control).unwrap();

        assert!(!layering.has_unique_rank_orders);
        assert_eq!(
            layering.layers,
            vec![vec![graph.node_ix("ordered").unwrap()]]
        );
    }

    #[test]
    fn extreme_rank_span_is_rejected_before_rank_slot_allocation() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "n",
            NodeLabel {
                rank: Some(i32::MAX),
                ..Default::default()
            },
        );
        let mut work_control = RecordingWorkControl {
            max: 125_000,
            ..Default::default()
        };

        assert_eq!(
            order_controlled(&mut graph, OrderOptions::default(), &mut work_control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(graph.node("n").and_then(|node| node.order), None);
    }

    #[test]
    fn range_only_extreme_rank_is_rejected_before_rank_slot_allocation() {
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "cluster",
            NodeLabel {
                min_rank: Some(i32::MAX),
                max_rank: Some(i32::MAX),
                ..Default::default()
            },
        );
        let mut work_control = RecordingWorkControl {
            max: 125_000,
            ..Default::default()
        };

        assert_eq!(
            order_controlled(&mut graph, OrderOptions::default(), &mut work_control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(graph.node("cluster").and_then(|node| node.order), None);
    }
}
