use crate::error::Result;
use crate::graph::label::try_measure_edge_label;
use crate::graph::layout::NodeLayout;
use crate::graph::model::{AsciiGraph, GraphGroupKind};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;
use rustc_hash::FxHashMap as HashMap;

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
    let mut node_loops = HashMap::<&str, (usize, usize)>::default();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        execution.checkpoint_loop(OperationPhase::Layout, edge_index)?;
        resources.charge_layout_work(1)?;
        if !edge.stroke.is_visible() {
            continue;
        }
        if edge.from == edge.to {
            resources.charge_layout_work(2)?;
            let enclosing = match topology.endpoint_index(&edge.from) {
                Some(GraphEndpointIndex::Node(_)) => {
                    if topology.direct_node_group_index(&edge.from).is_none() {
                        continue;
                    }
                    if !node_loops.contains_key(edge.from.as_str()) {
                        node_loops
                            .try_reserve(1)
                            .map_err(|_| super::layout_work_allocation_failed())?;
                    }
                    let (count, width) = node_loops.entry(edge.from.as_str()).or_default();
                    *count = resources.checked_work_add(*count, 1)?;
                    if let Some(label) = &edge.label {
                        *width = (*width).max(
                            try_measure_edge_label(
                                label,
                                policy.terminal_width_profile,
                                resources,
                                edge.label_break_policy,
                            )?
                            .map_or(0, |metrics| metrics.max_width),
                        );
                    }
                    continue;
                }
                Some(GraphEndpointIndex::Group(group)) => topology.parent_group_index(group),
                None => None,
            };
            if let Some(group_index) =
                nearest_container(graph, topology, enclosing, resources, execution)?
            {
                let parent = &mut policies[group_index];
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
        let label = edge
            .label
            .as_deref()
            .map(|label| {
                try_measure_edge_label(
                    label,
                    policy.terminal_width_profile,
                    resources,
                    edge.label_break_policy,
                )
            })
            .transpose()?
            .flatten();
        if !vertical && !horizontal && label.is_none() {
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
            } else if horizontal {
                blocked = node.y.saturating_sub(1) <= from.center_y()
                    && from.center_y() <= resources.checked_grid_add(node.bottom(), 1)?
                    && node.x <= from.center_x().max(to.center_x())
                    && node.right() >= from.center_x().min(to.center_x());
            }
            if blocked {
                break;
            }
        }
        if !blocked && label.is_none() {
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
            let requested_title_clearance = policy.group_title_clearance;
            let policy = &mut policies[group_index];
            if blocked && vertical {
                policy.group_padding_x = policy.group_padding_x.max(MIN_TRANSIT_PADDING);
            } else if blocked && horizontal {
                policy.group_padding_y = policy.group_padding_y.max(MIN_TRANSIT_PADDING);
            }
            if let Some(label) = label {
                // Node contours no longer grow with their shared axis bands. Preserve the
                // internal channel's complete label block in the enclosing frame instead.
                let span_x = resources
                    .checked_grid_add(from.right().max(to.right()) - from.x.min(to.x), 1)?;
                let span_y = resources
                    .checked_grid_add(from.bottom().max(to.bottom()) - from.y.min(to.y), 1)?;
                let extra_x = resources
                    .checked_grid_add(label.max_width, 2)?
                    .saturating_sub(span_x)
                    .div_ceil(2);
                let extra_y = resources
                    .checked_grid_add(label.line_count, 2)?
                    .saturating_sub(span_y)
                    .div_ceil(2);
                policy.group_padding_x = policy
                    .group_padding_x
                    .max(resources.checked_grid_add(extra_x, 1)?);
                policy.group_padding_y = policy
                    .group_padding_y
                    .max(resources.checked_grid_add(extra_y, 1)?);
                // The upper envelope uses title clearance, while group_padding_y owns
                // only the bottom. Admit both halves of a centered multiline condition.
                policy.group_title_clearance = policy
                    .group_title_clearance
                    .max(resources.checked_grid_add(requested_title_clearance, extra_y)?);
            }
        }
    }
    if !node_loops.is_empty() {
        for (node_index, node) in layouts.iter().enumerate() {
            execution.checkpoint_loop(OperationPhase::Layout, node_index)?;
            resources.charge_layout_work(1)?;
            let Some(&(count, label_width)) = node_loops.get(node.id.as_str()) else {
                continue;
            };
            let Some(group_index) = nearest_container(
                graph,
                topology,
                topology.direct_node_group_index(&node.id),
                resources,
                execution,
            )?
            else {
                continue;
            };
            let outer_offset = resources.checked_grid_mul(count - 1, 2)?;
            let right_half_width = node.width.saturating_sub(1 + node.width / 2);
            // The lane needs one cell beyond the label and one more before the frame.
            let label_padding = resources
                .checked_grid_add(label_width, 2)?
                .saturating_sub(right_half_width);
            let padding_x =
                resources.checked_grid_add(MIN_TRANSIT_PADDING.max(label_padding), outer_offset)?;
            let padding_y = resources.checked_grid_add(MIN_TRANSIT_PADDING, outer_offset)?;
            let parent = &mut policies[group_index];
            parent.group_padding_x = parent.group_padding_x.max(padding_x);
            parent.group_padding_y = parent.group_padding_y.max(padding_y);
        }
    }
    Ok(policies)
}

