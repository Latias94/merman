use super::charset::GraphCharset;
use super::label::GRAPH_LABEL_LINE_GAP;
use super::layout::{GroupLayout, NodeLayout, layout_graph_with_resources};
use super::model::{AsciiGraph, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
use super::routing;
use super::surface::{GraphSurface, OutputTransform, TransformedSurface};
use crate::canvas::Canvas as RawCanvas;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use std::collections::HashSet;

type Canvas<'surface> = dyn GraphSurface + 'surface;

#[cfg(test)]
pub(crate) fn render_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Result<String> {
    let mut resources = ResourceContext::new(options.resources);
    render_graph_with_resources(graph, options, &mut resources)
}

pub(crate) fn render_graph_with_resources(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    options.validate()?;
    if graph.nodes.is_empty() {
        return Ok(String::new());
    }

    debug_assert_eq!(resources.policy(), options.resources);
    let charset = GraphCharset::for_options(options);
    let graph_layout = layout_graph_with_resources(graph, options, resources)?;
    graph_canvas_extent(&graph_layout.nodes, &graph_layout.groups, 0, 0, resources)?;
    let route_scene = routing::prepare_route_scene_with_resources(
        graph,
        &graph_layout,
        &graph.edges,
        &charset,
        resources,
    )?;
    let (edge_width, edge_height) = route_scene.canvas_extent();
    let extent = graph_canvas_extent(
        &graph_layout.nodes,
        &graph_layout.groups,
        edge_width,
        edge_height,
        resources,
    )?;
    let width = extent.width();
    let height = extent.height();
    let output_transform = OutputTransform::for_direction(graph.direction);
    if !output_transform.is_identity() {
        resources.charge_layout_work(extent.cells())?;
    }

    let mut canvas =
        RawCanvas::try_with_resources(width, height, options.terminal_width_profile, resources)?;
    let mut route_cells = HashSet::new();
    route_cells
        .try_reserve(route_scene.planned_cell_count())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    {
        let mut surface = TransformedSurface::new(
            &mut canvas,
            output_transform,
            width,
            height,
            options.terminal_width_profile,
        );
        for group in &graph_layout.groups {
            draw_group(&mut surface, group, &charset)?;
        }
        for layout in &graph_layout.nodes {
            draw_node(&mut surface, layout, &charset, options)?;
        }
        let mut route_drawing = routing::RouteDrawing::new(&mut surface, &mut route_cells);
        route_scene.paint_routes(&mut route_drawing)?;
    }

    redraw_transformed_node_labels(
        &mut canvas,
        &graph_layout.nodes,
        output_transform,
        width,
        height,
    )?;
    route_scene.draw_labels(
        &mut canvas,
        output_transform.route_label_transform(width, height),
    )?;
    if output_transform.is_identity() {
        for group in &graph_layout.groups {
            draw_group_title(&mut canvas, group)?;
        }
    } else {
        for group in &graph_layout.groups {
            draw_transformed_group_title(&mut canvas, group, output_transform, width, height)?;
        }
    }

    canvas.finish_with_options(options)
}

fn graph_canvas_extent(
    nodes: &[NodeLayout],
    groups: &[GroupLayout],
    edge_width: usize,
    edge_height: usize,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut width = edge_width;
    let mut height = edge_height;
    for layout in nodes {
        width = width.max(resources.checked_grid_add(layout.x, layout.width)?);
        height = height.max(resources.checked_grid_add(layout.y, layout.height)?);
    }
    for layout in groups {
        width = width.max(resources.checked_grid_add(layout.x, layout.width)?);
        height = height.max(resources.checked_grid_add(layout.y, layout.height)?);
    }
    resources.grid_extent(width, height)
}

impl OutputTransform {
    fn route_label_transform(self, width: usize, height: usize) -> routing::RouteLabelTransform {
        match self {
            Self::Identity => routing::RouteLabelTransform::Identity,
            Self::HorizontalMirror => routing::RouteLabelTransform::HorizontalMirror { width },
            Self::VerticalMirror => routing::RouteLabelTransform::VerticalMirror { height },
        }
    }
}

fn draw_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
        GraphNodeShape::StackedDocument => {
            draw_stacked_document_node(canvas, layout, charset, options)
        }
        GraphNodeShape::LinedDocument => draw_lined_document_node(canvas, layout, charset, options),
        GraphNodeShape::TaggedDocument => {
            draw_tagged_document_node(canvas, layout, charset, options)
        }
        GraphNodeShape::StackedRect => draw_stacked_rect_node(canvas, layout, charset, options),
        GraphNodeShape::LinedRect => draw_lined_rect_node(canvas, layout, charset, options),
        GraphNodeShape::TaggedRect => draw_tagged_rect_node(canvas, layout, charset, options),
        GraphNodeShape::Flag => draw_flag_node(canvas, layout, charset, options),
        GraphNodeShape::PaperTape => draw_paper_tape_node(canvas, layout, charset, options),
        GraphNodeShape::Text => Ok(()),
        GraphNodeShape::Hexagon => draw_hexagon_node(canvas, layout, charset, options),
        GraphNodeShape::Asymmetric => draw_asymmetric_node(canvas, layout, charset, options),
        GraphNodeShape::Trapezoid => draw_trapezoid_node(canvas, layout, charset, options),
        GraphNodeShape::TrapezoidAlt => draw_trapezoid_alt_node(canvas, layout, charset, options),
        GraphNodeShape::StateStart => draw_state_start_node(canvas, layout, charset, options),
        GraphNodeShape::StateEnd => draw_state_end_node(canvas, layout, charset, options),
        GraphNodeShape::ForkJoinHorizontal => draw_fork_join_node(canvas, layout, charset, false),
        GraphNodeShape::ForkJoinVertical => draw_fork_join_node(canvas, layout, charset, true),
        GraphNodeShape::Choice => draw_choice_node(canvas, layout),
    }
}

