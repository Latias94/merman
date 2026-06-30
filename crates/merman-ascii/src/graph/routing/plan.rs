use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::super::model::GraphEdgeStyle;
use super::label::{RoutedLabelPlacement, RoutedLabelText, routed_label_placement_for_text};
use super::path::StepDirection;
use crate::canvas::CanvasColor;
use crate::color::{AsciiColorRole, AsciiRgb};

mod boundary;
mod edges;
mod grid;
mod left_right;
mod select;
mod top_down;

pub(super) use select::{EdgeRoutePlan, EdgeRouteRequest, plan_edge_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
    pub(super) style: GraphEdgeStyle,
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
            style: GraphEdgeStyle::default(),
            min_canvas_extent: CanvasExtent::default(),
        }
    }

    pub(super) fn with_style(mut self, style: GraphEdgeStyle) -> Self {
        self.style = style;
        for cell in &mut self.cells {
            cell.paint = cell.paint.with_edge_style(cell.kind, style);
        }
        for label in &mut self.labels {
            label.paint = label.paint.with_color(style.label);
        }
        self
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
            style: GraphEdgeStyle::default(),
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
            let (label_width, label_height) = label
                .placement
                .canvas_extent_for_lines(label.text.line_count());
            width = width.max(label_width);
            height = height.max(label_height);
        }

        (width, height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlannedRouteSegment {
    Direct,
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlannedRouteCell {
    pub(super) coord: CanvasCoord,
    pub(super) ch: char,
    pub(super) kind: PlannedRouteCellKind,
    pub(super) segment: PlannedRouteSegment,
    pub(super) paint: PlannedRoutePaint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlannedRouteCellKind {
    EdgeLine,
    RouteCell,
    EdgeArrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlannedRouteLabel {
    pub(super) text: RoutedLabelText,
    pub(super) placement: RoutedLabelPlacement,
    pub(super) paint: PlannedRoutePaint,
}

impl PlannedRouteLabel {
    pub(super) fn new(text: RoutedLabelText, placement: RoutedLabelPlacement) -> Self {
        Self {
            text,
            placement,
            paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLabel),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlannedRoutePaint {
    pub(super) color: CanvasColor,
}

impl PlannedRoutePaint {
    pub(super) fn role(role: AsciiColorRole) -> Self {
        Self {
            color: CanvasColor::Role(role),
        }
    }

    fn with_color(self, color: Option<AsciiRgb>) -> Self {
        match color {
            Some(color) => Self {
                color: CanvasColor::Direct(color),
            },
            None => self,
        }
    }

    fn with_edge_style(self, kind: PlannedRouteCellKind, style: GraphEdgeStyle) -> Self {
        match kind {
            PlannedRouteCellKind::EdgeArrow => self.with_color(style.arrow.or(style.line)),
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => {
                self.with_color(style.line)
            }
        }
    }
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
        paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
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
        paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
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
        paint: PlannedRoutePaint::role(AsciiColorRole::EdgeArrow),
    }
}

fn planned_label(
    label: Option<&str>,
    start: CanvasCoord,
    end: CanvasCoord,
) -> Option<PlannedRouteLabel> {
    let label = label.filter(|label| !label.trim().is_empty())?;
    let text = RoutedLabelText::new(label)?;
    let placement = routed_label_placement_for_text(start, end, &text)?;
    Some(PlannedRouteLabel::new(text, placement))
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
