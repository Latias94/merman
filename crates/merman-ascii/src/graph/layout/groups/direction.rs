use super::super::super::model::{AsciiGraph, GraphDirection};
use super::super::super::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::error::Result;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

pub(super) fn plan_group_direction_overrides(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<Option<GraphDirection>>> {
    if graph.diagram_type() != "flowchart" {
        resources.charge_layout_work(graph.groups.len())?;
        let mut directions = Vec::new();
        directions
            .try_reserve(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;
        directions.extend(graph.groups.iter().map(|group| group.direction));
        return Ok(directions);
    }

    let external_connections = flowchart_external_connections(graph, topology, resources)?;
    let effective_directions = flowchart_effective_directions(graph, topology, resources)?;

    resources.charge_layout_work(graph.groups.len())?;
    let mut overrides = Vec::new();
    overrides
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    for (group_index, group) in graph.groups.iter().enumerate() {
        let direction = effective_directions.get(group_index).copied().flatten();
        let has_external_connection = external_connections
            .get(group_index)
            .copied()
            .unwrap_or(false);
        overrides.push(if group.direction.is_some() || !has_external_connection {
            direction
        } else {
            None
        });
    }
    Ok(overrides)
}

fn flowchart_external_connections(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<bool>> {
    resources.charge_layout_work(graph.groups.len())?;
    let mut external_connections = Vec::new();
    external_connections
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    external_connections.resize(graph.groups.len(), false);

    for edge in &graph.edges {
        let mut source_scope = topology.groups_containing_endpoint(&edge.from, resources)?;
        let mut target_scope = topology.groups_containing_endpoint(&edge.to, resources)?;
        include_group_endpoint_scope(topology, &edge.from, &mut source_scope)?;
        include_group_endpoint_scope(topology, &edge.to, &mut target_scope)?;

        resources.charge_layout_work(graph.groups.len())?;
        for (group_index, has_external_connection) in external_connections.iter_mut().enumerate() {
            if source_scope.contains(&group_index) ^ target_scope.contains(&group_index) {
                *has_external_connection = true;
            }
        }
    }

    Ok(external_connections)
}

fn include_group_endpoint_scope(
    topology: &GraphGroupTopology<'_>,
    endpoint: &str,
    scope: &mut std::collections::HashSet<usize>,
) -> Result<()> {
    let Some(GraphEndpointIndex::Group(group_index)) = topology.endpoint_index(endpoint) else {
        return Ok(());
    };
    scope
        .try_reserve(1)
        .map_err(|_| layout_work_allocation_failed())?;
    scope.insert(group_index);
    Ok(())
}

fn flowchart_effective_directions(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<Option<GraphDirection>>> {
    resources.charge_layout_work(graph.groups.len())?;
    let mut directions = Vec::new();
    directions
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    directions.resize(graph.groups.len(), None);

    resources.charge_layout_work(graph.groups.len())?;
    let mut states = Vec::new();
    states
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;
    states.resize(graph.groups.len(), DirectionVisitState::Unvisited);

    resources.charge_layout_work(graph.groups.len())?;
    let mut stack = Vec::new();
    stack
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_work_allocation_failed())?;

    for start_index in 0..graph.groups.len() {
        resources.charge_layout_work(1)?;
        if states[start_index] == DirectionVisitState::Complete {
            continue;
        }
        stack.push(DirectionFrame::Enter(start_index));

        while let Some(frame) = stack.pop() {
            resources.charge_layout_work(1)?;
            match frame {
                DirectionFrame::Enter(group_index) => match states[group_index] {
                    DirectionVisitState::Complete => {}
                    DirectionVisitState::Visiting => {
                        // Parser-backed projection rejects ownership cycles. Keep internal test
                        // graphs bounded and deterministic by resolving any residual cycle from
                        // the root direction instead of recursing indefinitely.
                        directions[group_index] = Some(perpendicular_default(graph.direction));
                        states[group_index] = DirectionVisitState::Complete;
                    }
                    DirectionVisitState::Unvisited => {
                        states[group_index] = DirectionVisitState::Visiting;
                        stack.push(DirectionFrame::Resolve(group_index));
                        if let Some(parent_index) = topology.parent_group_index(group_index)
                            && states[parent_index] != DirectionVisitState::Complete
                        {
                            stack.push(DirectionFrame::Enter(parent_index));
                        }
                    }
                },
                DirectionFrame::Resolve(group_index) => {
                    let parent_direction = topology
                        .parent_group_index(group_index)
                        .and_then(|parent_index| directions.get(parent_index).copied().flatten())
                        .unwrap_or(graph.direction);
                    directions[group_index] = Some(
                        graph.groups[group_index]
                            .direction
                            .unwrap_or_else(|| perpendicular_default(parent_direction)),
                    );
                    states[group_index] = DirectionVisitState::Complete;
                }
            }
        }
    }

    Ok(directions)
}

fn perpendicular_default(parent_direction: GraphDirection) -> GraphDirection {
    match parent_direction {
        GraphDirection::TopDown => GraphDirection::LeftRight,
        GraphDirection::LeftRight | GraphDirection::RightLeft | GraphDirection::BottomTop => {
            GraphDirection::TopDown
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectionVisitState {
    Unvisited,
    Visiting,
    Complete,
}

#[derive(Debug, Clone, Copy)]
enum DirectionFrame {
    Enter(usize),
    Resolve(usize),
}

fn layout_work_allocation_failed() -> crate::error::AsciiError {
    crate::error::AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    #[test]
    fn explicit_direction_survives_external_connections() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("outside", "Outside");
        graph.add_group_with_style(
            "group",
            "Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge("b", "outside");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("group topology should be valid");
        let overrides = plan_group_direction_overrides(&graph, &topology, &mut resources)
            .expect("explicit direction planning should succeed");

        assert_eq!(overrides, vec![Some(GraphDirection::LeftRight)]);
    }

    #[test]
    fn implicit_isolated_groups_toggle_each_parent_axis() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["a".to_string(), "b".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("a", "b");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("nested group topology should be valid");
        let overrides = plan_group_direction_overrides(&graph, &topology, &mut resources)
            .expect("implicit direction planning should succeed");

        assert_eq!(
            overrides,
            vec![
                Some(GraphDirection::TopDown),
                Some(GraphDirection::LeftRight),
            ]
        );
    }

    #[test]
    fn implicit_external_groups_keep_the_root_layout() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("outside", "Outside");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["a".to_string(), "b".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge("b", "outside");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("group topology should be valid");
        let overrides = plan_group_direction_overrides(&graph, &topology, &mut resources)
            .expect("implicit direction planning should succeed");

        assert_eq!(overrides, vec![None]);
    }

    #[test]
    fn direction_planning_has_an_exact_work_boundary() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("outside", "Outside");
        graph.add_group_with_style(
            "group",
            "Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge("b", "outside");

        let mut topology_resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources)
            .expect("group topology should be valid");
        let mut measured_resources = unbounded_resources();
        let measured = plan_group_direction_overrides(&graph, &topology, &mut measured_resources)
            .expect("unbounded direction planning should pass");
        let required_work = measured_resources.layout_work_used();
        assert!(required_work > 0);

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, required_work)
            .expect("exact direction-planning work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let exact = plan_group_direction_overrides(&graph, &topology, &mut exact_resources)
            .expect("exact direction-planning work should pass");
        assert_eq!(exact, measured);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, required_work - 1)
            .expect("max-minus-one direction-planning work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = plan_group_direction_overrides(&graph, &topology, &mut below_resources)
            .expect_err("max-minus-one direction-planning work should reject");
        assert!(matches!(
            error,
            crate::error::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == required_work
                    && details.max == required_work - 1
        ));
    }
}
