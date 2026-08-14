use super::super::label::GRAPH_LABEL_LINE_GAP;
use super::super::layout::{CanvasCoord, GraphLayout, GroupLayout};
use super::super::model::GraphGroupKind;
use super::path::StepDirection;
use super::plan::{
    LabelAnchor, MAX_MARKER_CANDIDATES, MarkerCandidate, MarkerEndpoint, PlannedCellId,
    PlannedRouteSegment, RoutePlan,
};
use super::{PreparedRoute, RouteOwner, layout_allocation_failed};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_core::{OperationControl, OperationPhase};
use std::collections::{HashMap, HashSet, hash_map::Entry};

mod labels;
mod marker;

pub(super) use labels::allocate_route_label_placements;
use labels::{label_anchor_contains, resolve_label_anchor, route_label_candidates};
use marker::marker_candidate_continues_chain;
pub(super) use marker::{MarkerCandidateDisposition, allocate_marker_berths};

fn marker_occupant_is_compatible(
    routes: &[PreparedRoute],
    marker: MarkerOccupant,
    endpoint_id: &str,
    point_direction: StepDirection,
) -> bool {
    routes.get(marker.route_index).is_some_and(|route| {
        route.owner.endpoint_id(marker.endpoint) == endpoint_id
            && route.plan.marker_point_direction(marker.endpoint) == point_direction
    })
}

fn terminal_claim_is_compatible(
    routes: &[PreparedRoute],
    claim: &TerminalClaim,
    endpoint_id: &str,
    point_direction: StepDirection,
) -> bool {
    claim.point_direction == point_direction
        && routes
            .get(claim.route_index)
            .is_some_and(|route| route.owner.endpoint_id(claim.endpoint) == endpoint_id)
}

