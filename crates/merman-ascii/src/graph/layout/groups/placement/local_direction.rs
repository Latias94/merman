use super::super::members::{
    GroupPlacementMember, group_bounds_for_placements, group_member_indices,
    group_placement_members, member_grid_bounds,
};
use super::super::side_constraints::override_member_semantics;
use super::super::{layout_work_allocation_failed, shift_external_nodes_away_from_group};
use crate::error::Result;
use crate::graph::layout::GridCoord;
use crate::graph::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection, GraphEdgeMarker, GraphEdgeStroke,
    GraphEdgeStyle, GraphNodeShape, GraphNodeStyle,
};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::HashMap;

pub(super) fn apply_subgraph_direction_overrides(
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

fn place_group_nodes(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<HashMap<usize, GridCoord>> {
    let ranked = super::super::super::grid::place_ranked_grid_nodes_without_group_adjustments(
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
}
