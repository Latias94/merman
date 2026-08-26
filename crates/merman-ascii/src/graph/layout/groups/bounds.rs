use super::super::super::label::{GraphLabel, GraphLabelMetrics};
use super::super::super::model::{AsciiGraph, AsciiGraphGroup, GraphDirection, GraphGroupKind};
use super::super::super::topology::GraphGroupTopology;
use super::super::grid;
use super::super::{DividerSpan, GroupLayout, NodeLayout};
use super::LaidOutGroups;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{FlowchartLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use merman_core::OperationPhase;
use std::collections::{HashMap, HashSet, VecDeque};

const EMPTY_GROUP_RANK_GAP: usize = 2;

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

fn checked_group_right(layout: &GroupLayout, resources: &ResourceContext) -> Result<usize> {
    let width = layout
        .width
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    resources.checked_grid_add(layout.x, width)
}

fn checked_group_bottom(layout: &GroupLayout, resources: &ResourceContext) -> Result<usize> {
    let height = layout
        .height
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    resources.checked_grid_add(layout.y, height)
}

fn include_group_layout_bounds(
    bounds: &mut Option<GroupLayoutBounds>,
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
) {
    if let Some(current) = bounds {
        current.x = current.x.min(x);
        current.y = current.y.min(y);
        current.right = current.right.max(right);
        current.bottom = current.bottom.max(bottom);
    } else {
        *bounds = Some(GroupLayoutBounds {
            x,
            y,
            right,
            bottom,
        });
    }
}

pub(super) fn empty_group_minimum_size(
    group: &AsciiGraphGroup,
    policy: &FlowchartLayoutPolicy,
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

pub(super) fn subgraph_offsets(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &FlowchartLayoutPolicy,
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    let mut min_x = 0isize;
    let mut min_y = 0isize;
    for group_index in 0..graph.groups.len() {
        let Some(bounds) =
            raw_group_bounds(graph, layouts, group_index, topology, policy, resources)?
        else {
            continue;
        };
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
    }

    Ok((
        usize::try_from(
            min_x
                .checked_neg()
                .ok_or_else(|| grid_overflow(resources))?,
        )
        .map_err(|_| grid_overflow(resources))?,
        usize::try_from(
            min_y
                .checked_neg()
                .ok_or_else(|| grid_overflow(resources))?,
        )
        .map_err(|_| grid_overflow(resources))?,
    ))
}

pub(super) fn layout_groups(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &FlowchartLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<LaidOutGroups> {
    // Charge indexed lookups plus the linear topology, layout, and output passes up front.
    let mut member_count = 0usize;
    let mut has_empty_group = false;
    for (group_index, group) in graph.groups.iter().enumerate() {
        checkpoint_layout(execution, group_index)?;
        member_count = resources.checked_work_add(member_count, group.nodes.len())?;
        has_empty_group |= group.nodes.is_empty();
    }
    let group_visits = resources.checked_work_mul(graph.groups.len(), 6)?;
    let layout_work = resources.checked_work_add(
        resources.checked_work_add(layouts.len(), member_count)?,
        group_visits,
    )?;
    resources.charge_layout_work(layout_work)?;

    let mut node_layout_by_index = Vec::<Option<&NodeLayout>>::new();
    node_layout_by_index
        .try_reserve_exact(graph.nodes.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    node_layout_by_index.resize(graph.nodes.len(), None);
    for (layout_index, layout) in layouts.iter().enumerate() {
        checkpoint_layout(execution, layout_index)?;
        let Some(node_index) = topology.node_index(&layout.id) else {
            continue;
        };
        if let Some(slot) = node_layout_by_index.get_mut(node_index)
            && slot.is_none()
        {
            *slot = Some(layout);
        }
    }

    let mut child_first_order = child_first_group_order(graph, topology, resources, execution)?;
    let mut groups_by_graph_index = Vec::<Option<GroupLayout>>::new();
    groups_by_graph_index
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    groups_by_graph_index.resize_with(graph.groups.len(), || None);

    let leaf_group_levels = if has_empty_group {
        Some(grid::rank_leaf_group_levels(
            graph, topology, resources, execution,
        )?)
    } else {
        None
    };

    for (order_index, group_index) in child_first_order.iter().copied().enumerate() {
        checkpoint_layout(execution, order_index)?;
        let group = graph
            .groups
            .get(group_index)
            .ok_or_else(|| invalid_group_membership(graph))?;
        let mut member_bounds = None::<GroupLayoutBounds>;
        for (member_index, member) in group.nodes.iter().enumerate() {
            checkpoint_layout(execution, member_index)?;
            if let Some(layout) = topology
                .node_index(member)
                .and_then(|node_index| node_layout_by_index.get(node_index))
                .and_then(|layout| *layout)
            {
                include_group_layout_bounds(
                    &mut member_bounds,
                    layout.x,
                    layout.y,
                    checked_node_right(layout, resources)?,
                    checked_node_bottom(layout, resources)?,
                );
                continue;
            }

            if let Some(layout) = topology
                .group_index(member)
                .filter(|child_index| *child_index != group_index)
                .and_then(|child_index| groups_by_graph_index.get(child_index))
                .and_then(Option::as_ref)
            {
                include_group_layout_bounds(
                    &mut member_bounds,
                    layout.x,
                    layout.y,
                    checked_group_right(layout, resources)?,
                    checked_group_bottom(layout, resources)?,
                );
            }
        }
        let (title, bounds) = if let Some(member_bounds) = member_bounds {
            let title_metrics = group_title_metrics_for_layout(
                group,
                member_bounds.x,
                member_bounds.right,
                policy.terminal_width_profile,
                resources,
            )?;
            let bounds = group_layout_bounds_for_members(
                group,
                title_metrics,
                member_bounds,
                policy,
                resources,
            )?;
            let title = group_title_for_layout(
                group,
                member_bounds.x,
                member_bounds.right,
                policy.terminal_width_profile,
                resources,
            )?;
            (title, bounds)
        } else {
            let title_metrics =
                empty_group_title_metrics(group, policy.terminal_width_profile, resources)?;
            let (width, height) = empty_group_minimum_size_for_metrics(
                group,
                title_metrics,
                policy.group_title_clearance,
                resources,
            )?;
            let (x, y) = empty_group_origin(
                graph,
                topology,
                graph.direction,
                group_index,
                width,
                policy,
                leaf_group_levels.as_deref(),
                layouts,
                &groups_by_graph_index,
                resources,
            )?;
            let title = empty_group_title(group, policy.terminal_width_profile, resources)?;
            (
                title,
                group_layout_bounds_from_size(x, y, width, height, resources)?,
            )
        };
        let width = resources.checked_grid_add(
            bounds
                .right
                .checked_sub(bounds.x)
                .ok_or_else(|| grid_overflow(resources))?,
            1,
        )?;
        let height = resources.checked_grid_add(
            bounds
                .bottom
                .checked_sub(bounds.y)
                .ok_or_else(|| grid_overflow(resources))?,
            1,
        )?;

        let layout = GroupLayout {
            id: group.id.clone(),
            kind: group.kind,
            title,
            style: group.style,
            divider_span: None,
            x: bounds.x,
            y: bounds.y,
            width,
            height,
        };
        let slot = groups_by_graph_index
            .get_mut(group_index)
            .ok_or_else(|| invalid_group_membership(graph))?;
        *slot = Some(layout);
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

#[allow(clippy::too_many_arguments)]
fn empty_group_origin(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    direction: GraphDirection,
    group_index: usize,
    width: usize,
    policy: &FlowchartLayoutPolicy,
    leaf_group_levels: Option<&[Option<usize>]>,
    node_layouts: &[NodeLayout],
    group_layouts: &[Option<GroupLayout>],
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    let Some(level) = leaf_group_levels
        .and_then(|levels| levels.get(group_index))
        .copied()
        .flatten()
    else {
        return Ok((0, 0));
    };

    let (ancestor_x_inset, ancestor_y_inset) =
        empty_group_ancestor_insets(graph, topology, group_index, width, policy, resources)?;
    let scan_work = resources.checked_work_add(node_layouts.len(), group_layouts.len())?;
    resources.charge_layout_work(scan_work)?;
    let mut same_level_start = None::<usize>;
    let mut same_level_cross_end = None::<usize>;
    let mut previous = None::<(usize, usize)>;
    let mut next = None::<(usize, usize)>;

    for layout in node_layouts {
        let (layout_level, root_start, root_end, cross_end) = match direction.canonical() {
            GraphDirection::LeftRight => (
                layout.grid.x,
                layout.x,
                checked_node_right(layout, resources)?,
                checked_node_bottom(layout, resources)?,
            ),
            GraphDirection::TopDown => (
                layout.grid.y,
                layout.y,
                checked_node_bottom(layout, resources)?,
                checked_node_right(layout, resources)?,
            ),
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        include_rank_neighbor(
            layout_level,
            root_start,
            root_end,
            cross_end,
            level,
            &mut same_level_start,
            &mut same_level_cross_end,
            &mut previous,
            &mut next,
        );
    }

    if let Some(levels) = leaf_group_levels {
        for (candidate_group_index, layout) in group_layouts.iter().enumerate() {
            let Some(candidate_level) = levels.get(candidate_group_index).copied().flatten() else {
                continue;
            };
            let Some(layout) = layout.as_ref() else {
                continue;
            };
            if candidate_level != level {
                continue;
            }
            let (root_start, root_end, cross_end) = match direction.canonical() {
                GraphDirection::LeftRight => (
                    layout.x,
                    checked_group_right(layout, resources)?,
                    checked_group_bottom(layout, resources)?,
                ),
                GraphDirection::TopDown => (
                    layout.y,
                    checked_group_bottom(layout, resources)?,
                    checked_group_right(layout, resources)?,
                ),
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            };
            include_rank_neighbor(
                candidate_level,
                root_start,
                root_end,
                cross_end,
                level,
                &mut same_level_start,
                &mut same_level_cross_end,
                &mut previous,
                &mut next,
            );
        }
    }

    let leaf_group_levels = leaf_group_levels.unwrap_or(&[]);
    let root_start = if let Some(start) = same_level_start {
        start
    } else if let Some((previous_level, end)) = previous {
        let intermediate_span = leaf_group_rank_span(
            graph,
            leaf_group_levels,
            direction,
            resources.checked_grid_add(previous_level, 1)?,
            level,
            policy,
            resources,
        )?;
        resources.checked_grid_add(
            resources.checked_grid_add(end, EMPTY_GROUP_RANK_GAP)?,
            intermediate_span,
        )?
    } else if let Some((next_level, start)) = next {
        let occupied_span = leaf_group_rank_span(
            graph,
            leaf_group_levels,
            direction,
            level,
            next_level,
            policy,
            resources,
        )?;
        start.saturating_sub(occupied_span)
    } else {
        let base = match direction.canonical() {
            GraphDirection::LeftRight => ancestor_x_inset,
            GraphDirection::TopDown => ancestor_y_inset,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        resources.checked_grid_add(
            base,
            leaf_group_rank_span(
                graph,
                leaf_group_levels,
                direction,
                0,
                level,
                policy,
                resources,
            )?,
        )?
    };
    let cross_start = same_level_cross_end
        .map(|end| resources.checked_grid_add(end, EMPTY_GROUP_RANK_GAP))
        .transpose()?
        .unwrap_or(match direction.canonical() {
            GraphDirection::LeftRight => ancestor_y_inset,
            GraphDirection::TopDown => ancestor_x_inset,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        });

    Ok(match direction.canonical() {
        GraphDirection::LeftRight => (root_start, cross_start),
        GraphDirection::TopDown => (cross_start, root_start),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    })
}

#[allow(clippy::too_many_arguments)]
fn leaf_group_rank_span(
    graph: &AsciiGraph,
    leaf_group_levels: &[Option<usize>],
    direction: GraphDirection,
    range_start: usize,
    range_end: usize,
    policy: &FlowchartLayoutPolicy,
    resources: &ResourceContext,
) -> Result<usize> {
    if range_start >= range_end {
        return Ok(0);
    }
    resources.charge_layout_work(leaf_group_levels.len())?;
    let mut size_by_level = HashMap::<usize, usize>::new();
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
        let root_size = match direction.canonical() {
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

fn empty_group_ancestor_insets(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    width: usize,
    policy: &FlowchartLayoutPolicy,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    let mut x_inset = 0usize;
    let mut y_inset = 0usize;
    let mut child_width = width;
    let mut parent = topology.parent_group_index(group_index);
    let mut visits = 0usize;
    while let Some(parent_index) = parent {
        resources.charge_layout_work(1)?;
        visits = resources.checked_work_add(visits, 1)?;
        if visits > graph.groups.len() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "cyclic or multiply-owned compound graph membership",
            });
        }
        let Some(parent_group) = graph.groups.get(parent_index) else {
            break;
        };
        x_inset = resources.checked_grid_add(x_inset, policy.group_padding_x)?;
        child_width =
            resources.checked_grid_add(child_width, policy.group_padding_x.saturating_mul(2))?;
        let top_inset = match parent_group.kind {
            GraphGroupKind::Container => {
                let title_width = child_width.saturating_sub(2).max(1);
                let title = GraphLabel::try_measure_wrapped_with_profile(
                    &parent_group.title,
                    title_width,
                    policy.terminal_width_profile,
                    resources,
                )?;
                resources.checked_grid_add(title.content_height, policy.group_title_clearance)?
            }
            GraphGroupKind::Divider => 1,
        };
        y_inset = resources.checked_grid_add(y_inset, top_inset)?;
        parent = topology.parent_group_index(parent_index);
    }
    Ok((x_inset, y_inset))
}

#[allow(clippy::too_many_arguments)]
fn include_rank_neighbor(
    candidate_level: usize,
    root_start: usize,
    root_end: usize,
    cross_end: usize,
    target_level: usize,
    same_level_start: &mut Option<usize>,
    same_level_cross_end: &mut Option<usize>,
    previous: &mut Option<(usize, usize)>,
    next: &mut Option<(usize, usize)>,
) {
    if candidate_level == target_level {
        *same_level_start =
            Some((*same_level_start).map_or(root_start, |start| start.min(root_start)));
        *same_level_cross_end =
            Some((*same_level_cross_end).map_or(cross_end, |end| end.max(cross_end)));
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

fn group_layout_bounds_from_size(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    resources: &ResourceContext,
) -> Result<GroupLayoutBounds> {
    Ok(GroupLayoutBounds {
        x,
        y,
        right: resources.checked_grid_add(
            x,
            width
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?,
        )?,
        bottom: resources.checked_grid_add(
            y,
            height
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?,
        )?,
    })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RawBounds {
    pub(super) x: isize,
    pub(super) y: isize,
    pub(super) right: isize,
    pub(super) bottom: isize,
}

impl RawBounds {
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
    policy: &FlowchartLayoutPolicy,
    resources: &ResourceContext,
) -> Result<RawBounds> {
    let x = member_bounds
        .x
        .checked_sub(isize::try_from(policy.group_padding_x).map_err(|_| grid_overflow(resources))?)
        .ok_or_else(|| grid_overflow(resources))?;
    let right = member_bounds
        .right
        .checked_add(isize::try_from(policy.group_padding_x).map_err(|_| grid_overflow(resources))?)
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
                        isize::try_from(policy.group_padding_y)
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

fn raw_group_bounds(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    group_index: usize,
    topology: &GraphGroupTopology<'_>,
    policy: &FlowchartLayoutPolicy,
    resources: &mut ResourceContext,
) -> Result<Option<RawBounds>> {
    if graph.groups.get(group_index).is_none() {
        return Ok(None);
    }

    let mut layout_bounds_by_id = HashMap::new();
    layout_bounds_by_id
        .try_reserve(layouts.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for layout in layouts {
        resources.charge_layout_work(1)?;
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
    let mut completed = HashMap::<usize, Option<RawBounds>>::new();
    completed
        .try_reserve(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let mut visiting = HashSet::<usize>::new();
    visiting
        .try_reserve(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let stack_capacity = graph.groups.len().checked_mul(2).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    let mut stack = Vec::new();
    stack
        .try_reserve(stack_capacity)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    resources.charge_layout_work(1)?;
    stack.push((group_index, false));

    while let Some((index, exiting)) = stack.pop() {
        resources.charge_layout_work(1)?;
        if completed.contains_key(&index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            completed.insert(index, None);
            continue;
        };

        if exiting {
            visiting.remove(&index);
            let bounds = raw_group_bounds_from_completed_children(
                index,
                group,
                &layout_bounds_by_id,
                topology,
                &completed,
                policy,
                resources,
            )?;
            completed.insert(index, bounds);
            continue;
        }

        if !visiting.insert(index) {
            completed.insert(index, None);
            continue;
        }

        resources.charge_layout_work(1)?;
        stack.push((index, true));
        for member in group.nodes.iter().rev() {
            resources.charge_layout_work(1)?;
            if let Some(child_index) = topology
                .group_index(member)
                .filter(|child_index| *child_index != index)
                && !completed.contains_key(&child_index)
                && !visiting.contains(&child_index)
            {
                resources.charge_layout_work(1)?;
                stack.push((child_index, false));
            }
        }
    }

    Ok(completed.remove(&group_index).flatten())
}

fn raw_group_bounds_from_completed_children(
    group_index: usize,
    group: &AsciiGraphGroup,
    layout_bounds_by_id: &HashMap<&str, RawBounds>,
    topology: &GraphGroupTopology<'_>,
    completed: &HashMap<usize, Option<RawBounds>>,
    policy: &FlowchartLayoutPolicy,
    resources: &mut ResourceContext,
) -> Result<Option<RawBounds>> {
    let mut member_bounds = None::<RawBounds>;

    for member in &group.nodes {
        resources.charge_layout_work(1)?;
        let bounds = if let Some(bounds) = layout_bounds_by_id.get(member.as_str()).copied() {
            Some(bounds)
        } else if let Some(child_index) = topology
            .group_index(member)
            .filter(|child_index| *child_index != group_index)
        {
            completed.get(&child_index).copied().flatten()
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
        None => Ok(Some(raw_empty_group_bounds(group, policy, resources)?)),
    }
}

fn raw_empty_group_bounds(
    group: &AsciiGraphGroup,
    policy: &FlowchartLayoutPolicy,
    resources: &ResourceContext,
) -> Result<RawBounds> {
    let (width, height) = empty_group_minimum_size(group, policy, resources)?;
    Ok(RawBounds {
        x: 0,
        y: 0,
        right: isize::try_from(
            width
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?,
        )
        .map_err(|_| grid_overflow(resources))?,
        bottom: isize::try_from(
            height
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?,
        )
        .map_err(|_| grid_overflow(resources))?,
    })
}

#[derive(Debug, Clone, Copy)]
struct GroupLayoutBounds {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

fn group_title_for_layout(
    group: &AsciiGraphGroup,
    min_x: usize,
    max_right: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<GraphLabel> {
    Ok(match group.kind {
        GraphGroupKind::Container => {
            let member_width = max_right
                .checked_sub(min_x)
                .ok_or_else(|| grid_overflow(resources))?;
            GraphLabel::try_wrapped_with_profile(
                &group.title,
                resources.checked_grid_add(member_width, 3)?.max(1),
                width_profile,
                resources,
            )?
        }
        GraphGroupKind::Divider => GraphLabel::empty_with_profile(width_profile),
    })
}

fn group_title_metrics_for_layout(
    group: &AsciiGraphGroup,
    min_x: usize,
    max_right: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<GraphLabelMetrics> {
    match group.kind {
        GraphGroupKind::Container => {
            let member_width = max_right
                .checked_sub(min_x)
                .ok_or_else(|| grid_overflow(resources))?;
            GraphLabel::try_measure_wrapped_with_profile(
                &group.title,
                resources.checked_grid_add(member_width, 3)?.max(1),
                width_profile,
                resources,
            )
        }
        GraphGroupKind::Divider => {
            GraphLabel::try_measure_with_profile("", width_profile, resources)
        }
    }
}

fn group_layout_bounds_for_members(
    group: &AsciiGraphGroup,
    title: GraphLabelMetrics,
    member_bounds: GroupLayoutBounds,
    policy: &FlowchartLayoutPolicy,
    resources: &ResourceContext,
) -> Result<GroupLayoutBounds> {
    let x = member_bounds.x.saturating_sub(policy.group_padding_x);
    let right = resources.checked_grid_add(member_bounds.right, policy.group_padding_x)?;

    Ok(match group.kind {
        GraphGroupKind::Container => {
            let title_space =
                resources.checked_grid_add(title.content_height, policy.group_title_clearance)?;
            GroupLayoutBounds {
                x,
                y: member_bounds.y.saturating_sub(title_space),
                right,
                bottom: resources.checked_grid_add(member_bounds.bottom, policy.group_padding_y)?,
            }
        }
        GraphGroupKind::Divider => GroupLayoutBounds {
            x,
            y: member_bounds
                .y
                .saturating_sub(policy.group_padding_y.min(1)),
            right,
            bottom: member_bounds.bottom,
        },
    })
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
    use crate::resource::AsciiResourcePolicy;
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
            &AsciiRenderOptions::default().flowchart_layout(),
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