fn draw_group(canvas: &mut Canvas<'_>, group: &GroupLayout, charset: &GraphCharset) -> Result<()> {
    if group.kind == GraphGroupKind::Container {
        paint_group_background(canvas, group);
    }
    match group.kind {
        GraphGroupKind::Container => draw_group_box(canvas, group, charset),
        GraphGroupKind::Divider => draw_group_divider(canvas, group, charset),
    }
}

fn paint_node_background(canvas: &mut Canvas<'_>, layout: &NodeLayout) {
    let Some(color) = layout.style.background else {
        return;
    };
    for y in layout.y..=layout.bottom() {
        for x in layout.x..=layout.right() {
            canvas.set_background_color(x, y, color);
        }
    }
}

fn paint_group_background(canvas: &mut Canvas<'_>, group: &GroupLayout) {
    let Some(color) = group.style.background else {
        return;
    };
    for y in group.y..=group.bottom() {
        for x in group.x..=group.right() {
            canvas.set_background_color(x, y, color);
        }
    }
}

fn draw_group_box(
    canvas: &mut Canvas<'_>,
    group: &GroupLayout,
    charset: &GraphCharset,
) -> Result<()> {
    let right = group.right();
    let bottom = group.bottom();

    set_group_border(canvas, group.x, group.y, charset.top_left, group.style)?;
    set_group_border(canvas, right, group.y, charset.top_right, group.style)?;
    set_group_border(canvas, group.x, bottom, charset.bottom_left, group.style)?;
    set_group_border(canvas, right, bottom, charset.bottom_right, group.style)?;

    for x in (group.x + 1)..right {
        set_group_border(canvas, x, group.y, charset.horizontal, group.style)?;
        set_group_border(canvas, x, bottom, charset.horizontal, group.style)?;
    }

    for y in (group.y + 1)..bottom {
        set_group_border(canvas, group.x, y, charset.vertical, group.style)?;
        set_group_border(canvas, right, y, charset.vertical, group.style)?;
    }
    Ok(())
}

fn draw_group_divider(
    canvas: &mut Canvas<'_>,
    group: &GroupLayout,
    charset: &GraphCharset,
) -> Result<()> {
    let Some(span) = group.divider_span else {
        return Ok(());
    };
    for x in span.x_start..=span.x_end {
        set_group_border(canvas, x, group.y, charset.dotted_horizontal, group.style)?;
    }
    Ok(())
}

