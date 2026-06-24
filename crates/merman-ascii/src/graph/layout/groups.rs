use super::super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection, GraphEdgeArrow, GraphEdgeStroke,
    GraphEdgeStyle, GraphGroupKind, GraphNodeShape, GraphNodeStyle,
};
use super::{GridCoord, GroupLayout, NodeLayout};
use crate::options::AsciiRenderOptions;
use std::collections::{BTreeMap, HashMap, HashSet};

mod bounds;

use self::bounds::{RawBounds, raw_group_bounds_for_members};

pub(super) fn apply_group_placement_adjustments(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    apply_subgraph_direction_overrides(graph, placements);
    stack_divider_sections(graph, placements);
    separate_external_nodes_from_groups(graph, placements);
}

pub(super) fn subgraph_offsets(graph: &AsciiGraph, layouts: &[NodeLayout]) -> (usize, usize) {
    bounds::subgraph_offsets(graph, layouts)
}

pub(super) fn layout_groups(graph: &AsciiGraph, layouts: &[NodeLayout]) -> Vec<GroupLayout> {
    bounds::layout_groups(graph, layouts)
}

fn apply_subgraph_direction_overrides(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    for group_index in 0..graph.groups.len() {
        let Some(group) = graph.groups.get(group_index) else {
            continue;
        };
        let Some(direction) = group.direction else {
            continue;
        };
        let members = group_placement_members(graph, group_index);
        if members.len() < 2 {
            continue;
        }

        let override_graph = build_group_override_graph(graph, &members);
        let member_indices = (0..override_graph.nodes.len()).collect::<Vec<_>>();
        let root_indices = group_root_indices(&override_graph, &member_indices);
        if root_indices.is_empty() {
            continue;
        }

        let start_x = members
            .iter()
            .filter_map(|member| {
                member_origin(placements, &member.node_indices).map(|coord| coord.x)
            })
            .min()
            .unwrap_or(0);
        let start_y = members
            .iter()
            .filter_map(|member| {
                member_origin(placements, &member.node_indices).map(|coord| coord.y)
            })
            .min()
            .unwrap_or(0);

        let local = place_group_nodes(&override_graph, &member_indices, &root_indices, direction);
        for (member_index, coord) in local {
            let Some(member) = members.get(member_index) else {
                continue;
            };
            let Some(current_origin) = member_origin(placements, &member.node_indices) else {
                continue;
            };
            let target_origin = GridCoord {
                x: start_x + coord.x,
                y: start_y + coord.y,
            };
            let delta_x = target_origin.x as isize - current_origin.x as isize;
            let delta_y = target_origin.y as isize - current_origin.y as isize;
            shift_member_indices(placements, &member.node_indices, delta_x, delta_y);
        }

        let group_member_indices = group_member_indices(graph, group_index);
        if group_member_indices.len() < 2 {
            continue;
        }
        if let Some(bounds) =
            group_bounds_for_placements(graph, group_index, &group_member_indices, placements)
        {
            shift_external_nodes_away_from_group(graph, &group_member_indices, bounds, placements);
        }
    }
}

