//! Ranking algorithms (network simplex, tight tree, longest path).
//!
//! Ported from Dagre's `rank.js` and related helpers. The implementation here is parity-oriented
//! (deterministic and defensive) to support headless diagram rendering.

pub mod feasible_tree;
pub mod network_simplex;
pub mod tree;
pub mod util;

pub fn rank(g: &mut crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>) {
    let mut work_control = crate::NoopWorkControl;
    rank_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject rank work");
}

pub(crate) fn rank_controlled(
    g: &mut crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
    work_control: &mut dyn crate::WorkControl,
) -> Result<(), crate::WorkError> {
    let ranker = g.graph().ranker.clone();
    match ranker.as_deref() {
        Some("network-simplex") => network_simplex::network_simplex_controlled(g, work_control)?,
        Some("tight-tree") => {
            work_control.charge(longest_path_work_units(g)?)?;
            util::longest_path_controlled(g)?;
            let _ = feasible_tree::feasible_tree_controlled(g, work_control)?;
        }
        Some("longest-path") => {
            work_control.charge(longest_path_work_units(g)?)?;
            util::longest_path_controlled(g)?;
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

fn longest_path_work_units(
    g: &crate::graphlib::Graph<crate::NodeLabel, crate::EdgeLabel, crate::GraphLabel>,
) -> Result<usize, crate::WorkError> {
    crate::work::checked_add(g.node_count(), crate::work::checked_mul(g.edge_count(), 2)?)
}
