use super::super::label::GraphLabel;
use super::super::model::{
    AsciiGraph, AsciiGraphNode, GraphDirection, GraphNodeShape, GraphRootPolicy,
};
use super::groups;
use super::{GridCoord, NodeLayout};
use crate::options::AsciiRenderOptions;
use crate::text::display_width;
use std::collections::{BTreeMap, HashMap, HashSet};

pub(super) fn layout_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    match graph.direction.canonical() {
        GraphDirection::LeftRight => layout_left_right_grid_nodes(graph, options),
        GraphDirection::TopDown => layout_top_down_grid_nodes(graph, options),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    let placements = place_left_right_grid_nodes(graph);
    let mut column_widths = BTreeMap::new();
    let mut row_heights = BTreeMap::new();

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            node_width(node, options).saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
        }

        let height = node_height(node, options);
        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(&mut row_heights, coord.y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                groups::node_padding_y(graph, &placements, index, options),
            );
        }
    }

    let coord_by_id = graph
        .nodes
        .iter()
        .zip(placements.iter().copied())
        .map(|(node, coord)| (node.id.as_str(), coord))
        .collect::<HashMap<_, _>>();
    for edge in &graph.edges {
        let (Some(from), Some(to)) = (
            coord_by_id.get(edge.from.as_str()).copied(),
            coord_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        if to.x <= from.x {
            continue;
        }

        let length_gap = options
            .graph_padding_x
            .saturating_add(edge.length.saturating_sub(1) * 2);
        let label_gap = edge
            .label
            .as_deref()
            .map(|label| display_width(label) + 2)
            .unwrap_or_default();
        set_axis_size(&mut column_widths, to.x - 1, length_gap.max(label_gap));
    }

    let layouts = placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::new(&node.label),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: axis_position(&column_widths, coord.x),
            y: axis_position(&row_heights, coord.y),
            width: axis_span(&column_widths, coord.x, 3),
            height: axis_span(&row_heights, coord.y, 3),
        })
        .collect();

    (layouts, column_widths, row_heights)
}

fn place_left_right_grid_nodes(graph: &AsciiGraph) -> Vec<GridCoord> {
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = vec![None; graph.nodes.len()];
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();

    let root_indices = graph_root_indices(graph);
    let should_separate_roots =
        should_separate_left_right_roots(graph, &root_indices, &index_by_id);
    let (external_roots, subgraph_roots): (Vec<_>, Vec<_>) =
        root_indices.into_iter().partition(|root_index| {
            !should_separate_roots
                || groups::node_group_index(graph, &graph.nodes[*root_index].id).is_none()
        });

    for root_index in external_roots {
        place_left_right_node(
            root_index,
            0,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }
    for root_index in subgraph_roots {
        place_left_right_node(
            root_index,
            4,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }

    for node_index in 0..graph.nodes.len() {
        if placements[node_index].is_none() {
            place_left_right_node(
                node_index,
                0,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }

        let Some(parent_coord) = placements[node_index] else {
            continue;
        };
        let child_level = parent_coord.x + 4;
        for child_index in child_indices(graph, node_index, &index_by_id) {
            if placements[child_index].is_some() {
                continue;
            }
            place_left_right_node(
                child_index,
                child_level,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }
    }

    let mut placements = placements
        .into_iter()
        .map(|coord| coord.unwrap_or(GridCoord { x: 0, y: 0 }))
        .collect::<Vec<_>>();
    groups::apply_group_placement_adjustments(graph, &mut placements);
    placements
}

fn should_separate_left_right_roots(
    graph: &AsciiGraph,
    root_indices: &[usize],
    index_by_id: &HashMap<&str, usize>,
) -> bool {
    let has_external_roots = root_indices
        .iter()
        .any(|index| groups::node_group_index(graph, &graph.nodes[*index].id).is_none());
    let has_subgraph_roots_with_edges = root_indices.iter().any(|index| {
        groups::node_group_index(graph, &graph.nodes[*index].id).is_some()
            && !child_indices(graph, *index, index_by_id).is_empty()
    });

    has_external_roots && has_subgraph_roots_with_edges
}

fn graph_root_indices(graph: &AsciiGraph) -> Vec<usize> {
    let nodes_with_incoming = graph
        .edges
        .iter()
        .map(|edge| edge.to.as_str())
        .collect::<HashSet<_>>();

    let declared_first =
        graph.root_policy == GraphRootPolicy::DeclaredFirst && !graph.nodes.is_empty();
    let mut roots = Vec::new();
    if declared_first {
        roots.push(0);
    }

    let incoming_roots = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (!nodes_with_incoming.contains(node.id.as_str()) && (!declared_first || index != 0))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    roots.extend(incoming_roots);
    if roots.is_empty() && !graph.nodes.is_empty() {
        roots.push(0);
    }

    roots
}

fn child_indices<'a>(
    graph: &'a AsciiGraph,
    node_index: usize,
    index_by_id: &HashMap<&'a str, usize>,
) -> Vec<usize> {
    let node = &graph.nodes[node_index];
    graph
        .edges
        .iter()
        .filter(|edge| edge.from == node.id)
        .filter_map(|edge| index_by_id.get(edge.to.as_str()).copied())
        .collect()
}

fn place_left_right_node(
    node_index: usize,
    level: usize,
    placements: &mut [Option<GridCoord>],
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested_y = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = reserve_grid_spot(
        occupied,
        GridCoord {
            x: level,
            y: requested_y,
        },
        GraphDirection::LeftRight,
    );
    placements[node_index] = Some(coord);
    highest_position_per_level.insert(level, coord.y + 4);
}

pub(crate) fn reserve_grid_spot(
    occupied: &mut HashSet<(usize, usize)>,
    requested_coord: GridCoord,
    direction: GraphDirection,
) -> GridCoord {
    let mut coord = requested_coord;
    while grid_spot_occupied(occupied, coord) {
        match direction.canonical() {
            GraphDirection::LeftRight => coord.y += 4,
            GraphDirection::TopDown => coord.x += 4,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    for x in coord.x..(coord.x + 3) {
        for y in coord.y..(coord.y + 3) {
            occupied.insert((x, y));
        }
    }

    coord
}

fn grid_spot_occupied(occupied: &HashSet<(usize, usize)>, coord: GridCoord) -> bool {
    (coord.x..(coord.x + 3)).any(|x| (coord.y..(coord.y + 3)).any(|y| occupied.contains(&(x, y))))
}

fn layout_top_down_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    let placements = place_top_down_grid_nodes(graph);
    let mut column_widths = BTreeMap::new();
    let mut row_heights = BTreeMap::new();

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            node_width(node, options).saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
        }

        let height = node_height(node, options);
        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(&mut row_heights, coord.y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                groups::node_padding_y(graph, &placements, index, options),
            );
        }
    }

    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    for edge in &graph.edges {
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let from = placements[from_index];
        let to = placements[to_index];
        if to.y <= from.y || from.x != to.x {
            continue;
        }
        if let Some(label) = edge.label.as_deref() {
            set_axis_size(&mut column_widths, from.x + 1, display_width(label) + 2);
        }
    }

    let layouts = placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::new(&node.label),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: axis_position(&column_widths, coord.x),
            y: axis_position(&row_heights, coord.y),
            width: axis_span(&column_widths, coord.x, 3),
            height: axis_span(&row_heights, coord.y, 3),
        })
        .collect();

    (layouts, column_widths, row_heights)
}

