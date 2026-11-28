//! Mermaid 12.4 Grid anchors on an existing layout route (grid/edgeLabels.ts).

use super::{
    GridSegment, POINT_EPSILON, crosses_rect, finite, finite_rect, invalid_geometry, normalize,
    reserve_vec,
};
use crate::Result;
use crate::graph_layout::{Axis, Size};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;
use std::collections::HashSet;

pub(in crate::graph_layout::mermaid_grid) mod detour;
pub(in crate::graph_layout::mermaid_grid) mod foreign;

const LABEL_CLEARANCE: f64 = 6.0;
const MARKER_LENGTH: f64 = 12.0;
const MARKER_HALF_WIDTH: f64 = 7.0;
const GUTTER: f64 = 14.0;
const GUTTER_STEP: f64 = 18.0;
const GUTTER_STEPS: u8 = 16;
const MAX_LANES: usize = 128;

#[derive(Clone, Copy, Debug, Default)]
pub struct GridLabelMarkers {
    pub start: bool,
    pub end: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridLabelObstacles<'a> {
    /// Measured non-group nodes, group titles and already placed labels, excluding this label.
    pub solids: &'a [Rect],
    /// Group perimeters. Labels can sit inside a group if they do not cut its border.
    pub group_borders: &'a [Rect],
    /// Segments of other edges before outline clipping and paint.
    pub foreign_segments: &'a [GridSegment],
}

#[derive(Clone, Copy)]
pub(in crate::graph_layout::mermaid_grid) struct LabelCollisionView<'a> {
    pub solids: &'a [Rect],
    pub group_borders: &'a [Rect],
    /// The flattened route inventory before and after the owner's occurrence.
    pub foreign_segments: [&'a [GridSegment]; 2],
}

#[derive(Debug)]
pub(in crate::graph_layout::mermaid_grid) enum LabelSearchError {
    Layout(crate::Error),
    WorkLimit,
}

impl From<crate::Error> for LabelSearchError {
    fn from(error: crate::Error) -> Self {
        Self::Layout(error)
    }
}

impl From<OperationWorkError> for LabelSearchError {
    fn from(error: OperationWorkError) -> Self {
        Self::Layout(error.into())
    }
}

pub(in crate::graph_layout::mermaid_grid) type SearchResult<T> =
    std::result::Result<T, LabelSearchError>;

pub(in crate::graph_layout::mermaid_grid) struct LabelRouteProposal {
    pub points: Vec<Point>,
    pub segments: Vec<GridSegment>,
}

impl LabelRouteProposal {
    fn materialize(
        start: Point,
        segments: Vec<GridSegment>,
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Self> {
        let count = segments
            .len()
            .checked_add(1)
            .ok_or_else(|| work.meter.arithmetic_overflow())?;
        work.charge(count)?;
        let mut points = reserve_vec(count)?;
        points.push(start);
        points.extend(segments.iter().map(|segment| segment.end));
        Ok(Self { points, segments })
    }
}

pub(in crate::graph_layout::mermaid_grid) struct LabelSearchWork<'a> {
    meter: &'a OperationWorkMeter,
    remaining: Option<usize>,
}

impl<'a> LabelSearchWork<'a> {
    pub(in crate::graph_layout::mermaid_grid) fn new(
        meter: &'a OperationWorkMeter,
        remaining: Option<usize>,
    ) -> Self {
        Self { meter, remaining }
    }

    pub(in crate::graph_layout::mermaid_grid) fn charge(
        &mut self,
        units: usize,
    ) -> SearchResult<()> {
        let next = self
            .remaining
            .and_then(|remaining| remaining.checked_sub(units));
        if self.remaining.is_some() && next.is_none() {
            self.meter.checkpoint(OperationPhase::Layout)?;
            return Err(LabelSearchError::WorkLimit);
        }
        self.meter.charge(units)?;
        self.remaining = next;
        Ok(())
    }

