use super::super::layout::CanvasCoord;

mod grid;
mod left_right;
mod select;
mod top_down;

pub(super) use grid::plan_left_right_grid_path_route;
pub(super) use left_right::{
    left_right_back_edge_bottom_y, plan_left_right_bottom_lane_route, plan_left_right_direct_route,
    plan_left_right_down_route, plan_left_right_down_then_right_route,
    plan_left_right_reverse_over_self_loop_route, plan_left_right_right_then_up_route,
    plan_left_right_self_loop_route, self_loop_bottom_y_for_edges, self_loop_right_x,
};
pub(super) use select::{EdgeRouteRequest, plan_edge_route, route_canvas_extent};
pub(super) use top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
    top_down_back_edge_lane_x,
};

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
mod tests;
