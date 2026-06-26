use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::label::{RoutedLabelPlacement, routed_label_placement};
use super::path::StepDirection;

mod boundary;
mod edges;
mod grid;
mod left_right;
mod select;
mod top_down;

pub(super) use select::{EdgeRouteRequest, plan_edge_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
    min_canvas_extent: CanvasExtent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct CanvasExtent {
    width: usize,
    height: usize,
}

impl RoutePlan {
    pub(super) fn new(cells: Vec<PlannedRouteCell>, labels: Vec<PlannedRouteLabel>) -> Self {
        Self {
            cells,
            labels,
            min_canvas_extent: CanvasExtent::default(),
        }
    }

    pub(super) fn with_min_canvas_extent(
        cells: Vec<PlannedRouteCell>,
        labels: Vec<PlannedRouteLabel>,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            cells,
            labels,
            min_canvas_extent: CanvasExtent { width, height },
        }
    }

    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        let mut width = self.min_canvas_extent.width;
        let mut height = self.min_canvas_extent.height;

        for cell in &self.cells {
            width = width.max(cell.coord.x.saturating_add(1));
            height = height.max(cell.coord.y.saturating_add(1));
        }
        for label in &self.labels {
            let (label_width, label_height) = label.placement.canvas_extent();
            width = width.max(label_width);
            height = height.max(label_height);
        }

        (width, height)
    }
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
    pub(super) text: String,
    pub(super) placement: RoutedLabelPlacement,
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
    let label = label.filter(|label| !label.is_empty())?;
    let placement = routed_label_placement(start, end, label)?;
    Some(PlannedRouteLabel {
        text: label.to_string(),
        placement,
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
