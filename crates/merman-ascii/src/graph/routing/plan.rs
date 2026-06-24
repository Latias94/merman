use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::path::StepDirection;

mod boundary;
mod edges;
mod extent;
mod grid;
mod left_right;
mod select;
mod top_down;

pub(super) use extent::route_canvas_extent;
pub(super) use select::{EdgeRouteRequest, plan_edge_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum PlannedRouteSegment {
    Direct,
    Internal,
    Boundary,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlannedRouteCell {
    pub(super) coord: CanvasCoord,
    pub(super) ch: char,
    pub(super) kind: PlannedRouteCellKind,
    pub(super) segment: PlannedRouteSegment,
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
    route_cell_in_segment(x, y, ch, PlannedRouteSegment::Direct)
}

fn route_cell_in_segment(
    x: usize,
    y: usize,
    ch: char,
    segment: PlannedRouteSegment,
) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::RouteCell,
        segment,
    }
}

fn edge_line_cell(x: usize, y: usize, ch: char) -> PlannedRouteCell {
    edge_line_cell_in_segment(x, y, ch, PlannedRouteSegment::Direct)
}

fn edge_line_cell_in_segment(
    x: usize,
    y: usize,
    ch: char,
    segment: PlannedRouteSegment,
) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::EdgeLine,
        segment,
    }
}

fn edge_arrow_cell(x: usize, y: usize, ch: char) -> PlannedRouteCell {
    edge_arrow_cell_in_segment(x, y, ch, PlannedRouteSegment::Direct)
}

fn edge_arrow_cell_in_segment(
    x: usize,
    y: usize,
    ch: char,
    segment: PlannedRouteSegment,
) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind: PlannedRouteCellKind::EdgeArrow,
        segment,
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

fn route_turn_char(previous: StepDirection, next: StepDirection, charset: &GraphCharset) -> char {
    if !charset.unicode {
        return '+';
    }

    match (previous, next) {
        (StepDirection::Right, StepDirection::Down) | (StepDirection::Up, StepDirection::Left) => {
            charset.top_right
        }
        (StepDirection::Right, StepDirection::Up) | (StepDirection::Down, StepDirection::Left) => {
            charset.corner_right_up
        }
        (StepDirection::Left, StepDirection::Down) | (StepDirection::Up, StepDirection::Right) => {
            charset.top_left
        }
        (StepDirection::Left, StepDirection::Up) | (StepDirection::Down, StepDirection::Right) => {
            charset.corner_down_right
        }
        _ => '+',
    }
}

#[cfg(test)]
mod tests;
