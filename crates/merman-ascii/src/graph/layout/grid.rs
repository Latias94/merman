use super::super::label::GraphLabel;
use super::super::model::{AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection};
use super::super::shape::{GraphNodeShapeSemantics, GraphNodeShapeSize};
use super::groups;
use super::{GridCoord, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::graph::topology::GraphGroupTopology;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use dugong::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use dugong::{
    EdgeLabel, GraphLabel as DagreGraphLabel, NodeLabel, RankDir, WorkControl, WorkError,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

const MINIMUM_NODE_GRID_WIDTH: usize = 3;
const MINIMUM_NODE_GRID_HEIGHT: usize = 3;
const MINIMUM_NODE_GRID_CELLS: usize = MINIMUM_NODE_GRID_WIDTH * MINIMUM_NODE_GRID_HEIGHT;
const AXIS_ENTRIES_PER_NODE: usize = 4;
const GRID_UNITS_PER_RANK: usize = 4;

pub(super) type AxisSizes = HashMap<usize, usize>;
type LevelPositions = HashMap<usize, usize>;
type NodeIndexById<'a> = HashMap<&'a str, usize>;
pub(super) type GridNodeLayoutParts = (Vec<NodeLayout>, AxisSizes, AxisSizes);

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
    resources: &mut ResourceContext,
) -> Result<GridNodeLayoutParts> {
    match graph.direction.canonical() {
        GraphDirection::LeftRight => {
            layout_left_right_grid_nodes(graph, options, topology, resources)
        }
        GraphDirection::TopDown => layout_top_down_grid_nodes(graph, options, topology, resources),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    topology: Option<&GraphGroupTopology<'_>>,
    resources: &mut ResourceContext,
) -> Result<GridNodeLayoutParts> {
    let placements =
        place_left_right_grid_nodes(graph, topology, options.terminal_width_profile, resources)?;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let mut column_widths = new_axis_sizes(graph.nodes.len(), resources)?;
    let mut row_heights = new_axis_sizes(graph.nodes.len(), resources)?;

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        let shape_size = node_shape_size(node, options, resources)?;
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

    let coord_by_id = node_coords_by_id(graph, &placements)?;
    for edge in &graph.edges {
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
) -> Result<Vec<GridCoord>> {
    let mut placements =
        place_ranked_grid_nodes(graph, topology, GraphDirection::LeftRight, resources)?;
    if !graph.groups.is_empty() {
        groups::apply_group_placement_adjustments(
            graph,
            &mut placements,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
            resources,
        )?;
    }
    Ok(placements)
}

pub(super) fn place_ranked_grid_nodes_without_group_adjustments(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<Vec<GridCoord>> {
    place_ranked_grid_nodes(graph, None, direction.canonical(), resources)
}

fn place_ranked_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<Vec<GridCoord>> {
    let rank_levels = dagre_rank_levels(graph, topology, direction, resources)?;
    let parent_indices = rank_parent_indices(graph, &rank_levels, resources)?;
    let mut placement_order = index_order(graph.nodes.len())?;
    charge_sort_work(placement_order.len(), resources)?;
    placement_order.sort_unstable_by_key(|node_index| (rank_levels[*node_index], *node_index));

    let mut placements = Vec::new();
    try_reserve_vec(&mut placements, graph.nodes.len())?;
    placements.resize(graph.nodes.len(), None);
    let mut occupied = new_occupied_grid(graph.nodes.len(), resources)?;
    let mut highest_position_per_level = new_level_positions(graph.nodes.len())?;

    for node_index in placement_order {
        let level = rank_levels[node_index];
        let next_available = highest_position_per_level
            .get(&level)
            .copied()
            .unwrap_or_default();
        let requested = preferred_parent_lane(
            node_index,
            &rank_levels,
            &parent_indices,
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

    finalize_ranked_placements(placements, graph.diagram_type())
}

fn rank_parent_indices(
    graph: &AsciiGraph,
    rank_levels: &[usize],
    resources: &ResourceContext,
) -> Result<Vec<Vec<usize>>> {
    resources
        .charge_layout_work(resources.checked_work_add(graph.nodes.len(), graph.edges.len())?)?;
    let index_by_id = node_indices_by_id(graph)?;
    let mut parents = Vec::new();
    try_reserve_vec(&mut parents, graph.nodes.len())?;
    parents.resize_with(graph.nodes.len(), Vec::new);
    for edge in &graph.edges {
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        if from_index == to_index || rank_levels[from_index] >= rank_levels[to_index] {
            continue;
        }
        let incoming = &mut parents[to_index];
        incoming
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        incoming.push(from_index);
    }
    Ok(parents)
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
    if parent_count == 0 {
        Ok(None)
    } else {
        Ok(Some(lane_total / parent_count))
    }
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

fn dagre_rank_levels(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    let (rank_graph, node_ids) = build_dagre_rank_graph(graph, topology, direction, resources)?;
    let plan = {
        let mut work_control = AsciiDagreWorkControl::new(resources);
        match dugong::rank::plan_controlled(&rank_graph, &mut work_control) {
            Ok(plan) => plan,
            Err(error) => return Err(work_control.into_ascii_error(error, graph.diagram_type())),
        }
    };

    let projection_work = resources.checked_work_add(
        plan.nodes.len(),
        resources.checked_work_mul(node_ids.len(), 4)?,
    )?;
    resources.charge_layout_work(projection_work)?;
    let mut rank_by_id = HashMap::new();
    try_reserve_hash_map(&mut rank_by_id, plan.nodes.len())?;
    for node in plan.nodes {
        if let Some(rank) = node.rank {
            rank_by_id.insert(node.id, rank);
        }
    }

    let mut node_ranks = Vec::new();
    try_reserve_vec(&mut node_ranks, node_ids.len())?;
    for id in &node_ids {
        let Some(rank) = rank_by_id.get(id.as_str()).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible rank planning for an unranked graph node",
            });
        };
        node_ranks.push(rank);
    }

    // Dagre rank values can contain intentionally empty numeric ranks (especially around compound
    // borders). Terminal geometry needs the ordering and same-rank equivalence, not zero-width
    // canvas cells for those absent layers. Densify only the ranks occupied by caller nodes; edge
    // `minlen` still controls physical corridor width through the existing edge-length spacing.
    let mut occupied_ranks = Vec::new();
    try_reserve_vec(&mut occupied_ranks, node_ranks.len())?;
    occupied_ranks.extend(node_ranks.iter().copied());
    charge_sort_work(occupied_ranks.len(), resources)?;
    occupied_ranks.sort_unstable();
    occupied_ranks.dedup();
    let mut dense_rank = HashMap::new();
    try_reserve_hash_map(&mut dense_rank, occupied_ranks.len())?;
    for (index, rank) in occupied_ranks.into_iter().enumerate() {
        dense_rank.insert(
            rank,
            resources.checked_grid_mul(index, GRID_UNITS_PER_RANK)?,
        );
    }

    let mut levels = Vec::new();
    try_reserve_vec(&mut levels, node_ranks.len())?;
    for rank in node_ranks {
        let Some(level) = dense_rank.get(&rank).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible dense rank projection",
            });
        };
        levels.push(level);
    }
    Ok(levels)
}

fn build_dagre_rank_graph(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &ResourceContext,
) -> Result<(Graph<NodeLabel, EdgeLabel, DagreGraphLabel>, Vec<String>)> {
    let mut rank_graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    rank_graph.set_graph(DagreGraphLabel {
        rankdir: dagre_rank_direction(direction),
        ..DagreGraphLabel::default()
    });
    rank_graph.set_default_node_label(NodeLabel::default);
    rank_graph.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        weight: 1.0,
        ..EdgeLabel::default()
    });

    let node_order = stable_node_order(graph, resources)?;
    let mut node_ids = Vec::new();
    try_reserve_vec(&mut node_ids, graph.nodes.len())?;
    node_ids.resize_with(graph.nodes.len(), String::new);
    for (ordinal, node_index) in node_order.into_iter().enumerate() {
        let internal_id = format!("node:{ordinal}");
        rank_graph.set_node(internal_id.clone(), NodeLabel::default());
        node_ids[node_index] = internal_id;
    }

    let group_order = stable_group_order(graph, resources)?;
    let mut group_ids = Vec::new();
    try_reserve_vec(&mut group_ids, graph.groups.len())?;
    group_ids.resize_with(graph.groups.len(), String::new);
    for (ordinal, group_index) in group_order.into_iter().enumerate() {
        let internal_id = format!("group:{ordinal}");
        rank_graph.set_node(internal_id.clone(), NodeLabel::default());
        group_ids[group_index] = internal_id;
    }

    if let Some(topology) = topology {
        let parent_capacity = resources.checked_work_add(graph.nodes.len(), graph.groups.len())?;
        // Admit the two assignment scans plus the bounded union/validation and replay owned by
        // Graphlib's atomic parent batch before materializing the compound forest.
        resources.charge_layout_work(resources.checked_work_mul(parent_capacity, 8)?)?;
        let mut parent_assignments = Vec::new();
        try_reserve_vec(&mut parent_assignments, parent_capacity)?;
        for (node_index, node_id) in node_ids.iter().enumerate() {
            let Some(group_index) = topology.direct_node_group_index(&graph.nodes[node_index].id)
            else {
                continue;
            };
            if let Some(group_id) = group_ids.get(group_index) {
                let (Some(node_ix), Some(group_ix)) =
                    (rank_graph.node_ix(node_id), rank_graph.node_ix(group_id))
                else {
                    continue;
                };
                parent_assignments.push((node_ix, group_ix));
            }
        }
        for (group_index, group_id) in group_ids.iter().enumerate() {
            let Some(parent_index) = topology.parent_group_index(group_index) else {
                continue;
            };
            if let Some(parent_id) = group_ids.get(parent_index) {
                let (Some(group_ix), Some(parent_ix)) =
                    (rank_graph.node_ix(group_id), rank_graph.node_ix(parent_id))
                else {
                    continue;
                };
                parent_assignments.push((group_ix, parent_ix));
            }
        }
        rank_graph
            .try_set_unparented_parents_ix(&parent_assignments)
            .map_err(|_| AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "cyclic or multiply-owned compound graph membership",
            })?;
    }

    let index_by_id = node_indices_by_id(graph)?;
    let edge_order = stable_edge_order(graph, resources)?;
    for (ordinal, edge_index) in edge_order.into_iter().enumerate() {
        let edge = &graph.edges[edge_index];
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        rank_graph.set_edge_named(
            node_ids[from_index].clone(),
            node_ids[to_index].clone(),
            Some(format!("edge:{ordinal}")),
            Some(EdgeLabel {
                minlen: edge.length,
                weight: 1.0,
                ..EdgeLabel::default()
            }),
        );
    }

    Ok((rank_graph, node_ids))
}

