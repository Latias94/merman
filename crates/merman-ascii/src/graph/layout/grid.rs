use super::super::label::GraphLabel;
use super::super::model::{AsciiGraph, AsciiGraphEdge, AsciiGraphNode, GraphDirection};
use super::super::shape::{GraphNodeShapeSemantics, GraphNodeShapeSize};
use super::groups;
use super::{GridCoord, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
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
const MINIMUM_GROUP_RANK_GAP: usize = 1;

pub(super) type AxisSizes = HashMap<usize, usize>;
type LevelPositions = HashMap<usize, usize>;
type NodeIndexById<'a> = HashMap<&'a str, usize>;
pub(super) type GridNodeLayoutParts = (Vec<NodeLayout>, AxisSizes, AxisSizes);

struct RankedGridPlacements {
    nodes: Vec<GridCoord>,
    leaf_group_levels: Vec<Option<usize>>,
}

struct DagreRankLevels {
    nodes: Vec<usize>,
    leaf_groups: Vec<Option<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GroupRankAnchor {
    Node(usize),
    Group(usize),
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
    let ranked =
        place_left_right_grid_nodes(graph, topology, options.terminal_width_profile, resources)?;
    let placements = ranked.nodes;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let axis_entity_count = resources.checked_grid_add(graph.nodes.len(), graph.groups.len())?;
    let mut column_widths = new_axis_sizes(axis_entity_count, resources)?;
    let mut row_heights = new_axis_sizes(axis_entity_count, resources)?;

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
) -> Result<RankedGridPlacements> {
    let mut ranked =
        place_ranked_grid_nodes(graph, topology, GraphDirection::LeftRight, resources)?;
    if !graph.groups.is_empty() {
        groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
            resources,
        )?;
    }
    Ok(ranked)
}

pub(super) fn place_ranked_grid_nodes_without_group_adjustments(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<Vec<GridCoord>> {
    Ok(place_ranked_grid_nodes(graph, None, direction.canonical(), resources)?.nodes)
}

fn place_ranked_grid_nodes(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
) -> Result<RankedGridPlacements> {
    let rank_levels = dagre_rank_levels(graph, topology, direction, resources)?;
    let parent_indices = rank_parent_indices(graph, &rank_levels.nodes, resources)?;
    let mut placement_order = index_order(graph.nodes.len())?;
    charge_sort_work(placement_order.len(), resources)?;
    placement_order
        .sort_unstable_by_key(|node_index| (rank_levels.nodes[*node_index], *node_index));

    let mut placements = Vec::new();
    try_reserve_vec(&mut placements, graph.nodes.len())?;
    placements.resize(graph.nodes.len(), None);
    let mut occupied = new_occupied_grid(graph.nodes.len(), resources)?;
    let mut highest_position_per_level = new_level_positions(graph.nodes.len())?;

    for node_index in placement_order {
        let level = rank_levels.nodes[node_index];
        let next_available = highest_position_per_level
            .get(&level)
            .copied()
            .unwrap_or_default();
        let requested = preferred_parent_lane(
            node_index,
            &rank_levels.nodes,
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

    Ok(RankedGridPlacements {
        nodes: finalize_ranked_placements(placements, graph.diagram_type())?,
        leaf_group_levels: rank_levels.leaf_groups,
    })
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
) -> Result<DagreRankLevels> {
    let (rank_graph, node_ids, group_ids, group_anchors) =
        build_dagre_rank_graph(graph, topology, direction, resources)?;
    let plan = {
        let mut work_control = AsciiDagreWorkControl::new(resources);
        match dugong::rank::plan_controlled(&rank_graph, &mut work_control) {
            Ok(plan) => plan,
            Err(error) => return Err(work_control.into_ascii_error(error, graph.diagram_type())),
        }
    };

    let endpoint_count = resources.checked_work_add(node_ids.len(), group_ids.len())?;
    let projection_work = resources.checked_work_add(
        plan.nodes.len(),
        resources.checked_work_mul(endpoint_count, 4)?,
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

    let mut leaf_group_ranks = Vec::new();
    try_reserve_vec(&mut leaf_group_ranks, group_ids.len())?;
    leaf_group_ranks.resize(group_ids.len(), None);
    for (group_index, anchor) in group_anchors.iter().copied().enumerate() {
        if anchor != GroupRankAnchor::Group(group_index) {
            continue;
        }
        let Some(group_id) = group_ids.get(group_index) else {
            continue;
        };
        let Some(rank) = rank_by_id.get(group_id.as_str()).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible rank planning for an empty compound group",
            });
        };
        leaf_group_ranks[group_index] = Some(rank);
    }

    // Dagre rank values can contain intentionally empty numeric ranks (especially around compound
    // borders). Terminal geometry needs the ordering and same-rank equivalence, not zero-width
    // canvas cells for those absent layers. Empty groups are real compound leaves, so retain their
    // occupied ranks alongside caller nodes without projecting them as phantom node layouts.
    let mut occupied_ranks = Vec::new();
    try_reserve_vec(&mut occupied_ranks, endpoint_count)?;
    occupied_ranks.extend(node_ranks.iter().copied());
    occupied_ranks.extend(leaf_group_ranks.iter().flatten().copied());
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

    let mut node_levels = Vec::new();
    try_reserve_vec(&mut node_levels, node_ranks.len())?;
    for rank in node_ranks {
        let Some(level) = dense_rank.get(&rank).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible dense rank projection",
            });
        };
        node_levels.push(level);
    }

    let mut leaf_group_levels = Vec::new();
    try_reserve_vec(&mut leaf_group_levels, leaf_group_ranks.len())?;
    for rank in leaf_group_ranks {
        let level = rank
            .map(|rank| {
                dense_rank
                    .get(&rank)
                    .copied()
                    .ok_or_else(|| AsciiError::UnsupportedFeature {
                        diagram_type: graph.diagram_type(),
                        feature: "Dagre-compatible dense empty-group rank projection",
                    })
            })
            .transpose()?;
        leaf_group_levels.push(level);
    }

    Ok(DagreRankLevels {
        nodes: node_levels,
        leaf_groups: leaf_group_levels,
    })
}

