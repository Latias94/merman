use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphNodeShape};
use super::super::cell::edge_line_char;
use super::{
    PlannedRouteCell, PlannedRouteCellKind, PlannedRouteSegment, RoutePlan, edge_arrow_cell,
    edge_line_cell, planned_label, route_cell,
};

pub(super) fn plan_top_down_direct_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if to.y <= from.bottom() + 1 {
        return None;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = Vec::new();
    cells.push(PlannedRouteCell {
        coord: CanvasCoord {
            x,
            y: from.bottom(),
        },
        ch: charset.down_connector,
        kind: PlannedRouteCellKind::EdgeLine,
        segment: PlannedRouteSegment::Direct,
    });
    for y in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
            segment: PlannedRouteSegment::Direct,
        });
    }
    cells.push(PlannedRouteCell {
        coord: CanvasCoord { x, y: end },
        ch: match edge.arrow {
            GraphEdgeArrow::Open => line,
            GraphEdgeArrow::Point => charset.arrow_down,
        },
        kind: match edge.arrow {
            GraphEdgeArrow::Open => PlannedRouteCellKind::RouteCell,
            GraphEdgeArrow::Point => PlannedRouteCellKind::EdgeArrow,
        },
        segment: PlannedRouteSegment::Direct,
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x, y: start },
        CanvasCoord { x, y: end },
    )
    .into_iter()
    .collect();

    Some(RoutePlan::new(cells, labels))
}

pub(super) fn plan_top_down_bent_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if uses_drop_then_turn_bent_route(from.shape) || uses_drop_then_turn_bent_route(to.shape) {
        return plan_top_down_drop_then_turn_route(from, to, edge, charset);
    }

    plan_top_down_side_bend_route(from, to, edge, charset)
}

fn uses_drop_then_turn_bent_route(shape: GraphNodeShape) -> bool {
    matches!(
        shape,
        GraphNodeShape::StateStart
            | GraphNodeShape::StateEnd
            | GraphNodeShape::ForkJoinHorizontal
            | GraphNodeShape::ForkJoinVertical
            | GraphNodeShape::Choice
    )
}

fn plan_top_down_side_bend_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let turn_y = from.center_y();
    let end_y = to.y.checked_sub(1)?;
    if end_y <= turn_y {
        return None;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let target_x = to.center_x();
    let mut cells = Vec::new();
    let (label_start_x, label_end_x);

    if target_x > from.center_x() {
        if target_x <= from.right() {
            return None;
        }

        label_start_x = from.right();
        label_end_x = target_x;
        cells.push(edge_line_cell(
            from.right(),
            turn_y,
            charset.right_connector,
        ));
        for x in (from.right() + 1)..target_x {
            cells.push(route_cell(x, turn_y, horizontal));
        }
        cells.push(route_cell(target_x, turn_y, charset.top_right));
    } else {
        if from.x <= target_x {
            return None;
        }

        label_start_x = target_x;
        label_end_x = from.x;
        cells.push(edge_line_cell(from.x, turn_y, charset.left_connector));
        for x in ((target_x + 1)..from.x).rev() {
            cells.push(route_cell(x, turn_y, horizontal));
        }
        cells.push(route_cell(target_x, turn_y, charset.top_left));
    }

    for y in (turn_y + 1)..end_y {
        cells.push(route_cell(target_x, y, vertical));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(target_x, end_y, vertical),
        GraphEdgeArrow::Point => edge_arrow_cell(target_x, end_y, charset.arrow_down),
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: label_start_x,
            y: turn_y,
        },
        CanvasCoord {
            x: label_end_x,
            y: turn_y,
        },
    )
    .into_iter()
    .collect();

    Some(RoutePlan::new(cells, labels))
}

