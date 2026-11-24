mod anchors;

use super::ir::{Axis, Direction, MeasuredGraph, Point, Size, SolvedGraph};
use super::shoal_bridge::{self, NodeHint, OrderHint, Placement, WidthPolicy};
use crate::layout_frame::{FrameDirection, GraphPlacement, ShoalFrameNode};
use crate::layout_work::OperationLayoutWorkControl;
use crate::{
    Error, LayoutAdaptation, LayoutAdaptationObservation, LayoutExecution, Result, ShoalLayoutFrame,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// IDs are supplied in measured node and group order so persisted hints survive reindexing.
pub(crate) fn solve<'id>(
    graph: &MeasuredGraph,
    node_ids: impl ExactSizeIterator<Item = &'id str> + Clone,
    group_ids: impl ExactSizeIterator<Item = &'id str> + Clone,
    execution: Option<&LayoutExecution<'_>>,
    flow_relative_nodes: &[(usize, Axis)],
    allow_turn: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<(SolvedGraph, GraphPlacement)> {
    let Some(execution) = execution else {
        let solved = shoal_bridge::solve(
            graph,
            Placement {
                flow_relative_nodes,
                ..Placement::default()
            },
            work,
        )?;
        return Ok((solved, GraphPlacement::default()));
    };
    let fit_width = execution.adaptation == LayoutAdaptation::FitWidth;
    let width = if fit_width {
        WidthPolicy::FitWidth {
            max_width: execution.container_width,
            allow_turn,
            turn_breakpoint: 640.0,
            fold_wide_ranks: execution.fold_wide_ranks,
        }
    } else {
        WidthPolicy::Natural
    };
    let family = execution.family_id().as_str();
    let previous = execution
        .previous_shoal_frame
        .as_deref()
        .filter(|frame| frame.family == family && !frame.nodes.is_empty());
    let capture = execution.capture_shoal_frame;
    if (capture || previous.is_some())
        && (node_ids.len() != graph.nodes.len() || group_ids.len() != graph.groups.len())
    {
        return Err(Error::InvalidModel {
            message: "Shoal frame identities do not match the measured graph".into(),
        });
    }
    let anchors = if graph.groups.is_empty()
        && (capture
            || previous.is_some_and(|frame| frame.nodes.values().any(|n| n.position.is_some())))
    {
        Some(anchors::Components::new(graph, node_ids.clone(), work)?)
    } else {
        None
    };
    let hints = if let Some(previous) = previous {
        hints(
            graph,
            node_ids.clone(),
            group_ids.clone(),
            previous,
            anchors.as_ref(),
            work,
        )?
    } else {
        Vec::new()
    };
    let solved = shoal_bridge::solve(
        graph,
        Placement {
            width,
            capture_ranks: capture,
            reuse_components: !fit_width && anchors.is_some(),
            hints: &hints,
            incident_signatures: anchors
                .as_ref()
                .map_or(&[], anchors::Components::signatures),
            flow_relative_nodes,
        },
        work,
    )?;
    let placement = GraphPlacement {
        adaptation: fit_width.then(|| {
            LayoutAdaptationObservation::fit_width(execution.container_width, solved.bounds.width)
        }),
        frame: if capture {
            Some(Arc::new(frame(
                graph, &solved, node_ids, group_ids, family, anchors, work,
            )?))
        } else {
            None
        },
    };
    Ok((solved, placement))
}

fn hints<'id>(
    graph: &MeasuredGraph,
    nodes: impl ExactSizeIterator<Item = &'id str> + Clone,
    groups: impl ExactSizeIterator<Item = &'id str>,
    previous: &ShoalLayoutFrame,
    anchors: Option<&anchors::Components>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<Option<NodeHint>>> {
    let comparisons = previous.nodes.len().ilog2() as usize + 1;
    work.charge_adapter(work.checked_mul(nodes.len(), comparisons)?)?;
    work.charge_adapter(previous.groups.len())?;
    let mut previous_groups = HashMap::with_capacity(previous.groups.len());
    for (index, id) in previous.groups.iter().enumerate() {
        work.charge_adapter(id.len())?;
        previous_groups.insert(id.as_str(), index);
    }
    let mut parents = Vec::with_capacity(groups.len());
    for id in groups {
        work.charge_adapter(id.len())?;
        parents.push(previous_groups.get(id).copied());
    }
    let matching = anchors
        .map(|anchors| anchors.matches(nodes.clone(), previous, work))
        .transpose()?;
    let mut hints = Vec::with_capacity(nodes.len());
    for (index, (id, measured)) in nodes.zip(&graph.nodes).enumerate() {
        work.charge_adapter(id.len())?;
        let hint = previous.nodes.get(id).and_then(|node| {
            let same_parent = match (measured.parent, node.parent) {
                (None, None) => true,
                (Some(group), Some(previous_group)) => {
                    parents[group as usize] == Some(previous_group as usize)
                }
                _ => false,
            };
            same_parent.then(|| NodeHint {
                order: OrderHint {
                    rank: i32::try_from(node.rank).unwrap_or(i32::MAX),
                    cross_key: node.cross_key,
                },
                direction: Direction::from(node.direction),
                position: anchors
                    .zip(node.position)
                    .filter(|_| matching.as_ref().is_some_and(|matching| matching[index]))
                    .map(|(anchors, position)| {
                        (
                            merman_layout_shoal::PositionHint {
                                center: Point {
                                    x: position.x,
                                    y: position.y,
                                },
                                size: Size {
                                    width: position.width,
                                    height: position.height,
                                },
                                incident_signature: Some(position.incident_signature.bits()),
                            },
                            anchors.signature(index),
                        )
                    }),
            })
        });
        hints.push(hint);
    }
    Ok(hints)
}

fn frame<'id>(
    graph: &MeasuredGraph,
    solved: &SolvedGraph,
    node_ids: impl ExactSizeIterator<Item = &'id str>,
    group_ids: impl ExactSizeIterator<Item = &'id str>,
    family: &str,
    anchors: Option<anchors::Components>,
    work: &mut OperationLayoutWorkControl,
) -> Result<ShoalLayoutFrame> {
    if solved.node_ranks.len() != node_ids.len() {
        return Err(Error::InvalidModel {
            message: "Shoal omitted the requested node ranks".into(),
        });
    }
    let comparisons = node_ids.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge_adapter(work.checked_mul(node_ids.len(), comparisons)?)?;
    let mut groups = Vec::with_capacity(group_ids.len());
    for id in group_ids {
        work.charge_adapter(id.len())?;
        groups.push(id.to_owned());
    }
    let mut nodes = BTreeMap::new();
    let positions = anchors
        .map(|anchors| anchors.positions(&solved.nodes, work))
        .transpose()?;
    for (index, ((id, measured), (&rank, rect))) in node_ids
        .zip(&graph.nodes)
        .zip(solved.node_ranks.iter().zip(&solved.nodes))
        .enumerate()
    {
        work.charge_adapter(id.len())?;
        let direction = measured.parent.map_or(solved.direction, |index| {
            solved.groups[index as usize].direction
        });
        let center = rect.center();
        let cross_key = match direction {
            Direction::Down | Direction::Up => center.x,
            Direction::Right | Direction::Left => center.y,
        };
        nodes.insert(
            id.to_owned(),
            ShoalFrameNode {
                parent: measured.parent,
                rank,
                cross_key,
                direction: direction.into(),
                position: positions.as_ref().map(|positions| positions[index]),
            },
        );
    }
    Ok(ShoalLayoutFrame {
        family: family.to_owned(),
        groups,
        nodes,
    })
}

impl From<FrameDirection> for Direction {
    fn from(direction: FrameDirection) -> Self {
        match direction {
            FrameDirection::Down => Self::Down,
            FrameDirection::Up => Self::Up,
            FrameDirection::Left => Self::Left,
            FrameDirection::Right => Self::Right,
        }
    }
}

impl From<Direction> for FrameDirection {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Down => Self::Down,
            Direction::Up => Self::Up,
            Direction::Left => Self::Left,
            Direction::Right => Self::Right,
        }
    }
}
