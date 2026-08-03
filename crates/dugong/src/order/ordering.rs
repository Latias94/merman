use super::cross_count::cross_count_ix_controlled;
use super::workspace::OrderWorkspace;
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship, init_order};
use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul};
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
    work_control.charge(order_setup_work_units(g)?)?;
    let mut max_rank: i32 = i32::MIN;
    let mut nodes_by_rank: Vec<Vec<usize>> = Vec::new();

    g.for_each_node_ix(|v_ix, _id, node| {
        let mut push_rank = |rank: i32| {
            if rank < 0 {
                return;
            }
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
    });
    if max_rank == i32::MIN {
        return Ok(());
    }

    let layering = init_order(g);
    assign_order(g, &layering);

    // With at most one node in every rank there is no alternate ordering and therefore no
    // crossing-minimization decision to make. Preserve the stable initial order and skip the five
    // sweep/cross-count rounds entirely.
    if opts.disable_optimal_order_heuristic || layering.iter().all(|layer| layer.len() <= 1) {
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
        let use_down = i % 2 == 1;
        let bias_right = i % 4 >= 2;
        let relationship = if use_down {
            Relationship::InEdges
        } else {
            Relationship::OutEdges
        };
        work_control.charge(workspace.iteration_work_units(relationship)?)?;

        if use_down {
            sweep(
                g,
                &ranks_down,
                bias_right,
                relationship,
                &mut workspace,
                work_control,
            )?;
        } else {
            sweep(
                g,
                &ranks_up,
                bias_right,
                relationship,
                &mut workspace,
                work_control,
            )?;
        }

        let layering_now = build_layer_matrix_ix(g, max_rank);
        let cross_count = cross_count_ix_controlled(g, &layering_now, work_control)?;
        if cross_count.value < best_cc {
            last_best = 0;
            best_cc = cross_count.value;
            best_layering = Some(layering_now);
        }

        // Crossing weights are products of edge weights. If every participating edge has a finite,
        // non-negative weight, zero is the global lower bound. Equal-score sweeps never replace the
        // first best layering, so continuing cannot affect the selected output.
        if cross_count.is_proven_minimum {
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

fn order_setup_work_units<N, E, G>(g: &Graph<N, E, G>) -> Result<usize, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut node_slots = 0usize;
    g.for_each_node_ix(|node_ix, _id, _node| {
        node_slots = node_slots.max(node_ix.saturating_add(1));
    });
    let node_work = checked_add(checked_mul(g.node_count(), 3)?, node_slots)?;
    let edge_work = checked_mul(g.edge_count(), 4)?;
    let mut work = checked_add(node_work, edge_work);
    let mut rank_slots = 0usize;
    g.for_each_node(|_id, node| {
        let Ok(current) = work else {
            return;
        };
        if let Some(rank) = node.rank().filter(|rank| *rank >= 0) {
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
        let span = match (node.min_rank(), node.max_rank()) {
            (Some(min_rank), Some(max_rank)) if max_rank >= min_rank => {
                usize::try_from(i64::from(max_rank) - i64::from(min_rank) + 1)
                    .map_err(|_| WorkError::ArithmeticOverflow)
            }
            _ => Ok(0),
        };
        work = span.and_then(|span| checked_add(current, span));
    });
    checked_add(work?, checked_mul(rank_slots, 4)?)
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
}
