use super::super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphGroup, AsciiGraphNode, GraphDirection, GraphEdgeMarker,
    GraphEdgeStroke, GraphEdgeStyle, GraphGroupKind, GraphNodeShape, GraphNodeStyle,
};
use super::super::topology::{GraphEndpointIndex, GraphGroupTopology};
use super::{GridCoord, GroupLayout, NodeLayout};
use crate::error::Result;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::{HashMap, HashSet};

mod bounds;
mod direction;
mod placement;
mod side_constraints;

use self::bounds::{RawBounds, raw_group_bounds_for_members};
use self::side_constraints::{include_side_constrained_group_followers, override_member_semantics};

pub(super) fn apply_group_placement_adjustments(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<()> {
    placement::apply_group_placement_adjustments(
        graph,
        placements,
        topology,
        width_profile,
        resources,
    )
}

pub(super) fn subgraph_offsets(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    bounds::subgraph_offsets(graph, layouts, topology, width_profile, resources)
}

pub(super) fn layout_groups(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<GroupLayout>> {
    bounds::layout_groups(graph, layouts, topology, width_profile, resources)
}

pub(super) fn empty_group_minimum_size(
    group: &AsciiGraphGroup,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    bounds::empty_group_minimum_size(group, width_profile, resources)
}

fn apply_subgraph_direction_overrides(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    direction_overrides: &[Option<GraphDirection>],
    disabled_overrides: &[bool],
    resources: &mut ResourceContext,
) -> Result<()> {
    for group_index in 0..graph.groups.len() {
        if disabled_overrides
            .get(group_index)
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        let Some(group) = graph.groups.get(group_index) else {
            continue;
        };
        let Some(direction) = direction_overrides.get(group_index).copied().flatten() else {
            continue;
        };
        resources.charge_layout_work(group.nodes.len())?;
        let members = group_placement_members(graph, topology, group_index, resources)?;
        if members.len() < 2 {
            continue;
        }

        let layout_direction = direction.before_root_output_transform(graph.direction);
        let override_graph =
            build_group_override_graph(graph, topology, &members, layout_direction, resources)?;

        let mut start_x = None::<usize>;
        let mut start_y = None::<usize>;
        for member in &members {
            let Some(origin) = member_origin(placements, &member.node_indices, resources)? else {
                continue;
            };
            start_x = Some(start_x.map_or(origin.x, |current| current.min(origin.x)));
            start_y = Some(start_y.map_or(origin.y, |current| current.min(origin.y)));
        }
        let start_x = start_x.unwrap_or_default();
        let start_y = start_y.unwrap_or_default();

        let mut local = place_group_nodes(&override_graph, layout_direction, resources)?;
        mirror_local_placements(&mut local, layout_direction, resources)?;
        for (member_index, coord) in local {
            let Some(member) = members.get(member_index) else {
                continue;
            };
            let Some(current_origin) = member_origin(placements, &member.node_indices, resources)?
            else {
                continue;
            };
            let target_origin = GridCoord {
                x: resources.checked_grid_add(start_x, coord.x)?,
                y: resources.checked_grid_add(start_y, coord.y)?,
            };
            let delta_x = checked_signed_delta(target_origin.x, current_origin.x, resources)?;
            let delta_y = checked_signed_delta(target_origin.y, current_origin.y, resources)?;
            shift_member_indices(
                placements,
                &member.node_indices,
                delta_x,
                delta_y,
                resources,
            )?;
        }

        resources.charge_layout_work(graph.nodes.len())?;
        let group_member_indices = group_member_indices(topology, group_index, resources)?;
        if group_member_indices.len() < 2 {
            continue;
        }
        if let Some(bounds) = group_bounds_for_placements(
            graph,
            group_index,
            &group_member_indices,
            placements,
            width_profile,
            resources,
        )? {
            shift_external_nodes_away_from_group(
                graph,
                &group_member_indices,
                bounds,
                placements,
                resources,
            )?;
        }
    }
    Ok(())
}

fn separate_external_nodes_from_groups(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.groups.is_empty() || placements.is_empty() {
        return Ok(());
    }
    let endpoint_group_ids = graph_endpoint_group_ids(graph, resources)?;
    if endpoint_group_ids.is_empty() {
        return Ok(());
    }

    let max_passes = graph
        .groups
        .len()
        .checked_mul(placements.len())
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?
        .max(1);
    for _ in 0..max_passes {
        resources.charge_layout_work(1)?;
        let mut changed = false;
        for group_index in 0..graph.groups.len() {
            resources.charge_layout_work(1)?;
            if !endpoint_group_ids.contains(graph.groups[group_index].id.as_str()) {
                continue;
            }
            resources.charge_layout_work(graph.nodes.len())?;
            let member_indices = group_member_indices(topology, group_index, resources)?;
            if member_indices.is_empty() {
                continue;
            }
            let Some(group_bounds) = group_bounds_for_placements(
                graph,
                group_index,
                &member_indices,
                placements,
                width_profile,
                resources,
            )?
            else {
                continue;
            };
            changed |= shift_external_nodes_away_from_group(
                graph,
                &member_indices,
                group_bounds,
                placements,
                resources,
            )?;
        }
        if !changed {
            break;
        }
    }
    Ok(())
}

fn stack_divider_sections(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.groups.is_empty() || placements.is_empty() {
        return Ok(());
    }

    let divider_group_count = graph
        .groups
        .iter()
        .filter(|group| group.kind == GraphGroupKind::Divider)
        .count();
    if divider_group_count < 2 {
        return Ok(());
    }

    let index_work = resources.checked_work_add(graph.groups.len(), divider_group_count)?;
    resources.charge_layout_work(index_work)?;
    let mut child_dividers_by_parent = Vec::<Vec<usize>>::new();
    child_dividers_by_parent
        .try_reserve(graph.groups.len())
        .map_err(|_| {
            crate::error::AsciiError::allocation_failed(
                crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
            )
        })?;
    child_dividers_by_parent.resize_with(graph.groups.len(), Vec::new);
    for (child_index, _) in graph
        .groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.kind == GraphGroupKind::Divider)
    {
        let Some(parent_index) = topology.parent_group_index(child_index) else {
            continue;
        };
        let Some(children) = child_dividers_by_parent.get_mut(parent_index) else {
            continue;
        };
        children.try_reserve(1).map_err(|_| {
            crate::error::AsciiError::allocation_failed(
                crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
            )
        })?;
        children.push(child_index);
    }

    for child_dividers in child_dividers_by_parent {
        if child_dividers.len() < 2 {
            continue;
        }

        let mut sections = Vec::new();
        sections.try_reserve(child_dividers.len()).map_err(|_| {
            crate::error::AsciiError::AllocationFailed {
                phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
            }
        })?;
        for child_index in child_dividers {
            resources.charge_layout_work(graph.nodes.len())?;
            let member_indices = group_member_indices(topology, child_index, resources)?;
            if member_indices.is_empty() {
                continue;
            }
            let Some(bounds) = member_grid_bounds(&member_indices, placements, resources)? else {
                continue;
            };
            sections.push((member_indices, bounds));
        }
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
            let Some(bounds) = member_grid_bounds(&member_indices, placements, resources)? else {
                continue;
            };
            let delta_x = anchor_left.checked_sub(bounds.x).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxGridCells)
            })?;
            if delta_x != 0 {
                shift_member_indices_x(placements, &member_indices, delta_x, resources)?;
            }

            let Some(bounds) = member_grid_bounds(&member_indices, placements, resources)? else {
                continue;
            };

            if let Some(desired_top) = next_top
                && bounds.y < desired_top
            {
                shift_member_indices_y(
                    placements,
                    &member_indices,
                    usize::try_from(desired_top.checked_sub(bounds.y).ok_or_else(|| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })?)
                    .map_err(|_| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })?,
                    resources,
                )?;
            }

            let Some(updated_bounds) = member_grid_bounds(&member_indices, placements, resources)?
            else {
                continue;
            };
            next_top = Some(updated_bounds.bottom.checked_add(4).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxGridCells)
            })?);
        }
    }
    Ok(())
}

