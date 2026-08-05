//! Coordinate system adjustment helpers.
//!
//! Dagre internally assumes a top-to-bottom coordinate system. For left-to-right / right-to-left
//! layouts we swap axes and restore them afterwards. This module mirrors upstream's
//! `coordinate-system.js`.

use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul};
use crate::{EdgeLabel, GraphLabel, NodeLabel, RankDir, WorkControl, WorkError};

pub fn adjust(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    match g.graph().rankdir {
        RankDir::LR | RankDir::RL => swap_width_height(g),
        RankDir::TB | RankDir::BT => {}
    }
}

pub(crate) fn adjust_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    if matches!(g.graph().rankdir, RankDir::LR | RankDir::RL) {
        work_control.charge(graph_scan_work_units(g)?)?;
    }
    adjust(g);
    Ok(())
}

pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    match g.graph().rankdir {
        RankDir::BT | RankDir::RL => reverse_y(g),
        RankDir::TB | RankDir::LR => {}
    }

    match g.graph().rankdir {
        RankDir::LR | RankDir::RL => {
            swap_xy(g);
            swap_width_height(g);
        }
        RankDir::TB | RankDir::BT => {}
    }
}

pub(crate) fn undo_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    let (graph_scan_multiplier, point_scan_multiplier) = match g.graph().rankdir {
        RankDir::TB => return Ok(()),
        RankDir::BT => (1usize, 1usize),
        RankDir::LR => (2usize, 1usize),
        RankDir::RL => (3usize, 2usize),
    };

    // Point cardinality is not stored separately. Admit the slot-backed planning scan before
    // reading every live edge's point length, then admit all rankdir-specific mutation passes.
    charge_nonzero(work_control, g.edge_slot_count())?;
    let mut point_count = Ok(0usize);
    g.for_each_edge(|_key, edge| {
        point_count = point_count.and_then(|total| checked_add(total, edge.points.len()));
    });
    let mutation_work = checked_add(
        checked_mul(graph_scan_work_units(g)?, graph_scan_multiplier)?,
        checked_mul(point_count?, point_scan_multiplier)?,
    )?;
    charge_nonzero(work_control, mutation_work)?;
    undo(g);
    Ok(())
}

fn graph_scan_work_units(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Result<usize, WorkError> {
    checked_add(g.node_order_slot_count(), g.edge_slot_count())
}

fn charge_nonzero(work_control: &mut dyn WorkControl, units: usize) -> Result<(), WorkError> {
    if units == 0 {
        return Ok(());
    }
    work_control.charge(units)
}

fn swap_width_height(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    g.for_each_node_mut(|_id, n| {
        (n.width, n.height) = (n.height, n.width);
    });
    g.for_each_edge_mut(|_ek, e| {
        (e.width, e.height) = (e.height, e.width);
    });
}

fn reverse_y(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    g.for_each_node_mut(|_id, n| {
        if let Some(y) = n.y {
            n.y = Some(-y);
        }
    });
    g.for_each_edge_mut(|_ek, e| {
        for p in &mut e.points {
            p.y = -p.y;
        }
        if let Some(y) = e.y {
            e.y = Some(-y);
        }
    });
}

fn swap_xy(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    g.for_each_node_mut(|_id, n| {
        if let (Some(x), Some(y)) = (n.x, n.y) {
            n.x = Some(y);
            n.y = Some(x);
        }
    });
    g.for_each_edge_mut(|_ek, e| {
        for p in &mut e.points {
            (p.x, p.y) = (p.y, p.x);
        }
        if let (Some(x), Some(y)) = (e.x, e.y) {
            e.x = Some(y);
            e.y = Some(x);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;
    use crate::graphlib::GraphOptions;

    #[derive(Default)]
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

    fn work_graph(rankdir: RankDir) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel {
            rankdir,
            ..GraphLabel::default()
        });
        for id in ["a", "b"] {
            graph.set_node(
                id,
                NodeLabel {
                    width: 10.0,
                    height: 20.0,
                    x: Some(30.0),
                    y: Some(40.0),
                    ..NodeLabel::default()
                },
            );
        }
        graph.set_edge_named("a", "b", Some("removed"), Some(EdgeLabel::default()));
        graph.set_edge_named(
            "a",
            "b",
            Some("live"),
            Some(EdgeLabel {
                width: 5.0,
                height: 7.0,
                x: Some(11.0),
                y: Some(13.0),
                points: vec![
                    Point { x: 1.0, y: 2.0 },
                    Point { x: 3.0, y: 4.0 },
                    Point { x: 5.0, y: 6.0 },
                ],
                ..EdgeLabel::default()
            }),
        );
        assert!(graph.remove_edge("a", "b", Some("removed")));
        graph
    }

    #[test]
    fn controlled_coordinate_work_matches_rankdir_scans_and_points() {
        for (rankdir, adjust_charges, undo_charges) in [
            (RankDir::TB, vec![], vec![]),
            (RankDir::BT, vec![], vec![2, 7]),
            (RankDir::LR, vec![4], vec![2, 11]),
            (RankDir::RL, vec![4], vec![2, 18]),
        ] {
            let mut adjusted = work_graph(rankdir);
            let mut adjust_work = RecordingWorkControl::default();
            adjust_controlled(&mut adjusted, &mut adjust_work).unwrap();
            assert_eq!(adjust_work.charges, adjust_charges, "adjust {rankdir:?}");

            let mut undone = work_graph(rankdir);
            let mut undo_work = RecordingWorkControl::default();
            undo_controlled(&mut undone, &mut undo_work).unwrap();
            assert_eq!(undo_work.charges, undo_charges, "undo {rankdir:?}");
        }
    }

    #[test]
    fn controlled_coordinate_undo_rejects_before_mutation() {
        let mut graph = work_graph(RankDir::RL);
        let source_node = graph.node("a").cloned();
        let source_edge = graph.edge("a", "b", Some("live")).cloned();
        let mut work_control = RecordingWorkControl::with_limit(2 + 18 - 1);

        assert_eq!(
            undo_controlled(&mut graph, &mut work_control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(work_control.charges, [2, 18]);
        assert_eq!(graph.node("a").cloned(), source_node);
        assert_eq!(graph.edge("a", "b", Some("live")).cloned(), source_edge);
    }
}
