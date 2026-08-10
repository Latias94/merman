use super::charset::GraphCharset;
use super::label::{GRAPH_LABEL_LINE_GAP, GraphLabel};
use super::layout::{CanvasCoord, GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{
    AsciiGraph, AsciiGraphEdge, GraphEdgeMarker, GraphEdgeStroke, GraphGroupKind, GraphNodeShape,
    GraphNodeStyle,
};
use super::surface::GraphSurface;
use super::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::canvas::Canvas as RawCanvas;
#[cfg(test)]
use crate::canvas::CanvasColor;
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::cmp::Ordering;
use std::collections::{HashMap, hash_map::Entry};

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_cell_with_paint, set_route_cell_with_paint};
use label::{EdgeLabel, RoutedLabelPlacement, draw_routed_label};
use path::StepDirection;
#[cfg(test)]
use plan::plan_edge_route;
use plan::{
    EdgeRouteCandidates, EdgeRouteRequest, LabelAnchor, MAX_MARKER_CANDIDATES, MarkerCandidate,
    MarkerEndpoint, PlannedCellId, PlannedRouteCellKind, PlannedRouteSegment, RoutePlan,
    plan_edge_route_candidates_with_topology,
};

type Canvas<'surface> = dyn GraphSurface + 'surface;

pub(super) struct RouteDrawing<'a> {
    canvas: &'a mut Canvas<'a>,
    route_cells: &'a mut RouteCells,
}

impl<'a> RouteDrawing<'a> {
    pub(super) fn new(canvas: &'a mut Canvas<'a>, route_cells: &'a mut RouteCells) -> Self {
        Self {
            canvas,
            route_cells,
        }
    }
}

pub(super) struct RouteScene {
    routes: Vec<PreparedRoute>,
    extent: (usize, usize),
    planned_cell_count: usize,
}

struct PreparedRoute {
    plan: RoutePlan,
    owner: RouteOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOwner {
    canonical_edge_index: usize,
    source_edge_index: usize,
    from: String,
    to: String,
    boundary_group_indices: Vec<usize>,
}

impl RouteOwner {
    fn endpoint_id(&self, endpoint: MarkerEndpoint) -> &str {
        match endpoint {
            MarkerEndpoint::Start => &self.from,
            MarkerEndpoint::End => &self.to,
        }
    }

    #[cfg(test)]
    fn for_test(canonical_edge_index: usize, from: &str, to: &str) -> Self {
        Self {
            canonical_edge_index,
            source_edge_index: canonical_edge_index,
            from: from.to_string(),
            to: to.to_string(),
            boundary_group_indices: Vec::new(),
        }
    }
}

impl PreparedRoute {
    #[cfg(test)]
    fn for_test(plan: RoutePlan, canonical_edge_index: usize) -> Self {
        Self::for_test_with_endpoints(plan, canonical_edge_index, "source", "target")
    }

    #[cfg(test)]
    fn for_test_with_endpoints(
        plan: RoutePlan,
        canonical_edge_index: usize,
        from: &str,
        to: &str,
    ) -> Self {
        Self {
            plan,
            owner: RouteOwner::for_test(canonical_edge_index, from, to),
        }
    }

    fn paint_body(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        paint_route_plan_body(drawing, &self.plan)
    }

    fn paint_markers(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        paint_route_plan_markers(drawing, &self.plan)
    }
}

impl RouteScene {
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        self.extent
    }

    pub(super) fn planned_cell_count(&self) -> usize {
        self.planned_cell_count
    }

    pub(super) fn paint_routes(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        for route in &self.routes {
            route.paint_body(drawing)?;
        }
        for route in &self.routes {
            route.paint_markers(drawing)?;
        }
        Ok(())
    }

    pub(super) fn draw_labels(
        &self,
        canvas: &mut RawCanvas,
        transform: RouteLabelTransform,
    ) -> Result<()> {
        for route in &self.routes {
            for label in &route.plan.labels {
                let label = transform.apply(EdgeLabel {
                    text: label.text.clone(),
                    placement: label.placement,
                    color: label.paint.color,
                });
                draw_routed_label(canvas, &label)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RouteLabelTransform {
    Identity,
    HorizontalMirror { width: usize },
    VerticalMirror { height: usize },
}

impl RouteLabelTransform {
    fn apply(self, label: EdgeLabel) -> EdgeLabel {
        match self {
            Self::Identity => label,
            Self::HorizontalMirror { width } => EdgeLabel {
                placement: label.placement.with_position(
                    width
                        .saturating_sub(label.placement.x())
                        .saturating_sub(label.placement.width()),
                    label.placement.y(),
                ),
                ..label
            },
            Self::VerticalMirror { height } => {
                let line_count = label.text.line_count();
                EdgeLabel {
                    text: label.text.reversed(),
                    placement: label.placement.with_position(
                        label.placement.x(),
                        height.saturating_sub(label.placement.y().saturating_add(line_count)),
                    ),
                    color: label.color,
                }
            }
        }
    }
}

pub(super) fn prepare_route_scene_with_resources(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<RouteScene> {
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    let (canonical_edges, source_edge_indices) = canonicalize_edges(edges, resources)?;
    let mut routes = Vec::new();
    routes
        .try_reserve(canonical_edges.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let route_scan_width = graph_layout
        .nodes
        .len()
        .checked_add(graph_layout.groups.len())
        .and_then(|count| count.checked_add(canonical_edges.len()))
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;

    let mut occupancy =
        SceneOccupancy::try_new_for_routes(graph_layout, canonical_edges.len(), resources)?;

    for (edge_index, edge) in canonical_edges.iter().enumerate() {
        if edge.stroke == super::model::GraphEdgeStroke::Invisible {
            continue;
        }
        resources.charge_layout_work(route_scan_width)?;
        let Some(from) = endpoint_layout(graph_layout, &edge.from, charset) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let Some(to) = endpoint_layout(graph_layout, &edge.to, charset) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let candidates = match plan_edge_route_candidates_with_topology(
            EdgeRouteRequest {
                graph,
                graph_layout,
                edges: &canonical_edges,
                from: &from,
                to: &to,
                edge_index,
                edge,
                charset,
            },
            topology.as_ref(),
            resources,
        )? {
            EdgeRouteCandidates::Routed(candidates) => candidates,
            EdgeRouteCandidates::Unsupported(route) => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: graph.diagram_type(),
                    feature: route.feature(),
                });
            }
        };

        let boundary_group_indices =
            route_boundary_group_indices(graph, topology.as_ref(), edge, resources)?;
        let owner = RouteOwner {
            canonical_edge_index: edge_index,
            source_edge_index: source_edge_indices[edge_index],
            from: edge.from.clone(),
            to: edge.to.clone(),
            boundary_group_indices,
        };
        let mut selected = None::<(RouteCandidateScore, usize, RoutePlan)>;
        for (candidate_index, plan) in candidates.into_iter().enumerate() {
            let plan = plan
                .with_marker_requests(edge.start_marker, edge.end_marker, graph.diagram_type())?
                .with_style(edge.style);
            let Some(score) =
                occupancy.score_route(&routes, &plan, &owner, resources, graph.diagram_type())?
            else {
                continue;
            };
            if selected
                .as_ref()
                .is_none_or(|(current_score, current_index, _)| {
                    (score, candidate_index) < (*current_score, *current_index)
                })
            {
                selected = Some((score, candidate_index, plan));
            }
        }
        let Some((_, _, plan)) = selected else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "routes crossing reserved graph geometry",
            });
        };
        let start = plan.terminal_candidate(MarkerEndpoint::Start, graph.diagram_type())?;
        let end = plan.terminal_candidate(MarkerEndpoint::End, graph.diagram_type())?;
        let prepared = PreparedRoute { plan, owner };
        let route_index = routes.len();
        occupancy.commit_route(route_index, &prepared, start, end, resources)?;
        routes.push(prepared);
    }

    allocate_marker_berths(
        &mut routes,
        &mut occupancy,
        charset,
        resources,
        graph.diagram_type(),
    )?;
    allocate_route_label_placements(&mut routes, &mut occupancy, resources, graph.diagram_type())?;

    let mut width = 0;
    let mut height = 0;
    let mut planned_cell_count = 0usize;
    for route in &routes {
        planned_cell_count = planned_cell_count
            .checked_add(route.plan.active_cells().count())
            .ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        let (plan_width, plan_height) = route.plan.canvas_extent_with_resources(resources)?;
        width = width.max(plan_width);
        height = height.max(plan_height);
    }

    Ok(RouteScene {
        routes,
        extent: (width, height),
        planned_cell_count,
    })
}