pub(super) struct NodePaddingIndex {
    has_external_incoming_overhead: Vec<bool>,
}

impl NodePaddingIndex {
    pub(super) fn try_new(
        graph: &AsciiGraph,
        placements: &[GridCoord],
        topology: Option<&GraphGroupTopology<'_>>,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let Some(topology) = topology else {
            return Ok(Self {
                has_external_incoming_overhead: Vec::new(),
            });
        };

        let node_passes = resources.checked_work_mul(graph.nodes.len(), 2)?;
        let work = resources.checked_work_add(
            resources.checked_work_add(node_passes, graph.groups.len())?,
            graph.edges.len(),
        )?;
        resources.charge_layout_work(work)?;

        let mut has_external_incoming = try_bool_slots(graph.nodes.len(), resources)?;
        for edge in &graph.edges {
            let Some(to_index) = topology.node_index(&edge.to) else {
                continue;
            };
            let Some(group_index) = topology.direct_node_group_index(&edge.to) else {
                continue;
            };
            if topology.direct_node_group_index(&edge.from) != Some(group_index) {
                has_external_incoming[to_index] = true;
            }
        }

        let mut minimum_entry_y_by_group = Vec::new();
        minimum_entry_y_by_group
            .try_reserve(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;
        minimum_entry_y_by_group.resize(graph.groups.len(), None::<usize>);
        for (node_index, node) in graph.nodes.iter().enumerate() {
            if !has_external_incoming[node_index] {
                continue;
            }
            let Some(group_index) = topology.direct_node_group_index(&node.id) else {
                continue;
            };
            let Some(y) = placements.get(node_index).map(|coord| coord.y) else {
                continue;
            };
            let Some(minimum_y) = minimum_entry_y_by_group.get_mut(group_index) else {
                continue;
            };
            *minimum_y = Some(minimum_y.map_or(y, |current| current.min(y)));
        }

        let mut has_external_incoming_overhead = try_bool_slots(graph.nodes.len(), resources)?;
        for (node_index, node) in graph.nodes.iter().enumerate() {
            if !has_external_incoming[node_index] {
                continue;
            }
            let Some(group_index) = topology.direct_node_group_index(&node.id) else {
                continue;
            };
            let Some(y) = placements.get(node_index).map(|coord| coord.y) else {
                continue;
            };
            has_external_incoming_overhead[node_index] = minimum_entry_y_by_group
                .get(group_index)
                .and_then(|minimum_y| *minimum_y)
                == Some(y);
        }

        Ok(Self {
            has_external_incoming_overhead,
        })
    }
}

