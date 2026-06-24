use super::super::super::label::GraphLabel;
use super::super::super::model::{AsciiGraph, AsciiGraphGroup, GraphGroupKind};
use super::super::{DividerSpan, GroupLayout, NodeLayout};
use std::collections::{HashMap, HashSet};

pub(super) fn subgraph_offsets(graph: &AsciiGraph, layouts: &[NodeLayout]) -> (usize, usize) {
    let mut min_x = 0isize;
    let mut min_y = 0isize;

    for group_index in 0..graph.groups.len() {
        let Some(bounds) = raw_group_bounds(graph, layouts, group_index) else {
            continue;
        };
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
    }

    (
        min_x.checked_neg().unwrap_or_default() as usize,
        min_y.checked_neg().unwrap_or_default() as usize,
    )
}

pub(super) fn layout_groups(graph: &AsciiGraph, layouts: &[NodeLayout]) -> Vec<GroupLayout> {
    let mut groups = Vec::new();

    for group in &graph.groups {
        let node_members = layouts
            .iter()
            .filter(|layout| group.nodes.iter().any(|node| node == &layout.id))
            .collect::<Vec<_>>();
        let child_members = groups
            .iter()
            .filter(|layout: &&GroupLayout| group.nodes.iter().any(|node| node == &layout.id))
            .collect::<Vec<_>>();
        if node_members.is_empty() && child_members.is_empty() {
            continue;
        }

        let min_x = node_members
            .iter()
            .map(|layout| layout.x)
            .chain(child_members.iter().map(|layout| layout.x))
            .min()
            .unwrap_or(0);
        let min_y = node_members
            .iter()
            .map(|layout| layout.y)
            .chain(child_members.iter().map(|layout| layout.y))
            .min()
            .unwrap_or(0);
        let max_right = node_members
            .iter()
            .map(|layout| layout.right())
            .chain(child_members.iter().map(|layout| layout.right()))
            .max()
            .unwrap_or(0);
        let max_bottom = node_members
            .iter()
            .map(|layout| layout.bottom())
            .chain(child_members.iter().map(|layout| layout.bottom()))
            .max()
            .unwrap_or(0);
        let title = group_title_for_layout(group, min_x, max_right);
        let bounds =
            group_layout_bounds_for_members(group, &title, min_x, min_y, max_right, max_bottom);
        let width = bounds.right - bounds.x + 1;
        let height = bounds.bottom - bounds.y + 1;

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
    }

    assign_divider_spans(graph, &mut groups);
    groups
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
) -> RawBounds {
    let x = member_bounds.x - 2;
    let right = member_bounds.right + 2;

    match group.kind {
        GraphGroupKind::Container => {
            let title_width = (member_bounds.right - member_bounds.x + 3).max(1) as usize;
            let title = GraphLabel::wrapped(&group.title, title_width);
            let title_space = title.content_height() + 3;
            RawBounds {
                x,
                y: member_bounds.y - title_space as isize,
                right,
                bottom: member_bounds.bottom + 2,
            }
        }
        GraphGroupKind::Divider => RawBounds {
            x,
            y: member_bounds.y - 1,
            right,
            bottom: member_bounds.bottom,
        },
    }
}

