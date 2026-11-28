//! Ordinary owner detours for a short, forward Grid label segment.

use super::foreign::{ProtectedGeometry, continuous, respects_protection, same_ray};
use super::{
    GUTTER, GUTTER_STEP, GUTTER_STEPS, GridLabelMarkers, GridSegment, LABEL_CLEARANCE,
    LabelCollisionView, LabelRouteProposal, LabelSearchWork, MARKER_LENGTH, MAX_LANES,
    POINT_EPSILON, RankedCoordinates, SearchResult, finite_rect, invalid_geometry, label_is_clear,
    label_rect_at, label_segments, marker_rect, measured_label_size, rect_axes, reserve_vec,
    segment_contains_anchor,
};
use crate::graph_layout::{Axis, Size};
use kurbo::{Point, Rect};

const MAX_CENTERS: usize = 64;
const MAX_POINTS: usize = 10;

/// A one-leg route that needs the short-span center fallback of `detourCenterCandidates`.
pub(in crate::graph_layout::mermaid_grid) struct ShortDetour {
    leg: GridSegment,
    size: Size,
    along_half: f64,
    cross_half: f64,
}

impl ShortDetour {
    pub(in crate::graph_layout::mermaid_grid) fn new(
        segments: &[GridSegment],
        terminals: [Point; 2],
        size: Size,
    ) -> Option<Self> {
        let [segment] = segments else { return None };
        if [segment.start, segment.end] != terminals || !measured_label_size(size) {
            return None;
        }
        let leg = GridSegment::new(terminals[0], terminals[1])?;
        let (along, cross, start, end) = match leg.axis {
            Axis::Horizontal => (size.width, size.height, leg.start.x, leg.end.x),
            Axis::Vertical => (size.height, size.width, leg.start.y, leg.end.y),
        };
        let inset = along / 2.0 + LABEL_CLEARANCE;
        (start < end && (end - inset) - (start + inset) <= POINT_EPSILON).then_some(Self {
            leg,
            size,
            along_half: along / 2.0,
            cross_half: cross / 2.0,
        })
    }

    /// Return a local proposal after checking the complete label box and the repaired shaft.
    pub(in crate::graph_layout::mermaid_grid) fn propose(
        &self,
        markers: GridLabelMarkers,
        obstacles: LabelCollisionView<'_>,
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Option<(Point, LabelRouteProposal)>> {
        let leg = self.leg;
        let clearance = [
            markers
                .start
                .then(|| marker_rect(leg.start, leg.end))
                .flatten(),
            markers
                .end
                .then(|| marker_rect(leg.end, leg.start))
                .flatten(),
        ];
        let protected = ProtectedGeometry {
            solids: obstacles.solids,
            group_borders: obstacles.group_borders,
            own_reservation: None,
        };
        let lanes = self.lanes(obstacles.solids, &clearance, work)?;
        work.charge(MAX_POINTS)?;
        let mut points = reserve_vec(MAX_POINTS)?;
        for lane in lanes {
            for along in self.centers(lane, obstacles.solids, work)? {
                work.charge(MAX_POINTS)?;
                self.build_points(along, lane, &mut points);
                let segments = label_segments(&points, work.meter)?;
                let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
                    continue;
                };
                if !same_ray(leg, *first, true)
                    || !same_ray(leg, *last, false)
                    || !continuous(&segments, work)?
                    || !respects_protection(&segments, &protected, work)?
                {
                    continue;
                }

                let center = self.point(along, lane);
                let rect = label_rect_at(center, self.size);
                if !finite_rect(rect) {
                    return Err(invalid_geometry().into());
                }
                let solids = obstacles
                    .solids
                    .iter()
                    .chain(clearance.iter().flatten())
                    .copied();
                work.charge(segments.len())?;
                if segments
                    .iter()
                    .any(|&segment| segment_contains_anchor(segment, center))
                    && label_is_clear(rect, obstacles, solids, work)?
                {
                    let route = LabelRouteProposal::materialize(leg.start, segments, work)?;
                    return Ok(Some((center, route)));
                }
            }
        }
        Ok(None)
    }

    fn point(&self, along: f64, cross: f64) -> Point {
        match self.leg.axis {
            Axis::Horizontal => Point::new(along, cross),
            Axis::Vertical => Point::new(cross, along),
        }
    }

