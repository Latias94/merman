//! Self-edge extraction and reinsertion.
//!
//! Upstream Dagre temporarily removes self-loop edges before ranking/normalization and later
//! re-inserts them via dummy `"selfedge"` nodes during BK positioning. This keeps the ranker
//! constraints valid and makes self-loops deterministic.

use rustc_hash::FxHashSet;

use crate::graphlib::Graph;
use crate::order::IndexedLayerMatrix;
use crate::work::{checked_add, checked_mul};
use crate::{
    EdgeLabel, GraphLabel, NodeLabel, NoopWorkControl, Point, SelfEdge, WorkControl, WorkError,
};

pub fn remove_self_edges(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let _ = remove_self_edges_with_count(g);
}

pub(crate) fn remove_self_edges_with_count(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> usize {
    let self_loop_keys: Vec<_> = g.edges().filter(|ek| ek.v == ek.w).cloned().collect();
    let mut removed = 0usize;
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
        removed += 1;
    }
    removed
}

pub fn insert_self_edges(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    let mut layering = crate::order::build_current_layer_matrix_ix_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject self-edge layering");
    insert_self_edges_with_layering_controlled(g, &mut layering, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject self-edge insertion");
}

pub(crate) fn insert_self_edges_with_layering_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &mut IndexedLayerMatrix,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    // The indexed matrix is the exact artifact accepted by ordering. Scan it once to plan every
    // derived dummy before changing either graph labels or layer storage.
    work_control.charge(checked_add(layering.rank_count(), layering.slot_count())?)?;
    let mut layer_capacities = Vec::with_capacity(layering.rank_count());
    let mut source_nodes = 0usize;
    let mut dummy_count = 0usize;
    for layer in layering.layers() {
        let mut layer_dummies = 0usize;
        for &graph_ix in layer {
            let Some(node) = g.node_label_by_ix(graph_ix) else {
                continue;
            };
            source_nodes = checked_add(source_nodes, 1)?;
            layer_dummies = checked_add(layer_dummies, node.self_edges.len())?;
        }
        dummy_count = checked_add(dummy_count, layer_dummies)?;
        layer_capacities.push(checked_add(layer.len(), layer_dummies)?);
    }
    if dummy_count == 0 {
        return Ok(());
    }

    let final_slots = checked_add(layering.slot_count(), dummy_count)?;
    let final_occupied_entries = checked_add(layering.occupied_entries(), dummy_count)?;
    // Global `_seN` candidates never repeat. Across the complete batch, every failed probe can
    // therefore collide with at most one pre-existing live node, followed by one success per
    // dummy. Charge that conservative bound before generating IDs or mutating the graph.
    let id_probe_bound = checked_add(g.node_count(), dummy_count)?;
    let execution_work = checked_add(
        checked_add(layering.rank_count(), final_slots)?,
        checked_add(
            source_nodes,
            checked_add(checked_mul(dummy_count, 2)?, id_probe_bound)?,
        )?,
    )?;
    work_control.charge(execution_work)?;

    let mut dummy_ids = Vec::with_capacity(dummy_count);
    let mut next_candidate = || crate::util::unique_id("_se");
    while dummy_ids.len() < dummy_count {
        dummy_ids.push(next_available_selfedge_id(g, &mut next_candidate));
    }
    let mut dummy_ids = dummy_ids.into_iter();

    for (layer, final_capacity) in layering.layers_mut().iter_mut().zip(layer_capacities) {
        let source_layer = std::mem::take(layer);
        let mut expanded = Vec::with_capacity(final_capacity);
        for graph_ix in source_layer {
            let Some((rank, self_edges)) = g.node_label_mut_by_ix(graph_ix).and_then(|node| {
                let rank = node.rank?;
                // The expanded layer itself is the authoritative Dagre order artifact. Reading
                // its admitted length avoids a second order accumulator and any unchecked sum.
                node.order = Some(expanded.len());
                Some((rank, std::mem::take(&mut node.self_edges)))
            }) else {
                // Canonical layout produces dense, unique layers. Preserve any defensive sparse
                // slot here so an internal invariant failure is not silently re-ordered.
                expanded.push(graph_ix);
                continue;
            };
            expanded.push(graph_ix);

            for self_edge in self_edges {
                let selfedge_id = dummy_ids
                    .next()
                    .expect("the admitted dummy ID batch matches the self-edge count");
                let width = self_edge.label.width;
                let height = self_edge.label.height;
                g.set_node(
                    selfedge_id.clone(),
                    NodeLabel {
                        width,
                        height,
                        rank: Some(rank),
                        order: Some(expanded.len()),
                        dummy: Some("selfedge".to_string()),
                        edge_label: Some(self_edge.label),
                        edge_obj: Some(self_edge.edge_obj),
                        ..Default::default()
                    },
                );
                let dummy_ix = g
                    .node_ix(&selfedge_id)
                    .expect("a newly inserted self-edge dummy has a graph index");
                expanded.push(dummy_ix);
            }
        }
        debug_assert_eq!(expanded.len(), final_capacity);
        *layer = expanded;
    }
    debug_assert!(dummy_ids.next().is_none());
    layering.record_entry_counts(final_slots, final_occupied_entries);
    Ok(())
}

