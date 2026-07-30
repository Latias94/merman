use super::cross_count::cross_count_ix;
use super::workspace::OrderWorkspace;
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship, init_order};
use crate::graphlib::Graph;

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
        return;
    }

    let layering = init_order(g);
    assign_order(g, &layering);

    if opts.disable_optimal_order_heuristic {
        return;
    }

    let mut workspace = OrderWorkspace::new(g, &nodes_by_rank, max_rank);
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

        if use_down {
            sweep(
                g,
                &ranks_down,
                bias_right,
                Relationship::InEdges,
                &mut workspace,
            );
        } else {
            sweep(
                g,
                &ranks_up,
                bias_right,
                Relationship::OutEdges,
                &mut workspace,
            );
        }

        let layering_now = build_layer_matrix_ix(g, max_rank);
        let cc = cross_count_ix(g, &layering_now);
        if cc < best_cc {
            last_best = 0;
            best_cc = cc;
            best_layering = Some(layering_now);
        }

        i += 1;
        last_best += 1;
    }

    if let Some(best) = best_layering {
        assign_order_ix(g, &best);
    }
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
) where
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
        let sorted = workspace.sort_rank(g, rank, relationship, bias_right);

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
