use super::super::charset::GraphCharset;
use super::super::layout::CanvasCoord;
use super::super::model::{GraphEdgeMarker, GraphEdgeStroke, GraphEdgeStyle};
use super::label::{
    RoutedLabelDescriptor, RoutedLabelPlacement, routed_label_placement_for_descriptor,
};
use super::path::StepDirection;
use crate::canvas::CanvasColor;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use std::collections::{HashMap, HashSet};

mod boundary;
mod candidates;
mod compound;
mod edges;
mod grid;
mod left_right;
mod same_rank;
mod select;
mod top_down;

pub(super) use candidates::{EdgeRouteCandidates, plan_edge_route_candidates_with_topology};
#[cfg(test)]
pub(super) use select::EdgeRoutePlan;
pub(super) use select::EdgeRouteRequest;
#[cfg(test)]
pub(super) use select::plan_edge_route;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
    pub(super) style: GraphEdgeStyle,
    pub(super) diagram_type: &'static str,
    anchors: MarkerAnchors,
    start_marker: GraphEdgeMarker,
    end_marker: GraphEdgeMarker,
    suppressed_cells: HashSet<PlannedCellId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MarkerEndpoint {
    Start,
    End,
}

pub(super) const MAX_MARKER_CANDIDATES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkerTerminalTail {
    cells: [PlannedCellId; MAX_MARKER_CANDIDATES],
    len: usize,
}

impl MarkerTerminalTail {
    const fn empty() -> Self {
        Self {
            cells: [PlannedCellId(0); MAX_MARKER_CANDIDATES],
            len: 0,
        }
    }

    fn with_appended(mut self, cell: PlannedCellId) -> Option<Self> {
        let slot = self.cells.get_mut(self.len)?;
        *slot = cell;
        self.len += 1;
        Some(self)
    }

