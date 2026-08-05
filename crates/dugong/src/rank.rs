//! Ranking algorithms (network simplex, tight tree, longest path).
//!
//! Ported from Dagre's `rank.js` and related helpers. The implementation here is parity-oriented
//! (deterministic and defensive) to support headless diagram rendering.

pub mod feasible_tree;
pub mod network_simplex;
pub mod tree;
pub mod util;

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
}
