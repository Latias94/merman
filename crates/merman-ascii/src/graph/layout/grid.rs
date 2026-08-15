use super::super::label::{GraphLabel, GraphNodeLabelPlan};
use super::super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection, GraphNodeSide,
};
use super::super::shape::{GraphNodeShapeSemantics, GraphNodeShapeSize};
use super::groups;
use super::{GridCoord, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

mod rank;

pub(super) use rank::rank_leaf_group_levels;

const MINIMUM_NODE_GRID_WIDTH: usize = 3;
const MINIMUM_NODE_GRID_HEIGHT: usize = 3;
const MINIMUM_NODE_GRID_CELLS: usize = MINIMUM_NODE_GRID_WIDTH * MINIMUM_NODE_GRID_HEIGHT;
const AXIS_ENTRIES_PER_NODE: usize = 4;
const MINIMUM_GROUP_RANK_GAP: usize = 1;

pub(super) type AxisSizes = HashMap<usize, usize>;
type NodeIndexById<'a> = HashMap<&'a str, usize>;
type LevelPositions = HashMap<usize, usize>;
pub(super) type GridNodeLayoutParts = (Vec<NodeLayout>, AxisSizes, AxisSizes);

struct RankedGridPlacements {
    nodes: Vec<GridCoord>,
    leaf_group_levels: Vec<Option<usize>>,
}

fn checkpoint_layout(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(merman_core::OperationPhase::Layout, iteration)
}

pub(super) fn preflight_minimum_grid_extent(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<()> {
    // Every placed node owns a disjoint 3x3 exclusion block in the temporary routing grid. Check
    // that unavoidable storage before constructing any hash-backed layout containers.
    let minimum_cells = minimum_node_grid_cells(graph.nodes.len(), resources)?;
    resources.grid_extent(minimum_cells, 1)?;
    resources.grid_extent(options.graph_padding_x, 1)?;
    resources.grid_extent(options.graph_padding_y, 1)?;
    Ok(())
}

pub(super) fn layout_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlan],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    match graph.direction.canonical() {
        GraphDirection::LeftRight => layout_left_right_grid_nodes(
            graph,
            options,
            topology,
            label_plans,
            resources,
            execution,
        ),
        GraphDirection::TopDown => {
            layout_top_down_grid_nodes(graph, options, topology, label_plans, resources, execution)
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

pub(super) fn plan_node_labels(
    graph: &AsciiGraph,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<GraphNodeLabelPlan>> {
    plan_node_labels_impl(graph, width_profile, resources, || {})
}

fn plan_node_labels_impl(
    graph: &AsciiGraph,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    before_plan_reserve: impl FnOnce(),
) -> Result<Vec<GraphNodeLabelPlan>> {
    resources.transaction(|resources| {
        plan_node_labels_transactional(graph, width_profile, resources, before_plan_reserve)
    })
}

fn plan_node_labels_transactional(
    graph: &AsciiGraph,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    before_plan_reserve: impl FnOnce(),
) -> Result<Vec<GraphNodeLabelPlan>> {
    // Measure the full batch without retaining the outer container so aggregate document/output
    // admission precedes O(N) plan storage. The scratch replay rebuilds the deterministic plans,
    // while its work is charged to the shared render ledger before allocation.
    let base_work = resources.layout_work_used();
    let mut document_cells = 0usize;
    let mut materialized_bytes = 0usize;
    for node in &graph.nodes {
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

    before_plan_reserve();
    let mut plans = Vec::new();
    try_reserve_vec(&mut plans, graph.nodes.len())?;
    let replay_resources = resources.detached();
    for node in &graph.nodes {
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

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlan],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    let ranked = place_left_right_grid_nodes(
        graph,
        topology,
        options.terminal_width_profile,
        resources,
        execution,
    )?;
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
        let shape_size = node_shape_size(node, label_plan, options, resources)?;
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            shape_size.width.saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
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
                groups::node_padding_y(index, &node_padding, options, resources)?,
            );
        }
    }

    reserve_leaf_group_rank_axis_sizes(
        graph,
        &ranked.leaf_group_levels,
        GraphDirection::LeftRight,
        options,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;

    let coord_by_id = node_coords_by_id(graph, &placements)?;
    for (index, edge) in graph.edges.iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let (Some(from), Some(to)) = (
            coord_by_id.get(edge.from.as_str()).copied(),
            coord_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        apply_horizontal_edge_spacing(edge, from, to, options, &mut column_widths, resources)?;
        if from.x == to.x {
            apply_vertical_edge_spacing(
                edge,
                from,
                to,
                options,
                &mut column_widths,
                &mut row_heights,
                resources,
            )?;
        }
    }

    let width = checked_axis_total(&column_widths, resources)?;
    let height = checked_axis_total(&row_heights, resources)?;
    resources.grid_extent(width, height)?;

    let layouts = build_node_layouts(
        graph,
        placements,
        &column_widths,
        &row_heights,
        options.terminal_width_profile,
    )?;

    Ok((layouts, column_widths, row_heights))
}

fn place_left_right_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    width_profile: TerminalWidthProfile,
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
        groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
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

fn charge_sort_work(len: usize, resources: &ResourceContext) -> Result<()> {
    let comparison_height = if len <= 1 {
        1
    } else {
        usize::BITS as usize - (len - 1).leading_zeros() as usize
    };
    resources.charge_layout_work(resources.checked_work_mul(len, comparison_height)?)
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
    options: &AsciiRenderOptions,
    topology: Option<&GraphGroupTopology<'_>>,
    label_plans: &[GraphNodeLabelPlan],
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GridNodeLayoutParts> {
    let ranked = place_top_down_grid_nodes(
        graph,
        topology,
        options.terminal_width_profile,
        resources,
        execution,
    )?;
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
        let shape_size = node_shape_size(node, label_plan, options, resources)?;
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            shape_size.width.saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
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
                groups::node_padding_y(index, &node_padding, options, resources)?,
            );
        }
    }

    reserve_leaf_group_rank_axis_sizes(
        graph,
        &ranked.leaf_group_levels,
        GraphDirection::TopDown,
        options,
        &mut column_widths,
        &mut row_heights,
        resources,
    )?;

    let index_by_id = node_indices_by_id(graph)?;
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
            options,
            &mut column_widths,
            &mut row_heights,
            resources,
        )?;
        if to.x != from.x && from.y == to.y {
            apply_horizontal_edge_spacing(edge, from, to, options, &mut column_widths, resources)?;
        }
    }

    let width = checked_axis_total(&column_widths, resources)?;
    let height = checked_axis_total(&row_heights, resources)?;
    resources.grid_extent(width, height)?;

    let layouts = build_node_layouts(
        graph,
        placements,
        &column_widths,
        &row_heights,
        options.terminal_width_profile,
    )?;

    Ok((layouts, column_widths, row_heights))
}

