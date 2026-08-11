use super::{
    GroupPlacementMember, group_bounds_for_placements, group_member_indices,
    layout_work_allocation_failed, node_bounds, try_bool_slots,
};
use crate::Result;
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::graph::{
    AsciiGraph, GraphDirection, GraphNodeSemantics, GraphNodeSide, GraphNodeSideConstraint,
};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::try_clone_layout_text;
use std::collections::HashMap;

pub(super) fn reserve_group_left_constraint_space(
    graph: &AsciiGraph,
    placements: &mut [super::GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
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
            width_profile,
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

pub(super) fn include_side_constrained_group_followers(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    members: &mut Vec<GroupPlacementMember>,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut included_nodes = try_bool_slots(graph.nodes.len(), resources)?;
    for member in members.iter() {
        resources.charge_layout_work(member.node_indices.len())?;
        for node_index in &member.node_indices {
            if let Some(included) = included_nodes.get_mut(*node_index) {
                *included = true;
            }
        }
    }

    resources.charge_layout_work(graph.nodes.len())?;
    for (node_index, node) in graph.nodes.iter().enumerate() {
        if included_nodes.get(node_index).copied().unwrap_or(false) {
            continue;
        }
        let Some(constraint) = node.semantics.side_constraint.as_ref() else {
            continue;
        };
        let owner_groups =
            topology.groups_containing_endpoint(constraint.anchor_id(), resources)?;
        if !owner_groups.contains(&group_index) {
            continue;
        }
        resources.charge_layout_work(1)?;
        members
            .try_reserve(1)
            .map_err(|_| layout_work_allocation_failed())?;
        let mut node_indices = Vec::new();
        node_indices
            .try_reserve_exact(1)
            .map_err(|_| layout_work_allocation_failed())?;
        node_indices.push(node_index);
        members.push(GroupPlacementMember {
            id: try_clone_layout_text(&node.id, resources)?,
            endpoint: GraphEndpointIndex::Node(node_index),
            node_indices,
        });
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
        compartments: None,
        side_constraint: Some(GraphNodeSideConstraint::new(
            try_clone_layout_text(&anchor_member.id, resources)?,
            constraint.side(),
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    #[test]
    fn side_follower_scan_accepts_exact_work_and_rejects_before_member_materialization() {
        const EXACT_WORK: usize = 11;

        let mut graph = AsciiGraph::new_for_diagram("state", GraphDirection::TopDown);
        graph.add_node_with_semantics(
            "anchor",
            "Anchor",
            GraphNodeShape::Rect,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
        graph.add_node_with_semantics(
            "note",
            "Note",
            GraphNodeShape::Rect,
            GraphNodeStyle::default(),
            GraphNodeSemantics {
                compartments: None,
                side_constraint: Some(GraphNodeSideConstraint::new("anchor", GraphNodeSide::Right)),
            },
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            Some(GraphDirection::LeftRight),
            vec!["anchor".to_string()],
            GraphGroupStyle::default(),
        );

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources)
            .expect("state group topology should be valid");
        let initial_member = GroupPlacementMember {
            id: "anchor".to_string(),
            endpoint: GraphEndpointIndex::Node(0),
            node_indices: vec![0],
        };
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXACT_WORK)
            .expect("exact layout-work limit should be valid");
        let mut exact_members = vec![initial_member.clone()];
        let mut exact_resources = ResourceContext::new(exact_policy);

        include_side_constrained_group_followers(
            &graph,
            &topology,
            0,
            &mut exact_members,
            &mut exact_resources,
        )
        .expect("side-constrained follower planning should succeed");

        assert_eq!(exact_members.len(), 2);
        assert_eq!(exact_resources.layout_work_used(), EXACT_WORK);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXACT_WORK - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_members = vec![initial_member];
        let mut below_resources = ResourceContext::new(below_policy);
        let error = include_side_constrained_group_followers(
            &graph,
            &topology,
            0,
            &mut below_members,
            &mut below_resources,
        )
        .expect_err("max-minus-one work should reject before follower materialization");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, EXACT_WORK);
        assert_eq!(details.max, EXACT_WORK - 1);
        assert_eq!(below_members.len(), 1);
        assert_eq!(below_members[0].id, "anchor");
        assert!(matches!(
            below_members[0].endpoint,
            GraphEndpointIndex::Node(0)
        ));
        assert_eq!(below_members[0].node_indices, vec![0]);
    }
}