fn place_top_down_grid_nodes(graph: &AsciiGraph) -> Vec<GridCoord> {
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = vec![None; graph.nodes.len()];
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();

    for root_index in graph_root_indices(graph) {
        place_top_down_node(
            root_index,
            0,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }

    for node_index in 0..graph.nodes.len() {
        if placements[node_index].is_none() {
            place_top_down_node(
                node_index,
                0,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }

        let Some(parent_coord) = placements[node_index] else {
            continue;
        };
        let child_level = parent_coord.y + 4;
        for child_index in child_indices(graph, node_index, &index_by_id) {
            if placements[child_index].is_some() {
                continue;
            }
            place_top_down_node(
                child_index,
                child_level,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }
    }

    let mut placements = placements
        .into_iter()
        .map(|coord| coord.unwrap_or(GridCoord { x: 0, y: 0 }))
        .collect::<Vec<_>>();
    groups::apply_group_placement_adjustments(graph, &mut placements);
    placements
}

fn place_top_down_node(
    node_index: usize,
    level: usize,
    placements: &mut [Option<GridCoord>],
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested_x = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = reserve_grid_spot(
        occupied,
        GridCoord {
            x: requested_x,
            y: level,
        },
        GraphDirection::TopDown,
    );
    placements[node_index] = Some(coord);
    highest_position_per_level.insert(level, coord.x + 4);
}

fn set_axis_size(axis_sizes: &mut BTreeMap<usize, usize>, index: usize, size: usize) {
    axis_sizes
        .entry(index)
        .and_modify(|current| *current = (*current).max(size))
        .or_insert(size);
}

pub(super) fn axis_position(axis_sizes: &BTreeMap<usize, usize>, index: usize) -> usize {
    axis_sizes
        .range(..index)
        .map(|(_, size)| *size)
        .sum::<usize>()
        + axis_sizes.get(&index).copied().unwrap_or_default() / 2
}

fn axis_span(axis_sizes: &BTreeMap<usize, usize>, start: usize, len: usize) -> usize {
    (start..(start + len))
        .map(|index| axis_sizes.get(&index).copied().unwrap_or_default())
        .sum()
}

fn node_height(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    match node.shape {
        GraphNodeShape::StateStart
        | GraphNodeShape::StateEnd
        | GraphNodeShape::ForkJoinHorizontal
        | GraphNodeShape::Choice => 3,
        GraphNodeShape::ForkJoinVertical => 7,
        GraphNodeShape::Rect
        | GraphNodeShape::Rounded
        | GraphNodeShape::Diamond
        | GraphNodeShape::Subroutine
        | GraphNodeShape::Cylinder => {
            2 + GraphLabel::new(&node.label).content_height() + options.box_border_padding * 2
        }
    }
}

fn node_width(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    let base = GraphLabel::new(&node.label).width() + options.box_border_padding * 2 + 2;
    match node.shape {
        GraphNodeShape::StateStart | GraphNodeShape::StateEnd | GraphNodeShape::Choice => 5,
        GraphNodeShape::ForkJoinHorizontal => 7,
        GraphNodeShape::ForkJoinVertical => 3,
        GraphNodeShape::Subroutine => base + 2,
        GraphNodeShape::Cylinder => base + 2,
        GraphNodeShape::Rect | GraphNodeShape::Rounded | GraphNodeShape::Diamond => base,
    }
}
