use super::model::{AsciiGraph, AsciiGraphNode, GraphDirection, GraphNodeShape};
use crate::options::AsciiRenderOptions;
use crate::text::display_width;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLayout {
    pub(super) nodes: Vec<NodeLayout>,
    pub(super) groups: Vec<GroupLayout>,
    column_widths: BTreeMap<usize, usize>,
    row_heights: BTreeMap<usize, usize>,
    offset_x: usize,
    offset_y: usize,
}

impl GraphLayout {
    pub(super) fn grid_to_canvas(&self, coord: GridCoord) -> CanvasCoord {
        CanvasCoord {
            x: self.offset_x + axis_position(&self.column_widths, coord.x),
            y: self.offset_y + axis_position(&self.row_heights, coord.y),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NodeLayout {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) shape: GraphNodeShape,
    pub(super) grid: GridCoord,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl NodeLayout {
    pub(super) fn center_x(&self) -> usize {
        self.x + self.width / 2
    }

    pub(super) fn center_y(&self) -> usize {
        self.y + self.height / 2
    }

    pub(super) fn right(&self) -> usize {
        self.x + self.width - 1
    }

    pub(super) fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GridCoord {
    pub(super) x: usize,
    pub(super) y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CanvasCoord {
    pub(super) x: usize,
    pub(super) y: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GroupLayout {
    pub(super) title: String,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl GroupLayout {
    pub(super) fn right(&self) -> usize {
        self.x + self.width - 1
    }

    pub(super) fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

pub(super) fn layout_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> GraphLayout {
    let (nodes, column_widths, row_heights) = layout_nodes(graph, options);
    let offset_x = nodes
        .first()
        .map(|node| {
            node.x
                .saturating_sub(axis_position(&column_widths, node.grid.x))
        })
        .unwrap_or_default();
    let offset_y = nodes
        .first()
        .map(|node| {
            node.y
                .saturating_sub(axis_position(&row_heights, node.grid.y))
        })
        .unwrap_or_default();
    let groups = layout_groups(graph, &nodes);
    GraphLayout {
        nodes,
        groups,
        column_widths,
        row_heights,
        offset_x,
        offset_y,
    }
}

fn layout_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    match graph.direction {
        GraphDirection::LeftRight => layout_left_right_grid_nodes(graph, options),
        GraphDirection::TopDown => layout_top_down_linear_nodes(graph, options),
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

        let height = node_height(options);
        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(&mut row_heights, coord.y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(&mut row_heights, coord.y - 1, options.graph_padding_y);
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

    let has_groups = has_non_empty_group(graph);
    let group_offset_x = usize::from(has_groups) * 2;
    let group_offset_y = usize::from(has_groups) * 4;

    let layouts = placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape,
            grid: coord,
            x: group_offset_x + axis_position(&column_widths, coord.x),
            y: group_offset_y + axis_position(&row_heights, coord.y),
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

    for root_index in left_right_root_indices(graph) {
        place_left_right_node(
            root_index,
            0,
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

    placements
        .into_iter()
        .map(|coord| coord.unwrap_or(GridCoord { x: 0, y: 0 }))
        .collect()
}

fn left_right_root_indices(graph: &AsciiGraph) -> Vec<usize> {
    let mut nodes_found = HashSet::new();
    let mut roots = Vec::new();

    for (index, node) in graph.nodes.iter().enumerate() {
        if !nodes_found.contains(node.id.as_str()) {
            roots.push(index);
        }
        nodes_found.insert(node.id.as_str());
        for edge in graph.edges.iter().filter(|edge| edge.from == node.id) {
            nodes_found.insert(edge.to.as_str());
        }
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
    );
    placements[node_index] = Some(coord);
    highest_position_per_level.insert(level, coord.y + 4);
}

fn reserve_grid_spot(
    occupied: &mut HashSet<(usize, usize)>,
    requested_coord: GridCoord,
) -> GridCoord {
    let mut coord = requested_coord;
    while grid_spot_occupied(occupied, coord) {
        coord.y += 4;
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

fn layout_top_down_linear_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    let has_groups = has_non_empty_group(graph);
    let group_offset_x = usize::from(has_groups) * 2;
    let group_offset_y = usize::from(has_groups) * 4;
    let mut column_widths = BTreeMap::new();
    let mut row_heights = BTreeMap::new();
    let measured = graph
        .nodes
        .iter()
        .map(|node| {
            let width = node_width(node, options);
            let height = node_height(options);
            (node, width, height)
        })
        .collect::<Vec<_>>();

    let mut canvas_width = measured
        .iter()
        .map(|(_, width, _)| *width)
        .max()
        .unwrap_or_default();
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
        if to_index <= from_index {
            continue;
        }
        if let Some(label) = edge.label.as_deref() {
            canvas_width = canvas_width.max(display_width(label) + 4);
        }
    }
    let mut y = 0;
    for (index, (_, width, height)) in measured.iter().enumerate() {
        let grid_y = index * 4;
        let node_width = canvas_width.max(*width);
        set_axis_size(&mut column_widths, 0, 1);
        set_axis_size(&mut column_widths, 1, node_width.saturating_sub(2));
        set_axis_size(&mut column_widths, 2, 1);
        set_axis_size(&mut row_heights, grid_y, 1);
        set_axis_size(&mut row_heights, grid_y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, grid_y + 2, 1);
        if grid_y > 0 {
            set_axis_size(&mut row_heights, grid_y - 1, options.graph_padding_y);
        }
    }

    let layouts = measured
        .into_iter()
        .enumerate()
        .map(|(index, (node, width, height))| {
            let grid = GridCoord { x: 0, y: index * 4 };
            let layout_width = canvas_width.max(width);
            let layout = NodeLayout {
                id: node.id.clone(),
                label: node.label.clone(),
                shape: node.shape,
                grid,
                x: group_offset_x + (canvas_width - layout_width) / 2,
                y,
                width: layout_width,
                height,
            };
            y += height + options.graph_padding_y;
            layout
        })
        .map(|mut layout| {
            layout.y += group_offset_y;
            layout
        })
        .collect();

    (layouts, column_widths, row_heights)
}

fn set_axis_size(axis_sizes: &mut BTreeMap<usize, usize>, index: usize, size: usize) {
    axis_sizes
        .entry(index)
        .and_modify(|current| *current = (*current).max(size))
        .or_insert(size);
}

fn axis_position(axis_sizes: &BTreeMap<usize, usize>, index: usize) -> usize {
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

fn node_height(options: &AsciiRenderOptions) -> usize {
    1 + options.box_border_padding * 2 + 2
}

fn has_non_empty_group(graph: &AsciiGraph) -> bool {
    graph.groups.iter().any(|group| {
        group
            .nodes
            .iter()
            .any(|group_node| graph.nodes.iter().any(|node| node.id == *group_node))
    })
}

pub(super) fn layout_groups(graph: &AsciiGraph, layouts: &[NodeLayout]) -> Vec<GroupLayout> {
    graph
        .groups
        .iter()
        .filter_map(|group| {
            let members = layouts
                .iter()
                .filter(|layout| group.nodes.iter().any(|node| node == &layout.id))
                .collect::<Vec<_>>();
            if members.is_empty() {
                return None;
            }

            let min_x = members.iter().map(|layout| layout.x).min().unwrap_or(0);
            let min_y = members.iter().map(|layout| layout.y).min().unwrap_or(0);
            let max_right = members
                .iter()
                .map(|layout| layout.right())
                .max()
                .unwrap_or(0);
            let max_bottom = members
                .iter()
                .map(|layout| layout.bottom())
                .max()
                .unwrap_or(0);
            let x = min_x.saturating_sub(2);
            let y = min_y.saturating_sub(4);
            let right = max_right + 2;
            let bottom = max_bottom + 2;
            let min_width = display_width(&group.title) + 4;
            let width = (right - x + 1).max(min_width);
            let height = bottom - y + 1;

            Some(GroupLayout {
                title: group.title.clone(),
                x,
                y,
                width,
                height,
            })
        })
        .collect()
}

fn node_width(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    let base = display_width(&node.label) + options.box_border_padding * 2 + 2;
    match node.shape {
        GraphNodeShape::Subroutine => base + 2,
        GraphNodeShape::Cylinder => base + 2,
        GraphNodeShape::Rect | GraphNodeShape::Rounded | GraphNodeShape::Diamond => base,
    }
}
