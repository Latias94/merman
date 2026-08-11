use super::bounds::{RawBounds, raw_group_bounds_for_members};
use super::{layout_work_allocation_failed, node_bounds, try_bool_slots};
use crate::error::Result;
use crate::graph::layout::GridCoord;
use crate::graph::model::AsciiGraph;
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::try_clone_layout_text;
use std::collections::HashSet;

pub(super) fn graph_endpoint_group_ids<'a>(
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

pub(super) fn group_member_indices(
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    topology.group_member_node_indices(group_index, resources)
}

pub(super) fn group_bounds_for_placements(
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
pub(super) struct GroupPlacementMember {
    pub(super) id: String,
    pub(super) endpoint: GraphEndpointIndex,
    pub(super) node_indices: Vec<usize>,
}

pub(super) fn group_placement_members(
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

fn include_side_constrained_group_followers(
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

pub(super) fn member_grid_bounds(
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

pub(super) fn shift_member_indices_y(
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

pub(super) fn shift_member_indices_x(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{
        GraphDirection, GraphGroupStyle, GraphNodeSemantics, GraphNodeShape, GraphNodeSide,
        GraphNodeSideConstraint, GraphNodeStyle,
    };
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
                side_constraint: Some(GraphNodeSideConstraint::new("anchor", GraphNodeSide::Right)),
                ..GraphNodeSemantics::default()
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