fn stable_node_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.nodes.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        stable_object_key_cmp(&graph.nodes[*left].id, &graph.nodes[*right].id)
            .then_with(|| left.cmp(right))
    });
    Ok(order)
}

fn stable_group_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.groups.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        stable_object_key_cmp(&graph.groups[*left].id, &graph.groups[*right].id)
            .then_with(|| left.cmp(right))
    });
    Ok(order)
}

fn stable_object_key_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    match (
        is_javascript_array_index(left),
        is_javascript_array_index(right),
    ) {
        (true, true) => match (left.parse::<u32>(), right.parse::<u32>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => left.cmp(right),
    }
}

fn stable_edge_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.edges.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        let left_edge = &graph.edges[*left];
        let right_edge = &graph.edges[*right];
        stable_object_key_cmp(&left_edge.from, &right_edge.from)
            .then_with(|| stable_object_key_cmp(&left_edge.to, &right_edge.to))
            .then_with(|| left_edge.length.cmp(&right_edge.length))
            .then_with(|| {
                stable_explicit_edge_id(left_edge).cmp(&stable_explicit_edge_id(right_edge))
            })
            .then_with(|| left.cmp(right))
    });
    Ok(order)
}

fn stable_explicit_edge_id(edge: &AsciiGraphEdge) -> Option<&str> {
    if edge.is_user_defined_id {
        edge.id.as_deref()
    } else {
        None
    }
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

const fn dagre_rank_direction(direction: GraphDirection) -> RankDir {
    match direction {
        GraphDirection::TopDown => RankDir::TB,
        GraphDirection::BottomTop => RankDir::BT,
        GraphDirection::LeftRight => RankDir::LR,
        GraphDirection::RightLeft => RankDir::RL,
    }
}

struct AsciiDagreWorkControl<'a> {
    resources: &'a ResourceContext,
    ascii_error: Option<AsciiError>,
}

