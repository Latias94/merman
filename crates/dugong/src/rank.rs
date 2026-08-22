//! Ranking algorithms (network simplex, tight tree, longest path).
//!
//! Ported from Dagre's `rank.js` and related helpers. The implementation here is parity-oriented
//! (deterministic and defensive) to support headless diagram rendering.

pub mod feasible_tree;
pub mod network_simplex;
pub mod tree;
pub mod util;

/// Discrete Dagre rank assigned to one caller-owned node.
///
/// Leaf nodes receive `rank`. Compound nodes instead expose the inclusive rank span derived from
/// their nesting-border nodes, matching Dagre's `assignRankMinMax` phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankPlanNode {
    pub id: String,
    pub rank: Option<i32>,
    pub min_rank: Option<i32>,
    pub max_rank: Option<i32>,
}

/// Rank-only result from the canonical Dagre cycle-breaking, nesting, and ranker phases.
///
/// `reversed_edges` contains original caller edge keys selected by Dagre's feedback-arc pass. The
/// temporary rank graph may reverse those edges internally, but their caller-facing identity is
/// restored here so terminal renderers can retain marker and label ownership.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RankPlan {
    pub nodes: Vec<RankPlanNode>,
    pub reversed_edges: Vec<crate::graphlib::EdgeKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RankError {
    Work(crate::WorkError),
    InvalidNetworkSimplexTree,
}

impl From<crate::WorkError> for RankError {
    fn from(error: crate::WorkError) -> Self {
        Self::Work(error)
    }
}

impl From<RankError> for crate::LayoutError {
    fn from(error: RankError) -> Self {
        match error {
            RankError::Work(error) => Self::Work(error),
            RankError::InvalidNetworkSimplexTree => Self::InvalidNetworkSimplexTree,
        }
    }
}

pub fn rank(
    g: &mut crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
) -> Result<(), crate::LayoutError> {
    let mut work_control = crate::NoopWorkControl;
    rank_controlled(g, &mut work_control).map_err(crate::LayoutError::from)
}

/// Runs the canonical Dagre rank phases without coordinate assignment.
///
/// The caller graph is never mutated. Unlike the complete layout pipeline, this rank-only seam
/// keeps caller `minlen` values in semantic rank units instead of doubling them for SVG edge-label
/// proxy placement.
pub fn plan(
    g: &crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
) -> Result<RankPlan, crate::LayoutError> {
    let mut work_control = crate::NoopWorkControl;
    plan_controlled(g, &mut work_control)
}

/// Runs [`plan`] under caller-owned work control.
pub fn plan_controlled(
    g: &crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
    work_control: &mut dyn crate::WorkControl,
) -> Result<RankPlan, crate::LayoutError> {
    crate::pipeline::rank_plan_controlled(g, work_control)
}

pub(crate) fn rank_controlled(
    g: &mut crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
    work_control: &mut dyn crate::WorkControl,
) -> Result<(), RankError> {
    let ranker = g.graph().ranker.clone();
    match ranker.as_deref() {
        Some("network-simplex") => network_simplex::network_simplex_controlled(g, work_control)?,
        Some("tight-tree") => {
            util::longest_path_controlled(g, work_control)?;
            let _ = feasible_tree::feasible_tree_controlled(g, work_control)?;
        }
        Some("longest-path") => {
            util::longest_path_controlled(g, work_control)?;
        }
        _ => network_simplex::network_simplex_controlled(g, work_control)?,
    }
    Ok(())
}