    fn as_slice(&self) -> &[PlannedCellId] {
        &self.cells[..self.len]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MarkerCandidate {
    pub(super) cell: PlannedCellId,
    pub(super) coord: CanvasCoord,
    pub(super) point_direction: StepDirection,
    terminal_tail: MarkerTerminalTail,
}

impl MarkerCandidate {
    pub(super) fn is_primary(&self) -> bool {
        self.terminal_tail.as_slice().is_empty()
    }

    pub(super) fn terminal_tail(&self) -> &[PlannedCellId] {
        self.terminal_tail.as_slice()
    }

    pub(super) fn follows_terminal_predecessor(self, predecessor: Self) -> bool {
        let Some((last, prefix)) = self.terminal_tail.as_slice().split_last() else {
            return false;
        };
        *last == predecessor.cell
            && prefix == predecessor.terminal_tail.as_slice()
            && self.point_direction == predecessor.point_direction
            && marker_candidate_is_immediately_inward(
                predecessor.coord,
                self.coord,
                self.point_direction,
            )
    }
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
            diagram_type: "flowchart",
            anchors,
            start_marker: GraphEdgeMarker::Open,
            end_marker: GraphEdgeMarker::Open,
            suppressed_cells: HashSet::new(),
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

    pub(super) fn try_with_stroke(
        mut self,
        stroke: GraphEdgeStroke,
        charset: &GraphCharset,
        diagram_type: &'static str,
    ) -> Result<Self> {
        for cell in &mut self.cells {
            if cell.kind == PlannedRouteCellKind::RouteCell {
                let directions = super::cell::route_char_directions(cell.ch);
                if directions != 0 {
                    cell.directions = directions;
                    cell.ch = super::cell::stroke_route_char(stroke, directions, charset.unicode);
                }
                cell.stroke = stroke;
                cell.unicode = charset.unicode;
            }
        }
        self.diagram_type = diagram_type;
        Ok(self)
    }

    pub(super) fn with_segment(mut self, segment: PlannedRouteSegment) -> Self {
        for cell in &mut self.cells {
            cell.segment = segment;
        }
        self
    }

    #[cfg(test)]
    pub(super) fn with_markers(
        mut self,
        start_marker: GraphEdgeMarker,
        end_marker: GraphEdgeMarker,
        charset: &GraphCharset,
        diagram_type: &'static str,
    ) -> Result<Self> {
        self = self.with_marker_requests(start_marker, end_marker, diagram_type)?;
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

        self.materialize_marker_at(
            MarkerEndpoint::Start,
            MarkerCandidate {
                cell: self.anchors.start.cell,
                coord: start_coord,
                point_direction: self.anchors.start.point_direction,
                terminal_tail: MarkerTerminalTail::empty(),
            },
            charset,
            diagram_type,
        )?;
        self.materialize_marker_at(
            MarkerEndpoint::End,
            MarkerCandidate {
                cell: self.anchors.end.cell,
                coord: end_coord,
                point_direction: self.anchors.end.point_direction,
                terminal_tail: MarkerTerminalTail::empty(),
            },
            charset,
            diagram_type,
        )?;
        Ok(self)
    }

    pub(super) fn with_marker_requests(
        mut self,
        start_marker: GraphEdgeMarker,
        end_marker: GraphEdgeMarker,
        diagram_type: &'static str,
    ) -> Result<Self> {
        if start_marker != GraphEdgeMarker::Open {
            self.marker_coord(self.anchors.start, diagram_type)?;
        }
        if end_marker != GraphEdgeMarker::Open {
            self.marker_coord(self.anchors.end, diagram_type)?;
        }
        self.start_marker = start_marker;
        self.end_marker = end_marker;
        Ok(self)
    }

    pub(super) fn materialized_marker_cell(
        &self,
        endpoint: MarkerEndpoint,
        diagram_type: &'static str,
    ) -> Result<Option<&PlannedRouteCell>> {
        let (marker, anchor) = match endpoint {
            MarkerEndpoint::Start => (self.start_marker, self.anchors.start),
            MarkerEndpoint::End => (self.end_marker, self.anchors.end),
        };
        if marker == GraphEdgeMarker::Open {
            return Ok(None);
        }
        let cell = self
            .cells
            .get(anchor.cell.0)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with missing endpoint marker cells",
            })?;
        if self.is_cell_suppressed(anchor.cell) || cell.kind != PlannedRouteCellKind::EdgeArrow {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with unmaterialized endpoint markers",
            });
        }
        Ok(Some(cell))
    }

    pub(super) fn marker_candidates(
        &self,
        endpoint: MarkerEndpoint,
        diagram_type: &'static str,
        resources: &mut ResourceContext,
    ) -> Result<Vec<MarkerCandidate>> {
        let (marker, anchor) = match endpoint {
            MarkerEndpoint::Start => (self.start_marker, self.anchors.start),
            MarkerEndpoint::End => (self.end_marker, self.anchors.end),
        };
        if marker == GraphEdgeMarker::Open {
            return Ok(Vec::new());
        }

        let primary_candidate = self.terminal_candidate(endpoint, diagram_type)?;
        let primary = self.cells[primary_candidate.cell.0];

        let mut coordinate_index = HashMap::<CanvasCoord, Option<PlannedCellId>>::new();
        coordinate_index
            .try_reserve(self.cells.len())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        for (index, cell) in self.cells.iter().enumerate() {
            resources.charge_layout_work(1)?;
            coordinate_index
                .entry(cell.coord)
                .and_modify(|existing| *existing = None)
                .or_insert(Some(PlannedCellId(index)));
        }

        let mut candidates = Vec::new();
        candidates
            .try_reserve(self.cells.len().min(MAX_MARKER_CANDIDATES))
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        candidates.push(primary_candidate);

        let other_anchor = match endpoint {
            MarkerEndpoint::Start => self.anchors.end,
            MarkerEndpoint::End => self.anchors.start,
        };
        let mut predecessor = primary_candidate;
        for _ in 0..self.cells.len() {
            if candidates.len() >= MAX_MARKER_CANDIDATES {
                break;
            }
            resources.charge_layout_work(1)?;
            let Some(next_coord) =
                marker_relocation_step(predecessor.coord, anchor.point_direction, resources)?
            else {
                break;
            };
            let Some(candidate_id) = coordinate_index.get(&next_coord).copied().flatten() else {
                break;
            };
            if candidate_id == other_anchor.cell || self.is_cell_suppressed(candidate_id) {
                break;
            }
            let candidate_cell = self.cells[candidate_id.0];
            if candidate_cell.segment != primary.segment
                || marker_candidate_has_perpendicular_neighbor(
                    candidate_cell.coord,
                    anchor.point_direction,
                    &coordinate_index,
                )
            {
                break;
            }
            let Some(terminal_tail) = predecessor.terminal_tail.with_appended(predecessor.cell)
            else {
                break;
            };
            let candidate = MarkerCandidate {
                cell: candidate_id,
                coord: candidate_cell.coord,
                point_direction: anchor.point_direction,
                terminal_tail,
            };
            if !candidate.follows_terminal_predecessor(predecessor) {
                break;
            }
            candidates
                .try_reserve(1)
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            candidates.push(candidate);
            predecessor = candidate;
        }

        Ok(candidates)
    }

    pub(super) fn terminal_candidate(
        &self,
        endpoint: MarkerEndpoint,
        diagram_type: &'static str,
    ) -> Result<MarkerCandidate> {
        let anchor = match endpoint {
            MarkerEndpoint::Start => self.anchors.start,
            MarkerEndpoint::End => self.anchors.end,
        };
        let coord = self.marker_coord(anchor, diagram_type)?;
        Ok(MarkerCandidate {
            cell: anchor.cell,
            coord,
            point_direction: anchor.point_direction,
            terminal_tail: MarkerTerminalTail::empty(),
        })
    }

    pub(super) const fn marker_point_direction(&self, endpoint: MarkerEndpoint) -> StepDirection {
        match endpoint {
            MarkerEndpoint::Start => self.anchors.start.point_direction,
            MarkerEndpoint::End => self.anchors.end.point_direction,
        }
    }

    pub(super) fn materialize_marker_at(
        &mut self,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
        charset: &GraphCharset,
        diagram_type: &'static str,
    ) -> Result<()> {
        if candidate.terminal_tail().contains(&candidate.cell) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "endpoint marker suppression includes selected berth",
            });
        }
        for suppressed in candidate.terminal_tail() {
            if self.cells.get(suppressed.0).is_none() {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type,
                    feature: "routes with missing suppressed terminal cells",
                });
            }
        }
        if self.cells.get(candidate.cell.0).is_none() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with missing endpoint marker cells",
            });
        }
        if self.is_cell_suppressed(candidate.cell) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "endpoint marker selected on a suppressed terminal cell",
            });
        }
        self.suppressed_cells
            .try_reserve(candidate.terminal_tail().len())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        self.suppressed_cells
            .extend(candidate.terminal_tail().iter().copied());

        let marker = match endpoint {
            MarkerEndpoint::Start => {
                self.anchors.start = MarkerAnchor::new(candidate.cell, candidate.point_direction);
                self.start_marker
            }
            MarkerEndpoint::End => {
                self.anchors.end = MarkerAnchor::new(candidate.cell, candidate.point_direction);
                self.end_marker
            }
        };
        self.materialize_marker(
            MarkerAnchor::new(candidate.cell, candidate.point_direction),
            marker,
            charset,
            diagram_type,
        )
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
        cell.paint = PlannedRoutePaint::role(AsciiColorRole::EdgeArrow)
            .with_edge_style(PlannedRouteCellKind::EdgeArrow, self.style);
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
            diagram_type: "flowchart",
            anchors,
            start_marker: GraphEdgeMarker::Open,
            end_marker: GraphEdgeMarker::Open,
            suppressed_cells: HashSet::new(),
            min_canvas_extent: CanvasExtent { width, height },
        }
    }

    pub(super) fn active_cells(&self) -> impl Iterator<Item = (PlannedCellId, &PlannedRouteCell)> {
        self.cells.iter().enumerate().filter_map(|(index, cell)| {
            let cell_id = PlannedCellId(index);
            (!self.is_cell_suppressed(cell_id)).then_some((cell_id, cell))
        })
    }

    pub(super) fn is_cell_suppressed(&self, cell: PlannedCellId) -> bool {
        self.suppressed_cells.contains(&cell)
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

        for (_, cell) in self.active_cells() {
            width = width.max(resources.checked_grid_add(cell.coord.x, 1)?);
            height = height.max(resources.checked_grid_add(cell.coord.y, 1)?);
        }
        for label in &self.labels {
            let label_width =
                resources.checked_grid_add(label.placement.x(), label.placement.width())?;
            let label_height =
                resources.checked_grid_add(label.placement.y(), label.line_count().max(1))?;
            width = width.max(label_width);
            height = height.max(label_height);
        }

        Ok((width, height))
    }
}

