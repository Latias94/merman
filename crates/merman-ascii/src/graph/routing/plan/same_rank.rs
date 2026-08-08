use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::super::cell::edge_line_char;
use super::{RoutePlan, edge_arrow_cell, edge_line_cell, planned_label, route_cell};

pub(super) fn plan_same_rank_direct_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if from.center_y() != to.center_y() {
        return None;
    }

    let (start, end, points_right) = if to.x > from.right() + 1 {
        (from.right() + 1, to.x - 1, true)
    } else if from.x > to.right() + 1 {
        (to.right() + 1, from.x - 1, false)
    } else {
        return None;
    };
    if !direct_route_is_clear(layouts, from, to, start, end) {
        return None;
    }

    let y = from.center_y();
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = Vec::new();
    if points_right {
        if charset.unicode {
            cells.push(edge_line_cell(from.right(), y, charset.right_connector));
        }
        for x in start..end {
            cells.push(route_cell(x, y, line));
        }
        cells.push(match edge.arrow {
            GraphEdgeArrow::Open => route_cell(end, y, line),
            GraphEdgeArrow::Point => edge_arrow_cell(end, y, charset.arrow_right),
        });
    } else {
        if charset.unicode {
            cells.push(edge_line_cell(from.x, y, charset.left_connector));
        }
        cells.push(match edge.arrow {
            GraphEdgeArrow::Open => route_cell(start, y, line),
            GraphEdgeArrow::Point => edge_arrow_cell(start, y, charset.arrow_left),
        });
        for x in (start + 1)..=end {
            cells.push(route_cell(x, y, line));
        }
    }

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x: start, y },
        CanvasCoord { x: end, y },
    )
    .into_iter()
    .collect();

    Some(RoutePlan::new(cells, labels))
}

pub(super) fn plan_same_rank_bottom_lane_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if from.center_y() != to.center_y() || start_x == end_x {
        return None;
    }

    let bottom_y = from.bottom() + 2;
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let min_x = start_x.min(end_x);
    let max_x = start_x.max(end_x);
    let mut cells = Vec::new();

    cells.push(edge_line_cell(
        start_x,
        from.bottom(),
        charset.down_connector,
    ));
    for y in (from.bottom() + 1)..bottom_y {
        cells.push(route_cell(start_x, y, vertical));
    }
    let start_corner = if start_x < end_x {
        charset.corner_down_right
    } else {
        charset.bottom_right
    };
    cells.push(route_cell(start_x, bottom_y, start_corner));

    for x in (min_x + 1)..max_x {
        cells.push(route_cell(x, bottom_y, horizontal));
    }
    let end_corner = if start_x < end_x {
        charset.bottom_right
    } else {
        charset.corner_down_right
    };
    cells.push(route_cell(end_x, bottom_y, end_corner));

    let arrow_y = bottom_y - 1;
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => edge_line_cell(end_x, arrow_y, vertical),
        GraphEdgeArrow::Point => edge_arrow_cell(end_x, arrow_y, charset.arrow_up),
    });
    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: min_x,
            y: bottom_y,
        },
        CanvasCoord {
            x: max_x,
            y: bottom_y,
        },
    )
    .into_iter()
    .collect();

    Some(RoutePlan::with_min_canvas_extent(
        cells,
        labels,
        max_x + 3,
        bottom_y + 1,
    ))
}

fn direct_route_is_clear(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    start: usize,
    end: usize,
) -> bool {
    let y = from.center_y();
    layouts
        .iter()
        .filter(|layout| layout.id != from.id && layout.id != to.id)
        .all(|layout| {
            y < layout.y || y > layout.bottom() || end < layout.x || start > layout.right()
        })
}
