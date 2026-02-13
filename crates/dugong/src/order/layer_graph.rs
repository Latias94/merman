//! Layer graph construction for ordering sweeps.
//!
//! This mirrors upstream Dagre's `buildLayerGraph` helper, materializing a rank-local graph used
//! for barycenter-based sorting.

use super::{LayerGraphLabel, OrderEdgeWeight, OrderNodeRange, Relationship, WeightLabel};
use crate::graphlib::{Graph, GraphOptions};

pub fn build_layer_graph<N, E, G>(
    g: &Graph<N, E, G>,
    rank: i32,
    relationship: Relationship,
    nodes_with_rank: Option<&[String]>,
) -> Graph<N, WeightLabel, LayerGraphLabel>
where
    N: Default + Clone + 'static + OrderNodeRange,
    E: Default + 'static + OrderEdgeWeight,
    G: Default,
{
    let root = create_root_node(g);
    build_layer_graph_with_root(g, rank, relationship, &root, nodes_with_rank)
}

pub(super) fn build_layer_graph_with_root<N, E, G>(
    g: &Graph<N, E, G>,
    rank: i32,
    relationship: Relationship,
    root: &str,
    nodes_with_rank: Option<&[String]>,
) -> Graph<N, WeightLabel, LayerGraphLabel>
where
    N: Default + Clone + 'static + OrderNodeRange,
    E: Default + 'static + OrderEdgeWeight,
    G: Default,
{
    let mut result: Graph<N, WeightLabel, LayerGraphLabel> = Graph::new(GraphOptions {
        compound: true,
        multigraph: false,
        ..Default::default()
    });
    result.set_graph(LayerGraphLabel {
        root: root.to_string(),
    });
    result.set_node(root.to_string(), N::default());

    let mut visit_node = |v: &str| {
        let node = g.node(v).cloned().unwrap_or_default();
        let parent = g.parent(v);

        let in_range = node.rank() == Some(rank)
            || (node.min_rank().is_some()
                && node.max_rank().is_some()
                && node.min_rank().is_some_and(|min| min <= rank)
                && node.max_rank().is_some_and(|max| rank <= max));

        if !in_range {
            return;
        }

        result.set_node(v.to_string(), node.clone());
        result.set_parent(
            v.to_string(),
            parent
                .map(|p| p.to_string())
                .unwrap_or_else(|| root.to_string()),
        );

        match relationship {
            Relationship::InEdges => {
                g.for_each_in_edge(v, None, |ek, lbl| {
                    let u = ek.v.as_str();

                    if !result.has_node(u) {
                        let lbl = g.node(u).cloned().unwrap_or_default();
                        result.set_node(u.to_string(), lbl);
                    }

                    let existing_weight = result.edge(u, v, None).map(|l| l.weight).unwrap_or(0.0);
                    let weight = lbl.weight();
                    result.set_edge_with_label(
                        u.to_string(),
                        v.to_string(),
                        WeightLabel {
                            weight: weight + existing_weight,
                        },
                    );
                });
            }
            Relationship::OutEdges => {
                // Reverse out-edges so `barycenter(...)` can always read `in_edges(...)`.
                g.for_each_out_edge(v, None, |ek, lbl| {
                    let u = ek.w.as_str();

                    if !result.has_node(u) {
                        let lbl = g.node(u).cloned().unwrap_or_default();
                        result.set_node(u.to_string(), lbl);
                    }

                    let existing_weight = result.edge(u, v, None).map(|l| l.weight).unwrap_or(0.0);
                    let weight = lbl.weight();
                    result.set_edge_with_label(
                        u.to_string(),
                        v.to_string(),
                        WeightLabel {
                            weight: weight + existing_weight,
                        },
                    );
                });
            }
        }

        if node.has_min_rank() {
            let override_label = node.subgraph_layer_label(rank);
            result.set_node(v.to_string(), override_label);
        }
    };

    match nodes_with_rank {
        Some(vs) => {
            for v in vs {
                visit_node(v.as_str());
            }
        }
        None => {
            for v in g.nodes() {
                visit_node(v);
            }
        }
    }

    result
}

pub(super) fn create_root_node<N, E, G>(g: &Graph<N, E, G>) -> String
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    loop {
        let v = crate::util::unique_id("_root");
        if !g.has_node(&v) {
            return v;
        }
    }
}
