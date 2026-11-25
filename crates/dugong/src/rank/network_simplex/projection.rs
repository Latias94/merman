//! Leaf and edge-endpoint projection from the directed private layout graph.

use super::{RankError, simplify_work_units_for, solve_simplified_controlled};
use crate::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use crate::util::SimplifiedEdges;
use crate::{EdgeLabel, GraphLabel, NodeLabel, WorkControl, WorkError};

type RankGraph = Graph<NodeLabel, EdgeLabel, GraphLabel>;

struct RankArc {
    tail: usize,
    head: usize,
    weight: f64,
    minlen: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Outside,
    Leaf,
    Implicit,
    RankedLeaf(i32),
}

pub(crate) struct LeafRankProjection {
    slots: Vec<(usize, Role)>,
    numeric_nodes: usize,
}

impl LeafRankProjection {
    fn collect(graph: &RankGraph) -> (Self, Vec<RankArc>) {
        let mut roles = vec![Role::Outside; graph.node_slot_count()];
        let mut slots = Vec::with_capacity(graph.node_count());
        let mut arcs = Vec::with_capacity(graph.edge_count());

        graph.for_each_node_ix(|source, id, _| {
            if graph.child_count(id) == 0 {
                roles[source] = Role::Leaf;
                if !is_javascript_array_index(id) {
                    slots.push((source, Role::Leaf));
                }
            }
        });
        graph.for_each_edge_ix(|tail, head, _, label| {
            arcs.push(RankArc {
                tail,
                head,
                weight: label.weight,
                minlen: label.minlen,
            });
            for source in [tail, head] {
                if roles[source] == Role::Outside {
                    roles[source] = Role::Implicit;
                    if graph.javascript_array_index_by_node_ix(source).is_none() {
                        slots.push((source, Role::Implicit));
                    }
                }
            }
        });

        let ordinary_nodes = slots.len();
        for id in graph.nodes().take(graph.array_index_node_count()) {
            if let Some(source) = graph.node_ix(id)
                && roles[source] != Role::Outside
            {
                slots.push((source, roles[source]));
            }
        }
        let numeric_nodes = slots.len() - ordinary_nodes;
        slots.rotate_right(numeric_nodes);
        (
            Self {
                slots,
                numeric_nodes,
            },
            arcs,
        )
    }

    fn simplify(&self, graph: &RankGraph, arcs: &[RankArc]) -> Box<RankGraph> {
        let mut simplified = Box::new(Graph::new(GraphOptions::default()));
        simplified.set_graph(graph.graph().clone());
        for &(source, role) in &self.slots {
            if let Some(id) = graph.node_id_by_ix(source) {
                let label = if role == Role::Leaf {
                    graph.node_label_by_ix(source).cloned().unwrap_or_default()
                } else {
                    NodeLabel::default()
                };
                simplified.set_node(id, label);
            }
        }
        let mut edges = SimplifiedEdges::default();
        for arc in arcs {
            edges.push(arc.tail, arc.head, arc.weight, arc.minlen);
        }
        edges.insert(graph, &mut simplified);
        simplified
    }

    fn stage(&mut self, simplified: &RankGraph) {
        for (projected, (_, role)) in self.slots.iter_mut().enumerate() {
            if *role == Role::Leaf
                && let Some(rank) = simplified
                    .node_label_by_ix(projected)
                    .and_then(|node| node.rank)
            {
                *role = Role::RankedLeaf(rank);
            }
        }
    }

    pub(crate) fn publish(self, graph: &mut RankGraph) {
        for (source, role) in self.slots {
            if let Role::RankedLeaf(rank) = role
                && let Some(node) = graph.node_label_mut_by_ix(source)
            {
                node.rank = Some(rank);
            }
        }
    }
}