fn marker_relocation_step(
    coord: CanvasCoord,
    point_direction: StepDirection,
    resources: &ResourceContext,
) -> Result<Option<CanvasCoord>> {
    Ok(match point_direction {
        StepDirection::Up => Some(CanvasCoord {
            x: coord.x,
            y: resources.checked_grid_add(coord.y, 1)?,
        }),
        StepDirection::Right => coord
            .x
            .checked_sub(1)
            .map(|x| CanvasCoord { x, y: coord.y }),
        StepDirection::Down => coord
            .y
            .checked_sub(1)
            .map(|y| CanvasCoord { x: coord.x, y }),
        StepDirection::Left => Some(CanvasCoord {
            x: resources.checked_grid_add(coord.x, 1)?,
            y: coord.y,
        }),
    })
}

fn marker_candidate_is_immediately_inward(
    predecessor: CanvasCoord,
    candidate: CanvasCoord,
    point_direction: StepDirection,
) -> bool {
    match point_direction {
        StepDirection::Up => {
            predecessor.x == candidate.x && predecessor.y.checked_add(1) == Some(candidate.y)
        }
        StepDirection::Right => {
            predecessor.y == candidate.y && predecessor.x.checked_sub(1) == Some(candidate.x)
        }
        StepDirection::Down => {
            predecessor.x == candidate.x && predecessor.y.checked_sub(1) == Some(candidate.y)
        }
        StepDirection::Left => {
            predecessor.y == candidate.y && predecessor.x.checked_add(1) == Some(candidate.x)
        }
    }
}