fn route_boundary_group_indices(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    edge: &AsciiGraphEdge,
    resources: &mut ResourceContext,
) -> Result<Vec<usize>> {
    let Some(topology) = topology else {
        return Ok(Vec::new());
    };
    let from_groups = topology.groups_containing_endpoint(&edge.from, resources)?;
    let to_groups = topology.groups_containing_endpoint(&edge.to, resources)?;
    let from_group_endpoint = match topology.endpoint_index(&edge.from) {
        Some(GraphEndpointIndex::Group(index)) => Some(index),
        Some(GraphEndpointIndex::Node(_)) | None => None,
    };
    let to_group_endpoint = match topology.endpoint_index(&edge.to) {
        Some(GraphEndpointIndex::Group(index)) => Some(index),
        Some(GraphEndpointIndex::Node(_)) | None => None,
    };

    let mut indices = Vec::new();
    indices
        .try_reserve(graph.groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group_index in 0..graph.groups.len() {
        resources.charge_layout_work(1)?;
        let crosses_boundary =
            from_groups.contains(&group_index) != to_groups.contains(&group_index);
        if crosses_boundary
            || from_group_endpoint == Some(group_index)
            || to_group_endpoint == Some(group_index)
        {
            indices.push(group_index);
        }
    }
    Ok(indices)
}

fn canonicalize_edges(
    edges: &[AsciiGraphEdge],
    resources: &mut ResourceContext,
) -> Result<(Vec<AsciiGraphEdge>, Vec<usize>)> {
    let mut order = Vec::new();
    order
        .try_reserve(edges.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for index in 0..edges.len() {
        resources.charge_layout_work(1)?;
        order.push(index);
    }

    let sort_levels = sort_levels(order.len());
    resources.charge_layout_work(resources.checked_work_mul(order.len(), sort_levels)?)?;
    order.sort_by(|left, right| {
        compare_edges(&edges[*left], &edges[*right]).then_with(|| left.cmp(right))
    });

    let mut canonical_edges = Vec::new();
    canonical_edges
        .try_reserve(edges.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for index in &order {
        resources.charge_layout_work(1)?;
        canonical_edges.push(edges[*index].clone());
    }
    Ok((canonical_edges, order))
}

fn compare_edges(left: &AsciiGraphEdge, right: &AsciiGraphEdge) -> Ordering {
    left.from
        .cmp(&right.from)
        .then_with(|| left.to.cmp(&right.to))
        .then_with(|| explicit_edge_id(left).cmp(&explicit_edge_id(right)))
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| edge_stroke_order(left.stroke).cmp(&edge_stroke_order(right.stroke)))
        .then_with(|| marker_order(left.start_marker).cmp(&marker_order(right.start_marker)))
        .then_with(|| marker_order(left.end_marker).cmp(&marker_order(right.end_marker)))
        .then_with(|| left.length.cmp(&right.length))
        .then_with(|| color_order(left.style.line).cmp(&color_order(right.style.line)))
        .then_with(|| color_order(left.style.arrow).cmp(&color_order(right.style.arrow)))
        .then_with(|| color_order(left.style.label).cmp(&color_order(right.style.label)))
}

fn explicit_edge_id(edge: &AsciiGraphEdge) -> Option<&str> {
    edge.is_user_defined_id
        .then_some(edge.id.as_deref())
        .flatten()
}

const fn edge_stroke_order(stroke: GraphEdgeStroke) -> u8 {
    match stroke {
        GraphEdgeStroke::Normal => 0,
        GraphEdgeStroke::Dotted => 1,
        GraphEdgeStroke::Thick => 2,
        GraphEdgeStroke::Invisible => 3,
    }
}

const fn marker_order(marker: GraphEdgeMarker) -> u8 {
    match marker {
        GraphEdgeMarker::Open => 0,
        GraphEdgeMarker::Point => 1,
        GraphEdgeMarker::Circle => 2,
        GraphEdgeMarker::Cross => 3,
    }
}

const fn color_order(color: Option<AsciiRgb>) -> Option<(u8, u8, u8)> {
    match color {
        Some(color) => Some((color.r, color.g, color.b)),
        None => None,
    }
}

fn sort_levels(len: usize) -> usize {
    if len <= 1 {
        0
    } else {
        usize::try_from(len.ilog2()).unwrap_or(usize::MAX) + 1
    }
}

#[cfg(test)]
pub(super) fn prepare_route_scene(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
) -> Result<RouteScene> {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    prepare_route_scene_with_resources(graph, graph_layout, edges, charset, &mut resources)
}

fn endpoint_layout(
    graph_layout: &GraphLayout,
    endpoint_id: &str,
    charset: &GraphCharset,
) -> Option<NodeLayout> {
    let endpoint = graph_layout
        .groups
        .iter()
        .position(|layout| layout.id == endpoint_id)
        .map(GraphEndpointIndex::Group)
        .or_else(|| {
            graph_layout
                .nodes
                .iter()
                .position(|layout| layout.id == endpoint_id)
                .map(GraphEndpointIndex::Node)
        })?;

    match endpoint {
        GraphEndpointIndex::Node(node_index) => graph_layout.nodes.get(node_index).cloned(),
        GraphEndpointIndex::Group(group_index) => graph_layout
            .groups
            .get(group_index)
            .map(|group| group_endpoint_layout(group, charset)),
    }
}

fn group_endpoint_layout(group: &GroupLayout, charset: &GraphCharset) -> NodeLayout {
    NodeLayout {
        id: group.id.clone(),
        label: GraphLabel::new_with_profile("", charset.width_profile),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x: group.x,
        y: group.y,
        width: group.width,
        height: group.height,
    }
}

#[cfg(test)]
fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    paint_route_plan_body(drawing, plan)?;
    paint_route_plan_markers(drawing, plan)
}

fn paint_route_plan_body(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    for (_, cell) in plan.active_cells() {
        match cell.kind {
            PlannedRouteCellKind::EdgeLine => set_edge_cell_with_paint(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?,
            PlannedRouteCellKind::RouteCell => set_route_cell_with_paint(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?,
            PlannedRouteCellKind::EdgeArrow => {}
        }
    }
    Ok(())
}

fn paint_route_plan_markers(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    for (_, cell) in plan.active_cells() {
        if cell.kind == PlannedRouteCellKind::EdgeArrow {
            set_edge_cell_with_paint(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?;
        }
    }
    Ok(())
}

fn allocate_marker_berths(
    routes: &mut [PreparedRoute],
    occupancy: &mut SceneOccupancy<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let marker_capacity = routes.len().checked_mul(2).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    let mut pending = Vec::new();
    pending
        .try_reserve(marker_capacity)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (route_index, route) in routes.iter().enumerate() {
        for endpoint in [MarkerEndpoint::Start, MarkerEndpoint::End] {
            let candidates = route
                .plan
                .marker_candidates(endpoint, diagram_type, resources)?;
            if !candidates.is_empty() {
                for candidate in candidates
                    .iter()
                    .filter(|candidate| !candidate.is_primary())
                {
                    resources.charge_layout_work(1)?;
                    occupancy.register_terminal_claim(
                        candidate.coord,
                        TerminalClaim {
                            route_index,
                            endpoint,
                            point_direction: candidate.point_direction,
                        },
                    )?;
                }
                pending.push(PendingMarker {
                    route_index,
                    endpoint,
                    candidates,
                });
            }
        }
    }

    let sort_work = resources.checked_work_mul(pending.len(), sort_levels(pending.len()))?;
    resources.charge_layout_work(sort_work)?;
    pending.sort_by(|left, right| {
        left.candidates
            .len()
            .cmp(&right.candidates.len())
            .then_with(|| {
                routes[left.route_index]
                    .owner
                    .canonical_edge_index
                    .cmp(&routes[right.route_index].owner.canonical_edge_index)
            })
            .then_with(|| {
                marker_endpoint_order(left.endpoint).cmp(&marker_endpoint_order(right.endpoint))
            })
    });

    for marker in pending {
        let mut selected = None;
        let mut predecessor = None;
        for candidate in marker.candidates.iter().copied() {
            resources.charge_layout_work(1)?;
            if !marker_candidate_continues_chain(predecessor, candidate) {
                break;
            }
            match occupancy.marker_candidate_disposition(
                routes,
                marker.route_index,
                marker.endpoint,
                candidate,
                resources,
            )? {
                MarkerCandidateDisposition::Available => {
                    selected = Some(candidate);
                    break;
                }
                MarkerCandidateDisposition::CompatiblePassThrough => {
                    predecessor = Some(candidate);
                }
                MarkerCandidateDisposition::Blocked => break,
            }
        }
        let Some(candidate) = selected else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "independent endpoint marker berth exhausted",
            });
        };
        routes[marker.route_index].plan.materialize_marker_at(
            marker.endpoint,
            candidate,
            charset,
            diagram_type,
        )?;
        occupancy.suppress_route_terminal_tail(
            marker.route_index,
            candidate.terminal_tail(),
            &routes[marker.route_index].plan,
            resources,
            diagram_type,
        )?;
        let cell = routes[marker.route_index]
            .plan
            .materialized_marker_cell(marker.endpoint, diagram_type)?
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with unmaterialized endpoint markers",
            })?;
        occupancy.occupy_marker(
            cell.coord,
            MarkerOccupant {
                route_index: marker.route_index,
                endpoint: marker.endpoint,
            },
            resources,
            diagram_type,
        )?;
    }
    Ok(())
}

