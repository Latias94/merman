use super::constraints::add_subgraph_constraints_ix;
use super::cross_count::cross_count_ix;
use super::layer_graph::{build_layer_graph_with_root_lite_ix, create_root_node};
use super::types::OrderNodeLite;
use super::{
    LayerGraphLabel, OrderEdgeWeight, OrderNodeLabel, Relationship, WeightLabel, init_order,
};
use crate::graphlib::{Graph, GraphOptions};

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderOptions {
    pub disable_optimal_order_heuristic: bool,
}

pub fn order<N, E, G>(g: &mut Graph<N, E, G>, opts: OrderOptions)
where
    N: Default + Clone + OrderNodeLabel + 'static,
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

    let root = create_root_node(g);

    let mut layer_graphs_in: Vec<Graph<OrderNodeLite, WeightLabel, LayerGraphLabel>> =
        Vec::with_capacity((max_rank + 1).max(0) as usize);
    let mut layer_graphs_out: Vec<Graph<OrderNodeLite, WeightLabel, LayerGraphLabel>> =
        Vec::with_capacity((max_rank + 1).max(0) as usize);
    for rank in 0..=max_rank {
        let nodes = nodes_by_rank
            .get(rank as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        layer_graphs_in.push(build_layer_graph_with_root_lite_ix(
            g,
            rank,
            Relationship::InEdges,
            &root,
            nodes,
        ));
        layer_graphs_out.push(build_layer_graph_with_root_lite_ix(
            g,
            rank,
            Relationship::OutEdges,
            &root,
            nodes,
        ));
    }
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
            sweep(g, &ranks_down, bias_right, &root, &mut layer_graphs_in);
        } else {
            sweep(g, &ranks_up, bias_right, &root, &mut layer_graphs_out);
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
    root: &str,
    layer_graphs: &mut [Graph<OrderNodeLite, WeightLabel, LayerGraphLabel>],
) where
    N: Default + Clone + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut cg: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    for &rank in ranks {
        let Some(lg) = layer_graphs.get_mut(rank as usize) else {
            continue;
        };

        sync_layer_graph_orders(g, lg, root);

        let sorted = super::barycenter::sort_subgraph_ix(lg, root, &cg, bias_right);

        for (i, &v_ix) in sorted.vs.iter().enumerate() {
            let Some(id) = lg.node_id_by_ix(v_ix) else {
                continue;
            };
            let Some(original_ix) = g.node_ix(id) else {
                continue;
            };
            if let Some(n) = g.node_label_mut_by_ix(original_ix) {
                n.set_order(i);
            }
        }
        add_subgraph_constraints_ix(lg, &mut cg, &sorted.vs);
    }
}

fn sync_layer_graph_orders<N, E, G>(
    original: &Graph<N, E, G>,
    layer_graph: &mut Graph<OrderNodeLite, WeightLabel, LayerGraphLabel>,
    root: &str,
) where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    layer_graph.for_each_node_mut(|id, node| {
        if id == root {
            return;
        }
        if node.order().is_none() {
            return;
        }
        let order = original.node(id).and_then(|n| n.order()).unwrap_or(0);
        node.set_order(order);
    });
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
