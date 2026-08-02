//! Self-edge extraction and reinsertion.
//!
//! Upstream Dagre temporarily removes self-loop edges before ranking/normalization and later
//! re-inserts them via dummy `"selfedge"` nodes during BK positioning. This keeps the ranker
//! constraints valid and makes self-loops deterministic.

use rustc_hash::FxHashSet;

use crate::graphlib::Graph;
use crate::{EdgeLabel, GraphLabel, NodeLabel, Point, SelfEdge};

pub fn remove_self_edges(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let self_loop_keys: Vec<_> = g.edges().filter(|ek| ek.v == ek.w).cloned().collect();
    for ek in self_loop_keys {
        if ek.v != ek.w {
            continue;
        }
        let Some(label) = g.edge_by_key(&ek).cloned() else {
            continue;
        };
        if let Some(n) = g.node_mut(&ek.v) {
            n.self_edges.push(SelfEdge {
                edge_obj: ek.clone(),
                label,
            });
        }
        let _ = g.remove_edge_key(&ek);
    }
}

pub fn insert_self_edges(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let layering = crate::util::build_layer_matrix(g);
    for layer in layering {
        let mut extra: usize = 0;
        for (idx, node_id) in layer.iter().enumerate() {
            let Some(rank) = g.node(node_id).and_then(|n| n.rank) else {
                continue;
            };

            if let Some(n) = g.node_mut(node_id) {
                n.order = Some(idx + extra);
            }

            let self_edges = g
                .node(node_id)
                .map(|n| n.self_edges.clone())
                .unwrap_or_default();
            if self_edges.is_empty() {
                continue;
            }
            if let Some(n) = g.node_mut(node_id) {
                n.self_edges.clear();
            }

            for se in self_edges {
                extra += 1;
                let selfedge_id = crate::util::unique_id("_se");
                g.set_node(
                    selfedge_id.clone(),
                    NodeLabel {
                        width: se.label.width,
                        height: se.label.height,
                        rank: Some(rank),
                        order: Some(idx + extra),
                        dummy: Some("selfedge".to_string()),
                        edge_label: Some(se.label.clone()),
                        edge_obj: Some(se.edge_obj.clone()),
                        ..Default::default()
                    },
                );
            }
        }
    }
}

pub fn position_self_edges(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    position_self_edges_with(g, |graph, ids| {
        let _ = remove_nodes_batch(graph, ids);
    });
}

fn position_self_edges_with<F>(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>, retire_batch: F)
where
    F: FnOnce(&mut Graph<NodeLabel, EdgeLabel, GraphLabel>, &[String]),
{
    let mut ids: Vec<String> = Vec::new();
    g.for_each_node(|id, n| {
        if n.dummy.as_deref() == Some("selfedge") {
            ids.push(id.to_string());
        }
    });

    // Generated self-edge dummies always point back to ordinary source nodes. A malformed graph
    // can instead make one dummy depend on another; immediate removal then affects later endpoint
    // lookup and `set_edge_named` endpoint recreation. Keep the former sequential semantics for
    // that defensive case rather than admitting it into a non-equivalent batch.
    if has_cross_dummy_dependency(g, &ids) {
        for id in ids {
            if restore_self_edge(g, &id) {
                let _ = g.remove_node(&id);
            }
        }
        return;
    }

    let mut retired_ids = Vec::with_capacity(ids.len());
    for id in ids {
        if restore_self_edge(g, &id) {
            retired_ids.push(id);
        }
    }

    if retired_ids.is_empty() {
        return;
    }

    // Edge restoration is complete, so one graph-wide pass can retire the independent dummies
    // without changing surviving edge order.
    retire_batch(g, &retired_ids);
}

fn remove_nodes_batch(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>, ids: &[String]) -> usize {
    g.remove_nodes(ids.iter().map(String::as_str))
}

fn has_cross_dummy_dependency(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, ids: &[String]) -> bool {
    let dummy_ids: FxHashSet<&str> = ids.iter().map(String::as_str).collect();
    ids.iter().any(|id| {
        let Some(edge_obj) = g.node(id).and_then(|node| node.edge_obj.as_ref()) else {
            return false;
        };
        (edge_obj.v.as_str() != id.as_str() && dummy_ids.contains(edge_obj.v.as_str()))
            || (edge_obj.w.as_str() != id.as_str() && dummy_ids.contains(edge_obj.w.as_str()))
    })
}

