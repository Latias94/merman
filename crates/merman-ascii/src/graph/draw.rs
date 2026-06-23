use super::charset::GraphCharset;
use super::label::GRAPH_LABEL_LINE_GAP;
use super::layout::{CanvasCoord, GroupLayout, NodeLayout, layout_graph};
use super::model::{
    AsciiGraph, GraphDirection, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle,
};
use super::routing;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::terminal::char_display_width;
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
        routing::edge_canvas_extent(graph, &graph_layout, &graph.edges, graph.direction);
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
                routing::DrawEdgeRequest {
                    graph,
                    graph_layout: &graph_layout,
                    edges: &graph.edges,
                    edge_index,
                    edge,
                    charset: &charset,
                },
            )?;
        }
        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from != edge.to)
        {
            routing::draw_edge(
                &mut route_drawing,
                routing::DrawEdgeRequest {
                    graph,
                    graph_layout: &graph_layout,
                    edges: &graph.edges,
                    edge_index,
                    edge,
                    charset: &charset,
                },
            )?;
        }
    }

    let output_transform = OutputTransform::for_direction(graph.direction);
    if output_transform.is_identity() {
        for label in &edge_labels {
            routing::draw_routed_label(&mut canvas, label);
        }
        for group in &graph_layout.groups {
            draw_group_title(&mut canvas, group);
        }
        return Ok(canvas.finish_with_options(options));
    }

    let mut canvas = output_transform.transform_canvas(canvas, width, height);
    redraw_transformed_node_labels(
        &mut canvas,
        &graph_layout.nodes,
        output_transform,
        width,
        height,
    );
    for label in &edge_labels {
        let label = routing::transform_routed_label(label, |coord| {
            output_transform.coord(coord, width, height)
        });
        routing::draw_routed_label(&mut canvas, &label);
    }
    for group in &graph_layout.groups {
        draw_transformed_group_title(&mut canvas, group, output_transform, width, height);
    }

    Ok(canvas.finish_with_options(options))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputTransform {
    Identity,
    HorizontalMirror,
    VerticalMirror,
}

impl OutputTransform {
    fn for_direction(direction: GraphDirection) -> Self {
        match direction {
            GraphDirection::LeftRight | GraphDirection::TopDown => Self::Identity,
            GraphDirection::RightLeft => Self::HorizontalMirror,
            GraphDirection::BottomTop => Self::VerticalMirror,
        }
    }

    fn is_identity(self) -> bool {
        self == Self::Identity
    }

    fn coord(self, coord: CanvasCoord, width: usize, height: usize) -> CanvasCoord {
        match self {
            Self::Identity => coord,
            Self::HorizontalMirror => CanvasCoord {
                x: width.saturating_sub(1).saturating_sub(coord.x),
                y: coord.y,
            },
            Self::VerticalMirror => CanvasCoord {
                x: coord.x,
                y: height.saturating_sub(1).saturating_sub(coord.y),
            },
        }
    }

    fn transform_canvas(self, source: Canvas, width: usize, height: usize) -> Canvas {
        let mut canvas = Canvas::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let Some(ch) = source.get(x, y) else {
                    continue;
                };
                let coord = self.coord_for_char(CanvasCoord { x, y }, ch, width, height);
                let ch = self.map_char(ch);
                if let Some(color) = source.get_color(x, y) {
                    canvas.set_canvas_color(coord.x, coord.y, ch, color);
                } else {
                    canvas.set(coord.x, coord.y, ch);
                }
            }
        }
        canvas
    }

    fn coord_for_char(
        self,
        coord: CanvasCoord,
        ch: char,
        width: usize,
        height: usize,
    ) -> CanvasCoord {
        match self {
            Self::HorizontalMirror => CanvasCoord {
                x: width
                    .saturating_sub(coord.x)
                    .saturating_sub(char_display_width(ch)),
                y: coord.y,
            },
            Self::Identity | Self::VerticalMirror => self.coord(coord, width, height),
        }
    }

    fn text_x(self, x: usize, text: &str, width: usize) -> usize {
        match self {
            Self::HorizontalMirror => width.saturating_sub(x).saturating_sub(display_width(text)),
            Self::Identity | Self::VerticalMirror => x,
        }
    }

    fn text_y(self, y: usize, height: usize) -> usize {
        match self {
            Self::VerticalMirror => height.saturating_sub(1).saturating_sub(y),
            Self::Identity | Self::HorizontalMirror => y,
        }
    }

    fn map_char(self, ch: char) -> char {
        match self {
            Self::Identity => ch,
            Self::HorizontalMirror => mirror_horizontal_char(ch),
            Self::VerticalMirror => mirror_vertical_char(ch),
        }
    }
}