fn try_bool_slots(len: usize, resources: &ResourceContext) -> Result<Vec<bool>> {
    resources.charge_layout_work(len)?;
    let mut slots = Vec::new();
    slots
        .try_reserve(len)
        .map_err(|_| layout_work_allocation_failed())?;
    slots.resize(len, false);
    Ok(slots)
}

fn layout_work_allocation_failed() -> crate::error::AsciiError {
    crate::error::AsciiError::allocation_failed(
        crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    )
}

pub(super) fn node_padding_y(
    node_index: usize,
    index: &NodePaddingIndex,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<usize> {
    const SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD: usize = 4;

    if !index
        .has_external_incoming_overhead
        .get(node_index)
        .copied()
        .unwrap_or(false)
    {
        return Ok(options.graph_padding_y);
    }

    resources.checked_grid_add(options.graph_padding_y, SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD)
}

fn graph_endpoint_group_ids<'a>(
    graph: &'a AsciiGraph,
    resources: &ResourceContext,
) -> Result<HashSet<&'a str>> {
    let endpoint_count = resources.checked_work_mul(graph.edges.len(), 2)?;
    resources
        .charge_layout_work(resources.checked_work_add(graph.groups.len(), endpoint_count)?)?;

    let mut group_ids = HashSet::new();
    group_ids
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    group_ids.extend(graph.groups.iter().map(|group| group.id.as_str()));

    let mut endpoint_group_ids = HashSet::new();
    endpoint_group_ids
        .try_reserve(graph.groups.len().min(endpoint_count))
        .map_err(|_| layout_work_allocation_failed())?;
    for edge in &graph.edges {
        for endpoint in [edge.from.as_str(), edge.to.as_str()] {
            if group_ids.contains(endpoint) {
                endpoint_group_ids.insert(endpoint);
            }
        }
    }
    Ok(endpoint_group_ids)
}