fn separate_external_nodes_from_groups(graph: &AsciiGraph, placements: &mut [GridCoord]) {
    if graph.groups.is_empty() || placements.is_empty() {
        return;
    }
    let endpoint_group_ids = graph_endpoint_group_ids(graph);
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
            let member_indices = group_member_indices(graph, group_index);
            if member_indices.is_empty() {
                continue;
            }
            let Some(group_bounds) =
                group_bounds_for_placements(graph, group_index, &member_indices, placements)
            else {
                continue;
            };
            changed |= shift_external_nodes_away_from_group(
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

        let sections: Vec<(Vec<usize>, RawBounds)> = child_dividers
            .into_iter()
            .filter_map(|child_index| {
                let member_indices = group_member_indices(graph, child_index);
                if member_indices.is_empty() {
                    return None;
                }
                let bounds = member_grid_bounds(&member_indices, placements)?;
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
            let Some(bounds) = member_grid_bounds(&member_indices, placements) else {
                continue;
            };
            let delta_x = anchor_left - bounds.x;
            if delta_x != 0 {
                shift_member_indices_x(placements, &member_indices, delta_x);
            }

            let Some(bounds) = member_grid_bounds(&member_indices, placements) else {
                continue;
            };

            if let Some(desired_top) = next_top {
                if bounds.y < desired_top {
                    shift_member_indices_y(
                        placements,
                        &member_indices,
                        (desired_top - bounds.y) as usize,
                    );
                }
            }

            let Some(updated_bounds) = member_grid_bounds(&member_indices, placements) else {
                continue;
            };
            next_top = Some(updated_bounds.bottom + 4);
        }
    }
}

pub(super) fn node_padding_y(
    graph: &AsciiGraph,
    placements: &[GridCoord],
    node_index: usize,
    options: &AsciiRenderOptions,
) -> usize {
    const SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD: usize = 4;

    let Some(node) = graph.nodes.get(node_index) else {
        return options.graph_padding_y;
    };
    let Some(group_index) = node_group_index(graph, &node.id) else {
        return options.graph_padding_y;
    };
    if !has_incoming_edge_from_outside_group(graph, &node.id, group_index) {
        return options.graph_padding_y;
    }

    let node_y = placements
        .get(node_index)
        .map(|coord| coord.y)
        .unwrap_or_default();
    let has_higher_external_entry = graph.groups[group_index].nodes.iter().any(|other_id| {
        if other_id == &node.id
            || !has_incoming_edge_from_outside_group(graph, other_id, group_index)
        {
            return false;
        }
        let Some(other_index) = graph.nodes.iter().position(|other| other.id == *other_id) else {
            return false;
        };
        placements
            .get(other_index)
            .is_some_and(|coord| coord.y < node_y)
    });
    if has_higher_external_entry {
        return options.graph_padding_y;
    }

    options.graph_padding_y + SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD
}

pub(super) fn node_group_index(graph: &AsciiGraph, node_id: &str) -> Option<usize> {
    graph
        .groups
        .iter()
        .position(|group| group.nodes.iter().any(|member| member == node_id))
}

fn has_incoming_edge_from_outside_group(
    graph: &AsciiGraph,
    node_id: &str,
    group_index: usize,
) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.to == node_id && node_group_index(graph, &edge.from) != Some(group_index))
}

fn graph_endpoint_group_ids(graph: &AsciiGraph) -> HashSet<&str> {
    let group_ids = graph
        .groups
        .iter()
        .map(|group| group.id.as_str())
        .collect::<HashSet<_>>();
    graph
        .edges
        .iter()
        .flat_map(|edge| [edge.from.as_str(), edge.to.as_str()])
        .filter(|endpoint| group_ids.contains(endpoint))
        .collect()
}

fn group_member_indices(graph: &AsciiGraph, group_index: usize) -> Vec<usize> {
    let group_index_by_id = graph
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let node_index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut indices = HashSet::new();
    let mut visited_groups = HashSet::new();
    let mut stack = vec![group_index];

    while let Some(index) = stack.pop() {
        if !visited_groups.insert(index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            continue;
        };

        for member in &group.nodes {
            if let Some(node_index) = node_index_by_id.get(member.as_str()).copied() {
                indices.insert(node_index);
            } else if let Some(child_group_index) = group_index_by_id.get(member.as_str()).copied()
            {
                stack.push(child_group_index);
            }
        }
    }

    let mut indices = indices.into_iter().collect::<Vec<_>>();
    indices.sort_unstable();
    indices
}

fn group_bounds_for_placements(
    graph: &AsciiGraph,
    group_index: usize,
    member_indices: &[usize],
    placements: &[GridCoord],
) -> Option<RawBounds> {
    let group = graph.groups.get(group_index)?;
    let mut member_bounds = None::<RawBounds>;

    for index in member_indices {
        let bounds = node_bounds(*placements.get(*index)?);
        if let Some(current) = &mut member_bounds {
            current.include(bounds);
        } else {
            member_bounds = Some(bounds);
        }
    }

    Some(raw_group_bounds_for_members(group, member_bounds?))
}

#[derive(Debug, Clone)]
struct GroupPlacementMember {
    id: String,
    node_indices: Vec<usize>,
}

fn group_placement_members(graph: &AsciiGraph, group_index: usize) -> Vec<GroupPlacementMember> {
    let Some(group) = graph.groups.get(group_index) else {
        return Vec::new();
    };
    let group_index_by_id = graph
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let node_index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();

    let mut members = Vec::new();
    for member in &group.nodes {
        if let Some(node_index) = node_index_by_id.get(member.as_str()).copied() {
            members.push(GroupPlacementMember {
                id: member.clone(),
                node_indices: vec![node_index],
            });
        } else if let Some(child_group_index) = group_index_by_id.get(member.as_str()).copied() {
            let node_indices = group_member_indices(graph, child_group_index);
            if node_indices.is_empty() {
                continue;
            }
            members.push(GroupPlacementMember {
                id: member.clone(),
                node_indices,
            });
        }
    }

    members
}