fn mirror_horizontal_char(ch: char) -> char {
    match ch {
        '>' => '<',
        '<' => '>',
        '►' => '◄',
        '◄' => '►',
        '/' => '\\',
        '\\' => '/',
        '┌' => '┐',
        '┐' => '┌',
        '└' => '┘',
        '┘' => '└',
        '├' => '┤',
        '┤' => '├',
        '╭' => '╮',
        '╮' => '╭',
        '╰' => '╯',
        '╯' => '╰',
        ch => ch,
    }
}

fn mirror_vertical_char(ch: char) -> char {
    match ch {
        '^' => 'v',
        'v' => '^',
        '▲' => '▼',
        '▼' => '▲',
        '/' => '\\',
        '\\' => '/',
        '┌' => '└',
        '└' => '┌',
        '┐' => '┘',
        '┘' => '┐',
        '┬' => '┴',
        '┴' => '┬',
        '╭' => '╰',
        '╰' => '╭',
        '╮' => '╯',
        '╯' => '╮',
        ch => ch,
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
        GraphNodeShape::StateStart => draw_state_start_node(canvas, layout, charset),
        GraphNodeShape::StateEnd => draw_state_end_node(canvas, layout, charset),
        GraphNodeShape::ForkJoinHorizontal => draw_fork_join_node(canvas, layout, charset, false),
        GraphNodeShape::ForkJoinVertical => draw_fork_join_node(canvas, layout, charset, true),
        GraphNodeShape::Choice => draw_choice_node(canvas, layout),
    }
}

fn draw_group(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    match group.kind {
        GraphGroupKind::Container => draw_group_box(canvas, group, charset),
        GraphGroupKind::Divider => draw_group_divider(canvas, group, charset),
    }
}

fn draw_group_box(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    let right = group.right();
    let bottom = group.bottom();

    set_group_border(canvas, group.x, group.y, charset.top_left, group.style);
    set_group_border(canvas, right, group.y, charset.top_right, group.style);
    set_group_border(canvas, group.x, bottom, charset.bottom_left, group.style);
    set_group_border(canvas, right, bottom, charset.bottom_right, group.style);

    for x in (group.x + 1)..right {
        set_group_border(canvas, x, group.y, charset.horizontal, group.style);
        set_group_border(canvas, x, bottom, charset.horizontal, group.style);
    }

    for y in (group.y + 1)..bottom {
        set_group_border(canvas, group.x, y, charset.vertical, group.style);
        set_group_border(canvas, right, y, charset.vertical, group.style);
    }
}

fn draw_group_divider(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    let Some(span) = group.divider_span else {
        return;
    };
    for x in span.x_start..=span.x_end {
        set_group_border(canvas, x, group.y, charset.dotted_horizontal, group.style);
    }
}

fn draw_group_title(canvas: &mut Canvas, group: &GroupLayout) {
    if group.kind == GraphGroupKind::Divider {
        return;
    }
    for (line_index, line) in group.title.lines().iter().enumerate() {
        let Some((title_x, title_y)) = group_title_line_position(group, line, line_index) else {
            continue;
        };
        write_group_title(canvas, title_x, title_y, line, group.style);
    }
}

fn draw_transformed_group_title(
    canvas: &mut Canvas,
    group: &GroupLayout,
    transform: OutputTransform,
    width: usize,
    height: usize,
) {
    if group.kind == GraphGroupKind::Divider {
        return;
    }
    let line_step = GRAPH_LABEL_LINE_GAP + 1;
    let content_y = group.y + 1;
    let last_line_y = content_y + group.title.lines().len().saturating_sub(1) * line_step;
    let transformed_content_y = match transform {
        OutputTransform::VerticalMirror => height.saturating_sub(1).saturating_sub(last_line_y),
        OutputTransform::Identity | OutputTransform::HorizontalMirror => content_y,
    };

    for (line_index, line) in group.title.lines().iter().enumerate() {
        let Some((title_x, _)) = group_title_line_position(group, line, line_index) else {
            continue;
        };
        write_group_title(
            canvas,
            transform.text_x(title_x, line, width),
            transformed_content_y + line_index * line_step,
            line,
            group.style,
        );
    }
}

