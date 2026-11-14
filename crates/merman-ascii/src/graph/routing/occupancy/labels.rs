use super::{OccupiedRect, RouteBounds, SceneOccupancy};
use crate::error::{AsciiError, Result};
use crate::graph::layout::CanvasCoord;
use crate::graph::routing::PreparedRoute;
use crate::graph::routing::label::RoutedLabelPlacement;
use crate::graph::routing::layout_allocation_failed;
use crate::graph::routing::plan::{LabelAnchor, PlannedRouteSegment, RoutePlan};
use crate::resource::ResourceContext;

#[cfg(test)]
pub(in crate::graph::routing) fn allocate_route_label_placements(
    routes: &mut [PreparedRoute],
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let label_count = routes.iter().try_fold(0usize, |count, route| {
        resources.checked_work_add(count, route.plan.labels.len())
    })?;
    if label_count == 0 {
        return Ok(());
    }

    for (route_index, route) in routes.iter_mut().enumerate() {
        allocate_committed_route_labels(route, route_index, occupancy, resources, diagram_type)?;
    }
    Ok(())
}

pub(in crate::graph::routing) fn allocate_committed_route_labels(
    route: &mut PreparedRoute,
    route_index: usize,
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let lane_radius = occupancy.label_lane_radius;
    let label_len = route.plan.labels.len();
    for label_index in 0..label_len {
        let (original, line_count, unresolved_anchor) = {
            let label = &route.plan.labels[label_index];
            (label.placement, label.line_count(), label.anchor)
        };
        let original_rect = OccupiedRect::try_new(
            original.x(),
            original.y(),
            original.width(),
            line_count,
            resources,
        )?;
        let anchor =
            resolve_label_anchor(&route.plan, unresolved_anchor, original_rect, resources)?;
        route.plan.labels[label_index].anchor = anchor;
        let candidates = route_label_candidates(
            original,
            line_count,
            anchor,
            occupancy.route_bounds[route_index],
            resources,
            lane_radius,
        )?;
        let mut selected = None;
        for candidate in candidates {
            let candidate_rect = OccupiedRect::try_new(
                candidate.x(),
                candidate.y(),
                candidate.width(),
                line_count,
                resources,
            )?;
            if occupancy.label_candidate_is_clear(
                route_index,
                &route.owner,
                anchor,
                candidate_rect,
                resources,
            )? && let Some(reservation) =
                label_reservation(anchor, candidate_rect, lane_radius, resources)?
            {
                selected = Some((candidate, reservation));
                break;
            }
        }

        let Some((placement, footprint)) = selected else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "edge label placement exhausted after bounded route-local search",
            });
        };
        route.plan.labels[label_index].placement = placement;
        occupancy.occupy_label(footprint, resources)?;
    }
    Ok(())
}

pub(super) fn resolve_label_anchor(
    plan: &RoutePlan,
    anchor: LabelAnchor,
    original: OccupiedRect,
    resources: &mut ResourceContext,
) -> Result<LabelAnchor> {
    let LabelAnchor::PlacementHint(hint) = anchor else {
        return Ok(anchor);
    };

    let mut selected = None;
    for (_, cell) in plan.active_cells() {
        resources.charge_layout_work(1)?;
        let distance = original.point_distance(cell.coord, resources)?;
        let hint_distance = resources
            .checked_work_add(cell.coord.x.abs_diff(hint.x), cell.coord.y.abs_diff(hint.y))?;
        let key = (
            distance,
            hint_distance,
            cell.coord.y,
            cell.coord.x,
            planned_segment_order(cell.segment),
        );
        if selected.is_none_or(|(best_key, _)| key < best_key) {
            selected = Some((key, *cell));
        }
    }

    let Some((_, cell)) = selected else {
        return Ok(LabelAnchor::Segment {
            start: hint,
            end: hint,
            route_segment: None,
        });
    };
    Ok(LabelAnchor::Segment {
        start: cell.coord,
        end: cell.coord,
        route_segment: Some(cell.segment),
    })
}

const fn planned_segment_order(segment: PlannedRouteSegment) -> u8 {
    match segment {
        PlannedRouteSegment::Direct => 0,
        PlannedRouteSegment::Boundary => 1,
    }
}

