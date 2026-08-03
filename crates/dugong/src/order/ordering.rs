use super::cross_count::cross_count_ix_controlled;
use super::workspace::OrderWorkspace;
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship, init_order};
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
    order_controlled_impl(g, opts, work_control, true)
}

fn order_controlled_impl<N, E, G>(
    g: &mut Graph<N, E, G>,
    opts: OrderOptions,
    work_control: &mut dyn WorkControl,
    allow_proven_shortcuts: bool,
) -> Result<(), WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    // The setup plan inspects node slots, rank spans, and leaf status before any of the matching
    // vectors or sort inputs are materialized. Admit that bounded planning scan first.
    work_control.charge(checked_mul(g.node_count(), 2)?)?;
    let setup_plan = order_setup_plan(g)?;
    work_control.charge(setup_plan.work_units)?;
    work_control.charge(checked_mul(setup_plan.node_slots, 3)?)?;
    let mut max_rank: i32 = i32::MIN;
    let mut nodes_by_rank: Vec<Vec<usize>> = Vec::new();
    let mut primary_rank_sizes: Vec<usize> = Vec::new();
    let mut sweep_node_count = 0usize;
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
            if rank >= 0 {
                let rank = rank as usize;
                if primary_rank_sizes.len() <= rank {
                    primary_rank_sizes.resize(rank + 1, 0);
                }
                primary_rank_sizes[rank] += 1;
            }
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
            sweep_node_count += 1;
            sweep_nodes[v_ix] = true;
        }
    });
    if max_rank == i32::MIN {
        return Ok(());
    }
    let rank_slots =
        usize::try_from(i64::from(max_rank) + 1).map_err(|_| WorkError::ArithmeticOverflow)?;
    if primary_rank_sizes.len() < rank_slots {
        primary_rank_sizes.resize(rank_slots, 0);
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
    if allow_proven_shortcuts
        && initial_layering_covers_sweep_nodes
        && layering.iter().all(|layer| layer.len() <= 1)
    {
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
    while last_best < 4 {
        run_sweep_round(g, &ranks_down, &ranks_up, i, &mut workspace, work_control)?;

        // Layer sizes are rank-stable across sweeps. Charge the bounded plan scan, then the rank
        // buckets, local sorts, and result materialization before they are allocated.
        work_control.charge(primary_rank_sizes.len())?;
        work_control.charge(layer_matrix_work_units(
            &primary_rank_sizes,
            g.node_count(),
        )?)?;
        let layering_now = build_layer_matrix_ix(g, max_rank);
        let layering_now_node_count = layering_now
            .iter()
            .try_fold(0usize, |total, layer| checked_add(total, layer.len()))?;
        let cross_count = cross_count_ix_controlled(g, &layering_now, work_control)?;
        if cross_count.value < best_cc {
            last_best = 0;
            best_cc = cross_count.value;
            best_layering = Some(layering_now);
        }

        // Crossing weights are products of edge weights. If every participating edge has a finite,
        // non-negative weight, zero is the global lower bound. Equal-score sweeps never replace the
        // first best layering, so continuing cannot affect the selected output.
        if allow_proven_shortcuts && cross_count.is_proven_minimum {
            if layering_now_node_count == sweep_node_count {
                break;
            }

            // The best score is already the global lower bound, so later layer matrices and cross
            // counts cannot replace it. Preserve the observable order side effects for range-only
            // or otherwise unrepresented sweep nodes by running the remaining sweeps, while
            // deleting the now-provably-dead materialization and Fenwick work.
            i += 1;
            last_best += 1;
            while last_best < 4 {
                run_sweep_round(g, &ranks_down, &ranks_up, i, &mut workspace, work_control)?;
                i += 1;
                last_best += 1;
            }
            break;
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
) -> Result<(), WorkError>
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
    work_control.charge(workspace.iteration_work_units(relationship)?)?;

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
    let edge_work = checked_mul(g.edge_count(), 4)?;
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
) -> Result<(), WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    workspace.begin_sweep();

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
                n.set_order(i);
            }
        }
        workspace.add_constraints(rank, &sorted.vs);
    }
    Ok(())
}

fn build_layer_matrix_ix<N, E, G>(g: &Graph<N, E, G>, max_rank: i32) -> Vec<Vec<usize>>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut layers: Vec<Vec<(usize, usize)>> = vec![Vec::new(); (max_rank + 1).max(0) as usize];
    g.for_each_node_ix(|v_ix, _id, node| {
        let Some(rank) = node.rank() else {
            return;
        };
        if rank < 0 {
            return;
        }
        let Some(order) = node.order() else {
            return;
        };
        let idx = rank as usize;
        if let Some(layer) = layers.get_mut(idx) {
            layer.push((order, v_ix));
        }
    });
    let mut out: Vec<Vec<usize>> = Vec::with_capacity(layers.len());
    for mut layer in layers {
        layer.sort_by_key(|(o, _)| *o);
        out.push(layer.into_iter().map(|(_, v)| v).collect());
    }
    out
}

