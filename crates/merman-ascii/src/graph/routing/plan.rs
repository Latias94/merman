use super::super::charset::GraphCharset;
use super::super::layout::{CanvasCoord, NodeLayout};
use super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow, GraphNodeShape};
use super::cell::edge_line_char;

mod grid;
mod select;

pub(super) use grid::plan_left_right_grid_path_route;
pub(super) use select::{EdgeRouteRequest, plan_edge_route, route_canvas_extent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlannedRouteCell {
    pub(super) coord: CanvasCoord,
    pub(super) ch: char,
    pub(super) kind: PlannedRouteCellKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlannedRouteCellKind {
    EdgeLine,
    RouteCell,
    EdgeArrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlannedRouteLabel {
    pub(super) start: CanvasCoord,
    pub(super) end: CanvasCoord,
    pub(super) text: String,
}

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
        });
    }
    for x in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
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
    if from.shape != GraphNodeShape::Diamond {
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

pub(super) fn top_down_back_edge_lane_x(from: &NodeLayout, to: &NodeLayout) -> usize {
    from.right().max(to.right()) + 4
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

fn route_cell(x: usize, y: usize, ch: char) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::RouteCell,
    }
}

fn edge_line_cell(x: usize, y: usize, ch: char) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::EdgeLine,
    }
}

fn edge_arrow_cell(x: usize, y: usize, ch: char) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::EdgeArrow,
    }
}

fn planned_label(
    label: Option<&str>,
    start: CanvasCoord,
    end: CanvasCoord,
) -> Option<PlannedRouteLabel> {
    label
        .filter(|label| !label.is_empty())
        .map(|label| PlannedRouteLabel {
            start,
            end,
            text: label.to_string(),
        })
}

