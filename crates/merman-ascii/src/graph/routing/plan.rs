use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::super::model::{GraphEdgeMarker, GraphEdgeStyle};
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
    anchors: MarkerAnchors,
    min_canvas_extent: CanvasExtent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct CanvasExtent {
    width: usize,
    height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MarkerAnchor {
    cell: PlannedCellId,
    point_direction: StepDirection,
}

impl MarkerAnchor {
    pub(super) const fn new(cell: PlannedCellId, point_direction: StepDirection) -> Self {
        Self {
            cell,
            point_direction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MarkerAnchors {
    start: MarkerAnchor,
    end: MarkerAnchor,
}

impl MarkerAnchors {
    pub(super) const fn new(start: MarkerAnchor, end: MarkerAnchor) -> Self {
        Self { start, end }
    }
}

impl RoutePlan {
    pub(super) fn new(
        cells: Vec<PlannedRouteCell>,
        labels: Vec<PlannedRouteLabel>,
        anchors: MarkerAnchors,
    ) -> Self {
        Self {
            cells,
            labels,
            style: GraphEdgeStyle::default(),
            anchors,
            min_canvas_extent: CanvasExtent::default(),
        }
    }

    #[cfg(test)]
    pub(super) fn new_without_markers_for_test(
        cells: Vec<PlannedRouteCell>,
        labels: Vec<PlannedRouteLabel>,
    ) -> Self {
        let placeholder = MarkerAnchor::new(PlannedCellId(0), StepDirection::Right);
        Self::new(cells, labels, MarkerAnchors::new(placeholder, placeholder))
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

    pub(super) fn with_markers(
        mut self,
        start_marker: GraphEdgeMarker,
        end_marker: GraphEdgeMarker,
        charset: &GraphCharset,
        diagram_type: &'static str,
    ) -> Result<Self> {
        let start_coord = self.marker_coord(self.anchors.start, diagram_type)?;
        let end_coord = self.marker_coord(self.anchors.end, diagram_type)?;
        if start_marker != GraphEdgeMarker::Open
            && end_marker != GraphEdgeMarker::Open
            && start_coord == end_coord
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes too short for independent endpoint markers",
            });
        }

        self.materialize_marker(self.anchors.start, start_marker, charset, diagram_type)?;
        self.materialize_marker(self.anchors.end, end_marker, charset, diagram_type)?;
        Ok(self)
    }

    fn marker_coord(
        &self,
        anchor: MarkerAnchor,
        diagram_type: &'static str,
    ) -> Result<CanvasCoord> {
        self.cells
            .get(anchor.cell.0)
            .map(|cell| cell.coord)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with missing endpoint marker cells",
            })
    }

    fn materialize_marker(
        &mut self,
        anchor: MarkerAnchor,
        marker: GraphEdgeMarker,
        charset: &GraphCharset,
        diagram_type: &'static str,
    ) -> Result<()> {
        if marker == GraphEdgeMarker::Open {
            return Ok(());
        }
        let cell = self
            .cells
            .get_mut(anchor.cell.0)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with missing endpoint marker cells",
            })?;
        cell.ch = marker_char(marker, point_char(anchor.point_direction, charset), charset);
        cell.kind = PlannedRouteCellKind::EdgeArrow;
        cell.paint = PlannedRoutePaint::role(AsciiColorRole::EdgeArrow);
        Ok(())
    }

    pub(super) fn with_min_canvas_extent(
        cells: Vec<PlannedRouteCell>,
        labels: Vec<PlannedRouteLabel>,
        anchors: MarkerAnchors,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            cells,
            labels,
            style: GraphEdgeStyle::default(),
            anchors,
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

#[cfg(test)]
fn materialize_test_markers(
    planned: Result<Option<RoutePlan>>,
    edge: &super::super::model::AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    planned
        .expect("test route planning work must remain representable")
        .map(|plan| {
            plan.with_markers(edge.start_marker, edge.end_marker, charset, "flowchart")
                .expect("test endpoint markers must fit the planned route")
        })
}

fn point_char(direction: StepDirection, charset: &GraphCharset) -> char {
    match direction {
        StepDirection::Up => charset.arrow_up,
        StepDirection::Right => charset.arrow_right,
        StepDirection::Down => charset.arrow_down,
        StepDirection::Left => charset.arrow_left,
    }
}

fn marker_char(marker: GraphEdgeMarker, point: char, charset: &GraphCharset) -> char {
    match marker {
        GraphEdgeMarker::Open => point,
        GraphEdgeMarker::Point => point,
        GraphEdgeMarker::Circle => charset.circle_marker,
        GraphEdgeMarker::Cross => charset.cross_marker,
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
pub(super) struct PlannedCellId(usize);

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
    ) -> Result<PlannedCellId> {
        resources.charge_layout_work(1)?;
        self.inner
            .try_reserve(1)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        let id = PlannedCellId(self.inner.len());
        self.inner.push(build());
        Ok(id)
    }

    fn try_push_anchor(
        &mut self,
        resources: &mut ResourceContext,
        build: impl FnOnce() -> PlannedRouteCell,
        point_direction: StepDirection,
    ) -> Result<MarkerAnchor> {
        let cell = self.try_push(resources, build)?;
        Ok(MarkerAnchor::new(cell, point_direction))
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