fn reserve_leaf_group_rank_axis_sizes(
    graph: &AsciiGraph,
    leaf_group_levels: &[Option<usize>],
    direction: GraphDirection,
    options: &AsciiRenderOptions,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(leaf_group_levels.len())?;
    for (group_index, level) in leaf_group_levels.iter().copied().enumerate() {
        let (Some(level), Some(group)) = (level, graph.groups.get(group_index)) else {
            continue;
        };
        let (width, height) =
            groups::empty_group_minimum_size(group, options.terminal_width_profile, resources)?;
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
                        options.graph_padding_x.max(MINIMUM_GROUP_RANK_GAP),
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
                        options.graph_padding_y.max(MINIMUM_GROUP_RANK_GAP),
                    );
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }
    Ok(())
}

fn apply_vertical_edge_spacing(
    edge: &AsciiGraphEdge,
    from: GridCoord,
    to: GridCoord,
    options: &AsciiRenderOptions,
    column_widths: &mut AxisSizes,
    row_heights: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    if to.y == from.y {
        return Ok(());
    }

    let length_gap = resources.checked_grid_add(
        options.graph_padding_y,
        resources.checked_grid_mul(edge.length.saturating_sub(1), 2)?,
    )?;
    let label_width = if from.x == to.x {
        edge.label
            .as_deref()
            .map(|label| {
                GraphLabel::try_measure_width_with_profile(
                    label,
                    options.terminal_width_profile,
                    resources,
                )
            })
            .transpose()?
    } else {
        None
    };
    set_axis_size(row_heights, from.y.max(to.y) - 1, length_gap);
    if let Some(label_width) = label_width {
        set_axis_size(
            column_widths,
            from.x + 1,
            resources.checked_grid_add(label_width, 2)?,
        );
    }
    Ok(())
}