fn raw_group_bounds(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    group_index: usize,
) -> Option<RawBounds> {
    graph.groups.get(group_index)?;

    let mut layout_bounds_by_id = HashMap::new();
    for layout in layouts {
        layout_bounds_by_id
            .entry(layout.id.as_str())
            .or_insert(RawBounds {
                x: layout.x as isize,
                y: layout.y as isize,
                right: layout.right() as isize,
                bottom: layout.bottom() as isize,
            });
    }
    let mut group_index_by_id = HashMap::new();
    for (index, group) in graph.groups.iter().enumerate() {
        group_index_by_id.entry(group.id.as_str()).or_insert(index);
    }
    let mut completed = HashMap::<usize, Option<RawBounds>>::new();
    let mut visiting = HashSet::<usize>::new();
    let mut stack = vec![(group_index, false)];

    while let Some((index, exiting)) = stack.pop() {
        if completed.contains_key(&index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            completed.insert(index, None);
            continue;
        };

        if exiting {
            visiting.remove(&index);
            completed.insert(
                index,
                raw_group_bounds_from_completed_children(
                    index,
                    group,
                    &layout_bounds_by_id,
                    &group_index_by_id,
                    &completed,
                ),
            );
            continue;
        }

        if !visiting.insert(index) {
            completed.insert(index, None);
            continue;
        }

        stack.push((index, true));
        for member in group.nodes.iter().rev() {
            if let Some(child_index) = group_index_by_id
                .get(member.as_str())
                .copied()
                .filter(|child_index| *child_index != index)
                && !completed.contains_key(&child_index)
                && !visiting.contains(&child_index)
            {
                stack.push((child_index, false));
            }
        }
    }

    completed.remove(&group_index).flatten()
}

fn raw_group_bounds_from_completed_children(
    group_index: usize,
    group: &AsciiGraphGroup,
    layout_bounds_by_id: &HashMap<&str, RawBounds>,
    group_index_by_id: &HashMap<&str, usize>,
    completed: &HashMap<usize, Option<RawBounds>>,
) -> Option<RawBounds> {
    let mut member_bounds = None::<RawBounds>;

    for member in &group.nodes {
        let bounds = if let Some(bounds) = layout_bounds_by_id.get(member.as_str()).copied() {
            Some(bounds)
        } else if let Some(child_index) = group_index_by_id
            .get(member.as_str())
            .copied()
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

    Some(raw_group_bounds_for_members(group, member_bounds?))
}

#[derive(Debug, Clone, Copy)]
struct GroupLayoutBounds {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

fn group_title_for_layout(group: &AsciiGraphGroup, min_x: usize, max_right: usize) -> GraphLabel {
    match group.kind {
        GraphGroupKind::Container => {
            GraphLabel::wrapped(&group.title, (max_right.saturating_sub(min_x) + 3).max(1))
        }
        GraphGroupKind::Divider => GraphLabel::new(""),
    }
}

fn group_layout_bounds_for_members(
    group: &AsciiGraphGroup,
    title: &GraphLabel,
    min_x: usize,
    min_y: usize,
    max_right: usize,
    max_bottom: usize,
) -> GroupLayoutBounds {
    let x = min_x.saturating_sub(2);
    let right = max_right.saturating_add(2);

    match group.kind {
        GraphGroupKind::Container => {
            let title_space = title.content_height() + 3;
            GroupLayoutBounds {
                x,
                y: min_y.saturating_sub(title_space),
                right,
                bottom: max_bottom.saturating_add(2),
            }
        }
        GraphGroupKind::Divider => GroupLayoutBounds {
            x,
            y: min_y.saturating_sub(1),
            right,
            bottom: max_bottom,
        },
    }
}

fn assign_divider_spans(graph: &AsciiGraph, groups: &mut [GroupLayout]) {
    for graph_group in graph
        .groups
        .iter()
        .filter(|group| group.kind == GraphGroupKind::Divider)
    {
        let Some(layout_index) = groups.iter().position(|layout| layout.id == graph_group.id)
        else {
            continue;
        };
        let span = divider_parent_span(graph, groups, &graph_group.id)
            .or_else(|| divider_inner_span(&groups[layout_index]));
        groups[layout_index].divider_span = span;
    }
}

fn divider_parent_span(
    graph: &AsciiGraph,
    groups: &[GroupLayout],
    divider_id: &str,
) -> Option<DividerSpan> {
    let parent = graph
        .groups
        .iter()
        .find(|group| group.nodes.iter().any(|member| member == divider_id))?;
    groups
        .iter()
        .find(|layout| layout.id == parent.id)
        .and_then(divider_inner_span)
}

fn divider_inner_span(group: &GroupLayout) -> Option<DividerSpan> {
    let x_start = group.x.saturating_add(1);
    let x_end = group.right().saturating_sub(1);
    (x_start <= x_end).then_some(DividerSpan { x_start, x_end })
}
