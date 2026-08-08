use super::super::super::label::GraphLabel;
use super::super::super::model::{AsciiGraph, AsciiGraphGroup, GraphGroupKind};
use super::super::super::topology::GraphGroupTopology;
use super::super::{DividerSpan, GroupLayout, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use std::collections::{HashMap, HashSet};

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

pub(super) fn subgraph_offsets(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    let mut min_x = 0isize;
    let mut min_y = 0isize;
    for group_index in 0..graph.groups.len() {
        let Some(bounds) = raw_group_bounds(
            graph,
            layouts,
            group_index,
            topology,
            width_profile,
            resources,
        )?
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
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<GroupLayout>> {
    // Charge one layout lookup, one member lookup, and two linear group passes up front.
    let member_count = graph.groups.iter().try_fold(0usize, |total, group| {
        resources.checked_work_add(total, group.nodes.len())
    })?;
    let group_visits = resources.checked_work_mul(graph.groups.len(), 2)?;
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
    for layout in layouts {
        let Some(node_index) = topology.node_index(&layout.id) else {
            continue;
        };
        if let Some(slot) = node_layout_by_index.get_mut(node_index) {
            if slot.is_none() {
                *slot = Some(layout);
            }
        }
    }

    let mut groups = Vec::<GroupLayout>::new();
    groups
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let mut group_layout_index_by_graph_index = Vec::<Option<usize>>::new();
    group_layout_index_by_graph_index
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    group_layout_index_by_graph_index.resize(graph.groups.len(), None);

    for (group_index, group) in graph.groups.iter().enumerate() {
        let mut member_bounds = None::<GroupLayoutBounds>;
        for member in &group.nodes {
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
                .and_then(|child_index| {
                    group_layout_index_by_graph_index
                        .get(child_index)
                        .and_then(|layout_index| *layout_index)
                })
                .and_then(|layout_index| groups.get(layout_index))
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
        let Some(member_bounds) = member_bounds else {
            continue;
        };
        let title = group_title_for_layout(
            group,
            member_bounds.x,
            member_bounds.right,
            width_profile,
            resources,
        )?;
        let bounds = group_layout_bounds_for_members(
            group,
            &title,
            member_bounds.x,
            member_bounds.y,
            member_bounds.right,
            member_bounds.bottom,
            resources,
        )?;
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

        let layout_index = groups.len();
        groups.push(GroupLayout {
            id: group.id.clone(),
            kind: group.kind,
            title,
            style: group.style,
            divider_span: None,
            x: bounds.x,
            y: bounds.y,
            width,
            height,
        });
        if let Some(slot) = group_layout_index_by_graph_index.get_mut(group_index) {
            *slot = Some(layout_index);
        }
    }

    assign_divider_spans(
        graph,
        topology,
        &group_layout_index_by_graph_index,
        &mut groups,
    );
    Ok(groups)
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
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RawBounds> {
    let x = member_bounds
        .x
        .checked_sub(2)
        .ok_or_else(|| grid_overflow(resources))?;
    let right = member_bounds
        .right
        .checked_add(2)
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
            let title = GraphLabel::wrapped_with_profile(&group.title, title_width, width_profile);
            let title_space = title
                .content_height()
                .checked_add(3)
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
                    .checked_add(2)
                    .ok_or_else(|| grid_overflow(resources))?,
            })
        }
        GraphGroupKind::Divider => Ok(RawBounds {
            x,
            y: member_bounds
                .y
                .checked_sub(1)
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
    width_profile: TerminalWidthProfile,
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
                width_profile,
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
    width_profile: TerminalWidthProfile,
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

    Ok(Some(raw_group_bounds_for_members(
        group,
        match member_bounds {
            Some(bounds) => bounds,
            None => return Ok(None),
        },
        width_profile,
        resources,
    )?))
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
            GraphLabel::wrapped_with_profile(
                &group.title,
                resources.checked_grid_add(member_width, 3)?.max(1),
                width_profile,
            )
        }
        GraphGroupKind::Divider => GraphLabel::new_with_profile("", width_profile),
    })
}

fn group_layout_bounds_for_members(
    group: &AsciiGraphGroup,
    title: &GraphLabel,
    min_x: usize,
    min_y: usize,
    max_right: usize,
    max_bottom: usize,
    resources: &ResourceContext,
) -> Result<GroupLayoutBounds> {
    let x = min_x.saturating_sub(2);
    let right = resources.checked_grid_add(max_right, 2)?;

    Ok(match group.kind {
        GraphGroupKind::Container => {
            let title_space = resources.checked_grid_add(title.content_height(), 3)?;
            GroupLayoutBounds {
                x,
                y: min_y.saturating_sub(title_space),
                right,
                bottom: resources.checked_grid_add(max_bottom, 2)?,
            }
        }
        GraphGroupKind::Divider => GroupLayoutBounds {
            x,
            y: min_y.saturating_sub(1),
            right,
            bottom: max_bottom,
        },
    })
}

fn assign_divider_spans(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_layout_index_by_graph_index: &[Option<usize>],
    groups: &mut [GroupLayout],
) {
    for (group_index, graph_group) in graph.groups.iter().enumerate() {
        if graph_group.kind != GraphGroupKind::Divider {
            continue;
        }
        let Some(layout_index) = group_layout_index_by_graph_index
            .get(group_index)
            .and_then(|layout_index| *layout_index)
        else {
            continue;
        };
        let span = topology
            .parent_group_index(group_index)
            .and_then(|parent_index| {
                group_layout_index_by_graph_index
                    .get(parent_index)
                    .and_then(|layout_index| *layout_index)
            })
            .and_then(|parent_layout_index| groups.get(parent_layout_index))
            .and_then(divider_inner_span)
            .or_else(|| groups.get(layout_index).and_then(divider_inner_span));
        if let Some(layout) = groups.get_mut(layout_index) {
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
    fn indexed_group_bounds_accept_exact_work_and_reject_max_minus_one() {
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

        let member_count = graph
            .groups
            .iter()
            .map(|group| group.nodes.len())
            .sum::<usize>();
        let exact_work = layouts.len() + member_count + graph.groups.len() * 2;
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let groups = layout_groups(
            &graph,
            &layouts,
            &topology,
            TerminalWidthProfile::Unicode,
            &mut exact_resources,
        )
        .expect("exact indexed group-bound work should pass");
        assert_eq!(groups.len(), graph.groups.len());
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = layout_groups(
            &graph,
            &layouts,
            &topology,
            TerminalWidthProfile::Unicode,
            &mut below_resources,
        )
        .expect_err("max-minus-one work should fail before layout-index allocation");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
        assert_eq!(below_resources.layout_work_used(), 0);
    }
}