fn draw_group_title(canvas: &mut Canvas<'_>, group: &GroupLayout) -> Result<()> {
    if group.kind == GraphGroupKind::Divider || !canvas.is_identity() {
        return Ok(());
    }
    for (line_index, line) in group.title.lines().iter().enumerate() {
        let Some((title_x, title_y)) = group_title_line_position(group, line, line_index) else {
            continue;
        };
        write_group_title(canvas, title_x, title_y, line, group.style)?;
    }
    Ok(())
}

fn draw_transformed_group_title(
    canvas: &mut Canvas<'_>,
    group: &GroupLayout,
    transform: OutputTransform,
    width: usize,
    height: usize,
) -> Result<()> {
    if group.kind == GraphGroupKind::Divider {
        return Ok(());
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
        )?;
    }
    Ok(())
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

fn set_group_border(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    ch: char,
    style: GraphGroupStyle,
) -> Result<()> {
    if let Some(color) = style.border {
        canvas.set_color(x, y, ch, color)
    } else {
        canvas.set_role(x, y, ch, AsciiColorRole::GroupBorder)
    }
}

fn write_group_title(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    text: &str,
    style: GraphGroupStyle,
) -> Result<()> {
    if let Some(color) = style.title {
        canvas.write_text_color(x, y, text, color)
    } else {
        canvas.write_text_role(x, y, text, AsciiColorRole::MutedText)
    }
}

fn set_node_border(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    ch: char,
    style: GraphNodeStyle,
) -> Result<()> {
    if let Some(color) = style.border {
        canvas.set_color(x, y, ch, color)
    } else {
        canvas.set_role(x, y, ch, AsciiColorRole::NodeBorder)
    }
}

fn write_node_text(
    canvas: &mut Canvas<'_>,
    x: usize,
    y: usize,
    text: &str,
    style: GraphNodeStyle,
) -> Result<()> {
    if let Some(color) = style.text {
        canvas.write_text_color(x, y, text, color)
    } else {
        canvas.write_text_role(x, y, text, AsciiColorRole::Text)
    }
}

fn draw_rect_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    let right = layout.right();
    let bottom = layout.bottom();

    set_node_border(canvas, layout.x, layout.y, charset.top_left, layout.style)?;
    set_node_border(canvas, right, layout.y, charset.top_right, layout.style)?;
    set_node_border(canvas, layout.x, bottom, charset.bottom_left, layout.style)?;
    set_node_border(canvas, right, bottom, charset.bottom_right, layout.style)?;

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style)?;
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style)?;
    }

    for y in (layout.y + 1)..bottom {
        set_node_border(canvas, layout.x, y, charset.vertical, layout.style)?;
        set_node_border(canvas, right, y, charset.vertical, layout.style)?;
    }

    write_centered_label(canvas, layout, options)
}

fn draw_rounded_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_circle_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_stadium_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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

    draw_node_with_corners(canvas, layout, charset, options, corners)
}

fn draw_double_circle_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

#[derive(Debug, Clone, Copy)]
struct RoundedCorners {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
}

fn draw_node_with_corners(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    corners: RoundedCorners,
) -> Result<()> {
    let right = layout.right();
    let bottom = layout.bottom();

    set_node_border(canvas, layout.x, layout.y, corners.top_left, layout.style)?;
    set_node_border(canvas, right, layout.y, corners.top_right, layout.style)?;
    set_node_border(canvas, layout.x, bottom, corners.bottom_left, layout.style)?;
    set_node_border(canvas, right, bottom, corners.bottom_right, layout.style)?;

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style)?;
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style)?;
    }

    for y in (layout.y + 1)..bottom {
        set_node_border(canvas, layout.x, y, charset.vertical, layout.style)?;
        set_node_border(canvas, right, y, charset.vertical, layout.style)?;
    }

    write_centered_label(canvas, layout, options)
}

