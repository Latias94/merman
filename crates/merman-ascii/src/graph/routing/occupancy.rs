use super::super::label::GRAPH_LABEL_LINE_GAP;
use super::super::layout::{CanvasCoord, GraphLayout, GroupLayout};
use super::super::model::GraphGroupKind;
use super::cell::{
    DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP, route_char_directions, stroke_route_char,
};
use super::path::StepDirection;
use super::plan::{
    LabelAnchor, MAX_MARKER_CANDIDATES, MarkerCandidate, MarkerEndpoint, NodeAttachments,
    PlannedCellId, PlannedRouteCellKind, PlannedRouteSegment, RoutePlan,
};
use super::{PreparedRoute, RouteOwner, layout_allocation_failed};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::{OperationControl, OperationPhase};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::hash_map::Entry;

mod labels;
mod marker;

const OCCUPANCY_SCAN_CHECKPOINT_INTERVAL: usize = 64;

pub(super) use labels::allocate_committed_route_labels;
#[cfg(test)]
pub(super) use labels::allocate_route_label_placements;
use labels::{
    label_anchor_contains, label_reservation, resolve_label_anchor, route_label_candidates,
};
#[cfg(test)]
pub(super) use marker::allocate_marker_berths;
use marker::marker_candidate_continues_chain;
pub(super) use marker::{MarkerCandidateDisposition, allocate_committed_route_marker_berths};

