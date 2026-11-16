use super::super::model::{AsciiGraph, AsciiGraphGroup, GraphDirection, GraphGroupKind};
use super::super::topology::GraphGroupTopology;
use super::{GridCoord, GroupLayout, NodeLayout};
use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::{AsciiResourceLimitId, ResourceContext};

mod bounds;
mod direction;
mod members;
mod placement;
mod side_constraints;
mod spacing;
mod transit;

use self::bounds::RawBounds;
use self::members::{
    group_member_indices, member_grid_bounds, shift_member_indices_x, shift_member_indices_y,
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
) -> Result<Vec<Option<GraphDirection>>> {
    placement::apply_group_placement_adjustments(
        graph, placements, topology, policy, resources, execution,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reserve_compound_axis_spacing(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    demand: &crate::graph::demand::SceneDemand<'_>,
    column_widths: &mut super::grid::AxisSizes,
    row_heights: &mut super::grid::AxisSizes,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    // Both passes measure the same initial projection. Merge their requirements by maximum,
    // rather than adding a second allowance to an already expanded separator band.
    let transaction_resources = resources.clone();
    transaction_resources.transaction(|resources| {
        let divider_rows = bounds::empty_divider_row_requirements(
            graph,
            layouts,
            topology,
            policy,
            row_heights,
            resources,
            execution,
        )?;
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(divider_rows.len())?;
        // Spacing inserts only trailing node-separator rows: a group grid bottom is also a
        // member node's bottom. Reserve at most one such key per node as well as every Divider
        // key, so spacing cannot consume the capacity needed by the final allocation-free merge.
        let additional_rows = if divider_rows.is_empty() {
            0
        } else {
            resources.checked_work_add(layouts.len(), divider_rows.len())?
        };
        row_heights
            .try_reserve(additional_rows)
            .map_err(|_| layout_work_allocation_failed())?;
        let mut spacing_resources = resources.clone();
        spacing::reserve_compound_axis_spacing(
            graph,
            layouts,
            topology,
            policy,
            demand,
            column_widths,
            row_heights,
            &mut spacing_resources,
            execution,
        )?;
        for (row, height) in divider_rows {
            super::grid::set_axis_size(row_heights, row, height);
        }
        Ok(())
    })
}

pub(super) fn layout_scene_groups(
    graph: &AsciiGraph,
    layouts: &mut [NodeLayout],
    topology: &GraphGroupTopology<'_>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(LaidOutGroups, usize, usize)> {
    bounds::layout_scene_groups(graph, layouts, topology, policy, resources, execution)
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
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.groups.is_empty() || placements.is_empty() {
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
            if graph.groups[group_index].kind != GraphGroupKind::Container {
                continue;
            }
            resources.charge_layout_work(graph.nodes.len())?;
            let member_indices = group_member_indices(topology, group_index, resources)?;
            if member_indices.is_empty() {
                continue;
            }
            // Axis projection owns physical frame clearance, but it requires one separator
            // band outside both member envelopes. Abutting node bands have no such host.
            let Some(group_bounds) = member_grid_bounds(&member_indices, placements, resources)?
            else {
                continue;
            };
            let group_bounds = RawBounds {
                x: group_bounds.x.saturating_sub(1),
                y: group_bounds.y.saturating_sub(1),
                right: group_bounds
                    .right
                    .checked_add(1)
                    .ok_or_else(|| resources.grid_overflow())?,
                bottom: group_bounds
                    .bottom
                    .checked_add(1)
                    .ok_or_else(|| resources.grid_overflow())?,
            };
            changed |= shift_external_nodes_away_from_group(
                graph,
                topology,
                group_index,
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
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    member_indices: &[usize],
    group_bounds: RawBounds,
    placements: &mut [GridCoord],
    resources: &mut ResourceContext,
) -> Result<bool> {
    let graph_direction = graph.direction.canonical();
    let mut changed = false;
    let ancestors =
        topology.groups_containing_endpoint(&graph.groups[group_index].id, resources)?;

    for index in 0..placements.len() {
        if member_indices.binary_search(&index).is_ok() {
            continue;
        }
        if !raw_bounds_intersects(group_bounds, node_bounds(placements[index], resources)?) {
            continue;
        }

        let mut candidate = topology.direct_node_group_index(&graph.nodes[index].id);
        let mut external_group = None;
        for _ in 0..graph.groups.len() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            let Some(group) = candidate else { break };
            if group == group_index || ancestors.contains(&group) {
                break;
            }
            external_group = Some(group);
            candidate = topology.parent_group_index(group);
        }
        // Move the complete external branch below the common scope. Shifting only the
        // collided member would split a sibling group or move part of a nested subtree.
        let block = external_group
            .map(|group| topology.group_member_node_indices(group, resources))
            .transpose()?;
        let block = block.as_deref().unwrap_or(std::slice::from_ref(&index));
        while block_overlaps_group_or_nodes(block, group_bounds, placements, resources)? {
            resources.charge_layout_work(1)?;
            changed = true;
            match graph_direction {
                GraphDirection::LeftRight => {
                    shift_member_indices_y(placements, block, 4, resources)?;
                }
                GraphDirection::TopDown => {
                    shift_member_indices_x(placements, block, 4, resources)?;
                }
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }
    }

    Ok(changed)
}

fn block_overlaps_group_or_nodes(
    members: &[usize],
    group_bounds: RawBounds,
    placements: &[GridCoord],
    resources: &ResourceContext,
) -> Result<bool> {
    // Charge the target-bound calculation and each actual comparison before doing the geometry
    // work. This keeps the O(N) scan visible to the render-wide layout-work budget and makes a
    // rejected comparison side-effect free for callers that may mutate placements afterwards.
    resources.charge_layout_work(members.len())?;
    let Some(bounds) = member_grid_bounds(members, placements, resources)? else {
        return Ok(false);
    };
    if raw_bounds_intersects(group_bounds, bounds) {
        return Ok(true);
    }
    for (other_index, other_coord) in placements.iter().enumerate() {
        if members.binary_search(&other_index).is_ok() {
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
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::AsciiResourcePolicy;
    use crate::{AsciiLayoutProfile, AsciiRenderOptions};
    use merman_model::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    fn concurrency_graph(direction: GraphDirection, populated: &[bool]) -> AsciiGraph {
        let mut graph = AsciiGraph::new(direction);
        let mut sections = Vec::new();
        for (index, populated) in populated.iter().copied().enumerate() {
            let id = format!("section_{index}");
            let members = if populated {
                let node = format!("node_{index}");
                graph.add_node(&node, &node);
                vec![node]
            } else {
                Vec::new()
            };
            graph.add_group_with_kind_and_style(
                &id,
                "",
                None,
                members,
                GraphGroupKind::Divider,
                GraphGroupStyle::default(),
            );
            sections.push(id);
        }
        graph.add_group_with_style("RUN", "RUN", None, sections, GraphGroupStyle::default());
        graph
    }

    fn assert_sections_inside_parent(layout: &super::super::GraphLayout, ids: &[&str]) {
        let parent = layout
            .groups
            .iter()
            .find(|group| group.id == "RUN")
            .unwrap();
        let mut previous_bottom = None;
        for id in ids {
            let section = layout.groups.iter().find(|group| group.id == *id).unwrap();
            assert!(parent.x < section.x && section.right() < parent.right());
            assert!(parent.y < section.y && section.bottom() < parent.bottom());
            if let Some(bottom) = previous_bottom {
                assert!(
                    bottom + 1 < section.y,
                    "ordered sections need a clear row: {id}: {layout:?}"
                );
            }
            previous_bottom = Some(section.bottom());
        }
    }

    #[test]
    fn empty_divider_sections_keep_vertical_order_in_every_root_direction() {
        for direction in [
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
        ] {
            for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                for populated in [
                    &[false, true][..],
                    &[true, false][..],
                    &[true, false, false, true][..],
                    &[false, false, false][..],
                ] {
                    let graph = concurrency_graph(direction, populated);
                    let mut resources = unbounded_resources();
                    let layout = super::super::layout_graph_with_resources(
                        &graph,
                        &AsciiRenderOptions::default().with_layout_profile(profile),
                        &mut resources,
                    )
                    .expect("empty sections should share the ordinary compound projection");
                    let ids = (0..populated.len())
                        .map(|index| format!("section_{index}"))
                        .collect::<Vec<_>>();
                    let ids = ids.iter().map(String::as_str).collect::<Vec<_>>();
                    assert_sections_inside_parent(&layout, &ids);
                    for (index, populated) in populated.iter().copied().enumerate() {
                        if populated {
                            let id = format!("node_{index}");
                            let node = layout.nodes.iter().find(|node| node.id == id).unwrap();
                            let section_id = format!("section_{index}");
                            let section = layout
                                .groups
                                .iter()
                                .find(|group| group.id == section_id)
                                .unwrap();
                            assert!(section.y < node.y && node.bottom() <= section.bottom());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn empty_divider_subtrees_reserve_their_full_height_between_nested_siblings() {
        for direction in [
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
        ] {
            let mut graph = AsciiGraph::new(direction);
            for id in ["a", "b", "outside"] {
                graph.add_node(id, id);
            }
            for (id, title, members) in [
                ("leaf", "Empty\nsection\nwith\na\ntall\nlabel", vec![]),
                ("shell", "Shell", vec!["leaf".into()]),
                ("before_body", "Before", vec!["a".into()]),
                ("after_body", "After", vec!["b".into()]),
                ("neighbor", "Neighbor", vec!["outside".into()]),
            ] {
                graph.add_group_with_style(id, title, None, members, GraphGroupStyle::default());
            }
            for (id, members) in [
                ("leading", vec![]),
                ("before", vec!["before_body".into()]),
                ("empty", vec!["shell".into()]),
                ("after", vec!["after_body".into()]),
                ("trailing", vec![]),
            ] {
                graph.add_group_with_kind_and_style(
                    id,
                    "",
                    None,
                    members,
                    GraphGroupKind::Divider,
                    GraphGroupStyle::default(),
                );
            }
            graph.add_group_with_style(
                "RUN",
                "RUN",
                None,
                ["leading", "before", "empty", "after", "trailing"]
                    .map(String::from)
                    .to_vec(),
                GraphGroupStyle::default(),
            );
            graph.add_group_with_style(
                "outer",
                "Outer",
                None,
                vec!["RUN".into(), "neighbor".into()],
                GraphGroupStyle::default(),
            );
            let mut resources = unbounded_resources();
            let layout = super::super::layout_graph_with_resources(
                &graph,
                &AsciiRenderOptions::default(),
                &mut resources,
            )
            .expect("empty nested sections must reserve measured space between populated ones");
            assert_sections_inside_parent(
                &layout,
                &["leading", "before", "empty", "after", "trailing"],
            );
            let group = |id: &str| layout.groups.iter().find(|group| group.id == id).unwrap();
            for (parent, child) in [
                ("shell", "leaf"),
                ("empty", "shell"),
                ("before", "before_body"),
                ("after", "after_body"),
                ("outer", "RUN"),
                ("outer", "neighbor"),
            ] {
                let (parent, child) = (group(parent), group(child));
                assert!(parent.x < child.x && child.right() < parent.right());
                assert!(parent.y < child.y && child.bottom() <= parent.bottom());
            }
            let (run, neighbor) = (group("RUN"), group("neighbor"));
            assert!(
                run.right() + 1 < neighbor.x
                    || neighbor.right() + 1 < run.x
                    || run.bottom() + 1 < neighbor.y
                    || neighbor.bottom() + 1 < run.y,
                "empty section envelopes must stay clear of external siblings: {layout:?}"
            );
        }
    }

    #[test]
    fn external_separator_reservation_moves_complete_sibling_subtrees() {
        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            let mut graph = AsciiGraph::new(direction);
            for id in ["g", "h0", "h1"] {
                graph.add_node(id, id);
            }
            for (id, members) in [
                ("G", vec!["g".into()]),
                ("H", vec!["h0".into(), "h1".into()]),
                ("outer", vec!["G".into(), "H".into()]),
            ] {
                graph.add_group_with_style(id, id, None, members, GraphGroupStyle::default());
            }
            graph.add_edge("G", "H");
            let mut resources = unbounded_resources();
            let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
            let mut placements = [
                GridCoord { x: 0, y: 0 },
                GridCoord { x: 3, y: 0 },
                GridCoord { x: 7, y: 0 },
            ];
            if direction == GraphDirection::TopDown {
                for coord in &mut placements {
                    std::mem::swap(&mut coord.x, &mut coord.y);
                }
            }
            let original = placements;
            separate_external_nodes_from_groups(&graph, &mut placements, &topology, &mut resources)
                .unwrap();
            assert_eq!(placements[0], original[0]);
            assert_eq!(
                placements[2].x - placements[1].x,
                original[2].x - original[1].x
            );
            assert_eq!(
                placements[2].y - placements[1].y,
                original[2].y - original[1].y
            );
            assert_ne!(
                placements[1], original[1],
                "the adjacent block needs a separator"
            );
        }
    }

    #[test]
    fn separated_group_endpoints_do_not_repeatedly_shift_their_members() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("x", "X");
        graph.add_node("y", "Y");
        for (id, node) in [("source", "x"), ("target", "y")] {
            graph.add_group_with_style(id, id, None, vec![node.into()], GraphGroupStyle::default());
        }
        graph.add_edge("source", "target");
        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        for original in [
            [GridCoord { x: 0, y: 0 }, GridCoord { x: 4, y: 0 }],
            [GridCoord { x: 0, y: 0 }, GridCoord { x: 0, y: 4 }],
        ] {
            let mut placements = original;
            separate_external_nodes_from_groups(&graph, &mut placements, &topology, &mut resources)
                .unwrap();
            assert_eq!(
                placements, original,
                "frame clearance must be resolved by projection"
            );
        }
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
    fn block_overlap_scan_accepts_exact_work_and_rejects_max_minus_one() {
        let placements = [
            GridCoord { x: 0, y: 0 },
            GridCoord { x: 10, y: 0 },
            GridCoord { x: 20, y: 0 },
        ];
        let group_bounds = RawBounds {
            x: 30,
            y: 30,
            right: 32,
            bottom: 32,
        };
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let measured_resources = ResourceContext::new(unbounded);
        assert!(
            !block_overlaps_group_or_nodes(&[0], group_bounds, &placements, &measured_resources)
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
            !block_overlaps_group_or_nodes(&[0], group_bounds, &placements, &exact_resources)
                .expect("exact overlap-scan budget should pass")
        );
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one overlap-scan budget should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let error =
            block_overlaps_group_or_nodes(&[0], group_bounds, &placements, &below_resources)
                .expect_err("max-minus-one overlap-scan budget should reject");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }
}
