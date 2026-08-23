use super::direction::plan_group_direction_overrides;
use super::side_constraints::reserve_group_left_constraint_space;
use super::{
    layout_work_allocation_failed, node_bounds, raw_bounds_intersects,
    separate_external_nodes_from_groups, stack_divider_sections, try_bool_slots,
};
use crate::error::Result;
use crate::graph::layout::{GridCoord, charge_sort_work};
use crate::graph::model::{AsciiGraph, GraphDirection};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_core::OperationPhase;

mod local_direction;

use self::local_direction::apply_subgraph_direction_overrides;
pub(super) fn apply_group_placement_adjustments(
    graph: &AsciiGraph,
    placements: &mut [GridCoord],
    topology: &GraphGroupTopology<'_>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let original_placements = clone_grid_placements(placements, resources)?;
    let original_root_axis = root_axis_positions(graph.direction, placements, resources)?;
    let endpoint_index =
        GroupEndpointIndex::try_new(graph, topology, &original_root_axis, resources)?;
    let direction_overrides = plan_group_direction_overrides(graph, topology, resources)?;
    let mut disabled_overrides = try_bool_slots(graph.groups.len(), resources)?;
    let placement_context = GroupPlacementContext {
        graph,
        topology,
        width_profile,
        direction_overrides: &direction_overrides,
        original_placements: &original_placements,
        original_root_axis: &original_root_axis,
        endpoint_index: &endpoint_index,
    };
    let mut placement_state = solve_group_placement_constraints(
        &placement_context,
        placements,
        &mut disabled_overrides,
        resources,
        execution,
    )?;

    separate_placement_blocks_on_cross_axis(
        graph.direction,
        placements,
        &placement_state.blocks,
        resources,
        execution,
    )?;
    reserve_group_left_constraint_space(graph, placements, topology, width_profile, resources)?;
    separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;

    if !placement_state_is_valid(
        graph.direction,
        placements,
        &placement_state.invariants,
        resources,
        execution,
    )? {
        disable_all_group_overrides(&direction_overrides, &mut disabled_overrides, resources)?;
        placement_state = solve_group_placement_constraints(
            &placement_context,
            placements,
            &mut disabled_overrides,
            resources,
            execution,
        )?;
        separate_placement_blocks_on_cross_axis(
            graph.direction,
            placements,
            &placement_state.blocks,
            resources,
            execution,
        )?;
        reserve_group_left_constraint_space(graph, placements, topology, width_profile, resources)?;
        separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;
    }

    if !placement_state_is_valid(
        graph.direction,
        placements,
        &placement_state.invariants,
        resources,
        execution,
    )? {
        restore_grid_placements(placements, &original_placements, resources)?;
        separate_external_nodes_from_groups(graph, placements, topology, width_profile, resources)?;
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
    placements.copy_from_slice(original_placements);
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
    direction_overrides: &'context [Option<GraphDirection>],
    original_placements: &'context [GridCoord],
    original_root_axis: &'context [usize],
    endpoint_index: &'context GroupEndpointIndex,
}

fn solve_group_placement_constraints(
    context: &GroupPlacementContext<'_, '_>,
    placements: &mut [GridCoord],
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GroupPlacementState> {
    // Rebuild each attempt from the Dagre placement. A conflicting rigid-block cycle therefore
    // disables only the implicated local override instead of accumulating partial shifts.
    let maximum_attempts = resources.checked_work_add(context.graph.groups.len(), 1)?;
    for _ in 0..maximum_attempts {
        resources.charge_layout_work(1)?;
        restore_grid_placements(placements, context.original_placements, resources)?;
        apply_subgraph_direction_overrides(
            context,
            placements,
            disabled_overrides,
            resources,
            execution,
        )?;
        stack_divider_sections(context.graph, placements, context.topology, resources)?;

        let blocks = PlacementBlocks::try_new(
            context.graph,
            context.topology,
            context.direction_overrides,
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
                    disable_all_group_overrides(
                        context.direction_overrides,
                        disabled_overrides,
                        resources,
                    )?;
                }
            }
        }
    }

    restore_grid_placements(placements, context.original_placements, resources)?;
    let blocks = PlacementBlocks::try_new(
        context.graph,
        context.topology,
        context.direction_overrides,
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
        direction_overrides: &[Option<GraphDirection>],
        disabled_overrides: &[bool],
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let movement_groups = movement_groups_by_node(
            graph,
            topology,
            direction_overrides,
            disabled_overrides,
            resources,
        )?;
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
    direction_overrides: &[Option<GraphDirection>],
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
            if direction_overrides
                .get(group_index)
                .copied()
                .flatten()
                .is_some()
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
    direction_overrides: &[Option<GraphDirection>],
    disabled_overrides: &mut [bool],
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(direction_overrides.len())?;
    for (group_index, direction) in direction_overrides.iter().enumerate() {
        if direction.is_some()
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
    execution: AsciiExecution<'_>,
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
                    execution,
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
    execution: AsciiExecution<'_>,
) -> Result<usize> {
    const NODE_GRID_SPAN: usize = 3;

    checkpoint_layout(execution)?;
    resources.charge_layout_work_product(current_members.len(), previous_members.len())?;
    let mut required_shift = 0;
    for current_member in current_members {
        let Some(current) = placements.get(*current_member).copied() else {
            continue;
        };
        for previous_member in previous_members {
            checkpoint_layout(execution)?;
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
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    checkpoint_layout(execution)?;
    resources.charge_layout_work(invariants.len())?;
    for invariant in invariants {
        checkpoint_layout(execution)?;
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
            checkpoint_layout(execution)?;
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

fn checkpoint_layout(execution: AsciiExecution<'_>) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::OperationControl;
    use merman_core::resources::ResourceProfile;

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
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
    fn placement_pair_scan_observes_cancellation_before_work_exhaustion() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(1)
            .expect("the setup should consume the only admitted work unit");
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let placements = [GridCoord { x: 0, y: 0 }, GridCoord { x: 4, y: 0 }];

        let error = placement_state_is_valid(
            GraphDirection::TopDown,
            &placements,
            &[],
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("cancellation should win before the next pair-work debit");

        assert!(matches!(
            error,
            crate::AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }
}