pub(super) fn label_anchor_contains(
    anchor: LabelAnchor,
    coord: CanvasCoord,
    route_segment: PlannedRouteSegment,
) -> bool {
    let LabelAnchor::Segment {
        start,
        end,
        route_segment: expected_segment,
    } = anchor
    else {
        return false;
    };
    if expected_segment.is_some_and(|expected| expected != route_segment) {
        return false;
    }
    if start.y == end.y {
        return coord.y == start.y
            && coord.x >= start.x.min(end.x)
            && coord.x <= start.x.max(end.x);
    }
    if start.x == end.x {
        return coord.x == start.x
            && coord.y >= start.y.min(end.y)
            && coord.y <= start.y.max(end.y);
    }
    coord == start || coord == end
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelAnchorAxis {
    Horizontal(HorizontalLabelHost),
    Vertical(VerticalLabelHost),
    Point(CanvasCoord),
}

/// Reserve the text and its perpendicular gap to the host together. A later route must
/// not pass between them and make the text appear to identify that unrelated route.
pub(super) fn label_reservation(
    anchor: LabelAnchor,
    text: OccupiedRect,
    lane_radius: usize,
    resources: &ResourceContext,
) -> Result<Option<OccupiedRect>> {
    let axis = match LabelAnchorAxis::from_anchor(anchor) {
        LabelAnchorAxis::Point(point) if text.x <= point.x && point.x < text.right => {
            LabelAnchorAxis::Horizontal(HorizontalLabelHost {
                min_x: point.x,
                max_x: point.x,
                y: point.y,
            })
        }
        LabelAnchorAxis::Point(point) if text.y <= point.y && point.y < text.bottom => {
            LabelAnchorAxis::Vertical(VerticalLabelHost {
                x: point.x,
                min_y: point.y,
                max_y: point.y,
            })
        }
        axis => axis,
    };
    let (x, y, right, bottom, gap) = match axis {
        LabelAnchorAxis::Horizontal(host) if text.x <= host.max_x && host.min_x < text.right => {
            let host_end = resources.checked_grid_add(host.y, 1)?;
            (
                text.x,
                text.y.min(host_end),
                text.right,
                text.bottom.max(host.y),
                text.y
                    .saturating_sub(host_end)
                    .max(host.y.saturating_sub(text.bottom)),
            )
        }
        LabelAnchorAxis::Vertical(host) if text.y <= host.max_y && host.min_y < text.bottom => {
            let host_end = resources.checked_grid_add(host.x, 1)?;
            (
                text.x.min(host_end),
                text.y,
                text.right.max(host.x),
                text.bottom,
                text.x
                    .saturating_sub(host_end)
                    .max(host.x.saturating_sub(text.right)),
            )
        }
        _ => return Ok(None),
    };
    if gap > lane_radius {
        return Ok(None);
    }
    OccupiedRect::try_new(x, y, right - x, bottom - y, resources).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HorizontalLabelHost {
    min_x: usize,
    max_x: usize,
    y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerticalLabelHost {
    x: usize,
    min_y: usize,
    max_y: usize,
}

impl LabelAnchorAxis {
    fn from_anchor(anchor: LabelAnchor) -> Self {
        match anchor {
            LabelAnchor::Segment { start, end, .. } if start.y == end.y && start.x != end.x => {
                Self::Horizontal(HorizontalLabelHost {
                    min_x: start.x.min(end.x),
                    max_x: start.x.max(end.x),
                    y: start.y,
                })
            }
            LabelAnchor::Segment { start, end, .. } if start.x == end.x && start.y != end.y => {
                Self::Vertical(VerticalLabelHost {
                    x: start.x,
                    min_y: start.y.min(end.y),
                    max_y: start.y.max(end.y),
                })
            }
            LabelAnchor::Segment { start, .. } | LabelAnchor::PlacementHint(start) => {
                Self::Point(start)
            }
        }
    }
}

pub(super) fn route_label_candidates(
    original: RoutedLabelPlacement,
    line_count: usize,
    anchor: LabelAnchor,
    route_bounds: Option<RouteBounds>,
    resources: &mut ResourceContext,
    lane_radius: usize,
) -> Result<Vec<RoutedLabelPlacement>> {
    const CANDIDATE_CAPACITY: usize = 64;
    let mut candidates = Vec::new();
    candidates
        .try_reserve(CANDIDATE_CAPACITY)
        .map_err(|_| layout_allocation_failed())?;
    push_label_candidate(
        &mut candidates,
        original,
        original.x(),
        original.y(),
        resources,
    )?;

    match LabelAnchorAxis::from_anchor(anchor) {
        LabelAnchorAxis::Horizontal(host) => {
            push_horizontal_host_candidates(
                &mut candidates,
                original,
                line_count,
                host,
                resources,
                lane_radius,
            )?;
        }
        LabelAnchorAxis::Vertical(host) => {
            push_vertical_host_candidates(
                &mut candidates,
                original,
                line_count,
                host,
                resources,
                lane_radius,
            )?;
        }
        LabelAnchorAxis::Point(point) => {
            push_point_host_candidates(
                &mut candidates,
                original,
                line_count,
                point,
                route_bounds,
                resources,
                lane_radius,
            )?;
        }
    }
    Ok(candidates)
}

fn push_horizontal_host_candidates(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    line_count: usize,
    host: HorizontalLabelHost,
    resources: &mut ResourceContext,
    lane_radius: usize,
) -> Result<()> {
    let host_end = resources.checked_grid_add(host.max_x, 1)?;
    let midpoint = host.min_x + (host.max_x - host.min_x) / 2;
    let x_candidates = [
        Some(original.x()),
        midpoint.checked_sub(original.width() / 2),
        Some(host.min_x),
        host_end.checked_sub(original.width().max(1)),
        Some(host.max_x),
        host.min_x.checked_sub(original.width().saturating_sub(1)),
    ];
    for x in x_candidates.into_iter().flatten() {
        push_label_candidate(candidates, original, x, original.y(), resources)?;
        for gap in 0..=lane_radius {
            let above_clearance = resources.checked_grid_add(line_count.max(1), gap)?;
            if let Some(above) = host.y.checked_sub(above_clearance) {
                push_label_candidate(candidates, original, x, above, resources)?;
            }
            let below = resources.checked_grid_add(resources.checked_grid_add(host.y, 1)?, gap)?;
            push_label_candidate(candidates, original, x, below, resources)?;
        }
    }
    Ok(())
}

fn push_vertical_host_candidates(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    line_count: usize,
    host: VerticalLabelHost,
    resources: &mut ResourceContext,
    lane_radius: usize,
) -> Result<()> {
    let host_end = resources.checked_grid_add(host.max_y, 1)?;
    let midpoint = host.min_y + (host.max_y - host.min_y) / 2;
    let y_candidates = [
        Some(original.y()),
        midpoint.checked_sub(line_count / 2),
        Some(host.min_y),
        host_end.checked_sub(line_count.max(1)),
        Some(host.max_y),
        host.min_y.checked_sub(line_count.saturating_sub(1)),
    ];
    for y in y_candidates.into_iter().flatten() {
        push_label_candidate(candidates, original, original.x(), y, resources)?;
        for gap in 0..=lane_radius {
            let left_clearance = resources.checked_grid_add(original.width().max(1), gap)?;
            if let Some(left) = host.x.checked_sub(left_clearance) {
                push_label_candidate(candidates, original, left, y, resources)?;
            }
            let right = resources.checked_grid_add(resources.checked_grid_add(host.x, 1)?, gap)?;
            push_label_candidate(candidates, original, right, y, resources)?;
        }
    }
    Ok(())
}

fn push_point_host_candidates(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    line_count: usize,
    point: CanvasCoord,
    route_bounds: Option<RouteBounds>,
    resources: &mut ResourceContext,
    lane_radius: usize,
) -> Result<()> {
    let centered_x = point.x.checked_sub(original.width() / 2);
    let centered_y = point.y.checked_sub(line_count.saturating_sub(1) / 2);
    let prefer_vertical_lanes = route_bounds.is_none_or(RouteBounds::prefers_vertical_label_lanes);
    for gap in 0..=lane_radius {
        let left_clearance = resources.checked_grid_add(original.width().max(1), gap)?;
        let above_clearance = resources.checked_grid_add(line_count.max(1), gap)?;
        let left = point.x.checked_sub(left_clearance);
        let right = Some(resources.checked_grid_add(resources.checked_grid_add(point.x, 1)?, gap)?);
        let above = point.y.checked_sub(above_clearance);
        let below = Some(resources.checked_grid_add(resources.checked_grid_add(point.y, 1)?, gap)?);

        let vertical = [
            (centered_x, above),
            (centered_x, below),
            (Some(original.x()), above),
            (Some(original.x()), below),
        ];
        let horizontal = [
            (left, centered_y),
            (right, centered_y),
            (left, Some(original.y())),
            (right, Some(original.y())),
        ];
        let ordered = if prefer_vertical_lanes {
            [vertical, horizontal]
        } else {
            [horizontal, vertical]
        };
        for lane in ordered {
            for (candidate_x, candidate_y) in lane {
                if let (Some(candidate_x), Some(candidate_y)) = (candidate_x, candidate_y) {
                    push_label_candidate(
                        candidates,
                        original,
                        candidate_x,
                        candidate_y,
                        resources,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn push_label_candidate(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    x: usize,
    y: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(candidates.len())?;
    let candidate = original.with_position(x, y);
    if !candidates.contains(&candidate) {
        candidates
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        candidates.push(candidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;
    use merman_model::resources::ResourceProfile;

    #[test]
    fn vertical_host_candidates_align_the_full_multiline_block() {
        let mut resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let candidates = route_label_candidates(
            RoutedLabelPlacement::new(21, 14, 5),
            5,
            LabelAnchor::Segment {
                start: CanvasCoord { x: 20, y: 10 },
                end: CanvasCoord { x: 20, y: 20 },
                route_segment: None,
            },
            None,
            &mut resources,
            4,
        )
        .unwrap();
        for y in [10, 13, 16] {
            assert!(candidates.contains(&RoutedLabelPlacement::new(21, y, 5)));
        }
    }
}
