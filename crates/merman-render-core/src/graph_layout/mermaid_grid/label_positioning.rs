//! Layout-owned label placement on the original Grid routes.

use super::forest::GridItemKind;
use super::geometry::reserve;
use super::prepared::{GridEdgeLabel, GridRoutedEdge, PreparedGrid};
use super::route_plan::{allocation_error, code_units};
use super::routing::GridSegment;
use super::routing::labels::detour::ShortDetour;
use super::routing::labels::foreign::{ProtectedGeometry, crosses_label, reroute_around_label};
use super::routing::labels::{
    GridLabelMarkers, LabelCollisionView, LabelRouteProposal, LabelSearchError, LabelSearchWork,
    SearchResult, label_fallback_anchor, label_rect_at, label_segments, measured_label_size,
    place_label_on_segments, visit_label_centers,
};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;
use std::collections::HashMap;
use std::ops::Range;

// Use the reference's default allowance with the work units of this flat scan.
const MAX_LABEL_SEARCH_WORK: usize = 2_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LabelPassOutcome {
    ExistingSegments,
    ForeignReroutes,
    OwnerDetour,
    SearchWorkLimit,
    ExistingSegmentsExhausted,
}

#[derive(Default)]
struct LabelSlot {
    segments: Range<usize>,
    center: Option<Point>,
    obstacle: Option<usize>,
}

struct PendingLabel {
    slot: usize,
    label: GridEdgeLabel,
}

impl PendingLabel {
    fn markers(&self) -> GridLabelMarkers {
        GridLabelMarkers {
            start: self.label.start_marker,
            end: self.label.end_marker,
        }
    }
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
        let mut slots = Vec::new();
        reserve(&mut slots, routes.len())?;
        slots.resize_with(routes.len(), LabelSlot::default);
        let mut segments = Vec::new();
        let mut pending = Vec::new();
        for (slot, (edge, route)) in self.edges.iter().zip(routes.iter()).enumerate() {
            work.charge(1)?;
            let Some(label) = edge.label.filter(|label| measured_label_size(label.size)) else {
                continue;
            };
            slots[slot].segments = append_segments(&mut segments, &route.points, work)?;
            if slots[slot].segments.is_empty() {
                let center = |item| {
                    self.item_bounds(item).map(|bounds| {
                        let center = bounds.center();
                        Point::new(center.x, center.y)
                    })
                };
                slots[slot].center = label_fallback_anchor(
                    &route.points,
                    &[],
                    center(edge.source),
                    center(edge.target),
                    work,
                )?;
            } else {
                pending.try_reserve(1).map_err(|_| allocation_error())?;
                pending.push(PendingLabel { slot, label });
            }
        }

        let mut edits = HashMap::new();
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
            for ((edge, route), slot) in self.edges.iter().zip(routes.iter()).zip(&mut slots) {
                work.charge(1)?;
                if let Some(label) = edge.label.filter(|label| measured_label_size(label.size)) {
                    if let Some(anchor) = slot.center {
                        // Parked labels are already obstacles when upstream builds the pass index.
                        slot.obstacle = Some(reserve_label(&mut solids, label, anchor)?);
                    }
                } else {
                    slot.segments = append_segments(&mut segments, &route.points, work)?;
                }
            }

            let mut pass = LabelPass {
                grid: self,
                routes,
                original_segments: &segments,
                solids,
                group_borders,
                edits: HashMap::new(),
            };
            match pass.position_with_retry(
                &pending,
                &mut slots,
                &mut LabelSearchWork::new(work, Some(limit)),
            ) {
                Ok(outcome) => {
                    if matches!(
                        outcome,
                        LabelPassOutcome::ForeignReroutes | LabelPassOutcome::OwnerDetour
                    ) {
                        edits = pass.edits;
                    }
                    outcome
                }
                Err(LabelSearchError::WorkLimit) => LabelPassOutcome::SearchWorkLimit,
                Err(LabelSearchError::Layout(error)) => return Err(error),
            }
        };

        if matches!(
            outcome,
            LabelPassOutcome::SearchWorkLimit | LabelPassOutcome::ExistingSegmentsExhausted
        ) {
            for label in pending {
                let route = &routes[label.slot];
                let slot = &mut slots[label.slot];
                slot.center = label_fallback_anchor(
                    &route.points,
                    &segments[slot.segments.clone()],
                    None,
                    None,
                    work,
                )?;
            }
        }

        // Admit the complete writeback before publishing route edits or centers.
        work.charge(routes.len())?;
        for (index, (route, slot)) in routes.iter_mut().zip(slots).enumerate() {
            if let Some(edit) = edits.remove(&index) {
                route.points = edit.points;
            }
            route.label_center = slot.center;
        }
        Ok(outcome)
    }
}

