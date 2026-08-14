use super::super::members::{
    GroupPlacementMember, group_bounds_for_placements, group_member_indices,
    group_placement_members, member_grid_bounds,
};
use super::super::side_constraints::override_member_semantics;
use super::super::{layout_work_allocation_failed, shift_external_nodes_away_from_group};
use super::GroupPlacementContext;
use crate::error::Result;
use crate::graph::layout::GridCoord;
use crate::graph::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection, GraphEdgeMarker, GraphEdgeStroke,
    GraphEdgeStyle, GraphNodeShape, GraphNodeStyle,
};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_core::OperationPhase;
use std::collections::HashMap;

pub(super) fn apply_subgraph_direction_overrides(
    context: &GroupPlacementContext<'_, '_>,
    placements: &mut [GridCoord],
    disabled_overrides: &[bool],
    resources: &mut ResourceContext,
    execution: Option<AsciiExecution<'_>>,
) -> Result<()> {
    let graph = context.graph;
    let topology = context.topology;
    for group_index in 0..graph.groups.len() {
        if let Some(execution) = execution {
            execution.checkpoint(OperationPhase::Layout)?;
        }
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
        let Some(direction) = context
            .direction_overrides
            .get(group_index)
            .copied()
            .flatten()
        else {
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

        let mut local = place_group_nodes(&override_graph, layout_direction, resources, execution)?;
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
            context.width_profile,
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
    build_group_override_graph_impl(graph, topology, members, direction, resources, || {})
}

fn build_group_override_graph_impl(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    members: &[GroupPlacementMember],
    direction: GraphDirection,
    resources: &mut ResourceContext,
    before_edge_allocation: impl FnOnce(),
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

    resources.charge_layout_work(graph.edges.len())?;
    before_edge_allocation();
    override_graph
        .edges
        .try_reserve(graph.edges.len())
        .map_err(|_| crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for edge in &graph.edges {
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
    resources.charge_layout_work(member_indices.len())?;
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

    resources.charge_layout_work(member_indices.len())?;
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
    execution: Option<AsciiExecution<'_>>,
) -> Result<HashMap<usize, GridCoord>> {
    let ranked = super::super::super::grid::place_ranked_grid_nodes_without_group_adjustments(
        graph, direction, resources, execution,
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
    use crate::options::AsciiRenderOptions;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    fn policy_with_work_limit(limit: usize) -> AsciiResourcePolicy {
        AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
            .expect("positive layout-work limit should be valid")
    }

    #[test]
    fn descendant_bounds_scan_accepts_exact_work_and_rejects_max_minus_one() {
        let placements = [
            GridCoord { x: 0, y: 0 },
            GridCoord { x: 4, y: 4 },
            GridCoord { x: 8, y: 8 },
        ];
        let member_indices = [0, 1, 2];
        let exact_work = member_indices.len();

        let exact_resources = ResourceContext::new(policy_with_work_limit(exact_work));
        let origin = member_origin(&placements, &member_indices, &exact_resources)
            .expect("the exact descendant-bounds work limit should pass")
            .expect("the member should have bounds");
        assert_eq!(origin, GridCoord { x: 0, y: 0 });
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_resources = ResourceContext::new(policy_with_work_limit(exact_work - 1));
        let error = member_origin(&placements, &member_indices, &below_resources)
            .expect_err("max-minus-one descendant-bounds work should fail");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn descendant_shift_accepts_exact_work_and_rejects_before_mutation_at_max_minus_one() {
        let original = [
            GridCoord { x: 4, y: 8 },
            GridCoord { x: 8, y: 12 },
            GridCoord { x: 12, y: 16 },
        ];
        let member_indices = [0, 1, 2];
        let exact_work = member_indices.len();

        let mut exact_placements = original;
        let exact_resources = ResourceContext::new(policy_with_work_limit(exact_work));
        shift_member_indices(
            &mut exact_placements,
            &member_indices,
            4,
            -4,
            &exact_resources,
        )
        .expect("the exact descendant-shift work limit should pass");
        assert_eq!(
            exact_placements,
            [
                GridCoord { x: 8, y: 4 },
                GridCoord { x: 12, y: 8 },
                GridCoord { x: 16, y: 12 },
            ]
        );
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let mut below_placements = original;
        let below_resources = ResourceContext::new(policy_with_work_limit(exact_work - 1));
        let error = shift_member_indices(
            &mut below_placements,
            &member_indices,
            4,
            -4,
            &below_resources,
        )
        .expect_err("max-minus-one descendant-shift work should fail before mutation");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
        assert_eq!(below_placements, original);
    }

    #[test]
    fn nested_local_direction_layout_accepts_exact_work_and_rejects_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id.to_uppercase());
        }
        graph.add_group_with_style(
            "inner",
            "Inner",
            Some(GraphDirection::TopDown),
            vec!["b".to_string(), "c".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("b", "c");
        graph.add_edge("a", "b");

        let options = AsciiRenderOptions::unicode();
        let mut measured_resources = unbounded_resources();
        let measured = crate::graph::layout::layout_graph_with_resources(
            &graph,
            &options,
            &mut measured_resources,
        )
        .expect("the nested local-direction graph should lay out");
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 1);

        let mut exact_resources = ResourceContext::new(policy_with_work_limit(exact_work));
        let exact = crate::graph::layout::layout_graph_with_resources(
            &graph,
            &options,
            &mut exact_resources,
        )
        .expect("the production layout entry should accept its exact work budget");
        assert_eq!(exact, measured);
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let mut below_resources = ResourceContext::new(policy_with_work_limit(exact_work - 1));
        let error = crate::graph::layout::layout_graph_with_resources(
            &graph,
            &options,
            &mut below_resources,
        )
        .expect_err("the production layout entry should reject at max-minus-one");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn override_edges_are_admitted_before_their_container_is_allocated() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id.to_uppercase());
        }
        graph.add_group_with_style(
            "group",
            "Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge("b", "c");

        let mut planning_resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut planning_resources)
            .expect("group topology should be valid");
        let members = group_placement_members(&graph, &topology, 0, &mut planning_resources)
            .expect("group members should resolve");

        let mut measured_resources = unbounded_resources();
        build_group_override_graph_impl(
            &graph,
            &topology,
            &members,
            graph.direction,
            &mut measured_resources,
            || {},
        )
        .expect("unbounded resources should admit override edges");
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work >= graph.edges.len());
        assert_eq!(measured_resources.layout_work_used(), exact_work);

        let mut exact_resources = ResourceContext::new(policy_with_work_limit(exact_work));
        let exact_allocated = Cell::new(false);
        build_group_override_graph_impl(
            &graph,
            &topology,
            &members,
            graph.direction,
            &mut exact_resources,
            || exact_allocated.set(true),
        )
        .expect("the exact override-edge work limit should pass");
        assert!(exact_allocated.get());

        let mut below_resources = ResourceContext::new(policy_with_work_limit(exact_work - 1));
        let below_allocated = Cell::new(false);
        let error = build_group_override_graph_impl(
            &graph,
            &topology,
            &members,
            graph.direction,
            &mut below_resources,
            || below_allocated.set(true),
        )
        .expect_err("max-minus-one override-edge work should fail before allocation");
        assert!(!below_allocated.get());
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == exact_work
                    && details.max == exact_work - 1
        ));
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