impl<'a> AsciiDagreWorkControl<'a> {
    fn new(resources: &'a ResourceContext) -> Self {
        Self {
            resources,
            ascii_error: None,
        }
    }

    fn into_ascii_error(
        mut self,
        error: dugong::LayoutError,
        diagram_type: &'static str,
    ) -> AsciiError {
        if let Some(error) = self.ascii_error.take() {
            return error;
        }
        match error {
            dugong::LayoutError::Work(WorkError::ArithmeticOverflow) => self
                .resources
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits),
            _ => AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "Dagre-compatible rank planning for this graph topology",
            },
        }
    }
}

impl WorkControl for AsciiDagreWorkControl<'_> {
    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkError> {
        if self.ascii_error.is_some() {
            return Err(WorkError::Interrupted);
        }
        if let Err(error) = self.resources.charge_layout_work(units) {
            self.ascii_error = Some(error);
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }
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
    resources: &mut ResourceContext,
) -> Result<GridNodeLayoutParts> {
    let placements =
        place_top_down_grid_nodes(graph, topology, options.terminal_width_profile, resources)?;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let mut column_widths = new_axis_sizes(graph.nodes.len(), resources)?;
    let mut row_heights = new_axis_sizes(graph.nodes.len(), resources)?;

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        let shape_size = node_shape_size(node, options, resources)?;
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

    let index_by_id = node_indices_by_id(graph)?;
    for edge in &graph.edges {
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
    set_axis_size(row_heights, from.y.max(to.y) - 1, length_gap);
    if from.x == to.x
        && let Some(label) = edge.label.as_deref()
    {
        let label_width =
            GraphLabel::new_with_profile(label, options.terminal_width_profile).width();
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
                GraphLabel::new_with_profile(label, options.terminal_width_profile).width(),
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
) -> Result<Vec<GridCoord>> {
    let mut placements =
        place_ranked_grid_nodes(graph, topology, GraphDirection::TopDown, resources)?;
    if !graph.groups.is_empty() {
        groups::apply_group_placement_adjustments(
            graph,
            &mut placements,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
            resources,
        )?;
    }
    Ok(placements)
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
    let mut layouts = Vec::new();
    try_reserve_vec(&mut layouts, placements.len())?;
    for (coord, node) in placements.into_iter().zip(graph.nodes.iter()) {
        layouts.push(NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::new_with_profile(&node.label, width_profile),
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
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<GraphNodeShapeSize> {
    let label = GraphLabel::new_with_profile(&node.label, options.terminal_width_profile);
    GraphNodeShapeSemantics::new(node.shape).try_size_for_label(&label, options, resources)
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