fn terminal_claims_allow_route_cell(
    existing_routes: &[PreparedRoute],
    owner: &RouteOwner,
    claims: &[TerminalClaim],
    resources: &mut ResourceContext,
    execution: Option<AsciiExecution<'_>>,
) -> Result<bool> {
    for endpoint_id in [&owner.from, &owner.to] {
        let mut all_incident_to_endpoint = true;
        for claim in claims {
            checkpoint_layout(execution)?;
            resources.charge_layout_work(1)?;
            if existing_routes
                .get(claim.route_index)
                .is_none_or(|route| route.owner.endpoint_id(claim.endpoint) != endpoint_id)
            {
                all_incident_to_endpoint = false;
                break;
            }
        }
        if all_incident_to_endpoint {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OccupiedRect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) right: usize,
    pub(super) bottom: usize,
}

impl OccupiedRect {
    pub(super) fn try_new(
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        resources: &ResourceContext,
    ) -> Result<Self> {
        Ok(Self {
            x,
            y,
            right: resources.checked_grid_add(x, width.max(1))?,
            bottom: resources.checked_grid_add(y, height.max(1))?,
        })
    }

    pub(super) fn intersects(self, other: Self) -> bool {
        self.x < other.right
            && other.x < self.right
            && self.y < other.bottom
            && other.y < self.bottom
    }

    pub(super) fn contains(self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.right && y >= self.y && y < self.bottom
    }

    fn intersects_horizontal_span(self, x_start: usize, x_end: usize, y: usize) -> bool {
        y >= self.y && y < self.bottom && x_start < self.right && self.x <= x_end
    }

    fn intersects_vertical_span(self, x: usize, y_start: usize, y_end: usize) -> bool {
        x >= self.x && x < self.right && y_start < self.bottom && self.y <= y_end
    }

    fn cell_count(self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_work_mul(self.right - self.x, self.bottom - self.y)
    }

    fn point_distance(self, coord: CanvasCoord, resources: &ResourceContext) -> Result<usize> {
        let dx = if coord.x < self.x {
            self.x - coord.x
        } else if coord.x >= self.right {
            coord.x - (self.right - 1)
        } else {
            0
        };
        let dy = if coord.y < self.y {
            self.y - coord.y
        } else if coord.y >= self.bottom {
            coord.y - (self.bottom - 1)
        } else {
            0
        };
        resources.checked_work_add(dx, dy)
    }

    fn is_perimeter(self, coord: CanvasCoord) -> bool {
        self.contains(coord.x, coord.y)
            && (coord.x == self.x
                || coord.x == self.right - 1
                || coord.y == self.y
                || coord.y == self.bottom - 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RouteBounds {
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RouteCandidateScore {
    total_cost: usize,
    shared_cells: usize,
    cell_count: usize,
    marker_pressure: usize,
}

impl RouteBounds {
    fn include(&mut self, coord: CanvasCoord) {
        self.min_x = self.min_x.min(coord.x);
        self.max_x = self.max_x.max(coord.x);
        self.min_y = self.min_y.min(coord.y);
        self.max_y = self.max_y.max(coord.y);
    }

    fn prefers_vertical_label_lanes(self) -> bool {
        self.max_x - self.min_x >= self.max_y - self.min_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RouteCellOwner {
    pub(super) route_index: usize,
    pub(super) cell: PlannedCellId,
    pub(super) segment: PlannedRouteSegment,
}

#[derive(Debug)]
pub(super) struct RouteCellOccupancy {
    pub(super) owners: Vec<RouteCellOwner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalClaim {
    route_index: usize,
    endpoint: MarkerEndpoint,
    point_direction: StepDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkerOccupant {
    route_index: usize,
    endpoint: MarkerEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProtectedKind {
    Node,
    GroupBorder,
    GroupTitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProtectedShape {
    Rect(OccupiedRect),
    HorizontalSpan {
        x_start: usize,
        x_end: usize,
        y: usize,
    },
    VerticalSpan {
        x: usize,
        y_start: usize,
        y_end: usize,
    },
}

impl ProtectedShape {
    fn contains(self, coord: CanvasCoord) -> bool {
        match self {
            Self::Rect(rect) => rect.contains(coord.x, coord.y),
            Self::HorizontalSpan { x_start, x_end, y } => {
                coord.y == y && coord.x >= x_start && coord.x <= x_end
            }
            Self::VerticalSpan { x, y_start, y_end } => {
                coord.x == x && coord.y >= y_start && coord.y <= y_end
            }
        }
    }

    pub(super) fn intersects(self, rect: OccupiedRect) -> bool {
        match self {
            Self::Rect(protected) => protected.intersects(rect),
            Self::HorizontalSpan { x_start, x_end, y } => {
                rect.intersects_horizontal_span(x_start, x_end, y)
            }
            Self::VerticalSpan { x, y_start, y_end } => {
                rect.intersects_vertical_span(x, y_start, y_end)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProtectedGeometry<'a> {
    owner_id: &'a str,
    group_index: Option<usize>,
    pub(super) kind: ProtectedKind,
    pub(super) shape: ProtectedShape,
}

impl ProtectedGeometry<'_> {
    fn allows_endpoint_port(self, endpoint_id: &str, coord: CanvasCoord) -> bool {
        if self.owner_id != endpoint_id || self.kind == ProtectedKind::GroupTitle {
            return false;
        }
        match (self.kind, self.shape) {
            (ProtectedKind::Node, ProtectedShape::Rect(rect)) => rect.is_perimeter(coord),
            (ProtectedKind::GroupBorder, shape) => shape.contains(coord),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(super) struct SceneOccupancy<'layout> {
    pub(super) route_cells: HashMap<CanvasCoord, RouteCellOccupancy>,
    pub(super) route_bounds: Vec<Option<RouteBounds>>,
    terminal_claims: HashMap<CanvasCoord, Vec<TerminalClaim>>,
    markers: HashMap<CanvasCoord, MarkerOccupant>,
    labels: HashSet<CanvasCoord>,
    pub(super) protected: Vec<ProtectedGeometry<'layout>>,
    control: Option<OperationControl>,
}

impl<'layout> SceneOccupancy<'layout> {
    #[cfg(test)]
    pub(super) fn try_new_for_routes(
        graph_layout: &'layout GraphLayout,
        route_capacity: usize,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        Self::try_new_for_routes_with_execution(graph_layout, route_capacity, resources, None)
    }

    pub(super) fn try_new_for_routes_with_execution(
        graph_layout: &'layout GraphLayout,
        route_capacity: usize,
        resources: &mut ResourceContext,
        execution: Option<AsciiExecution<'_>>,
    ) -> Result<Self> {
        let marker_capacity = resources.checked_work_mul(route_capacity, 2)?;
        let mut protected_capacity = graph_layout.nodes.len();
        for group in &graph_layout.groups {
            let border_count = match group.kind {
                GraphGroupKind::Divider => usize::from(group.divider_span.is_some()),
                GraphGroupKind::Container => 4,
            };
            protected_capacity = resources.checked_work_add(protected_capacity, border_count)?;
            protected_capacity =
                resources.checked_work_add(protected_capacity, group.title.lines().len())?;
        }

        let mut scene = Self {
            route_cells: HashMap::new(),
            route_bounds: Vec::new(),
            terminal_claims: HashMap::new(),
            markers: HashMap::new(),
            labels: HashSet::new(),
            protected: Vec::new(),
            control: execution.and_then(AsciiExecution::cloned_control),
        };
        scene
            .route_bounds
            .try_reserve(route_capacity)
            .map_err(|_| layout_allocation_failed())?;
        try_reserve_hash_map(&mut scene.terminal_claims, marker_capacity)?;
        try_reserve_hash_map(&mut scene.markers, marker_capacity)?;
        scene
            .labels
            .try_reserve(route_capacity)
            .map_err(|_| layout_allocation_failed())?;
        scene
            .protected
            .try_reserve(protected_capacity)
            .map_err(|_| layout_allocation_failed())?;

        for node in &graph_layout.nodes {
            scene.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            scene.protected.push(ProtectedGeometry {
                owner_id: &node.id,
                group_index: None,
                kind: ProtectedKind::Node,
                shape: ProtectedShape::Rect(OccupiedRect::try_new(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    resources,
                )?),
            });
        }
        for (group_index, group) in graph_layout.groups.iter().enumerate() {
            scene.register_group_geometry(group_index, group, resources)?;
        }

        Ok(scene)
    }

    pub(in crate::graph::routing) fn checkpoint_layout(&self) -> Result<()> {
        match self.control.as_ref() {
            Some(control) => control
                .checkpoint_at(OperationPhase::Layout)
                .map_err(AsciiError::Cancelled),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    pub(super) fn try_new(
        routes: &[PreparedRoute],
        graph_layout: &'layout GraphLayout,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Self> {
        let mut scene = Self::try_new_for_routes(graph_layout, routes.len(), resources)?;
        for (route_index, route) in routes.iter().enumerate() {
            let start = route
                .plan
                .terminal_candidate(MarkerEndpoint::Start, diagram_type)?;
            let end = route
                .plan
                .terminal_candidate(MarkerEndpoint::End, diagram_type)?;
            scene.commit_route(route_index, route, start, end, resources)?;
        }
        Ok(scene)
    }

    #[cfg(test)]
    pub(super) fn try_admit_route(
        &mut self,
        route_index: usize,
        route: &PreparedRoute,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<bool> {
        let start = route
            .plan
            .terminal_candidate(MarkerEndpoint::Start, diagram_type)?;
        let end = route
            .plan
            .terminal_candidate(MarkerEndpoint::End, diagram_type)?;

        if self
            .score_route(&[], &route.plan, &route.owner, resources, diagram_type)?
            .is_none()
        {
            return Ok(false);
        }

        self.commit_route(route_index, route, start, end, resources)?;
        Ok(true)
    }

    #[cfg(test)]
    pub(super) fn score_route(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<RouteCandidateScore>> {
        self.score_route_with_execution(existing_routes, plan, owner, resources, diagram_type, None)
    }

    pub(super) fn score_route_with_execution(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
        execution: Option<AsciiExecution<'_>>,
    ) -> Result<Option<RouteCandidateScore>> {
        let mut shared_cells = 0usize;

        for (_, cell) in plan.active_cells() {
            checkpoint_layout(execution)?;
            resources.charge_layout_work(self.protected.len().max(1))?;
            let crosses_reserved = self.protected.iter().any(|protected| {
                let is_endpoint_port = protected.allows_endpoint_port(&owner.from, cell.coord)
                    || protected.allows_endpoint_port(&owner.to, cell.coord);
                let is_owned_group_border = protected.kind == ProtectedKind::GroupBorder
                    && protected.group_index.is_some_and(|group_index| {
                        owner.boundary_group_indices.contains(&group_index)
                    });
                protected.shape.contains(cell.coord) && !is_endpoint_port && !is_owned_group_border
            });
            if crosses_reserved {
                return Ok(None);
            }
            if let Some(claims) = self.terminal_claims.get(&cell.coord)
                && !terminal_claims_allow_route_cell(
                    existing_routes,
                    owner,
                    claims,
                    resources,
                    execution,
                )?
            {
                return Ok(None);
            }
            if self.route_cells.contains_key(&cell.coord) {
                checkpoint_layout(execution)?;
                resources.charge_layout_work(1)?;
                shared_cells = resources.checked_work_add(shared_cells, 1)?;
            }
        }

        checkpoint_layout(execution)?;
        if !self.plan_labels_have_clear_candidate(plan, resources)? {
            return Ok(None);
        }

        let mut marker_pressure = 0usize;
        for endpoint in [MarkerEndpoint::Start, MarkerEndpoint::End] {
            let candidates = plan.marker_candidates(endpoint, diagram_type, resources)?;
            if candidates.is_empty() {
                continue;
            }
            let mut available = 0usize;
            let mut predecessor = None;
            for candidate in candidates.iter().copied() {
                checkpoint_layout(execution)?;
                resources.charge_layout_work(1)?;
                if !marker_candidate_continues_chain(predecessor, candidate) {
                    break;
                }
                match self.marker_candidate_disposition_before_commit(
                    existing_routes,
                    owner,
                    endpoint,
                    candidate,
                    resources,
                )? {
                    MarkerCandidateDisposition::Available => {
                        available = resources.checked_work_add(available, 1)?;
                        predecessor = Some(candidate);
                    }
                    MarkerCandidateDisposition::CompatiblePassThrough => {
                        predecessor = Some(candidate);
                    }
                    MarkerCandidateDisposition::Blocked => break,
                }
            }
            if available == 0 {
                return Ok(None);
            }
            marker_pressure = resources.checked_work_add(
                marker_pressure,
                MAX_MARKER_CANDIDATES.saturating_sub(available.min(MAX_MARKER_CANDIDATES)),
            )?;
        }

        let cell_count = plan.active_cells().count();
        let total_cost = resources.checked_work_add(cell_count, shared_cells)?;
        Ok(Some(RouteCandidateScore {
            total_cost,
            shared_cells,
            cell_count,
            marker_pressure,
        }))
    }

    fn plan_labels_have_clear_candidate(
        &self,
        plan: &RoutePlan,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        if plan.labels.is_empty() {
            return Ok(true);
        }

        let route_bounds = plan.active_cells().fold(None, |bounds, (_, cell)| {
            let mut bounds = bounds.unwrap_or(RouteBounds {
                min_x: cell.coord.x,
                max_x: cell.coord.x,
                min_y: cell.coord.y,
                max_y: cell.coord.y,
            });
            bounds.include(cell.coord);
            Some(bounds)
        });

        for label in &plan.labels {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let original = OccupiedRect::try_new(
                label.placement.x(),
                label.placement.y(),
                label.placement.width(),
                label.line_count(),
                resources,
            )?;
            let anchor = resolve_label_anchor(plan, label.anchor, original, resources)?;
            let candidates = route_label_candidates(
                label.placement,
                label.line_count(),
                anchor,
                route_bounds,
                resources,
            )?;
            let mut clear = false;
            for candidate in candidates {
                let footprint = OccupiedRect::try_new(
                    candidate.x(),
                    candidate.y(),
                    candidate.width(),
                    label.line_count(),
                    resources,
                )?;
                if self.plan_label_candidate_is_clear(plan, anchor, footprint, resources)? {
                    clear = true;
                    break;
                }
            }
            if !clear {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn plan_label_candidate_is_clear(
        &self,
        plan: &RoutePlan,
        anchor: LabelAnchor,
        candidate: OccupiedRect,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(candidate) {
                return Ok(false);
            }
        }

        for y in candidate.y..candidate.bottom {
            for x in candidate.x..candidate.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord)
                    || self.labels.contains(&coord)
                    || self.route_cells.contains_key(&coord)
                {
                    return Ok(false);
                }
            }
        }

        self.checkpoint_layout()?;
        resources.charge_layout_work(plan.cells.len())?;
        if plan.active_cells().any(|(_, cell)| {
            candidate.contains(cell.coord.x, cell.coord.y)
                && !label_anchor_contains(anchor, cell.coord, cell.segment)
        }) {
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn marker_candidate_disposition_before_commit(
        &self,
        existing_routes: &[PreparedRoute],
        owner: &RouteOwner,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
        resources: &mut ResourceContext,
    ) -> Result<MarkerCandidateDisposition> {
        let endpoint_id = owner.endpoint_id(endpoint);
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let allowed = !protected.shape.contains(candidate.coord)
                || (candidate.is_primary()
                    && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                        || (protected.kind == ProtectedKind::GroupBorder
                            && (protected.allows_endpoint_port(&owner.from, candidate.coord)
                                || protected.allows_endpoint_port(&owner.to, candidate.coord)))));
            if !allowed {
                return Ok(MarkerCandidateDisposition::Blocked);
            }
        }

        if let Some(marker) = self.markers.get(&candidate.coord) {
            return Ok(
                if marker_occupant_is_compatible(
                    existing_routes,
                    *marker,
                    endpoint_id,
                    candidate.point_direction,
                ) {
                    MarkerCandidateDisposition::CompatiblePassThrough
                } else {
                    MarkerCandidateDisposition::Blocked
                },
            );
        }

        let claims = self.terminal_claims.get(&candidate.coord);
        if let Some(claims) = claims {
            for claim in claims {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if !terminal_claim_is_compatible(
                    existing_routes,
                    claim,
                    endpoint_id,
                    candidate.point_direction,
                ) {
                    return Ok(MarkerCandidateDisposition::Blocked);
                }
            }
        }

        if let Some(cell) = self.route_cells.get(&candidate.coord) {
            let Some(claims) = claims else {
                return Ok(MarkerCandidateDisposition::Blocked);
            };
            for cell_owner in &cell.owners {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let mut matched = false;
                for claim in claims {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(1)?;
                    if claim.route_index == cell_owner.route_index
                        && terminal_claim_is_compatible(
                            existing_routes,
                            claim,
                            endpoint_id,
                            candidate.point_direction,
                        )
                    {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return Ok(MarkerCandidateDisposition::Blocked);
                }
            }
        }

        Ok(MarkerCandidateDisposition::Available)
    }

    pub(super) fn commit_route(
        &mut self,
        route_index: usize,
        route: &PreparedRoute,
        start: MarkerCandidate,
        end: MarkerCandidate,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        debug_assert_eq!(self.route_bounds.len(), route_index);
        let mut bounds: Option<RouteBounds> = None;
        for (cell_index, cell) in route.plan.cells.iter().enumerate() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if route
                .plan
                .is_cell_suppressed(PlannedCellId::new(cell_index))
            {
                continue;
            }
            match &mut bounds {
                Some(bounds) => bounds.include(cell.coord),
                None => {
                    bounds = Some(RouteBounds {
                        min_x: cell.coord.x,
                        max_x: cell.coord.x,
                        min_y: cell.coord.y,
                        max_y: cell.coord.y,
                    });
                }
            }
            let owner = RouteCellOwner {
                route_index,
                cell: PlannedCellId::new(cell_index),
                segment: cell.segment,
            };
            match self.route_cells.entry(cell.coord) {
                Entry::Occupied(mut occupied) => {
                    occupied
                        .get_mut()
                        .owners
                        .try_reserve(1)
                        .map_err(|_| layout_allocation_failed())?;
                    occupied.get_mut().owners.push(owner);
                }
                Entry::Vacant(vacant) => {
                    let mut owners = Vec::new();
                    owners
                        .try_reserve(1)
                        .map_err(|_| layout_allocation_failed())?;
                    owners.push(owner);
                    vacant.insert(RouteCellOccupancy { owners });
                }
            }
        }
        self.route_bounds.push(bounds);

        for (endpoint, candidate) in [(MarkerEndpoint::Start, start), (MarkerEndpoint::End, end)] {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            self.register_terminal_claim(
                candidate.coord,
                TerminalClaim {
                    route_index,
                    endpoint,
                    point_direction: candidate.point_direction,
                },
            )?;
        }
        Ok(())
    }

    fn register_terminal_claim(&mut self, coord: CanvasCoord, claim: TerminalClaim) -> Result<()> {
        if let Some(claims) = self.terminal_claims.get_mut(&coord) {
            claims
                .try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            claims.push(claim);
            return Ok(());
        }

        self.terminal_claims
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        let mut claims = Vec::new();
        claims
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        claims.push(claim);
        self.terminal_claims.insert(coord, claims);
        Ok(())
    }

    fn register_group_geometry(
        &mut self,
        group_index: usize,
        group: &'layout GroupLayout,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        match group.kind {
            GraphGroupKind::Divider => {
                if let Some(span) = group.divider_span {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(1)?;
                    self.protected.push(ProtectedGeometry {
                        owner_id: &group.id,
                        group_index: Some(group_index),
                        kind: ProtectedKind::GroupBorder,
                        shape: ProtectedShape::HorizontalSpan {
                            x_start: span.x_start,
                            x_end: span.x_end,
                            y: group.y,
                        },
                    });
                }
            }
            GraphGroupKind::Container => {
                let right = group.right();
                let bottom = group.bottom();
                self.checkpoint_layout()?;
                resources.charge_layout_work(4)?;
                self.protected.extend([
                    ProtectedGeometry {
                        owner_id: &group.id,
                        group_index: Some(group_index),
                        kind: ProtectedKind::GroupBorder,
                        shape: ProtectedShape::HorizontalSpan {
                            x_start: group.x,
                            x_end: right,
                            y: group.y,
                        },
                    },
                    ProtectedGeometry {
                        owner_id: &group.id,
                        group_index: Some(group_index),
                        kind: ProtectedKind::GroupBorder,
                        shape: ProtectedShape::HorizontalSpan {
                            x_start: group.x,
                            x_end: right,
                            y: bottom,
                        },
                    },
                    ProtectedGeometry {
                        owner_id: &group.id,
                        group_index: Some(group_index),
                        kind: ProtectedKind::GroupBorder,
                        shape: ProtectedShape::VerticalSpan {
                            x: group.x,
                            y_start: group.y,
                            y_end: bottom,
                        },
                    },
                    ProtectedGeometry {
                        owner_id: &group.id,
                        group_index: Some(group_index),
                        kind: ProtectedKind::GroupBorder,
                        shape: ProtectedShape::VerticalSpan {
                            x: right,
                            y_start: group.y,
                            y_end: bottom,
                        },
                    },
                ]);
            }
        }

        let available_title_width = group.width.saturating_sub(2);
        let inner_left = resources.checked_grid_add(group.x, 1)?;
        let center = resources.checked_grid_add(group.x, group.width.saturating_sub(1) / 2)?;
        let line_step = resources.checked_grid_add(GRAPH_LABEL_LINE_GAP, 1)?;
        for (line_index, line) in group.title.lines().iter().enumerate() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let title_width = group.title.line_width(line);
            if title_width == 0 || title_width > available_title_width {
                continue;
            }
            let title_x = center
                .checked_sub(title_width / 2)
                .ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?
                .max(inner_left);
            let title_y = resources.checked_grid_add(
                resources.checked_grid_add(group.y, 1)?,
                resources.checked_grid_mul(line_index, line_step)?,
            )?;
            self.protected.push(ProtectedGeometry {
                owner_id: &group.id,
                group_index: Some(group_index),
                kind: ProtectedKind::GroupTitle,
                shape: ProtectedShape::Rect(OccupiedRect::try_new(
                    title_x,
                    title_y,
                    title_width,
                    1,
                    resources,
                )?),
            });
        }
        Ok(())
    }

    fn marker_candidate_disposition(
        &self,
        routes: &[PreparedRoute],
        route_index: usize,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
        resources: &mut ResourceContext,
    ) -> Result<MarkerCandidateDisposition> {
        let endpoint_id = routes[route_index].owner.endpoint_id(endpoint);
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.contains(candidate.coord)
                && !(candidate.is_primary()
                    && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                        || (protected.kind == ProtectedKind::GroupBorder
                            && (protected.allows_endpoint_port(
                                &routes[route_index].owner.from,
                                candidate.coord,
                            ) || protected.allows_endpoint_port(
                                &routes[route_index].owner.to,
                                candidate.coord,
                            )))))
            {
                return Ok(MarkerCandidateDisposition::Blocked);
            }
        }

        if let Some(marker) = self.markers.get(&candidate.coord) {
            return Ok(
                if marker_occupant_is_compatible(
                    routes,
                    *marker,
                    endpoint_id,
                    candidate.point_direction,
                ) {
                    MarkerCandidateDisposition::CompatiblePassThrough
                } else {
                    MarkerCandidateDisposition::Blocked
                },
            );
        }

        let Some(route_cell) = self.route_cells.get(&candidate.coord) else {
            return Ok(MarkerCandidateDisposition::Blocked);
        };
        self.checkpoint_layout()?;
        resources.charge_layout_work(route_cell.owners.len())?;
        if !route_cell
            .owners
            .iter()
            .any(|owner| owner.route_index == route_index && owner.cell == candidate.cell)
        {
            return Ok(MarkerCandidateDisposition::Blocked);
        }
        let Some(claims) = self.terminal_claims.get(&candidate.coord) else {
            return Ok(if route_cell.owners.len() == 1 {
                MarkerCandidateDisposition::Available
            } else {
                MarkerCandidateDisposition::Blocked
            });
        };
        for owner in &route_cell.owners {
            if owner.route_index == route_index {
                if owner.cell != candidate.cell {
                    return Ok(MarkerCandidateDisposition::Blocked);
                }
                continue;
            }
            self.checkpoint_layout()?;
            resources.charge_layout_work(claims.len())?;
            let compatible = claims.iter().any(|claim| {
                claim.route_index == owner.route_index
                    && terminal_claim_is_compatible(
                        routes,
                        claim,
                        endpoint_id,
                        candidate.point_direction,
                    )
            });
            if !compatible {
                return Ok(MarkerCandidateDisposition::Blocked);
            }
        }
        Ok(MarkerCandidateDisposition::Available)
    }

    fn suppress_route_terminal_tail(
        &mut self,
        route_index: usize,
        suppressed_tail: &[PlannedCellId],
        plan: &RoutePlan,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<()> {
        if suppressed_tail.is_empty() {
            return Ok(());
        }

        for suppressed in suppressed_tail {
            let Some(cell) = plan.cells.get(suppressed.index()) else {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type,
                    feature: "routes with missing suppressed terminal cells",
                });
            };
            let mut remove_coord = false;
            self.checkpoint_layout()?;
            if let Some(occupancy) = self.route_cells.get_mut(&cell.coord) {
                resources.charge_layout_work(occupancy.owners.len())?;
                occupancy
                    .owners
                    .retain(|owner| owner.route_index != route_index || owner.cell != *suppressed);
                remove_coord = occupancy.owners.is_empty();
            }
            if remove_coord {
                self.route_cells.remove(&cell.coord);
            }
        }

        let mut bounds: Option<RouteBounds> = None;
        for (_, cell) in plan.active_cells() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            match &mut bounds {
                Some(bounds) => bounds.include(cell.coord),
                None => {
                    bounds = Some(RouteBounds {
                        min_x: cell.coord.x,
                        max_x: cell.coord.x,
                        min_y: cell.coord.y,
                        max_y: cell.coord.y,
                    });
                }
            }
        }
        let Some(bounds_slot) = self.route_bounds.get_mut(route_index) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "route terminal suppression without owned bounds",
            });
        };
        *bounds_slot = bounds;
        Ok(())
    }

    fn occupy_marker(
        &mut self,
        coord: CanvasCoord,
        marker: MarkerOccupant,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<()> {
        self.checkpoint_layout()?;
        resources.charge_layout_work(1)?;
        match self.markers.entry(coord) {
            Entry::Occupied(existing) => {
                debug_assert!(
                    existing.get().route_index != marker.route_index
                        || existing.get().endpoint != marker.endpoint
                );
                Err(AsciiError::UnsupportedFeature {
                    diagram_type,
                    feature: "conflicting edge marker ownership",
                })
            }
            Entry::Vacant(vacant) => {
                vacant.insert(marker);
                Ok(())
            }
        }
    }

    pub(super) fn label_candidate_is_clear(
        &self,
        route_index: usize,
        anchor: LabelAnchor,
        candidate: OccupiedRect,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(candidate) {
                return Ok(false);
            }
        }

        for y in candidate.y..candidate.bottom {
            for x in candidate.x..candidate.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord) || self.labels.contains(&coord) {
                    return Ok(false);
                }
                if let Some(route_cell) = self.route_cells.get(&coord) {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(route_cell.owners.len())?;
                    if route_cell.owners.iter().any(|owner| {
                        owner.route_index != route_index
                            || !label_anchor_contains(anchor, coord, owner.segment)
                    }) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    fn occupy_label(
        &mut self,
        footprint: OccupiedRect,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let footprint_cells = footprint.cell_count(resources)?;
        self.labels
            .try_reserve(footprint_cells)
            .map_err(|_| layout_allocation_failed())?;
        for y in footprint.y..footprint.bottom {
            for x in footprint.x..footprint.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                self.labels.insert(CanvasCoord { x, y });
            }
        }
        Ok(())
    }
}

fn checkpoint_layout(execution: Option<AsciiExecution<'_>>) -> Result<()> {
    if let Some(execution) = execution {
        execution.checkpoint(OperationPhase::Layout)?;
    }
    Ok(())
}

fn try_reserve_hash_map<K, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()>
where
    K: Eq + std::hash::Hash,
{
    map.try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}
