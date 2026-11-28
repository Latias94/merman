//! Provisional foreign-edge repair from Mermaid 12.4 `grid/edgeLabels.ts`.

use super::{
    GUTTER, GUTTER_STEP, GUTTER_STEPS, GridSegment, LABEL_CLEARANCE, LabelRouteProposal,
    LabelSearchWork, MAX_LANES, POINT_EPSILON, RankedCoordinates, SearchResult, crosses_rect,
    finite_rect, invalid_geometry, label_segments, length, rect_axes, reserve_vec,
    segment_contains_anchor,
};
use crate::graph_layout::Axis;
use kurbo::{Point, Rect};

const MAX_REPAIRS: usize = 8;

pub(in crate::graph_layout::mermaid_grid) struct ProtectedGeometry<'a> {
    pub solids: &'a [Rect],
    pub group_borders: &'a [Rect],
    pub own_reservation: Option<(usize, Point)>,
}

pub(in crate::graph_layout::mermaid_grid) fn crosses_label(
    segments: &[GridSegment],
    rect: Rect,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<bool> {
    for &segment in segments {
        work.charge(1)?;
        if crosses_rect(segment, rect, true) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Repair one blocked rectangle while protecting the original terminal rays and any own anchor.
pub(in crate::graph_layout::mermaid_grid) fn reroute_around_label(
    original: &[GridSegment],
    terminals: [Point; 2],
    blocked: Rect,
    protected: ProtectedGeometry<'_>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<Option<LabelRouteProposal>> {
    let (Some(first), Some(last)) = (original.first(), original.last()) else {
        return Ok(None);
    };
    if [first.start, last.end] != terminals || !continuous(original, work)? {
        return Ok(None);
    }

    let mut proposed: Option<Vec<GridSegment>> = None;
    for _ in 0..MAX_REPAIRS {
        let active = proposed.as_deref().unwrap_or(original);
        let intersections = measure_intersections(active, blocked, work)?;
        let Some(index) = intersections.first else {
            break;
        };
        let Some(repaired) =
            repair_segment(active, index, intersections, blocked, &protected, work)?
        else {
            return Ok(None);
        };
        proposed = Some(repaired);
    }

    let Some(segments) = proposed else {
        return Ok(None);
    };
    if crosses_label(&segments, blocked, work)? {
        return Ok(None);
    }
    LabelRouteProposal::materialize(terminals[0], segments, work).map(Some)
}

#[derive(Default)]
struct Intersections {
    first: Option<usize>,
    count: usize,
    length: f64,
}

fn measure_intersections(
    segments: &[GridSegment],
    rect: Rect,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<Intersections> {
    let mut hits = Intersections::default();
    for (index, &segment) in segments.iter().enumerate() {
        work.charge(1)?;
        if crosses_rect(segment, rect, true) {
            hits.first.get_or_insert(index);
            hits.count += 1;
            let (low, high) = segment.span();
            let (along, _) = rect_axes(rect, segment.axis);
            hits.length += high.min(along.high) - low.max(along.low);
        }
    }
    Ok(hits)
}

fn repair_segment(
    segments: &[GridSegment],
    index: usize,
    blocked_before: Intersections,
    blocked: Rect,
    protected: &ProtectedGeometry<'_>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<Option<Vec<GridSegment>>> {
    let leg = segments[index];
    let (low, high) = leg.span();
    let (along, _) = rect_axes(blocked, leg.axis);
    let entry = low.max(along.low - LABEL_CLEARANCE);
    let exit = high.min(along.high + LABEL_CLEARANCE);
    if exit - entry <= POINT_EPSILON {
        return Ok(None);
    }

    let candidate_len = segments
        .len()
        .checked_add(5)
        .ok_or_else(|| work.meter.arithmetic_overflow())?;
    let mut points = Vec::new();
    for lane in lanes(leg, blocked, protected, work)? {
        work.charge(candidate_len)?;
        if points.capacity() < candidate_len {
            points = reserve_vec(candidate_len)?;
        }
        points.clear();
        points.push(segments[0].start);
        points.extend(segments[..index].iter().map(|segment| segment.end));
        let point = |along, cross| match leg.axis {
            Axis::Horizontal => Point::new(along, cross),
            Axis::Vertical => Point::new(cross, along),
        };
        // Mermaid inserts the four corners in ascending along-coordinate order.
        points.extend([
            point(entry, leg.fixed()),
            point(entry, lane),
            point(exit, lane),
            point(exit, leg.fixed()),
        ]);
        points.extend(segments[index..].iter().map(|segment| segment.end));

        let proposal = label_segments(&points, work.meter)?;
        let (Some(first), Some(last)) = (proposal.first(), proposal.last()) else {
            continue;
        };
        if !same_ray(segments[0], *first, true)
            || !same_ray(segments[segments.len() - 1], *last, false)
            || !continuous(&proposal, work)?
        {
            continue;
        }
        let blocked_after = measure_intersections(&proposal, blocked, work)?;
        let improved = blocked_after.count < blocked_before.count
            || (blocked_after.count == blocked_before.count
                && blocked_after.length < blocked_before.length - POINT_EPSILON);
        if improved && respects_protection(&proposal, protected, work)? {
            return Ok(Some(proposal));
        }
    }
    Ok(None)
}

pub(super) fn same_ray(original: GridSegment, candidate: GridSegment, start: bool) -> bool {
    let along = |point: Point| match original.axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    };
    let terminal = |segment: GridSegment| if start { segment.start } else { segment.end };
    original.axis == candidate.axis
        && terminal(original) == terminal(candidate)
        && length(candidate) > POINT_EPSILON
        && (along(original.end) > along(original.start))
            == (along(candidate.end) > along(candidate.start))
}

pub(super) fn continuous(
    segments: &[GridSegment],
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<bool> {
    for pair in segments.windows(2) {
        work.charge(1)?;
        if pair[0].end != pair[1].start {
            return Ok(false);
        }
    }
    Ok(true)
}

fn lanes(
    segment: GridSegment,
    blocked: Rect,
    protected: &ProtectedGeometry<'_>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<Vec<f64>> {
    let (_, cross) = rect_axes(blocked, segment.axis);
    let (low, high) = segment.span();
    let mut lanes = RankedCoordinates::default();
    for step in 0..GUTTER_STEPS {
        let offset = GUTTER + f64::from(step) * GUTTER_STEP;
        lanes.push(cross.low - offset, work)?;
        lanes.push(cross.high + offset, work)?;
    }
    for (slot, &rect) in protected.solids.iter().enumerate() {
        work.charge(1)?;
        if protected
            .own_reservation
            .is_some_and(|(own, _)| own == slot)
        {
            continue;
        }
        if !finite_rect(rect) {
            return Err(invalid_geometry().into());
        }
        let (along, cross) = rect_axes(rect, segment.axis);
        if along.high >= low - LABEL_CLEARANCE && along.low <= high + LABEL_CLEARANCE {
            lanes.push(cross.low - GUTTER, work)?;
            lanes.push(cross.high + GUTTER, work)?;
        }
    }
    lanes.into_ranked(segment.fixed(), MAX_LANES, work)
}

pub(super) fn respects_protection(
    segments: &[GridSegment],
    protected: &ProtectedGeometry<'_>,
    work: &mut LabelSearchWork<'_>,
) -> SearchResult<bool> {
    let mut anchored = protected.own_reservation.is_none();
    for &leg in segments {
        work.charge(1)?;
        if let Some((_, center)) = protected.own_reservation {
            anchored |= segment_contains_anchor(leg, center);
        }
        for (slot, &rect) in protected.solids.iter().enumerate() {
            work.charge(1)?;
            if protected
                .own_reservation
                .is_some_and(|(own, _)| own == slot)
            {
                continue;
            }
            if crosses_rect(leg, rect, true) {
                return Ok(false);
            }
        }
        // A sibling route stays on its side of every group perimeter, including ancestors.
        for &group in protected.group_borders {
            work.charge(1)?;
            if touches_border(leg, group) {
                return Ok(false);
            }
        }
    }
    Ok(anchored)
}

fn touches_border(leg: GridSegment, border: Rect) -> bool {
    let (along, cross) = rect_axes(border, leg.axis);
    let (low, high) = leg.span();
    let fixed = leg.fixed();
    if fixed == cross.low || fixed == cross.high {
        low.max(along.low) <= high.min(along.high)
    } else {
        fixed > cross.low
            && fixed < cross.high
            && ((low..=high).contains(&along.low) || (low..=high).contains(&along.high))
    }
}
