use super::members::{GroupPlacementMember, group_bounds_for_placements, group_member_indices};
use super::{node_bounds, try_bool_slots};
use crate::Result;
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::graph::{
    AsciiGraph, GraphDirection, GraphNodeSemantics, GraphNodeSide, GraphNodeSideConstraint,
};
use crate::options::FlowchartLayoutPolicy;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::try_clone_layout_text;
use std::collections::HashMap;

pub(super) fn reserve_group_left_constraint_space(
    graph: &AsciiGraph,
    placements: &mut [super::GridCoord],
    topology: &GraphGroupTopology<'_>,
    policy: &FlowchartLayoutPolicy,
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.direction.canonical() != GraphDirection::TopDown {
        return Ok(());
    }

    for (group_index, group) in graph.groups.iter().enumerate() {
        resources.charge_layout_work(graph.nodes.len())?;
        let mut fixed_left_nodes = try_bool_slots(graph.nodes.len(), resources)?;
        let mut furthest_left_note_right = None::<isize>;
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let Some(constraint) = node.semantics.side_constraint.as_ref() else {
                continue;
            };
            if constraint.side() != GraphNodeSide::Left {
                continue;
            }
            let anchor_is_group = constraint.anchor_id() == group.id;
            let anchor_is_group_member = topology
                .groups_containing_endpoint(constraint.anchor_id(), resources)?
                .contains(&group_index);
            if !anchor_is_group && !anchor_is_group_member {
                continue;
            }
            fixed_left_nodes[node_index] = true;
            let Some(placement) = placements.get(node_index).copied() else {
                continue;
            };
            let note_right = node_bounds(placement, resources)?.right;
            furthest_left_note_right = Some(
                furthest_left_note_right.map_or(note_right, |current| current.max(note_right)),
            );
        }
        let Some(furthest_left_note_right) = furthest_left_note_right else {
            continue;
        };

        let member_indices = group_member_indices(topology, group_index, resources)?;
        let Some(group_bounds) = group_bounds_for_placements(
            graph,
            group_index,
            &member_indices,
            placements,
            policy,
            resources,
        )?
        else {
            continue;
        };
        if furthest_left_note_right < group_bounds.x {
            continue;
        }
        let shift = furthest_left_note_right
            .checked_sub(group_bounds.x)
            .and_then(|distance| distance.checked_add(1))
            .and_then(|distance| usize::try_from(distance).ok())
            .ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxGridCells)
            })?;
        for (node_index, placement) in placements.iter_mut().enumerate() {
            if fixed_left_nodes.get(node_index).copied().unwrap_or(false) {
                continue;
            }
            resources.charge_layout_work(1)?;
            placement.x = resources.checked_grid_add(placement.x, shift)?;
        }
    }
    Ok(())
}

pub(super) fn override_member_semantics(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    members: &[GroupPlacementMember],
    endpoint_to_member: &HashMap<GraphEndpointIndex, usize>,
    member: &GroupPlacementMember,
    resources: &ResourceContext,
) -> Result<GraphNodeSemantics> {
    let GraphEndpointIndex::Node(node_index) = member.endpoint else {
        return Ok(GraphNodeSemantics::default());
    };
    let Some(constraint) = graph
        .nodes
        .get(node_index)
        .and_then(|node| node.semantics.side_constraint.as_ref())
    else {
        return Ok(GraphNodeSemantics::default());
    };
    let Some(anchor_endpoint) = topology.endpoint_index(constraint.anchor_id()) else {
        return Ok(GraphNodeSemantics::default());
    };
    let Some(anchor_member_index) = endpoint_to_member.get(&anchor_endpoint).copied() else {
        return Ok(GraphNodeSemantics::default());
    };
    let Some(member_index) = endpoint_to_member.get(&member.endpoint).copied() else {
        return Ok(GraphNodeSemantics::default());
    };
    if anchor_member_index == member_index {
        return Ok(GraphNodeSemantics::default());
    }
    let Some(anchor_member) = members.get(anchor_member_index) else {
        return Ok(GraphNodeSemantics::default());
    };
    Ok(GraphNodeSemantics {
        side_constraint: Some(GraphNodeSideConstraint::new(
            try_clone_layout_text(&anchor_member.id, resources)?,
            constraint.side(),
        )),
    })
}