fn group_title_line_position(
    group: &GroupLayout,
    line: &str,
    line_index: usize,
) -> Option<(usize, usize)> {
    let title_width = display_width(line);
    if title_width > group.width.saturating_sub(2) {
        return None;
    }

    let title_x = (group.x + group.width.saturating_sub(1) / 2)
        .saturating_sub(title_width / 2)
        .max(group.x + 1);
    Some((
        title_x,
        group.y + 1 + line_index * (GRAPH_LABEL_LINE_GAP + 1),
    ))
}

fn set_group_border(canvas: &mut Canvas, x: usize, y: usize, ch: char, style: GraphGroupStyle) {
    if let Some(color) = style.border {
        canvas.set_color(x, y, ch, color);
    } else {
        canvas.set_role(x, y, ch, AsciiColorRole::GroupBorder);
    }
}

fn write_group_title(canvas: &mut Canvas, x: usize, y: usize, text: &str, style: GraphGroupStyle) {
    if let Some(color) = style.title {
        canvas.write_text_color(x, y, text, color);
    } else {
        canvas.write_text_role(x, y, text, AsciiColorRole::MutedText);
    }
}

fn set_node_border(canvas: &mut Canvas, x: usize, y: usize, ch: char, style: GraphNodeStyle) {
    if let Some(color) = style.border {
        canvas.set_color(x, y, ch, color);
    } else {
        canvas.set_role(x, y, ch, AsciiColorRole::NodeBorder);
    }
}

fn write_node_text(canvas: &mut Canvas, x: usize, y: usize, text: &str, style: GraphNodeStyle) {
    if let Some(color) = style.text {
        canvas.write_text_color(x, y, text, color);
    } else {
        canvas.write_text_role(x, y, text, AsciiColorRole::Text);
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

    set_node_border(canvas, layout.x, layout.y, charset.top_left, layout.style);
    set_node_border(canvas, right, layout.y, charset.top_right, layout.style);
    set_node_border(canvas, layout.x, bottom, charset.bottom_left, layout.style);
    set_node_border(canvas, right, bottom, charset.bottom_right, layout.style);

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style);
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style);
    }

    for y in (layout.y + 1)..bottom {
        set_node_border(canvas, layout.x, y, charset.vertical, layout.style);
        set_node_border(canvas, right, y, charset.vertical, layout.style);
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

    set_node_border(canvas, layout.x, layout.y, corners.top_left, layout.style);
    set_node_border(canvas, right, layout.y, corners.top_right, layout.style);
    set_node_border(canvas, layout.x, bottom, corners.bottom_left, layout.style);
    set_node_border(canvas, right, bottom, corners.bottom_right, layout.style);

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style);
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style);
    }

    for y in (layout.y + 1)..bottom {
        set_node_border(canvas, layout.x, y, charset.vertical, layout.style);
        set_node_border(canvas, right, y, charset.vertical, layout.style);
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

    set_node_border(
        canvas,
        layout.x,
        layout.y,
        charset.rounded_top_left,
        layout.style,
    );
    set_node_border(
        canvas,
        right,
        layout.y,
        charset.rounded_top_right,
        layout.style,
    );
    set_node_border(
        canvas,
        layout.x,
        layout.y + 1,
        charset.rounded_top_left,
        layout.style,
    );
    set_node_border(
        canvas,
        right,
        layout.y + 1,
        charset.rounded_top_right,
        layout.style,
    );
    set_node_border(canvas, layout.x, center_y, '<', layout.style);
    set_node_border(canvas, right, center_y, '>', layout.style);
    set_node_border(
        canvas,
        layout.x,
        bottom - 1,
        charset.rounded_bottom_left,
        layout.style,
    );
    set_node_border(
        canvas,
        right,
        bottom - 1,
        charset.rounded_bottom_right,
        layout.style,
    );
    set_node_border(
        canvas,
        layout.x,
        bottom,
        charset.rounded_bottom_left,
        layout.style,
    );
    set_node_border(
        canvas,
        right,
        bottom,
        charset.rounded_bottom_right,
        layout.style,
    );

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style);
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style);
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
            set_node_border(canvas, left_inner, y, charset.vertical, layout.style);
            set_node_border(canvas, right_inner, y, charset.vertical, layout.style);
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
            set_node_border(canvas, x, layout.y + 1, charset.horizontal, layout.style);
        }
    }
    let text_y = layout.y + 1 + options.box_border_padding;
    for x in (layout.x + 1)..layout.right() {
        canvas.set(x, text_y, ' ');
    }
    write_centered_label(canvas, layout, options);
}