fn draw_diamond_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    let right = layout.right();
    let bottom = layout.bottom();
    let center_y = layout.center_y();

    set_node_border(
        canvas,
        layout.x,
        layout.y,
        charset.rounded_top_left,
        layout.style,
    )?;
    set_node_border(
        canvas,
        right,
        layout.y,
        charset.rounded_top_right,
        layout.style,
    )?;
    set_node_border(
        canvas,
        layout.x,
        layout.y + 1,
        charset.rounded_top_left,
        layout.style,
    )?;
    set_node_border(
        canvas,
        right,
        layout.y + 1,
        charset.rounded_top_right,
        layout.style,
    )?;
    set_node_border(canvas, layout.x, center_y, '<', layout.style)?;
    set_node_border(canvas, right, center_y, '>', layout.style)?;
    set_node_border(
        canvas,
        layout.x,
        bottom - 1,
        charset.rounded_bottom_left,
        layout.style,
    )?;
    set_node_border(
        canvas,
        right,
        bottom - 1,
        charset.rounded_bottom_right,
        layout.style,
    )?;
    set_node_border(
        canvas,
        layout.x,
        bottom,
        charset.rounded_bottom_left,
        layout.style,
    )?;
    set_node_border(
        canvas,
        right,
        bottom,
        charset.rounded_bottom_right,
        layout.style,
    )?;

    for x in (layout.x + 1)..right {
        set_node_border(canvas, x, layout.y, charset.horizontal, layout.style)?;
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style)?;
    }

    write_centered_label(canvas, layout, options)
}

fn draw_subroutine_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    if layout.width > 5 {
        let left_inner = layout.x + 2;
        let right_inner = layout.right().saturating_sub(2);
        for y in (layout.y + 1)..layout.bottom() {
            set_node_border(canvas, left_inner, y, charset.vertical, layout.style)?;
            set_node_border(canvas, right_inner, y, charset.vertical, layout.style)?;
        }
        let text_y = layout.y + 1 + options.box_border_padding;
        for x in (left_inner + 1)..right_inner {
            canvas.set(x, text_y, ' ')?;
        }
    }
    write_centered_label(canvas, layout, options)
}

fn draw_cylinder_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rounded_node(canvas, layout, charset, options)?;
    if layout.height > 3 {
        for x in (layout.x + 1)..layout.right() {
            set_node_border(canvas, x, layout.y + 1, charset.horizontal, layout.style)?;
        }
    }
    let text_y = layout.y + 1 + options.box_border_padding;
    for x in (layout.x + 1)..layout.right() {
        canvas.set(x, text_y, ' ')?;
    }
    write_centered_label(canvas, layout, options)
}

fn draw_lean_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    lean_right: bool,
) -> Result<()> {
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
    )?;
    set_node_border(
        canvas,
        top_right,
        top,
        if lean_right { '\\' } else { '/' },
        layout.style,
    )?;
    set_node_border(
        canvas,
        bottom_left,
        bottom,
        if lean_right { '\\' } else { '/' },
        layout.style,
    )?;
    set_node_border(
        canvas,
        bottom_right,
        bottom,
        if lean_right { '/' } else { '\\' },
        layout.style,
    )?;

    let top_inner_start = top_left + 1;
    let top_inner_end = top_right;
    for x in top_inner_start..top_inner_end {
        set_node_border(canvas, x, top, charset.horizontal, layout.style)?;
    }

    let bottom_inner_start = bottom_left + 1;
    let bottom_inner_end = bottom_right;
    for x in bottom_inner_start..bottom_inner_end {
        set_node_border(canvas, x, bottom, charset.horizontal, layout.style)?;
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
        )?;
        set_node_border(
            canvas,
            right_x,
            y,
            if lean_right { '\\' } else { '/' },
            layout.style,
        )?;
        for x in (left_x + 1)..right_x {
            canvas.set(x, y, ' ')?;
        }
    }

    write_centered_label(canvas, layout, options)
}

fn draw_datastore_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;

    let right = layout.right();
    for y in (layout.y + 1)..layout.bottom() {
        set_node_border(canvas, layout.x, y, ' ', layout.style)?;
        set_node_border(canvas, right, y, ' ', layout.style)?;
    }
    Ok(())
}

