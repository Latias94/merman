#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-state",
    feature = "diagram-swimlane"
))]
pub(crate) mod self_loop;

pub(crate) fn rankdir(direction: crate::graph_layout::ir::Direction) -> dugong::RankDir {
    use crate::graph_layout::ir::Direction;
    use dugong::RankDir;

    match direction {
        Direction::Down => RankDir::TB,
        Direction::Up => RankDir::BT,
        Direction::Right => RankDir::LR,
        Direction::Left => RankDir::RL,
    }
}