fn group_member_indices(
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    topology.group_member_node_indices(group_index, resources)
}

fn group_bounds_for_placements(
    graph: &AsciiGraph,
    group_index: usize,
    member_indices: &[usize],
    placements: &[GridCoord],
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<RawBounds>> {
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(None);
    };
    let mut member_bounds = None::<RawBounds>;

    for index in member_indices {
        let Some(coord) = placements.get(*index).copied() else {
            return Ok(None);
        };
        let bounds = node_bounds(coord, resources)?;
        if let Some(current) = &mut member_bounds {
            current.include(bounds);
        } else {
            member_bounds = Some(bounds);
        }
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

#[derive(Debug, Clone)]
struct GroupPlacementMember {
    id: String,
    endpoint: GraphEndpointIndex,
    node_indices: Vec<usize>,
}

fn group_placement_members(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<GroupPlacementMember>> {
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(Vec::new());
    };

    resources.charge_layout_work(group.nodes.len())?;
    let mut members = Vec::new();
    members.try_reserve(group.nodes.len()).map_err(|_| {
        crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        }
    })?;
    for member in &group.nodes {
        resources.charge_layout_work(1)?;
        match topology.endpoint_index(member) {
            Some(GraphEndpointIndex::Node(node_index)) => {
                members.push(GroupPlacementMember {
                    id: crate::safe_text::try_clone_layout_text(member, resources)?,
                    endpoint: GraphEndpointIndex::Node(node_index),
                    node_indices: vec![node_index],
                });
            }
            Some(GraphEndpointIndex::Group(child_group_index)) => {
                let node_indices =
                    topology.group_member_node_indices(child_group_index, resources)?;
                if node_indices.is_empty() {
                    continue;
                }
                members.push(GroupPlacementMember {
                    id: crate::safe_text::try_clone_layout_text(member, resources)?,
                    endpoint: GraphEndpointIndex::Group(child_group_index),
                    node_indices,
                });
            }
            None => {}
        }
    }

    include_side_constrained_group_followers(
        graph,
        topology,
        group_index,
        &mut members,
        resources,
    )?;

    Ok(members)
}

fn build_group_override_graph(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    members: &[GroupPlacementMember],
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<AsciiGraph> {
    let mut override_graph = AsciiGraph::new_for_diagram(graph.diagram_type(), direction);
    override_graph.root_policy = graph.root_policy;

    let mut endpoint_to_member = HashMap::<GraphEndpointIndex, usize>::new();
    resources.charge_layout_work(members.len())?;
    let member_node_count = members.iter().try_fold(0usize, |total, member| {
        resources.checked_work_add(total, member.node_indices.len())
    })?;
    let endpoint_capacity = resources.checked_work_add(members.len(), member_node_count)?;
    resources.charge_layout_work(endpoint_capacity)?;
    endpoint_to_member
        .try_reserve(endpoint_capacity)
        .map_err(|_| layout_work_allocation_failed())?;
    for (member_index, member) in members.iter().enumerate() {
        endpoint_to_member.insert(member.endpoint, member_index);
        for node_index in &member.node_indices {
            endpoint_to_member
                .entry(GraphEndpointIndex::Node(*node_index))
                .or_insert(member_index);
        }
    }

    override_graph
        .nodes
        .try_reserve(members.len())
        .map_err(|_| crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for member in members {
        resources.charge_layout_work(1)?;
        let semantics = override_member_semantics(
            graph,
            topology,
            members,
            &endpoint_to_member,
            member,
            resources,
        )?;
        override_graph.nodes.push(AsciiGraphNode {
            id: member.id.clone(),
            label: member.id.clone(),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            semantics,
        });
    }

    override_graph
        .edges
        .try_reserve(graph.edges.len())
        .map_err(|_| crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for edge in &graph.edges {
        resources.charge_layout_work(1)?;
        let Some(from_endpoint) = topology.endpoint_index(&edge.from) else {
            continue;
        };
        let Some(to_endpoint) = topology.endpoint_index(&edge.to) else {
            continue;
        };
        let Some(from_member_index) = endpoint_to_member.get(&from_endpoint).copied() else {
            continue;
        };
        let Some(to_member_index) = endpoint_to_member.get(&to_endpoint).copied() else {
            continue;
        };
        if from_member_index == to_member_index {
            continue;
        }
        let from = override_graph.nodes[from_member_index].id.clone();
        let to = override_graph.nodes[to_member_index].id.clone();
        override_graph.edges.push(AsciiGraphEdge {
            id: edge.id.clone(),
            is_user_defined_id: edge.is_user_defined_id,
            from,
            to,
            label: None,
            stroke: GraphEdgeStroke::Normal,
            start_marker: GraphEdgeMarker::Open,
            end_marker: GraphEdgeMarker::Point,
            length: edge.length,
            style: GraphEdgeStyle::default(),
        });
    }

    Ok(override_graph)
}

fn member_origin(
    placements: &[GridCoord],
    member_indices: &[usize],
    resources: &ResourceContext,
) -> Result<Option<GridCoord>> {
    let Some(bounds) = member_grid_bounds(member_indices, placements, resources)? else {
        return Ok(None);
    };
    Ok(Some(GridCoord {
        x: usize::try_from(bounds.x.max(0)).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
        y: usize::try_from(bounds.y.max(0)).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
    }))
}

fn checked_signed_delta(
    target: usize,
    current: usize,
    resources: &ResourceContext,
) -> Result<isize> {
    let target = isize::try_from(target).map_err(|_| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxGridCells)
    })?;
    let current = isize::try_from(current).map_err(|_| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxGridCells)
    })?;
    target.checked_sub(current).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxGridCells)
    })
}