fn nearest_container(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    mut enclosing: Option<usize>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    for depth in 0..graph.groups.len() {
        execution.checkpoint_loop(OperationPhase::Layout, depth)?;
        resources.charge_layout_work(1)?;
        let Some(group_index) = enclosing else { break };
        // Divider sections are open; only the nearest actual frame needs loop clearance.
        if graph.groups[group_index].kind == GraphGroupKind::Container {
            return Ok(Some(group_index));
        }
        enclosing = topology.parent_group_index(group_index);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::GridCoord;
    use crate::graph::model::{
        GraphDirection, GraphEdgeAttrs, GraphEdgeStroke, GraphGroupStyle, GraphNodeShape,
        GraphNodeStyle,
    };
    use crate::{
        AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy,
        TerminalWidthProfile,
    };
    use merman_model::OperationControl;

    #[test]
    fn contained_loop_envelopes_pad_only_the_nearest_first_parent_container() {
        let unlabeled = [None, None, None];
        let labeled = [Some("first"), Some("second"), Some("third")];
        let ambiguous = [Some("····"), None, Some("x")];
        let blank = [Some("   "), Some("<br/><br/>"), None];
        for (width, labels, profile, padding_x, padding_y) in [
            (3, &unlabeled[..0], TerminalWidthProfile::Unicode, 2, 2),
            (3, &unlabeled[..1], TerminalWidthProfile::Unicode, 3, 3),
            (3, &unlabeled[..], TerminalWidthProfile::Unicode, 7, 7),
            (3, &blank[..], TerminalWidthProfile::Unicode, 7, 7),
            (3, &labeled[..], TerminalWidthProfile::Unicode, 11, 7),
            (4, &labeled[..], TerminalWidthProfile::Unicode, 11, 7),
            (5, &labeled[..], TerminalWidthProfile::Unicode, 10, 7),
            (20, &labeled[..], TerminalWidthProfile::Unicode, 7, 7),
            (5, &ambiguous[..], TerminalWidthProfile::Unicode, 8, 7),
            (5, &ambiguous[..], TerminalWidthProfile::Cjk, 12, 7),
        ] {
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            graph.add_node("A", "A");
            graph.add_group_with_kind_and_style(
                "section",
                "",
                None,
                vec!["A".into()],
                GraphGroupKind::Divider,
                GraphGroupStyle::default(),
            );
            for (id, member) in [
                ("BUFFER", "section"),
                ("outer", "BUFFER"),
                ("other_parent", "section"),
            ] {
                graph.add_group_with_style(
                    id,
                    id,
                    None,
                    vec![member.into()],
                    GraphGroupStyle::default(),
                );
            }
            for label in labels {
                graph.add_edge_with_attrs(
                    "A",
                    "A",
                    GraphEdgeAttrs {
                        label: label.map(String::from),
                        ..GraphEdgeAttrs::default()
                    },
                );
            }
            graph.add_edge_with_attrs(
                "A",
                "A",
                GraphEdgeAttrs {
                    label: Some("this invisible label must not expand the frame".into()),
                    stroke: GraphEdgeStroke::Invisible,
                    ..GraphEdgeAttrs::default()
                },
            );
            let layouts = [NodeLayout {
                id: "A".into(),
                label: GraphLabel::new_with_profile("A", profile),
                shape: GraphNodeShape::Rect,
                style: GraphNodeStyle::default(),
                grid: GridCoord { x: 0, y: 0 },
                x: 10,
                y: 10,
                width,
                height: 3,
            }];
            let resource_policy = AsciiResourcePolicy::unbounded();
            let mut topology_resources = ResourceContext::new(resource_policy);
            let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources).unwrap();
            let policy = AsciiRenderOptions::unicode()
                .with_terminal_width_profile(profile)
                .flowchart_layout()
                .graph_policy();
            let resources = ResourceContext::new(resource_policy);
            let policies = group_routing_policies(
                &graph,
                &layouts,
                &topology,
                &policy,
                &resources,
                AsciiExecution::for_test(&resource_policy),
            )
            .expect("visible loop envelopes should fit their nearest enclosing frame");
            for (group, actual) in graph.groups.iter().zip(policies) {
                let expected = if group.id == "BUFFER" {
                    GraphLayoutPolicy {
                        group_padding_x: padding_x,
                        group_padding_y: padding_y,
                        ..policy
                    }
                } else {
                    policy
                };
                assert_eq!(
                    actual, expected,
                    "width={width}, labels={labels:?}, profile={profile:?}, group={}",
                    group.id
                );
            }
        }
    }

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
