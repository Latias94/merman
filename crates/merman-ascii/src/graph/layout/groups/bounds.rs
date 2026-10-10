use super::super::super::label::{GraphLabel, GraphLabelMetrics};
use super::super::super::model::{AsciiGraph, AsciiGraphGroup, GraphDirection, GraphGroupKind};
use super::super::super::topology::GraphGroupTopology;
use super::super::grid;
use super::super::{DividerSpan, GroupLayout, NodeLayout};
use super::{LaidOutGroups, layout_work_allocation_failed};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{GraphLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use merman_core::OperationPhase;
use rustc_hash::FxHashMap as HashMap;
use std::collections::VecDeque;

const EMPTY_GROUP_RANK_GAP: usize = 2;

fn signed(value: usize, resources: &ResourceContext) -> Result<isize> {
    isize::try_from(value).map_err(|_| grid_overflow(resources))
}

fn signed_add(value: isize, extra: usize, resources: &ResourceContext) -> Result<isize> {
    value
        .checked_add(signed(extra, resources)?)
        .ok_or_else(|| grid_overflow(resources))
}

fn raw_span(start: isize, end: isize, resources: &ResourceContext) -> Result<usize> {
    end.checked_sub(start)
        .and_then(|size| size.checked_add(1))
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| grid_overflow(resources))
}

fn grid_overflow(resources: &ResourceContext) -> crate::error::AsciiError {
    resources.grid_overflow()
}

fn checked_node_right(layout: &NodeLayout, resources: &ResourceContext) -> Result<usize> {
    let width = layout
        .width
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    resources.checked_grid_add(layout.x, width)
}

fn checked_node_bottom(layout: &NodeLayout, resources: &ResourceContext) -> Result<usize> {
    let height = layout
        .height
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    resources.checked_grid_add(layout.y, height)
}

pub(super) fn empty_group_minimum_size(
    group: &AsciiGraphGroup,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    let title = empty_group_title_metrics(group, policy.terminal_width_profile, resources)?;
    empty_group_minimum_size_for_metrics(group, title, policy.group_title_clearance, resources)
}