fn own_terminal_marker_escape(
    protected: &ProtectedGeometry<'_>,
    endpoint_id: &str,
    plan: &RoutePlan,
    endpoint: MarkerEndpoint,
    candidate: MarkerCandidate,
) -> bool {
    if protected.kind != ProtectedKind::Node || protected.owner_id != endpoint_id {
        return false;
    }
    plan.attachments.is_some_and(|attachments| {
        let attachment = match endpoint {
            MarkerEndpoint::Start => attachments.start,
            MarkerEndpoint::End => attachments.end,
        };
        candidate.coord != attachment.contact && attachment.allows_escape(candidate.coord)
    })
}

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
    directions: u8,
    resources: &mut ResourceContext,
    control: &OperationControl,
) -> Result<bool> {
    for endpoint_id in [&owner.from, &owner.to] {
        let mut all_incident_to_endpoint = true;
        for claim in claims {
            control
                .checkpoint_at(OperationPhase::Layout)
                .map_err(AsciiError::Cancelled)?;
            resources.charge_layout_work(1)?;
            let claim_axis = match claim.point_direction {
                StepDirection::Left | StepDirection::Right => DIR_LEFT | DIR_RIGHT,
                StepDirection::Up | StepDirection::Down => DIR_UP | DIR_DOWN,
            };
            // Alternative marker positions are opportunities, not occupied terminals.
            // A transverse crossing preserves the committed heads; later marker
            // admission checks the crossing's owner before using this position.
            if !claim.primary && super::cell::is_transverse_crossing(directions, claim_axis) {
                continue;
            }
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

    fn conflicts_with_marker(self, coord: CanvasCoord, direction: StepDirection) -> bool {
        self.contains(coord.x, coord.y)
            || (matches!(direction, StepDirection::Up | StepDirection::Down)
                && (self.y..self.bottom).contains(&coord.y)
                && (coord.x.checked_add(1) == Some(self.x) || coord.x == self.right))
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
    target_port_reuse: usize,
    source_port_reuse: usize,
    ambiguous_shared_cells: usize,
    total_cost: usize,
    shared_cells: usize,
    cell_count: usize,
    marker_pressure: usize,
}

impl RouteCandidateScore {
    pub(super) fn has_congestion(self) -> bool {
        self.target_port_reuse != 0
            || self.source_port_reuse != 0
            || self.ambiguous_shared_cells != 0
    }
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
    primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkerOccupant {
    route_index: usize,
    endpoint: MarkerEndpoint,
    point_direction: StepDirection,
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

    fn transverse_border_directions(self, coord: CanvasCoord, directions: u8) -> Option<u8> {
        let horizontal = DIR_LEFT | DIR_RIGHT;
        let vertical = DIR_UP | DIR_DOWN;
        match self {
            Self::HorizontalSpan { x_start, x_end, y }
                if coord.y == y
                    && x_start < coord.x
                    && coord.x < x_end
                    && directions == vertical =>
            {
                Some(horizontal)
            }
            Self::VerticalSpan { x, y_start, y_end }
                if coord.x == x
                    && y_start < coord.y
                    && coord.y < y_end
                    && directions == horizontal =>
            {
                Some(vertical)
            }
            _ => None,
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
    graph_layout: &'layout GraphLayout,
    pub(super) route_cells: HashMap<CanvasCoord, RouteCellOccupancy>,
    pub(super) route_bounds: Vec<Option<RouteBounds>>,
    terminal_claims: HashMap<CanvasCoord, Vec<TerminalClaim>>,
    target_ports: HashMap<(CanvasCoord, StepDirection), usize>,
    source_ports: HashMap<(CanvasCoord, StepDirection), usize>,
    markers: HashMap<CanvasCoord, MarkerOccupant>,
    // Includes each label's text and the clear perpendicular gap to its host.
    labels: HashSet<CanvasCoord>,
    pub(super) protected: Vec<ProtectedGeometry<'layout>>,
    pub(super) label_lane_radius: usize,
    control: OperationControl,
    occupied_extent: CanvasCoord,
}

impl<'layout> SceneOccupancy<'layout> {
    pub(super) fn occupied_extent(&self) -> CanvasCoord {
        self.occupied_extent
    }
    pub(super) fn target_port_usage(&self, attachment: super::plan::NodeAttachment) -> usize {
        self.target_ports
            .get(&(attachment.contact, attachment.outward))
            .copied()
            .unwrap_or_default()
    }

    /// Counts committed start-marker requests before marker berth allocation.
    pub(super) fn source_port_usage(&self, attachment: super::plan::NodeAttachment) -> usize {
        self.source_ports
            .get(&(attachment.contact, attachment.outward))
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn route_cell_congestion(
        &self,
        existing_routes: &[PreparedRoute],
        owner: &RouteOwner,
        attachments: Option<NodeAttachments>,
        coord: CanvasCoord,
        resources: &mut ResourceContext,
    ) -> Result<usize> {
        let Some(occupied) = self.route_cells.get(&coord) else {
            return Ok(0);
        };
        let mut penalty = 0;
        for shared_owner in &occupied.owners {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let existing = &existing_routes[shared_owner.route_index];
            let shares_terminal_ray =
                attachments
                    .zip(existing.plan.attachments)
                    .is_some_and(|(current, previous)| {
                        (owner.from == existing.owner.from
                            && current.start == previous.start
                            && current.start.allows_escape(coord))
                            || (owner.to == existing.owner.to
                                && current.end == previous.end
                                && current.end.allows_escape(coord))
                    });
            if !shares_terminal_ray {
                penalty = resources.checked_work_add(penalty, 1)?;
            }
        }
        Ok(penalty)
    }
    pub(super) fn try_new_for_routes(
        graph_layout: &'layout GraphLayout,
        route_capacity: usize,
        label_lane_radius: usize,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
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
            graph_layout,
            route_cells: HashMap::default(),
            route_bounds: Vec::new(),
            terminal_claims: HashMap::default(),
            target_ports: HashMap::default(),
            source_ports: HashMap::default(),
            markers: HashMap::default(),
            labels: HashSet::default(),
            protected: Vec::new(),
            label_lane_radius,
            control: execution.cloned_control(),
            occupied_extent: CanvasCoord { x: 0, y: 0 },
        };
        scene
            .route_bounds
            .try_reserve(route_capacity)
            .map_err(|_| layout_allocation_failed())?;
        try_reserve_hash_map(&mut scene.terminal_claims, marker_capacity)?;
        try_reserve_hash_map(&mut scene.target_ports, route_capacity)?;
        try_reserve_hash_map(&mut scene.source_ports, route_capacity)?;
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
        self.control
            .checkpoint_at(OperationPhase::Layout)
            .map_err(AsciiError::Cancelled)
    }

    fn checkpoint_layout_scan(&self, iteration: usize) -> Result<()> {
        if iteration > 0 && iteration.is_multiple_of(OCCUPANCY_SCAN_CHECKPOINT_INTERVAL) {
            self.checkpoint_layout()?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn try_new(
        routes: &[PreparedRoute],
        graph_layout: &'layout GraphLayout,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Self> {
        let policy = resources.policy();
        let mut scene = Self::try_new_for_routes(
            graph_layout,
            routes.len(),
            crate::options::GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
            resources,
            AsciiExecution::for_test(&policy),
        )?;
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

    pub(super) fn score_route(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<RouteCandidateScore>> {
        let mut shared_cells = 0usize;
        let mut ambiguous_shared_cells = 0usize;

        for (_, cell) in plan.active_cells() {
            if !self.route_cell_is_clear(
                existing_routes,
                owner,
                plan.attachments,
                cell.coord,
                if cell.directions == 0 {
                    route_char_directions(cell.ch)
                } else {
                    cell.directions
                },
                resources,
            )? {
                return Ok(None);
            }
            if self.route_cells.contains_key(&cell.coord) {
                shared_cells = resources.checked_work_add(shared_cells, 1)?;
                let penalty = self.route_cell_congestion(
                    existing_routes,
                    owner,
                    plan.attachments,
                    cell.coord,
                    resources,
                )?;
                ambiguous_shared_cells =
                    resources.checked_work_add(ambiguous_shared_cells, penalty)?;
            }
        }

        self.checkpoint_layout()?;
        let mut marker_pressure = 0usize;
        let mut marker_berths = [None; 2];
        for (index, endpoint) in [MarkerEndpoint::Start, MarkerEndpoint::End]
            .into_iter()
            .enumerate()
        {
            let Some(candidates) = plan.marker_candidates(endpoint, diagram_type, resources)?
            else {
                return Ok(None);
            };
            if candidates.is_empty() {
                continue;
            }
            let mut available = 0usize;
            let mut predecessor = None;
            for candidate in candidates.iter().copied() {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if !marker_candidate_continues_chain(predecessor, candidate) {
                    break;
                }
                match self.marker_candidate_disposition_before_commit(
                    existing_routes,
                    owner,
                    plan,
                    endpoint,
                    candidate,
                    resources,
                )? {
                    MarkerCandidateDisposition::Available => {
                        marker_berths[index].get_or_insert(candidate.coord);
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

        if marker_berths[0].is_some() && marker_berths[0] == marker_berths[1] {
            return Ok(None);
        }
        if !self.plan_labels_have_clear_candidate(plan, owner, marker_berths, resources)? {
            return Ok(None);
        }

        self.checkpoint_layout()?;
        let mut cell_count = 0usize;
        for (cell_index, _) in plan.active_cells().enumerate() {
            self.checkpoint_layout_scan(cell_index)?;
            cell_count = resources.checked_work_add(cell_count, 1)?;
        }
        let total_cost = resources.checked_work_add(cell_count, shared_cells)?;
        let target_port_reuse = plan
            .attachments
            .map_or(0, |ports| self.target_port_usage(ports.end));
        let source_port_reuse = if plan.requests_marker(MarkerEndpoint::Start) {
            plan.attachments
                .map_or(0, |ports| self.source_port_usage(ports.start))
        } else {
            0
        };
        Ok(Some(RouteCandidateScore {
            target_port_reuse,
            source_port_reuse,
            ambiguous_shared_cells,
            total_cost,
            shared_cells,
            cell_count,
            marker_pressure,
        }))
    }

    // Candidate admission and obstacle-aware search use the same cell-level semantic guards.
    // Search only supplies straight segment directions; final admission also checks corners,
    // labels, and independent marker berths before any route is committed.
    pub(super) fn route_cell_is_clear(
        &self,
        existing_routes: &[PreparedRoute],
        owner: &RouteOwner,
        attachments: Option<NodeAttachments>,
        coord: CanvasCoord,
        directions: u8,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        self.checkpoint_layout()?;
        if self.labels.contains(&coord) {
            return Ok(false);
        }
        resources.charge_layout_work(self.protected.len().max(1))?;
        for (protected_index, protected) in self.protected.iter().enumerate() {
            self.checkpoint_layout_scan(protected_index)?;
            let is_endpoint_port = protected.allows_endpoint_port(&owner.from, coord)
                || protected.allows_endpoint_port(&owner.to, coord);
            let is_owned_group_border = protected.kind == ProtectedKind::GroupBorder
                && protected.group_index.is_some_and(|group_index| {
                    owner
                        .boundary_group_indices
                        .binary_search(&group_index)
                        .is_ok()
                })
                && protected
                    .shape
                    .transverse_border_directions(coord, directions)
                    .is_some();
            if let (ProtectedKind::Node, ProtectedShape::Rect(rect), Some(attachments)) =
                (protected.kind, protected.shape, attachments)
            {
                let in_reading_margin = coord.x >= rect.x.saturating_sub(1)
                    && coord.x <= rect.right
                    && coord.y >= rect.y.saturating_sub(1)
                    && coord.y <= rect.bottom;
                if in_reading_margin
                    && !(protected.owner_id == owner.from && attachments.start.allows_escape(coord)
                        || protected.owner_id == owner.to && attachments.end.allows_escape(coord))
                {
                    return Ok(false);
                }
            }
            let is_own_terminal_escape = protected.kind == ProtectedKind::Node
                && attachments.is_some_and(|attachments| {
                    protected.owner_id == owner.from && attachments.start.allows_escape(coord)
                        || protected.owner_id == owner.to && attachments.end.allows_escape(coord)
                });
            if protected.shape.contains(coord)
                && !is_endpoint_port
                && !is_own_terminal_escape
                && !is_owned_group_border
            {
                return Ok(false);
            }
        }
        if let Some(marker) = self.markers.get(&coord) {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let existing = &existing_routes[marker.route_index];
            let existing_endpoint = existing.owner.endpoint_id(marker.endpoint);
            let existing_attachment =
                existing
                    .plan
                    .attachments
                    .map(|attachments| match marker.endpoint {
                        MarkerEndpoint::Start => attachments.start,
                        MarkerEndpoint::End => attachments.end,
                    });
            let shares_terminal = attachments.is_some_and(|attachments| {
                [
                    (owner.from.as_str(), attachments.start),
                    (owner.to.as_str(), attachments.end),
                ]
                .into_iter()
                .any(|(endpoint, attachment)| {
                    endpoint == existing_endpoint
                        && existing_attachment.is_some_and(|existing_attachment| {
                            attachment.contact == existing_attachment.contact
                                && attachment.outward == existing_attachment.outward
                                && attachment.allows_escape(coord)
                        })
                })
            });
            if !shares_terminal {
                return Ok(false);
            }
        }
        if let Some(claims) = self.terminal_claims.get(&coord)
            && !terminal_claims_allow_route_cell(
                existing_routes,
                owner,
                claims,
                directions,
                resources,
                &self.control,
            )?
        {
            return Ok(false);
        }
        if let Some(occupied) = self.route_cells.get(&coord) {
            self.checkpoint_layout()?;
            resources.charge_layout_work(occupied.owners.len().max(1))?;
            for shared_owner in &occupied.owners {
                let existing = &existing_routes[shared_owner.route_index];
                let reverse_pair = owner.from != owner.to
                    && existing.owner.from == owner.to
                    && existing.owner.to == owner.from;
                let shared_contact = attachments.is_some_and(|ports| {
                    coord == ports.start.contact || coord == ports.end.contact
                });
                let existing_cell = &existing.plan.cells[shared_owner.cell.index()];
                let existing_directions = if existing_cell.directions == 0 {
                    route_char_directions(existing_cell.ch)
                } else {
                    existing_cell.directions
                };
                let transverse =
                    super::cell::is_transverse_crossing(directions, existing_directions);
                if owner.stroke != existing.owner.stroke && !shared_contact && !transverse {
                    // A painted cell cannot retain two stroke identities. Reject the route
                    // here so another lane can be tried before emission starts.
                    return Ok(false);
                }
                if reverse_pair && !shared_contact && !transverse {
                    // Opposite authored edges need distinct transit paths. Sharing the
                    // same stroke would erase which edge owns each direction and label;
                    // a perpendicular crossing preserves both independent paths.
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn plan_labels_have_clear_candidate(
        &self,
        plan: &RoutePlan,
        owner: &RouteOwner,
        marker_berths: [Option<CanvasCoord>; 2],
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        if plan.labels.is_empty() {
            return Ok(true);
        }

        self.checkpoint_layout()?;
        let mut route_bounds = None;
        for (cell_index, (_, cell)) in plan.active_cells().enumerate() {
            self.checkpoint_layout_scan(cell_index)?;
            let mut bounds = route_bounds.unwrap_or(RouteBounds {
                min_x: cell.coord.x,
                max_x: cell.coord.x,
                min_y: cell.coord.y,
                max_y: cell.coord.y,
            });
            bounds.include(cell.coord);
            route_bounds = Some(bounds);
        }

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
                self.label_lane_radius,
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
                let Some(reservation) =
                    label_reservation(anchor, footprint, self.label_lane_radius, resources)?
                else {
                    continue;
                };
                if self.label_region_is_clear(owner, footprint, resources)?
                    && !marker_berths
                        .into_iter()
                        .zip([MarkerEndpoint::Start, MarkerEndpoint::End])
                        .any(|(coord, endpoint)| {
                            coord.is_some_and(|coord| {
                                reservation.conflicts_with_marker(
                                    coord,
                                    plan.marker_point_direction(endpoint),
                                )
                            })
                        })
                    && self.plan_label_candidate_is_clear(plan, anchor, footprint, resources)?
                {
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
        let Some(reservation) =
            label_reservation(anchor, candidate, self.label_lane_radius, resources)?
        else {
            return Ok(false);
        };
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(reservation) {
                return Ok(false);
            }
        }

        if !self.label_reading_clearance_is_clear(candidate, resources)? {
            return Ok(false);
        }

        for y in reservation.y..reservation.bottom {
            for x in reservation.x..reservation.right {
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
        for (cell_index, (_, cell)) in plan.active_cells().enumerate() {
            self.checkpoint_layout_scan(cell_index)?;
            if reservation.contains(cell.coord.x, cell.coord.y)
                && !label_anchor_contains(anchor, cell.coord, cell.segment)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn marker_candidate_disposition_before_commit(
        &self,
        existing_routes: &[PreparedRoute],
        owner: &RouteOwner,
        plan: &RoutePlan,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
        resources: &mut ResourceContext,
    ) -> Result<MarkerCandidateDisposition> {
        if !self.marker_reading_clearance_is_clear(candidate, resources)? {
            return Ok(MarkerCandidateDisposition::Blocked);
        }
        let endpoint_id = owner.endpoint_id(endpoint);
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let allowed = !protected.shape.contains(candidate.coord)
                || (candidate.is_primary()
                    && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                        || own_terminal_marker_escape(
                            protected,
                            endpoint_id,
                            plan,
                            endpoint,
                            candidate,
                        )
                        || self
                            .divider_allows_terminal_marker(protected, plan, endpoint, candidate)
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

    fn marker_reading_clearance_is_clear(
        &self,
        candidate: MarkerCandidate,
        resources: &ResourceContext,
    ) -> Result<bool> {
        if matches!(
            candidate.point_direction,
            StepDirection::Up | StepDirection::Down
        ) {
            for x in [
                candidate.coord.x.checked_sub(1),
                candidate.coord.x.checked_add(1),
            ]
            .into_iter()
            .flatten()
            {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if self.labels.contains(&CanvasCoord {
                    x,
                    ..candidate.coord
                }) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn divider_allows_terminal_marker(
        &self,
        protected: &ProtectedGeometry<'_>,
        plan: &RoutePlan,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
    ) -> bool {
        if protected.kind != ProtectedKind::GroupBorder
            || !protected
                .group_index
                .and_then(|index| self.graph_layout.groups.get(index))
                .is_some_and(|group| group.kind == GraphGroupKind::Divider)
        {
            return false;
        }
        let Some(attachments) = plan.attachments else {
            return false;
        };
        let attachment = match endpoint {
            MarkerEndpoint::Start => attachments.start,
            MarkerEndpoint::End => attachments.end,
        };
        let contact = attachment.contact;
        let coord = candidate.coord;
        let adjacent = (contact.x == coord.x && contact.y.abs_diff(coord.y) == 1)
            || (contact.y == coord.y && contact.x.abs_diff(coord.x) == 1);
        let directions = match candidate.point_direction {
            StepDirection::Up | StepDirection::Down => DIR_UP | DIR_DOWN,
            StepDirection::Left | StepDirection::Right => DIR_LEFT | DIR_RIGHT,
        };
        // An open separator can coincide with a primary berth immediately beside the
        // actual endpoint. The marker must retain a transverse, continuous attachment.
        adjacent
            && attachment.allows_escape(coord)
            && protected
                .shape
                .transverse_border_directions(coord, directions)
                .is_some()
    }

    pub(super) fn preserve_group_crossings(
        &self,
        plan: &mut RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        if owner.boundary_group_indices.is_empty() {
            return Ok(());
        }
        for cell in &mut plan.cells {
            self.checkpoint_layout()?;
            resources.charge_layout_work(self.protected.len())?;
            for (index, protected) in self.protected.iter().enumerate() {
                self.checkpoint_layout_scan(index)?;
                if protected.kind != ProtectedKind::GroupBorder
                    || protected.group_index.is_none_or(|group_index| {
                        owner
                            .boundary_group_indices
                            .binary_search(&group_index)
                            .is_err()
                    })
                {
                    continue;
                }
                let directions = route_char_directions(cell.ch);
                if let Some(border_directions) = protected
                    .shape
                    .transverse_border_directions(cell.coord, directions)
                {
                    // The route owns the branch; the compound frame keeps its thin border.
                    cell.ch = stroke_route_char(
                        super::super::model::GraphEdgeStroke::Normal,
                        directions | border_directions,
                        cell.unicode,
                    );
                    cell.kind = PlannedRouteCellKind::EdgeLine;
                }
            }
        }
        Ok(())
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
        if let Some(ports) = route.plan.attachments {
            resources.charge_layout_work(1)?;
            let count = self
                .target_ports
                .entry((ports.end.contact, ports.end.outward))
                .or_default();
            *count = resources.checked_work_add(*count, 1)?;
            if route.plan.requests_marker(MarkerEndpoint::Start) {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                try_reserve_hash_map(&mut self.source_ports, 1)?;
                let count = self
                    .source_ports
                    .entry((ports.start.contact, ports.start.outward))
                    .or_default();
                *count = resources.checked_work_add(*count, 1)?;
            }
        }
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
            self.occupied_extent.x = self.occupied_extent.x.max(cell.coord.x);
            self.occupied_extent.y = self.occupied_extent.y.max(cell.coord.y);
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
                    primary: true,
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
            let protected_left = title_x.saturating_sub(1).max(inner_left);
            let protected_right = resources
                .checked_grid_add(resources.checked_grid_add(title_x, title_width)?, 1)?
                .min(group.right());
            self.protected.push(ProtectedGeometry {
                owner_id: &group.id,
                group_index: Some(group_index),
                kind: ProtectedKind::GroupTitle,
                shape: ProtectedShape::Rect(OccupiedRect::try_new(
                    protected_left,
                    title_y,
                    protected_right - protected_left,
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
        if !self.marker_reading_clearance_is_clear(candidate, resources)? {
            return Ok(MarkerCandidateDisposition::Blocked);
        }
        let endpoint_id = routes[route_index].owner.endpoint_id(endpoint);
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.contains(candidate.coord)
                && !(candidate.is_primary()
                    && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                        || own_terminal_marker_escape(
                            protected,
                            endpoint_id,
                            &routes[route_index].plan,
                            endpoint,
                            candidate,
                        )
                        || self.divider_allows_terminal_marker(
                            protected,
                            &routes[route_index].plan,
                            endpoint,
                            candidate,
                        )
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
        let mut owns_candidate = false;
        for (owner_index, owner) in route_cell.owners.iter().enumerate() {
            self.checkpoint_layout_scan(owner_index)?;
            if owner.route_index == route_index && owner.cell == candidate.cell {
                owns_candidate = true;
                break;
            }
        }
        if !owns_candidate {
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
            let mut compatible = false;
            for (claim_index, claim) in claims.iter().enumerate() {
                self.checkpoint_layout_scan(claim_index)?;
                if claim.route_index == owner.route_index
                    && terminal_claim_is_compatible(
                        routes,
                        claim,
                        endpoint_id,
                        candidate.point_direction,
                    )
                {
                    compatible = true;
                    break;
                }
            }
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
        owner: &RouteOwner,
        anchor: LabelAnchor,
        candidate: OccupiedRect,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        let Some(reservation) =
            label_reservation(anchor, candidate, self.label_lane_radius, resources)?
        else {
            return Ok(false);
        };
        if !self.label_region_is_clear(owner, candidate, resources)? {
            return Ok(false);
        }
        for protected in &self.protected {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(reservation) {
                return Ok(false);
            }
        }

        if !self.label_reading_clearance_is_clear(candidate, resources)? {
            return Ok(false);
        }

        for y in reservation.y..reservation.bottom {
            for x in reservation.x..reservation.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord) || self.labels.contains(&coord) {
                    return Ok(false);
                }
                if let Some(route_cell) = self.route_cells.get(&coord) {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(route_cell.owners.len())?;
                    for (owner_index, owner) in route_cell.owners.iter().enumerate() {
                        self.checkpoint_layout_scan(owner_index)?;
                        if owner.route_index != route_index
                            || !label_anchor_contains(anchor, coord, owner.segment)
                        {
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn label_region_is_clear(
        &self,
        owner: &RouteOwner,
        candidate: OccupiedRect,
        resources: &ResourceContext,
    ) -> Result<bool> {
        for group in &self.graph_layout.groups {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container {
                continue;
            }
            let endpoint_inside =
                |id: &str| -> Result<bool> {
                    resources.charge_layout_work(resources.checked_work_add(
                        self.graph_layout.nodes.len(),
                        self.graph_layout.groups.len(),
                    )?)?;
                    Ok(self.graph_layout.nodes.iter().any(|node| {
                        node.id == id && super::geometry::group_contains_node(group, node)
                    }) || self.graph_layout.groups.iter().any(|child| {
                        child.id == id
                            && group.x < child.x
                            && child.right() < group.right()
                            && group.y < child.y
                            && child.bottom() < group.bottom()
                    }))
                };
            let from_inside = endpoint_inside(&owner.from)?;
            let to_inside = endpoint_inside(&owner.to)?;
            let interior = if group.id == owner.from {
                to_inside
            } else if group.id == owner.to {
                from_inside
            } else {
                from_inside && to_inside
            };
            if interior {
                if candidate.x <= group.x
                    || candidate.right > group.right()
                    || candidate.y <= group.y
                    || candidate.bottom > group.bottom()
                {
                    return Ok(false);
                }
            } else if (group.id == owner.from
                || group.id == owner.to
                || (!from_inside && !to_inside))
                && candidate.x <= group.right()
                && group.x < candidate.right
                && candidate.y <= group.bottom()
                && group.y < candidate.bottom
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn label_reading_clearance_is_clear(
        &self,
        candidate: OccupiedRect,
        resources: &ResourceContext,
    ) -> Result<bool> {
        // The ordinary footprint check owns the interior. Reserve only the two adjacent
        // columns here so independent labels require exactly one blank reading column.
        let right = resources.checked_grid_add(candidate.right, 1)?;
        for y in candidate.y..candidate.bottom {
            for x in [candidate.x.checked_sub(1), Some(right - 1)]
                .into_iter()
                .flatten()
            {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.labels.contains(&coord)
                    || self.markers.get(&coord).is_some_and(|marker| {
                        matches!(
                            marker.point_direction,
                            StepDirection::Up | StepDirection::Down
                        )
                    })
                {
                    return Ok(false);
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
        self.occupied_extent.x = self
            .occupied_extent
            .x
            .max(footprint.right.saturating_sub(1));
        self.occupied_extent.y = self
            .occupied_extent
            .y
            .max(footprint.bottom.saturating_sub(1));
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

fn try_reserve_hash_map<K, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()>
where
    K: Eq + std::hash::Hash,
{
    map.try_reserve(additional)
        .map_err(|_| layout_allocation_failed())
}