fn draw_document_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;

    let bottom = layout.bottom();
    let fold_start = layout.right().saturating_sub(2);
    for x in layout.x..=layout.right() {
        let ch = if x >= fold_start { '/' } else { '~' };
        set_node_border(canvas, x, bottom, ch, layout.style)?;
    }
    Ok(())
}

fn draw_stacked_rect_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    draw_stacked_decorator(canvas, layout, charset)
}

fn draw_lined_rect_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    draw_lined_decorator(canvas, layout, charset)
}

fn draw_tagged_rect_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    draw_tagged_decorator(canvas, layout)
}

fn draw_stacked_document_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_document_node(canvas, layout, charset, options)?;
    draw_stacked_decorator(canvas, layout, charset)
}

fn draw_lined_document_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_document_node(canvas, layout, charset, options)?;
    draw_lined_decorator(canvas, layout, charset)
}

fn draw_tagged_document_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_document_node(canvas, layout, charset, options)?;
    draw_tagged_decorator(canvas, layout)
}

fn draw_stacked_decorator(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
) -> Result<()> {
    if layout.width < 6 || layout.height < 4 {
        return Ok(());
    }
    let inner_right = layout.right().saturating_sub(2);
    let inner_bottom = layout.bottom().saturating_sub(1);
    for y in (layout.y + 1)..inner_bottom {
        set_node_border(canvas, inner_right, y, charset.vertical, layout.style)?;
    }
    for x in (layout.x + 2)..inner_right {
        set_node_border(canvas, x, inner_bottom, charset.horizontal, layout.style)?;
    }
    Ok(())
}

fn draw_lined_decorator(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
) -> Result<()> {
    if layout.width < 7 {
        return Ok(());
    }
    let left_inner = layout.x + 2;
    let right_inner = layout.right().saturating_sub(2);
    for y in (layout.y + 1)..layout.bottom() {
        set_node_border(canvas, left_inner, y, charset.vertical, layout.style)?;
        set_node_border(canvas, right_inner, y, charset.vertical, layout.style)?;
    }
    Ok(())
}

fn draw_tagged_decorator(canvas: &mut Canvas<'_>, layout: &NodeLayout) -> Result<()> {
    let right = layout.right();
    let center_y = layout.center_y();
    if center_y > layout.y {
        set_node_border(canvas, right, center_y - 1, '\\', layout.style)?;
    }
    set_node_border(canvas, right, center_y, '>', layout.style)?;
    if center_y < layout.bottom() {
        set_node_border(canvas, right, center_y + 1, '/', layout.style)?;
    }
    Ok(())
}

fn draw_flag_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    let right = layout.right();
    set_node_border(canvas, right, layout.y, '\\', layout.style)?;
    set_node_border(canvas, right, layout.center_y(), '>', layout.style)?;
    set_node_border(canvas, right, layout.bottom(), '/', layout.style)
}

fn draw_paper_tape_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    draw_rect_node(canvas, layout, charset, options)?;
    for x in (layout.x + 1)..layout.right() {
        set_node_border(canvas, x, layout.y, '~', layout.style)?;
        set_node_border(canvas, x, layout.bottom(), '~', layout.style)?;
    }
    Ok(())
}

fn draw_hexagon_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_asymmetric_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_trapezoid_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_trapezoid_alt_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
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
    )
}

fn draw_state_start_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    let symbol = if charset.unicode { '●' } else { '*' };
    draw_state_pseudo_node(canvas, layout, charset, options, symbol)
}

fn draw_state_end_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) -> Result<()> {
    let symbol = if charset.unicode { '◎' } else { '@' };
    draw_state_pseudo_node(canvas, layout, charset, options, symbol)
}

fn draw_state_pseudo_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    symbol: char,
) -> Result<()> {
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
    )?;
    let symbol = symbol.to_string();
    write_node_text(
        canvas,
        layout.center_x(),
        layout.center_y(),
        &symbol,
        layout.style,
    )
}

fn draw_fork_join_node(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    charset: &GraphCharset,
    vertical: bool,
) -> Result<()> {
    if vertical {
        for y in layout.y..=layout.bottom() {
            set_node_border(
                canvas,
                layout.center_x(),
                y,
                charset.thick_vertical,
                layout.style,
            )?;
        }
    } else {
        for x in layout.x..=layout.right() {
            set_node_border(
                canvas,
                x,
                layout.center_y(),
                charset.thick_horizontal,
                layout.style,
            )?;
        }
    }
    Ok(())
}