fn empty_group_title(
    group: &AsciiGraphGroup,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<GraphLabel> {
    match group.kind {
        GraphGroupKind::Container => {
            GraphLabel::try_new_with_profile(&group.title, width_profile, resources)
        }
        GraphGroupKind::Divider => Ok(GraphLabel::empty_with_profile(width_profile)),
    }
}

fn empty_group_title_metrics(
    group: &AsciiGraphGroup,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<GraphLabelMetrics> {
    match group.kind {
        GraphGroupKind::Container => {
            GraphLabel::try_measure_with_profile(&group.title, width_profile, resources)
        }
        GraphGroupKind::Divider => {
            GraphLabel::try_measure_with_profile("", width_profile, resources)
        }
    }
}

fn empty_group_minimum_size_for_metrics(
    group: &AsciiGraphGroup,
    title: GraphLabelMetrics,
    group_title_clearance: usize,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    match group.kind {
        GraphGroupKind::Container => Ok((
            resources.checked_grid_add(title.width.max(1), 2)?.max(3),
            resources
                .checked_grid_add(title.content_height, group_title_clearance)?
                .max(4),
        )),
        // Divider groups still need a non-degenerate perimeter when they are edge endpoints.
        GraphGroupKind::Divider => Ok((3, 3)),
    }
}

pub(super) fn layout_scene_groups(
    graph: &AsciiGraph,
    layouts: &mut [NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(LaidOutGroups, usize, usize)> {
    let mut bounds =
        raw_group_bounds_batch(graph, layouts, topology, policy, resources, execution)?;
    resources.charge_layout_work(resources.checked_work_add(bounds.len(), layouts.len())?)?;
    let mut min_x = 0isize;
    let mut min_y = 0isize;
    for (index, bound) in bounds.iter().flatten().enumerate() {
        checkpoint_layout(execution, index)?;
        min_x = min_x.min(bound.x);
        min_y = min_y.min(bound.y);
    }
    let dx = min_x
        .checked_neg()
        .ok_or_else(|| grid_overflow(resources))?;
    let dy = min_y
        .checked_neg()
        .ok_or_else(|| grid_overflow(resources))?;
    let offset_x = usize::try_from(dx).map_err(|_| grid_overflow(resources))?;
    let offset_y = usize::try_from(dy).map_err(|_| grid_overflow(resources))?;
    for (index, bound) in bounds.iter_mut().flatten().enumerate() {
        checkpoint_layout(execution, index)?;
        bound.translate(dx, dy, resources)?;
    }
    for (index, node) in layouts.iter_mut().enumerate() {
        checkpoint_layout(execution, index)?;
        node.x = resources.checked_grid_add(node.x, offset_x)?;
        node.y = resources.checked_grid_add(node.y, offset_y)?;
    }
    let groups = layout_groups_from_bounds(
        graph, layouts, topology, policy, &bounds, resources, execution,
    )?;
    Ok((groups, offset_x, offset_y))
}

#[cfg(test)]
pub(super) fn layout_groups(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<LaidOutGroups> {
    let resolved_bounds =
        raw_group_bounds_batch(graph, layouts, topology, policy, resources, execution)?;
    layout_groups_from_bounds(
        graph,
        layouts,
        topology,
        policy,
        &resolved_bounds,
        resources,
        execution,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_groups_from_bounds(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resolved_bounds: &[Option<RawBounds>],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<LaidOutGroups> {
    // Charge indexed lookups plus the linear topology, layout, and output passes up front.
    let mut member_count = 0usize;
    for (group_index, group) in graph.groups.iter().enumerate() {
        checkpoint_layout(execution, group_index)?;
        member_count = resources.checked_work_add(member_count, group.nodes.len())?;
    }
    let group_visits = resources.checked_work_mul(graph.groups.len(), 6)?;
    let layout_work = resources.checked_work_add(
        resources.checked_work_add(layouts.len(), member_count)?,
        group_visits,
    )?;
    resources.charge_layout_work(layout_work)?;

    let mut child_first_order = child_first_group_order(graph, topology, resources, execution)?;
    let mut groups_by_graph_index = Vec::<Option<GroupLayout>>::new();
    groups_by_graph_index
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    groups_by_graph_index.resize_with(graph.groups.len(), || None);
    for (group_index, group) in graph.groups.iter().enumerate() {
        checkpoint_layout(execution, group_index)?;
        let bounds = resolved_bounds
            .get(group_index)
            .copied()
            .flatten()
            .ok_or_else(|| invalid_group_membership(graph))?;
        let width = raw_span(bounds.x, bounds.right, resources)?;
        let height = raw_span(bounds.y, bounds.bottom, resources)?;
        let title = match group.kind {
            GraphGroupKind::Container if !group.nodes.is_empty() => {
                // The perimeter includes the padding on both sides; recover the same title host
                // used by raw_group_bounds_for_members before painting its prepared label.
                let padding = resources.checked_grid_mul(policy.group_padding_x.max(1), 2)?;
                let title_width = resources
                    .checked_grid_add(width.saturating_sub(padding).saturating_sub(1), 3)?
                    .max(1);
                GraphLabel::try_wrapped_with_profile(
                    &group.title,
                    title_width,
                    policy.terminal_width_profile,
                    resources,
                )?
            }
            _ => empty_group_title(group, policy.terminal_width_profile, resources)?,
        };
        groups_by_graph_index[group_index] = Some(GroupLayout {
            id: group.id.clone(),
            kind: group.kind,
            title,
            style: group.style,
            divider_span: None,
            x: usize::try_from(bounds.x.max(0)).map_err(|_| grid_overflow(resources))?,
            y: usize::try_from(bounds.y.max(0)).map_err(|_| grid_overflow(resources))?,
            width,
            height,
        });
    }

    let mut groups = Vec::<GroupLayout>::new();
    groups
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (group_index, layout) in groups_by_graph_index.into_iter().enumerate() {
        checkpoint_layout(execution, group_index)?;
        groups.push(layout.ok_or_else(|| invalid_group_membership(graph))?);
    }

    assign_divider_spans(graph, topology, &mut groups);
    // Bounds require children before parents, while authored backgrounds require the inverse so a
    // containing group cannot erase a nested group's fill. Keep both orders explicit instead of
    // coupling paint behavior to declaration order.
    child_first_order.reverse();
    Ok(LaidOutGroups {
        items: groups,
        background_order: child_first_order,
    })
}

fn child_first_group_order(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<usize>> {
    let mut remaining_children = Vec::new();
    remaining_children
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    remaining_children.resize(graph.groups.len(), 0usize);

    for child_index in 0..graph.groups.len() {
        checkpoint_layout(execution, child_index)?;
        let Some(parent_index) = topology.parent_group_index(child_index) else {
            continue;
        };
        let count = remaining_children
            .get(parent_index)
            .copied()
            .ok_or_else(|| invalid_group_membership(graph))?;
        let count = resources.checked_work_add(count, 1)?;
        *remaining_children
            .get_mut(parent_index)
            .ok_or_else(|| invalid_group_membership(graph))? = count;
    }

    let mut ready = VecDeque::new();
    ready
        .try_reserve(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (group_index, remaining) in remaining_children.iter().copied().enumerate() {
        checkpoint_layout(execution, group_index)?;
        if remaining == 0 {
            ready.push_back(group_index);
        }
    }

    let mut order = Vec::new();
    order
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    while let Some(group_index) = ready.pop_front() {
        checkpoint_layout(execution, order.len())?;
        order.push(group_index);
        let Some(parent_index) = topology.parent_group_index(group_index) else {
            continue;
        };
        let remaining = remaining_children
            .get_mut(parent_index)
            .ok_or_else(|| invalid_group_membership(graph))?;
        *remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| invalid_group_membership(graph))?;
        if *remaining == 0 {
            ready.push_back(parent_index);
        }
    }

    if order.len() != graph.groups.len() {
        return Err(invalid_group_membership(graph));
    }
    Ok(order)
}

fn checkpoint_layout(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(OperationPhase::Layout, iteration)
}

fn invalid_group_membership(graph: &AsciiGraph) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: graph.diagram_type(),
        feature: "cyclic or multiply-owned compound graph membership",
    }
}

#[derive(Clone, Copy)]
struct ScopedPeer {
    level: usize,
    bounds: RawBounds,
}

/// Empty subtrees are placed against objects in their immediate scope. Their local envelope is
/// translated as a whole; nodes belonging to another scope cannot move an internal empty frame.
fn empty_subtree_origin(
    graph: &AsciiGraph,
    level: usize,
    policy: &GraphLayoutPolicy,
    leaf_group_levels: &[Option<usize>],
    peers: &[ScopedPeer],
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(isize, isize)> {
    resources.charge_layout_work(peers.len())?;
    let mut same_level_start = None::<isize>;
    let mut cross_end = None::<isize>;
    let mut previous = None::<(usize, isize)>;
    let mut next = None::<(usize, isize)>;
    for (peer_index, peer) in peers.iter().enumerate() {
        checkpoint_layout(execution, peer_index)?;
        let (root_start, root_end, peer_cross_end) = match graph.direction.canonical() {
            GraphDirection::LeftRight => (peer.bounds.x, peer.bounds.right, peer.bounds.bottom),
            GraphDirection::TopDown => (peer.bounds.y, peer.bounds.bottom, peer.bounds.right),
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        cross_end = Some(cross_end.map_or(peer_cross_end, |end| end.max(peer_cross_end)));
        include_rank_neighbor(
            peer.level,
            root_start,
            root_end,
            level,
            &mut same_level_start,
            &mut previous,
            &mut next,
        );
    }
    let root_start = if let Some(start) = same_level_start {
        start
    } else if let Some((previous_level, end)) = previous {
        let intermediate_span = leaf_group_rank_span(
            graph,
            leaf_group_levels,
            resources.checked_grid_add(previous_level, 1)?,
            level,
            policy,
            resources,
        )?;
        signed_add(
            end,
            resources.checked_grid_add(EMPTY_GROUP_RANK_GAP, intermediate_span)?,
            resources,
        )?
    } else if let Some((next_level, start)) = next {
        let occupied_span = leaf_group_rank_span(
            graph,
            leaf_group_levels,
            level,
            next_level,
            policy,
            resources,
        )?;
        start
            .checked_sub(signed(
                resources.checked_grid_add(EMPTY_GROUP_RANK_GAP, occupied_span)?,
                resources,
            )?)
            .ok_or_else(|| grid_overflow(resources))?
    } else {
        0
    };
    let cross_start = match cross_end {
        Some(end) => signed_add(end, 2, resources)?,
        None => 0,
    };
    Ok(match graph.direction.canonical() {
        GraphDirection::LeftRight => (root_start, cross_start),
        GraphDirection::TopDown => (cross_start, root_start),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    })
}

fn leaf_group_rank_span(
    graph: &AsciiGraph,
    leaf_group_levels: &[Option<usize>],
    range_start: usize,
    range_end: usize,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<usize> {
    if range_start >= range_end {
        return Ok(0);
    }
    resources.charge_layout_work(leaf_group_levels.len())?;
    let mut size_by_level = HashMap::<usize, usize>::default();
    size_by_level
        .try_reserve(leaf_group_levels.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (group_index, level) in leaf_group_levels.iter().copied().enumerate() {
        let (Some(level), Some(group)) = (level, graph.groups.get(group_index)) else {
            continue;
        };
        if level < range_start || level >= range_end {
            continue;
        }
        let (width, height) = empty_group_minimum_size(group, policy, resources)?;
        let root_size = match graph.direction.canonical() {
            GraphDirection::LeftRight => width,
            GraphDirection::TopDown => height,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        size_by_level
            .entry(level)
            .and_modify(|size| *size = (*size).max(root_size))
            .or_insert(root_size);
    }
    size_by_level.values().try_fold(0usize, |span, size| {
        resources.checked_grid_add(
            span,
            resources.checked_grid_add(*size, EMPTY_GROUP_RANK_GAP)?,
        )
    })
}

fn include_rank_neighbor(
    candidate_level: usize,
    root_start: isize,
    root_end: isize,
    target_level: usize,
    same_level_start: &mut Option<isize>,
    previous: &mut Option<(usize, isize)>,
    next: &mut Option<(usize, isize)>,
) {
    if candidate_level == target_level {
        *same_level_start =
            Some((*same_level_start).map_or(root_start, |start| start.min(root_start)));
    } else if candidate_level < target_level {
        if (*previous).is_none_or(|(level, _)| candidate_level >= level) {
            *previous = Some(match *previous {
                Some((level, end)) if level == candidate_level => (level, end.max(root_end)),
                _ => (candidate_level, root_end),
            });
        }
    } else if (*next).is_none_or(|(level, _)| candidate_level <= level) {
        *next = Some(match *next {
            Some((level, start)) if level == candidate_level => (level, start.min(root_start)),
            _ => (candidate_level, root_start),
        });
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RawBounds {
    pub(super) x: isize,
    pub(super) y: isize,
    pub(super) right: isize,
    pub(super) bottom: isize,
}

impl RawBounds {
    pub(super) fn translate(
        &mut self,
        dx: isize,
        dy: isize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.x = self
            .x
            .checked_add(dx)
            .ok_or_else(|| grid_overflow(resources))?;
        self.y = self
            .y
            .checked_add(dy)
            .ok_or_else(|| grid_overflow(resources))?;
        self.right = self
            .right
            .checked_add(dx)
            .ok_or_else(|| grid_overflow(resources))?;
        self.bottom = self
            .bottom
            .checked_add(dy)
            .ok_or_else(|| grid_overflow(resources))?;
        Ok(())
    }

    pub(super) fn include(&mut self, other: RawBounds) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }
}

pub(super) fn raw_group_bounds_for_members(
    group: &AsciiGraphGroup,
    member_bounds: RawBounds,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<RawBounds> {
    let x = member_bounds
        .x
        .checked_sub(
            isize::try_from(policy.group_padding_x.max(1)).map_err(|_| grid_overflow(resources))?,
        )
        .ok_or_else(|| grid_overflow(resources))?;
    let right = member_bounds
        .right
        .checked_add(
            isize::try_from(policy.group_padding_x.max(1)).map_err(|_| grid_overflow(resources))?,
        )
        .ok_or_else(|| grid_overflow(resources))?;

    match group.kind {
        GraphGroupKind::Container => {
            let title_width = member_bounds
                .right
                .checked_sub(member_bounds.x)
                .and_then(|width| width.checked_add(3))
                .and_then(|width| usize::try_from(width).ok())
                .ok_or_else(|| grid_overflow(resources))?
                .max(1);
            let title = GraphLabel::try_measure_wrapped_with_profile(
                &group.title,
                title_width,
                policy.terminal_width_profile,
                resources,
            )?;
            let title_space = title
                .content_height
                .checked_add(policy.group_title_clearance)
                .ok_or_else(|| grid_overflow(resources))?;
            let title_space = isize::try_from(title_space).map_err(|_| grid_overflow(resources))?;
            Ok(RawBounds {
                x,
                y: member_bounds
                    .y
                    .checked_sub(title_space)
                    .ok_or_else(|| grid_overflow(resources))?,
                right,
                bottom: member_bounds
                    .bottom
                    .checked_add(
                        isize::try_from(policy.group_padding_y.max(1))
                            .map_err(|_| grid_overflow(resources))?,
                    )
                    .ok_or_else(|| grid_overflow(resources))?,
            })
        }
        GraphGroupKind::Divider => Ok(RawBounds {
            x,
            y: member_bounds
                .y
                .checked_sub(
                    isize::try_from(policy.group_padding_y.min(1))
                        .map_err(|_| grid_overflow(resources))?,
                )
                .ok_or_else(|| grid_overflow(resources))?,
            right,
            bottom: member_bounds.bottom,
        }),
    }
}

pub(super) fn raw_group_bounds_batch(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<Option<RawBounds>>> {
    resources.transaction(|resources| {
        checkpoint_layout(execution, 0)?;
        resources.charge_layout_work(graph.groups.len())?;
        let mut member_count = 0usize;
        for (group_index, group) in graph.groups.iter().enumerate() {
            checkpoint_layout(execution, group_index)?;
            member_count = resources.checked_work_add(member_count, group.nodes.len())?;
        }
        let group_passes = resources.checked_work_mul(graph.groups.len(), 6)?;
        resources.charge_layout_work(resources.checked_work_add(
            resources.checked_work_add(layouts.len(), member_count)?,
            group_passes,
        )?)?;

        let mut layout_bounds_by_id = HashMap::default();
        layout_bounds_by_id
            .try_reserve(layouts.len())
            .map_err(|_| layout_work_allocation_failed())?;
        for (layout_index, layout) in layouts.iter().enumerate() {
            checkpoint_layout(execution, layout_index)?;
            let right = checked_node_right(layout, resources)?;
            let bottom = checked_node_bottom(layout, resources)?;
            layout_bounds_by_id
                .entry(layout.id.as_str())
                .or_insert(RawBounds {
                    x: isize::try_from(layout.x).map_err(|_| grid_overflow(resources))?,
                    y: isize::try_from(layout.y).map_err(|_| grid_overflow(resources))?,
                    right: isize::try_from(right).map_err(|_| grid_overflow(resources))?,
                    bottom: isize::try_from(bottom).map_err(|_| grid_overflow(resources))?,
                });
        }
        let order = child_first_group_order(graph, topology, resources, execution)?;
        let has_empty_group = graph.groups.iter().any(|group| group.nodes.is_empty());
        let leaf_group_levels = if has_empty_group {
            let mut rank_resources = resources.clone();
            Some(grid::rank_leaf_group_levels(
                graph,
                topology,
                &mut rank_resources,
                execution,
            )?)
        } else {
            None
        };
        let mut node_levels = HashMap::default();
        let mut group_levels = Vec::new();
        if has_empty_group {
            resources.charge_layout_work(
                resources.checked_work_add(layouts.len(), graph.groups.len())?,
            )?;
            node_levels
                .try_reserve(layouts.len())
                .map_err(|_| layout_work_allocation_failed())?;
            for (node_index, node) in layouts.iter().enumerate() {
                checkpoint_layout(execution, node_index)?;
                let level = match graph.direction.canonical() {
                    GraphDirection::LeftRight => node.grid.x,
                    GraphDirection::TopDown => node.grid.y,
                    GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
                };
                node_levels.insert(node.id.as_str(), level);
            }
            group_levels
                .try_reserve_exact(graph.groups.len())
                .map_err(|_| layout_work_allocation_failed())?;
            group_levels.resize(graph.groups.len(), None);
        }
        let scopes = if has_empty_group {
            Some(CompoundScopes::try_new(
                graph, layouts, topology, resources, execution,
            )?)
        } else {
            None
        };
        let mut completed = Vec::<Option<RawBounds>>::new();
        completed
            .try_reserve_exact(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;
        completed.resize(graph.groups.len(), None);
        let mut contains_nodes = Vec::new();
        contains_nodes
            .try_reserve_exact(if has_empty_group {
                graph.groups.len()
            } else {
                0
            })
            .map_err(|_| layout_work_allocation_failed())?;
        contains_nodes.resize(
            if has_empty_group {
                graph.groups.len()
            } else {
                0
            },
            false,
        );
        for (order_index, group_index) in order.into_iter().enumerate() {
            checkpoint_layout(execution, order_index)?;
            let group = graph
                .groups
                .get(group_index)
                .ok_or_else(|| invalid_group_membership(graph))?;
            let level = if let Some(scopes) = &scopes {
                resources.charge_layout_work(group.nodes.len())?;
                let mut level = leaf_group_levels
                    .as_ref()
                    .and_then(|levels| levels.get(group_index))
                    .copied()
                    .flatten();
                for (member_index, member) in group.nodes.iter().enumerate() {
                    checkpoint_layout(execution, member_index)?;
                    let member_level = node_levels.get(member.as_str()).copied().or_else(|| {
                        topology
                            .group_index(member)
                            .and_then(|index| group_levels.get(index))
                            .copied()
                            .flatten()
                    });
                    if let Some(member_level) = member_level {
                        level = Some(
                            level.map_or(member_level, |current: usize| current.min(member_level)),
                        );
                    }
                }
                scopes.place_empty_children(
                    graph,
                    group_index,
                    layouts,
                    &contains_nodes,
                    &group_levels,
                    leaf_group_levels.as_deref().unwrap_or(&[]),
                    policy,
                    &mut completed,
                    resources,
                    execution,
                )?;
                contains_nodes[group_index] = !scopes.nodes[group_index].is_empty()
                    || scopes.children[group_index]
                        .iter()
                        .any(|child| contains_nodes[*child]);
                level
            } else {
                None
            };
            let bounds = if group.nodes.is_empty() {
                let (width, height) = empty_group_minimum_size(group, policy, resources)?;
                Some(RawBounds {
                    x: 0,
                    y: 0,
                    right: signed(width.saturating_sub(1), resources)?,
                    bottom: signed(height.saturating_sub(1), resources)?,
                })
            } else {
                raw_group_bounds_from_completed_children(
                    group_index,
                    group,
                    &layout_bounds_by_id,
                    topology,
                    &completed,
                    policy,
                    resources,
                    execution,
                )?
            };
            completed[group_index] = bounds;
            if has_empty_group {
                group_levels[group_index] = level;
            }
        }
        if let Some(scopes) = &scopes {
            scopes.place_empty_children(
                graph,
                graph.groups.len(),
                layouts,
                &contains_nodes,
                &group_levels,
                leaf_group_levels.as_deref().unwrap_or(&[]),
                policy,
                &mut completed,
                resources,
                execution,
            )?;
        }
        Ok(completed)
    })
}

/// The final slot is the root scope. Each node and child subtree belongs to one scope, matching
/// topology's first-parent ownership. This lets empty frames use exactly the same sibling objects
/// as their containing frame without consulting foreign nodes or partially completed ancestors.
struct CompoundScopes {
    children: Vec<Vec<usize>>,
    nodes: Vec<Vec<usize>>,
}

impl CompoundScopes {
    fn try_new(
        graph: &AsciiGraph,
        layouts: &[NodeLayout],
        topology: &GraphGroupTopology<'_>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let count = resources.checked_grid_add(graph.groups.len(), 1)?;
        resources.charge_layout_work(resources.checked_work_add(
            resources.checked_work_mul(count, 2)?,
            resources.checked_work_add(graph.groups.len(), layouts.len())?,
        )?)?;
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        children
            .try_reserve_exact(count)
            .map_err(|_| layout_work_allocation_failed())?;
        nodes
            .try_reserve_exact(count)
            .map_err(|_| layout_work_allocation_failed())?;
        children.resize_with(count, Vec::new);
        nodes.resize_with(count, Vec::new);
        for child in 0..graph.groups.len() {
            checkpoint_layout(execution, child)?;
            let parent = topology
                .parent_group_index(child)
                .unwrap_or(graph.groups.len());
            children[parent]
                .try_reserve(1)
                .map_err(|_| layout_work_allocation_failed())?;
            children[parent].push(child);
        }
        for (index, node) in layouts.iter().enumerate() {
            checkpoint_layout(execution, index)?;
            let parent = topology
                .direct_node_group_index(&node.id)
                .unwrap_or(graph.groups.len());
            nodes[parent]
                .try_reserve(1)
                .map_err(|_| layout_work_allocation_failed())?;
            nodes[parent].push(index);
        }
        Ok(Self { children, nodes })
    }

    #[allow(clippy::too_many_arguments)]
    fn place_empty_children(
        &self,
        graph: &AsciiGraph,
        scope: usize,
        layouts: &[NodeLayout],
        contains_nodes: &[bool],
        levels: &[Option<usize>],
        leaf_levels: &[Option<usize>],
        policy: &GraphLayoutPolicy,
        completed: &mut [Option<RawBounds>],
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        let count =
            resources.checked_work_add(self.children[scope].len(), self.nodes[scope].len())?;
        resources.charge_layout_work(count)?;
        let mut peers = Vec::new();
        peers
            .try_reserve_exact(count)
            .map_err(|_| layout_work_allocation_failed())?;
        for (index, node_index) in self.nodes[scope].iter().copied().enumerate() {
            checkpoint_layout(execution, index)?;
            let node = &layouts[node_index];
            peers.push(ScopedPeer {
                level: match graph.direction.canonical() {
                    GraphDirection::LeftRight => node.grid.x,
                    GraphDirection::TopDown => node.grid.y,
                    GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
                },
                bounds: RawBounds {
                    x: signed(node.x, resources)?,
                    y: signed(node.y, resources)?,
                    right: signed(checked_node_right(node, resources)?, resources)?,
                    bottom: signed(checked_node_bottom(node, resources)?, resources)?,
                },
            });
        }
        for (index, child) in self.children[scope].iter().copied().enumerate() {
            checkpoint_layout(execution, index)?;
            if contains_nodes[child] {
                peers.push(ScopedPeer {
                    level: levels[child].unwrap_or(0),
                    bounds: completed[child].ok_or_else(|| invalid_group_membership(graph))?,
                });
            }
        }
        for (index, child) in self.children[scope].iter().copied().enumerate() {
            checkpoint_layout(execution, index)?;
            if contains_nodes[child] {
                continue;
            }
            let bounds = completed[child].ok_or_else(|| invalid_group_membership(graph))?;
            let level = levels[child].unwrap_or(0);
            let (x, y) = empty_subtree_origin(
                graph,
                level,
                policy,
                leaf_levels,
                &peers,
                resources,
                execution,
            )?;
            let dx = x
                .checked_sub(bounds.x)
                .ok_or_else(|| grid_overflow(resources))?;
            let dy = y
                .checked_sub(bounds.y)
                .ok_or_else(|| grid_overflow(resources))?;
            self.translate_empty_subtree(child, dx, dy, completed, resources, execution)?;
            peers.push(ScopedPeer {
                level,
                bounds: completed[child].ok_or_else(|| invalid_group_membership(graph))?,
            });
        }
        Ok(())
    }

    fn translate_empty_subtree(
        &self,
        child: usize,
        dx: isize,
        dy: isize,
        completed: &mut [Option<RawBounds>],
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        let mut stack = Vec::new();
        resources.charge_layout_work(1)?;
        stack
            .try_reserve(1)
            .map_err(|_| layout_work_allocation_failed())?;
        stack.push(child);
        let mut index = 0usize;
        while let Some(group) = stack.pop() {
            checkpoint_layout(execution, index)?;
            index = resources.checked_work_add(index, 1)?;
            resources
                .charge_layout_work(resources.checked_work_add(1, self.children[group].len())?)?;
            if let Some(bounds) = &mut completed[group] {
                bounds.translate(dx, dy, resources)?;
            }
            stack
                .try_reserve(self.children[group].len())
                .map_err(|_| layout_work_allocation_failed())?;
            stack.extend(self.children[group].iter().copied());
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn raw_group_bounds_from_completed_children(
    group_index: usize,
    group: &AsciiGraphGroup,
    layout_bounds_by_id: &HashMap<&str, RawBounds>,
    topology: &GraphGroupTopology<'_>,
    completed: &[Option<RawBounds>],
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RawBounds>> {
    let mut member_bounds = None::<RawBounds>;

    for (member_index, member) in group.nodes.iter().enumerate() {
        checkpoint_layout(execution, member_index)?;
        let bounds = if let Some(bounds) = layout_bounds_by_id.get(member.as_str()).copied() {
            Some(bounds)
        } else if let Some(child_index) = topology
            .group_index(member)
            .filter(|child_index| *child_index != group_index)
        {
            completed.get(child_index).copied().flatten()
        } else {
            None
        };

        let Some(bounds) = bounds else {
            continue;
        };
        if let Some(current) = &mut member_bounds {
            current.include(bounds);
        } else {
            member_bounds = Some(bounds);
        };
    }

    match member_bounds {
        Some(bounds) => Ok(Some(raw_group_bounds_for_members(
            group, bounds, policy, resources,
        )?)),
        None => Ok(None),
    }
}

fn assign_divider_spans(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    groups: &mut [GroupLayout],
) {
    for (group_index, graph_group) in graph.groups.iter().enumerate() {
        if graph_group.kind != GraphGroupKind::Divider {
            continue;
        }
        let span = topology
            .parent_group_index(group_index)
            .and_then(|parent_index| groups.get(parent_index))
            .and_then(divider_inner_span)
            .or_else(|| groups.get(group_index).and_then(divider_inner_span));
        if let Some(layout) = groups.get_mut(group_index) {
            layout.divider_span = span;
        }
    }
}

fn divider_inner_span(group: &GroupLayout) -> Option<DividerSpan> {
    let x_start = group.x.checked_add(1)?;
    let x_end = group
        .x
        .checked_add(group.width.checked_sub(1)?)?
        .checked_sub(1)?;
    (x_start <= x_end).then_some(DividerSpan { x_start, x_end })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::layout::GridCoord;
    use crate::graph::model::{GraphDirection, GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    fn node_layout(id: &str, x: usize, y: usize) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            label: GraphLabel::new(id),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x, y },
            x,
            y,
            width: 3,
            height: 3,
        }
    }

    #[test]
    fn raw_group_batch_preserves_nested_bounds_and_admits_exact_work_atomically() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["a".into()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".into(), "b".into()],
            GraphGroupStyle::default(),
        );
        let layouts = vec![node_layout("a", 4, 4), node_layout("b", 12, 12)];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources)
            .expect("group topology should build");
        let layout_policy = AsciiRenderOptions::default()
            .flowchart_layout()
            .graph_policy();
        let measured = ResourceContext::new(unbounded);
        let bounds = raw_group_bounds_batch(
            &graph,
            &layouts,
            &topology,
            &layout_policy,
            &measured,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("batch bounds should resolve");
        let inner = bounds[0].expect("inner frame should resolve");
        let outer = bounds[1].expect("outer frame should resolve");
        assert_eq!((inner.x, inner.y, inner.right, inner.bottom), (2, 0, 8, 8));
        assert_eq!(
            (outer.x, outer.y, outer.right, outer.bottom),
            (0, -4, 16, 16)
        );
        let exact_work = measured.layout_work_used();
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact limit should be valid");
        let exact = ResourceContext::new(exact_policy);
        raw_group_bounds_batch(
            &graph,
            &layouts,
            &topology,
            &layout_policy,
            &exact,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("exact batch admission should pass");
        assert_eq!(exact.layout_work_used(), exact_work);
        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one limit should be valid");
        let below = ResourceContext::new(below_policy);
        let error = raw_group_bounds_batch(
            &graph,
            &layouts,
            &topology,
            &layout_policy,
            &below,
            AsciiExecution::for_test(&below_policy),
        )
        .expect_err("max-minus-one should reject the complete batch");
        assert!(
            matches!(error, AsciiError::ResourceLimitExceeded(details) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
        );
        assert_eq!(below.layout_work_used(), 0);

        let control = merman_core::OperationControl::new();
        control.cancel();
        let cancelled = ResourceContext::new(exact_policy);
        let error = raw_group_bounds_batch(
            &graph,
            &layouts,
            &topology,
            &layout_policy,
            &cancelled,
            AsciiExecution::new(&control, &exact_policy),
        )
        .expect_err("cancellation must precede batch admission");
        assert!(matches!(error, AsciiError::Cancelled(_)));
        assert_eq!(cancelled.layout_work_used(), 0);
    }

    #[test]
    fn indexed_group_bounds_account_for_exact_work_and_reject_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["a".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string(), "b".to_string()],
            GraphGroupStyle::default(),
        );
        let layouts = vec![node_layout("a", 4, 4), node_layout("b", 12, 12)];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources)
            .expect("group topology should build");

        let mut measured_resources = ResourceContext::new(unbounded);
        layout_groups(
            &graph,
            &layouts,
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut measured_resources,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("unbounded indexed group-bound work should pass");
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 0);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let groups = layout_groups(
            &graph,
            &layouts,
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut exact_resources,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("exact indexed group-bound work should pass");
        assert_eq!(groups.items.len(), graph.groups.len());
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = layout_groups(
            &graph,
            &layouts,
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut below_resources,
            AsciiExecution::for_test(&below_policy),
        )
        .expect_err("max-minus-one indexed group-bound work should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn standalone_empty_group_receives_a_real_minimum_layout() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_group_with_style(
            "empty",
            "Empty",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("empty group topology should build");

        let groups = layout_groups(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .expect("empty group should receive a visible perimeter");

        assert_eq!(groups.items.len(), 1);
        assert_eq!(groups.items[0].id, "empty");
        assert!(groups.items[0].width >= "Empty".len() + 2);
        assert!(groups.items[0].height >= 4);
    }

    #[test]
    fn empty_group_layout_accepts_exact_work_and_rejects_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_group_with_style(
            "empty",
            "Empty",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources)
            .expect("empty group topology should build");

        let mut measured_resources = ResourceContext::new(unbounded);
        layout_groups(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut measured_resources,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("unbounded empty group layout should pass");
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 0);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        layout_groups(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut exact_resources,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("exact empty group layout-work budget should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = layout_groups(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut below_resources,
            AsciiExecution::for_test(&below_policy),
        )
        .expect_err("max-minus-one empty group layout-work budget should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn nested_empty_group_contributes_to_parent_bounds() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string()],
            GraphGroupStyle::default(),
        );
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("nested empty group topology should build");

        let groups = layout_groups(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::default()
                .flowchart_layout()
                .graph_policy(),
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .expect("nested empty groups should receive real bounds");
        let inner = groups
            .items
            .iter()
            .find(|group| group.id == "inner")
            .unwrap();
        let outer = groups
            .items
            .iter()
            .find(|group| group.id == "outer")
            .unwrap();

        assert!(outer.x <= inner.x);
        assert!(outer.y <= inner.y);
        assert!(outer.right() >= inner.right());
        assert!(outer.bottom() >= inner.bottom());
        assert_eq!(groups.background_order, vec![1, 0]);
    }
}
