use super::charset::GraphCharset;
use super::label::GRAPH_LABEL_LINE_GAP;
use super::layout::{GroupLayout, NodeLayout, layout_graph};
use super::model::{
    AsciiGraph, GraphDirection, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle,
};
use super::routing;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use std::collections::HashSet;

pub(crate) fn render_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    if graph.nodes.is_empty() {
        return Ok(String::new());
    }

    let charset = GraphCharset::for_options(options);
    let graph_layout = layout_graph(graph, options);
    let route_scene = routing::prepare_route_scene(graph, &graph_layout, &graph.edges, &charset)?;
    let (edge_width, edge_height) = route_scene.canvas_extent();
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

    let mut canvas = Canvas::with_width_profile(width, height, options.terminal_width_profile);
    let mut route_cells = HashSet::new();
    for group in &graph_layout.groups {
        draw_group(&mut canvas, group, &charset);
    }
    for layout in &graph_layout.nodes {
        draw_node(&mut canvas, layout, &charset, options);
    }
    {
        let mut route_drawing = routing::RouteDrawing::new(&mut canvas, &mut route_cells);
        route_scene.paint_routes(&mut route_drawing);
    }

    let output_transform = OutputTransform::for_direction(graph.direction);
    if output_transform.is_identity() {
        redraw_transformed_node_labels(
            &mut canvas,
            &graph_layout.nodes,
            output_transform,
            width,
            height,
        );
        route_scene.draw_labels(
            &mut canvas,
            output_transform.route_label_transform(width, height),
        );
        for group in &graph_layout.groups {
            draw_group_title(&mut canvas, group);
        }
        return Ok(canvas.finish_with_options(options));
    }

    let mut canvas =
        output_transform.transform_canvas(canvas, width, height, options.terminal_width_profile);
    redraw_transformed_node_labels(
        &mut canvas,
        &graph_layout.nodes,
        output_transform,
        width,
        height,
    );
    route_scene.draw_labels(
        &mut canvas,
        output_transform.route_label_transform(width, height),
    );
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

    fn transform_canvas(
        self,
        source: Canvas,
        width: usize,
        height: usize,
        width_profile: TerminalWidthProfile,
    ) -> Canvas {
        let mut canvas = Canvas::with_width_profile(width, height, width_profile);
        for (source_y, line) in source.into_styled_lines_trimmed().into_iter().enumerate() {
            if line.len() == 0 {
                continue;
            }
            match self {
                Self::HorizontalMirror => {
                    let line = line.mirrored();
                    line.write_to_at(&mut canvas, width.saturating_sub(line.len()), source_y);
                }
                Self::VerticalMirror => {
                    line.write_to(
                        &mut canvas,
                        height.saturating_sub(1).saturating_sub(source_y),
                    );
                }
                Self::Identity => line.write_to(&mut canvas, source_y),
            }
        }

        for y in 0..height {
            for x in 0..width {
                let Some(ch) = canvas.get(x, y) else {
                    continue;
                };
                let mapped = self.map_char(ch);
                if mapped == ch {
                    continue;
                }
                if let Some(style) = canvas.get_style(x, y) {
                    canvas.set_style(x, y, mapped, style);
                } else {
                    canvas.set(x, y, mapped);
                }
            }
        }
        canvas
    }

    fn text_x(self, x: usize, text_width: usize, width: usize) -> usize {
        match self {
            Self::HorizontalMirror => width.saturating_sub(x).saturating_sub(text_width),
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

    fn route_label_transform(self, width: usize, height: usize) -> routing::RouteLabelTransform {
        match self {
            Self::Identity => routing::RouteLabelTransform::Identity,
            Self::HorizontalMirror => routing::RouteLabelTransform::HorizontalMirror { width },
            Self::VerticalMirror => routing::RouteLabelTransform::VerticalMirror { height },
        }
    }
}

fn mirror_horizontal_char(ch: char) -> char {
    match ch {
        '>' => '<',
        '<' => '>',
        '▷' => '◁',
        '◁' => '▷',
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
        '⌜' => '⌝',
        '⌝' => '⌜',
        '⌞' => '⌟',
        '⌟' => '⌞',
        '(' => ')',
        ')' => '(',
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
        '⌜' => '⌞',
        '⌞' => '⌜',
        '⌝' => '⌟',
        '⌟' => '⌝',
        ch => ch,
    }
}

fn draw_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    paint_node_background(canvas, layout);
    match layout.shape {
        GraphNodeShape::Rect => draw_rect_node(canvas, layout, charset, options),
        GraphNodeShape::Rounded => draw_rounded_node(canvas, layout, charset, options),
        GraphNodeShape::Circle => draw_circle_node(canvas, layout, charset, options),
        GraphNodeShape::Stadium => draw_stadium_node(canvas, layout, charset, options),
        GraphNodeShape::DoubleCircle => draw_double_circle_node(canvas, layout, charset, options),
        GraphNodeShape::Diamond => draw_diamond_node(canvas, layout, charset, options),
        GraphNodeShape::Subroutine => draw_subroutine_node(canvas, layout, charset, options),
        GraphNodeShape::Cylinder => draw_cylinder_node(canvas, layout, charset, options),
        GraphNodeShape::LeanRight => draw_lean_node(canvas, layout, charset, options, true),
        GraphNodeShape::LeanLeft => draw_lean_node(canvas, layout, charset, options, false),
        GraphNodeShape::Datastore => draw_datastore_node(canvas, layout, charset, options),
        GraphNodeShape::Document => draw_document_node(canvas, layout, charset, options),
        GraphNodeShape::Hexagon => draw_hexagon_node(canvas, layout, charset, options),
        GraphNodeShape::Asymmetric => draw_asymmetric_node(canvas, layout, charset, options),
        GraphNodeShape::Trapezoid => draw_trapezoid_node(canvas, layout, charset, options),
        GraphNodeShape::TrapezoidAlt => draw_trapezoid_alt_node(canvas, layout, charset, options),
        GraphNodeShape::StateStart => draw_state_start_node(canvas, layout, charset, options),
        GraphNodeShape::StateEnd => draw_state_end_node(canvas, layout, charset, options),
        GraphNodeShape::ForkJoinHorizontal => {
            draw_fork_join_node(canvas, layout, charset, options, false)
        }
        GraphNodeShape::ForkJoinVertical => {
            draw_fork_join_node(canvas, layout, charset, options, true)
        }
        GraphNodeShape::Choice => draw_choice_node(canvas, layout),
    }
}