fn draw_choice_node(canvas: &mut Canvas<'_>, layout: &NodeLayout) -> Result<()> {
    let center_x = layout.center_x();
    let center_y = layout.center_y();
    set_node_border(
        canvas,
        center_x.saturating_sub(1),
        layout.y,
        '/',
        layout.style,
    )?;
    set_node_border(canvas, center_x + 1, layout.y, '\\', layout.style)?;
    set_node_border(canvas, layout.x, center_y, '<', layout.style)?;
    set_node_border(canvas, layout.right(), center_y, '>', layout.style)?;
    set_node_border(
        canvas,
        center_x.saturating_sub(1),
        layout.bottom(),
        '\\',
        layout.style,
    )?;
    set_node_border(canvas, center_x + 1, layout.bottom(), '/', layout.style)
}

fn write_centered_label(
    _canvas: &mut Canvas<'_>,
    _layout: &NodeLayout,
    _options: &AsciiRenderOptions,
) -> Result<()> {
    Ok(())
}

fn redraw_transformed_node_labels(
    canvas: &mut Canvas<'_>,
    layouts: &[NodeLayout],
    transform: OutputTransform,
    width: usize,
    height: usize,
) -> Result<()> {
    for layout in layouts {
        if !node_shape_draws_centered_label(layout.shape) {
            continue;
        }
        redraw_transformed_node_label(canvas, layout, transform, width, height)?;
    }
    Ok(())
}

fn node_shape_draws_centered_label(shape: GraphNodeShape) -> bool {
    !matches!(
        shape,
        GraphNodeShape::StateStart
            | GraphNodeShape::StateEnd
            | GraphNodeShape::Choice
            | GraphNodeShape::ForkJoinHorizontal
            | GraphNodeShape::ForkJoinVertical
    )
}

fn redraw_transformed_node_label(
    canvas: &mut Canvas<'_>,
    layout: &NodeLayout,
    transform: OutputTransform,
    width: usize,
    height: usize,
) -> Result<()> {
    let content_height = layout.label.content_height();
    let content_y = if layout.shape == GraphNodeShape::Text {
        layout.y
    } else {
        let inner_height = layout.height.saturating_sub(2);
        layout.y + 1 + inner_height.saturating_sub(content_height) / 2
    };
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
        )?;
    }

    for (line_index, line) in layout.label.lines().iter().enumerate() {
        let text_width = layout.label.line_width(line);
        let text_x = layout.x + centered_label_offset(layout.width, text_width);
        let transformed_x = transform.text_x(text_x, text_width, width);
        let transformed_y = transformed_content_y + line_index * line_step;
        write_node_text(canvas, transformed_x, transformed_y, line, layout.style)?;
    }
    Ok(())
}

fn clear_text_span(canvas: &mut Canvas<'_>, x: usize, y: usize, text_width: usize) -> Result<()> {
    for offset in 0..text_width {
        canvas.set(x + offset, y, ' ')?;
    }
    Ok(())
}

