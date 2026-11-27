//! Layout-owned label placement on the original Grid routes.

use super::forest::GridItemKind;
use super::geometry::reserve;
use super::prepared::{GridEdgeLabel, GridRoutedEdge, PreparedGrid};
use super::route_plan::allocation_error;
use super::routing::GridSegment;
use super::routing::labels::{
    GridLabelMarkers, LabelCollisionView, LabelSearchError, LabelSearchWork, label_fallback_anchor,
    label_rect_at, label_segments, measured_label_size, place_label_on_segments,
};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;
use std::ops::Range;

// Use the reference's default allowance with the work units of this flat scan.
const MAX_LABEL_SEARCH_WORK: usize = 2_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LabelPassOutcome {
    ExistingSegments,
    SearchWorkLimit,
    ExistingSegmentsExhausted,
}

struct PendingLabel {
    slot: usize,
    label: GridEdgeLabel,
    segments: Range<usize>,
}

impl PreparedGrid<'_> {
    /// Computes label centers in original edge occurrence order after all routes are materialized.
    /// The routes use untrimmed drawing coordinates.
    pub(super) fn position_labels(
        &self,
        routes: &mut [GridRoutedEdge],
        work: &OperationWorkMeter,
    ) -> Result<()> {
        self.position_labels_with_limit(routes, MAX_LABEL_SEARCH_WORK, work)
            .map(|_| ())
    }

    fn position_labels_with_limit(
        &self,
        routes: &mut [GridRoutedEdge],
        limit: usize,
        work: &OperationWorkMeter,
    ) -> Result<LabelPassOutcome> {
        work.checkpoint(OperationPhase::Layout)?;
        if routes.len() != self.edges.len() {
            return Err(Error::InvalidModel {
                message: format!(
                    "Expected {} Grid route occurrences for label placement, got {}",
                    self.edges.len(),
                    routes.len()
                ),
            });
        }

        work.charge(routes.len())?;
        let mut anchors = Vec::new();
        reserve(&mut anchors, routes.len())?;
        anchors.resize(routes.len(), None);
        let mut segments = Vec::new();
        let mut pending = Vec::new();
        for (slot, (edge, route)) in self.edges.iter().zip(routes.iter()).enumerate() {
            work.charge(1)?;
            let Some(label) = edge.label.filter(|label| measured_label_size(label.size)) else {
                continue;
            };
            let range = append_segments(&mut segments, &route.points, work)?;
            if range.is_empty() {
                let center = |item| {
                    self.item_bounds(item).map(|bounds| {
                        let center = bounds.center();
                        Point::new(center.x, center.y)
                    })
                };
                anchors[slot] = label_fallback_anchor(
                    &route.points,
                    &[],
                    center(edge.source),
                    center(edge.target),
                    work,
                )?;
            } else {
                pending.try_reserve(1).map_err(|_| allocation_error())?;
                pending.push(PendingLabel {
                    slot,
                    label,
                    segments: range,
                });
            }
        }

        let outcome = if pending.is_empty() {
            LabelPassOutcome::ExistingSegments
        } else {
            let mut solids = Vec::new();
            let mut group_borders = Vec::new();
            for (slot, item) in self.forest.items.iter().enumerate() {
                work.charge(1)?;
                let (solid, border) = match item.kind {
                    GridItemKind::Leaf(_) => (self.item_bounds(slot), None),
                    GridItemKind::Group(_) => {
                        (self.group_title_bounds(slot), self.item_bounds(slot))
                    }
                    GridItemKind::EdgeLabel => continue,
                };
                for (bounds, obstacles) in [(solid, &mut solids), (border, &mut group_borders)] {
                    if let Some(bounds) = bounds {
                        obstacles.try_reserve(1).map_err(|_| allocation_error())?;
                        obstacles.push(Rect::new(
                            bounds.x,
                            bounds.y,
                            bounds.x + bounds.width,
                            bounds.y + bounds.height,
                        ));
                    }
                }
            }
            for ((edge, route), &anchor) in self.edges.iter().zip(routes.iter()).zip(&anchors) {
                work.charge(1)?;
                if let Some(label) = edge.label.filter(|label| measured_label_size(label.size)) {
                    if let Some(anchor) = anchor {
                        // Parked labels are already obstacles when upstream builds the pass index.
                        reserve_label(&mut solids, label, anchor)?;
                    }
                } else {
                    append_segments(&mut segments, &route.points, work)?;
                }
            }
            search_existing_segments(
                &pending,
                &segments,
                &mut solids,
                &group_borders,
                &mut anchors,
                &mut LabelSearchWork::new(work, Some(limit)),
            )?
        };

        if outcome != LabelPassOutcome::ExistingSegments {
            for label in pending {
                let route = &routes[label.slot];
                anchors[label.slot] = label_fallback_anchor(
                    &route.points,
                    &segments[label.segments],
                    None,
                    None,
                    work,
                )?;
            }
        }

        // Admit the complete writeback before replacing any previous centers.
        work.charge(routes.len())?;
        for (route, anchor) in routes.iter_mut().zip(anchors) {
            route.label_center = anchor;
        }
        Ok(outcome)
    }
}

fn search_existing_segments(
    pending: &[PendingLabel],
    segments: &[GridSegment],
    solids: &mut Vec<Rect>,
    group_borders: &[Rect],
    anchors: &mut [Option<Point>],
    work: &mut LabelSearchWork<'_>,
) -> Result<LabelPassOutcome> {
    for owner in pending {
        let result = place_label_on_segments(
            &segments[owner.segments.clone()],
            owner.label.size,
            GridLabelMarkers {
                start: owner.label.start_marker,
                end: owner.label.end_marker,
            },
            LabelCollisionView {
                solids,
                group_borders,
                foreign_segments: [
                    &segments[..owner.segments.start],
                    &segments[owner.segments.end..],
                ],
            },
            work,
        );
        match result {
            Ok(Some(anchor)) => {
                anchors[owner.slot] = Some(anchor);
                reserve_label(solids, owner.label, anchor)?;
            }
            Ok(None) => return Ok(LabelPassOutcome::ExistingSegmentsExhausted),
            Err(LabelSearchError::WorkLimit) => return Ok(LabelPassOutcome::SearchWorkLimit),
            Err(LabelSearchError::Layout(error)) => return Err(error),
        }
    }
    Ok(LabelPassOutcome::ExistingSegments)
}

fn append_segments(
    segments: &mut Vec<GridSegment>,
    points: &[Point],
    work: &OperationWorkMeter,
) -> Result<Range<usize>> {
    let route_segments = label_segments(points, work)?;
    work.charge(route_segments.len())?;
    segments
        .try_reserve(route_segments.len())
        .map_err(|_| allocation_error())?;
    let start = segments.len();
    segments.extend(route_segments);
    Ok(start..segments.len())
}

fn reserve_label(solids: &mut Vec<Rect>, label: GridEdgeLabel, anchor: Point) -> Result<()> {
    let rect = label_rect_at(anchor, label.size);
    if !rect.is_finite() {
        return Err(Error::InvalidModel {
            message: "Grid label reservation exceeds the finite coordinate range".to_owned(),
        });
    }
    solids.try_reserve(1).map_err(|_| allocation_error())?;
    solids.push(rect);
    Ok(())
}

#[cfg(test)]
mod tests;