struct PendingMarker {
    route_index: usize,
    endpoint: MarkerEndpoint,
    candidates: Vec<MarkerCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerCandidateDisposition {
    Available,
    CompatiblePassThrough,
    Blocked,
}

fn marker_candidate_continues_chain(
    predecessor: Option<MarkerCandidate>,
    candidate: MarkerCandidate,
) -> bool {
    match predecessor {
        Some(predecessor) => candidate.follows_terminal_predecessor(predecessor),
        None => candidate.is_primary(),
    }
}

const fn marker_endpoint_order(endpoint: MarkerEndpoint) -> u8 {
    match endpoint {
        MarkerEndpoint::Start => 0,
        MarkerEndpoint::End => 1,
    }
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
    resources: &mut ResourceContext,
) -> Result<bool> {
    for endpoint_id in [&owner.from, &owner.to] {
        let mut all_incident_to_endpoint = true;
        for claim in claims {
            resources.charge_layout_work(1)?;
            if !existing_routes
                .get(claim.route_index)
                .is_some_and(|route| route.owner.endpoint_id(claim.endpoint) == endpoint_id)
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
struct OccupiedRect {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

impl OccupiedRect {
    fn try_new(
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

    fn intersects(self, other: Self) -> bool {
        self.x < other.right
            && other.x < self.right
            && self.y < other.bottom
            && other.y < self.bottom
    }

    fn contains(self, x: usize, y: usize) -> bool {
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
struct RouteBounds {
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RouteCandidateScore {
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
struct RouteCellOwner {
    route_index: usize,
    cell: PlannedCellId,
    segment: PlannedRouteSegment,
}

#[derive(Debug)]
struct RouteCellOccupancy {
    owners: Vec<RouteCellOwner>,
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
struct LabelOccupant {
    route_index: usize,
    label_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtectedKind {
    Node,
    GroupBorder,
    GroupTitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtectedShape {
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

    fn intersects(self, rect: OccupiedRect) -> bool {
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
struct ProtectedGeometry<'a> {
    owner_id: &'a str,
    group_index: Option<usize>,
    kind: ProtectedKind,
    shape: ProtectedShape,
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
struct SceneOccupancy<'layout> {
    route_cells: HashMap<CanvasCoord, RouteCellOccupancy>,
    route_bounds: Vec<Option<RouteBounds>>,
    terminal_claims: HashMap<CanvasCoord, Vec<TerminalClaim>>,
    markers: HashMap<CanvasCoord, MarkerOccupant>,
    labels: HashMap<CanvasCoord, LabelOccupant>,
    protected: Vec<ProtectedGeometry<'layout>>,
}

impl<'layout> SceneOccupancy<'layout> {
    fn try_new_for_routes(
        graph_layout: &'layout GraphLayout,
        route_capacity: usize,
        resources: &mut ResourceContext,
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
            labels: HashMap::new(),
            protected: Vec::new(),
        };
        scene
            .route_bounds
            .try_reserve(route_capacity)
            .map_err(|_| layout_allocation_failed())?;
        try_reserve_hash_map(&mut scene.terminal_claims, marker_capacity)?;
        try_reserve_hash_map(&mut scene.markers, marker_capacity)?;
        try_reserve_hash_map(&mut scene.labels, route_capacity)?;
        scene
            .protected
            .try_reserve(protected_capacity)
            .map_err(|_| layout_allocation_failed())?;

        for node in &graph_layout.nodes {
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

    #[cfg(test)]
    fn try_new(
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
    fn try_admit_route(
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

    fn score_route(
        &self,
        existing_routes: &[PreparedRoute],
        plan: &RoutePlan,
        owner: &RouteOwner,
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Option<RouteCandidateScore>> {
        let mut shared_cells = 0usize;

        for (_, cell) in plan.active_cells() {
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
                && !terminal_claims_allow_route_cell(existing_routes, owner, claims, resources)?
            {
                return Ok(None);
            }
            if self.route_cells.contains_key(&cell.coord) {
                resources.charge_layout_work(1)?;
                shared_cells = resources.checked_work_add(shared_cells, 1)?;
            }
        }

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
                resources.charge_layout_work(1)?;
                if !marker_candidate_continues_chain(predecessor, candidate) {
                    break;
                }
                match self.marker_candidate_disposition_before_commit(
                    existing_routes,
                    owner,
                    endpoint,
                    candidate,
                ) {
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
            resources.charge_layout_work(1)?;
            let original = OccupiedRect::try_new(
                label.placement.x(),
                label.placement.y(),
                label.placement.width(),
                label.text.line_count(),
                resources,
            )?;
            let anchor = resolve_label_anchor(plan, label.anchor, original, resources)?;
            let candidates = route_label_candidates(
                label.placement,
                label.text.line_count(),
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
                    label.text.line_count(),
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
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(candidate) {
                return Ok(false);
            }
        }

        for y in candidate.y..candidate.bottom {
            for x in candidate.x..candidate.right {
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord)
                    || self.labels.contains_key(&coord)
                    || self.route_cells.contains_key(&coord)
                {
                    return Ok(false);
                }
            }
        }

        resources.charge_layout_work(plan.cells.len())?;
        if plan.active_cells().any(|(_, cell)| {
            candidate.contains(cell.coord.x, cell.coord.y)
                && !label_anchor_contains(anchor, cell.coord, cell.segment)
        }) {
            return Ok(false);
        }
        Ok(true)
    }

    fn marker_candidate_disposition_before_commit(
        &self,
        existing_routes: &[PreparedRoute],
        owner: &RouteOwner,
        endpoint: MarkerEndpoint,
        candidate: MarkerCandidate,
    ) -> MarkerCandidateDisposition {
        let endpoint_id = owner.endpoint_id(endpoint);
        if !self.protected.iter().all(|protected| {
            !protected.shape.contains(candidate.coord)
                || (candidate.is_primary()
                    && (protected.allows_endpoint_port(endpoint_id, candidate.coord)
                        || (protected.kind == ProtectedKind::GroupBorder
                            && (protected.allows_endpoint_port(&owner.from, candidate.coord)
                                || protected.allows_endpoint_port(&owner.to, candidate.coord)))))
        }) {
            return MarkerCandidateDisposition::Blocked;
        }

        if let Some(marker) = self.markers.get(&candidate.coord) {
            return if marker_occupant_is_compatible(
                existing_routes,
                *marker,
                endpoint_id,
                candidate.point_direction,
            ) {
                MarkerCandidateDisposition::CompatiblePassThrough
            } else {
                MarkerCandidateDisposition::Blocked
            };
        }

        let claims = self.terminal_claims.get(&candidate.coord);
        let compatible_claim = |claim: &TerminalClaim| {
            terminal_claim_is_compatible(
                existing_routes,
                claim,
                endpoint_id,
                candidate.point_direction,
            )
        };
        if claims.is_some_and(|claims| claims.iter().any(|claim| !compatible_claim(claim))) {
            return MarkerCandidateDisposition::Blocked;
        }
        let available = self.route_cells.get(&candidate.coord).is_none_or(|cell| {
            let Some(claims) = claims else {
                return false;
            };
            cell.owners.iter().all(|cell_owner| {
                claims.iter().any(|claim| {
                    claim.route_index == cell_owner.route_index && compatible_claim(claim)
                })
            })
        });
        if available {
            MarkerCandidateDisposition::Available
        } else {
            MarkerCandidateDisposition::Blocked
        }
    }

    fn commit_route(
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
                let rect =
                    OccupiedRect::try_new(group.x, group.y, group.width, group.height, resources)?;
                let right = rect.right - 1;
                let bottom = rect.bottom - 1;
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

        let Some(bounds_slot) = self.route_bounds.get_mut(route_index) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "route terminal suppression without owned bounds",
            });
        };
        let mut bounds: Option<RouteBounds> = None;
        for (_, cell) in plan.active_cells() {
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

    fn label_candidate_is_clear(
        &self,
        route_index: usize,
        anchor: LabelAnchor,
        candidate: OccupiedRect,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        for protected in &self.protected {
            resources.charge_layout_work(1)?;
            if protected.shape.intersects(candidate) {
                return Ok(false);
            }
        }

        for y in candidate.y..candidate.bottom {
            for x in candidate.x..candidate.right {
                resources.charge_layout_work(1)?;
                let coord = CanvasCoord { x, y };
                if self.markers.contains_key(&coord) || self.labels.contains_key(&coord) {
                    return Ok(false);
                }
                if let Some(route_cell) = self.route_cells.get(&coord) {
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
        occupant: LabelOccupant,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let footprint_cells = footprint.cell_count(resources)?;
        self.labels
            .try_reserve(footprint_cells)
            .map_err(|_| layout_allocation_failed())?;
        for y in footprint.y..footprint.bottom {
            for x in footprint.x..footprint.right {
                resources.charge_layout_work(1)?;
                self.labels.insert(CanvasCoord { x, y }, occupant);
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

fn layout_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

fn allocate_route_label_placements(
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
        let label_len = route.plan.labels.len();
        for label_index in 0..label_len {
            let (original, line_count, unresolved_anchor) = {
                let label = &route.plan.labels[label_index];
                (label.placement, label.text.line_count(), label.anchor)
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
                    anchor,
                    candidate_rect,
                    resources,
                )? {
                    selected = Some((candidate, candidate_rect));
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
            occupancy.occupy_label(
                footprint,
                LabelOccupant {
                    route_index,
                    label_index,
                },
                resources,
            )?;
        }
    }
    Ok(())
}

fn resolve_label_anchor(
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

fn label_anchor_contains(
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
    Horizontal {
        min_x: usize,
        max_x: usize,
        y: usize,
    },
    Vertical {
        x: usize,
        min_y: usize,
        max_y: usize,
    },
    Point(CanvasCoord),
}

impl LabelAnchorAxis {
    fn from_anchor(anchor: LabelAnchor) -> Self {
        match anchor {
            LabelAnchor::Segment { start, end, .. } if start.y == end.y && start.x != end.x => {
                Self::Horizontal {
                    min_x: start.x.min(end.x),
                    max_x: start.x.max(end.x),
                    y: start.y,
                }
            }
            LabelAnchor::Segment { start, end, .. } if start.x == end.x && start.y != end.y => {
                Self::Vertical {
                    x: start.x,
                    min_y: start.y.min(end.y),
                    max_y: start.y.max(end.y),
                }
            }
            LabelAnchor::Segment { start, .. } | LabelAnchor::PlacementHint(start) => {
                Self::Point(start)
            }
        }
    }
}

fn route_label_candidates(
    original: RoutedLabelPlacement,
    line_count: usize,
    anchor: LabelAnchor,
    route_bounds: Option<RouteBounds>,
    resources: &mut ResourceContext,
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
        LabelAnchorAxis::Horizontal { min_x, max_x, y } => {
            push_horizontal_host_candidates(
                &mut candidates,
                original,
                line_count,
                min_x,
                max_x,
                y,
                resources,
            )?;
        }
        LabelAnchorAxis::Vertical { x, min_y, max_y } => {
            push_vertical_host_candidates(
                &mut candidates,
                original,
                line_count,
                x,
                min_y,
                max_y,
                resources,
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
            )?;
        }
    }
    Ok(candidates)
}

const MAX_LABEL_LANE_RADIUS: usize = 4;

fn push_horizontal_host_candidates(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    line_count: usize,
    min_x: usize,
    max_x: usize,
    y: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    let host_end = resources.checked_grid_add(max_x, 1)?;
    let midpoint = min_x + (max_x - min_x) / 2;
    let x_candidates = [
        Some(original.x()),
        midpoint.checked_sub(original.width() / 2),
        Some(min_x),
        host_end.checked_sub(original.width().max(1)),
    ];
    for x in x_candidates.into_iter().flatten() {
        push_label_candidate(candidates, original, x, original.y(), resources)?;
        for gap in 0..=MAX_LABEL_LANE_RADIUS {
            let above_clearance = resources.checked_grid_add(line_count.max(1), gap)?;
            if let Some(above) = y.checked_sub(above_clearance) {
                push_label_candidate(candidates, original, x, above, resources)?;
            }
            let below = resources.checked_grid_add(resources.checked_grid_add(y, 1)?, gap)?;
            push_label_candidate(candidates, original, x, below, resources)?;
        }
    }
    Ok(())
}

fn push_vertical_host_candidates(
    candidates: &mut Vec<RoutedLabelPlacement>,
    original: RoutedLabelPlacement,
    _line_count: usize,
    x: usize,
    min_y: usize,
    max_y: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    let host_end = resources.checked_grid_add(max_y, 1)?;
    let midpoint = min_y + (max_y - min_y) / 2;
    let y_candidates = [
        Some(original.y()),
        midpoint.checked_sub(1),
        Some(min_y),
        host_end.checked_sub(1),
    ];
    for y in y_candidates.into_iter().flatten() {
        push_label_candidate(candidates, original, original.x(), y, resources)?;
        for gap in 0..=MAX_LABEL_LANE_RADIUS {
            let left_clearance = resources.checked_grid_add(original.width().max(1), gap)?;
            if let Some(left) = x.checked_sub(left_clearance) {
                push_label_candidate(candidates, original, left, y, resources)?;
            }
            let right = resources.checked_grid_add(resources.checked_grid_add(x, 1)?, gap)?;
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
) -> Result<()> {
    let centered_x = point.x.checked_sub(original.width() / 2);
    let centered_y = point.y.checked_sub(line_count.saturating_sub(1) / 2);
    let prefer_vertical_lanes = route_bounds.is_none_or(RouteBounds::prefers_vertical_label_lanes);
    for gap in 0..=MAX_LABEL_LANE_RADIUS {
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
        let diagonal = [(left, above), (right, above), (left, below), (right, below)];
        let ordered = if prefer_vertical_lanes {
            [vertical, horizontal, diagonal]
        } else {
            [horizontal, vertical, diagonal]
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
            .map_err(|_| AsciiError::AllocationFailed {
                phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        candidates.push(candidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::plan::{PlannedRouteCell, PlannedRouteLabel, PlannedRoutePaint};
    use super::*;
    use crate::color::AsciiColorRole;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStyle};
    use crate::graph::routing::label::{RoutedLabelPlacement, RoutedLabelText};
    use crate::graph::routing::plan::{MarkerAnchor, MarkerAnchors, PlannedCellId};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::{AsciiRenderOptions, TerminalWidthProfile};
    use merman_core::resources::ResourceProfile;

    #[test]
    fn edge_style_is_applied_to_route_plan_cells_and_labels() {
        let line = AsciiRgb::new(1, 2, 3);
        let arrow = AsciiRgb::new(4, 5, 6);
        let label = AsciiRgb::new(7, 8, 9);
        let plan = RoutePlan::new_without_markers_for_test(
            vec![
                planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
                planned_cell(2, 0, '>', PlannedRouteCellKind::EdgeArrow),
            ],
            vec![PlannedRouteLabel::new(
                RoutedLabelText::new("label").expect("single-line label should exist"),
                RoutedLabelPlacement::new(0, 0, 5),
            )],
        );

        let mut canvas = RawCanvas::with_width_profile(5, 1, TerminalWidthProfile::Unicode);
        let mut route_cells = RouteCells::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells);
        let plan = plan.with_style(GraphEdgeStyle {
            line: Some(line),
            arrow: Some(arrow),
            label: Some(label),
        });

        paint_route_plan(&mut drawing, &plan)
            .expect("test route should fit the unbounded resource policy");

        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
        assert_eq!(
            canvas.get_color(1, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
        assert_eq!(
            canvas.get_color(2, 0),
            Some(crate::terminal::CanvasColor::Direct(arrow))
        );

        let scene = RouteScene {
            routes: vec![PreparedRoute::for_test(plan, 0)],
            extent: (5, 1),
            planned_cell_count: 3,
        };
        scene
            .draw_labels(&mut canvas, RouteLabelTransform::Identity)
            .expect("test route label should fit the unbounded resource policy");

        assert_eq!(canvas.get_color(0, 0), Some(CanvasColor::Direct(label)));
    }

    #[test]
    fn route_label_transform_mirrors_horizontal_label_placement() {
        let label = EdgeLabel {
            text: RoutedLabelText::new("north<br>south").expect("label should exist"),
            placement: RoutedLabelPlacement::new(2, 4, 5),
            color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
        };

        let transformed = RouteLabelTransform::HorizontalMirror { width: 20 }.apply(label);

        assert_eq!(transformed.text.lines(), ["north", "south"]);
        assert_eq!(transformed.placement, RoutedLabelPlacement::new(13, 4, 5));
    }

    #[test]
    fn route_label_transform_reverses_vertical_mirrored_multiline_labels() {
        let label = EdgeLabel {
            text: RoutedLabelText::new("north<br>south").expect("label should exist"),
            placement: RoutedLabelPlacement::new(2, 4, 5),
            color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
        };

        let transformed = RouteLabelTransform::VerticalMirror { height: 20 }.apply(label);

        assert_eq!(transformed.text.lines(), ["south", "north"]);
        assert_eq!(transformed.placement, RoutedLabelPlacement::new(2, 14, 5));
    }

    #[test]
    fn edge_arrow_style_falls_back_to_line_style() {
        let line = AsciiRgb::new(10, 11, 12);
        let plan = RoutePlan::new_without_markers_for_test(
            vec![planned_cell(0, 0, '>', PlannedRouteCellKind::EdgeArrow)],
            Vec::new(),
        );

        let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
        let mut route_cells = RouteCells::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells);

        paint_route_plan(
            &mut drawing,
            &plan.with_style(GraphEdgeStyle {
                line: Some(line),
                arrow: None,
                label: None,
            }),
        )
        .expect("test edge arrow should fit the unbounded resource policy");

        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
        );
    }

    #[test]
    fn route_body_admission_rejects_reserved_node_cells_before_commit() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = simple_graph_layout(&options);
        let blocker = &graph_layout.nodes[0];
        let coord = CanvasCoord {
            x: blocker.center_x(),
            y: blocker.center_y(),
        };
        let cell = planned_cell(coord.x, coord.y, '-', PlannedRouteCellKind::EdgeLine);
        let anchor = MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Right);
        let route = PreparedRoute::for_test_with_endpoints(
            RoutePlan::new(vec![cell], Vec::new(), MarkerAnchors::new(anchor, anchor)),
            0,
            "source",
            "target",
        );
        let mut resources = unbounded_resources();
        let mut occupancy =
            SceneOccupancy::try_new_for_routes(&graph_layout, 1, &mut resources).unwrap();

        assert!(
            !occupancy
                .try_admit_route(0, &route, &mut resources, "flowchart")
                .unwrap()
        );
        assert!(occupancy.route_cells.is_empty());
        assert!(occupancy.route_bounds.is_empty());
    }

    #[test]
    fn route_scene_selects_an_alternate_route_when_the_primary_lane_is_reserved() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "b");
        graph.add_edge("a", "b");
        graph.add_edge("a", "b");
        let mut graph_layout = layout_graph(&graph, &options);
        let from = graph_layout.nodes[0].clone();
        let to = graph_layout.nodes[1].clone();
        let primary = plan_edge_route(EdgeRouteRequest {
            graph: &graph,
            graph_layout: &graph_layout,
            edges: &graph.edges,
            from: &from,
            to: &to,
            edge_index: 1,
            edge: &graph.edges[1],
            charset: &charset,
        })
        .unwrap();
        let primary_lane_y = primary
            .cells
            .iter()
            .map(|cell| cell.coord.y)
            .max()
            .expect("parallel route should have a bottom lane");
        let blocked_coord = primary
            .cells
            .iter()
            .find(|cell| {
                cell.coord.y == primary_lane_y
                    && cell.coord.x > from.center_x()
                    && cell.coord.x < to.center_x()
            })
            .expect("parallel route should have a horizontal lane cell")
            .coord;
        graph_layout.nodes.push(NodeLayout {
            id: "route-blocker".to_string(),
            label: GraphLabel::new(""),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x: blocked_coord.x,
            y: blocked_coord.y,
            width: 1,
            height: 1,
        });

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("a bounded outer-lane candidate should avoid the reserved cell");

        assert_eq!(scene.routes.len(), 3);
        assert!(
            scene.routes[1]
                .plan
                .cells
                .iter()
                .all(|cell| cell.coord != blocked_coord)
        );
        assert_ne!(scene.routes[1].plan.cells, primary.cells);
    }

    #[test]
    fn conflicting_marker_owners_receive_independent_terminal_berths() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 3), 1),
        ];
        let mut resources = unbounded_resources();

        allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

        let first = routes[0]
            .plan
            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
            .unwrap()
            .unwrap();
        let second = routes[1]
            .plan
            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
            .unwrap()
            .unwrap();
        assert_ne!(first.coord, second.coord);
        assert_eq!((first.ch, second.ch), ('o', 'x'));
    }

    #[test]
    fn relocated_marker_suppresses_only_its_route_local_terminal_tail() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 3), 1),
        ];
        let mut resources = unbounded_resources();
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let mut occupancy =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

        allocate_marker_berths(
            &mut routes,
            &mut occupancy,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();

        let mut local_canvas = RawCanvas::with_width_profile(3, 1, TerminalWidthProfile::Unicode);
        let mut local_cells = RouteCells::new();
        paint_route_plan(
            &mut RouteDrawing::new(&mut local_canvas, &mut local_cells),
            &routes[1].plan,
        )
        .unwrap();
        assert!(routes[1].plan.is_cell_suppressed(PlannedCellId::new(2)));
        assert!(!routes[0].plan.is_cell_suppressed(PlannedCellId::new(2)));
        let shared_owners = &occupancy
            .route_cells
            .get(&CanvasCoord { x: 2, y: 0 })
            .expect("the unsuppressed route must retain the shared coordinate")
            .owners;
        assert!(
            shared_owners
                .iter()
                .any(|owner| { owner.route_index == 0 && owner.cell == PlannedCellId::new(2) })
        );
        assert!(
            !shared_owners
                .iter()
                .any(|owner| { owner.route_index == 1 && owner.cell == PlannedCellId::new(2) })
        );
        assert_eq!(local_canvas.get(0, 0), Some('-'));
        assert_eq!(local_canvas.get(1, 0), Some('x'));
        assert_eq!(
            local_canvas.get(2, 0),
            Some(' '),
            "the relocated marker must terminate its own route instead of producing -x-"
        );

        let mut shared_canvas = RawCanvas::with_width_profile(3, 1, TerminalWidthProfile::Unicode);
        let mut shared_cells = RouteCells::new();
        let mut drawing = RouteDrawing::new(&mut shared_canvas, &mut shared_cells);
        for route in &routes {
            route.paint_body(&mut drawing).unwrap();
        }
        for route in &routes {
            route.paint_markers(&mut drawing).unwrap();
        }
        assert_eq!(shared_canvas.get(0, 0), Some('-'));
        assert_eq!(shared_canvas.get(1, 0), Some('x'));
        assert_eq!(
            shared_canvas.get(2, 0),
            Some('o'),
            "suppressing one route's terminal tail must preserve the other route's marker ownership"
        );
    }

    #[test]
    fn relocated_marker_suppresses_a_three_cell_mixed_body_tail() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut plan = RoutePlan::new(
            vec![
                planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
                planned_cell(2, 0, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(3, 0, '-', PlannedRouteCellKind::RouteCell),
                planned_cell(4, 0, '-', PlannedRouteCellKind::EdgeLine),
            ],
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
                MarkerAnchor::new(PlannedCellId::new(4), StepDirection::Right),
            ),
        )
        .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
        .unwrap();
        let mut resources = unbounded_resources();
        let candidates = plan
            .marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
            .unwrap();
        let candidate = candidates[3];

        plan.materialize_marker_at(MarkerEndpoint::End, candidate, &charset, "flowchart")
            .unwrap();

        let mut canvas = RawCanvas::with_width_profile(5, 1, TerminalWidthProfile::Unicode);
        let mut route_cells = RouteCells::new();
        paint_route_plan(&mut RouteDrawing::new(&mut canvas, &mut route_cells), &plan).unwrap();
        assert_eq!(
            candidate.terminal_tail(),
            &[
                PlannedCellId::new(4),
                PlannedCellId::new(3),
                PlannedCellId::new(2),
            ]
        );
        for suppressed in candidate.terminal_tail() {
            assert!(plan.is_cell_suppressed(*suppressed));
        }
        assert_eq!(canvas.get(0, 0), Some('-'));
        assert_eq!(canvas.get(1, 0), Some('>'));
        assert_eq!(canvas.get(2, 0), Some(' '));
        assert_eq!(canvas.get(3, 0), Some(' '));
        assert_eq!(canvas.get(4, 0), Some(' '));
    }

    #[test]
    fn parallel_markers_occupy_a_contiguous_terminal_chain() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 4), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 4), 1),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Point, 4), 2),
        ];
        let mut resources = unbounded_resources();

        allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

        let markers = routes
            .iter()
            .map(|route| {
                let marker = route
                    .plan
                    .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                    .unwrap()
                    .unwrap();
                (marker.coord.x, marker.ch)
            })
            .collect::<Vec<_>>();
        assert_eq!(markers, [(3, 'o'), (2, 'x'), (1, '>')]);
        for (route_index, route) in routes.iter().enumerate() {
            let marker_x = markers[route_index].0;
            assert!(
                route
                    .plan
                    .active_cells()
                    .map(|(_, cell)| cell)
                    .all(|cell| cell.coord.x <= marker_x),
                "each route must terminate at its independently allocated marker"
            );
        }
    }

    #[test]
    fn identical_marker_glyphs_from_different_edges_do_not_coalesce() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 1),
        ];
        let mut resources = unbounded_resources();

        allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

        let coords = routes
            .iter()
            .map(|route| {
                route
                    .plan
                    .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                    .unwrap()
                    .unwrap()
                    .coord
            })
            .collect::<Vec<_>>();
        assert_ne!(coords[0], coords[1]);
    }

    #[test]
    fn marker_berth_exhaustion_is_explicit() {
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 1), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 1), 1),
        ];
        let mut resources = unbounded_resources();

        let error = allocate_test_marker_berths(&mut routes, &charset, &mut resources)
            .expect_err("one terminal cell cannot host two marker owners");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "independent endpoint marker berth exhausted",
            }
        );
    }

    #[test]
    fn route_score_rejects_an_interior_marker_berth_past_an_unrelated_crossing() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let marker_route = RoutePlan::new(
            (0..=2)
                .map(|x| planned_cell(x, 1, '-', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
                MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Right),
            ),
        )
        .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
        .unwrap();
        let crossing_route = RoutePlan::new(
            (0..=2)
                .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Up),
                MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Down),
            ),
        );
        let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
            crossing_route,
            0,
            "other-a",
            "other-b",
        )];
        let marker_route =
            PreparedRoute::for_test_with_endpoints(marker_route, 1, "source", "target");
        let mut resources = unbounded_resources();
        let occupancy =
            SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
                .unwrap();

        let score = occupancy
            .score_route(
                &existing_routes,
                &marker_route.plan,
                &marker_route.owner,
                &mut resources,
                "flowchart",
            )
            .unwrap();

        assert!(
            score.is_none(),
            "an unrelated crossing at the terminal must force another route candidate"
        );
    }

    #[test]
    fn route_score_rejects_a_later_crossing_of_a_reserved_primary_terminal() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
            marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1),
            0,
            "source",
            "target",
        )];
        let crossing_route = PreparedRoute::for_test_with_endpoints(
            vertical_route_plan_at_x(2),
            1,
            "other-a",
            "other-b",
        );
        let fallback_route = PreparedRoute::for_test_with_endpoints(
            vertical_route_plan_at_x(3),
            1,
            "other-a",
            "other-b",
        );
        let mut resources = unbounded_resources();
        let occupancy =
            SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
                .unwrap();

        let crossing_score = occupancy
            .score_route(
                &existing_routes,
                &crossing_route.plan,
                &crossing_route.owner,
                &mut resources,
                "flowchart",
            )
            .unwrap();
        let fallback_score = occupancy
            .score_route(
                &existing_routes,
                &fallback_route.plan,
                &fallback_route.owner,
                &mut resources,
                "flowchart",
            )
            .unwrap();

        assert!(
            crossing_score.is_none(),
            "a later route must not overwrite an already committed primary terminal corridor"
        );
        assert!(
            fallback_score.is_some(),
            "rejecting the crossing candidate must leave a clear fallback admissible"
        );
    }

    #[test]
    fn route_score_allows_an_incident_route_to_share_a_terminal_corridor() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
            marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1),
            0,
            "source",
            "shared",
        )];
        let incident_route = PreparedRoute::for_test_with_endpoints(
            vertical_route_plan_at_x(2),
            1,
            "shared",
            "other",
        );
        let mut resources = unbounded_resources();
        let occupancy =
            SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
                .unwrap();

        let score = occupancy
            .score_route(
                &existing_routes,
                &incident_route.plan,
                &incident_route.owner,
                &mut resources,
                "flowchart",
            )
            .unwrap();

        assert!(
            score.is_some(),
            "routes incident to the same authored endpoint may share its terminal corridor"
        );
    }

    #[test]
    fn route_score_does_not_reserve_an_unused_secondary_marker_corridor() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
            marker_request_plan_at_y(GraphEdgeMarker::Circle, 4, 1),
            0,
            "source",
            "target",
        )];
        let crossing_route = PreparedRoute::for_test_with_endpoints(
            vertical_route_plan_at_x(2),
            1,
            "other-a",
            "other-b",
        );
        let mut resources = unbounded_resources();
        let occupancy =
            SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
                .unwrap();

        let crossing_score = occupancy
            .score_route(
                &existing_routes,
                &crossing_route.plan,
                &crossing_route.owner,
                &mut resources,
                "flowchart",
            )
            .unwrap();

        assert!(
            crossing_score.is_some(),
            "an unused fallback marker berth must not reject an otherwise valid crossing route"
        );
    }

    #[test]
    fn marker_allocation_rejects_an_interior_berth_past_an_unrelated_crossing() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let marker_route = RoutePlan::new(
            (0..=2)
                .map(|x| planned_cell(x, 1, '-', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
                MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Right),
            ),
        )
        .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
        .unwrap();
        let crossing_route = RoutePlan::new(
            (0..=2)
                .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Up),
                MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Down),
            ),
        );
        let mut routes = vec![
            PreparedRoute::for_test_with_endpoints(marker_route, 0, "source", "target"),
            PreparedRoute::for_test_with_endpoints(crossing_route, 1, "other-a", "other-b"),
        ];
        let mut resources = unbounded_resources();
        let mut occupancy =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

        let error = allocate_marker_berths(
            &mut routes,
            &mut occupancy,
            &charset,
            &mut resources,
            "flowchart",
        )
        .expect_err("an endpoint marker must not move behind an unrelated crossing");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "independent endpoint marker berth exhausted",
            }
        );
    }

    #[test]
    fn marker_allocation_does_not_jump_over_an_unrelated_interior_crossing() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let crossing_route = RoutePlan::new_without_markers_for_test(
            (0..=2)
                .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
        );
        let mut routes = vec![
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 4), 0),
            PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 4), 1),
            PreparedRoute::for_test_with_endpoints(crossing_route, 2, "other-a", "other-b"),
        ];
        let mut resources = unbounded_resources();
        let mut occupancy =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

        let error = allocate_marker_berths(
            &mut routes,
            &mut occupancy,
            &charset,
            &mut resources,
            "flowchart",
        )
        .expect_err("a marker must not jump past an unrelated crossing to a deeper berth");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "independent endpoint marker berth exhausted",
            }
        );
    }

    #[test]
    fn route_scene_relocates_labels_that_cover_endpoint_markers() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let mut plan = marker_request_plan(GraphEdgeMarker::Point, 6);
        plan.labels.push(PlannedRouteLabel::new(
            RoutedLabelText::new("enter").unwrap(),
            RoutedLabelPlacement::new(1, 0, 5),
        ));
        let mut routes = vec![PreparedRoute::for_test(plan, 0)];
        let mut resources = unbounded_resources();
        let mut occupancy =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

        allocate_marker_berths(
            &mut routes,
            &mut occupancy,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        allocate_route_label_placements(&mut routes, &mut occupancy, &mut resources, "flowchart")
            .expect("route labels should move to an independent local lane");

        let marker = routes[0]
            .plan
            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
            .unwrap()
            .unwrap();
        let label = &routes[0].plan.labels[0];
        let label_rect = OccupiedRect::try_new(
            label.placement.x(),
            label.placement.y(),
            label.placement.width(),
            label.text.line_count(),
            &resources,
        )
        .unwrap();
        assert!(!label_rect.contains(marker.coord.x, marker.coord.y));
    }

    #[test]
    fn route_label_relocates_instead_of_covering_an_unrelated_route() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let mut label = PlannedRouteLabel::new(
            RoutedLabelText::new("tag").unwrap(),
            RoutedLabelPlacement::new(1, 2, 3),
        );
        label.anchor = LabelAnchor::Segment {
            start: CanvasCoord { x: 0, y: 2 },
            end: CanvasCoord { x: 4, y: 2 },
            route_segment: Some(PlannedRouteSegment::Direct),
        };
        let labeled_route = RoutePlan::new_without_markers_for_test(
            (0..=4)
                .map(|x| planned_cell(x, 2, '-', PlannedRouteCellKind::EdgeLine))
                .collect(),
            vec![label],
        );
        let crossing_route = RoutePlan::new_without_markers_for_test(
            (0..=4)
                .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
        );
        let mut routes = vec![
            PreparedRoute::for_test_with_endpoints(labeled_route, 0, "a", "b"),
            PreparedRoute::for_test_with_endpoints(crossing_route, 1, "c", "d"),
        ];
        let mut resources = unbounded_resources();

        allocate_test_label_placements(&mut routes, &graph_layout, &mut resources).unwrap();

        let label = &routes[0].plan.labels[0];
        let footprint = OccupiedRect::try_new(
            label.placement.x(),
            label.placement.y(),
            label.placement.width(),
            label.text.line_count(),
            &resources,
        )
        .unwrap();
        assert!(
            routes[1]
                .plan
                .cells
                .iter()
                .all(|cell| !footprint.contains(cell.coord.x, cell.coord.y))
        );
        assert_eq!(
            label.anchor,
            LabelAnchor::Segment {
                start: CanvasCoord { x: 0, y: 2 },
                end: CanvasCoord { x: 4, y: 2 },
                route_segment: Some(PlannedRouteSegment::Direct),
            }
        );
    }

    #[test]
    fn label_can_cover_only_its_own_host_segment() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let anchor = LabelAnchor::Segment {
            start: CanvasCoord { x: 0, y: 2 },
            end: CanvasCoord { x: 4, y: 2 },
            route_segment: Some(PlannedRouteSegment::Direct),
        };
        let plan = RoutePlan::new_without_markers_for_test(
            vec![
                planned_cell(0, 2, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(2, 2, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(3, 2, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(4, 2, '-', PlannedRouteCellKind::EdgeLine),
                planned_cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
            ],
            Vec::new(),
        );
        let routes = vec![PreparedRoute::for_test(plan, 0)];
        let mut resources = unbounded_resources();
        let occupancy =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();
        let host_cell = OccupiedRect::try_new(2, 2, 1, 1, &resources).unwrap();
        let non_host_cell = OccupiedRect::try_new(2, 1, 1, 1, &resources).unwrap();

        assert!(
            occupancy
                .label_candidate_is_clear(0, anchor, host_cell, &mut resources)
                .unwrap()
        );
        assert!(
            !occupancy
                .label_candidate_is_clear(0, anchor, non_host_cell, &mut resources)
                .unwrap()
        );
    }

    #[test]
    fn route_scene_relocates_labels_away_from_nodes_groups_and_other_labels() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = simple_graph_layout(&options);
        let node = &graph_layout.nodes[0];
        let node_route = labeled_plan(node.x, node.y, "node");
        let mut node_routes = vec![PreparedRoute::for_test(node_route, 0)];
        let mut resources = unbounded_resources();
        allocate_test_label_placements(&mut node_routes, &graph_layout, &mut resources)
            .expect("route labels should move away from node geometry");
        let label = &node_routes[0].plan.labels[0];
        let label_rect = OccupiedRect::try_new(
            label.placement.x(),
            label.placement.y(),
            label.placement.width(),
            label.text.line_count(),
            &resources,
        )
        .unwrap();
        let node_rect =
            OccupiedRect::try_new(node.x, node.y, node.width, node.height, &resources).unwrap();
        assert!(!label_rect.intersects(node_rect));

        let group_layout = grouped_graph_layout(&options);
        let group = &group_layout.groups[0];
        let group_route = labeled_plan(group.x, group.y, "group");
        let mut group_routes = vec![PreparedRoute::for_test(group_route, 0)];
        let mut resources = unbounded_resources();
        allocate_test_label_placements(&mut group_routes, &group_layout, &mut resources)
            .expect("route labels should move away from group borders and titles");
        let label = &group_routes[0].plan.labels[0];
        let label_rect = OccupiedRect::try_new(
            label.placement.x(),
            label.placement.y(),
            label.placement.width(),
            label.text.line_count(),
            &resources,
        )
        .unwrap();
        let occupancy =
            SceneOccupancy::try_new(&group_routes, &group_layout, &mut resources, "flowchart")
                .unwrap();
        assert!(
            occupancy
                .protected
                .iter()
                .all(|protected| !protected.shape.intersects(label_rect))
        );

        let mut routes = vec![
            PreparedRoute::for_test(labeled_plan(100, 100, "first"), 0),
            PreparedRoute::for_test(labeled_plan(100, 100, "second"), 1),
        ];
        let mut resources = unbounded_resources();
        allocate_test_label_placements(&mut routes, &graph_layout, &mut resources)
            .expect("duplicate label anchors should receive independent local lanes");
        let footprints = routes
            .iter()
            .map(|route| {
                let label = &route.plan.labels[0];
                OccupiedRect::try_new(
                    label.placement.x(),
                    label.placement.y(),
                    label.placement.width(),
                    label.text.line_count(),
                    &resources,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(!footprints[0].intersects(footprints[1]));
    }

    fn marker_request_plan(marker: GraphEdgeMarker, length: usize) -> RoutePlan {
        marker_request_plan_at_y(marker, length, 0)
    }

    fn marker_request_plan_at_y(marker: GraphEdgeMarker, length: usize, y: usize) -> RoutePlan {
        let cells = (0..length)
            .map(|x| planned_cell(x, y, '-', PlannedRouteCellKind::EdgeLine))
            .collect::<Vec<_>>();
        let start = MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left);
        let end = MarkerAnchor::new(
            PlannedCellId::new(length.saturating_sub(1)),
            StepDirection::Right,
        );
        RoutePlan::new(cells, Vec::new(), MarkerAnchors::new(start, end))
            .with_marker_requests(GraphEdgeMarker::Open, marker, "flowchart")
            .unwrap()
    }

    fn vertical_route_plan_at_x(x: usize) -> RoutePlan {
        RoutePlan::new(
            (0..=2)
                .map(|y| planned_cell(x, y, '|', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Up),
                MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Down),
            ),
        )
    }

    fn labeled_plan(x: usize, y: usize, text: &str) -> RoutePlan {
        RoutePlan::new_without_markers_for_test(
            vec![planned_cell(x, y + 1, '-', PlannedRouteCellKind::EdgeLine)],
            vec![PlannedRouteLabel::new(
                RoutedLabelText::new(text).unwrap(),
                RoutedLabelPlacement::new(x, y, text.len()),
            )],
        )
    }

    fn simple_graph_layout(options: &AsciiRenderOptions) -> GraphLayout {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        layout_graph(&graph, options)
    }

    fn grouped_graph_layout(options: &AsciiRenderOptions) -> GraphLayout {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["a".to_string()],
            Default::default(),
        );
        layout_graph(&graph, options)
    }

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    fn allocate_test_marker_berths(
        routes: &mut [PreparedRoute],
        charset: &GraphCharset,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
        let mut occupancy = SceneOccupancy::try_new(routes, &graph_layout, resources, "flowchart")?;
        allocate_marker_berths(routes, &mut occupancy, charset, resources, "flowchart")
    }

    fn allocate_test_label_placements(
        routes: &mut [PreparedRoute],
        graph_layout: &GraphLayout,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let mut occupancy = SceneOccupancy::try_new(routes, graph_layout, resources, "flowchart")?;
        allocate_route_label_placements(routes, &mut occupancy, resources, "flowchart")
    }

    fn route_scene_signature(scene: &RouteScene) -> Vec<(String, String, RoutePlan)> {
        scene
            .routes
            .iter()
            .map(|route| {
                (
                    route.owner.from.clone(),
                    route.owner.to.clone(),
                    route.plan.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn edge_canvas_extent_accounts_for_boundary_grid_path_label_width() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("y", "Y");
        graph.add_group_with_style(
            "one",
            "LR Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string()],
            Default::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge_with_attrs(
            "b",
            "y",
            GraphEdgeAttrs {
                label: Some("boundary label with enough width".to_string()),
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);
        let edge = &graph.edges[1];
        let from = endpoint_layout(&graph_layout, &edge.from, &charset)
            .expect("source layout should exist");
        let to =
            endpoint_layout(&graph_layout, &edge.to, &charset).expect("target layout should exist");
        let plan = plan_edge_route(EdgeRouteRequest {
            graph: &graph,
            graph_layout: &graph_layout,
            edges: &graph.edges,
            from: &from,
            to: &to,
            edge_index: 1,
            edge,
            charset: &charset,
        })
        .expect("boundary route should plan");
        let label = plan.labels.first().expect("boundary route should label");
        let (required_width, _) = label.placement.canvas_extent();

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("boundary scene should render");
        let (edge_width, _) = scene.canvas_extent();

        assert!(
            edge_width >= required_width,
            "edge canvas extent should reserve boundary label width {required_width}, got {edge_width}; plan: {plan:?}"
        );
    }

    #[test]
    fn canonical_edge_order_ignores_generated_ids_and_is_permutation_stable() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                label: Some("zeta".to_string()),
                end_marker: GraphEdgeMarker::Circle,
                ..Default::default()
            },
        );
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                label: Some("alpha".to_string()),
                end_marker: GraphEdgeMarker::Cross,
                ..Default::default()
            },
        );

        let mut generated_left = graph.edges[0].clone();
        generated_left.id = Some("generated-z".to_string());
        generated_left.is_user_defined_id = false;
        let mut generated_right = generated_left.clone();
        generated_right.id = Some("generated-a".to_string());
        assert_eq!(
            compare_edges(&generated_left, &generated_right),
            Ordering::Equal
        );

        let mut explicit_left = generated_left.clone();
        explicit_left.id = Some("edge-a".to_string());
        explicit_left.is_user_defined_id = true;
        let mut explicit_right = generated_left;
        explicit_right.id = Some("edge-b".to_string());
        explicit_right.is_user_defined_id = true;
        assert_eq!(
            compare_edges(&explicit_left, &explicit_right),
            Ordering::Less
        );

        let forward = graph.edges.clone();
        let mut reversed = forward.clone();
        reversed.reverse();
        let mut forward_resources = unbounded_resources();
        let mut reversed_resources = unbounded_resources();
        let (canonical_forward, _) = canonicalize_edges(&forward, &mut forward_resources).unwrap();
        let (canonical_reversed, _) =
            canonicalize_edges(&reversed, &mut reversed_resources).unwrap();

        assert_eq!(canonical_forward.len(), canonical_reversed.len());
        for (left, right) in canonical_forward.iter().zip(&canonical_reversed) {
            assert_eq!(compare_edges(left, right), Ordering::Equal);
        }
    }

    #[test]
    fn prepared_route_scene_is_stable_across_edge_permutations() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id.to_uppercase());
        }
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                end_marker: GraphEdgeMarker::Open,
                ..Default::default()
            },
        );
        graph.add_edge_with_attrs(
            "b",
            "c",
            GraphEdgeAttrs {
                end_marker: GraphEdgeMarker::Open,
                ..Default::default()
            },
        );
        graph.add_edge_with_attrs(
            "a",
            "c",
            GraphEdgeAttrs {
                end_marker: GraphEdgeMarker::Open,
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);
        let forward = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("canonical edge order should route the forward declaration");
        let mut permuted_edges = graph.edges.clone();
        permuted_edges.rotate_left(1);
        let permuted = prepare_route_scene(&graph, &graph_layout, &permuted_edges, &charset)
            .expect("canonical edge order should route the edge permutation");

        assert_eq!(
            route_scene_signature(&forward),
            route_scene_signature(&permuted)
        );
        assert!(
            forward
                .routes
                .iter()
                .all(|route| route.owner.source_edge_index < graph.edges.len())
        );
        assert!(
            permuted
                .routes
                .iter()
                .all(|route| route.owner.source_edge_index < permuted_edges.len())
        );
    }

    #[test]
    fn prepared_route_scene_prefers_a_clear_direct_route_over_extra_marker_berths() {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let charset = GraphCharset::for_options(&options);
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge("a", "b");
            let graph_layout = layout_graph(&graph, &options);

            let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
                .expect("a clear direct edge should produce a prepared route scene");
            let route = &scene.routes[0].plan;
            let source = graph_layout
                .nodes
                .iter()
                .find(|node| node.id == "a")
                .expect("source node should be laid out");

            assert!(
                route
                    .cells
                    .iter()
                    .all(|cell| cell.coord.y == source.center_y()),
                "marker relocation capacity must not outrank a shorter collision-free route: {route:?}"
            );
        }
    }

    #[test]
    fn prepare_route_scene_reports_missing_endpoint_layouts_before_painting() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "missing");
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&graph, &options);
        let charset = GraphCharset::for_options(&options);

        let error = match prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset) {
            Ok(_) => panic!("scene planning should fail on missing endpoint layouts"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint layouts",
            }
        );
    }

    #[test]
    fn prepare_route_scene_tracks_canvas_extent_for_each_route_plan() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("c", "C");
        graph.add_edge("a", "b");
        graph.add_edge_with_attrs(
            "b",
            "c",
            GraphEdgeAttrs {
                label: Some("wide label".to_string()),
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("supported graph should produce a prepared route scene");

        let mut expected_width = 0;
        let mut expected_height = 0;
        for route in &scene.routes {
            let (plan_width, plan_height) = route.plan.canvas_extent();
            expected_width = expected_width.max(plan_width);
            expected_height = expected_height.max(plan_height);
        }

        assert_eq!(scene.canvas_extent(), (expected_width, expected_height));
    }

    #[test]
    fn overlapping_route_cells_accept_exact_work_limit_and_reject_max_minus_one() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        let open_edge = GraphEdgeAttrs {
            end_marker: GraphEdgeMarker::Open,
            ..Default::default()
        };
        graph.add_edge_with_attrs("a", "b", open_edge.clone());
        graph.add_edge_with_attrs("a", "b", open_edge);
        let graph_layout = layout_graph(&graph, &options);
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured_resources = ResourceContext::new(unbounded);
        prepare_route_scene_with_resources(
            &graph,
            &graph_layout,
            &graph.edges,
            &charset,
            &mut measured_resources,
        )
        .expect("overlapping routes should plan");
        let exact = measured_resources.layout_work_used();
        assert!(exact > 1, "test graph should plan overlapping route cells");

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        prepare_route_scene_with_resources(
            &graph,
            &graph_layout,
            &graph.edges,
            &charset,
            &mut exact_resources,
        )
        .expect("exact planned-cell work limit should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = match prepare_route_scene_with_resources(
            &graph,
            &graph_layout,
            &graph.edges,
            &charset,
            &mut below_resources,
        ) {
            Ok(_) => panic!("max-minus-one planned-cell work limit should fail"),
            Err(error) => error,
        };
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    #[test]
    fn scene_geometry_accepts_exact_work_limit_and_rejects_max_minus_one() {
        let options = AsciiRenderOptions::ascii();
        let graph_layout = grouped_graph_layout(&options);
        let routes = Vec::new();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured_resources = ResourceContext::new(unbounded);
        let measured =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut measured_resources, "flowchart")
                .expect("group border and title geometry should precompute");
        assert!(
            measured
                .protected
                .iter()
                .any(|geometry| geometry.kind == ProtectedKind::GroupTitle)
        );
        let exact = measured_resources.layout_work_used();
        assert!(exact > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact scene-geometry work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        SceneOccupancy::try_new(&routes, &graph_layout, &mut exact_resources, "flowchart")
            .expect("exact scene-geometry work limit should pass");
        assert_eq!(exact_resources.layout_work_used(), exact);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one scene-geometry work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error =
            SceneOccupancy::try_new(&routes, &graph_layout, &mut below_resources, "flowchart")
                .expect_err("max-minus-one scene-geometry work limit should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    #[test]
    fn route_extent_reports_checked_cell_geometry_overflow() {
        let plan = RoutePlan::new_without_markers_for_test(
            vec![planned_cell(
                usize::MAX,
                0,
                '-',
                PlannedRouteCellKind::RouteCell,
            )],
            Vec::new(),
        );
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));

        let error = plan
            .canvas_extent_with_resources(&resources)
            .expect_err("overflowing route-cell geometry should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }

    fn planned_cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
        PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch,
            kind,
            segment: PlannedRouteSegment::Direct,
            paint: PlannedRoutePaint::role(match kind {
                PlannedRouteCellKind::EdgeArrow => AsciiColorRole::EdgeArrow,
                PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => {
                    AsciiColorRole::EdgeLine
                }
            }),
        }
    }
}
