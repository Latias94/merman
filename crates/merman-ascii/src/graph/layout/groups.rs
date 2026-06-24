use super::super::model::{AsciiGraph, GraphGroupKind};
use super::{GridCoord, GroupLayout, NodeLayout};

pub(super) fn apply_group_placement_adjustments(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    apply_subgraph_direction_overrides(graph, placements);
    stack_divider_sections(graph, placements);
    separate_external_nodes_from_groups(graph, placements);
}

pub(super) fn subgraph_offsets(graph: &AsciiGraph, layouts: &[NodeLayout]) -> (usize, usize) {
    let mut min_x = 0isize;
    let mut min_y = 0isize;

    for group_index in 0..graph.groups.len() {
        let Some(bounds) = super::raw_group_bounds(graph, layouts, group_index) else {
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
        let title = super::group_title_for_layout(group, min_x, max_right);
        let bounds = super::group_layout_bounds_for_members(
            group, &title, min_x, min_y, max_right, max_bottom,
        );
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

    super::assign_divider_spans(graph, &mut groups);
    groups
}

fn apply_subgraph_direction_overrides(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    for group_index in 0..graph.groups.len() {
        let Some(group) = graph.groups.get(group_index) else {
            continue;
        };
        let Some(direction) = group.direction else {
            continue;
        };
        let members = super::group_placement_members(graph, group_index);
        if members.len() < 2 {
            continue;
        }

        let override_graph = super::build_group_override_graph(graph, &members);
        let member_indices = (0..override_graph.nodes.len()).collect::<Vec<_>>();
        let root_indices = super::group_root_indices(&override_graph, &member_indices);
        if root_indices.is_empty() {
            continue;
        }

        let start_x = members
            .iter()
            .filter_map(|member| {
                super::member_origin(placements, &member.node_indices).map(|coord| coord.x)
            })
            .min()
            .unwrap_or(0);
        let start_y = members
            .iter()
            .filter_map(|member| {
                super::member_origin(placements, &member.node_indices).map(|coord| coord.y)
            })
            .min()
            .unwrap_or(0);

        let local =
            super::place_group_nodes(&override_graph, &member_indices, &root_indices, direction);
        for (member_index, coord) in local {
            let Some(member) = members.get(member_index) else {
                continue;
            };
            let Some(current_origin) = super::member_origin(placements, &member.node_indices)
            else {
                continue;
            };
            let target_origin = GridCoord {
                x: start_x + coord.x,
                y: start_y + coord.y,
            };
            let delta_x = target_origin.x as isize - current_origin.x as isize;
            let delta_y = target_origin.y as isize - current_origin.y as isize;
            super::shift_member_indices(placements, &member.node_indices, delta_x, delta_y);
        }

        let group_member_indices = super::group_member_indices(graph, group_index);
        if group_member_indices.len() < 2 {
            continue;
        }
        if let Some(bounds) = super::group_bounds_for_placements(
            graph,
            group_index,
            &group_member_indices,
            placements,
        ) {
            super::shift_external_nodes_away_from_group(
                graph,
                &group_member_indices,
                bounds,
                placements,
            );
        }
    }
}

fn separate_external_nodes_from_groups(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    if graph.groups.is_empty() || placements.is_empty() {
        return;
    }
    let endpoint_group_ids = super::graph_endpoint_group_ids(graph);
    if endpoint_group_ids.is_empty() {
        return;
    }

    let max_passes = graph.groups.len().saturating_mul(placements.len()).max(1);
    for _ in 0..max_passes {
        let mut changed = false;
        for group_index in 0..graph.groups.len() {
            if !endpoint_group_ids.contains(graph.groups[group_index].id.as_str()) {
                continue;
            }
            let member_indices = super::group_member_indices(graph, group_index);
            if member_indices.is_empty() {
                continue;
            }
            let Some(group_bounds) =
                super::group_bounds_for_placements(graph, group_index, &member_indices, placements)
            else {
                continue;
            };
            changed |= super::shift_external_nodes_away_from_group(
                graph,
                &member_indices,
                group_bounds,
                placements,
            );
        }
        if !changed {
            break;
        }
    }
}

fn stack_divider_sections(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    if graph.groups.is_empty() || placements.is_empty() {
        return;
    }

    let divider_groups = graph
        .groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.kind == GraphGroupKind::Divider)
        .collect::<Vec<_>>();
    if divider_groups.len() < 2 {
        return;
    }

    for parent in &graph.groups {
        let child_dividers = divider_groups
            .iter()
            .copied()
            .filter(|(_, child)| parent.nodes.iter().any(|member| member == &child.id))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if child_dividers.len() < 2 {
            continue;
        }

        let sections: Vec<(Vec<usize>, super::RawBounds)> = child_dividers
            .into_iter()
            .filter_map(|child_index| {
                let member_indices = super::group_member_indices(graph, child_index);
                if member_indices.is_empty() {
                    return None;
                }
                let bounds = super::member_grid_bounds(&member_indices, placements)?;
                Some((member_indices, bounds))
            })
            .collect::<Vec<_>>();
        if sections.len() < 2 {
            continue;
        }

        let anchor_left = sections
            .iter()
            .map(|(_, bounds)| bounds.x)
            .min()
            .unwrap_or(0);
        let mut next_top: Option<isize> = None;
        for (member_indices, _) in sections {
            let Some(bounds) = super::member_grid_bounds(&member_indices, placements) else {
                continue;
            };
            let delta_x = anchor_left - bounds.x;
            if delta_x != 0 {
                super::shift_member_indices_x(placements, &member_indices, delta_x);
            }

            let Some(bounds) = super::member_grid_bounds(&member_indices, placements) else {
                continue;
            };

            if let Some(desired_top) = next_top {
                if bounds.y < desired_top {
                    super::shift_member_indices_y(
                        placements,
                        &member_indices,
                        (desired_top - bounds.y) as usize,
                    );
                }
            }

            let Some(updated_bounds) = super::member_grid_bounds(&member_indices, placements)
            else {
                continue;
            };
            next_top = Some(updated_bounds.bottom + 4);
        }
    }
}