pub(super) fn rank_leaf_group_levels(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<Option<usize>>> {
    Ok(dagre_rank_levels(
        graph,
        Some(topology),
        graph.direction.canonical(),
        resources,
    )?
    .leaf_groups)
}

fn build_dagre_rank_graph(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &ResourceContext,
) -> Result<(
    Graph<NodeLabel, EdgeLabel, DagreGraphLabel>,
    Vec<String>,
    Vec<String>,
    Vec<GroupRankAnchor>,
)> {
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

    let node_order = graphlib_node_order(graph, resources)?;
    let mut node_ids = Vec::new();
    try_reserve_vec(&mut node_ids, graph.nodes.len())?;
    node_ids.resize_with(graph.nodes.len(), String::new);
    for (ordinal, node_index) in node_order.into_iter().enumerate() {
        let internal_id = format!("node:{ordinal}");
        rank_graph.set_node(internal_id.clone(), NodeLabel::default());
        node_ids[node_index] = internal_id;
    }

    let group_order = graphlib_group_order(graph, resources)?;
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
    let group_anchors = topology
        .map(|topology| group_rank_anchors(graph, topology, resources))
        .transpose()?
        .unwrap_or_default();
    let edge_order = graphlib_edge_order(graph)?;
    for (ordinal, edge_index) in edge_order.into_iter().enumerate() {
        let edge = &graph.edges[edge_index];
        let (Some(from_id), Some(to_id)) = (
            rank_endpoint_id(
                &edge.from,
                &index_by_id,
                topology,
                Some(group_anchors.as_slice()),
                &node_ids,
                &group_ids,
            ),
            rank_endpoint_id(
                &edge.to,
                &index_by_id,
                topology,
                Some(group_anchors.as_slice()),
                &node_ids,
                &group_ids,
            ),
        ) else {
            continue;
        };
        rank_graph.set_edge_named(
            from_id.to_string(),
            to_id.to_string(),
            Some(format!("edge:{ordinal}")),
            Some(EdgeLabel {
                minlen: edge.length,
                weight: 1.0,
                ..EdgeLabel::default()
            }),
        );
    }

    Ok((rank_graph, node_ids, group_ids, group_anchors))
}

fn graphlib_node_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.nodes.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(
            &graph.nodes[*left].id,
            *left,
            &graph.nodes[*right].id,
            *right,
        )
    });
    Ok(order)
}

fn graphlib_group_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.groups.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(
            &graph.groups[*left].id,
            *left,
            &graph.groups[*right].id,
            *right,
        )
    });
    Ok(order)
}