fn planned_label_on_canvas_lines(
    label: Option<&str>,
    lines: &[Vec<CanvasCoord>],
) -> Option<PlannedRouteLabel> {
    let label = label.filter(|label| !label.is_empty())?;
    let line = lines.iter().max_by_key(|line| line.len())?;
    let first = line.first().copied()?;
    let last = line.last().copied()?;
    Some(PlannedRouteLabel {
        start: first,
        end: last,
        text: label.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::{GraphLayout, GridCoord, layout_graph};
    use crate::graph::model::{
        AsciiGraph, GraphDirection, GraphEdgeStroke, GraphEdgeStyle, GraphNodeShape, GraphNodeStyle,
    };

    #[test]
    fn edge_route_selects_left_right_parallel_bottom_lane() {
        let options = AsciiRenderOptions::ascii();
        let layout = left_right_layout(&[("a", "b"), ("a", "b")], &options);
        let from = layout_node(&layout, "a");
        let to = layout_node(&layout, "b");
        let edges = vec![
            edge(Some("parallel"), GraphEdgeArrow::Point),
            edge(Some("parallel"), GraphEdgeArrow::Point),
        ];
        let charset = GraphCharset::for_options(&options);

        let selected = plan_edge_route(EdgeRouteRequest {
            graph_layout: &layout,
            edges: &edges,
            from,
            to,
            edge_index: 1,
            edge: &edges[1],
            direction: GraphDirection::LeftRight,
            charset: &charset,
        })
        .unwrap();
        let expected = plan_left_right_bottom_lane_route(from, to, &edges[1], &charset).unwrap();

        assert_eq!(selected, expected);
    }

    #[test]
    fn edge_route_selects_top_down_back_route() {
        let options = AsciiRenderOptions::ascii();
        let layout = left_right_layout(&[("a", "b")], &options);
        let from = node("a", 0, 6, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let edge = edge(Some("back"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&options);

        let selected = plan_edge_route(EdgeRouteRequest {
            graph_layout: &layout,
            edges: &[],
            from: &from,
            to: &to,
            edge_index: 0,
            edge: &edge,
            direction: GraphDirection::TopDown,
            charset: &charset,
        })
        .unwrap();
        let expected = plan_top_down_back_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(selected, expected);
    }

    #[test]
    fn route_canvas_extent_accounts_for_left_right_back_lane() {
        let from = node("a", 10, 0, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let layouts = vec![from, to];
        let edges = vec![edge_between("a", "b", None, GraphEdgeArrow::Point)];

        assert_eq!(
            route_canvas_extent(&layouts, &edges, GraphDirection::LeftRight),
            (14, 5)
        );
    }

    #[test]
    fn route_canvas_extent_accounts_for_top_down_back_label_width() {
        let from = node("a", 0, 6, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let layouts = vec![from, to];
        let edges = vec![edge_between("a", "b", Some("back"), GraphEdgeArrow::Point)];

        assert_eq!(
            route_canvas_extent(&layouts, &edges, GraphDirection::TopDown),
            (9, 0)
        );
    }

    #[test]
    fn left_right_direct_route_plans_ascii_line_arrow_and_label_without_connector() {
        let from = node("a", 0, 0, 5, 3);
        let to = node("b", 10, 0, 5, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(Some("label"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '>', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 5, y: 1 },
                end: CanvasCoord { x: 9, y: 1 },
                text: "label".to_string(),
            }]
        );
    }

    #[test]
    fn left_right_direct_route_plans_unicode_connector() {
        let from = node("a", 0, 0, 5, 3);
        let to = node("b", 10, 0, 5, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(4, 1, '├', PlannedRouteCellKind::EdgeLine),
                cell(5, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '►', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn left_right_direct_open_route_plans_line_endpoint_without_arrow() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 6, 0, 3, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(None, GraphEdgeArrow::Open);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells.last(),
            Some(&cell(5, 1, '-', PlannedRouteCellKind::RouteCell))
        );
    }

    #[test]
    fn left_right_direct_route_rejects_blocked_same_row_path() {
        let from = node("a", 0, 0, 3, 3);
        let blocker = node("blocker", 5, 0, 3, 3);
        let to = node("b", 10, 0, 3, 3);
        let layouts = vec![from.clone(), blocker, to.clone()];
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        assert!(plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).is_none());
    }

    #[test]
    fn left_right_grid_path_route_plans_unicode_connector_arrow_and_label() {
        let options = AsciiRenderOptions::unicode();
        let layout = left_right_layout(&[("a", "b")], &options);
        let from = layout_node(&layout, "a");
        let to = layout_node(&layout, "b");
        let edge = edge(Some("go"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&options);

        let plan = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(5, 2, '─', PlannedRouteCellKind::RouteCell),
                cell(6, 2, '─', PlannedRouteCellKind::RouteCell),
                cell(7, 2, '─', PlannedRouteCellKind::RouteCell),
                cell(8, 2, '─', PlannedRouteCellKind::RouteCell),
                cell(9, 2, '─', PlannedRouteCellKind::RouteCell),
                cell(4, 2, '├', PlannedRouteCellKind::EdgeLine),
                cell(9, 2, '►', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 5, y: 2 },
                end: CanvasCoord { x: 9, y: 2 },
                text: "go".to_string(),
            }]
        );
    }

    #[test]
    fn left_right_grid_path_route_plans_bent_path_cells_and_corner() {
        let options = AsciiRenderOptions::ascii();
        let layout = left_right_layout(&[("a", "b"), ("a", "c")], &options);
        let from = layout_node(&layout, "a");
        let to = layout_node(&layout, "c");
        let edge = edge_between("a", "c", Some("down"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&options);

        let plan = plan_left_right_grid_path_route(&layout, from, to, &edge, &charset).unwrap();

        assert!(
            plan.cells
                .iter()
                .any(|cell| cell.kind == PlannedRouteCellKind::RouteCell && cell.ch == '+')
        );
        assert!(
            plan.cells
                .iter()
                .any(|cell| cell.kind == PlannedRouteCellKind::RouteCell && cell.ch == '|')
        );
        assert!(
            plan.cells
                .iter()
                .any(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
        );
        assert_eq!(
            plan.labels.first().map(|label| label.text.as_str()),
            Some("down")
        );
    }

    #[test]
    fn left_right_down_route_plans_vertical_bent_line() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 0, 6, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_down_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 5, 'v', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn left_right_down_then_right_route_plans_basic_bend() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 6, 4, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_down_then_right_route(
            &[from.clone(), to.clone()],
            &[],
            &from,
            &to,
            &edge,
            &charset,
        )
        .unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 5, '+', PlannedRouteCellKind::RouteCell),
                cell(2, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 5, '>', PlannedRouteCellKind::EdgeArrow),
            ]
        );
    }

    #[test]
    fn left_right_right_then_up_route_plans_basic_bend() {
        let from = node("a", 0, 6, 3, 3);
        let to = node("b", 6, 0, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_right_then_up_route(
            &[from.clone(), to.clone()],
            &[],
            &from,
            &to,
            &edge,
            &charset,
        )
        .unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(2, 7, '|', PlannedRouteCellKind::EdgeLine),
                cell(3, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 7, '+', PlannedRouteCellKind::RouteCell),
                cell(7, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(7, 5, '|', PlannedRouteCellKind::RouteCell),
                cell(7, 6, '|', PlannedRouteCellKind::RouteCell),
                cell(7, 3, '^', PlannedRouteCellKind::EdgeArrow),
            ]
        );
    }

    #[test]
    fn left_right_down_then_right_route_plans_crossing_lane() {
        let from = node("a", 0, 0, 3, 3);
        let lower_source = node("b", 0, 8, 3, 3);
        let upper_target = node("c", 10, 0, 3, 3);
        let to = node("d", 10, 8, 3, 3);
        let layouts = vec![
            from.clone(),
            lower_source.clone(),
            upper_target.clone(),
            to.clone(),
        ];
        let edge = edge_between("a", "d", None, GraphEdgeArrow::Point);
        let crossing_edge = edge_between("b", "c", None, GraphEdgeArrow::Point);
        let edges = vec![edge.clone(), crossing_edge];
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan =
            plan_left_right_down_then_right_route(&layouts, &edges, &from, &to, &edge, &charset)
                .unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 5, '+', PlannedRouteCellKind::RouteCell),
                cell(2, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 5, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 5, '+', PlannedRouteCellKind::RouteCell),
                cell(6, 6, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 7, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 8, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 9, '+', PlannedRouteCellKind::RouteCell),
                cell(7, 9, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 9, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 9, '>', PlannedRouteCellKind::EdgeArrow),
            ]
        );
    }

    #[test]
    fn left_right_bottom_lane_route_plans_reverse_lane_and_label() {
        let from = node("a", 10, 0, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let edge = edge(Some("back"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_bottom_lane_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(11, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(11, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(11, 4, '+', PlannedRouteCellKind::RouteCell),
                cell(2, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(10, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '+', PlannedRouteCellKind::RouteCell),
                cell(1, 3, '^', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 1, y: 4 },
                end: CanvasCoord { x: 11, y: 4 },
                text: "back".to_string(),
            }]
        );
    }

    #[test]
    fn left_right_reverse_over_self_loop_route_plans_target_side_lane() {
        let from = node("a", 10, 0, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(Some("rev"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan =
            plan_left_right_reverse_over_self_loop_route(&layouts, &from, &to, &edge, &charset)
                .unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(10, 1, '|', PlannedRouteCellKind::EdgeLine),
                cell(6, 1, '+', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 1, '<', PlannedRouteCellKind::EdgeArrow),
                cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 3, y: 1 },
                end: CanvasCoord { x: 9, y: 1 },
                text: "rev".to_string(),
            }]
        );
    }

    #[test]
    fn left_right_self_loop_route_plans_loop_and_arrow() {
        let from = node("a", 0, 0, 3, 3);
        let layouts = vec![from.clone()];
        let edge = edge_between("a", "a", None, GraphEdgeArrow::Point);
        let edges = vec![edge.clone()];
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan =
            plan_left_right_self_loop_route(&layouts, &edges, &from, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
                cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 1, '+', PlannedRouteCellKind::RouteCell),
                cell(4, 2, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 4, '+', PlannedRouteCellKind::RouteCell),
                cell(2, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 4, '-', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '+', PlannedRouteCellKind::RouteCell),
                cell(1, 3, '^', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn top_down_bent_route_plans_right_bend_arrow_and_label() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 6, 5, 3, 3);
        let edge = edge(Some("bend"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
                cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '+', PlannedRouteCellKind::RouteCell),
                cell(7, 2, '|', PlannedRouteCellKind::RouteCell),
                cell(7, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(7, 4, 'v', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 7, y: 2 },
                end: CanvasCoord { x: 7, y: 4 },
                text: "bend".to_string(),
            }]
        );
    }

    #[test]
    fn top_down_bent_route_plans_left_bend_open_endpoint() {
        let from = node("a", 10, 0, 3, 3);
        let to = node("b", 0, 5, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Open);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_bent_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(10, 1, '|', PlannedRouteCellKind::EdgeLine),
                cell(2, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(3, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(1, 1, '+', PlannedRouteCellKind::RouteCell),
                cell(1, 2, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(1, 4, '|', PlannedRouteCellKind::RouteCell),
            ]
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn top_down_back_route_plans_lane_arrow_and_label() {
        let from = node("a", 0, 6, 3, 3);
        let to = node("b", 0, 0, 3, 3);
        let edge = edge(Some("back"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_back_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(2, 7, '|', PlannedRouteCellKind::EdgeLine),
                cell(3, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(4, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 7, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 7, '+', PlannedRouteCellKind::RouteCell),
                cell(6, 2, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 5, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 6, '|', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '+', PlannedRouteCellKind::RouteCell),
                cell(3, 1, '<', PlannedRouteCellKind::EdgeArrow),
                cell(4, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 6, y: 1 },
                end: CanvasCoord { x: 6, y: 7 },
                text: "back".to_string(),
            }]
        );
    }

    #[test]
    fn top_down_direct_route_plans_connector_line_arrow_and_label() {
        let from = node("a", 2, 0, 5, 3);
        let to = node("b", 2, 6, 5, 3);
        let edge = edge(Some("label"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(4, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(4, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 5, 'v', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 4, y: 3 },
                end: CanvasCoord { x: 4, y: 5 },
                text: "label".to_string(),
            }]
        );
    }

    #[test]
    fn top_down_direct_open_route_plans_line_endpoint_without_arrow() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 0, 5, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Open);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells.last(),
            Some(&cell(1, 4, '|', PlannedRouteCellKind::RouteCell))
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn top_down_direct_route_rejects_adjacent_boxes() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 0, 3, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        assert!(plan_top_down_direct_route(&from, &to, &edge, &charset).is_none());
    }

    fn cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
        PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch,
            kind,
        }
    }

    fn edge(label: Option<&str>, arrow: GraphEdgeArrow) -> AsciiGraphEdge {
        edge_between("a", "b", label, arrow)
    }

    fn edge_between(
        from: &str,
        to: &str,
        label: Option<&str>,
        arrow: GraphEdgeArrow,
    ) -> AsciiGraphEdge {
        AsciiGraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            label: label.map(ToOwned::to_owned),
            stroke: GraphEdgeStroke::Normal,
            arrow,
            length: 1,
            style: GraphEdgeStyle::default(),
        }
    }

    fn node(id: &str, x: usize, y: usize, width: usize, height: usize) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            label: GraphLabel::new(id),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x,
            y,
            width,
            height,
        }
    }

    fn left_right_layout(edges: &[(&str, &str)], options: &AsciiRenderOptions) -> GraphLayout {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        if edges.iter().any(|(_, to)| *to == "c") {
            graph.add_node("c", "C");
        }
        for (from, to) in edges {
            graph.add_edge(*from, *to);
        }
        layout_graph(&graph, options)
    }

    fn layout_node<'a>(layout: &'a GraphLayout, id: &str) -> &'a NodeLayout {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .expect("layout should contain test node")
    }
}
