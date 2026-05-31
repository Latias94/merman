use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::super::cell::edge_line_char;
use super::{
    PlannedRouteCell, PlannedRouteCellKind, RoutePlan, edge_arrow_cell, edge_line_cell,
    planned_label, route_cell,
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
    });
    for y in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
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
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x, y: start },
        CanvasCoord { x, y: end },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

pub(super) fn plan_top_down_bent_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if to.y <= from.center_y() + 1 {
        return None;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let source_y = from.center_y();
    let target_x = to.center_x();
    let end_y = to.y - 1;
    let mut cells = Vec::new();

    if target_x > from.center_x() {
        cells.push(edge_line_cell(
            from.right(),
            source_y,
            charset.right_connector,
        ));
        for x in (from.right() + 1)..target_x {
            cells.push(route_cell(x, source_y, horizontal));
        }
    } else {
        cells.push(edge_line_cell(from.x, source_y, charset.left_connector));
        for x in (target_x + 1)..from.x {
            cells.push(route_cell(x, source_y, horizontal));
        }
    }

    cells.push(route_cell(target_x, source_y, charset.corner_down_right));
    for y in (source_y + 1)..end_y {
        cells.push(route_cell(target_x, y, vertical));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(target_x, end_y, vertical),
        GraphEdgeArrow::Point => edge_arrow_cell(target_x, end_y, charset.arrow_down),
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: target_x,
            y: source_y + 1,
        },
        CanvasCoord {
            x: target_x,
            y: end_y,
        },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
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
    let labels = planned_label(
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

    Some(RoutePlan { cells, labels })
}

pub(super) fn top_down_back_edge_lane_x(from: &NodeLayout, to: &NodeLayout) -> usize {
    from.right().max(to.right()) + 4
}
