use super::super::horizontal_label::HorizontalLabelPadding;
use super::super::label::{
    GraphLabel, GraphNodeLabelPlan, GraphNodeLabelPlanHandle, try_measure_edge_label,
};
use super::super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection, GraphEdgeMarker, GraphNodeSide,
};
use super::super::shape::{GraphNodeShapeSemantics, GraphNodeShapeSize};
use super::groups;
use super::{GridCoord, NodeLayout, ROUTING_GRID_MARGIN, charge_sort_work};
use crate::error::{AsciiError, Result};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::options::{GraphLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::hash::Hash;

mod rank;

pub(super) use rank::rank_leaf_group_levels;

const MINIMUM_NODE_GRID_WIDTH: usize = 3;
const MINIMUM_NODE_GRID_HEIGHT: usize = 3;
const MINIMUM_NODE_GRID_CELLS: usize = MINIMUM_NODE_GRID_WIDTH * MINIMUM_NODE_GRID_HEIGHT;
const AXIS_ENTRIES_PER_NODE: usize = 4;
const MINIMUM_GROUP_RANK_GAP: usize = 1;
// One reading-margin cell per endpoint plus one free transverse transit cell.
const MINIMUM_BRANCH_RANK_GAP: usize = 3;
const ROUTING_CORRIDOR_BAND_SIZE: usize = 2;
const NODE_LABEL_PLAN_CHECKPOINT_INTERVAL: usize = 64;

pub(super) type AxisSizes = HashMap<usize, usize>;
type NodeIndexById<'a> = HashMap<&'a str, usize>;
type LevelPositions = HashMap<usize, usize>;
pub(super) type GridNodeLayoutParts = (Vec<NodeLayout>, AxisProjection, AxisProjection);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AxisProjection {
    prefix_sizes: Vec<usize>,
    content_cells: usize,
}

impl AxisProjection {
    fn try_new(
        axis_sizes: &AxisSizes,
        routing_cell_count: usize,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
        checkpoint_iteration: &mut usize,
    ) -> Result<Self> {
        resources.charge_layout_work(axis_sizes.len())?;
        let mut max_index = None;
        for index in axis_sizes.keys().copied() {
            checkpoint_projection(execution, checkpoint_iteration)?;
            max_index = Some(max_index.map_or(index, |current: usize| current.max(index)));
        }

        let content_cells = max_index
            .map(|index| resources.checked_grid_add(index, 1))
            .transpose()?
            .unwrap_or_default();
        let cell_count = content_cells.max(routing_cell_count);
        let prefix_count = resources.checked_grid_add(cell_count, 1)?;
        resources.grid_extent(prefix_count, 1)?;
        resources.charge_layout_work(cell_count)?;

        let mut prefix_sizes = Vec::new();
        try_reserve_vec(&mut prefix_sizes, prefix_count)?;
        prefix_sizes.push(0);
        let mut total = 0usize;
        for index in 0..cell_count {
            checkpoint_projection(execution, checkpoint_iteration)?;
            total = resources.checked_grid_add(total, effective_axis_size(axis_sizes, index))?;
            prefix_sizes.push(total);
        }
        Ok(Self {
            prefix_sizes,
            content_cells,
        })
    }

    pub(super) fn position(&self, index: usize) -> usize {
        let cell_count = self.prefix_sizes.len().saturating_sub(1);
        if index >= cell_count {
            return self.total();
        }
        let start = self.prefix_sizes[index];
        let end = self.prefix_sizes[index + 1];
        start + (end - start) / 2
    }

    fn span(&self, start: usize, len: usize) -> usize {
        let cell_count = self.prefix_sizes.len().saturating_sub(1);
        let start = start.min(cell_count);
        let end = start.saturating_add(len).min(cell_count);
        self.prefix_sizes[end] - self.prefix_sizes[start]
    }

    fn content_total(&self) -> usize {
        self.prefix_sizes[self.content_cells]
    }

    fn total(&self) -> usize {
        self.prefix_sizes.last().copied().unwrap_or_default()
    }
}

struct RankedGridPlacements {
    nodes: Vec<GridCoord>,
    leaf_group_levels: Vec<Option<usize>>,
    group_directions: Vec<Option<GraphDirection>>,
}

#[derive(Clone, Copy, Default)]
struct VisibleNodeDegree {
    incoming: usize,
    outgoing: usize,
}

struct NodeAxisBands {
    columns: HashSet<usize>,
    rows: HashSet<usize>,
}

impl NodeAxisBands {
    fn try_new(placements: &[GridCoord], resources: &ResourceContext) -> Result<Self> {
        let count = resources.checked_work_mul(placements.len(), 3)?;
        resources.charge_layout_work(resources.checked_work_mul(count, 2)?)?;
        let mut bands = Self {
            columns: HashSet::default(),
            rows: HashSet::default(),
        };
        try_reserve_hash_set(&mut bands.columns, count)?;
        try_reserve_hash_set(&mut bands.rows, count)?;
        for placement in placements {
            resources.checkpoint()?;
            for offset in 0..3 {
                bands
                    .columns
                    .insert(resources.checked_grid_add(placement.x, offset)?);
                bands
                    .rows
                    .insert(resources.checked_grid_add(placement.y, offset)?);
            }
        }
        Ok(bands)
    }
}

fn checkpoint_layout(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(merman_model::OperationPhase::Layout, iteration)
}

pub(super) fn preflight_minimum_grid_extent(
    graph: &AsciiGraph,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<()> {
    // Every placed node owns a disjoint 3x3 exclusion block in the temporary routing grid. Check
    // that unavoidable storage before constructing any hash-backed layout containers.
    let minimum_cells = minimum_node_grid_cells(graph.nodes.len(), resources)?;
    resources.grid_extent(minimum_cells, 1)?;
    resources.grid_extent(policy.rank_gap_x, 1)?;
    resources.grid_extent(policy.rank_gap_y, 1)?;
    Ok(())
}

pub(super) fn layout_nodes(
    graph: &AsciiGraph,
    policy: &GraphLayoutPolicy,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlanHandle<'_>],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    match graph.direction.canonical() {
        GraphDirection::LeftRight => {
            layout_left_right_grid_nodes(graph, policy, topology, label_plans, resources, execution)
        }
        GraphDirection::TopDown => {
            layout_top_down_grid_nodes(graph, policy, topology, label_plans, resources, execution)
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

pub(super) fn plan_node_labels<'a>(
    graph: &'a AsciiGraph,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<GraphNodeLabelPlanHandle<'a>>> {
    resources
        .transaction(|resources| plan_node_labels_transactional(graph, width_profile, resources))
}

fn plan_node_labels_transactional<'a>(
    graph: &'a AsciiGraph,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<GraphNodeLabelPlanHandle<'a>>> {
    // Measure the full batch without retaining the outer container so aggregate document/output
    // admission precedes O(N) plan storage. The scratch replay rebuilds the deterministic plans,
    // while its work is charged to the shared render ledger before allocation.
    let base_work = resources.layout_work_used();
    let mut document_cells = 0usize;
    let mut materialized_bytes = 0usize;
    for (index, node) in graph.nodes.iter().enumerate() {
        checkpoint_node_label_plan(resources, index)?;
        let plan = GraphNodeLabelPlan::try_for_node(
            node,
            graph.node_label_wrap_width(),
            graph.diagram_type(),
            width_profile,
            resources,
        )?;
        document_cells = document_cells
            .checked_add(plan.document_cells())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        materialized_bytes = materialized_bytes
            .checked_add(plan.materialized_bytes())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
    }
    let planning_work = resources
        .layout_work_used()
        .checked_sub(base_work)
        .ok_or_else(|| resources.work_overflow())?;
    // The first pass already charged its planning work. Check the replay work and aggregate
    // document/output bounds without mutating the shared ledger, then commit only the replay.
    // Node-label document cells are a plan bound; the final canvas owns the document ledger.
    resources.check_usage(planning_work, 0)?;
    resources.check(AsciiResourceLimitId::MaxDocumentCells, document_cells)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, materialized_bytes)?;
    // The second pass is deterministic and allocation-free, but its work still belongs to the
    // render-wide ledger. Charge that replay before reserving the owned plan vector.
    resources.charge_layout_work(planning_work)?;

    let mut plans = Vec::new();
    try_reserve_vec(&mut plans, graph.nodes.len())?;
    let replay_resources = resources.detached();
    for (index, node) in graph.nodes.iter().enumerate() {
        checkpoint_node_label_plan(&replay_resources, index)?;
        plans.push(GraphNodeLabelPlan::try_for_node(
            node,
            graph.node_label_wrap_width(),
            graph.diagram_type(),
            width_profile,
            &replay_resources,
        )?);
    }
    if replay_resources.layout_work_used() != planning_work {
        return Err(invalid_node_label_plans(graph.diagram_type()));
    }
    Ok(plans)
}

fn checkpoint_node_label_plan(resources: &ResourceContext, iteration: usize) -> Result<()> {
    if iteration.is_multiple_of(NODE_LABEL_PLAN_CHECKPOINT_INTERVAL) {
        resources.checkpoint()?;
    }
    Ok(())
}

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    policy: &GraphLayoutPolicy,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlanHandle<'_>],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    let ranked = place_left_right_grid_nodes(graph, topology, policy, resources, execution)?;
    let placements = ranked.nodes;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let axis_entity_count = resources.checked_grid_add(graph.nodes.len(), graph.groups.len())?;
    let mut column_widths = new_axis_sizes(axis_entity_count, resources)?;
    let mut row_heights = new_axis_sizes(axis_entity_count, resources)?;

    for (index, coord) in placements.iter().copied().enumerate() {
        checkpoint_layout(execution, index)?;
        let node = &graph.nodes[index];
        let label_plan = label_plans
            .get(index)
            .ok_or_else(|| invalid_node_label_plans(graph.diagram_type()))?;
        let shape_size = node_shape_size(node, label_plan, policy, resources)?;
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            shape_size.width.saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, policy.rank_gap_x);
        }

        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(
            &mut row_heights,
            coord.y + 1,
            shape_size.height.saturating_sub(2),
        );
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                groups::node_padding_y(index, &node_padding, policy, resources)?,
            );
        }
    }

    reserve_leaf_group_rank_axis_sizes(
        graph,
        &ranked.leaf_group_levels,
        GraphDirection::LeftRight,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;

    reserve_self_loop_lanes(
        graph,
        &placements,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;
    let index_by_id = node_indices_by_id(graph)?;
    let degrees = visible_node_degrees(graph, &index_by_id, resources, execution)?;
    let occupied = NodeAxisBands::try_new(&placements, resources)?;
    for (index, edge) in graph.edges.iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let from = placements[from_index];
        let to = placements[to_index];
        apply_horizontal_edge_spacing(edge, from, to, policy, &mut column_widths, resources)?;
        reserve_branch_rank_spacing(
            graph,
            edge,
            from,
            to,
            degrees[from_index].outgoing > 1 || degrees[to_index].incoming > 1,
            topology,
            &ranked.group_directions,
            &occupied,
            &mut column_widths,
            &mut row_heights,
            resources,
        )?;
        if from.x == to.x {
            apply_vertical_edge_spacing(
                edge,
                from,
                to,
                policy,
                &mut column_widths,
                &mut row_heights,
                resources,
            )?;
        }
    }

    finalize_compound_node_layouts(
        graph,
        placements,
        topology,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
        execution,
    )
}

