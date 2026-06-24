use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::super::cell::edge_line_char;
use super::{
    PlannedRouteCell, PlannedRouteCellKind, PlannedRouteSegment, RoutePlan, edge_arrow_cell,
    edge_line_cell, planned_label, route_cell,
};

pub(super) fn plan_left_right_direct_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if from.center_y() != to.center_y() || to.x <= from.right() + 1 {
        return None;
    }
    if !left_right_direct_route_is_clear(layouts, from, to) {
        return None;
    }

    let y = from.center_y();
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = Vec::new();
    if charset.unicode {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x: from.right(), y },
            ch: charset.right_connector,
            kind: PlannedRouteCellKind::EdgeLine,
            segment: PlannedRouteSegment::Direct,
        });
    }
    for x in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
            segment: PlannedRouteSegment::Direct,
        });
    }
    cells.push(PlannedRouteCell {
        coord: CanvasCoord { x: end, y },
        ch: match edge.arrow {
            GraphEdgeArrow::Open => line,
            GraphEdgeArrow::Point => charset.arrow_right,
        },
        kind: match edge.arrow {
            GraphEdgeArrow::Open => PlannedRouteCellKind::RouteCell,
            GraphEdgeArrow::Point => PlannedRouteCellKind::EdgeArrow,
        },
        segment: PlannedRouteSegment::Direct,
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x: start, y },
        CanvasCoord { x: end, y },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

pub(super) fn plan_left_right_down_route(
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
    let mut cells = vec![edge_line_cell(x, from.bottom(), charset.down_connector)];
    for y in start..end {
        cells.push(route_cell(x, y, line));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(x, end, line),
        GraphEdgeArrow::Point => edge_arrow_cell(x, end, charset.arrow_down),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

pub(super) fn plan_left_right_down_then_right_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if !has_left_right_crossing_pair(layouts, edges, from, to) {
        return plan_left_right_basic_down_then_right_route(from, to, edge, charset);
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(from, to);
    if lane_y <= from.bottom() || to.x <= lane_x + 1 {
        return None;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = vec![edge_line_cell(
        source_x,
        from.bottom(),
        charset.down_connector,
    )];
    for y in (from.bottom() + 1)..lane_y {
        cells.push(route_cell(source_x, y, vertical));
    }
    cells.push(route_cell(source_x, lane_y, charset.corner_down_right));

    for line_x in (source_x + 1)..lane_x {
        cells.push(route_cell(line_x, lane_y, horizontal));
    }
    cells.push(route_cell(lane_x, lane_y, charset.top_right));

    for y in (lane_y + 1)..to.center_y() {
        cells.push(route_cell(lane_x, y, vertical));
    }
    let end = to.x - 1;
    cells.push(route_cell(lane_x, to.center_y(), charset.corner_down_right));
    for line_x in (lane_x + 1)..end {
        cells.push(route_cell(line_x, to.center_y(), horizontal));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(end, to.center_y(), horizontal),
        GraphEdgeArrow::Point => edge_arrow_cell(end, to.center_y(), charset.arrow_right),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

pub(super) fn plan_left_right_right_then_up_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if !has_left_right_reverse_crossing_pair(layouts, edges, from, to) {
        return plan_left_right_basic_right_then_up_route(from, to, edge, charset);
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(to, from);
    if lane_x <= source_x || from.y <= lane_y || lane_y <= to.bottom() {
        return None;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = vec![edge_line_cell(source_x, from.y, charset.up_connector)];
    for y in (lane_y + 1)..from.y {
        cells.push(route_cell(source_x, y, vertical));
    }
    cells.push(route_cell(source_x, lane_y, charset.top_left));

    for x in (source_x + 1)..lane_x {
        cells.push(route_cell(x, lane_y, horizontal));
    }
    cells.push(route_cell(lane_x, lane_y, charset.corner_right_up));

    for y in (to.center_y() + 1)..lane_y {
        cells.push(route_cell(lane_x, y, vertical));
    }
    cells.push(route_cell(lane_x, to.center_y(), charset.top_left));

    let end = to.x - 1;
    for x in (lane_x + 1)..end {
        cells.push(route_cell(x, to.center_y(), horizontal));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(end, to.center_y(), horizontal),
        GraphEdgeArrow::Point => edge_arrow_cell(end, to.center_y(), charset.arrow_right),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

pub(super) fn plan_left_right_bottom_lane_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if start_x == end_x {
        return None;
    }

    let bottom_y = left_right_back_edge_bottom_y(from);
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

    Some(RoutePlan { cells, labels })
}

pub(super) fn plan_left_right_reverse_over_self_loop_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let lane_x = self_loop_right_x(layouts, to);
    if lane_x <= to.right() || from.x <= lane_x {
        return None;
    }

    let y = to.center_y();
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = vec![
        edge_line_cell(from.x, y, charset.left_connector),
        route_cell(lane_x, y, charset.down_junction),
    ];
    for x in (lane_x + 1)..from.x {
        cells.push(route_cell(x, y, horizontal));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(to.right() + 1, y, horizontal),
        GraphEdgeArrow::Point => edge_arrow_cell(to.right() + 1, y, charset.arrow_left),
    });
    for x in (to.right() + 2)..lane_x {
        cells.push(route_cell(x, y, horizontal));
    }
    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: to.right() + 1,
            y,
        },
        CanvasCoord {
            x: from.x.saturating_sub(1),
            y,
        },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

pub(super) fn plan_left_right_self_loop_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let y = from.center_y();
    let loop_x = self_loop_right_x(layouts, from);
    let bottom_y = self_loop_bottom_y_for_edges(layouts, edges, from);
    if loop_x <= from.right() || bottom_y <= y + 1 {
        return None;
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = Vec::new();
    if !from.shape.is_diamond_like() {
        cells.push(edge_line_cell(from.right(), y, charset.right_connector));
    }
    for x in (from.right() + 1)..loop_x {
        cells.push(route_cell(x, y, horizontal));
    }
    let top_corner = if self_loop_has_right_neighbor(layouts, from) {
        charset.down_junction
    } else {
        charset.top_right
    };
    cells.push(route_cell(loop_x, y, top_corner));

    for line_y in (y + 1)..bottom_y {
        cells.push(route_cell(loop_x, line_y, vertical));
    }
    cells.push(route_cell(loop_x, bottom_y, charset.bottom_right));

    for x in (from.center_x() + 1)..loop_x {
        cells.push(route_cell(x, bottom_y, horizontal));
    }
    cells.push(route_cell(
        from.center_x(),
        bottom_y,
        charset.corner_down_right,
    ));

    let arrow_y = from.bottom() + 1;
    for line_y in (arrow_y + 1)..bottom_y {
        cells.push(route_cell(from.center_x(), line_y, vertical));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => edge_line_cell(from.center_x(), arrow_y, vertical),
        GraphEdgeArrow::Point => edge_arrow_cell(from.center_x(), arrow_y, charset.arrow_up),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

fn plan_left_right_basic_down_then_right_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let x = from.center_x();
    let corner_y = to.center_y();
    if corner_y <= from.bottom() || to.x <= x + 1 {
        return None;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = vec![edge_line_cell(x, from.bottom(), charset.down_connector)];
    for y in (from.bottom() + 1)..corner_y {
        cells.push(route_cell(x, y, vertical));
    }
    cells.push(route_cell(x, corner_y, charset.corner_down_right));

    let end = to.x - 1;
    for line_x in (x + 1)..end {
        cells.push(route_cell(line_x, corner_y, horizontal));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(end, corner_y, horizontal),
        GraphEdgeArrow::Point => edge_arrow_cell(end, corner_y, charset.arrow_right),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

fn plan_left_right_basic_right_then_up_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let y = from.center_y();
    let corner_x = to.center_x();
    if corner_x <= from.right() || y <= to.bottom() + 1 {
        return None;
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = vec![edge_line_cell(from.right(), y, charset.right_connector)];
    for x in (from.right() + 1)..corner_x {
        cells.push(route_cell(x, y, horizontal));
    }
    cells.push(route_cell(corner_x, y, charset.corner_right_up));

    let arrow_y = to.bottom() + 1;
    for line_y in (arrow_y + 1)..y {
        cells.push(route_cell(corner_x, line_y, vertical));
    }
    cells.push(match edge.arrow {
        GraphEdgeArrow::Open => route_cell(corner_x, arrow_y, vertical),
        GraphEdgeArrow::Point => edge_arrow_cell(corner_x, arrow_y, charset.arrow_up),
    });

    Some(RoutePlan {
        cells,
        labels: Vec::new(),
    })
}

fn left_right_direct_route_is_clear(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
) -> bool {
    let y = from.center_y();
    let start = from.right() + 1;
    let end = to.x - 1;
    layouts
        .iter()
        .filter(|layout| layout.id != from.id && layout.id != to.id)
        .all(|layout| {
            y < layout.y || y > layout.bottom() || end < layout.x || start > layout.right()
        })
}

fn has_left_right_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    upper_source: &NodeLayout,
    lower_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == upper_source.x
            && other_target.x == lower_target.x
            && other_source.center_y() > upper_source.center_y()
            && other_target.center_y() < lower_target.center_y()
    })
}

fn has_left_right_reverse_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    lower_source: &NodeLayout,
    upper_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == lower_source.x
            && other_target.x == upper_target.x
            && other_source.center_y() < lower_source.center_y()
            && other_target.center_y() > upper_target.center_y()
    })
}

fn lane_x_between(from: &NodeLayout, to: &NodeLayout) -> usize {
    if from.x < to.x {
        (from.right() + to.x) / 2
    } else {
        (to.right() + from.x) / 2
    }
}

fn lane_y_between(upper: &NodeLayout, lower: &NodeLayout) -> usize {
    (upper.bottom() + lower.y) / 2
}

pub(super) fn left_right_back_edge_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

pub(super) fn self_loop_right_x(layouts: &[NodeLayout], from: &NodeLayout) -> usize {
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

pub(super) fn self_loop_bottom_y_for_edges(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
) -> usize {
    if has_same_row_reverse_edge_into(layouts, edges, from) {
        from.bottom() + 3
    } else {
        self_loop_bottom_y(from)
    }
}

fn self_loop_has_right_neighbor(layouts: &[NodeLayout], from: &NodeLayout) -> bool {
    layouts.iter().any(|layout| {
        layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
    })
}

fn self_loop_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn has_same_row_reverse_edge_into(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        if edge.to != target.id || edge.from == target.id {
            return false;
        }
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        from.center_y() == target.center_y() && from.x > target.x
    })
}