fn draw_group(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    if group.kind == GraphGroupKind::Container {
        paint_group_background(canvas, group);
    }
    match group.kind {
        GraphGroupKind::Container => draw_group_box(canvas, group, charset),
        GraphGroupKind::Divider => draw_group_divider(canvas, group, charset),
    }
}

fn paint_node_background(canvas: &mut Canvas, layout: &NodeLayout) {
    let Some(color) = layout.style.background else {
        return;
    };
    for y in layout.y..=layout.bottom() {
        for x in layout.x..=layout.right() {
            canvas.set_background_color(x, y, color);
        }
    }
}

fn paint_group_background(canvas: &mut Canvas, group: &GroupLayout) {
    let Some(color) = group.style.background else {
        return;
    };
    for y in group.y..=group.bottom() {
        for x in group.x..=group.right() {
            canvas.set_background_color(x, y, color);
        }
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
            transform.text_x(title_x, group.title.line_width(line), width),
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
    let title_width = group.title.line_width(line);
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

fn draw_circle_node(
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
            top_left: if charset.unicode { '◯' } else { 'o' },
            top_right: if charset.unicode { '◯' } else { 'o' },
            bottom_left: if charset.unicode { '◯' } else { 'o' },
            bottom_right: if charset.unicode { '◯' } else { 'o' },
        },
    );
}

fn draw_stadium_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let corners = if layout.height == 3 || !charset.unicode {
        RoundedCorners {
            top_left: '(',
            top_right: ')',
            bottom_left: '(',
            bottom_right: ')',
        }
    } else {
        RoundedCorners {
            top_left: charset.rounded_top_left,
            top_right: charset.rounded_top_right,
            bottom_left: charset.rounded_bottom_left,
            bottom_right: charset.rounded_bottom_right,
        }
    };

    draw_node_with_corners(canvas, layout, charset, options, corners);
}

