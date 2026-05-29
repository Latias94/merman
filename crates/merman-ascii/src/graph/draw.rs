use super::charset::GraphCharset;
use super::label::GRAPH_LABEL_LINE_GAP;
use super::layout::{GroupLayout, NodeLayout, layout_graph};
use super::model::{AsciiGraph, GraphNodeShape};
use super::routing;
use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::text::display_width;
use std::collections::HashSet;

pub(crate) fn render_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    if graph.nodes.is_empty() {
        return Ok(String::new());
    }

    let charset = GraphCharset::for_options(options);
    let graph_layout = layout_graph(graph, options);
    let (edge_width, edge_height) =
        routing::edge_canvas_extent(&graph_layout.nodes, &graph.edges, graph.direction);
    let width = graph_layout
        .nodes
        .iter()
        .map(|layout| layout.x + layout.width)
        .chain(
            graph_layout
                .groups
                .iter()
                .map(|layout| layout.x + layout.width),
        )
        .chain(std::iter::once(edge_width))
        .max()
        .unwrap_or_default();
    let height = graph_layout
        .nodes
        .iter()
        .map(|layout| layout.y + layout.height)
        .chain(
            graph_layout
                .groups
                .iter()
                .map(|layout| layout.y + layout.height),
        )
        .chain(std::iter::once(edge_height))
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
    let mut route_cells = HashSet::new();
    let mut edge_labels = Vec::new();
    for group in &graph_layout.groups {
        draw_group(&mut canvas, group, &charset);
    }
    for layout in &graph_layout.nodes {
        draw_node(&mut canvas, layout, &charset, options);
    }
    {
        let mut route_drawing =
            routing::RouteDrawing::new(&mut canvas, &mut route_cells, &mut edge_labels);
        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from == edge.to)
        {
            routing::draw_edge(
                &mut route_drawing,
                &graph_layout,
                &graph.edges,
                edge_index,
                edge,
                graph.direction,
                &charset,
            );
        }
        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from != edge.to)
        {
            routing::draw_edge(
                &mut route_drawing,
                &graph_layout,
                &graph.edges,
                edge_index,
                edge,
                graph.direction,
                &charset,
            );
        }
    }
    for label in &edge_labels {
        routing::draw_routed_label(&mut canvas, label);
    }
    for group in &graph_layout.groups {
        draw_group_title(&mut canvas, group);
    }

    Ok(canvas.finish())
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
}

fn draw_group_title(canvas: &mut Canvas, group: &GroupLayout) {
    let title_width = display_width(&group.title);
    if title_width > group.width.saturating_sub(2) {
        return;
    }

    let title_x = (group.x + group.width.saturating_sub(1) / 2)
        .saturating_sub(title_width / 2)
        .max(group.x + 1);
    canvas.write_text(title_x, group.y + 1, &group.title);
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
    options: &AsciiRenderOptions,
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

    write_centered_label(canvas, layout, options);
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

fn write_centered_label(canvas: &mut Canvas, layout: &NodeLayout, _options: &AsciiRenderOptions) {
    let inner_height = layout.height.saturating_sub(2);
    let content_height = layout.label.content_height();
    let content_y = layout.y + 1 + inner_height.saturating_sub(content_height) / 2;

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = display_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let text_y = content_y + line_index * (GRAPH_LABEL_LINE_GAP + 1);
        canvas.write_text(text_x, text_y, line);
    }
}

fn centered_label_offset(width: usize, text_width: usize) -> usize {
    let center = width.saturating_sub(1) / 2 + 1;
    center.saturating_sub(text_width.div_ceil(2))
}
