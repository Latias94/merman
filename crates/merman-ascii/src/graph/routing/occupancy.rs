use super::super::layout::{CanvasCoord, GraphLayout};
use super::super::model::GraphGroupKind;
use super::cell::{
    DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP, route_char_directions, stroke_route_char,
};
use super::conflict::{BlockerWitnesses, ReservationKind};
use super::label::RoutedLabelPlacement;
use super::path::StepDirection;
use super::plan::{
    LabelAnchor, MAX_MARKER_CANDIDATES, MarkerCandidate, MarkerEndpoint, NodeAttachments,
    PlannedCellId, PlannedRouteCellKind, PlannedRouteSegment, RoutePlan,
};
use super::{PreparedRoute, RouteOwner, layout_allocation_failed};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::{OperationControl, OperationPhase};
use rustc_hash::FxHashMap as HashMap;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::rc::Rc;

mod channels;
mod labels;
mod marker;
mod protected;
mod rebuild;
mod regions;

pub(super) use protected::ProtectedScene;
use regions::LabelContainerRelation;
pub(super) use regions::LabelRegionCache;

const OCCUPANCY_SCAN_CHECKPOINT_INTERVAL: usize = 64;
type RouteLabelPlacements = Vec<(RoutedLabelPlacement, LabelAnchor)>;

pub(super) use labels::allocate_committed_route_labels;
#[cfg(test)]
pub(super) use labels::allocate_route_label_placements;
pub(in crate::graph::routing) use labels::{label_anchor_contains, label_reservation};
use labels::{resolve_label_anchor, route_label_candidates};
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

