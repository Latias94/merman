use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;
use std::collections::{BTreeMap, HashMap, HashSet};
mod adapter;
mod model;

pub(crate) use adapter::from_flowchart_model;
pub(crate) use model::{AsciiGraph, GraphDirection};

use model::{AsciiGraphEdge, AsciiGraphNode, GraphEdgeArrow, GraphEdgeStroke, GraphNodeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphCharset {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    right_connector: char,
    down_connector: char,
    arrow_right: char,
    arrow_up: char,
    arrow_down: char,
    dotted_horizontal: char,
    dotted_vertical: char,
    corner_down_right: char,
    corner_right_up: char,
    rounded_top_left: char,
    rounded_top_right: char,
    rounded_bottom_left: char,
    rounded_bottom_right: char,
}

impl GraphCharset {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                right_connector: '|',
                down_connector: '-',
                arrow_right: '>',
                arrow_up: '^',
                arrow_down: 'v',
                dotted_horizontal: '.',
                dotted_vertical: ':',
                corner_down_right: '+',
                corner_right_up: '+',
                rounded_top_left: '/',
                rounded_top_right: '\\',
                rounded_bottom_left: '\\',
                rounded_bottom_right: '/',
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                right_connector: '├',
                down_connector: '┬',
                arrow_right: '►',
                arrow_up: '▲',
                arrow_down: '▼',
                dotted_horizontal: '┄',
                dotted_vertical: '┆',
                corner_down_right: '└',
                corner_right_up: '┘',
                rounded_top_left: '╭',
                rounded_top_right: '╮',
                rounded_bottom_left: '╰',
                rounded_bottom_right: '╯',
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeLayout {
    id: String,
    label: String,
    shape: GraphNodeShape,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl NodeLayout {
    fn center_x(&self) -> usize {
        self.x + self.width / 2
    }

    fn center_y(&self) -> usize {
        self.y + self.height / 2
    }

    fn right(&self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridCoord {
    x: usize,
    y: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupLayout {
    title: String,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl GroupLayout {
    fn right(&self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

pub(crate) fn render_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    if graph.nodes.is_empty() {
        return Ok(String::new());
    }

    let charset = GraphCharset::for_options(options);
    let layouts = layout_nodes(graph, options);
    let group_layouts = layout_groups(graph, &layouts);
    let width = layouts
        .iter()
        .map(|layout| layout.x + layout.width)
        .chain(group_layouts.iter().map(|layout| layout.x + layout.width))
        .max()
        .unwrap_or_default();
    let height = layouts
        .iter()
        .map(|layout| layout.y + layout.height)
        .chain(group_layouts.iter().map(|layout| layout.y + layout.height))
        .max()
        .unwrap_or_default();
    let actual_cells = width.saturating_mul(height);
    if actual_cells > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual: actual_cells,
            limit: options.max_grid_cells,
        });
    }

    let mut canvas = Canvas::new(width, height);
    for group in &group_layouts {
        draw_group(&mut canvas, group, &charset);
    }
    for layout in &layouts {
        draw_node(&mut canvas, layout, &charset, options);
    }
    for edge in &graph.edges {
        draw_edge(&mut canvas, &layouts, edge, graph.direction, &charset);
    }

    Ok(canvas.finish())
}

fn layout_nodes(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Vec<NodeLayout> {
    match graph.direction {
        GraphDirection::LeftRight => layout_left_right_grid_nodes(graph, options),
        GraphDirection::TopDown => layout_top_down_linear_nodes(graph, options),
    }
}

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> Vec<NodeLayout> {
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
        let label_gap = edge.label.as_deref().map(display_width).unwrap_or_default();
        set_axis_size(&mut column_widths, to.x - 1, length_gap.max(label_gap));
    }

    let group_offset_x = usize::from(!graph.groups.is_empty()) * 2;
    let group_offset_y = usize::from(!graph.groups.is_empty()) * 2;
    let label_y_offset = usize::from(graph.edges.iter().any(|edge| edge.label.is_some()));

    placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape,
            x: group_offset_x + axis_position(&column_widths, coord.x),
            y: group_offset_y + label_y_offset + axis_position(&row_heights, coord.y),
            width: axis_span(&column_widths, coord.x, 3),
            height: axis_span(&row_heights, coord.y, 3),
        })
        .collect()
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
) -> Vec<NodeLayout> {
    let group_offset_x = usize::from(!graph.groups.is_empty()) * 2;
    let group_offset_y = usize::from(!graph.groups.is_empty()) * 2;
    let measured = graph
        .nodes
        .iter()
        .map(|node| {
            let width = node_width(node, options);
            let height = node_height(options);
            (node, width, height)
        })
        .collect::<Vec<_>>();

    let canvas_width = measured
        .iter()
        .map(|(_, width, _)| *width)
        .max()
        .unwrap_or_default();
    let mut y = 0;
    measured
        .into_iter()
        .map(|(node, width, height)| {
            let layout = NodeLayout {
                id: node.id.clone(),
                label: node.label.clone(),
                shape: node.shape,
                x: group_offset_x + (canvas_width - width) / 2,
                y,
                width,
                height,
            };
            y += height + options.graph_padding_y;
            layout
        })
        .map(|mut layout| {
            layout.y += group_offset_y;
            layout
        })
        .collect()
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

fn layout_groups(graph: &AsciiGraph, layouts: &[NodeLayout]) -> Vec<GroupLayout> {
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
            let y = min_y.saturating_sub(2);
            let right = max_right + 2;
            let bottom = max_bottom + 1;
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

fn draw_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    match layout.shape {
        GraphNodeShape::Rect => draw_rect_node(canvas, layout, charset, options),
        GraphNodeShape::Rounded => draw_rounded_node(canvas, layout, charset, options),
        GraphNodeShape::Diamond => draw_diamond_node(canvas, layout, charset, options),
        GraphNodeShape::Subroutine => draw_subroutine_node(canvas, layout, charset, options),
        GraphNodeShape::Cylinder => draw_cylinder_node(canvas, layout, charset, options),
    }
}

fn draw_group(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    let right = group.right();
    let bottom = group.bottom();

    canvas.set(group.x, group.y, charset.top_left);
    canvas.set(right, group.y, charset.top_right);
    canvas.set(group.x, bottom, charset.bottom_left);
    canvas.set(right, bottom, charset.bottom_right);

    for x in (group.x + 1)..right {
        canvas.set(x, group.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    for y in (group.y + 1)..bottom {
        canvas.set(group.x, y, charset.vertical);
        canvas.set(right, y, charset.vertical);
    }

    let title = format!(" {} ", group.title);
    if display_width(&title) + 2 < group.width {
        canvas.write_text(group.x + 2, group.y, &title);
    }
}

fn draw_rect_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let right = layout.right();
    let bottom = layout.bottom();

    canvas.set(layout.x, layout.y, charset.top_left);
    canvas.set(right, layout.y, charset.top_right);
    canvas.set(layout.x, bottom, charset.bottom_left);
    canvas.set(right, bottom, charset.bottom_right);

    for x in (layout.x + 1)..right {
        canvas.set(x, layout.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    for y in (layout.y + 1)..bottom {
        canvas.set(layout.x, y, charset.vertical);
        canvas.set(right, y, charset.vertical);
    }

    write_centered_label(canvas, layout, options);
}

fn draw_rounded_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_node_with_corners(
        canvas,
        layout,
        charset,
        options,
        RoundedCorners {
            top_left: charset.rounded_top_left,
            top_right: charset.rounded_top_right,
            bottom_left: charset.rounded_bottom_left,
            bottom_right: charset.rounded_bottom_right,
        },
    );
}

#[derive(Debug, Clone, Copy)]
struct RoundedCorners {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
}

fn draw_node_with_corners(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    corners: RoundedCorners,
) {
    let right = layout.right();
    let bottom = layout.bottom();

    canvas.set(layout.x, layout.y, corners.top_left);
    canvas.set(right, layout.y, corners.top_right);
    canvas.set(layout.x, bottom, corners.bottom_left);
    canvas.set(right, bottom, corners.bottom_right);

    for x in (layout.x + 1)..right {
        canvas.set(x, layout.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    for y in (layout.y + 1)..bottom {
        canvas.set(layout.x, y, charset.vertical);
        canvas.set(right, y, charset.vertical);
    }

    write_centered_label(canvas, layout, options);
}

fn draw_diamond_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    _options: &AsciiRenderOptions,
) {
    let right = layout.right();
    let bottom = layout.bottom();
    let center_y = layout.center_y();

    canvas.set(layout.x, layout.y, charset.rounded_top_left);
    canvas.set(right, layout.y, charset.rounded_top_right);
    canvas.set(layout.x, layout.y + 1, charset.rounded_top_left);
    canvas.set(right, layout.y + 1, charset.rounded_top_right);
    canvas.set(layout.x, center_y, '<');
    canvas.set(right, center_y, '>');
    canvas.set(layout.x, bottom - 1, charset.rounded_bottom_left);
    canvas.set(right, bottom - 1, charset.rounded_bottom_right);
    canvas.set(layout.x, bottom, charset.rounded_bottom_left);
    canvas.set(right, bottom, charset.rounded_bottom_right);

    for x in (layout.x + 1)..right {
        canvas.set(x, layout.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    let text_width = display_width(&layout.label);
    let text_x = layout.x + centered_label_offset(layout.width, text_width);
    canvas.write_text(text_x, center_y, &layout.label);
}

fn draw_subroutine_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rect_node(canvas, layout, charset, options);
    if layout.width > 5 {
        let left_inner = layout.x + 2;
        let right_inner = layout.right().saturating_sub(2);
        for y in (layout.y + 1)..layout.bottom() {
            canvas.set(left_inner, y, charset.vertical);
            canvas.set(right_inner, y, charset.vertical);
        }
        let text_y = layout.y + 1 + options.box_border_padding;
        for x in (left_inner + 1)..right_inner {
            canvas.set(x, text_y, ' ');
        }
    }
    write_centered_label(canvas, layout, options);
}

fn draw_cylinder_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rounded_node(canvas, layout, charset, options);
    if layout.height > 3 {
        for x in (layout.x + 1)..layout.right() {
            canvas.set(x, layout.y + 1, charset.horizontal);
        }
    }
    let text_y = layout.y + 1 + options.box_border_padding;
    for x in (layout.x + 1)..layout.right() {
        canvas.set(x, text_y, ' ');
    }
    write_centered_label(canvas, layout, options);
}

fn write_centered_label(canvas: &mut Canvas, layout: &NodeLayout, options: &AsciiRenderOptions) {
    let text_width = display_width(&layout.label);
    let text_x = layout.x + centered_label_offset(layout.width, text_width);
    let text_y = layout.y + 1 + options.box_border_padding;
    canvas.write_text(text_x, text_y, &layout.label);
}

fn centered_label_offset(width: usize, text_width: usize) -> usize {
    let center = width.saturating_sub(1) / 2 + 1;
    center.saturating_sub(text_width.div_ceil(2))
}

fn draw_edge(
    canvas: &mut Canvas,
    layouts: &[NodeLayout],
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
    charset: &GraphCharset,
) {
    let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
        return;
    };
    let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => draw_left_right_edge(canvas, from, to, edge, charset),
        GraphDirection::TopDown => draw_top_down_edge(canvas, from, to, edge, charset),
    }

    draw_edge_label(canvas, from, to, edge, direction);
}

fn draw_left_right_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if from.center_y() < to.center_y() && to.x > from.x {
        draw_left_right_down_then_right_edge(canvas, from, to, edge, charset);
        return;
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        draw_left_right_down_edge(canvas, from, to, edge, charset);
        return;
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        draw_left_right_right_then_up_edge(canvas, from, to, edge, charset);
        return;
    }

    if to.x <= from.right() + 1 {
        return;
    }

    let y = from.center_y();
    if from.shape != GraphNodeShape::Diamond {
        canvas.set(from.right(), y, charset.right_connector);
    }
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    for x in start..end {
        canvas.set(x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(end, y, line),
        GraphEdgeArrow::Point => canvas.set(end, y, charset.arrow_right),
    }
}

fn draw_left_right_down_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    canvas.set(x, from.bottom(), charset.down_connector);
    for y in start..end {
        canvas.set(x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(x, end, line),
        GraphEdgeArrow::Point => canvas.set(x, end, charset.arrow_down),
    }
}

fn draw_left_right_down_then_right_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let x = from.center_x();
    let corner_y = to.center_y();
    if corner_y <= from.bottom() || to.x <= x + 1 {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..corner_y {
        canvas.set(x, y, vertical);
    }
    canvas.set(x, corner_y, charset.corner_down_right);

    let end = to.x - 1;
    for line_x in (x + 1)..end {
        canvas.set(line_x, corner_y, horizontal);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(end, corner_y, horizontal),
        GraphEdgeArrow::Point => canvas.set(end, corner_y, charset.arrow_right),
    }
}

fn draw_left_right_right_then_up_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let y = from.center_y();
    let corner_x = to.center_x();
    if corner_x <= from.right() || y <= to.bottom() + 1 {
        return;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    canvas.set(from.right(), y, charset.right_connector);
    for x in (from.right() + 1)..corner_x {
        canvas.set(x, y, horizontal);
    }
    canvas.set(corner_x, y, charset.corner_right_up);

    let arrow_y = to.bottom() + 1;
    for line_y in (arrow_y + 1)..y {
        canvas.set(corner_x, line_y, vertical);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(corner_x, arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(corner_x, arrow_y, charset.arrow_up),
    }
}

fn draw_top_down_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    for y in start..end {
        canvas.set(x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(x, end, line),
        GraphEdgeArrow::Point => canvas.set(x, end, charset.arrow_down),
    }
}

fn edge_line_char(
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    direction: GraphDirection,
) -> char {
    match (edge.stroke, direction) {
        (GraphEdgeStroke::Normal, GraphDirection::LeftRight) => charset.horizontal,
        (GraphEdgeStroke::Normal, GraphDirection::TopDown) => charset.vertical,
        (GraphEdgeStroke::Dotted, GraphDirection::LeftRight) => charset.dotted_horizontal,
        (GraphEdgeStroke::Dotted, GraphDirection::TopDown) => charset.dotted_vertical,
    }
}

fn draw_edge_label(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
) {
    let Some(label) = edge.label.as_deref() else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => {
            let start = from.right() + 1;
            let end = to.x.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let width = display_width(label);
            let x = start + available.saturating_sub(width) / 2;
            canvas.write_text(x, from.y.saturating_sub(1), label);
        }
        GraphDirection::TopDown => {
            let start = from.bottom() + 1;
            let end = to.y.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let y = start + available / 2;
            let width = display_width(label);
            let x = from.center_x().saturating_sub(width / 2);
            canvas.write_text(x, y, label);
        }
    }
}

#[cfg(test)]
mod graph_golden {
    use super::*;
    use crate::AsciiRenderOptions;
    use std::path::Path;

    fn fixture_expected(directory: &str, name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/mermaid-ascii")
            .join(directory)
            .join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            .replace("\r\n", "\n");
        let (_, expected) = content
            .split_once("\n---\n")
            .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
        expected.to_string()
    }

    #[test]
    fn single_node_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "single_node.txt"));
    }

    #[test]
    fn single_node_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "single_node.txt")
        );
    }

    #[test]
    fn two_nodes_linked_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "two_nodes_linked.txt"));
    }

    #[test]
    fn two_nodes_linked_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "two_nodes_linked.txt")
        );
    }

    #[test]
    fn long_node_labels_ascii_match_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("LongerName1", "LongerName1");
        graph.add_node("LongerName2", "LongerName2");
        graph.add_edge("LongerName1", "LongerName2");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("ascii", "two_nodes_longer_names.txt")
        );
    }

    #[test]
    fn top_down_chain_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_node("C", "C");
        graph.add_edge("A", "B");
        graph.add_edge("B", "C");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "flowchart_tb_simple.txt"));
    }
}