fn build_group_override_graph(graph: &AsciiGraph, members: &[GroupPlacementMember]) -> AsciiGraph {
    let mut override_graph = AsciiGraph::new_for_diagram(graph.diagram_type(), graph.direction);
    override_graph.root_policy = graph.root_policy;
    override_graph.nodes = members
        .iter()
        .map(|member| AsciiGraphNode {
            id: member.id.clone(),
            label: member.id.clone(),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
        })
        .collect();

    let mut node_to_member = HashMap::<&str, usize>::new();
    for (member_index, member) in members.iter().enumerate() {
        for node_index in &member.node_indices {
            let Some(node) = graph.nodes.get(*node_index) else {
                continue;
            };
            node_to_member
                .entry(node.id.as_str())
                .or_insert(member_index);
        }
    }

    let mut seen_edges = HashSet::<(usize, usize)>::new();
    for edge in &graph.edges {
        let Some(from_member_index) = node_to_member.get(edge.from.as_str()).copied() else {
            continue;
        };
        let Some(to_member_index) = node_to_member.get(edge.to.as_str()).copied() else {
            continue;
        };
        if from_member_index == to_member_index {
            continue;
        }
        if !seen_edges.insert((from_member_index, to_member_index)) {
            continue;
        }
        let from = override_graph.nodes[from_member_index].id.clone();
        let to = override_graph.nodes[to_member_index].id.clone();
        override_graph.edges.push(AsciiGraphEdge {
            from,
            to,
            label: None,
            stroke: GraphEdgeStroke::Normal,
            arrow: GraphEdgeArrow::Point,
            length: 1,
            style: GraphEdgeStyle::default(),
        });
    }

    override_graph
}

fn member_origin(placements: &[GridCoord], member_indices: &[usize]) -> Option<GridCoord> {
    let bounds = member_grid_bounds(member_indices, placements)?;
    Some(GridCoord {
        x: bounds.x.max(0) as usize,
        y: bounds.y.max(0) as usize,
    })
}

fn shift_member_indices(
    placements: &mut [GridCoord],
    member_indices: &[usize],
    delta_x: isize,
    delta_y: isize,
) {
    if delta_x == 0 && delta_y == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta_x.is_positive() {
                coord.x += delta_x as usize;
            } else {
                coord.x = coord.x.saturating_sub(delta_x.unsigned_abs());
            }
            if delta_y.is_positive() {
                coord.y += delta_y as usize;
            } else {
                coord.y = coord.y.saturating_sub(delta_y.unsigned_abs());
            }
        }
    }
}

fn member_grid_bounds(member_indices: &[usize], placements: &[GridCoord]) -> Option<RawBounds> {
    let mut bounds = None::<RawBounds>;

    for index in member_indices {
        let current = node_bounds(*placements.get(*index)?);
        if let Some(existing) = &mut bounds {
            existing.include(current);
        } else {
            bounds = Some(current);
        }
    }

    bounds
}

fn shift_member_indices_y(placements: &mut [GridCoord], member_indices: &[usize], delta: usize) {
    if delta == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            coord.y += delta;
        }
    }
}

fn shift_member_indices_x(placements: &mut [GridCoord], member_indices: &[usize], delta: isize) {
    if delta == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta.is_positive() {
                coord.x += delta as usize;
            } else {
                coord.x = coord.x.saturating_sub(delta.unsigned_abs());
            }
        }
    }
}

fn group_root_indices(graph: &AsciiGraph, member_indices: &[usize]) -> Vec<usize> {
    let member_ids = member_indices
        .iter()
        .filter_map(|index| graph.nodes.get(*index))
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();

    member_indices
        .iter()
        .copied()
        .filter(|index| {
            let Some(node) = graph.nodes.get(*index) else {
                return false;
            };
            !graph
                .edges
                .iter()
                .any(|edge| edge.to == node.id && member_ids.contains(edge.from.as_str()))
        })
        .collect()
}