fn occupied_marker_disposition(
    routes: &[PreparedRoute],
    owner: &RouteOwner,
    endpoint: MarkerEndpoint,
    candidate: MarkerCandidate,
    marker: MarkerOccupant,
) -> MarkerCandidateDisposition {
    if !marker_occupant_is_compatible(
        routes,
        marker,
        owner.endpoint_id(endpoint),
        candidate.point_direction,
    ) {
        return MarkerCandidateDisposition::Blocked;
    }
    if marker.endpoint == endpoint
        && marker.shared_group.is_some()
        && marker.shared_group == owner.shared_marker_group(endpoint, candidate.coord)
    {
        MarkerCandidateDisposition::Available
    } else {
        MarkerCandidateDisposition::CompatiblePassThrough
    }
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

fn terminal_claim_is_transverse(claim: &TerminalClaim, directions: u8) -> bool {
    let claim_axis = match claim.point_direction {
        StepDirection::Left | StepDirection::Right => DIR_LEFT | DIR_RIGHT,
        StepDirection::Up | StepDirection::Down => DIR_UP | DIR_DOWN,
    };
    !claim.primary && super::cell::is_transverse_crossing(directions, claim_axis)
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
            // Alternative marker positions are opportunities, not occupied terminals.
            // A transverse crossing preserves the committed heads; later marker
            // admission checks the crossing's owner before using this position.
            if terminal_claim_is_transverse(claim, directions) {
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
    total_cost: usize,
    ambiguous_shared_cells: usize,
    shared_cells: usize,
    cell_count: usize,
    scene_growth: usize,
    marker_pressure: usize,
}

impl RouteCandidateScore {
    pub(super) fn port_reuse(self) -> (usize, usize) {
        (self.target_port_reuse, self.source_port_reuse)
    }

    pub(super) fn has_congestion(self) -> bool {
        self.target_port_reuse != 0
            || self.source_port_reuse != 0
            || self.ambiguous_shared_cells != 0
    }

    pub(super) fn grows_scene(self) -> bool {
        self.scene_growth != 0
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
    shared_group: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct LabelOccupant {
    reservation: OccupiedRect,
    canonical_edge_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelCellKind {
    Text,
    HostClearance,
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
    route_capacity: usize,
    pub(super) route_cells: HashMap<CanvasCoord, RouteCellOccupancy>,
    pub(super) route_bounds: Vec<Option<RouteBounds>>,
    terminal_claims: HashMap<CanvasCoord, Vec<TerminalClaim>>,
    target_ports: HashMap<(CanvasCoord, StepDirection), usize>,
    source_ports: HashMap<(CanvasCoord, StepDirection), usize>,
    markers: HashMap<CanvasCoord, MarkerOccupant>,
    // Includes each label's text and the clear perpendicular gap to its host.
    labels: HashMap<CanvasCoord, LabelCellKind>,
    label_owners: Vec<LabelOccupant>,
    blockers: RefCell<Option<BlockerWitnesses>>,
    pub(super) protected: Rc<ProtectedScene<'layout>>,
    label_regions: Rc<LabelRegionCache<'layout>>,
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
            let shares_allocated_channel = owner.shared_channel_group().is_some()
                && owner.shared_channel_group() == existing.owner.shared_channel_group();
            if !shares_terminal_ray && !shares_allocated_channel {
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
        let resources = execution.resource_context(resources, OperationPhase::Layout);
        let resources = &resources;
        let protected = Self::try_prepare_protection(graph_layout, resources)?;
        let label_regions = LabelRegionCache::try_new(graph_layout, 0, resources)?;
        Self::try_new_with_protection(
            label_regions,
            route_capacity,
            label_lane_radius,
            protected,
            resources,
            execution,
        )
    }

    pub(super) fn try_prepare_protection(
        graph_layout: &'layout GraphLayout,
        resources: &ResourceContext,
    ) -> Result<Rc<ProtectedScene<'layout>>> {
        Ok(Rc::new(ProtectedScene::try_new(graph_layout, resources)?))
    }

    pub(super) fn try_new_with_protection(
        label_regions: Rc<LabelRegionCache<'layout>>,
        route_capacity: usize,
        label_lane_radius: usize,
        protected: Rc<ProtectedScene<'layout>>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        resources.checkpoint()?;
        let marker_capacity = resources.checked_work_mul(route_capacity, 2)?;
        let occupied_extent = protected.extent;
        let mut scene = Self {
            graph_layout: label_regions.layout(),
            route_capacity,
            route_cells: HashMap::default(),
            route_bounds: Vec::new(),
            terminal_claims: HashMap::default(),
            target_ports: HashMap::default(),
            source_ports: HashMap::default(),
            markers: HashMap::default(),
            labels: HashMap::default(),
            label_owners: Vec::new(),
            blockers: RefCell::new(None),
            protected,
            label_regions,
            label_lane_radius,
            control: execution.cloned_control(),
            occupied_extent,
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
        Ok(scene)
    }

    pub(in crate::graph::routing) fn checkpoint_layout(&self) -> Result<()> {
        self.control
            .checkpoint_at(OperationPhase::Layout)
            .map_err(AsciiError::Cancelled)
    }

    /// Enables evidence only for the probes whose rejected reservations will be repaired.
    pub(super) fn begin_blocker_collection(&self, resources: &ResourceContext) -> Result<()> {
        self.checkpoint_layout()?;
        let witnesses = BlockerWitnesses::try_new(resources)?;
        *self.blockers.borrow_mut() = Some(witnesses);
        Ok(())
    }

    /// Disables collection and transfers bounded evidence to the repair planner.
    pub(super) fn take_blocker_collection(
        &self,
        resources: &ResourceContext,
    ) -> Result<Option<BlockerWitnesses>> {
        self.checkpoint_layout()?;
        resources.checkpoint()?;
        Ok(self.blockers.borrow_mut().take())
    }

    fn collecting_blockers(&self) -> bool {
        self.blockers.borrow().is_some()
    }

    fn record_semantic_blocker(
        &self,
        canonical_edge_index: usize,
        kind: ReservationKind,
        resources: &ResourceContext,
    ) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        self.checkpoint_layout()?;
        if let Some(witnesses) = self.blockers.borrow_mut().as_mut() {
            witnesses.record(canonical_edge_index, kind, resources)?;
        }
        Ok(())
    }

    fn record_route_blocker(
        &self,
        routes: &[PreparedRoute],
        route_index: usize,
        kind: ReservationKind,
        resources: &ResourceContext,
    ) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        let route = routes
            .get(route_index)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "graph",
                feature: "reservation without semantic route ownership",
            })?;
        self.record_semantic_blocker(route.owner.canonical_edge_index, kind, resources)
    }

    fn record_static_restriction(&self, resources: &ResourceContext) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        self.checkpoint_layout()?;
        resources.charge_layout_work(1)?;
        if let Some(witnesses) = self.blockers.borrow_mut().as_mut() {
            witnesses.static_restriction = true;
        }
        Ok(())
    }

    fn record_capacity_restriction(&self, resources: &ResourceContext) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        self.checkpoint_layout()?;
        resources.charge_layout_work(1)?;
        if let Some(witnesses) = self.blockers.borrow_mut().as_mut() {
            witnesses.capacity_restriction = true;
        }
        Ok(())
    }

    fn record_label_blockers(&self, coord: CanvasCoord, resources: &ResourceContext) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        for label in &self.label_owners {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if label.reservation.contains(coord.x, coord.y) {
                self.record_semantic_blocker(
                    label.canonical_edge_index,
                    ReservationKind::Label,
                    resources,
                )?;
            }
        }
        Ok(())
    }

    fn record_terminal_claim_blockers(
        &self,
        routes: &[PreparedRoute],
        owner: &RouteOwner,
        claims: &[TerminalClaim],
        directions: u8,
        resources: &ResourceContext,
    ) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        let mut unrelated = false;
        for claim in claims {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if terminal_claim_is_transverse(claim, directions) {
                continue;
            }
            let incident = routes.get(claim.route_index).is_some_and(|route| {
                let endpoint = route.owner.endpoint_id(claim.endpoint);
                endpoint == owner.from || endpoint == owner.to
            });
            if !incident {
                unrelated = true;
                self.record_route_blocker(
                    routes,
                    claim.route_index,
                    ReservationKind::Port,
                    resources,
                )?;
            }
        }
        if !unrelated {
            // Every claim is individually incident, but their mixture has no single
            // common endpoint. Both incident sets participate in this exact rejection.
            for claim in claims {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if !terminal_claim_is_transverse(claim, directions) {
                    self.record_route_blocker(
                        routes,
                        claim.route_index,
                        ReservationKind::Port,
                        resources,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn record_marker_blockers(
        &self,
        routes: &[PreparedRoute],
        coord: CanvasCoord,
        marker: MarkerOccupant,
        resources: &ResourceContext,
    ) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        self.record_route_blocker(
            routes,
            marker.route_index,
            ReservationKind::Marker,
            resources,
        )?;
        if marker.shared_group.is_some() {
            // A shared marker stores one representative. Its other materialized
            // contributors are equally responsible, even after replay changes that index.
            for route in routes {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if route.owner.shared_marker_group(marker.endpoint, coord) != marker.shared_group
                    || !route.plan.requests_marker(marker.endpoint)
                {
                    continue;
                }
                let candidate = route.plan.terminal_candidate(marker.endpoint, "graph")?;
                if candidate.coord == coord
                    && !route.plan.is_cell_suppressed(candidate.cell)
                    && route
                        .plan
                        .cells
                        .get(candidate.cell.index())
                        .is_some_and(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
                {
                    self.record_semantic_blocker(
                        route.owner.canonical_edge_index,
                        ReservationKind::Marker,
                        resources,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn record_reserved_cell_blockers(
        &self,
        routes: &[PreparedRoute],
        coord: CanvasCoord,
        resources: &ResourceContext,
    ) -> Result<()> {
        if !self.collecting_blockers() {
            return Ok(());
        }
        if let Some(marker) = self.markers.get(&coord) {
            self.record_marker_blockers(routes, coord, *marker, resources)?;
        }
        if self.labels.contains_key(&coord) {
            self.record_label_blockers(coord, resources)?;
        }
        if let Some(cell) = self.route_cells.get(&coord) {
            for owner in &cell.owners {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                self.record_route_blocker(
                    routes,
                    owner.route_index,
                    ReservationKind::Route,
                    resources,
                )?;
            }
        }
        Ok(())
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

    #[cfg(test)]
    pub(super) fn score_route(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<RouteCandidateScore>> {
        Ok(self
            .evaluate_route(existing_routes, plan, owner, resources, diagram_type)?
            .map(|(score, _)| score))
    }

    pub(super) fn admit_route(
        &self,
        existing_routes: &[PreparedRoute],
        mut plan: RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
        let Some((score, placements)) =
            self.evaluate_route(existing_routes, &plan, owner, resources, diagram_type)?
        else {
            return Ok(None);
        };
        for (label, (placement, anchor)) in plan.labels.iter_mut().zip(placements) {
            label.placement = placement;
            label.anchor = anchor;
        }
        Ok(Some((score, plan)))
    }

    fn evaluate_route(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<(RouteCandidateScore, RouteLabelPlacements)>> {
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
                let penalty = self.route_cell_congestion(
                    existing_routes,
                    owner,
                    plan.attachments,
                    cell.coord,
                    resources,
                )?;
                if owner.shared_channel_group().is_none() || penalty != 0 {
                    shared_cells = resources.checked_work_add(shared_cells, 1)?;
                }
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
                self.record_capacity_restriction(resources)?;
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
            self.record_capacity_restriction(resources)?;
            return Ok(None);
        }
        let Some(placements) =
            self.plan_label_placements(existing_routes, plan, owner, marker_berths, resources)?
        else {
            return Ok(None);
        };

        self.checkpoint_layout()?;
        let mut cell_count = 0usize;
        for (cell_index, _) in plan.active_cells().enumerate() {
            self.checkpoint_layout_scan(cell_index)?;
            cell_count = resources.checked_work_add(cell_count, 1)?;
        }
        let mut total_cost = resources.checked_work_add(cell_count, shared_cells)?;
        let scene_growth = {
            let (mut width, mut height) = plan.body_extent_with_resources(resources)?;
            for (label, (placement, _)) in plan.labels.iter().zip(&placements) {
                width = width.max(resources.checked_grid_add(placement.x(), placement.width())?);
                height = height.max(resources.checked_grid_add(placement.y(), label.line_count())?);
            }
            let current_width = resources.checked_grid_add(self.occupied_extent.x, 1)?;
            let current_height = resources.checked_grid_add(self.occupied_extent.y, 1)?;
            let current_area = resources.checked_work_mul(current_width, current_height)?;
            let candidate_area =
                resources.checked_work_mul(width.max(current_width), height.max(current_height))?;
            let scene_growth = candidate_area - current_area;
            total_cost = resources.checked_work_add(total_cost, scene_growth)?;
            scene_growth
        };
        let target_port_reuse = plan
            .attachments
            .map_or(0, |ports| self.target_port_usage(ports.end));
        let source_port_reuse = if plan.requests_marker(MarkerEndpoint::Start) {
            plan.attachments
                .map_or(0, |ports| self.source_port_usage(ports.start))
        } else {
            0
        };
        Ok(Some((
            RouteCandidateScore {
                target_port_reuse,
                source_port_reuse,
                ambiguous_shared_cells,
                total_cost,
                shared_cells,
                cell_count,
                scene_growth,
                marker_pressure,
            },
            placements,
        )))
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
        if self.labels.contains_key(&coord) {
            self.record_label_blockers(coord, resources)?;
            return Ok(false);
        }
        if !self.labels.is_empty() && directions & (DIR_UP | DIR_DOWN) != 0 {
            for x in [coord.x.checked_sub(1), coord.x.checked_add(1)]
                .into_iter()
                .flatten()
            {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let neighbor = CanvasCoord { x, ..coord };
                if self.labels.get(&neighbor) == Some(&LabelCellKind::Text) {
                    self.record_label_blockers(neighbor, resources)?;
                    return Ok(false);
                }
            }
        }
        if !self.protected.all_at(coord, resources, |protected| {
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
                    return false;
                }
            }
            let is_own_terminal_escape = protected.kind == ProtectedKind::Node
                && attachments.is_some_and(|attachments| {
                    protected.owner_id == owner.from && attachments.start.allows_escape(coord)
                        || protected.owner_id == owner.to && attachments.end.allows_escape(coord)
                });
            !protected.shape.contains(coord)
                || is_endpoint_port
                || is_own_terminal_escape
                || is_owned_group_border
        })? {
            self.record_static_restriction(resources)?;
            return Ok(false);
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
                self.record_marker_blockers(existing_routes, coord, *marker, resources)?;
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
            self.record_terminal_claim_blockers(
                existing_routes,
                owner,
                claims,
                directions,
                resources,
            )?;
            return Ok(false);
        }
        let mut clear = true;
        if let Some(occupied) = self.route_cells.get(&coord) {
            self.checkpoint_layout()?;
            resources.charge_layout_work(occupied.owners.len().max(1))?;
            for (owner_index, shared_owner) in occupied.owners.iter().enumerate() {
                if self.collecting_blockers() {
                    self.checkpoint_layout_scan(owner_index)?;
                }
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
                let shares_allocated_channel = owner.shared_channel_group().is_some()
                    && owner.shared_channel_group() == existing.owner.shared_channel_group();
                let shares_terminal_ray = attachments.zip(existing.plan.attachments).is_some_and(
                    |(current, previous)| {
                        (owner.from == existing.owner.from
                            && current.start == previous.start
                            && current.start.allows_escape(coord))
                            || (owner.to == existing.owner.to
                                && current.end == previous.end
                                && current.end.allows_escape(coord))
                    },
                );
                let unrelated_sharing = !shares_allocated_channel
                    && !shares_terminal_ray
                    && !shared_contact
                    && !transverse;
                let unrelated_channel = (owner.shared_channel_group().is_some()
                    || existing.owner.shared_channel_group().is_some())
                    && !shares_allocated_channel
                    && !shared_contact
                    && !transverse;
                let conflicting_stroke =
                    owner.stroke != existing.owner.stroke && !shared_contact && !transverse;
                let conflicting_direction = reverse_pair && !shared_contact && !transverse;
                // Collinear sharing, channel membership, stroke identity, and authored
                // direction use the same guards with or without witness collection.
                if unrelated_sharing
                    || unrelated_channel
                    || conflicting_stroke
                    || conflicting_direction
                {
                    self.record_route_blocker(
                        existing_routes,
                        shared_owner.route_index,
                        ReservationKind::Route,
                        resources,
                    )?;
                    if !self.collecting_blockers() {
                        return Ok(false);
                    }
                    clear = false;
                }
            }
        }
        Ok(clear)
    }

    fn plan_label_placements(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        marker_berths: [Option<CanvasCoord>; 2],
        resources: &mut ResourceContext,
    ) -> Result<Option<RouteLabelPlacements>> {
        resources.charge_layout_work(plan.labels.len())?;
        let mut placements = Vec::new();
        placements
            .try_reserve(plan.labels.len())
            .map_err(|_| layout_allocation_failed())?;
        if plan.labels.is_empty() {
            return Ok(Some(placements));
        }
        let reading_guards = self.label_reading_guards(plan, resources)?;

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
        let body_width = resources.checked_grid_add(
            route_bounds.map_or(self.occupied_extent.x, |bounds| {
                bounds.max_x.max(self.occupied_extent.x)
            }),
            1,
        )?;
        let body_height = resources.checked_grid_add(
            route_bounds.map_or(self.occupied_extent.y, |bounds| {
                bounds.max_y.max(self.occupied_extent.y)
            }),
            1,
        )?;

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
            if owner.shared_channel_group().is_some()
                && let LabelAnchor::Segment { start, end, .. } = anchor
            {
                // A condition on a common trunk would appear to annotate other
                // branches. Its host must be private apart from its end junctions.
                for (_, cell) in plan.active_cells() {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(1)?;
                    if cell.coord != start
                        && cell.coord != end
                        && label_anchor_contains(anchor, cell.coord, cell.segment)
                        && self.route_cells.contains_key(&cell.coord)
                    {
                        if self.collecting_blockers() {
                            self.record_reserved_cell_blockers(
                                existing_routes,
                                cell.coord,
                                resources,
                            )?;
                        }
                        return Ok(None);
                    }
                }
            }
            let candidates = route_label_candidates(
                label.placement,
                label.line_count(),
                anchor,
                route_bounds,
                resources,
                self.label_lane_radius,
            )?;
            let mut selected = None;
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
                    && self.plan_label_candidate_is_clear(
                        existing_routes,
                        plan,
                        anchor,
                        footprint,
                        &reading_guards,
                        resources,
                    )?
                {
                    // The route already needs its full envelope. Ignoring it here can
                    // float a condition away from its host without saving a single cell.
                    let width = footprint.right.max(body_width);
                    let height = footprint.bottom.max(body_height);
                    let area = resources.checked_work_mul(width, height)?;
                    if selected.as_ref().is_none_or(|(best, _)| area < *best) {
                        selected = Some((area, candidate));
                    }
                }
            }
            let Some((_, placement)) = selected else {
                return Ok(None);
            };
            placements.push((placement, anchor));
        }
        Ok(Some(placements))
    }

    fn plan_label_candidate_is_clear(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        anchor: LabelAnchor,
        candidate: OccupiedRect,
        reading_guards: &[CanvasCoord],
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        if !self.label_preserves_route_reading(reading_guards, candidate, resources)? {
            return Ok(false);
        }
        let Some(reservation) =
            label_reservation(anchor, candidate, self.label_lane_radius, resources)?
        else {
            return Ok(false);
        };
        if !self.protected.all_in(reservation, resources, |protected| {
            !protected.shape.intersects(reservation)
        })? {
            self.record_static_restriction(resources)?;
            return Ok(false);
        }

        if !self.label_reading_clearance_is_clear(existing_routes, None, candidate, resources)? {
            return Ok(false);
        }

        let mut clear = true;
        for y in reservation.y..reservation.bottom {
            for x in reservation.x..reservation.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord)
                    || self.labels.contains_key(&coord)
                    || self.route_cells.contains_key(&coord)
                {
                    self.record_reserved_cell_blockers(existing_routes, coord, resources)?;
                    if !self.collecting_blockers() {
                        return Ok(false);
                    }
                    clear = false;
                }
            }
        }
        if !clear {
            return Ok(false);
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

    pub(super) fn label_reading_guards(
        &self,
        plan: &RoutePlan,
        resources: &ResourceContext,
    ) -> Result<Vec<CanvasCoord>> {
        let mut borders = Vec::new();
        for protected in self.protected.iter() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if protected.kind == ProtectedKind::GroupBorder {
                borders
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                borders.push(protected);
            }
        }
        let mut guards = Vec::new();
        // A label can replace its own host stroke, but not the immediate continuation
        // of a frame crossing: otherwise the frame looks connected to the label itself.
        for (_, cell) in plan.active_cells() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            let directions = if cell.directions == 0 {
                route_char_directions(cell.ch)
            } else {
                cell.directions
            };
            if directions.count_ones() > 1
                && directions != (DIR_LEFT | DIR_RIGHT)
                && directions != (DIR_UP | DIR_DOWN)
            {
                // Hiding a turn makes a condition appear to connect two unrelated
                // continuation columns. Inline text may replace only a straight host.
                guards
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                guards.push(cell.coord);
            }
            for protected in &borders {
                resources.charge_layout_work(1)?;
                let Some(border) = protected
                    .shape
                    .transverse_border_directions(cell.coord, directions)
                else {
                    continue;
                };
                let neighbors = if border == (DIR_LEFT | DIR_RIGHT) {
                    [
                        cell.coord
                            .y
                            .checked_sub(1)
                            .map(|y| CanvasCoord { y, ..cell.coord }),
                        cell.coord
                            .y
                            .checked_add(1)
                            .map(|y| CanvasCoord { y, ..cell.coord }),
                    ]
                } else {
                    [
                        cell.coord
                            .x
                            .checked_sub(1)
                            .map(|x| CanvasCoord { x, ..cell.coord }),
                        cell.coord
                            .x
                            .checked_add(1)
                            .map(|x| CanvasCoord { x, ..cell.coord }),
                    ]
                };
                for coord in neighbors.into_iter().flatten() {
                    guards
                        .try_reserve(1)
                        .map_err(|_| layout_allocation_failed())?;
                    guards.push(coord);
                }
            }
        }
        crate::graph::layout::charge_sort_work(guards.len(), resources)?;
        guards.sort_unstable_by_key(|coord| (coord.y, coord.x));
        guards.dedup();
        Ok(guards)
    }

    pub(super) fn label_preserves_route_reading(
        &self,
        guards: &[CanvasCoord],
        candidate: OccupiedRect,
        resources: &ResourceContext,
    ) -> Result<bool> {
        for coord in guards {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if candidate.contains(coord.x, coord.y) {
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
        if !self.marker_reading_clearance_is_clear(
            existing_routes,
            owner,
            plan,
            endpoint,
            candidate,
            resources,
        )? {
            return Ok(MarkerCandidateDisposition::Blocked);
        }
        let endpoint_id = owner.endpoint_id(endpoint);
        if !self
            .protected
            .all_at(candidate.coord, resources, |protected| {
                !protected.shape.contains(candidate.coord)
                    || (candidate.is_primary()
                        && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                            || own_terminal_marker_escape(
                                protected,
                                endpoint_id,
                                plan,
                                endpoint,
                                candidate,
                            )
                            || self.divider_allows_terminal_marker(
                                protected, plan, endpoint, candidate,
                            )
                            || (protected.kind == ProtectedKind::GroupBorder
                                && (protected.allows_endpoint_port(&owner.from, candidate.coord)
                                    || protected
                                        .allows_endpoint_port(&owner.to, candidate.coord)))))
            })?
        {
            self.record_static_restriction(resources)?;
            return Ok(MarkerCandidateDisposition::Blocked);
        }

        if let Some(marker) = self.markers.get(&candidate.coord) {
            let disposition =
                occupied_marker_disposition(existing_routes, owner, endpoint, candidate, *marker);
            if disposition == MarkerCandidateDisposition::Blocked {
                self.record_marker_blockers(existing_routes, candidate.coord, *marker, resources)?;
            }
            return Ok(disposition);
        }

        let claims = self.terminal_claims.get(&candidate.coord);
        if let Some(claims) = claims {
            let mut clear = true;
            for claim in claims {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                if !terminal_claim_is_compatible(
                    existing_routes,
                    claim,
                    endpoint_id,
                    candidate.point_direction,
                ) {
                    self.record_route_blocker(
                        existing_routes,
                        claim.route_index,
                        ReservationKind::Port,
                        resources,
                    )?;
                    if !self.collecting_blockers() {
                        return Ok(MarkerCandidateDisposition::Blocked);
                    }
                    clear = false;
                }
            }
            if !clear {
                return Ok(MarkerCandidateDisposition::Blocked);
            }
        }

        if let Some(cell) = self.route_cells.get(&candidate.coord) {
            let Some(claims) = claims else {
                if self.collecting_blockers() {
                    for cell_owner in &cell.owners {
                        self.checkpoint_layout()?;
                        resources.charge_layout_work(1)?;
                        self.record_route_blocker(
                            existing_routes,
                            cell_owner.route_index,
                            ReservationKind::Route,
                            resources,
                        )?;
                    }
                }
                return Ok(MarkerCandidateDisposition::Blocked);
            };
            let mut clear = true;
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
                    self.record_route_blocker(
                        existing_routes,
                        cell_owner.route_index,
                        ReservationKind::Route,
                        resources,
                    )?;
                    if !self.collecting_blockers() {
                        return Ok(MarkerCandidateDisposition::Blocked);
                    }
                    clear = false;
                }
            }
            if !clear {
                return Ok(MarkerCandidateDisposition::Blocked);
            }
        }

        Ok(MarkerCandidateDisposition::Available)
    }

    fn marker_reading_clearance_is_clear(
        &self,
        routes: &[PreparedRoute],
        owner: &RouteOwner,
        plan: &RoutePlan,
        endpoint: MarkerEndpoint,
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
                let coord = CanvasCoord {
                    x,
                    ..candidate.coord
                };
                if self.labels.contains_key(&coord) {
                    self.record_label_blockers(coord, resources)?;
                    return Ok(false);
                }
            }
        }
        let neighbors = match candidate.point_direction {
            StepDirection::Left | StepDirection::Right => [
                candidate.coord.x.checked_sub(1).map(|x| CanvasCoord {
                    x,
                    ..candidate.coord
                }),
                candidate.coord.x.checked_add(1).map(|x| CanvasCoord {
                    x,
                    ..candidate.coord
                }),
            ],
            StepDirection::Up | StepDirection::Down => [
                candidate.coord.y.checked_sub(1).map(|y| CanvasCoord {
                    y,
                    ..candidate.coord
                }),
                candidate.coord.y.checked_add(1).map(|y| CanvasCoord {
                    y,
                    ..candidate.coord
                }),
            ],
        };
        for coord in neighbors.into_iter().flatten() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if let Some(marker) = self.markers.get(&coord) {
                let existing = &routes[marker.route_index];
                if marker_occupant_is_compatible(
                    routes,
                    *marker,
                    owner.endpoint_id(endpoint),
                    candidate.point_direction,
                ) && (owner.from != existing.owner.from
                    || owner.to != existing.owner.to
                    || !plan.labels.is_empty()
                    || !existing.plan.labels.is_empty())
                {
                    self.record_marker_blockers(routes, coord, *marker, resources)?;
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
                    cell.directions = directions;
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
        self.commit_route_geometry(route_index, route, resources)?;
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

    fn commit_route_geometry(
        &mut self,
        route_index: usize,
        route: &PreparedRoute,
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

    fn marker_candidate_disposition(
        &self,
        routes: &[PreparedRoute],
        route_index: usize,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
        resources: &mut ResourceContext,
    ) -> Result<MarkerCandidateDisposition> {
        if !self.marker_reading_clearance_is_clear(
            routes,
            &routes[route_index].owner,
            &routes[route_index].plan,
            endpoint,
            candidate,
            resources,
        )? {
            return Ok(MarkerCandidateDisposition::Blocked);
        }
        let endpoint_id = routes[route_index].owner.endpoint_id(endpoint);
        if !self
            .protected
            .all_at(candidate.coord, resources, |protected| {
                !protected.shape.contains(candidate.coord)
                    || (candidate.is_primary()
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
            })?
        {
            self.record_static_restriction(resources)?;
            return Ok(MarkerCandidateDisposition::Blocked);
        }

        if let Some(marker) = self.markers.get(&candidate.coord) {
            let disposition = occupied_marker_disposition(
                routes,
                &routes[route_index].owner,
                endpoint,
                candidate,
                *marker,
            );
            if disposition == MarkerCandidateDisposition::Blocked {
                self.record_marker_blockers(routes, candidate.coord, *marker, resources)?;
            }
            return Ok(disposition);
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
            if route_cell.owners.len() != 1 && self.collecting_blockers() {
                for owner in &route_cell.owners {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(1)?;
                    if owner.route_index != route_index {
                        self.record_route_blocker(
                            routes,
                            owner.route_index,
                            ReservationKind::Route,
                            resources,
                        )?;
                    }
                }
            }
            return Ok(if route_cell.owners.len() == 1 {
                MarkerCandidateDisposition::Available
            } else {
                MarkerCandidateDisposition::Blocked
            });
        };
        let mut clear = true;
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
                self.record_route_blocker(
                    routes,
                    owner.route_index,
                    ReservationKind::Route,
                    resources,
                )?;
                if !self.collecting_blockers() {
                    return Ok(MarkerCandidateDisposition::Blocked);
                }
                clear = false;
            }
        }
        Ok(if clear {
            MarkerCandidateDisposition::Available
        } else {
            MarkerCandidateDisposition::Blocked
        })
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
                if marker.shared_group.is_some()
                    && marker.shared_group == existing.get().shared_group
                    && marker.endpoint == existing.get().endpoint
                    && marker.point_direction == existing.get().point_direction
                {
                    // Each route retains its materialized marker. The map only needs a
                    // representative; replay chooses one again if that route is removed.
                    return Ok(());
                }
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
        existing_routes: &[PreparedRoute],
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
        if !self.protected.all_in(reservation, resources, |protected| {
            !protected.shape.intersects(reservation)
        })? {
            self.record_static_restriction(resources)?;
            return Ok(false);
        }

        if !self.label_reading_clearance_is_clear(
            existing_routes,
            Some(route_index),
            candidate,
            resources,
        )? {
            return Ok(false);
        }

        let mut clear = true;
        for y in reservation.y..reservation.bottom {
            for x in reservation.x..reservation.right {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord) || self.labels.contains_key(&coord) {
                    if let Some(marker) = self.markers.get(&coord) {
                        self.record_marker_blockers(existing_routes, coord, *marker, resources)?;
                    }
                    if self.labels.contains_key(&coord) {
                        self.record_label_blockers(coord, resources)?;
                    }
                    if !self.collecting_blockers() {
                        return Ok(false);
                    }
                    clear = false;
                }
                if let Some(route_cell) = self.route_cells.get(&coord) {
                    self.checkpoint_layout()?;
                    resources.charge_layout_work(route_cell.owners.len())?;
                    for (owner_index, owner) in route_cell.owners.iter().enumerate() {
                        self.checkpoint_layout_scan(owner_index)?;
                        if owner.route_index != route_index
                            || !label_anchor_contains(anchor, coord, owner.segment)
                        {
                            self.record_route_blocker(
                                existing_routes,
                                owner.route_index,
                                ReservationKind::Route,
                                resources,
                            )?;
                            if !self.collecting_blockers() {
                                return Ok(false);
                            }
                            clear = false;
                        }
                    }
                }
            }
        }
        Ok(clear)
    }

    fn label_region_is_clear(
        &self,
        owner: &RouteOwner,
        candidate: OccupiedRect,
        resources: &ResourceContext,
    ) -> Result<bool> {
        for (group_index, group) in self.graph_layout.groups.iter().enumerate() {
            self.checkpoint_layout()?;
            resources.charge_layout_work(1)?;
            if group.kind != GraphGroupKind::Container {
                continue;
            }
            let relation = self
                .label_regions
                .classify(owner, group_index, group, resources)?;
            if relation == LabelContainerRelation::Interior {
                if candidate.x <= group.x
                    || candidate.right > group.right()
                    || candidate.y <= group.y
                    || candidate.bottom > group.bottom()
                {
                    self.record_static_restriction(resources)?;
                    return Ok(false);
                }
            } else if relation == LabelContainerRelation::Exterior
                && candidate.x <= group.right()
                && group.x < candidate.right
                && candidate.y <= group.bottom()
                && group.y < candidate.bottom
            {
                self.record_static_restriction(resources)?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn label_reading_clearance_is_clear(
        &self,
        existing_routes: &[PreparedRoute],
        route_index: Option<usize>,
        candidate: OccupiedRect,
        resources: &ResourceContext,
    ) -> Result<bool> {
        // The ordinary footprint check owns the interior. Reserve only the two adjacent
        // columns here so independent labels, vertical stems, and vertical markers
        // retain one blank reading column without blocking inline horizontal ink.
        let right = resources.checked_grid_add(candidate.right, 1)?;
        let mut clear = true;
        for y in candidate.y..candidate.bottom {
            for x in [candidate.x.checked_sub(1), Some(right - 1)]
                .into_iter()
                .flatten()
            {
                self.checkpoint_layout()?;
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if let Some(cell) = self.route_cells.get(&coord) {
                    for owner in &cell.owners {
                        self.checkpoint_layout()?;
                        resources.charge_layout_work(1)?;
                        let cell =
                            &existing_routes[owner.route_index].plan.cells[owner.cell.index()];
                        let directions = if cell.directions == 0 {
                            route_char_directions(cell.ch)
                        } else {
                            cell.directions
                        };
                        if Some(owner.route_index) != route_index
                            && directions & (DIR_UP | DIR_DOWN) != 0
                        {
                            self.record_route_blocker(
                                existing_routes,
                                owner.route_index,
                                ReservationKind::Route,
                                resources,
                            )?;
                            if !self.collecting_blockers() {
                                return Ok(false);
                            }
                            clear = false;
                        }
                    }
                }
                if self.labels.contains_key(&coord)
                    || self.markers.get(&coord).is_some_and(|marker| {
                        matches!(
                            marker.point_direction,
                            StepDirection::Up | StepDirection::Down
                        )
                    })
                {
                    if self.labels.contains_key(&coord) {
                        self.record_label_blockers(coord, resources)?;
                    }
                    if let Some(marker) = self.markers.get(&coord)
                        && matches!(
                            marker.point_direction,
                            StepDirection::Up | StepDirection::Down
                        )
                    {
                        self.record_marker_blockers(existing_routes, coord, *marker, resources)?;
                    }
                    if !self.collecting_blockers() {
                        return Ok(false);
                    }
                    clear = false;
                }
            }
        }
        Ok(clear)
    }

    fn occupy_label(
        &mut self,
        text: OccupiedRect,
        footprint: OccupiedRect,
        canonical_edge_index: usize,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let footprint_cells = footprint.cell_count(resources)?;
        self.checkpoint_layout()?;
        resources.charge_layout_work(1)?;
        self.label_owners
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        self.label_owners.push(LabelOccupant {
            reservation: footprint,
            canonical_edge_index,
        });
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
                let kind = if text.contains(x, y) {
                    LabelCellKind::Text
                } else {
                    LabelCellKind::HostClearance
                };
                self.labels.insert(CanvasCoord { x, y }, kind);
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