fn marker_candidate_has_perpendicular_neighbor(
    coord: CanvasCoord,
    point_direction: StepDirection,
    coordinate_index: &HashMap<CanvasCoord, Option<PlannedCellId>>,
) -> bool {
    let neighbors = match point_direction {
        StepDirection::Up | StepDirection::Down => [
            coord
                .x
                .checked_sub(1)
                .map(|x| CanvasCoord { x, y: coord.y }),
            coord
                .x
                .checked_add(1)
                .map(|x| CanvasCoord { x, y: coord.y }),
        ],
        StepDirection::Left | StepDirection::Right => [
            coord
                .y
                .checked_sub(1)
                .map(|y| CanvasCoord { x: coord.x, y }),
            coord
                .y
                .checked_add(1)
                .map(|y| CanvasCoord { x: coord.x, y }),
        ],
    };

    neighbors
        .into_iter()
        .flatten()
        .any(|neighbor| coordinate_index.contains_key(&neighbor))
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
    pub(super) stroke: GraphEdgeStroke,
    pub(super) directions: u8,
    pub(super) unicode: bool,
    pub(super) paint: PlannedRoutePaint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PlannedCellId(usize);

impl PlannedCellId {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
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

#[derive(Debug, Clone)]
pub(super) struct PlannedRouteLabel {
    pub(super) descriptor: RoutedLabelDescriptor,
    pub(super) placement: RoutedLabelPlacement,
    pub(super) paint: PlannedRoutePaint,
    pub(super) anchor: LabelAnchor,
}

impl PlannedRouteLabel {
    pub(super) fn new(
        descriptor: impl Into<RoutedLabelDescriptor>,
        placement: RoutedLabelPlacement,
    ) -> Self {
        let descriptor = descriptor.into();
        Self {
            descriptor,
            placement,
            paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLabel),
            anchor: LabelAnchor::PlacementHint(CanvasCoord {
                x: placement.x(),
                y: placement.y(),
            }),
        }
    }

    fn with_host_segment(
        descriptor: RoutedLabelDescriptor,
        placement: RoutedLabelPlacement,
        start: CanvasCoord,
        end: CanvasCoord,
    ) -> Self {
        Self {
            anchor: LabelAnchor::Segment {
                start,
                end,
                route_segment: None,
            },
            ..Self::new(descriptor, placement)
        }
    }

    pub(super) const fn width(&self) -> usize {
        self.descriptor.width()
    }

    pub(super) const fn line_count(&self) -> usize {
        self.descriptor.line_count()
    }
}

impl PartialEq for PlannedRouteLabel {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
            && self.placement == other.placement
            && self.paint == other.paint
    }
}

impl Eq for PlannedRouteLabel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LabelAnchor {
    Segment {
        start: CanvasCoord,
        end: CanvasCoord,
        route_segment: Option<PlannedRouteSegment>,
    },
    PlacementHint(CanvasCoord),
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
        stroke: GraphEdgeStroke::Normal,
        directions: 0,
        unicode: false,
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
        stroke: GraphEdgeStroke::Normal,
        directions: 0,
        unicode: false,
        paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
    }
}

fn planned_label(
    descriptor: Option<RoutedLabelDescriptor>,
    start: CanvasCoord,
    end: CanvasCoord,
) -> Option<PlannedRouteLabel> {
    let descriptor = descriptor?;
    let placement = routed_label_placement_for_descriptor(start, end, descriptor)?;
    Some(PlannedRouteLabel::with_host_segment(
        descriptor, placement, start, end,
    ))
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
mod u6_tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    #[test]
    fn marker_relocation_reports_checked_grid_overflow() {
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));

        let error = marker_relocation_step(
            CanvasCoord {
                x: usize::MAX,
                y: 0,
            },
            StepDirection::Left,
            &resources,
        )
        .expect_err("marker relocation must not turn coordinate overflow into berth exhaustion");

        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }
}

#[cfg(test)]
mod tests;
