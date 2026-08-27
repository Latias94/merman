use super::super::model::{AsciiGraph, AsciiGraphGroup, GraphDirection, GraphGroupKind};
use super::super::topology::GraphGroupTopology;
use super::{GridCoord, GroupLayout, NodeLayout};
use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::HashSet;

mod bounds;
mod direction;
mod members;
mod placement;
mod side_constraints;

use self::bounds::RawBounds;
use self::members::{
    graph_endpoint_group_ids, group_bounds_for_placements, group_member_indices,
    member_grid_bounds, shift_member_indices_x, shift_member_indices_y,
};

#[derive(Debug)]
pub(super) struct LaidOutGroups {
    pub(super) items: Vec<GroupLayout>,
    pub(super) background_order: Vec<usize>,
}

pub(super) fn apply_group_placement_adjustments(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    placement::apply_group_placement_adjustments(
        graph, placements, topology, policy, resources, execution,
    )
}

pub(super) fn subgraph_offsets(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    bounds::subgraph_offsets(graph, layouts, topology, policy, resources)
}

pub(super) fn layout_groups(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<LaidOutGroups> {
    bounds::layout_groups(graph, layouts, topology, policy, resources, execution)
}

pub(super) fn empty_group_minimum_size(
    group: &AsciiGraphGroup,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    bounds::empty_group_minimum_size(group, policy, resources)
}

fn separate_external_nodes_from_groups(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
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
                policy,
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
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<usize> {
    const SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD: usize = 4;

    if !index
        .has_external_incoming_overhead
        .get(node_index)
        .copied()
        .unwrap_or(false)
    {
        return Ok(policy.rank_gap_y);
    }

    resources.checked_grid_add(policy.rank_gap_y, SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD)
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
    // Charge the target-bound calculation and each actual comparison before doing the geometry
    // work. This keeps the O(N) scan visible to the render-wide layout-work budget and makes a
    // rejected comparison side-effect free for callers that may mutate placements afterwards.
    resources.charge_layout_work(1)?;
    let bounds = node_bounds(placements[index], resources)?;
    for (other_index, other_coord) in placements.iter().enumerate() {
        if index == other_index {
            continue;
        }
        resources.charge_layout_work(1)?;
        if raw_bounds_intersects(bounds, node_bounds(*other_coord, resources)?) {
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
    use crate::AsciiRenderOptions;
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
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

    #[test]
    fn node_overlap_scan_accepts_exact_work_and_rejects_max_minus_one() {
        let placements = [
            GridCoord { x: 0, y: 0 },
            GridCoord { x: 10, y: 0 },
            GridCoord { x: 20, y: 0 },
        ];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let measured_resources = ResourceContext::new(unbounded);
        assert!(
            !node_overlaps_any_other(0, &placements, &measured_resources)
                .expect("unbounded overlap scan should succeed")
        );
        let exact_work = measured_resources.layout_work_used();
        assert_eq!(
            exact_work, 3,
            "target plus two comparisons should be charged"
        );

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact overlap-scan budget should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        assert!(
            !node_overlaps_any_other(0, &placements, &exact_resources)
                .expect("exact overlap-scan budget should pass")
        );
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one overlap-scan budget should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let error = node_overlaps_any_other(0, &placements, &below_resources)
            .expect_err("max-minus-one overlap-scan budget should reject");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }
}