fn place_left_right_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RankedGridPlacements> {
    let mut ranked = place_ranked_grid_nodes(
        graph,
        topology,
        GraphDirection::LeftRight,
        resources,
        execution,
    )?;
    if !graph.groups.is_empty() {
        checkpoint_layout(execution, 0)?;
        ranked.group_directions = groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            policy,
            resources,
            execution,
        )?;
        checkpoint_layout(execution, 0)?;
    }
    Ok(ranked)
}

pub(super) fn place_ranked_grid_nodes_without_group_adjustments(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<GridCoord>> {
    Ok(place_ranked_grid_nodes(graph, None, direction.canonical(), resources, execution)?.nodes)
}

fn place_ranked_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RankedGridPlacements> {
    let rank_levels = rank::rank_levels(graph, topology, direction, resources, execution)?;
    let mut placement_order = index_order(graph.nodes.len())?;
    charge_sort_work(placement_order.len(), resources)?;
    placement_order.sort_unstable_by_key(|node_index| {
        let cross_axis_key = if direction.canonical() == GraphDirection::TopDown {
            rank_levels.side_constraints[*node_index]
                .map(|constraint| {
                    let side_order = match constraint.side {
                        GraphNodeSide::Left => 0,
                        GraphNodeSide::Right => 2,
                    };
                    (constraint.anchor_node_index, side_order, *node_index)
                })
                .unwrap_or((*node_index, 1, *node_index))
        } else {
            (*node_index, 1, *node_index)
        };
        (rank_levels.nodes[*node_index], cross_axis_key)
    });

    let mut placements = Vec::new();
    try_reserve_vec(&mut placements, graph.nodes.len())?;
    placements.resize(graph.nodes.len(), None);
    let mut occupied = new_occupied_grid(graph.nodes.len(), resources)?;
    let mut highest_position_per_level = new_level_positions(graph.nodes.len())?;

    for (iteration, node_index) in placement_order.into_iter().enumerate() {
        checkpoint_layout(execution, iteration)?;
        let level = rank_levels.nodes[node_index];
        let next_available = highest_position_per_level
            .get(&level)
            .copied()
            .unwrap_or_default();
        let requested = preferred_parent_lane(
            node_index,
            &rank_levels.nodes,
            &rank_levels.parent_indices,
            &placements,
            direction,
            resources,
        )?
        .map_or(next_available, |parent_lane| {
            parent_lane.max(next_available)
        });
        let requested_coord = match direction.canonical() {
            GraphDirection::LeftRight => GridCoord {
                x: level,
                y: requested,
            },
            GraphDirection::TopDown => GridCoord {
                x: requested,
                y: level,
            },
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        let coord = reserve_grid_spot(&mut occupied, requested_coord, direction, resources)?;
        let next_position = match direction.canonical() {
            GraphDirection::LeftRight => resources.checked_grid_add(coord.y, 4)?,
            GraphDirection::TopDown => resources.checked_grid_add(coord.x, 4)?,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        highest_position_per_level
            .entry(level)
            .and_modify(|current| *current = (*current).max(next_position))
            .or_insert(next_position);
        placements[node_index] = Some(coord);
    }

    Ok(RankedGridPlacements {
        nodes: finalize_ranked_placements(placements, graph.diagram_type())?,
        leaf_group_levels: rank_levels.leaf_groups,
        group_directions: Vec::new(),
    })
}

fn preferred_parent_lane(
    node_index: usize,
    rank_levels: &[usize],
    parent_indices: &[Vec<usize>],
    placements: &[Option<GridCoord>],
    direction: GraphDirection,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    let mut lane_total = 0usize;
    let mut parent_count = 0usize;
    let parents = parent_indices
        .get(node_index)
        .map_or(&[][..], Vec::as_slice);
    resources.charge_layout_work(parents.len())?;
    for &parent_index in parents {
        if rank_levels[parent_index] >= rank_levels[node_index] {
            continue;
        }
        let Some(parent) = placements.get(parent_index).copied().flatten() else {
            continue;
        };
        let lane = match direction.canonical() {
            GraphDirection::LeftRight => parent.y,
            GraphDirection::TopDown => parent.x,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        lane_total = resources.checked_grid_add(lane_total, lane)?;
        parent_count = resources.checked_grid_add(parent_count, 1)?;
    }
    Ok(lane_total.checked_div(parent_count))
}

fn finalize_ranked_placements(
    placements: Vec<Option<GridCoord>>,
    diagram_type: &'static str,
) -> Result<Vec<GridCoord>> {
    let mut finalized = Vec::new();
    try_reserve_vec(&mut finalized, placements.len())?;
    for placement in placements {
        let Some(placement) = placement else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "Dagre-compatible terminal rank placement",
            });
        };
        finalized.push(placement);
    }
    Ok(finalized)
}

fn index_order(len: usize) -> Result<Vec<usize>> {
    let mut order = Vec::new();
    try_reserve_vec(&mut order, len)?;
    order.extend(0..len);
    Ok(order)
}

fn reserve_grid_spot(
    occupied: &mut HashSet<(usize, usize)>,
    requested_coord: GridCoord,
    direction: GraphDirection,
    resources: &ResourceContext,
) -> Result<GridCoord> {
    let mut coord = requested_coord;
    loop {
        resources.charge_layout_work(MINIMUM_NODE_GRID_CELLS)?;
        if !grid_spot_occupied(occupied, coord, resources)? {
            break;
        }
        match direction.canonical() {
            GraphDirection::LeftRight => coord.y = resources.checked_grid_add(coord.y, 4)?,
            GraphDirection::TopDown => coord.x = resources.checked_grid_add(coord.x, 4)?,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    let end_x = resources.checked_grid_add(coord.x, MINIMUM_NODE_GRID_WIDTH)?;
    let end_y = resources.checked_grid_add(coord.y, MINIMUM_NODE_GRID_HEIGHT)?;
    resources.charge_layout_work(MINIMUM_NODE_GRID_CELLS)?;
    try_reserve_hash_set(occupied, MINIMUM_NODE_GRID_CELLS)?;
    for x in coord.x..end_x {
        for y in coord.y..end_y {
            occupied.insert((x, y));
        }
    }

    Ok(coord)
}

fn grid_spot_occupied(
    occupied: &HashSet<(usize, usize)>,
    coord: GridCoord,
    resources: &ResourceContext,
) -> Result<bool> {
    let end_x = resources.checked_grid_add(coord.x, MINIMUM_NODE_GRID_WIDTH)?;
    let end_y = resources.checked_grid_add(coord.y, MINIMUM_NODE_GRID_HEIGHT)?;
    Ok((coord.x..end_x).any(|x| (coord.y..end_y).any(|y| occupied.contains(&(x, y)))))
}

fn layout_top_down_grid_nodes(
    graph: &AsciiGraph,
    policy: &GraphLayoutPolicy,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlanHandle<'_>],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    let ranked = place_top_down_grid_nodes(graph, topology, policy, resources, execution)?;
    let placements = ranked.nodes;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let axis_entity_count = resources.checked_grid_add(graph.nodes.len(), graph.groups.len())?;
    let mut column_widths = new_axis_sizes(axis_entity_count, resources)?;
    let mut row_heights = new_axis_sizes(axis_entity_count, resources)?;

    for (index, coord) in placements.iter().copied().enumerate() {
        checkpoint_layout(execution, index)?;
        let node = &graph.nodes[index];
        let label_plan = label_plans
            .get(index)
            .ok_or_else(|| invalid_node_label_plans(graph.diagram_type()))?;
        let shape_size = node_shape_size(node, label_plan, policy, resources)?;
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            shape_size.width.saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, policy.rank_gap_x);
        }

        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(
            &mut row_heights,
            coord.y + 1,
            shape_size.height.saturating_sub(2),
        );
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                groups::node_padding_y(index, &node_padding, policy, resources)?,
            );
        }
    }

    reserve_leaf_group_rank_axis_sizes(
        graph,
        &ranked.leaf_group_levels,
        GraphDirection::TopDown,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;

    reserve_self_loop_lanes(
        graph,
        &placements,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;
    let index_by_id = node_indices_by_id(graph)?;
    let degrees = visible_node_degrees(graph, &index_by_id, resources, execution)?;
    let occupied = NodeAxisBands::try_new(&placements, resources)?;
    for (index, edge) in graph.edges.iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let from = placements[from_index];
        let to = placements[to_index];
        apply_vertical_edge_spacing(
            edge,
            from,
            to,
            policy,
            &mut column_widths,
            &mut row_heights,
            resources,
        )?;
        reserve_branch_rank_spacing(
            graph,
            edge,
            from,
            to,
            degrees[from_index].outgoing > 1 || degrees[to_index].incoming > 1,
            topology,
            &ranked.group_directions,
            &occupied,
            &mut column_widths,
            &mut row_heights,
            resources,
        )?;
        if to.x != from.x && from.y == to.y {
            apply_horizontal_edge_spacing(edge, from, to, policy, &mut column_widths, resources)?;
        }
    }

    finalize_compound_node_layouts(
        graph,
        placements,
        topology,
        policy,
        &mut column_widths,
        &mut row_heights,
        resources,
        execution,
    )
}