fn draw_state_start_node(canvas: &mut Canvas, layout: &NodeLayout, charset: &GraphCharset) {
    let symbol = if charset.unicode { '●' } else { '*' };
    draw_state_pseudo_node(canvas, layout, charset, symbol);
}

fn draw_state_end_node(canvas: &mut Canvas, layout: &NodeLayout, charset: &GraphCharset) {
    let symbol = if charset.unicode { '◎' } else { '@' };
    draw_state_pseudo_node(canvas, layout, charset, symbol);
}

fn draw_state_pseudo_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    symbol: char,
) {
    draw_node_with_corners(
        canvas,
        layout,
        charset,
        &AsciiRenderOptions::default(),
        RoundedCorners {
            top_left: charset.rounded_top_left,
            top_right: charset.rounded_top_right,
            bottom_left: charset.rounded_bottom_left,
            bottom_right: charset.rounded_bottom_right,
        },
    );
    let symbol = symbol.to_string();
    write_node_text(
        canvas,
        layout.center_x(),
        layout.center_y(),
        &symbol,
        layout.style,
    );
}

fn draw_fork_join_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    vertical: bool,
) {
    let ch = if vertical {
        charset.thick_vertical
    } else {
        charset.thick_horizontal
    };
    for y in layout.y..=layout.bottom() {
        for x in layout.x..=layout.right() {
            set_node_border(canvas, x, y, ch, layout.style);
        }
    }
}

fn draw_choice_node(canvas: &mut Canvas, layout: &NodeLayout) {
    let center_x = layout.center_x();
    let center_y = layout.center_y();
    set_node_border(
        canvas,
        center_x.saturating_sub(1),
        layout.y,
        '/',
        layout.style,
    );
    set_node_border(canvas, center_x + 1, layout.y, '\\', layout.style);
    set_node_border(canvas, layout.x, center_y, '<', layout.style);
    set_node_border(canvas, layout.right(), center_y, '>', layout.style);
    set_node_border(
        canvas,
        center_x.saturating_sub(1),
        layout.bottom(),
        '\\',
        layout.style,
    );
    set_node_border(canvas, center_x + 1, layout.bottom(), '/', layout.style);
}

fn write_centered_label(canvas: &mut Canvas, layout: &NodeLayout, _options: &AsciiRenderOptions) {
    let inner_height = layout.height.saturating_sub(2);
    let content_height = layout.label.content_height();
    let content_y = layout.y + 1 + inner_height.saturating_sub(content_height) / 2;

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = display_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let text_y = content_y + line_index * (GRAPH_LABEL_LINE_GAP + 1);
        write_node_text(canvas, text_x, text_y, line, layout.style);
    }
}

fn redraw_transformed_node_labels(
    canvas: &mut Canvas,
    layouts: &[NodeLayout],
    transform: OutputTransform,
    width: usize,
    height: usize,
) {
    for layout in layouts {
        redraw_transformed_node_label(canvas, layout, transform, width, height);
    }
}

fn redraw_transformed_node_label(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    transform: OutputTransform,
    width: usize,
    height: usize,
) {
    let inner_height = layout.height.saturating_sub(2);
    let content_height = layout.label.content_height();
    let content_y = layout.y + 1 + inner_height.saturating_sub(content_height) / 2;
    let line_step = GRAPH_LABEL_LINE_GAP + 1;
    let line_count = layout.label.lines().len();
    let last_line_y = content_y + line_count.saturating_sub(1) * line_step;
    let transformed_content_y = match transform {
        OutputTransform::VerticalMirror => height.saturating_sub(1).saturating_sub(last_line_y),
        OutputTransform::Identity | OutputTransform::HorizontalMirror => content_y,
    };

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = display_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let text_y = content_y + line_index * line_step;
        clear_text_span(
            canvas,
            transform.text_x(text_x, line, width),
            transform.text_y(text_y, height),
            line,
        );
    }

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = display_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let transformed_x = transform.text_x(text_x, line, width);
        let transformed_y = transformed_content_y + line_index * line_step;
        write_node_text(canvas, transformed_x, transformed_y, line, layout.style);
    }
}

fn clear_text_span(canvas: &mut Canvas, x: usize, y: usize, text: &str) {
    for offset in 0..display_width(text) {
        canvas.set(x + offset, y, ' ');
    }
}

fn centered_label_offset(width: usize, text_width: usize) -> usize {
    let center = width.saturating_sub(1) / 2 + 1;
    center.saturating_sub(text_width.div_ceil(2))
}