fn apply_horizontal_edge_spacing(
    edge: &AsciiGraphEdge,
    from: GridCoord,
    to: GridCoord,
    options: &AsciiRenderOptions,
    column_widths: &mut AxisSizes,
    resources: &ResourceContext,
) -> Result<()> {
    if to.x == from.x {
        return Ok(());
    }

    let length_gap = resources.checked_grid_add(
        options.graph_padding_x,
        resources.checked_grid_mul(edge.length.saturating_sub(1), 2)?,
    )?;
    let label_gap = edge
        .label
        .as_deref()
        .map(|label| {
            resources.checked_grid_add(
                GraphLabel::try_measure_width_with_profile(
                    label,
                    options.terminal_width_profile,
                    resources,
                )?,
                2,
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
    width_profile: TerminalWidthProfile,
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
        groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
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
    let mut index_by_id = HashMap::new();
    try_reserve_hash_map(&mut index_by_id, graph.nodes.len())?;
    for (index, node) in graph.nodes.iter().enumerate() {
        index_by_id.insert(node.id.as_str(), index);
    }
    Ok(index_by_id)
}

fn node_coords_by_id<'a>(
    graph: &'a AsciiGraph,
    placements: &[GridCoord],
) -> Result<HashMap<&'a str, GridCoord>> {
    let mut coord_by_id = HashMap::new();
    try_reserve_hash_map(&mut coord_by_id, graph.nodes.len())?;
    for (node, coord) in graph.nodes.iter().zip(placements.iter().copied()) {
        coord_by_id.insert(node.id.as_str(), coord);
    }
    Ok(coord_by_id)
}

fn new_occupied_grid(
    node_count: usize,
    resources: &ResourceContext,
) -> Result<HashSet<(usize, usize)>> {
    let mut occupied = HashSet::new();
    try_reserve_hash_set(
        &mut occupied,
        minimum_node_grid_cells(node_count, resources)?,
    )?;
    Ok(occupied)
}

fn new_level_positions(node_count: usize) -> Result<LevelPositions> {
    let mut positions = HashMap::new();
    try_reserve_hash_map(&mut positions, node_count)?;
    Ok(positions)
}

fn new_axis_sizes(node_count: usize, resources: &ResourceContext) -> Result<AxisSizes> {
    let mut axis_sizes = HashMap::new();
    let capacity = resources.checked_grid_mul(node_count, AXIS_ENTRIES_PER_NODE)?;
    try_reserve_hash_map(&mut axis_sizes, capacity)?;
    Ok(axis_sizes)
}

fn build_node_layouts(
    graph: &AsciiGraph,
    placements: Vec<GridCoord>,
    column_widths: &AxisSizes,
    row_heights: &AxisSizes,
    width_profile: TerminalWidthProfile,
) -> Result<Vec<NodeLayout>> {
    if placements.len() != graph.nodes.len() {
        return Err(invalid_node_label_plans(graph.diagram_type()));
    }
    let mut layouts = Vec::new();
    try_reserve_vec(&mut layouts, placements.len())?;
    for (coord, node) in placements.into_iter().zip(graph.nodes.iter()) {
        layouts.push(NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::unmaterialized_with_profile(width_profile),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: axis_position(column_widths, coord.x),
            y: axis_position(row_heights, coord.y),
            width: axis_span(column_widths, coord.x, 3),
            height: axis_span(row_heights, coord.y, 3),
        });
    }
    Ok(layouts)
}

