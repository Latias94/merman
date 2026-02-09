use super::{
    OrderEdgeWeight, OrderNodeLabel, Relationship, add_subgraph_constraints, build_layer_graph,
    cross_count, init_order, sort_subgraph,
};
use crate::graphlib::{Graph, GraphOptions};
use std::collections::BTreeMap;

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
    let mut nodes_by_rank: BTreeMap<i32, Vec<String>> = BTreeMap::new();

    for v in g.nodes() {
        let Some(node) = g.node(v) else {
            continue;
        };
        if let Some(rank) = node.rank() {
            max_rank = max_rank.max(rank);
            nodes_by_rank.entry(rank).or_default().push(v.to_string());
        }
        if let (Some(min_rank), Some(max_rank_node)) = (node.min_rank(), node.max_rank()) {
            for r in min_rank..=max_rank_node {
                if node.rank() == Some(r) {
                    continue;
                }
                nodes_by_rank.entry(r).or_default().push(v.to_string());
            }
        }
    }

    if max_rank == i32::MIN {
        return;
    }

    let layering = init_order(g);
    assign_order(g, &layering);

    if opts.disable_optimal_order_heuristic {
        return;
    }

    let mut best_cc: f64 = f64::INFINITY;
    let mut best_layering: Option<Vec<Vec<String>>> = None;

    let mut i: usize = 0;
    let mut last_best: usize = 0;
    while last_best < 4 {
        let use_down = i % 2 == 1;
        let bias_right = i % 4 >= 2;

        if use_down {
            let ranks: Vec<i32> = (1..=max_rank).collect();
            sweep(g, &nodes_by_rank, &ranks, Relationship::InEdges, bias_right);
        } else {
            let ranks: Vec<i32> = if max_rank >= 1 {
                (0..=(max_rank - 1)).rev().collect()
            } else {
                Vec::new()
            };
            sweep(
                g,
                &nodes_by_rank,
                &ranks,
                Relationship::OutEdges,
                bias_right,
            );
        }

        let layering_now = build_layer_matrix(g, max_rank);
        let cc = cross_count(g, &layering_now);
        if cc < best_cc {
            last_best = 0;
            best_cc = cc;
            best_layering = Some(layering_now);
        }

        i += 1;
        last_best += 1;
    }

    if let Some(best) = best_layering {
        assign_order(g, &best);
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
    nodes_by_rank: &BTreeMap<i32, Vec<String>>,
    ranks: &[i32],
    relationship: Relationship,
    bias_right: bool,
) where
    N: Default + Clone + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut cg: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    for &rank in ranks {
        let nodes = nodes_by_rank
            .get(&rank)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let lg = build_layer_graph(g, rank, relationship, Some(nodes));
        let root = lg.graph().root.clone();

        let sorted = sort_subgraph(&lg, &root, &cg, bias_right);
        for (i, v) in sorted.vs.iter().enumerate() {
            if let Some(n) = g.node_mut(v) {
                n.set_order(i);
            }
        }

        add_subgraph_constraints(&lg, &mut cg, &sorted.vs);
    }
}

fn build_layer_matrix<N, E, G>(g: &Graph<N, E, G>, max_rank: i32) -> Vec<Vec<String>>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut layers: Vec<Vec<(usize, String)>> = vec![Vec::new(); (max_rank + 1) as usize];
    for v in g.nodes() {
        let Some(node) = g.node(v) else {
            continue;
        };
        let Some(rank) = node.rank() else {
            continue;
        };
        let Some(order) = node.order() else {
            continue;
        };
        layers[rank.max(0) as usize].push((order, v.to_string()));
    }
    let mut out: Vec<Vec<String>> = Vec::with_capacity(layers.len());
    for mut layer in layers {
        layer.sort_by_key(|(o, _)| *o);
        out.push(layer.into_iter().map(|(_, v)| v).collect());
    }
    out
}
