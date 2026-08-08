use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::super::model::GraphEdgeStyle;
use super::label::{RoutedLabelPlacement, RoutedLabelText, routed_label_placement_for_text};
use super::path::StepDirection;
use crate::canvas::CanvasColor;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

mod boundary;
mod edges;
mod grid;
mod left_right;
mod same_rank;
mod select;
mod top_down;

#[cfg(test)]
pub(super) use select::plan_edge_route;
pub(super) use select::{EdgeRoutePlan, EdgeRouteRequest, plan_edge_route_with_topology};

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

    #[cfg(test)]
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        let resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        ));
        self.canvas_extent_with_resources(&resources)
            .expect("test route geometry must remain representable")
    }

    pub(super) fn canvas_extent_with_resources(
        &self,
        resources: &ResourceContext,
    ) -> Result<(usize, usize)> {
        let mut width = self.min_canvas_extent.width;
        let mut height = self.min_canvas_extent.height;

        for cell in &self.cells {
            width = width.max(resources.checked_grid_add(cell.coord.x, 1)?);
            height = height.max(resources.checked_grid_add(cell.coord.y, 1)?);
        }
        for label in &self.labels {
            let label_width =
                resources.checked_grid_add(label.placement.x(), label.placement.width())?;
            let label_height =
                resources.checked_grid_add(label.placement.y(), label.text.line_count().max(1))?;
            width = width.max(label_width);
            height = height.max(label_height);
        }

        Ok((width, height))
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

#[derive(Debug, Default)]
struct PlannedRouteCells {
    inner: Vec<PlannedRouteCell>,
}

impl PlannedRouteCells {
    fn new() -> Self {
        Self::default()
    }

    fn try_push(
        &mut self,
        resources: &mut ResourceContext,
        build: impl FnOnce() -> PlannedRouteCell,
    ) -> Result<()> {
        resources.charge_layout_work(1)?;
        self.inner
            .try_reserve(1)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        self.inner.push(build());
        Ok(())
    }

    fn into_vec(self) -> Vec<PlannedRouteCell> {
        self.inner
    }
}

#[cfg(test)]
fn unbounded_route_resources() -> ResourceContext {
    ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ))
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
    charset: &GraphCharset,
) -> Option<PlannedRouteLabel> {
    let text = RoutedLabelText::new_with_profile(label?, charset.width_profile)?;
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
