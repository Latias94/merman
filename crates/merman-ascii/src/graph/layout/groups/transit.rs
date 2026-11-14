use crate::error::Result;
use crate::graph::layout::NodeLayout;
use crate::graph::model::{AsciiGraph, GraphGroupKind};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

// A bypass needs a transit cell beyond the node's reading margin and before the frame.
const MIN_TRANSIT_PADDING: usize = 3;

pub(super) fn group_routing_policies(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<GraphLayoutPolicy>> {
    let mut policies = Vec::new();
    if graph.groups.is_empty() {
        return Ok(policies);
    }
    execution.checkpoint(OperationPhase::Layout)?;
    resources.charge_layout_work(graph.groups.len())?;
    policies
        .try_reserve_exact(graph.groups.len())
        .map_err(|_| super::layout_work_allocation_failed())?;
    policies.resize(graph.groups.len(), *policy);
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        execution.checkpoint_loop(OperationPhase::Layout, edge_index)?;
        resources.charge_layout_work(1)?;
        if !edge.stroke.is_visible() {
            continue;
        }
        if edge.from == edge.to {
            resources.charge_layout_work(2)?;
            if let Some(group_index) = topology.group_index(&edge.from)
                && graph.groups[group_index].kind == GraphGroupKind::Container
                && let Some(parent_index) = topology.parent_group_index(group_index)
                && graph.groups[parent_index].kind == GraphGroupKind::Container
            {
                // A loop must leave its berth before turning and return through another
                // contact. Keep both exterior cells inside the immediate parent frame.
                let parent = &mut policies[parent_index];
                parent.group_padding_x = parent.group_padding_x.max(MIN_TRANSIT_PADDING);
                parent.group_padding_y = parent.group_padding_y.max(MIN_TRANSIT_PADDING);
            }
            continue;
        }
        resources.charge_layout_work(resources.checked_work_mul(layouts.len(), 2)?)?;
        let Some(from) = layouts.iter().find(|node| node.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|node| node.id == edge.to) else {
            continue;
        };
        let vertical = from.center_x() == to.center_x() && from.center_y() != to.center_y();
        let horizontal = from.center_y() == to.center_y() && from.center_x() != to.center_x();
        if !vertical && !horizontal {
            continue;
        }
        let mut blocked = false;
        for (node_index, node) in layouts.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, node_index)?;
            resources.charge_layout_work(1)?;
            if node.id == from.id || node.id == to.id {
                continue;
            }
            if vertical {
                blocked = node.x.saturating_sub(1) <= from.center_x()
                    && from.center_x() <= resources.checked_grid_add(node.right(), 1)?
                    && node.y <= from.center_y().max(to.center_y())
                    && node.bottom() >= from.center_y().min(to.center_y());
            } else {
                blocked = node.y.saturating_sub(1) <= from.center_y()
                    && from.center_y() <= resources.checked_grid_add(node.bottom(), 1)?
                    && node.x <= from.center_x().max(to.center_x())
                    && node.right() >= from.center_x().min(to.center_x());
            }
            if blocked {
                break;
            }
        }
        if !blocked {
            continue;
        }
        let mut topology_resources = resources.clone();
        let from_groups =
            topology.groups_containing_endpoint(&edge.from, &mut topology_resources)?;
        let to_groups = topology.groups_containing_endpoint(&edge.to, &mut topology_resources)?;
        let mut innermost = None;
        for (group_index, group) in graph.groups.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, group_index)?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container
                || !from_groups.contains(&group_index)
                || !to_groups.contains(&group_index)
            {
                continue;
            }
            let depth = topology.group_depth(group_index, &mut topology_resources)?;
            if innermost.is_none_or(|(best, _)| depth > best) {
                innermost = Some((depth, group_index));
            }
        }
        if let Some((_, group_index)) = innermost {
            let policy = &mut policies[group_index];
            if vertical {
                policy.group_padding_x = policy.group_padding_x.max(MIN_TRANSIT_PADDING);
            } else {
                policy.group_padding_y = policy.group_padding_y.max(MIN_TRANSIT_PADDING);
            }
        }
    }
    Ok(policies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::{GraphDirection, GraphGroupStyle};
    use crate::{AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::OperationControl;

    #[test]
    fn cancellation_precedes_transit_padding_work_admission() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["left", "right"] {
            graph.add_group_with_style(id, id, None, vec![], GraphGroupStyle::default());
        }
        let mut topology_resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources).unwrap();
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let error = group_routing_policies(
            &graph,
            &[],
            &topology,
            &AsciiRenderOptions::unicode()
                .flowchart_layout()
                .graph_policy(),
            &resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("cancellation must win before the group-sized work debit");
        assert!(matches!(error, AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout));
        assert_eq!(resources.layout_work_used(), 0);
    }
}