fn graphlib_creation_order_cmp(
    left: &str,
    left_creation_index: usize,
    right: &str,
    right_creation_index: usize,
) -> std::cmp::Ordering {
    // Graphlib stores node and child ids as JavaScript object properties. Array-index keys are
    // enumerated numerically; every other string retains its first-property-creation order.
    match (
        is_javascript_array_index(left),
        is_javascript_array_index(right),
    ) {
        (true, true) => match (left.parse::<u32>(), right.parse::<u32>()) {
            (Ok(left), Ok(right)) => left
                .cmp(&right)
                .then_with(|| left_creation_index.cmp(&right_creation_index)),
            _ => left
                .cmp(right)
                .then_with(|| left_creation_index.cmp(&right_creation_index)),
        },
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => left_creation_index.cmp(&right_creation_index),
    }
}

fn graphlib_edge_order(graph: &AsciiGraph) -> Result<Vec<usize>> {
    // Graphlib stores edge objects under delimiter-containing property keys, so `edges()` observes
    // their first `setEdge` creation order. Re-sorting by endpoint or label metadata changes DFS
    // cycle breaking and Network Simplex tie-breaking compared with pinned Mermaid.
    index_order(graph.edges.len())
}

fn group_rank_anchors(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &ResourceContext,
) -> Result<Vec<GroupRankAnchor>> {
    let mut anchors = Vec::new();
    try_reserve_vec(&mut anchors, graph.groups.len())?;
    for group_index in 0..graph.groups.len() {
        resources.charge_layout_work(1)?;
        anchors.push(find_non_cluster_child_anchor(
            graph,
            topology,
            group_index,
            resources,
        )?);
    }
    Ok(anchors)
}

fn find_non_cluster_child_anchor(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    root_group_index: usize,
    resources: &ResourceContext,
) -> Result<GroupRankAnchor> {
    let root_children =
        ordered_direct_group_children(graph, topology, root_group_index, resources)?;
    if root_children.is_empty() {
        return Ok(GroupRankAnchor::Group(root_group_index));
    }

    let mut stack = Vec::new();
    try_reserve_vec(&mut stack, root_children.len())?;
    stack.extend(root_children.into_iter().rev());
    let mut visited_groups = HashSet::new();
    try_reserve_hash_set(&mut visited_groups, graph.groups.len())?;
    let mut reserve = None;

    while let Some(endpoint) = stack.pop() {
        resources.charge_layout_work(1)?;
        let candidate = match endpoint {
            GraphEndpointIndex::Node(node_index) => GroupRankAnchor::Node(node_index),
            GraphEndpointIndex::Group(group_index) => {
                if !visited_groups.insert(group_index) {
                    continue;
                }
                let children =
                    ordered_direct_group_children(graph, topology, group_index, resources)?;
                if children.is_empty() {
                    GroupRankAnchor::Group(group_index)
                } else {
                    try_reserve_vec(&mut stack, children.len())?;
                    stack.extend(children.into_iter().rev());
                    continue;
                }
            }
        };

        if group_anchor_has_common_edge(graph, root_group_index, candidate, resources)? {
            reserve = Some(candidate);
        } else {
            return Ok(candidate);
        }
    }

    Ok(reserve.unwrap_or(GroupRankAnchor::Group(root_group_index)))
}

fn ordered_direct_group_children(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &ResourceContext,
) -> Result<Vec<GraphEndpointIndex>> {
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(Vec::new());
    };
    resources.charge_layout_work(group.nodes.len())?;
    let mut children = Vec::new();
    try_reserve_vec(&mut children, group.nodes.len())?;
    let mut seen = HashSet::new();
    try_reserve_hash_set(&mut seen, group.nodes.len())?;
    for (creation_index, member_id) in group.nodes.iter().enumerate() {
        let endpoint = match topology.endpoint_index(member_id) {
            Some(GraphEndpointIndex::Node(node_index))
                if topology.direct_node_group_index(member_id) == Some(group_index) =>
            {
                GraphEndpointIndex::Node(node_index)
            }
            Some(GraphEndpointIndex::Group(child_group_index))
                if child_group_index != group_index
                    && topology.parent_group_index(child_group_index) == Some(group_index) =>
            {
                GraphEndpointIndex::Group(child_group_index)
            }
            _ => continue,
        };
        if seen.insert(endpoint) {
            children.push((creation_index, member_id.as_str(), endpoint));
        }
    }
    charge_sort_work(children.len(), resources)?;
    children.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(left.1, left.0, right.1, right.0)
    });
    let mut ordered = Vec::new();
    try_reserve_vec(&mut ordered, children.len())?;
    ordered.extend(children.into_iter().map(|(_, _, endpoint)| endpoint));
    Ok(ordered)
}