fn draw_double_circle_node(
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
            top_left: if charset.unicode { '◎' } else { '@' },
            top_right: if charset.unicode { '◎' } else { '@' },
            bottom_left: if charset.unicode { '◎' } else { '@' },
            bottom_right: if charset.unicode { '◎' } else { '@' },
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

fn draw_lean_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    lean_right: bool,
) {
    let right = layout.right();
    let top = layout.y;
    let bottom = layout.bottom();
    let slant = layout
        .height
        .saturating_sub(1)
        .min(layout.width.saturating_sub(2));
    let left_shift = if lean_right { 0 } else { slant };
    let right_shift = if lean_right { slant } else { 0 };
    let top_left = layout.x + left_shift;
    let top_right = right.saturating_sub(right_shift);
    let bottom_left = layout.x + right_shift;
    let bottom_right = right.saturating_sub(left_shift);

    set_node_border(
        canvas,
        top_left,
        top,
        if lean_right { '/' } else { '\\' },
        layout.style,
    );
    set_node_border(
        canvas,
        top_right,
        top,
        if lean_right { '\\' } else { '/' },
        layout.style,
    );
    set_node_border(
        canvas,
        bottom_left,
        bottom,
        if lean_right { '\\' } else { '/' },
        layout.style,
    );
    set_node_border(
        canvas,
        bottom_right,
        bottom,
        if lean_right { '/' } else { '\\' },
        layout.style,
    );

    let top_inner_start = top_left + 1;
    let top_inner_end = top_right;
    for x in top_inner_start..top_inner_end {
        set_node_border(canvas, x, top, charset.horizontal, layout.style);
    }

    let bottom_inner_start = bottom_left + 1;
    let bottom_inner_end = bottom_right;
    for x in bottom_inner_start..bottom_inner_end {
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style);
    }

    let start_y = top + 1;
    let end_y = bottom.saturating_sub(1);
    let denom = bottom.saturating_sub(top).max(1);
    for y in start_y..=end_y {
        let progress = y - top;
        let shift = progress.saturating_mul(slant) / denom;
        let left_x = if lean_right {
            top_left + shift
        } else {
            top_left.saturating_sub(shift)
        };
        let right_x = if lean_right {
            top_right + shift
        } else {
            top_right.saturating_sub(shift)
        };
        set_node_border(
            canvas,
            left_x,
            y,
            if lean_right { '/' } else { '\\' },
            layout.style,
        );
        set_node_border(
            canvas,
            right_x,
            y,
            if lean_right { '\\' } else { '/' },
            layout.style,
        );
        for x in (left_x + 1)..right_x {
            canvas.set(x, y, ' ');
        }
    }

    write_centered_label(canvas, layout, options);
}

fn draw_datastore_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rect_node(canvas, layout, charset, options);

    let right = layout.right();
    for y in (layout.y + 1)..layout.bottom() {
        set_node_border(canvas, layout.x, y, ' ', layout.style);
        set_node_border(canvas, right, y, ' ', layout.style);
    }
}

fn draw_document_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rect_node(canvas, layout, charset, options);

    let bottom = layout.bottom();
    let fold_start = layout.right().saturating_sub(2);
    for x in layout.x..=layout.right() {
        let ch = if x >= fold_start { '/' } else { '~' };
        set_node_border(canvas, x, bottom, ch, layout.style);
    }
}

fn draw_hexagon_node(
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
            top_left: if charset.unicode { '⌜' } else { '*' },
            top_right: if charset.unicode { '⌝' } else { '*' },
            bottom_left: if charset.unicode { '⌞' } else { '*' },
            bottom_right: if charset.unicode { '⌟' } else { '*' },
        },
    );
}

fn draw_asymmetric_node(
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
            top_left: if charset.unicode { '▷' } else { '>' },
            top_right: if charset.unicode { '┐' } else { '+' },
            bottom_left: if charset.unicode { '▷' } else { '>' },
            bottom_right: if charset.unicode { '┘' } else { '+' },
        },
    );
}

fn draw_trapezoid_node(
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
            top_left: '/',
            top_right: '\\',
            bottom_left: if charset.unicode { '└' } else { '+' },
            bottom_right: if charset.unicode { '┘' } else { '+' },
        },
    );
}

fn draw_trapezoid_alt_node(
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
            top_left: if charset.unicode { '┌' } else { '+' },
            top_right: if charset.unicode { '┐' } else { '+' },
            bottom_left: '\\',
            bottom_right: '/',
        },
    );
}

fn draw_state_start_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let symbol = if charset.unicode { '●' } else { '*' };
    draw_state_pseudo_node(canvas, layout, charset, options, symbol);
}

fn draw_state_end_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let symbol = if charset.unicode { '◎' } else { '@' };
    draw_state_pseudo_node(canvas, layout, charset, options, symbol);
}