fn centered_label_offset(width: usize, text_width: usize) -> usize {
    let center = width.saturating_sub(1) / 2 + 1;
    center.saturating_sub(text_width.div_ceil(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalWidthProfile;
    use crate::canvas::Canvas as RawCanvas;
    use crate::graph::label::GraphLabel;
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs};
    use crate::graph::surface::GraphSurface;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::text::display_width_with_profile;
    use merman_core::resources::ResourceProfile;

    #[test]
    fn canvas_transform_preserves_complete_grapheme_clusters() {
        let mut canvas = RawCanvas::with_width_profile(8, 1, TerminalWidthProfile::Unicode);
        {
            let mut surface = TransformedSurface::new(
                &mut canvas,
                OutputTransform::HorizontalMirror,
                8,
                1,
                TerminalWidthProfile::Unicode,
            );
            surface
                .write_text_role(
                    1,
                    0,
                    "e\u{301}\u{1f469}\u{200d}\u{1f4bb}\u{1f1fa}\u{1f1f8}",
                    AsciiColorRole::Text,
                )
                .expect("test transformed label should fit the unbounded resource policy");
        }

        let rendered = canvas.finish_trimmed();

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
        for direction in [GraphDirection::BottomTop, GraphDirection::RightLeft] {
            let mut graph = AsciiGraph::new_for_diagram("state", direction);
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
            graph.add_group_with_style(
                "group",
                "Team e\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}",
                None,
                vec!["a".to_string(), "b".to_string()],
                GraphGroupStyle::default(),
            );

            let rendered = render_graph(&graph, &AsciiRenderOptions::ascii())
                .expect("state graph should render");

            for authored in [
                "Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb}",
                "\u{1f1fa}\u{1f1f8} Done",
                "go\u{301}\u{1f1fa}\u{1f1f8}",
                "Team e\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}",
            ] {
                assert!(
                    rendered.contains(authored),
                    "missing {authored:?} for {direction:?}:\n{rendered}"
                );
            }
        }
    }

    #[test]
    fn graph_grid_limit_accepts_exact_extent_and_rejects_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "b");
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let base_options = AsciiRenderOptions::ascii().with_resource_policy(unbounded);
        let mut resources = ResourceContext::new(unbounded);
        let charset = GraphCharset::for_options(&base_options);
        let layout = layout_graph_with_resources(&graph, &base_options, &mut resources)
            .expect("test graph should lay out");
        let routes = routing::prepare_route_scene_with_resources(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &mut resources,
        )
        .expect("test graph should route");
        let (edge_width, edge_height) = routes.canvas_extent();
        let exact = graph_canvas_extent(
            &layout.nodes,
            &layout.groups,
            edge_width,
            edge_height,
            &resources,
        )
        .expect("test graph extent should be representable")
        .cells();

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact)
            .expect("exact grid limit should be valid");
        render_graph(&graph, &base_options.with_resource_policy(exact_policy))
            .expect("exact grid limit should render");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact - 1)
            .expect("max-minus-one grid limit should be valid");
        let error = render_graph(&graph, &base_options.with_resource_policy(below_policy))
            .expect_err("max-minus-one grid limit should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    #[test]
    fn mirrored_graph_accepts_exact_work_limit_and_rejects_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::BottomTop);
        graph.add_node("a", "A");
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let base_options = AsciiRenderOptions::ascii().with_resource_policy(unbounded);
        let mut measured_resources = ResourceContext::new(unbounded);
        render_graph_with_resources(&graph, &base_options, &mut measured_resources)
            .expect("the unbounded mirror graph should render");
        let exact = measured_resources.layout_work_used();

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact mirror-work limit should be valid");
        let exact_options = base_options.with_resource_policy(exact_policy);
        let mut exact_resources = ResourceContext::new(exact_policy);
        render_graph_with_resources(&graph, &exact_options, &mut exact_resources)
            .expect("exact cumulative mirror-work limit should render");
        assert_eq!(exact_resources.layout_work_used(), exact);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one mirror-work limit should be valid");
        let below_options = base_options.with_resource_policy(below_policy);
        let mut below_resources = ResourceContext::new(below_policy);
        let error = render_graph_with_resources(&graph, &below_options, &mut below_resources)
            .expect_err("max-minus-one cumulative mirror-work limit should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    #[test]
    fn graph_canvas_extent_reports_checked_geometry_overflow() {
        let node = NodeLayout {
            id: "overflow".to_string(),
            label: GraphLabel::new("overflow"),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: super::super::layout::GridCoord { x: 0, y: 0 },
            x: usize::MAX,
            y: 0,
            width: 1,
            height: 1,
        };
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));

        let error = graph_canvas_extent(&[node], &[], 0, 0, &resources)
            .expect_err("overflowing graph geometry should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }

    #[test]
    fn graph_render_reports_shape_geometry_overflow() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        let mut options = AsciiRenderOptions::ascii();
        options.box_border_padding = usize::MAX;

        let error = render_graph(&graph, &options)
            .expect_err("overflowing authored-node geometry should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }
}