fn group_anchor_has_common_edge(
    graph: &AsciiGraph,
    group_index: usize,
    candidate: GroupRankAnchor,
    resources: &ResourceContext,
) -> Result<bool> {
    let Some(group_id) = graph.groups.get(group_index).map(|group| group.id.as_str()) else {
        return Ok(false);
    };
    let candidate_id = match candidate {
        GroupRankAnchor::Node(node_index) => {
            graph.nodes.get(node_index).map(|node| node.id.as_str())
        }
        GroupRankAnchor::Group(candidate_group_index) => graph
            .groups
            .get(candidate_group_index)
            .map(|group| group.id.as_str()),
    };
    let Some(candidate_id) = candidate_id else {
        return Ok(false);
    };

    // Pinned Mermaid snapshots both incident edge lists and compares the endpoint pairs after its
    // intentionally asymmetric `w` rewrite. A hash set preserves that behavior in linear work.
    resources.charge_layout_work(resources.checked_work_mul(graph.edges.len(), 2)?)?;
    let mut candidate_edges = HashSet::new();
    try_reserve_hash_set(&mut candidate_edges, graph.edges.len())?;
    for edge in &graph.edges {
        if edge.from == candidate_id || edge.to == candidate_id {
            candidate_edges.insert((edge.from.as_str(), edge.to.as_str()));
        }
    }
    for edge in &graph.edges {
        if edge.from != group_id && edge.to != group_id {
            continue;
        }
        let rewritten = (
            if edge.from == group_id {
                candidate_id
            } else {
                edge.from.as_str()
            },
            if edge.to == group_id {
                group_id
            } else {
                edge.to.as_str()
            },
        );
        if candidate_edges.contains(&rewritten) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rank_endpoint_id<'a>(
    endpoint: &str,
    node_index_by_id: &NodeIndexById<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    group_anchors: Option<&[GroupRankAnchor]>,
    node_ids: &'a [String],
    group_ids: &'a [String],
) -> Option<&'a str> {
    let endpoint_index = match topology {
        Some(topology) => topology.endpoint_index(endpoint)?,
        None => GraphEndpointIndex::Node(node_index_by_id.get(endpoint).copied()?),
    };

    match endpoint_index {
        GraphEndpointIndex::Node(node_index) => node_ids.get(node_index).map(String::as_str),
        GraphEndpointIndex::Group(group_index) => {
            // Mermaid's Dagre wrapper does not leave an externally connected edge attached to a
            // non-empty compound node: it either extracts the cluster as an outer leaf or rewrites
            // the endpoint through `findNonClusterChild`. A childless group is already a real leaf
            // and keeps its own rank id; routing still targets the visible group boundary.
            match group_anchors?.get(group_index).copied()? {
                GroupRankAnchor::Node(node_index) => node_ids.get(node_index).map(String::as_str),
                GroupRankAnchor::Group(anchor_group_index) => {
                    group_ids.get(anchor_group_index).map(String::as_str)
                }
            }
        }
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
    let ranked =
        place_top_down_grid_nodes(graph, topology, options.terminal_width_profile, resources)?;
    let placements = ranked.nodes;
    let node_padding = groups::NodePaddingIndex::try_new(graph, &placements, topology, resources)?;
    let axis_entity_count = resources.checked_grid_add(graph.nodes.len(), graph.groups.len())?;
    let mut column_widths = new_axis_sizes(axis_entity_count, resources)?;
    let mut row_heights = new_axis_sizes(axis_entity_count, resources)?;

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
) -> Result<RankedGridPlacements> {
    let mut ranked = place_ranked_grid_nodes(graph, topology, GraphDirection::TopDown, resources)?;
    if !graph.groups.is_empty() {
        groups::apply_group_placement_adjustments(
            graph,
            &mut ranked.nodes,
            topology.expect("non-empty graph groups must have topology"),
            width_profile,
            resources,
        )?;
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
    fn graphlib_node_and_group_order_preserves_ordinary_creation_order() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["z", "10", "a", "2", "01", "4294967295"] {
            graph.add_node(id, id);
            graph.add_group_with_style(
                format!("group-{id}"),
                id,
                None,
                Vec::new(),
                GraphGroupStyle::default(),
            );
        }
        graph.add_group_with_style(
            "10",
            "numeric ten",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "2",
            "numeric two",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );

        let resources = unbounded_resources();
        let node_order = graphlib_node_order(&graph, &resources).unwrap();
        let node_ids = node_order
            .into_iter()
            .map(|index| graph.nodes[index].id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(node_ids, ["2", "10", "z", "a", "01", "4294967295"]);

        let group_order = graphlib_group_order(&graph, &resources).unwrap();
        let group_ids = group_order
            .into_iter()
            .map(|index| graph.groups[index].id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            group_ids,
            [
                "2",
                "10",
                "group-z",
                "group-10",
                "group-a",
                "group-2",
                "group-01",
                "group-4294967295",
            ]
        );
    }

    #[test]
    fn dagre_rank_graph_preserves_authored_edge_creation_order() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["z", "a", "m"] {
            graph.add_node(id, id);
        }
        graph.add_edge("m", "z");
        graph.add_edge("z", "a");
        graph.add_edge("a", "m");

        let mut resources = unbounded_resources();
        let (rank_graph, node_ids, _, _) =
            build_dagre_rank_graph(&graph, None, GraphDirection::TopDown, &mut resources).unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(edges.len(), 3);
        assert_eq!(
            (&edges[0].v, &edges[0].w, edges[0].name.as_deref()),
            (&node_ids[2], &node_ids[0], Some("edge:0"))
        );
        assert_eq!(
            (&edges[1].v, &edges[1].w, edges[1].name.as_deref()),
            (&node_ids[0], &node_ids[1], Some("edge:1"))
        );
        assert_eq!(
            (&edges[2].v, &edges[2].w, edges[2].name.as_deref()),
            (&node_ids[1], &node_ids[2], Some("edge:2"))
        );
    }

    #[test]
    fn state_composite_group_endpoints_enter_the_dagre_rank_graph() {
        let mut graph = AsciiGraph::new_for_diagram("state", GraphDirection::TopDown);
        graph.add_node("before", "Before");
        graph.add_node("child", "Child");
        graph.add_node("after", "After");
        graph.add_group_with_style(
            "Inner",
            "Inner",
            None,
            vec!["child".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "Parent",
            "Parent",
            None,
            vec!["Inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("before", "Parent");
        graph.add_edge("Parent", "after");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let (rank_graph, node_ids, _, _) = build_dagre_rank_graph(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
        )
        .unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(edges.len(), 2);
        assert_eq!((&edges[0].v, &edges[0].w), (&node_ids[0], &node_ids[1]));
        assert_eq!((&edges[1].v, &edges[1].w), (&node_ids[1], &node_ids[2]));

        let levels = dagre_rank_levels(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
        )
        .unwrap();
        assert!(levels.nodes[0] < levels.nodes[1]);
        assert!(levels.nodes[1] < levels.nodes[2]);
    }

    #[test]
    fn pinned_group_to_first_child_keeps_the_outgoing_asymmetry() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("group", "first");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        // Mermaid's pinned `findCommonEdges` rewrites the `w` side asymmetrically, so the
        // group-to-first-child case intentionally remains anchored to that first child.
        assert_eq!(anchors, [GroupRankAnchor::Node(0)]);
    }

    #[test]
    fn child_to_group_avoids_the_common_first_child_edge() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "group");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors, [GroupRankAnchor::Node(1)]);
    }

    #[test]
    fn nested_group_anchor_rechecks_descendants_against_the_root_group() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "outer");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors[1], GroupRankAnchor::Node(1));
    }

    #[test]
    fn outgoing_parallel_common_edge_uses_the_next_descendant() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_node("sink", "Sink");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("group", "sink");
        graph.add_edge("first", "sink");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors, [GroupRankAnchor::Node(1)]);
    }

    #[test]
    fn compound_anchor_scans_are_checked_against_the_layout_work_budget() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "group");

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources).unwrap();

        let measured_resources = ResourceContext::new(unbounded);
        group_rank_anchors(&graph, &topology, &measured_resources).unwrap();
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 0);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .unwrap();
        let exact_resources = ResourceContext::new(exact_policy);
        group_rank_anchors(&graph, &topology, &exact_resources)
            .expect("exact compound-anchor work budget should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .unwrap();
        let below_resources = ResourceContext::new(below_policy);
        let error = group_rank_anchors(&graph, &topology, &below_resources)
            .expect_err("max-minus-one compound-anchor work budget should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn empty_group_uses_its_real_compound_leaf_rank() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("after", "After");
        graph.add_group_with_style(
            "empty",
            "Empty",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        graph.add_edge("empty", "after");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let (rank_graph, node_ids, group_ids, anchors) = build_dagre_rank_graph(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
        )
        .unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(anchors, [GroupRankAnchor::Group(0)]);
        assert_eq!((&edges[0].v, &edges[0].w), (&group_ids[0], &node_ids[0]));

        let levels = dagre_rank_levels(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
        )
        .unwrap();
        assert!(levels.leaf_groups[0].unwrap() < levels.nodes[0]);
    }
}