fn next_available_selfedge_id(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    next_candidate: &mut impl FnMut() -> String,
) -> String {
    loop {
        let candidate = next_candidate();
        if !g.has_node(&candidate) {
            return candidate;
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

    #[derive(Debug, Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        remaining: Option<usize>,
    }

    impl RecordingWorkControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                remaining: Some(limit),
                ..Self::default()
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            let Some(remaining) = self.remaining else {
                return Ok(());
            };
            let Some(next) = remaining.checked_sub(units) else {
                return Err(WorkError::Interrupted);
            };
            self.remaining = Some(next);
            Ok(())
        }
    }

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

    fn ranked_self_edge_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = test_graph();
        graph.set_graph(GraphLabel::default());
        for (id, rank, order) in [("a", 0, 0), ("b", 0, 1), ("c", 1, 0)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    order: Some(order),
                    ..Default::default()
                },
            );
        }
        for (node, name, width) in [
            ("a", "a-first", 11.0),
            ("a", "a-second", 12.0),
            ("b", "b-only", 13.0),
        ] {
            graph.set_edge_named(
                node,
                node,
                Some(name),
                Some(EdgeLabel {
                    width,
                    height: 7.0,
                    ..Default::default()
                }),
            );
        }
        remove_self_edges(&mut graph);
        graph
    }

    fn build_indexed_layering(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> IndexedLayerMatrix {
        let mut work_control = crate::NoopWorkControl;
        crate::order::build_current_layer_matrix_ix_controlled(graph, &mut work_control)
            .expect("the compact ranked fixture has a valid indexed layering")
    }

    #[test]
    fn controlled_insertion_expands_the_accepted_layering_in_source_order() {
        let mut graph = ranked_self_edge_graph();
        let mut layering = build_indexed_layering(&graph);
        assert!(layering.has_dense_unique_orders());

        let mut work_control = RecordingWorkControl::default();
        insert_self_edges_with_layering_controlled(&mut graph, &mut layering, &mut work_control)
            .expect("the admitted self-edge insertion succeeds");
        assert_eq!(work_control.charges.len(), 2);
        assert_eq!(layering.slot_count(), 6);
        assert!(layering.has_dense_unique_orders());

        let mut noop = crate::NoopWorkControl;
        let ids = layering
            .to_node_ids_controlled(&graph, &mut noop)
            .expect("indexed IDs materialize");
        let first_layer = ids[0]
            .iter()
            .map(|id| {
                graph
                    .node(id)
                    .and_then(|node| node.edge_obj.as_ref())
                    .and_then(|edge| edge.name.as_deref())
                    .unwrap_or(id)
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(first_layer, ["a", "a-first", "a-second", "b", "b-only"]);
        assert_eq!(ids[1], ["c"]);
        assert_eq!(graph.node("a").and_then(|node| node.order), Some(0));
        assert_eq!(graph.node("b").and_then(|node| node.order), Some(3));
        assert!(
            graph
                .node("a")
                .is_some_and(|node| node.self_edges.is_empty())
        );
        assert!(
            graph
                .node("b")
                .is_some_and(|node| node.self_edges.is_empty())
        );

        let rebuilt = build_indexed_layering(&graph);
        assert_eq!(layering, rebuilt);
    }

    #[test]
    fn public_insertion_reuses_the_controlled_source_order() {
        let mut graph = ranked_self_edge_graph();

        insert_self_edges(&mut graph);

        let layering = crate::util::build_layer_matrix(&graph);
        let first_layer = layering[0]
            .iter()
            .map(|id| {
                graph
                    .node(id)
                    .and_then(|node| node.edge_obj.as_ref())
                    .and_then(|edge| edge.name.as_deref())
                    .unwrap_or(id)
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(first_layer, ["a", "a-first", "a-second", "b", "b-only"]);
        assert_eq!(layering[1], ["c"]);
        assert_eq!(graph.node("a").and_then(|node| node.order), Some(0));
        assert_eq!(graph.node("b").and_then(|node| node.order), Some(3));
    }

    #[test]
    fn controlled_insertion_rejects_before_graph_or_layering_mutation() {
        let mut measured_graph = ranked_self_edge_graph();
        let mut measured_layering = build_indexed_layering(&measured_graph);
        let mut measured = RecordingWorkControl::default();
        insert_self_edges_with_layering_controlled(
            &mut measured_graph,
            &mut measured_layering,
            &mut measured,
        )
        .expect("the unbounded fixture succeeds");
        assert_eq!(measured.charges.len(), 2);
        let exact = measured.charges.iter().sum::<usize>();

        let mut rejected_graph = ranked_self_edge_graph();
        let mut rejected_layering = build_indexed_layering(&rejected_graph);
        let source_layering = rejected_layering.clone();
        let source_nodes = rejected_graph.node_ids();
        let source_orders = [
            rejected_graph.node("a").and_then(|node| node.order),
            rejected_graph.node("b").and_then(|node| node.order),
        ];
        let mut rejected = RecordingWorkControl::with_limit(exact - 1);
        assert_eq!(
            insert_self_edges_with_layering_controlled(
                &mut rejected_graph,
                &mut rejected_layering,
                &mut rejected,
            ),
            Err(WorkError::Interrupted)
        );
        assert_eq!(rejected_layering, source_layering);
        assert_eq!(rejected_graph.node_ids(), source_nodes);
        assert_eq!(
            [
                rejected_graph.node("a").and_then(|node| node.order),
                rejected_graph.node("b").and_then(|node| node.order),
            ],
            source_orders
        );
        assert_eq!(
            rejected_graph
                .node("a")
                .map_or(0, |node| node.self_edges.len()),
            2
        );
        assert_eq!(
            rejected_graph
                .node("b")
                .map_or(0, |node| node.self_edges.len()),
            1
        );

        let mut admitted_graph = ranked_self_edge_graph();
        let mut admitted_layering = build_indexed_layering(&admitted_graph);
        let mut admitted = RecordingWorkControl::with_limit(exact);
        insert_self_edges_with_layering_controlled(
            &mut admitted_graph,
            &mut admitted_layering,
            &mut admitted,
        )
        .expect("the exact self-edge budget succeeds");
        assert_eq!(admitted.remaining, Some(0));
    }

    #[test]
    fn controlled_insertion_without_self_edges_keeps_layer_storage() {
        let mut graph = ranked_self_edge_graph();
        for id in ["a", "b"] {
            graph
                .node_mut(id)
                .expect("fixture node exists")
                .self_edges
                .clear();
        }
        let mut layering = build_indexed_layering(&graph);
        let source = layering.clone();
        let first_layer_ptr = layering.layers()[0].as_ptr();
        let mut work_control = RecordingWorkControl::default();

        insert_self_edges_with_layering_controlled(&mut graph, &mut layering, &mut work_control)
            .expect("a no-self-edge matrix is a no-op");

        assert_eq!(work_control.charges.len(), 1);
        assert_eq!(layering, source);
        assert_eq!(layering.layers()[0].as_ptr(), first_layer_ptr);
    }

    #[test]
    fn selfedge_id_selection_skips_existing_user_nodes() {
        let mut graph = test_graph();
        graph.set_node(
            "_se-collision",
            NodeLabel {
                width: 99.0,
                ..Default::default()
            },
        );
        let mut candidates = ["_se-collision", "_se-free"].into_iter().map(str::to_owned);
        let selected = next_available_selfedge_id(&graph, &mut || {
            candidates.next().expect("the fixture provides a free ID")
        });

        assert_eq!(selected, "_se-free");
        assert_eq!(
            graph.node("_se-collision").map(|node| node.width),
            Some(99.0)
        );
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

    fn self_edge_curve_graph(count: usize) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
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
        for index in 0..count {
            let id = format!("self_{index}");
            graph.set_node(
                &id,
                self_edge_dummy(&id, Some(150.0 + index as f64), Some(70.0 + index as f64)),
            );
        }
        graph
    }

    fn position_self_edges_sequentially(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> usize {
        let mut ids = Vec::new();
        graph.for_each_node(|id, node| {
            if node.dummy.as_deref() == Some("selfedge") {
                ids.push(id.to_string());
            }
        });
        let mut removals = 0;
        for id in ids {
            if restore_self_edge(graph, &id) && graph.remove_node(&id) {
                removals += 1;
            }
        }
        removals
    }

    #[test]
    fn self_edge_retirement_curve_reduces_live_removals_to_one_graph_pass() {
        for count in [1, 2, 4, 8, 16, 32, 64, 128] {
            let mut sequential = self_edge_curve_graph(count);
            let sequential_removals = position_self_edges_sequentially(&mut sequential);

            let mut batched = self_edge_curve_graph(count);
            let graph_passes = position_self_edges_counted(&mut batched);

            assert_eq!(sequential_removals, count);
            assert_eq!(graph_passes, 1);
            assert_eq!(batched.node_ids(), sequential.node_ids());
            assert_eq!(batched.edge_keys(), sequential.edge_keys());
            for edge in batched.edge_keys() {
                assert_eq!(batched.edge_by_key(&edge), sequential.edge_by_key(&edge));
            }
        }
    }
}