    pub(in crate::graph_layout::mermaid_grid) fn charge_sort(
        &mut self,
        count: usize,
        key_bytes: usize,
    ) -> SearchResult<()> {
        let comparisons = count
            .checked_mul(count.checked_ilog2().unwrap_or(0) as usize + 1)
            .and_then(|cost| cost.checked_mul(key_bytes.max(1)))
            .ok_or_else(|| self.meter.arithmetic_overflow())?;
        self.charge(comparisons)
    }
}

/// Try the existing-segment pass. `None` leaves reroute or degradation to the caller.
pub fn place_grid_label_on_route(
    points: &[Point],
    size: Size,
    markers: GridLabelMarkers,
    obstacles: GridLabelObstacles<'_>,
    work: &OperationWorkMeter,
) -> Result<Option<Point>> {
    work.checkpoint(OperationPhase::Layout)?;
    if !measured_label_size(size) {
        return Ok(None);
    }
    let segments = label_segments(points, work)?;
    let obstacles = LabelCollisionView {
        solids: obstacles.solids,
        group_borders: obstacles.group_borders,
        foreign_segments: [obstacles.foreign_segments, &[]],
    };
    match place_label_on_segments(
        &segments,
        size,
        markers,
        obstacles,
        &mut LabelSearchWork::new(work, None),
    ) {
        Ok(anchor) => Ok(anchor),
        Err(LabelSearchError::Layout(error)) => Err(error),
        Err(LabelSearchError::WorkLimit) => Ok(None),
    }
}

