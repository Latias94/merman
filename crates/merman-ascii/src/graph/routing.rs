use super::charset::GraphCharset;
use super::layout::NodeLayout;
use super::model::{
    AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphEdgeStroke, GraphNodeShape,
};
use crate::canvas::Canvas;
use crate::text::display_width;

pub(super) fn edge_canvas_extent(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;
    if direction != GraphDirection::LeftRight {
        return (width, height);
    }

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y(layout) + 1);
    }
    for edge in edges.iter().filter(|edge| edge.from != edge.to) {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        if from.center_y() == to.center_y() && from.x > to.x {
            width = width.max(from.center_x() + 1);
            height = height.max(left_right_back_edge_bottom_y(from) + 1);
        }
    }

    (width, height)
}

pub(super) fn draw_edge(
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
        GraphDirection::LeftRight => draw_left_right_edge(canvas, layouts, from, to, edge, charset),
        GraphDirection::TopDown => draw_top_down_edge(canvas, from, to, edge, charset),
    }

    draw_edge_label(canvas, from, to, edge, direction);
}

fn draw_left_right_edge(
    canvas: &mut Canvas,
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if from.id == to.id {
        draw_left_right_self_edge(canvas, layouts, from, edge, charset);
        return;
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        draw_left_right_back_edge(canvas, from, to, edge, charset);
        return;
    }

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

fn draw_left_right_back_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if start_x <= end_x {
        return;
    }

    let bottom_y = left_right_back_edge_bottom_y(from);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);

    canvas.set(start_x, from.bottom(), charset.down_connector);
    for y in (from.bottom() + 1)..bottom_y {
        canvas.set(start_x, y, vertical);
    }
    canvas.set(start_x, bottom_y, charset.bottom_right);

    for x in (end_x + 1)..start_x {
        canvas.set(x, bottom_y, horizontal);
    }
    canvas.set(end_x, bottom_y, charset.corner_down_right);

    let arrow_y = bottom_y - 1;
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(end_x, arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(end_x, arrow_y, charset.arrow_up),
    }
}

fn left_right_back_edge_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn draw_left_right_self_edge(
    canvas: &mut Canvas,
    layouts: &[NodeLayout],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    let y = from.center_y();
    let loop_x = self_loop_right_x(layouts, from);
    let bottom_y = self_loop_bottom_y(from);
    if loop_x <= from.right() || bottom_y <= y + 1 {
        return;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    if from.shape != GraphNodeShape::Diamond {
        canvas.set(from.right(), y, charset.right_connector);
    }
    for x in (from.right() + 1)..loop_x {
        canvas.set(x, y, horizontal);
    }
    let top_corner = if self_loop_has_right_neighbor(layouts, from) {
        charset.down_junction
    } else {
        charset.top_right
    };
    canvas.set(loop_x, y, top_corner);

    for line_y in (y + 1)..bottom_y {
        canvas.set(loop_x, line_y, vertical);
    }
    canvas.set(loop_x, bottom_y, charset.bottom_right);

    for x in (from.center_x() + 1)..loop_x {
        canvas.set(x, bottom_y, horizontal);
    }
    canvas.set(from.center_x(), bottom_y, charset.corner_down_right);

    let arrow_y = bottom_y - 1;
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(from.center_x(), arrow_y, vertical),
        GraphEdgeArrow::Point => canvas.set(from.center_x(), arrow_y, charset.arrow_up),
    }
}

fn self_loop_has_right_neighbor(layouts: &[NodeLayout], from: &NodeLayout) -> bool {
    layouts.iter().any(|layout| {
        layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
    })
}

fn self_loop_right_x(layouts: &[NodeLayout], from: &NodeLayout) -> usize {
    layouts
        .iter()
        .filter(|layout| {
            layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
        })
        .map(|layout| layout.x)
        .min()
        .map(|right_x| (from.right() + right_x) / 2)
        .unwrap_or_else(|| from.right() + 2)
}

fn self_loop_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
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
