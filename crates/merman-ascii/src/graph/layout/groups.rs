use super::super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphGroup, AsciiGraphNode, GraphDirection, GraphEdgeMarker,
    GraphEdgeStroke, GraphEdgeStyle, GraphGroupKind, GraphNodeShape, GraphNodeSide, GraphNodeStyle,
};
use super::super::topology::{GraphEndpointIndex, GraphGroupTopology};
use super::{GridCoord, GroupLayout, NodeLayout};
use crate::error::Result;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::{HashMap, HashSet};

mod bounds;

use self::bounds::{RawBounds, raw_group_bounds_for_members};

pub(super) fn apply_group_placement_adjustments(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<()> {
    let original_placements = clone_grid_placements(placements, resources)?;
    let original_root_axis = root_axis_positions(graph.direction, placements, resources)?;
    let endpoint_index =
        GroupEndpointIndex::try_new(graph, topology, &original_root_axis, resources)?;
    let mut disabled_overrides = try_bool_slots(graph.groups.len())?;
    disable_flowchart_external_connection_overrides(
        graph,
        topology,
        &mut disabled_overrides,
        resources,
    )?;
    let placement_context = GroupPlacementContext {
        graph,
        topology,
        width_profile,
        original_placements: &original_placements,
        original_root_axis: &original_root_axis,
        endpoint_index: &endpoint_index,
    };
    let mut placement_state = solve_group_placement_constraints(
        &placement_context,
        placements,
        &mut disabled_overrides,
        resources,
    )?;

    separate_placement_blocks_on_cross_axis(
        graph.direction,
        placements,
        &placement_state.blocks,
        resources,
    )?;
    reserve_group_left_constraint_space(graph, placements, topology, width_profile, resources)?;
    separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;

    if !placement_state_is_valid(
        graph.direction,
        placements,
        &placement_state.invariants,
        resources,
    )? {
        disable_all_group_overrides(graph, &mut disabled_overrides, resources)?;
        placement_state = solve_group_placement_constraints(
            &placement_context,
            placements,
            &mut disabled_overrides,
            resources,
        )?;
        separate_placement_blocks_on_cross_axis(
            graph.direction,
            placements,
            &placement_state.blocks,
            resources,
        )?;
        reserve_group_left_constraint_space(graph, placements, topology, width_profile, resources)?;
        separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;
    }

    if !placement_state_is_valid(
        graph.direction,
        placements,
        &placement_state.invariants,
        resources,
    )? {
        restore_grid_placements(placements, &original_placements, resources)?;
        separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;
    }

    Ok(())
}

fn disable_flowchart_external_connection_overrides(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.diagram_type() != "flowchart" {
        return Ok(());
    }

    for edge in &graph.edges {
        let mut source_scope = topology.groups_containing_endpoint(&edge.from, resources)?;
        let mut target_scope = topology.groups_containing_endpoint(&edge.to, resources)?;
        include_group_endpoint_scope(topology, &edge.from, &mut source_scope)?;
        include_group_endpoint_scope(topology, &edge.to, &mut target_scope)?;

        resources.charge_layout_work(graph.groups.len())?;
        for group_index in 0..graph.groups.len() {
            if source_scope.contains(&group_index) ^ target_scope.contains(&group_index)
                && let Some(disabled) = disabled_overrides.get_mut(group_index)
            {
                *disabled = true;
            }
        }
    }

    Ok(())
}

fn include_group_endpoint_scope(
    topology: &GraphGroupTopology<'_>,
    endpoint: &str,
    scope: &mut HashSet<usize>,
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

fn reserve_group_left_constraint_space(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<()> {
    if graph.direction.canonical() != GraphDirection::TopDown {
        return Ok(());
    }

    for (group_index, group) in graph.groups.iter().enumerate() {
        resources.charge_layout_work(graph.nodes.len())?;
        let mut fixed_left_nodes = try_bool_slots(graph.nodes.len())?;
        let mut furthest_left_note_right = None::<isize>;
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let is_left_constraint =
                node.semantics
                    .side_constraint
                    .as_ref()
                    .is_some_and(|constraint| {
                        constraint.anchor_id() == group.id
                            && constraint.side() == GraphNodeSide::Left
                    });
            if !is_left_constraint {
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

fn clone_grid_placements(
    placements: &[GridCoord],
    resources: &mut ResourceContext,
) -> Result<Vec<GridCoord>> {
    resources.charge_layout_work(placements.len())?;
    let mut cloned = Vec::new();
    cloned
        .try_reserve(placements.len())
        .map_err(|_| layout_work_allocation_failed())?;
    cloned.extend_from_slice(placements);
    Ok(cloned)
}

fn restore_grid_placements(
    placements: &mut [GridCoord],
    original_placements: &[GridCoord],
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(placements.len())?;
    for (placement, original) in placements.iter_mut().zip(original_placements) {
        *placement = *original;
    }
    Ok(())
}

fn root_axis_positions(
    direction: GraphDirection,
    placements: &[GridCoord],
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    resources.charge_layout_work(placements.len())?;
    let mut positions = Vec::new();
    positions
        .try_reserve(placements.len())
        .map_err(|_| layout_work_allocation_failed())?;
    positions.extend(
        placements
            .iter()
            .map(|placement| root_axis_position(direction, *placement)),
    );
    Ok(positions)
}

struct GroupPlacementState {
    blocks: PlacementBlocks,
    invariants: Vec<RankInvariant>,
}

struct GroupPlacementContext<'context, 'graph> {
    graph: &'context AsciiGraph,
    topology: &'context GraphGroupTopology<'graph>,
    width_profile: TerminalWidthProfile,
    original_placements: &'context [GridCoord],
    original_root_axis: &'context [usize],
    endpoint_index: &'context GroupEndpointIndex,
}

fn solve_group_placement_constraints(
    context: &GroupPlacementContext<'_, '_>,
    placements: &mut [GridCoord],
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
) -> Result<GroupPlacementState> {
    // Rebuild each attempt from the Dagre placement. A conflicting rigid-block cycle therefore
    // disables only the implicated local override instead of accumulating partial shifts.
    let maximum_attempts = resources.checked_work_add(context.graph.groups.len(), 1)?;
    for _ in 0..maximum_attempts {
        resources.charge_layout_work(1)?;
        restore_grid_placements(placements, context.original_placements, resources)?;
        apply_subgraph_direction_overrides(
            context.graph,
            placements,
            context.topology,
            context.width_profile,
            disabled_overrides,
            resources,
        )?;
        stack_divider_sections(context.graph, placements, context.topology, resources)?;

        let blocks = PlacementBlocks::try_new(
            context.graph,
            context.topology,
            disabled_overrides,
            resources,
        )?;
        let (constraints, invariants) = build_block_constraints(
            context.graph,
            placements,
            context.topology,
            context.original_root_axis,
            context.endpoint_index,
            &blocks,
            resources,
        )?;
        match solve_block_offsets(blocks.blocks.len(), &constraints, resources)? {
            BlockOffsetSolution::Offsets(offsets) => {
                apply_block_offsets(
                    context.graph.direction,
                    placements,
                    &blocks,
                    &offsets,
                    resources,
                )?;
                return Ok(GroupPlacementState { blocks, invariants });
            }
            BlockOffsetSolution::PositiveCycle(cycle) => {
                if !disable_conflicting_group_overrides(
                    &blocks,
                    &cycle,
                    disabled_overrides,
                    resources,
                )? {
                    disable_all_group_overrides(context.graph, disabled_overrides, resources)?;
                }
            }
        }
    }

    restore_grid_placements(placements, context.original_placements, resources)?;
    let blocks = PlacementBlocks::try_new(
        context.graph,
        context.topology,
        disabled_overrides,
        resources,
    )?;
    Ok(GroupPlacementState {
        blocks,
        invariants: Vec::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlacementBlockId {
    Group(usize),
    Node,
}

#[derive(Debug)]
struct PlacementBlock {
    id: PlacementBlockId,
    members: Vec<usize>,
}

#[derive(Debug)]
struct PlacementBlocks {
    blocks: Vec<PlacementBlock>,
    block_by_node: Vec<usize>,
}

impl PlacementBlocks {
    fn try_new(
        graph: &AsciiGraph,
        topology: &GraphGroupTopology<'_>,
        disabled_overrides: &[bool],
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let movement_groups =
            movement_groups_by_node(graph, topology, disabled_overrides, resources)?;
        let mut block_by_group = Vec::new();
        block_by_group
            .try_reserve(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;
        block_by_group.resize(graph.groups.len(), None::<usize>);

        let mut blocks = Vec::new();
        blocks
            .try_reserve(graph.nodes.len())
            .map_err(|_| layout_work_allocation_failed())?;
        let mut block_by_node = Vec::new();
        block_by_node
            .try_reserve(graph.nodes.len())
            .map_err(|_| layout_work_allocation_failed())?;

        for (node_index, movement_group) in movement_groups.into_iter().enumerate() {
            resources.charge_layout_work(1)?;
            let block_index = if let Some(group_index) = movement_group {
                if let Some(block_index) = block_by_group[group_index] {
                    block_index
                } else {
                    let block_index = blocks.len();
                    blocks.push(PlacementBlock {
                        id: PlacementBlockId::Group(group_index),
                        members: Vec::new(),
                    });
                    block_by_group[group_index] = Some(block_index);
                    block_index
                }
            } else {
                let block_index = blocks.len();
                blocks.push(PlacementBlock {
                    id: PlacementBlockId::Node,
                    members: Vec::new(),
                });
                block_index
            };
            if let Some(block) = blocks.get_mut(block_index) {
                block
                    .members
                    .try_reserve(1)
                    .map_err(|_| layout_work_allocation_failed())?;
                block.members.push(node_index);
            }
            block_by_node.push(block_index);
        }

        Ok(Self {
            blocks,
            block_by_node,
        })
    }
}

fn movement_groups_by_node(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    disabled_overrides: &[bool],
    resources: &mut ResourceContext,
) -> Result<Vec<Option<usize>>> {
    let mut movement_groups = Vec::new();
    movement_groups
        .try_reserve(graph.nodes.len())
        .map_err(|_| layout_work_allocation_failed())?;
    for node in &graph.nodes {
        let mut selected = None;
        let mut current = topology.direct_node_group_index(&node.id);
        for _ in 0..graph.groups.len() {
            resources.charge_layout_work(1)?;
            let Some(group_index) = current else {
                break;
            };
            if graph.groups[group_index].direction.is_some()
                && !disabled_overrides
                    .get(group_index)
                    .copied()
                    .unwrap_or(false)
            {
                selected = Some(group_index);
            }
            current = topology.parent_group_index(group_index);
        }
        movement_groups.push(selected);
    }
    Ok(movement_groups)
}

#[derive(Debug, Clone, Copy)]
enum EndpointRole {
    Source,
    Target,
}

struct GroupEndpointIndex {
    source_member_by_group: Vec<Option<usize>>,
    target_member_by_group: Vec<Option<usize>>,
}

impl GroupEndpointIndex {
    fn try_new(
        graph: &AsciiGraph,
        topology: &GraphGroupTopology<'_>,
        original_root_axis: &[usize],
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let mut source_member_by_group = Vec::new();
        source_member_by_group
            .try_reserve(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;
        let mut target_member_by_group = Vec::new();
        target_member_by_group
            .try_reserve(graph.groups.len())
            .map_err(|_| layout_work_allocation_failed())?;

        for group_index in 0..graph.groups.len() {
            let members = topology.group_member_node_indices(group_index, resources)?;
            resources.charge_layout_work(members.len())?;
            // A group endpoint is represented by the member nearest its semantic boundary on the
            // original root axis. The same member supplies the post-override local coordinate.
            let source = members.iter().copied().max_by(|left, right| {
                group_endpoint_member_cmp(graph, original_root_axis, *left, *right)
            });
            let target = members.iter().copied().min_by(|left, right| {
                group_endpoint_member_cmp(graph, original_root_axis, *left, *right)
            });
            source_member_by_group.push(source);
            target_member_by_group.push(target);
        }

        Ok(Self {
            source_member_by_group,
            target_member_by_group,
        })
    }

    fn resolve(&self, endpoint: GraphEndpointIndex, role: EndpointRole) -> Option<usize> {
        match endpoint {
            GraphEndpointIndex::Node(node_index) => Some(node_index),
            GraphEndpointIndex::Group(group_index) => match role {
                EndpointRole::Source => self
                    .source_member_by_group
                    .get(group_index)
                    .copied()
                    .flatten(),
                EndpointRole::Target => self
                    .target_member_by_group
                    .get(group_index)
                    .copied()
                    .flatten(),
            },
        }
    }
}

fn group_endpoint_member_cmp(
    graph: &AsciiGraph,
    original_root_axis: &[usize],
    left: usize,
    right: usize,
) -> std::cmp::Ordering {
    original_root_axis
        .get(left)
        .copied()
        .unwrap_or_default()
        .cmp(&original_root_axis.get(right).copied().unwrap_or_default())
        .then_with(|| {
            graph
                .nodes
                .get(left)
                .map(|node| node.id.as_str())
                .unwrap_or_default()
                .cmp(
                    graph
                        .nodes
                        .get(right)
                        .map(|node| node.id.as_str())
                        .unwrap_or_default(),
                )
        })
}

#[derive(Debug, Clone, Copy)]
struct RawBlockConstraint {
    source_block: usize,
    target_block: usize,
    minimum_offset_delta: i128,
}

#[derive(Debug, Clone, Copy)]
struct BlockConstraint {
    source_block: usize,
    target_block: usize,
    minimum_offset_delta: i128,
}

#[derive(Debug, Clone, Copy)]
struct RankInvariant {
    source_node: usize,
    target_node: usize,
    minimum_gap: usize,
}

fn build_block_constraints(
    graph: &AsciiGraph,
    placements: &[GridCoord],
    topology: &GraphGroupTopology<'_>,
    original_root_axis: &[usize],
    endpoint_index: &GroupEndpointIndex,
    blocks: &PlacementBlocks,
    resources: &mut ResourceContext,
) -> Result<(Vec<BlockConstraint>, Vec<RankInvariant>)> {
    let mut raw_constraints = Vec::new();
    raw_constraints
        .try_reserve(graph.edges.len())
        .map_err(|_| layout_work_allocation_failed())?;
    let mut invariants = Vec::new();
    invariants
        .try_reserve(graph.edges.len())
        .map_err(|_| layout_work_allocation_failed())?;

    for edge in &graph.edges {
        resources.charge_layout_work(1)?;
        let (Some(source_endpoint), Some(target_endpoint)) = (
            topology.endpoint_index(&edge.from),
            topology.endpoint_index(&edge.to),
        ) else {
            continue;
        };
        let (Some(source_node), Some(target_node)) = (
            endpoint_index.resolve(source_endpoint, EndpointRole::Source),
            endpoint_index.resolve(target_endpoint, EndpointRole::Target),
        ) else {
            continue;
        };
        let (Some(original_source), Some(original_target)) = (
            original_root_axis.get(source_node).copied(),
            original_root_axis.get(target_node).copied(),
        ) else {
            continue;
        };
        let Some(minimum_gap) = original_target.checked_sub(original_source) else {
            continue;
        };
        if minimum_gap == 0 {
            continue;
        }
        let (Some(source_block), Some(target_block)) = (
            blocks.block_by_node.get(source_node).copied(),
            blocks.block_by_node.get(target_node).copied(),
        ) else {
            continue;
        };
        if source_block == target_block {
            continue;
        }
        let (Some(source_placement), Some(target_placement)) = (
            placements.get(source_node).copied(),
            placements.get(target_node).copied(),
        ) else {
            continue;
        };
        raw_constraints.push(RawBlockConstraint {
            source_block,
            target_block,
            // target + target_offset >= source + source_offset + original_gap
            minimum_offset_delta: block_constraint_delta(
                minimum_gap,
                root_axis_position(graph.direction, source_placement),
                root_axis_position(graph.direction, target_placement),
                resources,
            )?,
        });
        invariants.push(RankInvariant {
            source_node,
            target_node,
            minimum_gap,
        });
    }

    charge_sort_work(raw_constraints.len(), resources)?;
    raw_constraints
        .sort_unstable_by_key(|constraint| (constraint.source_block, constraint.target_block));
    let mut constraints: Vec<BlockConstraint> = Vec::new();
    constraints
        .try_reserve(raw_constraints.len())
        .map_err(|_| layout_work_allocation_failed())?;
    for raw in raw_constraints {
        if let Some(last) = constraints.last_mut()
            && last.source_block == raw.source_block
            && last.target_block == raw.target_block
        {
            last.minimum_offset_delta = last.minimum_offset_delta.max(raw.minimum_offset_delta);
            continue;
        }
        constraints.push(BlockConstraint {
            source_block: raw.source_block,
            target_block: raw.target_block,
            minimum_offset_delta: raw.minimum_offset_delta,
        });
    }

    Ok((constraints, invariants))
}

fn block_constraint_delta(
    minimum_gap: usize,
    source_axis: usize,
    target_axis: usize,
    resources: &ResourceContext,
) -> Result<i128> {
    let minimum_gap = i128::try_from(minimum_gap)
        .map_err(|_| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
    let source_axis = i128::try_from(source_axis)
        .map_err(|_| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
    let target_axis = i128::try_from(target_axis)
        .map_err(|_| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
    minimum_gap
        .checked_add(source_axis)
        .and_then(|value| value.checked_sub(target_axis))
        .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxGridCells))
}

enum BlockOffsetSolution {
    Offsets(Vec<usize>),
    PositiveCycle(Vec<usize>),
}

fn solve_block_offsets(
    block_count: usize,
    constraints: &[BlockConstraint],
    resources: &mut ResourceContext,
) -> Result<BlockOffsetSolution> {
    // Maximizing Bellman-Ford solves lower-bound difference constraints, including consistent
    // non-positive cycles. An update on the final pass yields a positive-cycle witness for the
    // local-direction fallback path.
    let mut offsets = Vec::new();
    offsets
        .try_reserve(block_count)
        .map_err(|_| layout_work_allocation_failed())?;
    offsets.resize(block_count, 0_i128);
    let mut predecessors = Vec::new();
    predecessors
        .try_reserve(block_count)
        .map_err(|_| layout_work_allocation_failed())?;
    predecessors.resize(block_count, None::<usize>);

    for pass in 0..block_count {
        let mut changed_node = None;
        for constraint in constraints {
            resources.charge_layout_work(1)?;
            let candidate = offsets[constraint.source_block]
                .checked_add(constraint.minimum_offset_delta)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
            if candidate <= offsets[constraint.target_block] {
                continue;
            }
            offsets[constraint.target_block] = candidate;
            predecessors[constraint.target_block] = Some(constraint.source_block);
            changed_node = Some(constraint.target_block);
        }
        let Some(changed_node) = changed_node else {
            return Ok(BlockOffsetSolution::Offsets(usize_offsets(
                &offsets, resources,
            )?));
        };
        if pass + 1 == block_count {
            return Ok(BlockOffsetSolution::PositiveCycle(positive_cycle_witness(
                changed_node,
                &predecessors,
                resources,
            )?));
        }
    }

    Ok(BlockOffsetSolution::Offsets(usize_offsets(
        &offsets, resources,
    )?))
}

fn usize_offsets(offsets: &[i128], resources: &ResourceContext) -> Result<Vec<usize>> {
    resources.charge_layout_work(offsets.len())?;
    let mut converted = Vec::new();
    converted
        .try_reserve(offsets.len())
        .map_err(|_| layout_work_allocation_failed())?;
    for offset in offsets {
        converted.push(
            usize::try_from(*offset)
                .map_err(|_| resources.overflow(AsciiResourceLimitId::MaxGridCells))?,
        );
    }
    Ok(converted)
}

fn positive_cycle_witness(
    changed_node: usize,
    predecessors: &[Option<usize>],
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    let mut cursor = changed_node;
    for _ in 0..predecessors.len() {
        resources.charge_layout_work(1)?;
        let Some(predecessor) = predecessors.get(cursor).copied().flatten() else {
            return all_block_indices(predecessors.len(), resources);
        };
        cursor = predecessor;
    }

    let cycle_start = cursor;
    let mut cycle = Vec::new();
    cycle
        .try_reserve(predecessors.len())
        .map_err(|_| layout_work_allocation_failed())?;
    loop {
        resources.charge_layout_work(1)?;
        cycle.push(cursor);
        let Some(predecessor) = predecessors.get(cursor).copied().flatten() else {
            return all_block_indices(predecessors.len(), resources);
        };
        cursor = predecessor;
        if cursor == cycle_start {
            break;
        }
        if cycle.len() >= predecessors.len() {
            return all_block_indices(predecessors.len(), resources);
        }
    }
    Ok(cycle)
}

fn all_block_indices(block_count: usize, resources: &mut ResourceContext) -> Result<Vec<usize>> {
    resources.charge_layout_work(block_count)?;
    let mut indices = Vec::new();
    indices
        .try_reserve(block_count)
        .map_err(|_| layout_work_allocation_failed())?;
    indices.extend(0..block_count);
    Ok(indices)
}

fn disable_conflicting_group_overrides(
    blocks: &PlacementBlocks,
    cycle: &[usize],
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
) -> Result<bool> {
    let mut changed = false;
    resources.charge_layout_work(cycle.len())?;
    for block_index in cycle {
        let Some(PlacementBlock {
            id: PlacementBlockId::Group(group_index),
            ..
        }) = blocks.blocks.get(*block_index)
        else {
            continue;
        };
        if let Some(disabled) = disabled_overrides.get_mut(*group_index)
            && !*disabled
        {
            *disabled = true;
            changed = true;
        }
    }
    Ok(changed)
}

fn disable_all_group_overrides(
    graph: &AsciiGraph,
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(graph.groups.len())?;
    for (group_index, group) in graph.groups.iter().enumerate() {
        if group.direction.is_some()
            && let Some(disabled) = disabled_overrides.get_mut(group_index)
        {
            *disabled = true;
        }
    }
    Ok(())
}

fn apply_block_offsets(
    direction: GraphDirection,
    placements: &mut [GridCoord],
    blocks: &PlacementBlocks,
    offsets: &[usize],
    resources: &mut ResourceContext,
) -> Result<()> {
    for (block_index, block) in blocks.blocks.iter().enumerate() {
        let offset = offsets.get(block_index).copied().unwrap_or_default();
        resources.charge_layout_work(block.members.len())?;
        if offset == 0 {
            continue;
        }
        for member in &block.members {
            if let Some(placement) = placements.get_mut(*member) {
                shift_root_axis(placement, direction, offset, resources)?;
            }
        }
    }
    Ok(())
}

fn separate_placement_blocks_on_cross_axis(
    direction: GraphDirection,
    placements: &mut [GridCoord],
    blocks: &PlacementBlocks,
    resources: &mut ResourceContext,
) -> Result<()> {
    for current_block_index in 0..blocks.blocks.len() {
        let current_block = &blocks.blocks[current_block_index];
        let maximum_passes = blocks.blocks[..current_block_index]
            .iter()
            .try_fold(1usize, |passes, block| {
                resources.checked_work_add(passes, block.members.len())
            })?;
        for _ in 0..maximum_passes {
            let mut required_shift = 0;
            for previous_block in &blocks.blocks[..current_block_index] {
                required_shift = required_shift.max(required_cross_axis_shift(
                    direction,
                    placements,
                    &current_block.members,
                    &previous_block.members,
                    resources,
                )?);
            }
            if required_shift == 0 {
                break;
            }
            shift_block_cross_axis(
                direction,
                placements,
                &current_block.members,
                required_shift,
                resources,
            )?;
        }
    }
    Ok(())
}

fn required_cross_axis_shift(
    direction: GraphDirection,
    placements: &[GridCoord],
    current_members: &[usize],
    previous_members: &[usize],
    resources: &ResourceContext,
) -> Result<usize> {
    const NODE_GRID_SPAN: usize = 3;

    resources.charge_layout_work_product(current_members.len(), previous_members.len())?;
    let mut required_shift = 0;
    for current_member in current_members {
        let Some(current) = placements.get(*current_member).copied() else {
            continue;
        };
        for previous_member in previous_members {
            let Some(previous) = placements.get(*previous_member).copied() else {
                continue;
            };
            match direction.canonical() {
                GraphDirection::LeftRight => {
                    let current_right = resources.checked_grid_add(current.x, 2)?;
                    let previous_right = resources.checked_grid_add(previous.x, 2)?;
                    let current_bottom = resources.checked_grid_add(current.y, 2)?;
                    let previous_bottom = resources.checked_grid_add(previous.y, 2)?;
                    if current.x <= previous_right
                        && previous.x <= current_right
                        && current.y <= previous_bottom
                        && previous.y <= current_bottom
                    {
                        let minimum_y = resources.checked_grid_add(previous.y, NODE_GRID_SPAN)?;
                        if current.y < minimum_y {
                            required_shift = required_shift.max(minimum_y - current.y);
                        }
                    }
                }
                GraphDirection::TopDown => {
                    let current_right = resources.checked_grid_add(current.x, 2)?;
                    let previous_right = resources.checked_grid_add(previous.x, 2)?;
                    let current_bottom = resources.checked_grid_add(current.y, 2)?;
                    let previous_bottom = resources.checked_grid_add(previous.y, 2)?;
                    if current.x <= previous_right
                        && previous.x <= current_right
                        && current.y <= previous_bottom
                        && previous.y <= current_bottom
                    {
                        let minimum_x = resources.checked_grid_add(previous.x, NODE_GRID_SPAN)?;
                        if current.x < minimum_x {
                            required_shift = required_shift.max(minimum_x - current.x);
                        }
                    }
                }
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }
    }
    Ok(required_shift)
}

fn shift_block_cross_axis(
    direction: GraphDirection,
    placements: &mut [GridCoord],
    members: &[usize],
    delta: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(members.len())?;
    for member in members {
        let Some(placement) = placements.get_mut(*member) else {
            continue;
        };
        match direction.canonical() {
            GraphDirection::LeftRight => {
                placement.y = resources.checked_grid_add(placement.y, delta)?;
            }
            GraphDirection::TopDown => {
                placement.x = resources.checked_grid_add(placement.x, delta)?;
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }
    Ok(())
}

fn placement_state_is_valid(
    direction: GraphDirection,
    placements: &[GridCoord],
    invariants: &[RankInvariant],
    resources: &mut ResourceContext,
) -> Result<bool> {
    resources.charge_layout_work(invariants.len())?;
    for invariant in invariants {
        let (Some(source), Some(target)) = (
            placements.get(invariant.source_node).copied(),
            placements.get(invariant.target_node).copied(),
        ) else {
            return Ok(false);
        };
        let required_target = resources
            .checked_grid_add(root_axis_position(direction, source), invariant.minimum_gap)?;
        if root_axis_position(direction, target) < required_target {
            return Ok(false);
        }
    }

    for left_index in 0..placements.len() {
        for right_index in resources.checked_work_add(left_index, 1)?..placements.len() {
            resources.charge_layout_work(1)?;
            if raw_bounds_intersects(
                node_bounds(placements[left_index], resources)?,
                node_bounds(placements[right_index], resources)?,
            ) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn charge_sort_work(len: usize, resources: &ResourceContext) -> Result<()> {
    let comparison_height = if len <= 1 {
        1
    } else {
        usize::BITS as usize - (len - 1).leading_zeros() as usize
    };
    resources.charge_layout_work(resources.checked_work_mul(len, comparison_height)?)
}

fn root_axis_position(direction: GraphDirection, placement: GridCoord) -> usize {
    match direction.canonical() {
        GraphDirection::LeftRight => placement.x,
        GraphDirection::TopDown => placement.y,
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn shift_root_axis(
    placement: &mut GridCoord,
    direction: GraphDirection,
    delta: usize,
    resources: &ResourceContext,
) -> Result<()> {
    match direction.canonical() {
        GraphDirection::LeftRight => {
            placement.x = resources.checked_grid_add(placement.x, delta)?;
        }
        GraphDirection::TopDown => {
            placement.y = resources.checked_grid_add(placement.y, delta)?;
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
    Ok(())
}

pub(super) fn subgraph_offsets(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    bounds::subgraph_offsets(graph, layouts, topology, width_profile, resources)
}

pub(super) fn layout_groups(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<GroupLayout>> {
    bounds::layout_groups(graph, layouts, topology, width_profile, resources)
}

pub(super) fn empty_group_minimum_size(
    group: &AsciiGraphGroup,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    bounds::empty_group_minimum_size(group, width_profile, resources)
}

fn apply_subgraph_direction_overrides(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
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
        let Some(direction) = group.direction else {
            continue;
        };
        resources.charge_layout_work(group.nodes.len())?;
        let members = group_placement_members(graph, topology, group_index, resources)?;
        if members.len() < 2 {
            continue;
        }

        let override_graph = build_group_override_graph(graph, topology, &members, resources)?;

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

        let layout_direction = direction.before_root_output_transform(graph.direction);
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

fn separate_external_nodes_from_groups(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
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
                width_profile,
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

        let node_passes = resources.checked_work_mul(graph.nodes.len(), 3)?;
        let work = resources.checked_work_add(
            resources.checked_work_add(node_passes, graph.groups.len())?,
            graph.edges.len(),
        )?;
        resources.charge_layout_work(work)?;

        let mut has_external_incoming = try_bool_slots(graph.nodes.len())?;
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

        let mut has_external_incoming_overhead = try_bool_slots(graph.nodes.len())?;
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

fn try_bool_slots(len: usize) -> Result<Vec<bool>> {
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
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<usize> {
    const SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD: usize = 4;

    if !index
        .has_external_incoming_overhead
        .get(node_index)
        .copied()
        .unwrap_or(false)
    {
        return Ok(options.graph_padding_y);
    }

    resources.checked_grid_add(options.graph_padding_y, SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD)
}

fn graph_endpoint_group_ids<'a>(
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

fn group_member_indices(
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    topology.group_member_node_indices(group_index, resources)
}

fn group_bounds_for_placements(
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
struct GroupPlacementMember {
    id: String,
    endpoint: GraphEndpointIndex,
    node_indices: Vec<usize>,
}

fn group_placement_members(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<GroupPlacementMember>> {
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(Vec::new());
    };

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
                    id: member.clone(),
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
                    id: member.clone(),
                    endpoint: GraphEndpointIndex::Group(child_group_index),
                    node_indices,
                });
            }
            None => {}
        }
    }

    Ok(members)
}

fn build_group_override_graph(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    members: &[GroupPlacementMember],
    resources: &mut ResourceContext,
) -> Result<AsciiGraph> {
    let mut override_graph = AsciiGraph::new_for_diagram(graph.diagram_type(), graph.direction);
    override_graph.root_policy = graph.root_policy;
    override_graph
        .nodes
        .try_reserve(members.len())
        .map_err(|_| crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for member in members {
        resources.charge_layout_work(1)?;
        override_graph.nodes.push(AsciiGraphNode {
            id: member.id.clone(),
            label: member.id.clone(),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            semantics: Default::default(),
        });
    }

    let mut endpoint_to_member = HashMap::<GraphEndpointIndex, usize>::new();
    resources.charge_layout_work(members.len())?;
    let member_node_count = members.iter().try_fold(0usize, |total, member| {
        resources.checked_work_add(total, member.node_indices.len())
    })?;
    let endpoint_capacity = resources.checked_work_add(members.len(), member_node_count)?;
    resources.charge_layout_work(endpoint_capacity)?;
    endpoint_to_member
        .try_reserve(endpoint_capacity)
        .map_err(|_| crate::error::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (member_index, member) in members.iter().enumerate() {
        endpoint_to_member.insert(member.endpoint, member_index);
        for node_index in &member.node_indices {
            endpoint_to_member
                .entry(GraphEndpointIndex::Node(*node_index))
                .or_insert(member_index);
        }
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

fn member_grid_bounds(
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

fn shift_member_indices_y(
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

fn shift_member_indices_x(
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

fn place_group_nodes(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<HashMap<usize, GridCoord>> {
    let ranked = super::grid::place_ranked_grid_nodes_without_group_adjustments(
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
    let bounds = node_bounds(placements[index], resources)?;
    for (other_index, other_coord) in placements.iter().enumerate() {
        if index != other_index
            && raw_bounds_intersects(bounds, node_bounds(*other_coord, resources)?)
        {
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
    use merman_core::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    #[test]
    fn flowchart_external_connection_override_scan_has_an_exact_work_boundary() {
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
        let mut measured_disabled = vec![false; graph.groups.len()];
        disable_flowchart_external_connection_overrides(
            &graph,
            &topology,
            &mut measured_disabled,
            &mut measured_resources,
        )
        .expect("unbounded external-connection scan should pass");
        assert_eq!(measured_disabled, vec![true]);
        let required_work = measured_resources.layout_work_used();
        assert!(required_work > 0);

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, required_work)
            .expect("exact external-connection work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let mut exact_disabled = vec![false; graph.groups.len()];
        disable_flowchart_external_connection_overrides(
            &graph,
            &topology,
            &mut exact_disabled,
            &mut exact_resources,
        )
        .expect("exact external-connection work should pass");
        assert_eq!(exact_disabled, vec![true]);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, required_work - 1)
            .expect("max-minus-one external-connection work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let mut below_disabled = vec![false; graph.groups.len()];
        let error = disable_flowchart_external_connection_overrides(
            &graph,
            &topology,
            &mut below_disabled,
            &mut below_resources,
        )
        .expect_err("max-minus-one external-connection work should reject");
        assert!(matches!(
            error,
            crate::error::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == required_work
                    && details.max == required_work - 1
        ));
    }

    #[test]
    fn positive_block_cycle_returns_a_fallback_witness() {
        let mut resources = unbounded_resources();
        let constraints = [
            BlockConstraint {
                source_block: 0,
                target_block: 1,
                minimum_offset_delta: 1,
            },
            BlockConstraint {
                source_block: 1,
                target_block: 0,
                minimum_offset_delta: 1,
            },
        ];

        let BlockOffsetSolution::PositiveCycle(mut cycle) =
            solve_block_offsets(2, &constraints, &mut resources)
                .expect("positive block cycles should be detected without rejecting the graph")
        else {
            panic!("expected a positive cycle witness");
        };
        cycle.sort_unstable();
        assert_eq!(cycle, vec![0, 1]);
    }

    #[test]
    fn group_endpoints_use_directional_boundary_members() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("early", "Early");
        graph.add_node("late", "Late");
        graph.add_group_with_style(
            "group",
            "Group",
            Some(GraphDirection::LeftRight),
            vec!["early".to_string(), "late".to_string()],
            GraphGroupStyle::default(),
        );
        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources)
            .expect("group topology should be valid");
        let endpoint_index =
            GroupEndpointIndex::try_new(&graph, &topology, &[2, 7], &mut resources)
                .expect("group endpoint representatives should be bounded");
        let endpoint = topology
            .endpoint_index("group")
            .expect("the group endpoint should resolve explicitly");

        assert_eq!(
            endpoint_index.resolve(endpoint, EndpointRole::Source),
            Some(1)
        );
        assert_eq!(
            endpoint_index.resolve(endpoint, EndpointRole::Target),
            Some(0)
        );
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

        let override_graph =
            build_group_override_graph(&graph, &topology, &members, &mut resources)
                .expect("child group endpoint edges should project into the local override graph");

        assert_eq!(override_graph.nodes.len(), 2);
        assert_eq!(override_graph.edges.len(), 1);
        assert_eq!(override_graph.edges[0].from, "left-group");
        assert_eq!(override_graph.edges[0].to, "right-group");
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
}