fn reserve_leaf_group_rank_axis_sizes(
    graph: &AsciiGraph,
    leaf_group_levels: &[Option<usize>],
    direction: GraphDirection,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(leaf_group_levels.len())?;
    for (group_index, level) in leaf_group_levels.iter().copied().enumerate() {
        let (Some(level), Some(group)) = (level, graph.groups.get(group_index)) else {
            continue;
        };
        let (width, height) = groups::empty_group_minimum_size(group, policy, resources)?;
        let second = resources.checked_grid_add(level, 1)?;
        let third = resources.checked_grid_add(level, 2)?;
        match direction.canonical() {
            GraphDirection::LeftRight => {
                set_axis_size(column_widths, level, 1);
                set_axis_size(column_widths, second, width.saturating_sub(2));
                set_axis_size(column_widths, third, 1);
                if level > 0 {
                    set_axis_size(
                        column_widths,
                        level - 1,
                        policy.rank_gap_x.max(MINIMUM_GROUP_RANK_GAP),
                    );
                }
            }
            GraphDirection::TopDown => {
                set_axis_size(row_heights, level, 1);
                set_axis_size(row_heights, second, height.saturating_sub(2));
                set_axis_size(row_heights, third, 1);
                if level > 0 {
                    set_axis_size(
                        row_heights,
                        level - 1,
                        policy.rank_gap_y.max(MINIMUM_GROUP_RANK_GAP),
                    );
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }
    Ok(())
}

fn reserve_self_loop_lanes(
    graph: &AsciiGraph,
    placements: &[GridCoord],
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(graph.edges.len())?;
    let mut loop_sizes = HashMap::<&str, (usize, usize)>::default();
    for edge in &graph.edges {
        resources.checkpoint()?;
        if edge.from == edge.to && edge.stroke.is_visible() {
            if !loop_sizes.contains_key(edge.from.as_str()) {
                try_reserve_hash_map(&mut loop_sizes, 1)?;
            }
            let (count, width) = loop_sizes.entry(edge.from.as_str()).or_default();
            *count = resources.checked_work_add(*count, 1)?;
            if let Some(label) = &edge.label {
                *width = (*width).max(
                    try_measure_edge_label(label, policy.terminal_width_profile, resources)?
                        .map_or(0, |metrics| metrics.max_width),
                );
            }
        }
    }
    if loop_sizes.is_empty() {
        return Ok(());
    }
    resources.charge_layout_work(graph.nodes.len())?;
    for (node, coord) in graph.nodes.iter().zip(placements) {
        resources.checkpoint()?;
        let (count, label_width) = loop_sizes
            .get(node.id.as_str())
            .copied()
            .unwrap_or_default();
        if count == 0 || (count == 1 && label_width == 0) {
            continue;
        }
        // The first right lane uses the midpoint to its next neighbor; subsequent loops
        // extend two cells past that midpoint. Reserve both halves of this offset plus
        // reading clearance. Bottom lanes instead start two cells below the node.
        let outer_offset = resources.checked_grid_mul(count - 1, 2)?;
        let lane_gap =
            resources.checked_grid_add(resources.checked_grid_mul(outer_offset, 2)?, 2)?;
        let node_width = (coord.x..=coord.x + 2).try_fold(0, |width, x| {
            resources.checked_grid_add(width, column_widths.get(&x).copied().unwrap_or(1))
        })?;
        let return_span = node_width.saturating_sub(1 + node_width / 2);
        // A horizontal loop label needs a visible stroke cell at either end. Its outer
        // vertical lane uses the midpoint of the gap to the next node.
        let label_gap = resources.checked_grid_mul(
            resources
                .checked_grid_add(label_width, 1)?
                .saturating_sub(return_span),
            2,
        )?;
        set_axis_size(
            column_widths,
            resources.checked_grid_add(coord.x, 3)?,
            lane_gap.max(label_gap),
        );
        if count > 1 {
            let vertical_gap =
                resources.checked_grid_add(resources.checked_grid_mul(count, 2)?, 1)?;
            set_axis_size(
                row_heights,
                resources.checked_grid_add(coord.y, 3)?,
                vertical_gap,
            );
        }
    }
    Ok(())
}

fn apply_vertical_edge_spacing(
    edge: &AsciiGraphEdge,
    from: GridCoord,
    to: GridCoord,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    if to.y == from.y {
        return Ok(());
    }

    let length_gap = resources.checked_grid_add(
        policy.rank_gap_y,
        resources.checked_grid_mul(edge.length.saturating_sub(1), 2)?,
    )?;
    let label_metrics = edge
        .label
        .as_deref()
        .map(|label| try_measure_edge_label(label, policy.terminal_width_profile, resources))
        .transpose()?
        .flatten();
    let label_gap = if let Some(metrics) = label_metrics {
        let marker_cells = [edge.start_marker, edge.end_marker]
            .into_iter()
            .filter(|marker| *marker != GraphEdgeMarker::Open)
            .count();
        // Preferred zero padding must still leave the full label block, independent
        // endpoint markers, and a visible connecting cell on either side.
        resources.checked_grid_add(metrics.line_count, marker_cells + 2)?
    } else {
        0
    };
    set_axis_size(row_heights, from.y.max(to.y) - 1, length_gap.max(label_gap));
    if from.x == to.x
        && let Some(metrics) = label_metrics
    {
        set_axis_size(
            column_widths,
            from.x + 1,
            resources.checked_grid_add(metrics.max_width, 2)?,
        );
    }
    Ok(())
}

fn apply_horizontal_edge_spacing(
    edge: &AsciiGraphEdge,
    from: GridCoord,
    to: GridCoord,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    if to.x == from.x {
        return Ok(());
    }

    let length_gap = resources.checked_grid_add(
        policy.rank_gap_x,
        resources.checked_grid_mul(edge.length.saturating_sub(1), 2)?,
    )?;
    let label_metrics = edge
        .label
        .as_deref()
        .map(|label| try_measure_edge_label(label, policy.terminal_width_profile, resources))
        .transpose()?
        .flatten();
    let label_gap = label_metrics
        .map(|metrics| {
            resources.checked_grid_add(
                metrics.max_width,
                HorizontalLabelPadding::new(edge.start_marker, edge.end_marker).gap_cells(),
            )
        })
        .transpose()?
        .unwrap_or_default();
    set_axis_size(
        column_widths,
        from.x.max(to.x) - 1,
        length_gap.max(label_gap),
    );
    Ok(())
}

fn place_top_down_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RankedGridPlacements> {
    let mut ranked = place_ranked_grid_nodes(
        graph,
        topology,
        GraphDirection::TopDown,
        resources,
        execution,
    )?;
    if !graph.groups.is_empty() {
        checkpoint_layout(execution, 0)?;
        ranked.group_directions = groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            policy,
            resources,
            execution,
        )?;
        checkpoint_layout(execution, 0)?;
    }
    Ok(ranked)
}

fn minimum_node_grid_cells(node_count: usize, resources: &ResourceContext) -> Result<usize> {
    resources.checked_grid_mul(node_count, MINIMUM_NODE_GRID_CELLS)
}

fn node_indices_by_id(graph: &AsciiGraph) -> Result<NodeIndexById<'_>> {
    let mut index_by_id = HashMap::default();
    try_reserve_hash_map(&mut index_by_id, graph.nodes.len())?;
    for (index, node) in graph.nodes.iter().enumerate() {
        index_by_id.insert(node.id.as_str(), index);
    }
    Ok(index_by_id)
}

fn visible_node_degrees(
    graph: &AsciiGraph,
    index_by_id: &NodeIndexById<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<VisibleNodeDegree>> {
    resources
        .charge_layout_work(resources.checked_work_add(graph.nodes.len(), graph.edges.len())?)?;
    let mut degrees = Vec::new();
    try_reserve_vec(&mut degrees, graph.nodes.len())?;
    degrees.resize(graph.nodes.len(), VisibleNodeDegree::default());
    for (index, edge) in graph.edges.iter().enumerate() {
        checkpoint_layout(execution, index)?;
        if !edge.stroke.is_visible() || edge.from == edge.to {
            continue;
        }
        let (Some(from), Some(to)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        degrees[from].outgoing = resources.checked_work_add(degrees[from].outgoing, 1)?;
        degrees[to].incoming = resources.checked_work_add(degrees[to].incoming, 1)?;
    }
    Ok(degrees)
}

#[allow(clippy::too_many_arguments)]
fn reserve_branch_rank_spacing(
    graph: &AsciiGraph,
    edge: &AsciiGraphEdge,
    from: GridCoord,
    to: GridCoord,
    is_branch: bool,
    topology: Option<&GraphGroupTopology<'_>>,
    group_directions: &[Option<GraphDirection>],
    occupied: &NodeAxisBands,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &mut ResourceContext,
) -> Result<()> {
    if !is_branch || !edge.stroke.is_visible() {
        return Ok(());
    }
    let mut direction = graph.direction;
    if let Some(topology) = topology {
        let target_scope = topology.groups_containing_endpoint(&edge.to, resources)?;
        let mut scope = topology.direct_node_group_index(&edge.from);
        for _ in 0..graph.groups.len() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            let Some(group_index) = scope else {
                break;
            };
            if target_scope.contains(&group_index)
                && let Some(local) = group_directions.get(group_index).copied().flatten()
            {
                direction = local;
                break;
            }
            scope = topology.parent_group_index(group_index);
        }
    }
    let (first, last, sizes, occupied) = match direction.canonical() {
        GraphDirection::LeftRight => (
            from.x.min(to.x),
            from.x.max(to.x),
            column_widths,
            &occupied.columns,
        ),
        GraphDirection::TopDown => (
            from.y.min(to.y),
            from.y.max(to.y),
            row_heights,
            &occupied.rows,
        ),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    };
    // All nodes share axis projections, including unrelated local-direction scopes. Expanding
    // any node-owned band would stretch its contour, so only a globally empty band may grow.
    if last - first > 2 {
        for separator in (resources.checked_grid_add(first, 3)?..last).rev() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if !occupied.contains(&separator) {
                set_axis_size(sizes, separator, MINIMUM_BRANCH_RANK_GAP);
                break;
            }
        }
    }
    Ok(())
}

fn new_occupied_grid(
    node_count: usize,
    resources: &ResourceContext,
) -> Result<HashSet<(usize, usize)>> {
    let mut occupied = HashSet::default();
    try_reserve_hash_set(
        &mut occupied,
        minimum_node_grid_cells(node_count, resources)?,
    )?;
    Ok(occupied)
}

fn new_level_positions(node_count: usize) -> Result<LevelPositions> {
    let mut positions = HashMap::default();
    try_reserve_hash_map(&mut positions, node_count)?;
    Ok(positions)
}

fn new_axis_sizes(node_count: usize, resources: &ResourceContext) -> Result<AxisSizes> {
    let mut axis_sizes = HashMap::default();
    let capacity = resources.checked_grid_mul(node_count, AXIS_ENTRIES_PER_NODE)?;
    try_reserve_hash_map(&mut axis_sizes, capacity)?;
    Ok(axis_sizes)
}

fn finalize_node_layouts(
    graph: &AsciiGraph,
    placements: Vec<GridCoord>,
    column_widths: &AxisSizes,
    row_heights: &AxisSizes,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    resources.transaction(|resources| {
        let mut checkpoint_iteration = 0usize;
        let (column_projection, row_projection) = finalize_axis_projections(
            placements.iter().copied(),
            column_widths,
            row_heights,
            resources,
            execution,
            &mut checkpoint_iteration,
        )?;
        resources.charge_layout_work(placements.len())?;
        let layouts = build_node_layouts(
            graph,
            placements,
            &column_projection,
            &row_projection,
            width_profile,
            execution,
            &mut checkpoint_iteration,
        )?;
        Ok((layouts, column_projection, row_projection))
    })
}

fn finalize_axis_projections(
    placements: impl ExactSizeIterator<Item = GridCoord>,
    column_widths: &AxisSizes,
    row_heights: &AxisSizes,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
    checkpoint_iteration: &mut usize,
) -> Result<(AxisProjection, AxisProjection)> {
    let (routing_columns, routing_rows) =
        routing_projection_cell_counts(placements, resources, execution)?;
    let columns = AxisProjection::try_new(
        column_widths,
        routing_columns,
        resources,
        execution,
        checkpoint_iteration,
    )?;
    let rows = AxisProjection::try_new(
        row_heights,
        routing_rows,
        resources,
        execution,
        checkpoint_iteration,
    )?;
    // Routing bands are projection storage, not unused diagram cells.
    resources.grid_extent(columns.content_total(), rows.content_total())?;
    Ok((columns, rows))
}

fn routing_projection_cell_counts(
    placements: impl ExactSizeIterator<Item = GridCoord>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(usize, usize)> {
    resources.charge_layout_work(placements.len())?;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    for (index, coord) in placements.enumerate() {
        checkpoint_layout(execution, index)?;
        max_x = max_x.max(resources.checked_grid_add(coord.x, 2)?);
        max_y = max_y.max(resources.checked_grid_add(coord.y, 2)?);
    }
    let routing_span = resources.checked_grid_add(ROUTING_GRID_MARGIN, 1)?;
    Ok((
        resources.checked_grid_add(max_x, routing_span)?,
        resources.checked_grid_add(max_y, routing_span)?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn finalize_compound_node_layouts(
    graph: &AsciiGraph,
    placements: Vec<GridCoord>,
    topology: Option<&GraphGroupTopology<'_>>,
    policy: &GraphLayoutPolicy,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    let (mut nodes, columns, rows) = finalize_node_layouts(
        graph,
        placements,
        column_widths,
        row_heights,
        policy.terminal_width_profile,
        resources,
        execution,
    )?;
    let Some(topology) = topology else {
        return Ok((nodes, columns, rows));
    };
    groups::reserve_compound_axis_spacing(
        graph,
        &nodes,
        topology,
        policy,
        column_widths,
        row_heights,
        resources,
        execution,
    )?;
    resources.transaction(|resources| {
        let mut checkpoint_iteration = 0usize;
        let (columns, rows) = finalize_axis_projections(
            nodes.iter().map(|node| node.grid),
            column_widths,
            row_heights,
            resources,
            execution,
            &mut checkpoint_iteration,
        )?;
        resources.charge_layout_work(nodes.len())?;
        for node in &mut nodes {
            checkpoint_projection(execution, &mut checkpoint_iteration)?;
            node.x = columns.position(node.grid.x);
            node.y = rows.position(node.grid.y);
            node.width = columns.span(node.grid.x, 3);
            node.height = rows.span(node.grid.y, 3);
        }
        Ok((nodes, columns, rows))
    })
}

fn build_node_layouts(
    graph: &AsciiGraph,
    placements: Vec<GridCoord>,
    column_projection: &AxisProjection,
    row_projection: &AxisProjection,
    width_profile: TerminalWidthProfile,
    execution: AsciiExecution<'_>,
    checkpoint_iteration: &mut usize,
) -> Result<Vec<NodeLayout>> {
    if placements.len() != graph.nodes.len() {
        return Err(invalid_node_label_plans(graph.diagram_type()));
    }
    let mut layouts = Vec::new();
    try_reserve_vec(&mut layouts, placements.len())?;
    for (coord, node) in placements.into_iter().zip(graph.nodes.iter()) {
        checkpoint_projection(execution, checkpoint_iteration)?;
        layouts.push(NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::unmaterialized_with_profile(width_profile),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: column_projection.position(coord.x),
            y: row_projection.position(coord.y),
            width: column_projection.span(coord.x, 3),
            height: row_projection.span(coord.y, 3),
        });
    }
    Ok(layouts)
}

fn checkpoint_projection(execution: AsciiExecution<'_>, iteration: &mut usize) -> Result<()> {
    execution.checkpoint_loop(merman_model::OperationPhase::Layout, *iteration)?;
    *iteration = iteration.saturating_add(1);
    Ok(())
}

pub(super) fn materialize_node_labels(
    layouts: &mut [NodeLayout],
    graph: &AsciiGraph,
    label_plans: &[GraphNodeLabelPlanHandle<'_>],
    resources: &ResourceContext,
) -> Result<()> {
    if layouts.len() != graph.nodes.len() || label_plans.len() != graph.nodes.len() {
        return Err(invalid_node_label_plans(graph.diagram_type()));
    }
    for ((layout, node), label_plan) in layouts
        .iter_mut()
        .zip(graph.nodes.iter())
        .zip(label_plans.iter())
    {
        layout.label = label_plan.materialize(node, resources)?;
    }
    Ok(())
}

pub(super) fn effective_axis_size(axis_sizes: &AxisSizes, index: usize) -> usize {
    axis_sizes
        .get(&index)
        .copied()
        .unwrap_or(ROUTING_CORRIDOR_BAND_SIZE)
        .max(1)
}

pub(super) fn set_axis_size(axis_sizes: &mut AxisSizes, index: usize, size: usize) {
    axis_sizes
        .entry(index)
        .and_modify(|current| *current = (*current).max(size))
        .or_insert(size);
}

fn node_shape_size(
    node: &AsciiGraphNode,
    label_plan: &GraphNodeLabelPlan,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<GraphNodeShapeSize> {
    let metrics = label_plan.metrics();
    GraphNodeShapeSemantics::new(node.shape).try_size_for_label_metrics(
        metrics.width,
        metrics.content_height,
        policy,
        resources,
    )
}

fn invalid_node_label_plans(diagram_type: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type,
        feature: "invalid graph node label plans",
    }
}

fn try_reserve_vec<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}

fn try_reserve_hash_map<K, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()>
where
    K: Eq + Hash,
{
    map.try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}

fn try_reserve_hash_set<T>(set: &mut HashSet<T>, additional: usize) -> Result<()>
where
    T: Eq + Hash,
{
    set.try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{GraphEdgeAttrs, GraphEdgeStroke};
    use crate::resource::AsciiResourcePolicy;
    use merman_model::resources::ResourceProfile;
    use merman_model::{OperationControl, OperationPhase};

    fn node_rank_gap(from: &NodeLayout, to: &NodeLayout, direction: GraphDirection) -> usize {
        match direction.canonical() {
            GraphDirection::LeftRight => to.x - from.right() - 1,
            GraphDirection::TopDown => to.y - from.bottom() - 1,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    #[test]
    fn visible_branches_reserve_a_transit_cell_outside_endpoint_reading_margins() {
        for direction in [
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
        ] {
            for fan_in in [false, true] {
                let mut graph = AsciiGraph::new(direction);
                for index in 1..=8 {
                    graph.add_node(format!("N{index:02}"), format!("S{index:02}"));
                }
                graph.add_node("T", "TARGET");
                for index in 1..=8 {
                    let leaf = format!("N{index:02}");
                    if fan_in {
                        graph.add_edge(leaf, "T");
                    } else {
                        graph.add_edge("T", leaf);
                    }
                }
                graph.add_node("Z", "TAIL");
                if fan_in {
                    graph.add_edge("T", "Z");
                } else {
                    graph.add_edge("Z", "T");
                }
                let mut invisible = graph.clone();
                for edge in &mut invisible.edges {
                    edge.stroke = GraphEdgeStroke::Invisible;
                }
                for preferred_gap in [0, 5] {
                    let options = AsciiRenderOptions::ascii()
                        .with_graph_padding_x(preferred_gap)
                        .with_graph_padding_y(preferred_gap);
                    let layout = layout_graph(&graph, &options);
                    let control = layout_graph(&invisible, &options);
                    let hub = &layout.nodes[8];
                    for leaf in &layout.nodes[..8] {
                        let (from, to) = if fan_in { (leaf, hub) } else { (hub, leaf) };
                        let gap = node_rank_gap(from, to, direction);
                        assert!(
                            gap.saturating_sub(2) >= 1 && gap >= preferred_gap,
                            "a branch needs a transit cell beyond both reading margins: {direction:?}, fan_in={fan_in}, gap={gap}"
                        );
                    }
                    let tail = &layout.nodes[9];
                    let (from, to) = if fan_in { (hub, tail) } else { (tail, hub) };
                    assert_eq!(node_rank_gap(from, to, direction), preferred_gap.max(1));
                    for (node, control_node) in layout.nodes.iter().zip(&control.nodes) {
                        assert_eq!(node.grid, control_node.grid);
                        assert_eq!(node.width, control_node.width);
                        assert_eq!(node.height, control_node.height);
                        match direction.canonical() {
                            GraphDirection::LeftRight => assert_eq!(node.y, control_node.y),
                            GraphDirection::TopDown => assert_eq!(node.x, control_node.x),
                            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn single_edges_and_invisible_branches_keep_zero_padding_rank_spacing() {
        let options = AsciiRenderOptions::ascii()
            .with_graph_padding_x(0)
            .with_graph_padding_y(0);
        for direction in [
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
        ] {
            for fan_in in [false, true] {
                for (first_stroke, second_stroke) in [
                    (GraphEdgeStroke::Normal, None),
                    (GraphEdgeStroke::Normal, Some(GraphEdgeStroke::Invisible)),
                    (GraphEdgeStroke::Invisible, Some(GraphEdgeStroke::Invisible)),
                ] {
                    let mut graph = AsciiGraph::new(direction);
                    for id in ["hub", "first", "second"] {
                        graph.add_node(id, id);
                    }
                    for (leaf, stroke) in [("first", Some(first_stroke)), ("second", second_stroke)]
                    {
                        let Some(stroke) = stroke else {
                            continue;
                        };
                        let (from, to) = if fan_in { (leaf, "hub") } else { ("hub", leaf) };
                        graph.add_edge_with_attrs(
                            from,
                            to,
                            GraphEdgeAttrs {
                                stroke,
                                ..GraphEdgeAttrs::default()
                            },
                        );
                    }
                    let layout = layout_graph(&graph, &options);
                    for edge in &graph.edges {
                        let from = layout
                            .nodes
                            .iter()
                            .find(|node| node.id == edge.from)
                            .unwrap();
                        let to = layout.nodes.iter().find(|node| node.id == edge.to).unwrap();
                        assert_eq!(
                            node_rank_gap(from, to, direction),
                            1,
                            "single edges and invisible constraints retain the minimum grid gap: {direction:?}, fan_in={fan_in}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn self_loops_do_not_turn_single_edges_into_branches() {
        let options = AsciiRenderOptions::ascii()
            .with_graph_padding_x(0)
            .with_graph_padding_y(0);
        for direction in [GraphDirection::TopDown, GraphDirection::LeftRight] {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge("a", "b");
            let control = layout_graph(&graph, &options);
            graph.add_edge("a", "a");
            graph.add_edge("b", "b");
            let layout = layout_graph(&graph, &options);
            assert_eq!(layout, control);
        }
    }

    #[test]
    fn branch_clearance_follows_the_applied_local_direction_without_stretching_nodes() {
        let options = AsciiRenderOptions::ascii()
            .with_node_padding(0)
            .with_graph_padding_x(0)
            .with_graph_padding_y(0);
        for root in [
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
        ] {
            for local in [
                GraphDirection::TopDown,
                GraphDirection::BottomTop,
                GraphDirection::LeftRight,
                GraphDirection::RightLeft,
            ] {
                for fan_in in [false, true] {
                    let mut graph = AsciiGraph::new(root);
                    for id in ["A", "B", "T"] {
                        graph.add_node(id, id);
                    }
                    for leaf in ["A", "B"] {
                        if fan_in {
                            graph.add_edge(leaf, "T");
                        } else {
                            graph.add_edge("T", leaf);
                        }
                    }
                    graph.add_group_with_style(
                        "g",
                        "g",
                        Some(local),
                        vec!["A".into(), "B".into(), "T".into()],
                        Default::default(),
                    );
                    let layout = layout_graph(&graph, &options);
                    for node in &layout.nodes {
                        assert_eq!(node.width, 3, "{root:?}, {local:?}, {node:?}");
                        assert_eq!(node.height, 3, "{root:?}, {local:?}, {node:?}");
                    }
                    let hub = &layout.nodes[2];
                    for leaf in &layout.nodes[..2] {
                        let (first, last) = match local.canonical() {
                            GraphDirection::LeftRight if leaf.x < hub.x => (leaf, hub),
                            GraphDirection::LeftRight => (hub, leaf),
                            GraphDirection::TopDown if leaf.y < hub.y => (leaf, hub),
                            GraphDirection::TopDown => (hub, leaf),
                            _ => unreachable!(),
                        };
                        assert!(node_rank_gap(first, last, local) >= 3);
                    }
                }
            }
        }
    }

    #[test]
    fn local_branch_spacing_preserves_unrelated_node_content_on_shared_axis_bands() {
        let options = AsciiRenderOptions::ascii()
            .with_node_padding(0)
            .with_graph_padding_x(0)
            .with_graph_padding_y(0);
        for (root, local) in [
            (GraphDirection::TopDown, GraphDirection::LeftRight),
            (GraphDirection::LeftRight, GraphDirection::TopDown),
        ] {
            let mut graph = AsciiGraph::new(root);
            for id in ["U", "V", "A", "B", "C"] {
                graph.add_node(id, id);
            }
            for (from, to) in [("U", "A"), ("V", "A"), ("A", "B"), ("A", "C")] {
                graph.add_edge(from, to);
            }
            graph.add_group_with_style(
                "g",
                "g",
                Some(local),
                vec!["A".into(), "B".into(), "C".into()],
                Default::default(),
            );
            let mut invisible = graph.clone();
            for edge in &mut invisible.edges {
                edge.stroke = GraphEdgeStroke::Invisible;
            }
            let layout = layout_graph(&graph, &options);
            let control = layout_graph(&invisible, &options);
            for (node, control) in layout.nodes.iter().zip(&control.nodes) {
                assert_eq!(node.grid, control.grid);
                assert_eq!(node.width, control.width, "{root:?}, {local:?}, {node:?}");
                assert_eq!(node.height, control.height, "{root:?}, {local:?}, {node:?}");
            }
        }
    }

    fn apply_test_edge_label_spacing(
        direction: GraphDirection,
        edge: &AsciiGraphEdge,
        options: &AsciiRenderOptions,
        column_widths: &mut AxisSizes,
        row_heights: &mut AxisSizes,
        resources: &ResourceContext,
    ) -> Result<()> {
        match direction {
            GraphDirection::LeftRight => apply_horizontal_edge_spacing(
                edge,
                GridCoord { x: 0, y: 0 },
                GridCoord { x: 4, y: 0 },
                &options.flowchart_layout().graph_policy(),
                column_widths,
                resources,
            ),
            GraphDirection::TopDown => apply_vertical_edge_spacing(
                edge,
                GridCoord { x: 0, y: 0 },
                GridCoord { x: 0, y: 4 },
                &options.flowchart_layout().graph_policy(),
                column_widths,
                row_heights,
                resources,
            ),
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    #[test]
    fn discarded_blank_edge_labels_do_not_expand_rank_spacing() {
        let options = AsciiRenderOptions::ascii()
            .with_graph_padding_x(0)
            .with_graph_padding_y(0);
        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge_with_attrs("a", "b", GraphEdgeAttrs::default());
            let edge = &mut graph.edges[0];
            let mut expected_columns = AxisSizes::default();
            let mut expected_rows = AxisSizes::default();
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            apply_test_edge_label_spacing(
                direction,
                edge,
                &options,
                &mut expected_columns,
                &mut expected_rows,
                &resources,
            )
            .unwrap();
            for blank in ["\n".repeat(128), " ".repeat(128), "<br/>".repeat(128)] {
                edge.label = Some(blank);
                let mut columns = AxisSizes::default();
                let mut rows = AxisSizes::default();
                apply_test_edge_label_spacing(
                    direction,
                    edge,
                    &options,
                    &mut columns,
                    &mut rows,
                    &resources,
                )
                .unwrap();
                assert_eq!(columns, expected_columns);
                assert_eq!(rows, expected_rows);
            }
        }
    }

    #[test]
    fn node_label_batch_admits_aggregate_limits_atomically() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("alpha", "Alpha");
        graph.add_node("bravo", "Bravo");
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let measured_resources = ResourceContext::new(unbounded);
        let plans = plan_node_labels(&graph, TerminalWidthProfile::Unicode, &measured_resources)
            .expect("unbounded node-label planning should succeed");
        assert_eq!(plans.len(), 2);
        let exact_work = measured_resources.layout_work_used();
        let exact_document_cells = 10;
        let exact_output_bytes = 10;
        assert_eq!(exact_work, 60);
        assert_eq!(
            plans
                .iter()
                .map(|plan| plan.document_cells())
                .sum::<usize>(),
            exact_document_cells
        );
        assert_eq!(
            plans
                .iter()
                .map(|plan| plan.materialized_bytes())
                .sum::<usize>(),
            exact_output_bytes
        );

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact node-label work limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_document_cells)
            .expect("exact node-label document limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes)
            .expect("exact node-label output limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        plan_node_labels(&graph, TerminalWidthProfile::Unicode, &exact_resources)
            .expect("exact node-label limits should permit planning");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        for (limit, actual) in [
            (AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work),
            (AsciiResourceLimitId::MaxDocumentCells, exact_document_cells),
            (AsciiResourceLimitId::MaxOutputBytes, exact_output_bytes),
        ] {
            let below_policy = exact_policy
                .with_limit(limit, actual - 1)
                .expect("max-minus-one node-label limit should be valid");
            let below_resources = ResourceContext::new(below_policy);
            let error = plan_node_labels(&graph, TerminalWidthProfile::Unicode, &below_resources)
                .expect_err("max-minus-one node-label limit should reject atomically");
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a node-label resource error, got {error:?}");
            };
            assert_eq!(details.limit, limit);
            assert_eq!(details.actual, actual);
            assert_eq!(details.max, actual - 1);
            assert_eq!(below_resources.layout_work_used(), 0, "limit={limit:?}");
            assert_eq!(below_resources.document_cells_used(), 0, "limit={limit:?}");
        }
    }

    #[test]
    fn node_label_planning_observes_cancellation_and_restores_shared_ledger() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("alpha", "Alpha");
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(7, 11)
            .expect("the shared ledger should accept its initial usage");
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Layout);
        control.cancel();

        let error = plan_node_labels(&graph, TerminalWidthProfile::Unicode, &controlled)
            .expect_err("node-label planning must observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_model::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 7);
        assert_eq!(resources.document_cells_used(), 11);
    }

    #[test]
    fn routing_projection_covers_every_search_track_without_growing_diagram_content() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("node", "Node");
        let mut columns = AxisSizes::default();
        let mut rows = AxisSizes::default();
        for (index, size) in [(0, 1), (1, 6), (2, 1)] {
            set_axis_size(&mut columns, index, size);
        }
        for (index, size) in [(0, 1), (1, 3), (2, 1)] {
            set_axis_size(&mut rows, index, size);
        }
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        let (nodes, columns, rows) = finalize_node_layouts(
            &graph,
            vec![GridCoord { x: 0, y: 0 }],
            &columns,
            &rows,
            TerminalWidthProfile::Unicode,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .expect("bounded routing tracks should be projected");
        let max_search_index = 2 + ROUTING_GRID_MARGIN;
        for projection in [&columns, &rows] {
            for index in 0..max_search_index {
                assert!(projection.position(index) < projection.position(index + 1));
            }
            assert_eq!(projection.position(3) - projection.position(2), 2);
        }
        assert_eq!(nodes[0].x, 0);
        assert_eq!(nodes[0].y, 0);
        assert_eq!(nodes[0].width, 8);
        assert_eq!(nodes[0].height, 5);
        assert_eq!(columns.content_total(), 8);
        assert_eq!(rows.content_total(), 5);
    }

    #[test]
    fn sparse_axis_projection_admits_linear_work_before_materializing_layouts() {
        const NODE_COUNT: usize = 256;
        const EXPECTED_WORK: usize = 2_321;

        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        let mut placements = Vec::with_capacity(NODE_COUNT);
        let mut column_widths = AxisSizes::default();
        let mut row_heights = AxisSizes::default();
        for index in 0..NODE_COUNT {
            let id = format!("node-{index}");
            graph.add_node(id.clone(), id);
            let x = index * 4;
            placements.push(GridCoord { x, y: 0 });
            for offset in 0..3 {
                set_axis_size(&mut column_widths, x + offset, 1);
            }
        }
        for index in 0..3 {
            set_axis_size(&mut row_heights, index, 1);
        }

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let measured = ResourceContext::new(unbounded);
        let (layouts, columns, rows) = finalize_node_layouts(
            &graph,
            placements.clone(),
            &column_widths,
            &row_heights,
            TerminalWidthProfile::Unicode,
            &measured,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("sparse axis projection should materialize");
        assert_eq!(layouts.len(), NODE_COUNT);
        assert_eq!(
            columns.total(),
            NODE_COUNT * 3 + (NODE_COUNT - 1) * 2 + ROUTING_GRID_MARGIN * 2
        );
        assert_eq!(rows.total(), 3 + ROUTING_GRID_MARGIN * 2);
        assert_eq!(measured.layout_work_used(), EXPECTED_WORK);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK)
            .expect("exact axis projection work limit should be valid");
        let exact = ResourceContext::new(exact_policy);
        finalize_node_layouts(
            &graph,
            placements.clone(),
            &column_widths,
            &row_heights,
            TerminalWidthProfile::Unicode,
            &exact,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("exact axis projection work should pass");
        assert_eq!(exact.layout_work_used(), EXPECTED_WORK);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK - 1)
            .expect("max-minus-one axis projection work limit should be valid");
        let below = ResourceContext::new(below_policy);
        let error = finalize_node_layouts(
            &graph,
            placements,
            &column_widths,
            &row_heights,
            TerminalWidthProfile::Unicode,
            &below,
            AsciiExecution::for_test(&below_policy),
        )
        .expect_err("max-minus-one work must reject before node layout materialization");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXPECTED_WORK
                    && details.max == EXPECTED_WORK - 1
        ));
        assert_eq!(below.layout_work_used(), 0);
    }

    #[test]
    fn edge_label_spacing_preflights_exact_work_before_axis_updates() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_edge("source", "target");
        graph.edges[0].label = Some("A<br>B".to_string());
        let edge = &graph.edges[0];
        let options = AsciiRenderOptions::ascii();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            let measured_resources = ResourceContext::new(unbounded);
            apply_test_edge_label_spacing(
                direction,
                edge,
                &options,
                &mut AxisSizes::default(),
                &mut AxisSizes::default(),
                &measured_resources,
            )
            .expect("unbounded label spacing should pass");
            let exact_work = measured_resources.layout_work_used();
            assert!(exact_work > 0, "edge label scans must debit layout work");

            let exact_policy = unbounded
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
                .expect("layout work limit should be valid");
            let exact_resources = ResourceContext::new(exact_policy);
            apply_test_edge_label_spacing(
                direction,
                edge,
                &options,
                &mut AxisSizes::default(),
                &mut AxisSizes::default(),
                &exact_resources,
            )
            .expect("edge label spacing should pass at the exact work limit");
            assert_eq!(exact_resources.layout_work_used(), exact_work);

            let below_policy = unbounded
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
                .expect("layout work limit should be valid");
            let below_resources = ResourceContext::new(below_policy);
            let mut column_widths = AxisSizes::default();
            let mut row_heights = AxisSizes::default();
            let error = apply_test_edge_label_spacing(
                direction,
                edge,
                &options,
                &mut column_widths,
                &mut row_heights,
                &below_resources,
            )
            .expect_err("max-minus-one work must reject edge label spacing");
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a layout-work resource error, got {error:?}");
            };
            assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
            assert_eq!(details.actual, exact_work);
            assert_eq!(details.max, exact_work - 1);
            assert!(column_widths.is_empty());
            assert!(row_heights.is_empty());
        }
    }
}