pub(super) fn materialize_node_labels(
    layouts: &mut [NodeLayout],
    graph: &AsciiGraph,
    label_plans: &[GraphNodeLabelPlan],
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

fn set_axis_size(axis_sizes: &mut AxisSizes, index: usize, size: usize) {
    axis_sizes
        .entry(index)
        .and_modify(|current| *current = (*current).max(size))
        .or_insert(size);
}

pub(super) fn axis_position(axis_sizes: &AxisSizes, index: usize) -> usize {
    axis_sizes
        .iter()
        .filter(|(axis_index, _)| **axis_index < index)
        .map(|(_, size)| *size)
        .sum::<usize>()
        + axis_sizes.get(&index).copied().unwrap_or_default() / 2
}

fn axis_span(axis_sizes: &AxisSizes, start: usize, len: usize) -> usize {
    (start..(start + len))
        .map(|index| axis_sizes.get(&index).copied().unwrap_or_default())
        .sum()
}

fn node_shape_size(
    node: &AsciiGraphNode,
    label_plan: &GraphNodeLabelPlan,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<GraphNodeShapeSize> {
    let metrics = label_plan.metrics();
    GraphNodeShapeSemantics::new(node.shape).try_size_for_label_metrics(
        metrics.width,
        metrics.content_height,
        options,
        resources,
    )
}

fn invalid_node_label_plans(diagram_type: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type,
        feature: "invalid graph node label plans",
    }
}

fn checked_axis_total(axis_sizes: &AxisSizes, resources: &ResourceContext) -> Result<usize> {
    axis_sizes.values().try_fold(0usize, |total, size| {
        resources.checked_grid_add(total, *size)
    })
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
    use crate::resource::AsciiResourcePolicy;
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};

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
                options,
                column_widths,
                resources,
            ),
            GraphDirection::TopDown => apply_vertical_edge_spacing(
                edge,
                GridCoord { x: 0, y: 0 },
                GridCoord { x: 0, y: 4 },
                options,
                column_widths,
                row_heights,
                resources,
            ),
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    #[test]
    fn node_label_batch_admits_aggregate_limits_before_reserving_plan_storage() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("alpha", "Alpha");
        graph.add_node("bravo", "Bravo");
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let measured_resources = ResourceContext::new(unbounded);
        let reserve_started = std::cell::Cell::new(false);
        let plans = plan_node_labels_impl(
            &graph,
            TerminalWidthProfile::Unicode,
            &measured_resources,
            || reserve_started.set(true),
        )
        .expect("unbounded node-label planning should succeed");
        assert!(reserve_started.get());
        assert_eq!(plans.len(), 2);
        let exact_work = measured_resources.layout_work_used();
        let exact_document_cells = 10;
        let exact_output_bytes = 10;
        assert_eq!(exact_work, 60);
        assert_eq!(
            plans
                .iter()
                .map(GraphNodeLabelPlan::document_cells)
                .sum::<usize>(),
            exact_document_cells
        );
        assert_eq!(
            plans
                .iter()
                .map(GraphNodeLabelPlan::materialized_bytes)
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
        let exact_reserve_started = std::cell::Cell::new(false);
        plan_node_labels_impl(
            &graph,
            TerminalWidthProfile::Unicode,
            &exact_resources,
            || exact_reserve_started.set(true),
        )
        .expect("exact node-label limits should permit planning");
        assert!(exact_reserve_started.get());
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
            let below_reserve_started = std::cell::Cell::new(false);
            let error = plan_node_labels_impl(
                &graph,
                TerminalWidthProfile::Unicode,
                &below_resources,
                || below_reserve_started.set(true),
            )
            .expect_err("max-minus-one node-label limit should reject before allocation");
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a node-label resource error, got {error:?}");
            };
            assert_eq!(details.limit, limit);
            assert_eq!(details.actual, actual);
            assert_eq!(details.max, actual - 1);
            assert!(!below_reserve_started.get());
            assert_eq!(below_resources.layout_work_used(), 0, "limit={limit:?}");
            assert_eq!(below_resources.document_cells_used(), 0, "limit={limit:?}");
        }
    }

    #[test]
    fn node_label_replay_observes_cancellation_and_restores_shared_ledger() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("alpha", "Alpha");
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(7, 11)
            .expect("the shared ledger should accept its initial usage");
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Layout);

        let error =
            plan_node_labels_impl(&graph, TerminalWidthProfile::Unicode, &controlled, || {
                control.cancel()
            })
            .expect_err("the deterministic label replay must observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 7);
        assert_eq!(resources.document_cells_used(), 11);
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
                &mut AxisSizes::new(),
                &mut AxisSizes::new(),
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
                &mut AxisSizes::new(),
                &mut AxisSizes::new(),
                &exact_resources,
            )
            .expect("edge label spacing should pass at the exact work limit");
            assert_eq!(exact_resources.layout_work_used(), exact_work);

            let below_policy = unbounded
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
                .expect("layout work limit should be valid");
            let below_resources = ResourceContext::new(below_policy);
            let mut column_widths = AxisSizes::new();
            let mut row_heights = AxisSizes::new();
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