pub(crate) fn rank_leaf_projection_controlled(
    graph: &RankGraph,
    work_control: &mut dyn WorkControl,
) -> Result<LeafRankProjection, RankError> {
    debug_assert!(graph.is_directed());
    let (mut projection, arcs) = LeafRankProjection::collect(graph);
    work_control.charge(simplify_work_units_for(
        projection.slots.len(),
        projection.numeric_nodes,
        graph.directed_array_index_adjacency_entry_count(),
        arcs.len(),
        arcs.len(),
    )?)?;
    if arcs.iter().any(|arc| arc.minlen > i32::MAX as usize) {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    let mut simplified = projection.simplify(graph, &arcs);
    drop(arcs);
    solve_simplified_controlled(&mut simplified, work_control, |simplified, work_control| {
        work_control.charge(projection.slots.len())?;
        projection.stage(simplified);
        Ok(())
    })?;
    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Limit {
        remaining: usize,
        attempted: Vec<usize>,
    }

    impl WorkControl for Limit {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.attempted.push(units);
            self.remaining = self
                .remaining
                .checked_sub(units)
                .ok_or(WorkError::Interrupted)?;
            Ok(())
        }
    }

    #[test]
    fn projected_simplification_preserves_slots_labels_and_canonical_edge_order() {
        let mut graph = RankGraph::new(GraphOptions {
            compound: true,
            multigraph: true,
            directed: true,
        });
        for (position, id) in [
            "late",
            "10",
            "omitted",
            "root",
            "early",
            "2",
            "a",
            "b",
            "c",
            "d",
            "e",
            "01",
            "4294967295",
        ]
        .into_iter()
        .enumerate()
        {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(position as i32),
                    ..NodeLabel::default()
                },
            );
        }
        for (child, parent) in [
            ("a", "late"),
            ("b", "10"),
            ("c", "omitted"),
            ("d", "early"),
            ("e", "2"),
        ] {
            graph.set_parent(child, parent);
        }
        graph.set_edge_named("root", "a", Some("retired"), None);
        for (tail, head, name, weight, minlen) in [
            ("early", "2", "first", 1e16, 0),
            ("root", "b", "zero", -0.0, 0),
            ("early", "2", "second", 1.0, 7),
            ("10", "late", "late", 2.0, 2),
            ("early", "2", "third", -1e16, 4),
            ("2", "early", "reverse", 3.0, 3),
            ("e", "01", "ordinary", 6.0, 3),
        ] {
            graph.set_edge_named(
                tail,
                head,
                Some(name),
                Some(EdgeLabel {
                    weight,
                    minlen,
                    ..EdgeLabel::default()
                }),
            );
        }
        assert!(graph.remove_edge("root", "a", Some("retired")));
        assert!(graph.edge_slot_count() > graph.edge_count());

        let legacy_view = crate::util::as_non_compound_graph(&graph);
        let expected = crate::util::simplify(&legacy_view);
        let (projection, arcs) = LeafRankProjection::collect(&graph);
        let actual = projection.simplify(&graph, &arcs);
        assert_eq!(projection.slots.len(), legacy_view.node_count());
        assert_eq!(
            projection.numeric_nodes,
            legacy_view.array_index_node_count()
        );
        assert_eq!(legacy_view.edge_slot_count(), graph.edge_count());
        assert_eq!(
            legacy_view.directed_array_index_adjacency_entry_count(),
            graph.directed_array_index_adjacency_entry_count()
        );
        assert_eq!(
            actual.node_ids(),
            [
                "2",
                "10",
                "root",
                "a",
                "b",
                "c",
                "d",
                "e",
                "01",
                "4294967295",
                "early",
                "late"
            ]
        );
        assert_eq!(actual.node_ids(), expected.node_ids());
        for id in actual.nodes() {
            assert_eq!(actual.node_ix(id), expected.node_ix(id));
            assert_eq!(
                actual.node(id).unwrap().rank,
                expected.node(id).unwrap().rank
            );
            assert_eq!(actual.successors(id), expected.successors(id));
            assert_eq!(actual.predecessors(id), expected.predecessors(id));
        }
        assert_eq!(actual.edge_keys(), expected.edge_keys());
        for (slot, key) in actual.edges().enumerate() {
            assert_eq!(expected.edge_key_by_ix(slot), Some(key));
            let actual = actual.edge_by_key(key).unwrap();
            let expected = expected.edge_by_key(key).unwrap();
            assert_eq!(actual.minlen, expected.minlen);
            assert_eq!(actual.weight.to_bits(), expected.weight.to_bits());
        }
        assert_eq!(actual.node("2").unwrap().rank, None);
        assert_eq!(actual.node("early").unwrap().rank, None);
        assert_eq!(actual.edge("early", "2", None).unwrap().minlen, 7);
        assert_eq!(actual.edge("root", "b", None).unwrap().minlen, 1);
    }

    #[test]
    fn simplify_admission_precedes_projection_minlen_arithmetic_errors() {
        use super::super::{network_simplex_controlled, simplify_work_units};

        let mut graph = RankGraph::new(GraphOptions::default());
        graph.set_edge_with_label(
            "10",
            "2",
            EdgeLabel {
                minlen: i32::MAX as usize + 1,
                ..EdgeLabel::default()
            },
        );
        let quote = simplify_work_units(&graph).unwrap();
        for (remaining, error) in [
            (quote - 1, WorkError::Interrupted),
            (quote, WorkError::ArithmeticOverflow),
        ] {
            let limit = || Limit {
                remaining,
                attempted: Vec::new(),
            };
            let mut legacy = limit();
            let mut candidate = limit();
            let mut copy = crate::util::as_non_compound_graph(&graph);
            assert_eq!(
                network_simplex_controlled(&mut copy, &mut legacy),
                Err(RankError::Work(error))
            );
            assert_eq!(
                rank_leaf_projection_controlled(&graph, &mut candidate).err(),
                Some(RankError::Work(error))
            );
            assert_eq!(legacy.attempted, [quote]);
            assert_eq!(candidate.attempted, legacy.attempted);
            assert_eq!(candidate.remaining, legacy.remaining);
        }
    }
}