fn place_group_nodes(
    graph: &AsciiGraph,
    member_indices: &[usize],
    root_indices: &[usize],
    direction: GraphDirection,
) -> HashMap<usize, GridCoord> {
    let member_ids = member_indices
        .iter()
        .filter_map(|index| graph.nodes.get(*index))
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = HashMap::new();
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();
    let mut visit_order = Vec::new();
    let mut cursor = 0usize;

    for root_index in root_indices {
        place_group_node(
            *root_index,
            0,
            direction,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
        visit_order.push(*root_index);
    }

    process_group_visit_order(
        graph,
        &member_ids,
        &index_by_id,
        direction,
        &mut placements,
        &mut occupied,
        &mut highest_position_per_level,
        &mut visit_order,
        &mut cursor,
    );

    let remaining_members = member_indices
        .iter()
        .copied()
        .filter(|index| !placements.contains_key(index))
        .collect::<Vec<_>>();
    if !remaining_members.is_empty() {
        for node_index in remaining_members {
            place_group_node(
                node_index,
                0,
                direction,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
            visit_order.push(node_index);
        }
        process_group_visit_order(
            graph,
            &member_ids,
            &index_by_id,
            direction,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
            &mut visit_order,
            &mut cursor,
        );
    }

    placements
}

fn process_group_visit_order<'a>(
    graph: &'a AsciiGraph,
    member_ids: &HashSet<&'a str>,
    index_by_id: &HashMap<&'a str, usize>,
    direction: GraphDirection,
    placements: &mut HashMap<usize, GridCoord>,
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
    visit_order: &mut Vec<usize>,
    cursor: &mut usize,
) {
    while *cursor < visit_order.len() {
        let node_index = visit_order[*cursor];
        *cursor += 1;

        let Some(parent_coord) = placements.get(&node_index).copied() else {
            continue;
        };
        let child_level = match direction {
            GraphDirection::LeftRight => parent_coord.x + 4,
            GraphDirection::TopDown => parent_coord.y + 4,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        for child_index in graph
            .edges
            .iter()
            .filter(|edge| {
                graph.nodes[node_index].id == edge.from && member_ids.contains(edge.to.as_str())
            })
            .filter_map(|edge| index_by_id.get(edge.to.as_str()).copied())
        {
            if placements.contains_key(&child_index) {
                continue;
            }
            place_group_node(
                child_index,
                child_level,
                direction,
                placements,
                occupied,
                highest_position_per_level,
            );
            visit_order.push(child_index);
        }
    }
}

fn shift_external_nodes_away_from_group(
    graph: &AsciiGraph,
    member_indices: &[usize],
    group_bounds: RawBounds,
    placements: &mut [GridCoord],
) -> bool {
    let member_indices = member_indices.iter().copied().collect::<HashSet<_>>();
    let graph_direction = graph.direction.canonical();
    let mut changed = false;

    for index in 0..placements.len() {
        if member_indices.contains(&index) {
            continue;
        }
        if !raw_bounds_intersects(group_bounds, node_bounds(placements[index])) {
            continue;
        }

        while raw_bounds_intersects(group_bounds, node_bounds(placements[index]))
            || node_overlaps_any_other(index, placements)
        {
            changed = true;
            match graph_direction {
                GraphDirection::LeftRight => placements[index].y += 4,
                GraphDirection::TopDown => placements[index].x += 4,
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }
    }

    changed
}

fn node_overlaps_any_other(index: usize, placements: &[GridCoord]) -> bool {
    let bounds = node_bounds(placements[index]);
    placements
        .iter()
        .enumerate()
        .any(|(other_index, other_coord)| {
            index != other_index && raw_bounds_intersects(bounds, node_bounds(*other_coord))
        })
}

fn node_bounds(coord: GridCoord) -> RawBounds {
    RawBounds {
        x: coord.x as isize,
        y: coord.y as isize,
        right: coord.x as isize + 2,
        bottom: coord.y as isize + 2,
    }
}

fn raw_bounds_intersects(left: RawBounds, right: RawBounds) -> bool {
    !(left.right < right.x
        || right.right < left.x
        || left.bottom < right.y
        || right.bottom < left.y)
}

fn place_group_node(
    node_index: usize,
    level: usize,
    direction: GraphDirection,
    placements: &mut HashMap<usize, GridCoord>,
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = match direction {
        GraphDirection::LeftRight => super::reserve_grid_spot(
            occupied,
            GridCoord {
                x: level,
                y: requested,
            },
            direction,
        ),
        GraphDirection::TopDown => super::reserve_grid_spot(
            occupied,
            GridCoord {
                x: requested,
                y: level,
            },
            direction,
        ),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    };
    placements.insert(node_index, coord);
    match direction {
        GraphDirection::LeftRight => {
            highest_position_per_level.insert(level, coord.y + 4);
        }
        GraphDirection::TopDown => {
            highest_position_per_level.insert(level, coord.x + 4);
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}