pub(crate) fn validate_rank_arithmetic(
    g: &crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
) -> Result<(), crate::WorkError> {
    for edge in g.edges() {
        let minlen = g.edge_by_key(edge).map_or(1, |label| label.minlen);
        if minlen > i32::MAX as usize {
            return Err(crate::WorkError::ArithmeticOverflow);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::{Graph, GraphOptions};
    use crate::{EdgeLabel, GraphLabel, NodeLabel, WorkControl, WorkError};

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            Ok(())
        }
    }

    fn chain(ranker: &str) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel {
            ranker: Some(ranker.to_string()),
            ..GraphLabel::default()
        });
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        });
        graph.set_path(&["a", "b", "c"]);
        graph
    }

    #[test]
    fn rankers_share_one_longest_path_owner_tranche() {
        let mut longest_graph = chain("longest-path");
        let mut longest_control = RecordingWorkControl::default();
        rank_controlled(&mut longest_graph, &mut longest_control)
            .expect("longest-path ranking succeeds");
        assert_eq!(longest_control.charges.len(), 1);

        let mut tight_graph = chain("tight-tree");
        let mut tight_control = RecordingWorkControl::default();
        rank_controlled(&mut tight_graph, &mut tight_control).expect("tight-tree ranking succeeds");
        assert!(tight_control.charges.len() > 1);
        assert_eq!(tight_control.charges[0], longest_control.charges[0]);
    }

    fn rank_plan_graph(ranker: &str) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.set_graph(GraphLabel {
            ranker: Some(ranker.to_string()),
            ..GraphLabel::default()
        });
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        });
        graph
    }

    fn planned_rank(plan: &RankPlan, id: &str) -> i32 {
        plan.nodes
            .iter()
            .find(|node| node.id == id)
            .and_then(|node| node.rank)
            .unwrap_or_else(|| panic!("missing planned rank for {id}"))
    }

    #[test]
    fn rank_plan_preserves_semantic_minlen_for_every_ranker() {
        for ranker in ["network-simplex", "tight-tree", "longest-path"] {
            let mut graph = rank_plan_graph(ranker);
            graph.set_node("a", NodeLabel::default());
            graph.set_node("b", NodeLabel::default());
            graph.set_node("c", NodeLabel::default());
            graph.set_edge_with_label(
                "a",
                "b",
                EdgeLabel {
                    minlen: 3,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
            graph.set_edge("b", "c");

            let plan = plan(&graph).expect("rank-only Dagre plan should succeed");
            assert!(planned_rank(&plan, "b") - planned_rank(&plan, "a") >= 3);
            assert!(planned_rank(&plan, "c") - planned_rank(&plan, "b") >= 1);
        }
    }

    #[test]
    fn rank_plan_connects_disconnected_components_and_keeps_all_nodes_ranked() {
        let mut graph = rank_plan_graph("network-simplex");
        graph.set_edge_named("a", "b", Some("ab"), None);
        graph.set_edge_named("c", "d", Some("cd"), None);

        let plan = plan(&graph).expect("disconnected rank plan should succeed");
        assert_eq!(plan.nodes.len(), 4);
        assert!(plan.nodes.iter().all(|node| node.rank.is_some()));
    }

    #[test]
    fn rank_plan_restores_feedback_arc_identity() {
        let mut graph = rank_plan_graph("network-simplex");
        graph.set_edge_named("a", "b", Some("ab"), None);
        graph.set_edge_named("b", "c", Some("bc"), None);
        graph.set_edge_named("c", "a", Some("ca"), None);

        let plan = plan(&graph).expect("cyclic rank plan should succeed");
        assert_eq!(plan.reversed_edges.len(), 1);
        assert_eq!(
            plan.reversed_edges[0],
            crate::graphlib::EdgeKey::new("c", "a", Some("ca"))
        );
        assert!(plan.nodes.iter().all(|node| node.rank.is_some()));
    }

    #[test]
    fn rank_plan_greedy_cycle_breaking_honors_weights_and_restores_original_identity() {
        let mut graph = rank_plan_graph("network-simplex");
        graph.graph_mut().acyclicer = Some("greedy".to_string());
        for (from, to, name, weight) in [
            ("a", "b", "ab", 10.0),
            ("b", "c", "bc", 10.0),
            ("c", "a", "ca", 1.0),
        ] {
            graph.set_edge_named(
                from,
                to,
                Some(name),
                Some(EdgeLabel {
                    minlen: 1,
                    weight,
                    ..EdgeLabel::default()
                }),
            );
        }

        let plan = plan(&graph).expect("weighted cyclic rank plan should succeed");
        assert_eq!(
            plan.reversed_edges,
            vec![crate::graphlib::EdgeKey::new("c", "a", Some("ca"))]
        );
        assert!(plan.nodes.iter().all(|node| node.rank.is_some()));
    }

    #[test]
    fn rank_plan_ignores_self_loops_without_mutating_caller_edge_ownership() {
        let mut graph = rank_plan_graph("network-simplex");
        graph.set_edge_named("a", "a", Some("self"), None);

        let plan = plan(&graph).expect("self-loop rank plan should succeed");
        assert!(plan.reversed_edges.is_empty());
        assert_eq!(plan.nodes.len(), 1);
        assert!(plan.nodes[0].rank.is_some());
        assert!(graph.has_edge("a", "a", Some("self")));
    }

    #[test]
    fn rank_plan_exposes_compound_rank_spans_without_assigning_cluster_leaf_ranks() {
        let mut graph = rank_plan_graph("network-simplex");
        graph.set_node("outer", NodeLabel::default());
        graph.set_node("cluster", NodeLabel::default());
        graph.set_node("a", NodeLabel::default());
        graph.set_node("b", NodeLabel::default());
        graph.set_parent("cluster", "outer");
        graph.set_parent("a", "cluster");
        graph.set_parent("b", "cluster");
        graph.set_edge("a", "b");

        let plan = plan(&graph).expect("compound rank plan should succeed");
        let cluster = plan
            .nodes
            .iter()
            .find(|node| node.id == "cluster")
            .expect("compound node should remain in the rank plan");
        assert_eq!(cluster.rank, None);
        let min_rank = cluster
            .min_rank
            .expect("compound min rank should be assigned");
        let max_rank = cluster
            .max_rank
            .expect("compound max rank should be assigned");
        assert!(max_rank >= min_rank);

        let outer = plan
            .nodes
            .iter()
            .find(|node| node.id == "outer")
            .expect("outer compound node should remain in the rank plan");
        assert_eq!(outer.rank, None);
        assert!(outer.min_rank.is_some_and(|rank| rank <= min_rank));
        assert!(outer.max_rank.is_some_and(|rank| rank >= max_rank));
    }
}