    fn lanes(
        &self,
        solids: &[Rect],
        clearance: &[Option<Rect>; 2],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Vec<f64>> {
        let fixed = self.leg.fixed();
        let (low, high) = self.leg.span();
        let padding = (self.along_half * 2.0) + LABEL_CLEARANCE * 2.0 + GUTTER;
        let mut lanes = RankedCoordinates::default();
        for step in 0..GUTTER_STEPS {
            let offset = self.cross_half + GUTTER + f64::from(step) * GUTTER_STEP;
            lanes.push(fixed - offset, work)?;
            lanes.push(fixed + offset, work)?;
        }
        for &rect in solids {
            work.charge(1)?;
            if !finite_rect(rect) {
                return Err(invalid_geometry().into());
            }
            let (along, cross) = rect_axes(rect, self.leg.axis);
            if along.high >= low - padding && along.low <= high + padding {
                lanes.push(cross.low - self.cross_half - GUTTER, work)?;
                lanes.push(cross.high + self.cross_half + GUTTER, work)?;
            }
        }
        for &rect in clearance.iter().flatten() {
            work.charge(1)?;
            let (_, cross) = rect_axes(rect, self.leg.axis);
            lanes.push(cross.low - self.cross_half - GUTTER, work)?;
            lanes.push(cross.high + self.cross_half + GUTTER, work)?;
        }
        lanes.into_ranked(fixed, MAX_LANES, work)
    }

    fn centers(
        &self,
        lane: f64,
        solids: &[Rect],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Vec<f64>> {
        let (low, high) = self.leg.span();
        let preferred = (low + high) / 2.0;
        if !preferred.is_finite() {
            return Err(invalid_geometry().into());
        }
        let mut centers = RankedCoordinates::default();
        centers.push(preferred, work)?;
        for &rect in solids {
            work.charge(1)?;
            let (along, cross) = rect_axes(rect, self.leg.axis);
            if along.high >= low - self.along_half
                && along.low <= high + self.along_half
                && cross.high >= lane - self.cross_half
                && cross.low <= lane + self.cross_half
            {
                centers.push(along.low - GUTTER - self.along_half, work)?;
                centers.push(along.high + GUTTER + self.along_half, work)?;
            }
        }
        centers.into_ranked(preferred, MAX_CENTERS, work)
    }

    fn build_points(&self, along: f64, lane: f64, points: &mut Vec<Point>) {
        let (start, end) = self.leg.span();
        let fixed = self.leg.fixed();
        let half_span = (self.along_half * 2.0 + LABEL_CLEARANCE * 2.0) / 2.0;
        let from = along - half_span;
        let to = along + half_span;
        let inset = ((end - start) / 2.0 - POINT_EPSILON).clamp(0.0, MARKER_LENGTH);
        let direction = if lane >= fixed { 1.0 } else { -1.0 };
        let return_line = lane + direction * (self.cross_half + LABEL_CLEARANCE);

        points.clear();
        points.push(self.leg.start);
        if from < start - POINT_EPSILON {
            let entry = start + inset;
            points.extend([
                self.point(entry, fixed),
                self.point(entry, return_line),
                self.point(from, return_line),
            ]);
        } else {
            points.push(self.point(from, fixed));
        }
        points.extend([self.point(from, lane), self.point(to, lane)]);
        if to > end + POINT_EPSILON {
            let exit = end - inset;
            points.extend([
                self.point(to, return_line),
                self.point(exit, return_line),
                self.point(exit, fixed),
            ]);
        } else {
            points.push(self.point(to, fixed));
        }
        points.push(self.leg.end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn short_detour_avoids_foreign_shafts_and_protected_group_borders() {
        let terminals = [Point::new(200.0, 160.0), Point::new(250.0, 160.0)];
        let leg = GridSegment::new(terminals[0], terminals[1]).unwrap();
        let detour = ShortDetour::new(
            &[leg],
            terminals,
            Size {
                width: 80.0,
                height: 20.0,
            },
        )
        .unwrap();
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let blockers =
            [GridSegment::new(Point::new(225.0, 132.0), Point::new(225.0, 142.0)).unwrap()];
        let mut obstacles = LabelCollisionView {
            solids: &[],
            group_borders: &[],
            foreign_segments: [&blockers, &[]],
        };
        let marks = GridLabelMarkers {
            start: false,
            end: true,
        };
        let mut search = LabelSearchWork::new(&meter, Some(10_000));
        let (center, route) = detour
            .propose(marks, obstacles, &mut search)
            .unwrap()
            .unwrap();
        assert_eq!(center, Point::new(225.0, 184.0));
        assert_eq!(route.points.first(), Some(&terminals[0]));
        assert_eq!(route.points.last(), Some(&terminals[1]));

        let borders = [Rect::new(100.0, 150.0, 350.0, 170.0)];
        obstacles.group_borders = &borders;
        assert!(
            detour
                .propose(marks, obstacles, &mut search)
                .unwrap()
                .is_none()
        );
    }
}