fn plan_top_down_drop_then_turn_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let end_y = to.y.checked_sub(1)?;
    if end_y <= from.bottom() {
        return None;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let source_x = from.center_x();
    let target_x = to.center_x();
    let mut cells = Vec::new();

    cells.push(edge_line_cell(
        source_x,
        from.bottom(),
        charset.down_connector,
    ));
    for y in (from.bottom() + 1)..end_y {
        cells.push(route_cell(source_x, y, vertical));
    }

    if target_x > source_x {
        cells.push(route_cell(source_x, end_y, charset.corner_down_right));
        for x in (source_x + 1)..target_x {
            cells.push(route_cell(x, end_y, horizontal));
        }
    } else {
        cells.push(route_cell(source_x, end_y, charset.corner_right_up));
        for x in ((target_x + 1)..source_x).rev() {
            cells.push(route_cell(x, end_y, horizontal));
        }
    }

    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(target_x, end_y, horizontal),
        GraphEdgeArrow::Point => edge_arrow_cell(target_x, end_y, charset.arrow_down),
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: source_x.min(target_x),
            y: end_y,
        },
        CanvasCoord {
            x: source_x.max(target_x),
            y: end_y,
        },
    )
    .into_iter()
    .collect();

    Some(RoutePlan::new(cells, labels))
}

pub(super) fn plan_top_down_side_entry_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let y = from.center_y();
    if y < to.y || y > to.bottom() {
        return None;
    }

    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = Vec::new();

    if from.center_x() < to.center_x() {
        if to.x <= from.right() + 1 {
            return None;
        }
        if charset.unicode {
            cells.push(edge_line_cell(from.right(), y, charset.right_connector));
        }
        let start = from.right() + 1;
        let end = to.x - 1;
        for x in start..end {
            cells.push(route_cell(x, y, line));
        }
        cells.push(match edge.arrow {
            GraphEdgeArrow::Open => route_cell(end, y, line),
            GraphEdgeArrow::Point => edge_arrow_cell(end, y, charset.arrow_right),
        });

        let labels = planned_label(
            edge.label.as_deref(),
            CanvasCoord { x: start, y },
            CanvasCoord { x: end, y },
        )
        .into_iter()
        .collect();

        return Some(RoutePlan::new(cells, labels));
    }

    if from.x <= to.right() + 1 {
        return None;
    }
    if charset.unicode {
        cells.push(edge_line_cell(from.x, y, charset.left_connector));
    }
    let start = to.right() + 1;
    let end = from.x - 1;
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(start, y, line),
        GraphEdgeArrow::Point => edge_arrow_cell(start, y, charset.arrow_left),
    });
    for x in (start + 1)..from.x {
        cells.push(route_cell(x, y, line));
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

pub(super) fn plan_top_down_back_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let lane_x = top_down_back_edge_lane_x(from, to);
    let source_y = from.center_y();
    let target_y = to.center_y();
    if source_y <= target_y || lane_x <= from.right() {
        return None;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = vec![edge_line_cell(
        from.right(),
        source_y,
        charset.right_connector,
    )];

    for x in (from.right() + 1)..lane_x {
        cells.push(route_cell(x, source_y, horizontal));
    }
    cells.push(route_cell(lane_x, source_y, charset.corner_right_up));

    for y in (target_y + 1)..source_y {
        cells.push(route_cell(lane_x, y, vertical));
    }
    cells.push(route_cell(lane_x, target_y, charset.top_right));

    match edge.arrow {
        GraphEdgeArrow::Open => {
            for x in (to.right() + 1)..lane_x {
                cells.push(route_cell(x, target_y, horizontal));
            }
        }
        GraphEdgeArrow::Point => {
            cells.push(edge_arrow_cell(
                to.right() + 1,
                target_y,
                charset.arrow_left,
            ));
            for x in (to.right() + 2)..lane_x {
                cells.push(route_cell(x, target_y, horizontal));
            }
        }
    }
    let labels: Vec<_> = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: lane_x,
            y: target_y,
        },
        CanvasCoord {
            x: lane_x,
            y: source_y,
        },
    )
    .into_iter()
    .collect();

    let min_width = labels.iter().fold(lane_x + 3, |width, label| {
        width.max(label.placement.canvas_extent().0 + 1)
    });

    Some(RoutePlan::with_min_canvas_extent(
        cells, labels, min_width, 0,
    ))
}

pub(super) fn top_down_back_edge_lane_x(from: &NodeLayout, to: &NodeLayout) -> usize {
    from.right().max(to.right()) + 4
}