pub(in crate::graph_layout::mermaid_grid) fn measured_label_size(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

pub(in crate::graph_layout::mermaid_grid) fn place_label_on_segments(
    segments: &[GridSegment],
    size: Size,
    markers: GridLabelMarkers,
    obstacles: LabelCollisionView<'_>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<Option<Point>> {
    visit_label_centers(segments, size, markers, obstacles, work, |_, _| Ok(true))
}

/// Visits centers which passed the supplied obstacle checks, in placement order.
pub(in crate::graph_layout::mermaid_grid) fn visit_label_centers(
    segments: &[GridSegment],
    size: Size,
    markers: GridLabelMarkers,
    obstacles: LabelCollisionView<'_>,
    work: &mut LabelSearchWork<'_>,
    mut accept: impl FnMut(Rect, &mut LabelSearchWork<'_>) -> SearchResult<bool>,
) -> SearchResult<Option<Point>> {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return Ok(None);
    };
    let marker_rects = [
        markers
            .start
            .then(|| marker_rect(first.start, first.end))
            .flatten(),
        markers
            .end
            .then(|| marker_rect(last.end, last.start))
            .flatten(),
    ];

    // Interior segments win an equal-length tie, followed by the first and last legs.
    work.charge(segments.len())?;
    let mut candidates = reserve_vec(segments.len())?;
    if segments.len() > 2 {
        candidates.extend(segments[1..segments.len() - 1].iter().copied());
        candidates.extend([*first, *last]);
    } else {
        candidates.extend_from_slice(segments);
    }
    stable_rank(&mut candidates, work, |a, b| {
        length(*a) - length(*b) > POINT_EPSILON
    })?;
    for segment in candidates {
        work.charge(1)?;
        if let Some(anchor) =
            place_on_segment(segment, size, obstacles, &marker_rects, work, &mut accept)?
        {
            return Ok(Some(anchor));
        }
    }
    work.meter.checkpoint(OperationPhase::Layout)?;
    Ok(None)
}

/// Anchor the first longest segment. Degenerate routes use the first finite route point, then
/// the source or target center. Use this only after the layout owner selects degraded placement.
pub fn grid_label_fallback_anchor(
    points: &[Point],
    source_center: Option<Point>,
    target_center: Option<Point>,
    work: &OperationWorkMeter,
) -> Result<Option<Point>> {
    let segments = label_segments(points, work)?;
    label_fallback_anchor(points, &segments, source_center, target_center, work)
}

pub(in crate::graph_layout::mermaid_grid) fn label_fallback_anchor(
    points: &[Point],
    segments: &[GridSegment],
    source_center: Option<Point>,
    target_center: Option<Point>,
    work: &OperationWorkMeter,
) -> Result<Option<Point>> {
    let mut longest = None;
    for &segment in segments {
        work.charge(1)?;
        if longest.is_none_or(|previous| length(segment) > length(previous)) {
            longest = Some(segment);
        }
    }
    if let Some(segment) = longest {
        let midpoint = Point::new(
            (segment.start.x + segment.end.x) / 2.0,
            (segment.start.y + segment.end.y) / 2.0,
        );
        return if finite(midpoint) {
            Ok(Some(midpoint))
        } else {
            Err(invalid_geometry())
        };
    }
    work.charge(points.len())?;
    Ok(points
        .iter()
        .copied()
        .find(|&point| finite(point))
        .or(source_center.filter(|&point| finite(point)))
        .or(target_center.filter(|&point| finite(point))))
}

pub(in crate::graph_layout::mermaid_grid) fn label_segments(
    points: &[Point],
    work: &OperationWorkMeter,
) -> Result<Vec<GridSegment>> {
    let segments = normalize(points, POINT_EPSILON, work)?;
    work.charge(segments.len())?;
    if segments
        .iter()
        .any(|&segment| segment.fixed() != segment.end_fixed() || !length(segment).is_finite())
    {
        return Err(invalid_geometry());
    }
    Ok(segments)
}

fn length(segment: GridSegment) -> f64 {
    let (low, high) = segment.span();
    high - low
}

fn segment_contains_anchor(segment: GridSegment, center: Point) -> bool {
    let (along, cross) = match segment.axis {
        Axis::Horizontal => (center.x, center.y),
        Axis::Vertical => (center.y, center.x),
    };
    let (low, high) = segment.span();
    high - low > POINT_EPSILON
        && (cross - segment.fixed()).abs() <= POINT_EPSILON
        && along >= low - POINT_EPSILON
        && along <= high + POINT_EPSILON
}

fn marker_rect(tip: Point, inner: Point) -> Option<Rect> {
    let leg = GridSegment::new(tip, inner)?;
    Some(match leg.axis {
        Axis::Horizontal => {
            let inset = tip.x + (inner.x - tip.x).signum() * MARKER_LENGTH;
            Rect::new(
                tip.x.min(inset),
                tip.y - MARKER_HALF_WIDTH,
                tip.x.max(inset),
                tip.y + MARKER_HALF_WIDTH,
            )
        }
        Axis::Vertical => {
            let inset = tip.y + (inner.y - tip.y).signum() * MARKER_LENGTH;
            Rect::new(
                tip.x - MARKER_HALF_WIDTH,
                tip.y.min(inset),
                tip.x + MARKER_HALF_WIDTH,
                tip.y.max(inset),
            )
        }
    })
}

#[derive(Clone, Copy)]
struct Interval {
    low: f64,
    high: f64,
}

impl Interval {
    fn expanded(self, half: f64) -> Self {
        Self {
            low: self.low - half,
            high: self.high + half,
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.low.max(other.low) < self.high.min(other.high) - POINT_EPSILON
    }
}

struct CenterCandidate {
    along: f64,
    interval: Interval,
}

fn place_on_segment(
    segment: GridSegment,
    size: Size,
    obstacles: LabelCollisionView<'_>,
    markers: &[Option<Rect>; 2],
    work: &mut LabelSearchWork<'_>,
    accept: &mut impl FnMut(Rect, &mut LabelSearchWork<'_>) -> SearchResult<bool>,
) -> SearchResult<Option<Point>> {
    let (along_size, cross_size) = match segment.axis {
        Axis::Horizontal => (size.width, size.height),
        Axis::Vertical => (size.height, size.width),
    };
    let half = along_size / 2.0;
    let inset = half + LABEL_CLEARANCE;
    let (start, end) = segment.span();
    let bounds = Interval {
        low: start + inset,
        high: end - inset,
    };
    if bounds.high - bounds.low <= POINT_EPSILON {
        return Ok(None);
    }
    let preferred = (start + end) / 2.0;
    let fixed = segment.fixed();
    let band = Interval {
        low: fixed,
        high: fixed,
    }
    .expanded(cross_size / 2.0);
    if !preferred.is_finite() || !band.low.is_finite() || !band.high.is_finite() {
        return Err(invalid_geometry().into());
    }

    let solids = || {
        obstacles
            .solids
            .iter()
            .chain(markers.iter().flatten())
            .copied()
    };
    let mut blocked = Vec::new();
    for rect in solids() {
        work.charge(1)?;
        if !finite_rect(rect) {
            return Err(invalid_geometry().into());
        }
        let (along, cross) = rect_axes(rect, segment.axis);
        if band.overlaps(cross) {
            push_blocked(&mut blocked, along.expanded(half), bounds)?;
        }
    }
    for &other in obstacles.foreign_segments.into_iter().flatten() {
        work.charge(1)?;
        let (low, high) = other.span();
        let cross = other.fixed();
        if segment.axis == other.axis {
            if band.low < cross && cross < band.high {
                push_blocked(&mut blocked, Interval { low, high }.expanded(half), bounds)?;
            }
        } else if band.overlaps(Interval { low, high }) {
            push_blocked(
                &mut blocked,
                Interval {
                    low: cross,
                    high: cross,
                }
                .expanded(half),
                bounds,
            )?;
        }
    }

    let count = blocked.len();
    work.charge_sort(count, 1)?;
    blocked.sort_unstable_by(|a, b| {
        a.low
            .total_cmp(&b.low)
            .then_with(|| a.high.total_cmp(&b.high))
    });
    let mut candidates = reserve_vec(
        count
            .checked_add(1)
            .ok_or_else(|| work.meter.arithmetic_overflow())?,
    )?;
    let mut cursor = bounds.low;
    for next in blocked
        .iter()
        .map(|interval| interval.low)
        .chain([bounds.high])
        .enumerate()
    {
        work.charge(1)?;
        let (index, high) = next;
        if high - cursor > POINT_EPSILON {
            candidates.push(CenterCandidate {
                along: preferred.clamp(cursor, high),
                interval: Interval { low: cursor, high },
            });
        }
        if let Some(interval) = blocked.get(index) {
            cursor = cursor.max(interval.high);
        }
    }
    stable_rank(&mut candidates, work, |a, b| {
        let delta = (a.along - preferred).abs() - (b.along - preferred).abs();
        if delta.abs() > POINT_EPSILON {
            return delta < 0.0;
        }
        let width_delta = (a.interval.high - a.interval.low) - (b.interval.high - b.interval.low);
        if width_delta.abs() > POINT_EPSILON {
            return width_delta > 0.0;
        }
        a.along < b.along
    })?;

    let mut seen = HashSet::new();
    seen.try_reserve(candidates.len())
        .map_err(|_| super::allocation_failed())?;
    for candidate in candidates {
        work.charge(1)?;
        if !seen.insert(rounded_center_key(candidate.along)) {
            continue;
        }
        let center = match segment.axis {
            Axis::Horizontal => Point::new(candidate.along, fixed),
            Axis::Vertical => Point::new(fixed, candidate.along),
        };
        let label = label_rect_at(center, size);
        if label_is_clear(label, obstacles, solids(), work)? && accept(label, work)? {
            return Ok(Some(center));
        }
    }
    Ok(None)
}

pub(in crate::graph_layout::mermaid_grid) fn label_rect_at(center: Point, size: Size) -> Rect {
    Rect::new(
        center.x - size.width / 2.0,
        center.y - size.height / 2.0,
        center.x + size.width / 2.0,
        center.y + size.height / 2.0,
    )
}

fn rect_axes(rect: Rect, axis: Axis) -> (Interval, Interval) {
    let x = Interval {
        low: rect.x0,
        high: rect.x1,
    };
    let y = Interval {
        low: rect.y0,
        high: rect.y1,
    };
    match axis {
        Axis::Horizontal => (x, y),
        Axis::Vertical => (y, x),
    }
}

fn push_blocked(blocked: &mut Vec<Interval>, interval: Interval, bounds: Interval) -> Result<()> {
    let low = interval.low.clamp(bounds.low, bounds.high);
    let high = interval.high.clamp(bounds.low, bounds.high);
    if high - low > POINT_EPSILON {
        blocked
            .try_reserve(1)
            .map_err(|_| super::allocation_failed())?;
        blocked.push(Interval { low, high });
    }
    Ok(())
}

fn label_is_clear(
    label: Rect,
    obstacles: LabelCollisionView<'_>,
    solids: impl Iterator<Item = Rect>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<bool> {
    for solid in solids {
        work.charge(1)?;
        if label.x0 < solid.x1 && label.x1 > solid.x0 && label.y0 < solid.y1 && label.y1 > solid.y0
        {
            return Ok(false);
        }
    }
    for &border in obstacles.group_borders {
        work.charge(1)?;
        if !finite_rect(border) {
            return Err(invalid_geometry().into());
        }
        let contains_x = |x| label.x0 <= x && x <= label.x1;
        let contains_y = |y| label.y0 <= y && y <= label.y1;
        let cuts_h = label.x0.max(border.x0) < label.x1.min(border.x1)
            && (contains_y(border.y0) || contains_y(border.y1));
        let cuts_v = label.y0.max(border.y0) < label.y1.min(border.y1)
            && (contains_x(border.x0) || contains_x(border.x1));
        if cuts_h || cuts_v {
            return Ok(false);
        }
    }
    for &foreign in obstacles.foreign_segments.into_iter().flatten() {
        work.charge(1)?;
        if crosses_rect(foreign, label, true) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rounded_center_key(center: f64) -> u64 {
    let scaled = center * 1e6;
    let lower = scaled.floor();
    let rounded = if scaled - lower < 0.5 {
        lower
    } else {
        lower + 1.0
    } / 1e6;
    // JavaScript Set treats both signed zeros as one key.
    if rounded == 0.0 { 0 } else { rounded.to_bits() }
}

#[derive(Default)]
struct RankedCoordinates {
    values: Vec<f64>,
    seen: HashSet<u64>,
}

impl RankedCoordinates {
    fn push(&mut self, coordinate: f64, work: &mut LabelSearchWork<'_>) -> SearchResult<()> {
        work.charge(1)?;
        if coordinate.is_finite() {
            self.seen
                .try_reserve(1)
                .map_err(|_| super::allocation_failed())?;
            if self.seen.insert(rounded_center_key(coordinate)) {
                self.values
                    .try_reserve(1)
                    .map_err(|_| super::allocation_failed())?;
                self.values.push(coordinate);
            }
        }
        Ok(())
    }

    fn into_ranked(
        mut self,
        anchor: f64,
        limit: usize,
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Vec<f64>> {
        stable_rank(&mut self.values, work, |a, b| {
            let delta = (*a - anchor).abs() - (*b - anchor).abs();
            if delta.abs() > POINT_EPSILON {
                delta < 0.0
            } else {
                a < b
            }
        })?;
        self.values.truncate(limit);
        Ok(self.values)
    }
}

fn stable_rank<T>(
    items: &mut [T],
    work: &mut LabelSearchWork<'_>,
    before: impl Fn(&T, &T) -> bool,
) -> SearchResult<()> {
    // The upstream epsilon ties do not define a total order. Keep insertion stable and meter
    // every comparison so a long candidate list can neither panic nor run outside its budget.
    for index in 1..items.len() {
        let mut current = index;
        while current > 0 {
            work.charge(1)?;
            if !before(&items[current], &items[current - 1]) {
                break;
            }
            items.swap(current, current - 1);
            current -= 1;
        }
    }
    Ok(())
}