struct LabelPass<'a, 'input> {
    grid: &'a PreparedGrid<'input>,
    routes: &'a [GridRoutedEdge],
    original_segments: &'a [GridSegment],
    solids: Vec<Rect>,
    group_borders: Vec<Rect>,
    edits: HashMap<usize, LabelRouteProposal>,
}

impl LabelPass<'_, '_> {
    fn position_with_retry(
        &mut self,
        pending: &[PendingLabel],
        slots: &mut [LabelSlot],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<LabelPassOutcome> {
        if self.position(pending, slots, work)? {
            return Ok(self.existing_route_outcome());
        }

        // A failed single-owner attempt has no provisional edits to rewind. The parked
        // reservations and original routes can be reused for the second pass.
        let [owner] = pending else {
            return Ok(LabelPassOutcome::ExistingSegmentsExhausted);
        };
        if !self.is_sibling_leaf_route(owner.slot) {
            return Ok(LabelPassOutcome::ExistingSegmentsExhausted);
        }
        let [start, .., end] = self.routes[owner.slot].points.as_slice() else {
            return Ok(LabelPassOutcome::ExistingSegmentsExhausted);
        };
        let range = slots[owner.slot].segments.clone();
        let Some(detour) = ShortDetour::new(
            &self.original_segments[range.clone()],
            [*start, *end],
            owner.label.size,
        ) else {
            return Ok(LabelPassOutcome::ExistingSegmentsExhausted);
        };
        if self.position(pending, slots, work)? {
            return Ok(self.existing_route_outcome());
        }
        let obstacles = LabelCollisionView {
            solids: &self.solids,
            group_borders: &self.group_borders,
            foreign_segments: [
                &self.original_segments[..range.start],
                &self.original_segments[range.end..],
            ],
        };
        let Some((center, route)) = detour.propose(owner.markers(), obstacles, work)? else {
            return Ok(LabelPassOutcome::ExistingSegmentsExhausted);
        };
        self.edits.try_reserve(1).map_err(|_| allocation_error())?;
        self.edits.insert(owner.slot, route);
        slots[owner.slot].center = Some(center);
        Ok(LabelPassOutcome::OwnerDetour)
    }

    fn existing_route_outcome(&self) -> LabelPassOutcome {
        if self.edits.is_empty() {
            LabelPassOutcome::ExistingSegments
        } else {
            LabelPassOutcome::ForeignReroutes
        }
    }

    fn is_sibling_leaf_route(&self, slot: usize) -> bool {
        let edge = &self.grid.edges[slot];
        let forest = &self.grid.forest;
        edge.source != edge.target
            && forest.parents[edge.source] == forest.parents[edge.target]
            && matches!(forest.items[edge.source].kind, GridItemKind::Leaf(_))
            && matches!(forest.items[edge.target].kind, GridItemKind::Leaf(_))
    }

    fn position(
        &mut self,
        pending: &[PendingLabel],
        slots: &mut [LabelSlot],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<bool> {
        for owner in pending {
            let center = match self.place_on_existing_route(owner, slots, work)? {
                Some(center) => Some(center),
                None => self.place_with_foreign_reroutes(owner, slots, work)?,
            };
            let Some(center) = center else {
                return Ok(false);
            };
            let slot = &mut slots[owner.slot];
            slot.center = Some(center);
            slot.obstacle = Some(reserve_label(&mut self.solids, owner.label, center)?);
        }
        Ok(true)
    }

    fn segments(&self, slot: usize, geometry: &LabelSlot) -> &[GridSegment] {
        self.edits.get(&slot).map_or_else(
            || &self.original_segments[geometry.segments.clone()],
            |edit| edit.segments.as_slice(),
        )
    }

    fn place_on_existing_route(
        &self,
        owner: &PendingLabel,
        slots: &[LabelSlot],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Option<Point>> {
        let mut foreign = Vec::new();
        let foreign_segments = if self.edits.is_empty() {
            let range = &slots[owner.slot].segments;
            [
                &self.original_segments[..range.start],
                &self.original_segments[range.end..],
            ]
        } else {
            for (slot, geometry) in slots.iter().enumerate() {
                work.charge(1)?;
                if slot != owner.slot {
                    let segments = self.segments(slot, geometry);
                    work.charge(segments.len())?;
                    foreign
                        .try_reserve(segments.len())
                        .map_err(|_| allocation_error())?;
                    foreign.extend_from_slice(segments);
                }
            }
            [foreign.as_slice(), &[]]
        };
        place_label_on_segments(
            self.segments(owner.slot, &slots[owner.slot]),
            owner.label.size,
            owner.markers(),
            LabelCollisionView {
                solids: &self.solids,
                group_borders: &self.group_borders,
                foreign_segments,
            },
            work,
        )
    }

    fn place_with_foreign_reroutes(
        &mut self,
        owner: &PendingLabel,
        slots: &[LabelSlot],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Option<Point>> {
        let mut accepted = Vec::new();
        let anchor = visit_label_centers(
            self.segments(owner.slot, &slots[owner.slot]),
            owner.label.size,
            owner.markers(),
            LabelCollisionView {
                solids: &self.solids,
                group_borders: &self.group_borders,
                foreign_segments: [&[], &[]],
            },
            work,
            |rect, work| {
                if let Some(proposals) = self.propose_reroutes(owner.slot, rect, slots, work)? {
                    accepted = proposals;
                    return Ok(true);
                }
                Ok(false)
            },
        )?;
        self.edits
            .try_reserve(accepted.len())
            .map_err(|_| allocation_error())?;
        self.edits.extend(accepted);
        Ok(anchor)
    }

    fn propose_reroutes(
        &self,
        owner: usize,
        rect: Rect,
        slots: &[LabelSlot],
        work: &mut LabelSearchWork<'_>,
    ) -> SearchResult<Option<Vec<(usize, LabelRouteProposal)>>> {
        let mut blockers = Vec::new();
        let mut max_id_bytes = 0;
        for (slot, geometry) in slots.iter().enumerate() {
            work.charge(1)?;
            if slot != owner && crosses_label(self.segments(slot, geometry), rect, work)? {
                blockers.try_reserve(1).map_err(|_| allocation_error())?;
                let id = self.grid.edges[slot].id;
                max_id_bytes = max_id_bytes.max(id.len());
                blockers.push((slot, id));
            }
        }
        work.charge_sort(blockers.len(), max_id_bytes)?;
        blockers
            .sort_unstable_by(|(a, a_id), (b, b_id)| code_units(a_id, b_id).then_with(|| a.cmp(b)));

        let mut proposals = Vec::new();
        for (slot, _) in blockers {
            work.charge(1)?;
            if !self.is_sibling_leaf_route(slot) {
                return Ok(None);
            }
            let [start, .., end] = self.routes[slot].points.as_slice() else {
                return Ok(None);
            };
            let geometry = &slots[slot];
            let Some(edit) = reroute_around_label(
                self.segments(slot, geometry),
                [*start, *end],
                rect,
                ProtectedGeometry {
                    solids: &self.solids,
                    group_borders: &self.group_borders,
                    own_reservation: geometry.obstacle.zip(geometry.center),
                },
                work,
            )?
            else {
                return Ok(None);
            };
            proposals.try_reserve(1).map_err(|_| allocation_error())?;
            proposals.push((slot, edit));
        }
        Ok(Some(proposals))
    }
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

fn reserve_label(solids: &mut Vec<Rect>, label: GridEdgeLabel, anchor: Point) -> Result<usize> {
    let rect = label_rect_at(anchor, label.size);
    if !rect.is_finite() {
        return Err(Error::InvalidModel {
            message: "Grid label reservation exceeds the finite coordinate range".to_owned(),
        });
    }
    solids.try_reserve(1).map_err(|_| allocation_error())?;
    let index = solids.len();
    solids.push(rect);
    Ok(index)
}

#[cfg(test)]
mod tests;
