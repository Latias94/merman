use super::layer_graph::{build_layer_graph_with_root, create_root_node};
use super::{
    LayerGraphLabel, OrderEdgeWeight, OrderNodeLabel, Relationship, WeightLabel,
    add_subgraph_constraints, cross_count, init_order, sort_subgraph,
};
use crate::graphlib::{Graph, GraphOptions};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderOptions {
    pub disable_optimal_order_heuristic: bool,
}

#[derive(Debug, Default, Clone)]
struct OrderTimings {
    total: std::time::Duration,
    build_nodes_by_rank: std::time::Duration,
    init_order: std::time::Duration,
    assign_initial: std::time::Duration,
    build_layer_graph_cache: std::time::Duration,
    sweeps: std::time::Duration,
    sweep_sync_orders: std::time::Duration,
    sweep_build_layer_graph: std::time::Duration,
    sweep_sort_subgraph: std::time::Duration,
    sweep_apply_order: std::time::Duration,
    sweep_add_constraints: std::time::Duration,
    build_layer_matrix: std::time::Duration,
    cross_count: std::time::Duration,
}

pub fn order<N, E, G>(g: &mut Graph<N, E, G>, opts: OrderOptions)
where
    N: Default + Clone + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let timing_enabled = std::env::var("DUGONG_ORDER_TIMING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let total_start = timing_enabled.then(std::time::Instant::now);
    let mut timings = OrderTimings::default();

    let mut max_rank: i32 = i32::MIN;
    let mut nodes_by_rank: BTreeMap<i32, Vec<String>> = BTreeMap::new();

    let build_nodes_by_rank_start = timing_enabled.then(std::time::Instant::now);
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
    if let Some(s) = build_nodes_by_rank_start {
        timings.build_nodes_by_rank = s.elapsed();
    }

    if max_rank == i32::MIN {
        return;
    }

    let init_order_start = timing_enabled.then(std::time::Instant::now);
    let layering = init_order(g);
    if let Some(s) = init_order_start {
        timings.init_order = s.elapsed();
    }

    let assign_initial_start = timing_enabled.then(std::time::Instant::now);
    assign_order(g, &layering);
    if let Some(s) = assign_initial_start {
        timings.assign_initial = s.elapsed();
    }

    if opts.disable_optimal_order_heuristic {
        return;
    }

    let root = create_root_node(g);

    let build_cache_start = timing_enabled.then(std::time::Instant::now);
    let mut layer_graphs_in: BTreeMap<i32, Graph<N, WeightLabel, LayerGraphLabel>> =
        BTreeMap::new();
    let mut layer_graphs_out: BTreeMap<i32, Graph<N, WeightLabel, LayerGraphLabel>> =
        BTreeMap::new();
    for rank in 0..=max_rank {
        let nodes = nodes_by_rank
            .get(&rank)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        layer_graphs_in.insert(
            rank,
            build_layer_graph_with_root(g, rank, Relationship::InEdges, &root, Some(nodes)),
        );
        layer_graphs_out.insert(
            rank,
            build_layer_graph_with_root(g, rank, Relationship::OutEdges, &root, Some(nodes)),
        );
    }
    if let Some(s) = build_cache_start {
        timings.build_layer_graph_cache = s.elapsed();
    }

    let mut best_cc: f64 = f64::INFINITY;
    let mut best_layering: Option<Vec<Vec<String>>> = None;

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
            let sweep_start = timing_enabled.then(std::time::Instant::now);
            sweep(
                g,
                &nodes_by_rank,
                &ranks_down,
                Relationship::InEdges,
                bias_right,
                &root,
                &mut layer_graphs_in,
                timing_enabled,
                &mut timings,
            );
            if let Some(s) = sweep_start {
                timings.sweeps += s.elapsed();
            }
        } else {
            let sweep_start = timing_enabled.then(std::time::Instant::now);
            sweep(
                g,
                &nodes_by_rank,
                &ranks_up,
                Relationship::OutEdges,
                bias_right,
                &root,
                &mut layer_graphs_out,
                timing_enabled,
                &mut timings,
            );
            if let Some(s) = sweep_start {
                timings.sweeps += s.elapsed();
            }
        }

        let build_layer_matrix_start = timing_enabled.then(std::time::Instant::now);
        let layering_now = build_layer_matrix(g, max_rank);
        if let Some(s) = build_layer_matrix_start {
            timings.build_layer_matrix += s.elapsed();
        }

        let cross_count_start = timing_enabled.then(std::time::Instant::now);
        let cc = cross_count(g, &layering_now);
        if let Some(s) = cross_count_start {
            timings.cross_count += s.elapsed();
        }
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

    if let Some(s) = total_start {
        timings.total = s.elapsed();
        eprintln!(
            "[dugong-timing] stage=order total={:?} build_nodes_by_rank={:?} init_order={:?} assign_initial={:?} build_layer_graph_cache={:?} sweeps={:?} sweep_sync_orders={:?} sweep_build_layer_graph={:?} sweep_sort_subgraph={:?} sweep_apply_order={:?} sweep_add_constraints={:?} build_layer_matrix={:?} cross_count={:?}",
            timings.total,
            timings.build_nodes_by_rank,
            timings.init_order,
            timings.assign_initial,
            timings.build_layer_graph_cache,
            timings.sweeps,
            timings.sweep_sync_orders,
            timings.sweep_build_layer_graph,
            timings.sweep_sort_subgraph,
            timings.sweep_apply_order,
            timings.sweep_add_constraints,
            timings.build_layer_matrix,
            timings.cross_count,
        );
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
    root: &str,
    layer_graphs: &mut BTreeMap<i32, Graph<N, WeightLabel, LayerGraphLabel>>,
    timing_enabled: bool,
    timings: &mut OrderTimings,
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

        let build_lg_start = timing_enabled.then(std::time::Instant::now);
        let lg = match layer_graphs.get_mut(&rank) {
            Some(v) => v,
            None => {
                layer_graphs.insert(
                    rank,
                    build_layer_graph_with_root(g, rank, relationship, root, Some(nodes)),
                );
                layer_graphs.get_mut(&rank).expect("just inserted")
            }
        };
        if let Some(s) = build_lg_start {
            timings.sweep_build_layer_graph += s.elapsed();
        }

        let sync_start = timing_enabled.then(std::time::Instant::now);
        sync_layer_graph_orders(g, lg, root);
        if let Some(s) = sync_start {
            timings.sweep_sync_orders += s.elapsed();
        }

        let sort_start = timing_enabled.then(std::time::Instant::now);
        let sorted = sort_subgraph(lg, root, &cg, bias_right);
        if let Some(s) = sort_start {
            timings.sweep_sort_subgraph += s.elapsed();
        }

        let apply_order_start = timing_enabled.then(std::time::Instant::now);
        for (i, v) in sorted.vs.iter().enumerate() {
            if let Some(n) = g.node_mut(v) {
                n.set_order(i);
            }
        }
        if let Some(s) = apply_order_start {
            timings.sweep_apply_order += s.elapsed();
        }

        let constraints_start = timing_enabled.then(std::time::Instant::now);
        add_subgraph_constraints(&lg, &mut cg, &sorted.vs);
        if let Some(s) = constraints_start {
            timings.sweep_add_constraints += s.elapsed();
        }
    }
}

fn sync_layer_graph_orders<N, E, G>(
    original: &Graph<N, E, G>,
    layer_graph: &mut Graph<N, WeightLabel, LayerGraphLabel>,
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