fn shift_member_indices(
    placements: &mut [GridCoord],
    member_indices: &[usize],
    delta_x: isize,
    delta_y: isize,
    resources: &ResourceContext,
) -> Result<()> {
    if delta_x == 0 && delta_y == 0 {
        return Ok(());
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta_x.is_positive() {
                coord.x = resources.checked_grid_add(
                    coord.x,
                    usize::try_from(delta_x).map_err(|_| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })?,
                )?;
            } else {
                coord.x = coord.x.checked_sub(delta_x.unsigned_abs()).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
            }
            if delta_y.is_positive() {
                coord.y = resources.checked_grid_add(
                    coord.y,
                    usize::try_from(delta_y).map_err(|_| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })?,
                )?;
            } else {
                coord.y = coord.y.checked_sub(delta_y.unsigned_abs()).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
            }
        }
    }
    Ok(())
}

fn member_grid_bounds(
    member_indices: &[usize],
    placements: &[GridCoord],
    resources: &ResourceContext,
) -> Result<Option<RawBounds>> {
    let mut bounds = None::<RawBounds>;

    for index in member_indices {
        let Some(coord) = placements.get(*index).copied() else {
            return Ok(None);
        };
        let current = node_bounds(coord, resources)?;
        if let Some(existing) = &mut bounds {
            existing.include(current);
        } else {
            bounds = Some(current);
        }
    }

    Ok(bounds)
}