fn layer_matrix_work_units(
    layer_sizes: &[usize],
    graph_node_count: usize,
) -> Result<usize, WorkError> {
    let (ranked_nodes, sort_work) = layer_sizes.iter().copied().try_fold(
        (0usize, 0usize),
        |(ranked_nodes, sort_work), size| {
            Ok((
                checked_add(ranked_nodes, size)?,
                checked_add(sort_work, checked_n_log_n(size)?)?,
            ))
        },
    )?;
    let bucket_work = checked_mul(layer_sizes.len(), 2)?;
    let node_work = checked_add(graph_node_count, checked_mul(ranked_nodes, 2)?)?;
    checked_add(checked_add(bucket_work, node_work)?, sort_work)
}

fn assign_order_ix<N, E, G>(g: &mut Graph<N, E, G>, layering: &[Vec<usize>])
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    for layer in layering {
        for (i, &v_ix) in layer.iter().enumerate() {
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
    fn wide_ordering_charges_both_global_and_layer_local_sorts() {
        const NODE_COUNT: usize = 256;
        let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
            Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for index in (0..NODE_COUNT).rev() {
            graph.set_node(
                format!("n{index}"),
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
        }
        let mut work_control = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };

        order_controlled(&mut graph, OrderOptions::default(), &mut work_control).unwrap();

        let one_sort = checked_n_log_n(NODE_COUNT).unwrap();
        assert!(work_control.used >= checked_mul(one_sort, 2).unwrap());
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
    fn zero_crossing_does_not_skip_range_only_ordering() {
        fn graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
            let mut graph = Graph::new(GraphOptions::default());
            graph.set_graph(GraphLabel::default());
            for (id, rank) in [
                ("a", Some(0)),
                ("c", Some(0)),
                ("x", None),
                ("b", Some(1)),
                ("d", Some(1)),
            ] {
                graph.set_node(
                    id,
                    NodeLabel {
                        rank,
                        min_rank: (id == "x").then_some(0),
                        max_rank: (id == "x").then_some(1),
                        ..Default::default()
                    },
                );
            }
            graph.set_edge("a", "b");
            graph.set_edge("c", "d");
            graph
        }

        let mut optimized = graph();
        let mut optimized_work = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };
        order_controlled_impl(
            &mut optimized,
            OrderOptions::default(),
            &mut optimized_work,
            true,
        )
        .unwrap();

        let mut reference = graph();
        let mut reference_work = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };
        order_controlled_impl(
            &mut reference,
            OrderOptions::default(),
            &mut reference_work,
            false,
        )
        .unwrap();

        for id in optimized.node_ids() {
            assert_eq!(
                optimized.node(&id).and_then(|node| node.order),
                reference.node(&id).and_then(|node| node.order),
                "order differs for {id}"
            );
        }
        assert_eq!(optimized.node("x").and_then(|node| node.order), Some(0));
        assert!(optimized_work.used < reference_work.used);
    }

    #[test]
    fn zero_crossing_does_not_skip_ranked_compound_ordering() {
        fn graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
            let mut graph = Graph::new(GraphOptions {
                compound: true,
                ..GraphOptions::default()
            });
            graph.set_graph(GraphLabel::default());
            for (id, rank) in [("a", 0), ("c", 0), ("b", 1), ("d", 1), ("child", 1)] {
                graph.set_node(
                    id,
                    NodeLabel {
                        rank: Some(rank),
                        ..Default::default()
                    },
                );
            }
            graph.set_node(
                "cluster",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            graph.set_parent("child", "cluster");
            graph.set_edge("a", "b");
            graph.set_edge("c", "d");
            graph
        }

        let mut optimized = graph();
        let mut optimized_work = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };
        order_controlled_impl(
            &mut optimized,
            OrderOptions::default(),
            &mut optimized_work,
            true,
        )
        .unwrap();

        let mut reference = graph();
        let mut reference_work = RecordingWorkControl {
            max: usize::MAX,
            ..Default::default()
        };
        order_controlled_impl(
            &mut reference,
            OrderOptions::default(),
            &mut reference_work,
            false,
        )
        .unwrap();

        for id in optimized.node_ids() {
            assert_eq!(
                optimized.node(&id).and_then(|node| node.order),
                reference.node(&id).and_then(|node| node.order),
                "order differs for {id}"
            );
        }
        assert!(optimized_work.used <= reference_work.used);
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