fn draw_state_pseudo_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    symbol: char,
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
    options: &AsciiRenderOptions,
    _vertical: bool,
) {
    let right = layout.right();
    let bottom = layout.bottom();

    set_node_border(canvas, layout.x, layout.y, charset.top_left, layout.style);
    set_node_border(canvas, right, layout.y, charset.top_right, layout.style);
    set_node_border(canvas, layout.x, bottom, charset.bottom_left, layout.style);
    set_node_border(canvas, right, bottom, charset.bottom_right, layout.style);

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.thick_horizontal, layout.style);
        set_node_border(canvas, x, bottom, charset.thick_horizontal, layout.style);
    }

    for y in (layout.y + 1)..bottom {
        set_node_border(canvas, layout.x, y, charset.thick_vertical, layout.style);
        set_node_border(canvas, right, y, charset.thick_vertical, layout.style);
    }

    write_centered_label(canvas, layout, options);
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
        let text_width = layout.label.line_width(line);
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
        if !node_shape_draws_centered_label(layout.shape) {
            continue;
        }
        redraw_transformed_node_label(canvas, layout, transform, width, height);
    }
}

fn node_shape_draws_centered_label(shape: GraphNodeShape) -> bool {
    !matches!(
        shape,
        GraphNodeShape::StateStart | GraphNodeShape::StateEnd | GraphNodeShape::Choice
    )
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
        let text_width = layout.label.line_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let text_y = content_y + line_index * line_step;
        clear_text_span(
            canvas,
            transform.text_x(text_x, text_width, width),
            transform.text_y(text_y, height),
            text_width,
        );
    }

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = layout.label.line_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let transformed_x = transform.text_x(text_x, text_width, width);
        let transformed_y = transformed_content_y + line_index * line_step;
        write_node_text(canvas, transformed_x, transformed_y, line, layout.style);
    }
}

fn clear_text_span(canvas: &mut Canvas, x: usize, y: usize, text_width: usize) {
    for offset in 0..text_width {
        canvas.set(x + offset, y, ' ');
    }
}

fn centered_label_offset(width: usize, text_width: usize) -> usize {
    let center = width.saturating_sub(1) / 2 + 1;
    center.saturating_sub(text_width.div_ceil(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalWidthProfile;
    use crate::graph::model::GraphEdgeAttrs;
    use crate::text::display_width_with_profile;

    #[test]
    fn canvas_transform_preserves_complete_grapheme_clusters() {
        let mut source = Canvas::with_width_profile(8, 1, TerminalWidthProfile::Unicode);
        source.write_text_role(
            1,
            0,
            "e\u{301}\u{1f469}\u{200d}\u{1f4bb}\u{1f1fa}\u{1f1f8}",
            AsciiColorRole::Text,
        );

        let rendered = OutputTransform::HorizontalMirror
            .transform_canvas(source, 8, 1, TerminalWidthProfile::Unicode)
            .finish_trimmed();

        assert!(rendered.contains("e\u{301}"), "{rendered:?}");
        assert!(
            rendered.contains("\u{1f469}\u{200d}\u{1f4bb}"),
            "{rendered:?}"
        );
        assert!(rendered.contains("\u{1f1fa}\u{1f1f8}"), "{rendered:?}");
    }

    #[test]
    fn graph_canvas_rows_match_cjk_width_with_ambiguous_node_and_edge_labels() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A·B");
        graph.add_node("b", "C");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                label: Some("go·".to_string()),
                ..GraphEdgeAttrs::default()
            },
        );
        let options =
            AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk);

        let rendered = render_graph(&graph, &options).expect("CJK graph should render");
        let widths = rendered
            .lines()
            .map(|line| display_width_with_profile(line, TerminalWidthProfile::Cjk))
            .collect::<Vec<_>>();

        assert!(rendered.contains("A·B"), "{rendered}");
        assert!(rendered.contains("go·"), "{rendered}");
        assert!(rendered.contains('>'), "{rendered}");
        assert!(
            widths.iter().all(|width| *width == widths[0]),
            "row widths {widths:?} should match under CJK profile:\n{rendered}"
        );
    }

    #[test]
    fn state_graph_mirror_keeps_complex_node_and_edge_labels() {
        let mut graph = AsciiGraph::new_for_diagram("state", GraphDirection::BottomTop);
        graph.add_node("a", "Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb}");
        graph.add_node("b", "\u{1f1fa}\u{1f1f8} Done");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                label: Some("go\u{301}\u{1f1fa}\u{1f1f8}".to_string()),
                ..GraphEdgeAttrs::default()
            },
        );

        let rendered =
            render_graph(&graph, &AsciiRenderOptions::ascii()).expect("state graph should render");

        for authored in [
            "Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb}",
            "\u{1f1fa}\u{1f1f8} Done",
            "go\u{301}\u{1f1fa}\u{1f1f8}",
        ] {
            assert!(
                rendered.contains(authored),
                "missing {authored:?}:\n{rendered}"
            );
        }
    }
}