fn shift_member_indices_y(
    placements: &mut [GridCoord],
    member_indices: &[usize],
    delta: usize,
    resources: &ResourceContext,
) -> Result<()> {
    if delta == 0 {
        return Ok(());
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            coord.y = resources.checked_grid_add(coord.y, delta)?;
        }
    }
    Ok(())
}

fn shift_member_indices_x(
    placements: &mut [GridCoord],
    member_indices: &[usize],
    delta: isize,
    resources: &ResourceContext,
) -> Result<()> {
    if delta == 0 {
        return Ok(());
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta.is_positive() {
                coord.x = resources.checked_grid_add(
                    coord.x,
                    usize::try_from(delta).map_err(|_| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })?,
                )?;
            } else {
                coord.x = coord.x.checked_sub(delta.unsigned_abs()).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
            }
        }
    }
    Ok(())
}

fn place_group_nodes(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<HashMap<usize, GridCoord>> {
    let ranked = super::grid::place_ranked_grid_nodes_without_group_adjustments(
        graph, direction, resources,
    )?;
    let mut placements = HashMap::new();
    placements.try_reserve(ranked.len()).map_err(|_| {
        crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        }
    })?;
    placements.extend(ranked.into_iter().enumerate());

    Ok(placements)
}

fn mirror_local_placements(
    placements: &mut HashMap<usize, GridCoord>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<()> {
    let reverse_x = matches!(direction, GraphDirection::RightLeft);
    let reverse_y = matches!(direction, GraphDirection::BottomTop);
    if !reverse_x && !reverse_y {
        return Ok(());
    }

    let Some((min_x, max_x, min_y, max_y)) = placements.values().try_fold(
        None,
        |bounds: Option<(usize, usize, usize, usize)>, coord| {
            resources.charge_layout_work(1)?;
            Ok::<_, crate::error::AsciiError>(Some(match bounds {
                Some((min_x, max_x, min_y, max_y)) => (
                    min_x.min(coord.x),
                    max_x.max(coord.x),
                    min_y.min(coord.y),
                    max_y.max(coord.y),
                ),
                None => (coord.x, coord.x, coord.y, coord.y),
            }))
        },
    )?
    else {
        return Ok(());
    };

    for coord in placements.values_mut() {
        resources.charge_layout_work(1)?;
        if reverse_x {
            coord.x = min_x
                .checked_add(max_x.checked_sub(coord.x).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?)
                .ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
        }
        if reverse_y {
            coord.y = min_y
                .checked_add(max_y.checked_sub(coord.y).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?)
                .ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
        }
    }

    Ok(())
}

fn shift_external_nodes_away_from_group(
    graph: &AsciiGraph,
    member_indices: &[usize],
    group_bounds: RawBounds,
    placements: &mut [GridCoord],
    resources: &mut ResourceContext,
) -> Result<bool> {
    let member_indices = member_indices.iter().copied().collect::<HashSet<_>>();
    let graph_direction = graph.direction.canonical();
    let mut changed = false;

    for index in 0..placements.len() {
        if member_indices.contains(&index) {
            continue;
        }
        if !raw_bounds_intersects(group_bounds, node_bounds(placements[index], resources)?) {
            continue;
        }

        while raw_bounds_intersects(group_bounds, node_bounds(placements[index], resources)?)
            || node_overlaps_any_other(index, placements, resources)?
        {
            resources.charge_layout_work(1)?;
            changed = true;
            match graph_direction {
                GraphDirection::LeftRight => {
                    placements[index].y = resources.checked_grid_add(placements[index].y, 4)?;
                }
                GraphDirection::TopDown => {
                    placements[index].x = resources.checked_grid_add(placements[index].x, 4)?;
                }
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }
    }

    Ok(changed)
}

fn node_overlaps_any_other(
    index: usize,
    placements: &[GridCoord],
    resources: &ResourceContext,
) -> Result<bool> {
    let bounds = node_bounds(placements[index], resources)?;
    for (other_index, other_coord) in placements.iter().enumerate() {
        if index != other_index
            && raw_bounds_intersects(bounds, node_bounds(*other_coord, resources)?)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn node_bounds(coord: GridCoord, resources: &ResourceContext) -> Result<RawBounds> {
    let right = resources.checked_grid_add(coord.x, 2)?;
    let bottom = resources.checked_grid_add(coord.y, 2)?;
    Ok(RawBounds {
        x: isize::try_from(coord.x).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
        y: isize::try_from(coord.y).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
        right: isize::try_from(right).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
        bottom: isize::try_from(bottom).map_err(|_| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxGridCells)
        })?,
    })
}

fn raw_bounds_intersects(left: RawBounds, right: RawBounds) -> bool {
    !(left.right < right.x
        || right.right < left.x
        || left.bottom < right.y
        || right.bottom < left.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    #[test]
    fn group_override_graph_keeps_child_group_endpoint_edges() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("left-member", "Left");
        graph.add_node("right-member", "Right");
        graph.add_group_with_style(
            "left-group",
            "Left Group",
            Some(GraphDirection::LeftRight),
            vec!["left-member".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "right-group",
            "Right Group",
            Some(GraphDirection::TopDown),
            vec!["right-member".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "parent",
            "Parent",
            Some(GraphDirection::TopDown),
            vec!["left-group".to_string(), "right-group".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("left-group", "right-group");
        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("nested group topology should be valid");
        let members = group_placement_members(&graph, &topology, 2, &mut resources)
            .expect("parent group members should resolve through endpoint ownership");

        let override_graph = build_group_override_graph(
            &graph,
            &topology,
            &members,
            graph.direction,
            &mut resources,
        )
        .expect("child group endpoint edges should project into the local override graph");

        assert_eq!(override_graph.nodes.len(), 2);
        assert_eq!(override_graph.edges.len(), 1);
        assert_eq!(override_graph.edges[0].from, "left-group");
        assert_eq!(override_graph.edges[0].to, "right-group");
    }

    #[test]
    fn opposing_cross_group_edges_fallback_to_a_safe_root_layout() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for node in ["g0", "g1", "h0", "h1"] {
            graph.add_node(node, node);
        }
        graph.add_group_with_style(
            "G",
            "G",
            Some(GraphDirection::LeftRight),
            vec!["g0".to_string(), "g1".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "H",
            "H",
            Some(GraphDirection::LeftRight),
            vec!["h0".to_string(), "h1".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("g0", "g1");
        graph.add_edge("h0", "h1");
        graph.add_edge("g0", "h0");
        graph.add_edge("h1", "g1");

        let mut resources = unbounded_resources();
        let layout = super::super::layout_graph_with_resources(
            &graph,
            &AsciiRenderOptions::default(),
            &mut resources,
        )
        .expect("conflicting local block constraints should fall back instead of rejecting");

        assert_eq!(layout.nodes.len(), 4);
        for left in 0..layout.nodes.len() {
            for right in left + 1..layout.nodes.len() {
                assert!(!raw_bounds_intersects(
                    node_bounds(layout.nodes[left].grid, &resources)
                        .expect("left node bounds should fit"),
                    node_bounds(layout.nodes[right].grid, &resources)
                        .expect("right node bounds should fit"),
                ));
            }
        }
    }

    #[test]
    fn group_node_bounds_reject_geometry_before_range_materialization() {
        let resources = unbounded_resources();
        let error = node_bounds(
            GridCoord {
                x: usize::MAX,
                y: 0,
            },
            &resources,
        )
        .expect_err("group grid bounds should reject coordinate overflow");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    }
}
