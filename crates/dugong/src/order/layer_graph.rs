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
    let mut result: Graph<N, WeightLabel, LayerGraphLabel> = Graph::new(GraphOptions {
        compound: true,
        multigraph: false,
        ..Default::default()
    });
    result.set_graph(LayerGraphLabel { root: root.clone() });
    result.set_node(root.clone(), N::default());

    let nodes: Vec<String> = match nodes_with_rank {
        Some(vs) => vs.to_vec(),
        None => g.nodes().map(|v| v.to_string()).collect(),
    };

    for v in nodes {
        let node = g.node(&v).cloned().unwrap_or_default();
        let parent = g.parent(&v).map(|p| p.to_string());

        let in_range = node.rank() == Some(rank)
            || (node.min_rank().is_some()
                && node.max_rank().is_some()
                && node.min_rank().is_some_and(|min| min <= rank)
                && node.max_rank().is_some_and(|max| rank <= max));

        if !in_range {
            continue;
        }

        result.set_node(v.clone(), node.clone());
        result.set_parent(v.clone(), parent.unwrap_or_else(|| root.clone()));

        let incident_edges = match relationship {
            Relationship::InEdges => g.in_edges(&v, None),
            Relationship::OutEdges => g.out_edges(&v, None),
        };

        for e in incident_edges {
            let u = if e.v == v { e.w.clone() } else { e.v.clone() };

            if !result.has_node(&u) {
                let lbl = g.node(&u).cloned().unwrap_or_default();
                result.set_node(u.clone(), lbl);
            }

            let existing_weight = result.edge(&u, &v, None).map(|l| l.weight).unwrap_or(0.0);
            let weight = g.edge_by_key(&e).map(|l| l.weight()).unwrap_or(0.0);
            result.set_edge_with_label(
                u,
                v.clone(),
                WeightLabel {
                    weight: weight + existing_weight,
                },
            );
        }

        if node.has_min_rank() {
            let override_label = node.subgraph_layer_label(rank);
            result.set_node(v, override_label);
        }
    }

    result
}

fn create_root_node<N, E, G>(g: &Graph<N, E, G>) -> String
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