fn restore_self_edge(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>, id: &str) -> bool {
    let Some(node) = g.node(id).cloned() else {
        return false;
    };
    if node.dummy.as_deref() != Some("selfedge") {
        return false;
    }
    let (Some(x), Some(y)) = (node.x, node.y) else {
        return false;
    };
    let Some(edge_obj) = node.edge_obj.clone() else {
        return false;
    };
    let Some(mut label) = node.edge_label.clone() else {
        return false;
    };
    let Some(v_node) = g.node(&edge_obj.v) else {
        return false;
    };
    let (Some(vx), Some(vy)) = (v_node.x, v_node.y) else {
        return false;
    };

    // Match upstream Dagre (`positionSelfEdges`): do not apply any extra snapping before
    // computing the 2/3 and 5/6 fractions.
    let i = vx + v_node.width / 2.0;
    let a = vy;
    let o = x - i;
    let l = v_node.height / 2.0;

    label.points = vec![
        Point {
            x: i + 2.0 * o / 3.0,
            y: a - l,
        },
        Point {
            x: i + 5.0 * o / 6.0,
            y: a - l,
        },
        Point { x: i + o, y: a },
        Point {
            x: i + 5.0 * o / 6.0,
            y: a + l,
        },
        Point {
            x: i + 2.0 * o / 3.0,
            y: a + l,
        },
    ];
    label.x = Some(x);
    label.y = Some(y);

    g.set_edge_named(edge_obj.v, edge_obj.w, edge_obj.name, Some(label));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::{EdgeKey, GraphOptions};

    fn test_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        })
    }

    fn self_edge_dummy(name: &str, x: Option<f64>, y: Option<f64>) -> NodeLabel {
        let mut edge_label = EdgeLabel {
            width: 12.0,
            height: 8.0,
            ..Default::default()
        };
        edge_label.extras.insert(
            "source".to_string(),
            serde_json::Value::String(name.to_string()),
        );
        NodeLabel {
            width: 12.0,
            height: 8.0,
            x,
            y,
            dummy: Some("selfedge".to_string()),
            edge_label: Some(edge_label),
            edge_obj: Some(EdgeKey::new("a", "a", Some(name))),
            ..Default::default()
        }
    }

    fn position_self_edges_counted(graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) -> usize {
        let mut retirement_passes = 0;
        position_self_edges_with(graph, |graph, ids| {
            retirement_passes += 1;
            let _ = remove_nodes_batch(graph, ids);
        });
        retirement_passes
    }

    #[test]
    fn positioned_self_edges_retire_in_one_batch_and_preserve_edge_order() {
        let mut graph = test_graph();
        graph.set_node(
            "a",
            NodeLabel {
                width: 40.0,
                height: 20.0,
                x: Some(100.0),
                y: Some(80.0),
                ..Default::default()
            },
        );
        for id in ["x", "y", "cluster", "keep_child"] {
            graph.set_node(id, NodeLabel::default());
        }
        graph.set_node("self_1", self_edge_dummy("self_1", Some(150.0), Some(70.0)));
        graph.set_node("self_2", self_edge_dummy("self_2", Some(160.0), Some(90.0)));
        graph.set_node("invalid", self_edge_dummy("invalid", None, Some(100.0)));
        graph.set_parent("keep_child", "cluster");
        graph.set_parent("self_1", "cluster");
        graph.set_parent("self_2", "cluster");
        graph.set_parent("invalid", "cluster");
        graph.set_edge_named("x", "y", Some("keep_before"), Some(EdgeLabel::default()));
        graph.set_edge_named("y", "x", Some("keep_after"), Some(EdgeLabel::default()));

        let retirement_passes = position_self_edges_counted(&mut graph);

        assert_eq!(retirement_passes, 1);
        assert!(!graph.has_node("self_1"));
        assert!(!graph.has_node("self_2"));
        assert!(graph.has_node("invalid"));
        assert_eq!(
            graph.node_ids(),
            vec!["a", "x", "y", "cluster", "keep_child", "invalid"]
        );
        assert_eq!(graph.children("cluster"), vec!["keep_child", "invalid"]);
        assert_eq!(
            graph
                .edge_keys()
                .into_iter()
                .map(|edge| edge.name.unwrap())
                .collect::<Vec<_>>(),
            vec!["keep_before", "keep_after", "self_1", "self_2"],
        );

        let first = graph.edge("a", "a", Some("self_1")).unwrap();
        assert_eq!(first.width, 12.0);
        assert_eq!(first.height, 8.0);
        assert_eq!(
            first.extras.get("source"),
            Some(&serde_json::Value::String("self_1".to_string()))
        );
        assert_eq!(first.x, Some(150.0));
        assert_eq!(first.y, Some(70.0));
        assert_eq!(
            first.points,
            vec![
                Point { x: 140.0, y: 70.0 },
                Point { x: 145.0, y: 70.0 },
                Point { x: 150.0, y: 80.0 },
                Point { x: 145.0, y: 90.0 },
                Point { x: 140.0, y: 90.0 },
            ],
        );

        let second = graph.edge("a", "a", Some("self_2")).unwrap();
        assert_eq!(second.x, Some(160.0));
        assert_eq!(second.y, Some(90.0));
        let outer_x = 120.0 + 2.0 * 40.0 / 3.0;
        let inner_x = 120.0 + 5.0 * 40.0 / 6.0;
        assert_eq!(
            second.points,
            vec![
                Point {
                    x: outer_x,
                    y: 70.0,
                },
                Point {
                    x: inner_x,
                    y: 70.0,
                },
                Point { x: 160.0, y: 80.0 },
                Point {
                    x: inner_x,
                    y: 90.0,
                },
                Point {
                    x: outer_x,
                    y: 90.0,
                },
            ],
        );
    }

    #[test]
    fn cross_dummy_dependencies_preserve_sequential_endpoint_recreation() {
        let mut graph = test_graph();
        graph.set_node(
            "a",
            NodeLabel {
                width: 40.0,
                height: 20.0,
                x: Some(100.0),
                y: Some(80.0),
                ..Default::default()
            },
        );
        graph.set_node("cluster", NodeLabel::default());
        graph.set_node("self_1", self_edge_dummy("self_1", Some(150.0), Some(70.0)));

        let mut from_retired_source = self_edge_dummy("from_v", Some(160.0), Some(90.0));
        from_retired_source.edge_obj = Some(EdgeKey::new("self_1", "self_1", Some("from_v")));
        graph.set_node("from_v", from_retired_source);

        let mut to_retired_target = self_edge_dummy("from_w", Some(170.0), Some(100.0));
        to_retired_target.edge_obj = Some(EdgeKey::new("a", "self_1", Some("from_w")));
        graph.set_node("from_w", to_retired_target);

        graph.set_parent("self_1", "cluster");
        graph.set_parent("from_v", "cluster");
        graph.set_parent("from_w", "cluster");

        let retirement_passes = position_self_edges_counted(&mut graph);

        assert_eq!(retirement_passes, 0);
        assert_eq!(graph.node("self_1"), Some(&NodeLabel::default()));
        assert!(graph.has_node("from_v"));
        assert!(!graph.has_node("from_w"));
        assert_eq!(graph.node_ids(), vec!["a", "cluster", "from_v", "self_1"]);
        assert_eq!(graph.parent("self_1"), None);
        assert_eq!(graph.children("cluster"), vec!["from_v"]);
        assert!(!graph.has_edge("self_1", "self_1", Some("from_v")));

        let recreated = graph.edge("a", "self_1", Some("from_w")).unwrap();
        assert_eq!(recreated.x, Some(170.0));
        assert_eq!(recreated.y, Some(100.0));
        assert_eq!(
            graph
                .edge_keys()
                .into_iter()
                .map(|edge| edge.name.unwrap())
                .collect::<Vec<_>>(),
            vec!["self_1", "from_w"],
        );
    }

    #[test]
    fn self_edge_retirement_skips_the_batch_without_complete_dummies() {
        let mut graph = test_graph();
        graph.set_node("keep", NodeLabel::default());
        graph.set_node("source_without_coords", NodeLabel::default());
        graph.set_node(
            "missing_coords",
            self_edge_dummy("missing_coords", None, None),
        );

        let mut missing_edge_obj = self_edge_dummy("missing_edge_obj", Some(140.0), Some(70.0));
        missing_edge_obj.edge_obj = None;
        graph.set_node("missing_edge_obj", missing_edge_obj);

        let mut missing_edge_label = self_edge_dummy("missing_edge_label", Some(150.0), Some(80.0));
        missing_edge_label.edge_label = None;
        graph.set_node("missing_edge_label", missing_edge_label);

        let mut missing_source = self_edge_dummy("missing_source", Some(160.0), Some(90.0));
        missing_source.edge_obj = Some(EdgeKey::new(
            "missing_source_node",
            "missing_source_node",
            Some("missing_source"),
        ));
        graph.set_node("missing_source", missing_source);

        let mut source_without_coords =
            self_edge_dummy("source_without_coords", Some(170.0), Some(100.0));
        source_without_coords.edge_obj = Some(EdgeKey::new(
            "source_without_coords",
            "source_without_coords",
            Some("source_without_coords"),
        ));
        graph.set_node("source_without_coords_dummy", source_without_coords);

        let nodes_before = graph.node_ids();
        let retirement_passes = position_self_edges_counted(&mut graph);

        assert_eq!(retirement_passes, 0);
        assert_eq!(graph.node_ids(), nodes_before);
        for leftover in [
            "missing_coords",
            "missing_edge_obj",
            "missing_edge_label",
            "missing_source",
            "source_without_coords_dummy",
        ] {
            assert!(graph.has_node(leftover));
        }
        assert_eq!(graph.edge_count(), 0);
    }
}
