use super::super::measured::{MeasuredGraph, MeasuredNode};
use super::{ClassLayoutGraph, class_cluster_candidates, project_graph};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel};
use indexmap::IndexMap;
use merman_render_core::graph_layout::Direction;

#[test]
fn forward_namespace_parents_do_not_reorder_declared_children() {
    let mut measured = MeasuredGraph {
        direction: Direction::Down,
        nodesep: 50.0,
        ranksep: 50.0,
        nodes: IndexMap::new(),
        parents: IndexMap::new(),
        edges: IndexMap::new(),
    };
    for (child, parent) in [("Child", "Parent"), ("Sibling", "Root"), ("Parent", "Root")] {
        measured.set_node(
            child.into(),
            MeasuredNode {
                is_namespace: true,
                ..Default::default()
            },
        );
        measured.set_parent(child.into(), parent.into());
    }
    measured.set_node(
        "Root".into(),
        MeasuredNode {
            is_namespace: true,
            ..Default::default()
        },
    );

    let graph = project_graph(measured);
    assert_eq!(graph.node_ids(), ["Child", "Parent", "Sibling", "Root"]);
    assert_eq!(graph.children("Root"), ["Sibling", "Parent"]);
    assert_eq!(graph.parent("Child"), Some("Parent"));
    assert_eq!(graph.parent("Parent"), Some("Root"));
}

fn class_candidate_test_graph(edges: &[(&str, &str)]) -> ClassLayoutGraph {
    let mut graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel::default());
    for id in [
        "root",
        "a",
        "b",
        "leaf",
        "sibling",
        "outside",
        "outside_child",
    ] {
        graph.set_node(id, NodeLabel::default());
    }
    for (child, parent) in [
        ("a", "root"),
        ("b", "a"),
        ("leaf", "b"),
        ("sibling", "root"),
        ("outside_child", "outside"),
    ] {
        graph.set_parent(child, parent);
    }
    for (index, (from, to)) in edges.iter().copied().enumerate() {
        graph.set_edge_named(
            from,
            to,
            Some(index.to_string()),
            Some(EdgeLabel::default()),
        );
    }
    graph
}

fn reference_class_cluster_candidates(graph: &ClassLayoutGraph) -> Vec<String> {
    let is_strict_descendant = |node: &str, ancestor: &str| {
        let mut parent = graph.parent(node);
        while let Some(id) = parent {
            if id == ancestor {
                return true;
            }
            parent = graph.parent(id);
        }
        false
    };
    graph
        .nodes()
        .filter(|id| graph.children_iter(id).next().is_some())
        .filter(|id| {
            graph.edges().all(|edge| {
                edge.v == *id
                    || edge.w == *id
                    || is_strict_descendant(&edge.v, id) == is_strict_descendant(&edge.w, id)
            })
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn class_cluster_boundary_index_matches_mermaid_descendant_semantics() {
    let ids = [
        "root",
        "a",
        "b",
        "leaf",
        "sibling",
        "outside",
        "outside_child",
    ];
    let mut edge_sets = vec![Vec::new()];
    for from in ids {
        for to in ids {
            edge_sets.push(vec![(from, to)]);
        }
    }
    edge_sets.push(vec![
        ("leaf", "sibling"),
        ("a", "leaf"),
        ("outside_child", "b"),
    ]);

    for edges in edge_sets {
        let graph = class_candidate_test_graph(&edges);
        assert_eq!(
            class_cluster_candidates(&graph),
            reference_class_cluster_candidates(&graph),
            "edges={edges:?}"
        );
    }
}

#[test]
fn class_cluster_boundary_index_handles_max_size_namespace_chain() {
    const NODE_COUNT: usize = 4_000;
    let mut graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel::default());
    for index in 0..NODE_COUNT {
        graph.set_node(format!("n{index}"), NodeLabel::default());
    }
    for index in (1..NODE_COUNT).rev() {
        graph.set_parent(format!("n{index}"), format!("n{}", index - 1));
    }

    let candidates = class_cluster_candidates(&graph);

    assert_eq!(candidates.len(), NODE_COUNT - 1);
    assert_eq!(candidates.first().map(String::as_str), Some("n0"));
    let expected_last = format!("n{}", NODE_COUNT - 2);
    assert_eq!(
        candidates.last().map(String::as_str),
        Some(expected_last.as_str())
    );
}
