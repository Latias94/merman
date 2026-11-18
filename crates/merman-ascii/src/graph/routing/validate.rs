//! Final semantic and geometry checks on retained plans, without planning another route.

#[cfg(test)]
use super::RouteScenePlan;
use super::allocation::AllocatedBranch;
use super::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP, route_char_directions};
use super::geometry::{CompoundRouteGeometry, FrameSegment};
use super::occupancy::{
    MarkerCandidateDisposition, OccupiedRect, SceneOccupancy, label_anchor_contains,
    label_reservation,
};
use super::path::StepDirection;
use super::plan::{
    EdgeRouteRequest, EndpointGeometry, LabelAnchor, MarkerCandidate, MarkerEndpoint,
    NodeAttachment, NodeAttachments, PlannedCellId, PlannedRouteCell, PlannedRouteCellKind,
    PlannedRouteLabel, PlannedRouteSegment,
};
use super::{
    PreparedRoute, group_endpoint_layout, layout_allocation_failed, route_boundary_group_indices,
};
use crate::error::{AsciiError, Result};
use crate::graph::charset::GraphCharset;
use crate::graph::layout::{CanvasCoord, GraphLayout};
use crate::graph::model::{AsciiGraph, AsciiGraphEdge, GraphEdgeMarker};
use crate::graph::topology::GraphGroupTopology;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;
use rustc_hash::FxHashMap as HashMap;

type CellIndex = HashMap<CanvasCoord, PlannedCellId>;
type PresentationIndex = HashMap<CanvasCoord, PresentationCell>;

#[derive(Default)]
struct PresentationCell {
    route: Option<(usize, PlannedRouteSegment)>,
    multiple_routes: bool,
    marker: Option<usize>,
    label: Option<(usize, usize)>,
    multiple_labels: bool,
}

#[derive(Default)]
struct RouteIndex {
    route: Option<usize>,
    retained_channel: Option<usize>,
}

/// Returns the smallest invalid canonical edge index, before label materialization or emit.
///
/// The caller maps this through its canonical source indices to `RoutingSearchExhausted`.
/// Resource, allocation, and cancellation failures remain errors. An orphan route with no
/// canonical edges has no source index and uses the existing internal-contract error instead.
#[cfg(test)]
pub(super) fn first_invalid_edge(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    scene: &RouteScenePlan<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    first_invalid_routes(
        graph,
        layout,
        edges,
        &scene.routes,
        &scene.labels,
        charset,
        resources,
        execution,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn first_invalid_routes(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    routes: &[PreparedRoute],
    labels: &super::label::RoutedLabelCatalogPlan<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    validate_routes(
        graph, layout, edges, routes, labels, charset, true, resources, execution,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn first_invalid_retained_route(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    routes: &[PreparedRoute],
    labels: &super::label::RoutedLabelCatalogPlan<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    validate_routes(
        graph, layout, edges, routes, labels, charset, false, resources, execution,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_routes(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    routes: &[PreparedRoute],
    labels: &super::label::RoutedLabelCatalogPlan<'_>,
    charset: &GraphCharset,
    complete: bool,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<usize>> {
    execution.checkpoint(OperationPhase::Layout)?;
    let mut resources = execution.resource_context(resources, OperationPhase::Layout);
    let resources = &mut resources;

    // One slot per canonical edge makes coverage independent of routing/repair order.
    resources.charge_layout_work_product(edges.len(), 2)?;
    let mut by_edge = Vec::new();
    by_edge
        .try_reserve_exact(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for _ in edges {
        resources.charge_layout_work(1)?;
        by_edge.push(RouteIndex::default());
    }
    let mut duplicate = None;
    for (route_index, route) in routes.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let index = route.owner.canonical_edge_index;
        let Some(slot) = by_edge.get_mut(index) else {
            if edges.is_empty() {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: graph.diagram_type(),
                    feature: "final route without a canonical semantic edge",
                });
            }
            // A corrupt owner cannot identify an authored edge; keep the result in range.
            duplicate = Some(0);
            continue;
        };
        if slot.route.replace(route_index).is_some() {
            duplicate = Some(duplicate.map_or(index, |previous: usize| previous.min(index)));
        }
        if let Some(branch) = route.owner.allocated_branch
            && let Some(slot) = by_edge.get_mut(branch.group)
        {
            resources.charge_layout_work(1)?;
            if slot
                .retained_channel
                .is_none_or(|previous| index < routes[previous].owner.canonical_edge_index)
            {
                slot.retained_channel = Some(route_index);
            }
        }
    }

    // Check coverage before constructing geometry indices, including the empty scene.
    for (index, edge) in edges.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let present = by_edge[index].route.is_some();
        if duplicate == Some(index)
            || (complete && edge.stroke.is_visible() != present)
            || (!edge.stroke.is_visible() && present)
        {
            // Geometry errors at smaller indices must still win over this coverage error.
            duplicate = Some(index);
            break;
        }
    }
    if duplicate == Some(0) || edges.is_empty() {
        return Ok(duplicate);
    }

    // Admit all temporary endpoint/protected-geometry storage before its reservation.
    let mut group_nodes = Vec::new();
    resources.charge_layout_work(layout.groups.len())?;
    group_nodes
        .try_reserve_exact(layout.groups.len())
        .map_err(|_| layout_allocation_failed())?;
    let endpoint_count = resources.checked_work_add(layout.nodes.len(), layout.groups.len())?;
    resources.charge_layout_work(endpoint_count)?;
    let mut endpoints = HashMap::default();
    endpoints
        .try_reserve(endpoint_count)
        .map_err(|_| layout_allocation_failed())?;
    for group in &layout.groups {
        resources.charge_layout_work(resources.checked_work_add(group.id.len(), 4)?)?;
        if !valid_extent(group.x, group.y, group.width, group.height, resources)? {
            return Ok(Some(0));
        }
        for line in group.title.lines() {
            resources.charge_layout_work(resources.checked_work_add(line.len(), 1)?)?;
        }
        group_nodes.push(group_endpoint_layout(group, charset));
    }
    for node in layout.nodes.iter().chain(&group_nodes) {
        resources.charge_layout_work(resources.checked_work_add(node.id.len(), 1)?)?;
        if !valid_extent(node.x, node.y, node.width, node.height, resources)? {
            return Ok(Some(0));
        }
        endpoints.insert(node.id.as_str(), node);
    }

    let topology = if graph.groups.is_empty() {
        None
    } else {
        // The topology primitive owns membership and nesting semantics. Its constructor
        // uses bulk admission, so checkpoint/precharge the member scans here as well.
        for node in &graph.nodes {
            resources.charge_layout_work(resources.checked_work_add(node.id.len(), 1)?)?;
        }
        for group in &graph.groups {
            resources.charge_layout_work(resources.checked_work_add(
                group.id.len(),
                resources.checked_work_add(group.nodes.len(), 1)?,
            )?)?;
            for member in &group.nodes {
                resources.charge_layout_work(member.len())?;
            }
        }
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    // Static geometry and a single retained presentation index are sufficient; no route
    // replays or pairwise scene scans are needed. The unlimited lane radius admits the
    // already chosen label lane and performs no lane search.
    let obstacles =
        SceneOccupancy::try_new_for_routes(layout, 0, usize::MAX, resources, execution)?;
    let presentation = presentation_index(routes, resources)?;

    for (index, edge) in edges.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if duplicate == Some(index) {
            return Ok(Some(index));
        }
        if !edge.stroke.is_visible() {
            continue;
        }
        let Some(route_index) = by_edge[index].route else {
            if complete {
                return Ok(Some(index));
            }
            continue;
        };
        let route = &routes[route_index];
        resources.charge_layout_work(resources.checked_work_add(
            resources.checked_work_add(route.owner.from.len(), route.owner.to.len())?,
            resources.checked_work_add(edge.from.len(), edge.to.len())?,
        )?)?;
        if route.owner.from != edge.from
            || route.owner.to != edge.to
            || route.owner.stroke != edge.stroke
            || route.plan.style != edge.style
        {
            return Ok(Some(index));
        }
        let expected_label = labels.descriptor(index);
        if route.plan.labels.len() != usize::from(expected_label.is_some())
            || route.plan.labels.first().is_some_and(|label| {
                Some(label.descriptor) != expected_label
                    || label.placement.width() != label.descriptor.width()
            })
        {
            return Ok(Some(index));
        }
        resources.charge_layout_work_product(graph.groups.len(), 4)?;
        let boundaries = route_boundary_group_indices(graph, topology.as_ref(), edge, resources)?;
        resources.charge_layout_work(route.owner.boundary_group_indices.len())?;
        if route.owner.boundary_group_indices != boundaries {
            return Ok(Some(index));
        }
        let (Some(&from), Some(&to)) = (
            endpoints.get(edge.from.as_str()),
            endpoints.get(edge.to.as_str()),
        ) else {
            return Ok(Some(index));
        };
        let request = EdgeRouteRequest {
            graph,
            graph_layout: layout,
            edges,
            from,
            to,
            edge_index: index,
            edge,
            charset,
        };
        resources.charge_layout_work(route.plan.cells.len())?;
        let mut cells = CellIndex::default();
        cells
            .try_reserve(route.plan.cells.len())
            .map_err(|_| layout_allocation_failed())?;
        for (cell_index, cell) in route.plan.cells.iter().enumerate() {
            resources.charge_layout_work(1)?;
            if cells
                .insert(cell.coord, PlannedCellId::new(cell_index))
                .is_some()
            {
                return Ok(Some(index));
            }
        }
        if !route_geometry_is_valid(
            request,
            route,
            &cells,
            &obstacles,
            &presentation,
            resources,
            execution,
        )? || !retained_channel_is_valid(route, routes, &by_edge, &cells, charset, resources)?
        {
            return Ok(Some(index));
        }
    }
    execution.checkpoint(OperationPhase::Layout)?;
    Ok(None)
}

fn valid_extent(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    resources: &ResourceContext,
) -> Result<bool> {
    resources.checked_grid_add(x, width)?;
    resources.checked_grid_add(y, height)?;
    Ok(width != 0 && height != 0)
}

fn presentation_index(
    routes: &[PreparedRoute],
    resources: &ResourceContext,
) -> Result<PresentationIndex> {
    let mut presentation = PresentationIndex::default();
    for route in routes {
        resources.charge_layout_work(1)?;
        let owner = route.owner.canonical_edge_index;
        for (_, cell) in route.plan.active_cells() {
            resources.charge_layout_work(1)?;
            presentation
                .try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            let retained = presentation.entry(cell.coord).or_default();
            if let Some((previous, _)) = retained.route {
                retained.multiple_routes |= previous != owner;
            } else {
                retained.route = Some((owner, cell.segment));
            }
            if cell.kind == PlannedRouteCellKind::EdgeArrow {
                retained.marker = Some(owner);
            }
        }
        for (index, label) in route.plan.labels.iter().enumerate() {
            resources.charge_layout_work(1)?;
            let rect = OccupiedRect::try_new(
                label.placement.x(),
                label.placement.y(),
                label.placement.width(),
                label.line_count(),
                resources,
            )?;
            let Some(reservation) = label_reservation(label.anchor, rect, usize::MAX, resources)?
            else {
                // The canonical per-edge checks report malformed hosts in index order.
                continue;
            };
            for y in reservation.y..reservation.bottom {
                for x in reservation.x..reservation.right {
                    resources.charge_layout_work(1)?;
                    presentation
                        .try_reserve(1)
                        .map_err(|_| layout_allocation_failed())?;
                    let retained = presentation.entry(CanvasCoord { x, y }).or_default();
                    let label_owner = (owner, index);
                    if let Some(previous) = retained.label {
                        retained.multiple_labels |= previous != label_owner;
                    } else {
                        retained.label = Some(label_owner);
                    }
                }
            }
        }
    }
    Ok(presentation)
}

fn route_geometry_is_valid(
    request: EdgeRouteRequest<'_>,
    route: &PreparedRoute,
    cells: &CellIndex,
    obstacles: &SceneOccupancy<'_>,
    presentation: &PresentationIndex,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    let plan = &route.plan;
    let Some(attachments) = plan.attachments else {
        return Ok(false);
    };
    let mut terminals = [None; 2];
    for (index, (endpoint, attachment, expected_marker)) in [
        (
            MarkerEndpoint::Start,
            attachments.start,
            request.edge.start_marker,
        ),
        (
            MarkerEndpoint::End,
            attachments.end,
            request.edge.end_marker,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        resources.charge_layout_work(1)?;
        let candidate = match plan.terminal_candidate(endpoint, request.graph.diagram_type()) {
            Ok(candidate) => candidate,
            Err(AsciiError::UnsupportedFeature { .. }) => return Ok(false),
            Err(error) => return Err(error),
        };
        let cell = &plan.cells[candidate.cell.index()];
        let (marker, glyph) = plan.marker_spec(endpoint, request.charset);
        if marker != expected_marker
            || plan.is_cell_suppressed(candidate.cell)
            || (marker != GraphEdgeMarker::Open
                && (cell.kind != PlannedRouteCellKind::EdgeArrow || cell.ch != glyph))
            || EndpointGeometry::for_request(request, endpoint, resources)?.resolve(
                candidate.coord,
                candidate.point_direction,
                request.charset,
            ) != Some(attachment)
            || !terminal_is_continuous(route, cells, attachment, candidate, resources)?
        {
            return Ok(false);
        }
        if marker != GraphEdgeMarker::Open
            && obstacles.marker_candidate_disposition_before_commit(
                &[],
                &route.owner,
                plan,
                endpoint,
                candidate,
                resources,
            )? == MarkerCandidateDisposition::Blocked
        {
            return Ok(false);
        }
        terminals[index] = Some((candidate, marker));
    }
    let [Some(start), Some(end)] = terminals else {
        return Ok(false);
    };
    if start.1 != GraphEdgeMarker::Open
        && end.1 != GraphEdgeMarker::Open
        && start.0.coord == end.0.coord
    {
        return Ok(false);
    }
    let geometry = CompoundRouteGeometry {
        layout: request.graph_layout,
        from: request.from,
        to: request.to,
        attachments: Some(attachments),
    };
    for (index, cell) in plan.cells.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if plan.is_cell_suppressed(PlannedCellId::new(index)) {
            if ![(attachments.start, start), (attachments.end, end)]
                .into_iter()
                .any(|(attachment, (candidate, marker))| {
                    (cell.coord == attachment.contact && attachment.connector.is_none())
                        || (marker != GraphEdgeMarker::Open
                            && before_terminal(attachment, candidate.coord, cell.coord))
                })
            {
                return Ok(false);
            }
            continue;
        }
        resources.checked_grid_add(cell.coord.x, 1)?;
        resources.checked_grid_add(cell.coord.y, 1)?;
        if cell.stroke != request.edge.stroke
            || (cell.kind == PlannedRouteCellKind::EdgeArrow
                && ![start, end].into_iter().any(|(candidate, marker)| {
                    marker != GraphEdgeMarker::Open && candidate.cell.index() == index
                }))
        {
            return Ok(false);
        }
        let directions = cell_directions(cell);
        if !obstacles.route_cell_is_clear(
            &[],
            &route.owner,
            Some(attachments),
            cell.coord,
            directions,
            resources,
        )? {
            return Ok(false);
        }
        // Test actual orthogonal steps at container frames, including corners. A
        // zero-length probe would incorrectly reject legal vertical frame crossings.
        let contact = [attachments.start, attachments.end]
            .into_iter()
            .find(|attachment| attachment.contact == cell.coord);
        let mut has_step = false;
        for heading in [
            StepDirection::Up,
            StepDirection::Right,
            StepDirection::Down,
            StepDirection::Left,
        ] {
            if contact.map_or(directions & direction_mask(heading) == 0, |contact| {
                contact.outward != heading
            }) {
                continue;
            }
            resources.charge_layout_work(1)?;
            let Some(neighbor) = step(cell.coord, heading, resources)? else {
                continue;
            };
            // ASCII '+' can describe several different corners. Only retained neighbor
            // cells and real endpoint contacts give it geometric steps to validate.
            if contact.is_none()
                && !cells.contains_key(&neighbor)
                && neighbor != attachments.start.contact
                && neighbor != attachments.end.contact
            {
                continue;
            }
            has_step = true;
            if geometry.segment(cell.coord, neighbor, resources, execution)?
                == FrameSegment::Blocked
            {
                return Ok(false);
            }
        }
        if !has_step {
            return Ok(false);
        }
    }
    if !route_is_connected(route, cells, start.0, end.0, resources)? {
        return Ok(false);
    }
    let reading_guards = if plan.labels.is_empty() {
        Vec::new()
    } else {
        obstacles.label_reading_guards(plan, resources)?
    };
    for (index, label) in plan.labels.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let LabelAnchor::Segment {
            start,
            end,
            route_segment,
        } = label.anchor
        else {
            return Ok(false);
        };
        if start.x != end.x && start.y != end.y {
            return Ok(false);
        }
        for coord in [start, end] {
            resources.charge_layout_work(1)?;
            let Some(id) = cells.get(&coord) else {
                return Ok(false);
            };
            if route_segment.is_some_and(|segment| plan.cells[id.index()].segment != segment) {
                return Ok(false);
            }
        }
        let rect = OccupiedRect::try_new(
            label.placement.x(),
            label.placement.y(),
            label.placement.width(),
            label.line_count(),
            resources,
        )?;
        if !obstacles.label_preserves_route_reading(&reading_guards, rect, resources)?
            || !obstacles.label_candidate_is_clear(
                &[],
                0,
                &route.owner,
                label.anchor,
                rect,
                resources,
            )?
            || !label_presentation_is_valid(
                route.owner.canonical_edge_index,
                index,
                label,
                rect,
                presentation,
                resources,
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn label_presentation_is_valid(
    owner: usize,
    label_index: usize,
    label: &PlannedRouteLabel,
    rect: OccupiedRect,
    presentation: &PresentationIndex,
    resources: &ResourceContext,
) -> Result<bool> {
    let Some(reservation) = label_reservation(label.anchor, rect, usize::MAX, resources)? else {
        return Ok(false);
    };
    for y in reservation.y..reservation.bottom {
        for x in reservation.x..reservation.right {
            resources.charge_layout_work(1)?;
            let coord = CanvasCoord { x, y };
            let Some(retained) = presentation.get(&coord) else {
                return Ok(false);
            };
            if retained.marker.is_some()
                || retained.multiple_labels
                || retained.label != Some((owner, label_index))
                || retained.multiple_routes
                || retained.route.is_some_and(|(route_owner, segment)| {
                    route_owner != owner || !label_anchor_contains(label.anchor, coord, segment)
                })
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn terminal_is_continuous(
    route: &PreparedRoute,
    cells: &CellIndex,
    attachment: NodeAttachment,
    candidate: MarkerCandidate,
    resources: &ResourceContext,
) -> Result<bool> {
    let distance = resources.checked_work_add(
        attachment.contact.x.abs_diff(candidate.coord.x),
        attachment.contact.y.abs_diff(candidate.coord.y),
    )?;
    if distance > route.plan.cells.len() {
        return Ok(false);
    }
    let axis = direction_mask(attachment.outward) | direction_mask(attachment.outward.opposite());
    let mut coord = attachment.contact;
    for _ in 0..distance {
        resources.charge_layout_work(1)?;
        let Some(next) = step(coord, attachment.outward, resources)? else {
            return Ok(false);
        };
        coord = next;
        let Some(id) = cells.get(&coord) else {
            return Ok(false);
        };
        let cell = &route.plan.cells[id.index()];
        if cell.kind != PlannedRouteCellKind::EdgeArrow && cell_directions(cell) & axis != axis {
            return Ok(false);
        }
    }
    Ok(true)
}

fn route_is_connected(
    route: &PreparedRoute,
    cells: &CellIndex,
    start: MarkerCandidate,
    end: MarkerCandidate,
    resources: &ResourceContext,
) -> Result<bool> {
    let Some(attachments) = route.plan.attachments else {
        return Ok(false);
    };
    let plan = &route.plan;
    resources.charge_layout_work_product(plan.cells.len(), 2)?;
    let mut visited = Vec::new();
    visited
        .try_reserve_exact(plan.cells.len())
        .map_err(|_| layout_allocation_failed())?;
    visited.resize(plan.cells.len(), false);
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(plan.cells.len())
        .map_err(|_| layout_allocation_failed())?;
    visited[start.cell.index()] = true;
    pending.push(start.cell);
    while let Some(id) = pending.pop() {
        resources.charge_layout_work(1)?;
        let cell = &plan.cells[id.index()];
        let directions = connection_directions(cell, attachments);
        for heading in [
            StepDirection::Up,
            StepDirection::Right,
            StepDirection::Down,
            StepDirection::Left,
        ] {
            resources.charge_layout_work(1)?;
            if directions & direction_mask(heading) == 0 {
                continue;
            }
            let Some(coord) = step(cell.coord, heading, resources)? else {
                continue;
            };
            let Some(&next) = cells.get(&coord) else {
                continue;
            };
            if visited[next.index()] || plan.is_cell_suppressed(next) {
                continue;
            }
            if connection_directions(&plan.cells[next.index()], attachments)
                & direction_mask(heading.opposite())
                != 0
            {
                visited[next.index()] = true;
                pending.push(next);
            }
        }
    }
    if !visited[end.cell.index()] {
        return Ok(false);
    }
    for (index, cell) in plan.cells.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if visited[index] || plan.is_cell_suppressed(PlannedCellId::new(index)) {
            continue;
        }
        // Relocating a marker can leave its contour connector ahead of a suppressed
        // terminal tail. This is the only legal active cell detached from the body.
        let mut separated_contact = false;
        for (attachment, terminal) in [(attachments.start, start), (attachments.end, end)] {
            if cell.coord != attachment.contact
                || attachment.connector.is_none()
                || cell.kind != PlannedRouteCellKind::EdgeLine
            {
                continue;
            }
            if let Some(coord) = step(cell.coord, attachment.outward, resources)?
                && before_terminal(attachment, terminal.coord, coord)
                && cells
                    .get(&coord)
                    .is_some_and(|id| plan.is_cell_suppressed(*id))
            {
                separated_contact = true;
            }
        }
        if !separated_contact {
            return Ok(false);
        }
    }
    Ok(true)
}

fn connection_directions(cell: &PlannedRouteCell, attachments: NodeAttachments) -> u8 {
    let mut contact_directions = 0;
    for attachment in [attachments.start, attachments.end] {
        if cell.coord == attachment.contact {
            contact_directions |= direction_mask(attachment.outward);
        }
    }
    if contact_directions != 0 {
        // Connector glyphs include the node's outline, not the route's transit axes.
        contact_directions
    } else {
        cell_directions(cell)
    }
}

fn retained_channel_is_valid(
    route: &PreparedRoute,
    routes: &[PreparedRoute],
    by_edge: &[RouteIndex],
    cells: &CellIndex,
    charset: &GraphCharset,
    resources: &ResourceContext,
) -> Result<bool> {
    let Some(branch) = route.owner.allocated_branch else {
        return Ok(true);
    };
    resources.charge_layout_work(1)?;
    let Some(leader) = by_edge
        .get(branch.group)
        .and_then(|slot| slot.route)
        .and_then(|index| routes.get(index))
    else {
        return Ok(false);
    };
    // Allocation is a candidate for each edge. Its canonical leader may have fallen
    // back to an ordinary terminal plan, so compare shared geometry only among the
    // members that actually retain the channel. A single retained member is valid.
    let Some(retained) = by_edge
        .get(branch.group)
        .and_then(|slot| slot.retained_channel)
        .and_then(|index| routes.get(index))
    else {
        return Ok(false);
    };
    let Some(reference) = retained.owner.allocated_branch else {
        return Ok(false);
    };
    let (shared, turn, attachment, marker, reference_attachment, reference_marker) =
        match branch.shared_endpoint {
            MarkerEndpoint::Start => (
                0,
                1,
                branch.attachments.start,
                route.plan.marker_spec(MarkerEndpoint::Start, charset),
                reference.attachments.start,
                retained.plan.marker_spec(MarkerEndpoint::Start, charset),
            ),
            MarkerEndpoint::End => (
                3,
                2,
                branch.attachments.end,
                route.plan.marker_spec(MarkerEndpoint::End, charset),
                reference.attachments.end,
                retained.plan.marker_spec(MarkerEndpoint::End, charset),
            ),
        };
    resources.charge_layout_work(resources.checked_work_add(
        route.owner.endpoint_id(branch.shared_endpoint).len(),
        leader.owner.endpoint_id(branch.shared_endpoint).len(),
    )?)?;
    if branch.group > route.owner.canonical_edge_index
        || reference.group != branch.group
        || reference.shared_endpoint != branch.shared_endpoint
        || route.plan.attachments != Some(branch.attachments)
        || route.owner.endpoint_id(branch.shared_endpoint)
            != leader.owner.endpoint_id(branch.shared_endpoint)
        || route.owner.stroke != leader.owner.stroke
        || route.plan.style != leader.plan.style
        || marker.0 != leader.plan.marker_spec(branch.shared_endpoint, charset).0
        || attachment != reference_attachment
        || marker != reference_marker
        || branch.points[shared] != reference.points[shared]
        || branch.points[turn] != reference.points[turn]
    {
        return Ok(false);
    }
    for pair in branch.points.windows(2) {
        resources.charge_layout_work(1)?;
        if pair[0].x != pair[1].x && pair[0].y != pair[1].y {
            return Ok(false);
        }
        let distance = resources
            .checked_work_add(pair[0].x.abs_diff(pair[1].x), pair[0].y.abs_diff(pair[1].y))?;
        if distance > route.plan.cells.len() {
            return Ok(false);
        }
        let mut coord = pair[0];
        for ordinal in 0..=distance {
            resources.charge_layout_work(1)?;
            let Some(id) = cells.get(&coord) else {
                return Ok(false);
            };
            let axis = if pair[0].y == pair[1].y {
                DIR_LEFT | DIR_RIGHT
            } else {
                DIR_UP | DIR_DOWN
            };
            if ordinal > 0
                && ordinal < distance
                && cell_directions(&route.plan.cells[id.index()]) & axis != axis
            {
                return Ok(false);
            }
            if ordinal < distance {
                let heading = if coord.x < pair[1].x {
                    StepDirection::Right
                } else if coord.x > pair[1].x {
                    StepDirection::Left
                } else if coord.y < pair[1].y {
                    StepDirection::Down
                } else {
                    StepDirection::Up
                };
                let Some(next) = step(coord, heading, resources)? else {
                    return Ok(false);
                };
                coord = next;
            }
        }
    }
    for (index, cell) in route.plan.cells.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if route.plan.is_cell_suppressed(PlannedCellId::new(index)) {
            continue;
        }
        if cell.coord != branch.attachments.start.contact
            && cell.coord != branch.attachments.end.contact
            && !on_channel(branch, cell.coord)
        {
            return Ok(false);
        }
    }
    for label in &route.plan.labels {
        resources.charge_layout_work(1)?;
        if !matches!(label.anchor, LabelAnchor::Segment { start, end, .. } if Some([start, end]) == branch.label_host)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn on_channel(branch: AllocatedBranch, coord: CanvasCoord) -> bool {
    branch.points.windows(2).any(|pair| {
        pair[0].y == pair[1].y
            && coord.y == pair[0].y
            && (pair[0].x.min(pair[1].x)..=pair[0].x.max(pair[1].x)).contains(&coord.x)
            || pair[0].x == pair[1].x
                && coord.x == pair[0].x
                && (pair[0].y.min(pair[1].y)..=pair[0].y.max(pair[1].y)).contains(&coord.y)
    })
}

fn before_terminal(attachment: NodeAttachment, terminal: CanvasCoord, coord: CanvasCoord) -> bool {
    attachment.allows_escape(coord)
        && match attachment.outward {
            StepDirection::Up => terminal.y < coord.y,
            StepDirection::Down => coord.y < terminal.y,
            StepDirection::Left => terminal.x < coord.x,
            StepDirection::Right => coord.x < terminal.x,
        }
}

fn cell_directions(cell: &PlannedRouteCell) -> u8 {
    if cell.directions == 0 {
        route_char_directions(cell.ch)
    } else {
        cell.directions
    }
}

fn direction_mask(heading: StepDirection) -> u8 {
    match heading {
        StepDirection::Up => DIR_UP,
        StepDirection::Right => DIR_RIGHT,
        StepDirection::Down => DIR_DOWN,
        StepDirection::Left => DIR_LEFT,
    }
}

fn step(
    coord: CanvasCoord,
    heading: StepDirection,
    resources: &ResourceContext,
) -> Result<Option<CanvasCoord>> {
    Ok(match heading {
        StepDirection::Up => coord.y.checked_sub(1).map(|y| CanvasCoord { y, ..coord }),
        StepDirection::Left => coord.x.checked_sub(1).map(|x| CanvasCoord { x, ..coord }),
        StepDirection::Right => Some(CanvasCoord {
            x: resources.checked_grid_add(coord.x, 1)?,
            ..coord
        }),
        StepDirection::Down => Some(CanvasCoord {
            y: resources.checked_grid_add(coord.y, 1)?,
            ..coord
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStroke, GraphNodeShape};
    use crate::graph::routing::label::{RoutedLabelDescriptor, RoutedLabelPlacement};
    use crate::graph::routing::{
        RouteCells, RouteDrawing, RouteLabelTransform, canonicalize_edges,
        prepare_route_scene_with_resources,
    };
    use crate::resource::{AsciiResourceLimitCause, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl};

    struct Fixture<'a> {
        graph: &'a AsciiGraph,
        layout: GraphLayout,
        edges: Vec<AsciiGraphEdge>,
        scene: RouteScenePlan<'a>,
        charset: GraphCharset,
    }

    impl<'a> Fixture<'a> {
        fn new(graph: &'a AsciiGraph, options: &AsciiRenderOptions) -> Self {
            Self::with_layout(graph, options, layout_graph(graph, options))
        }

        fn with_layout(
            graph: &'a AsciiGraph,
            options: &AsciiRenderOptions,
            layout: GraphLayout,
        ) -> Self {
            let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let edges = canonicalize_edges(&graph.edges, &mut resources)
                .unwrap()
                .values;
            let charset = GraphCharset::for_options(options);
            let scene = prepare_route_scene_with_resources(
                graph,
                &layout,
                &graph.edges,
                &charset,
                &mut resources,
            )
            .expect("the actual router must produce the uncorrupted scene");
            let fixture = Self {
                graph,
                layout,
                edges,
                scene,
                charset,
            };
            assert_eq!(
                fixture.invalid(),
                None,
                "the actual router's scene must validate"
            );
            fixture
        }

        fn check(
            &self,
            resources: &mut ResourceContext,
            execution: AsciiExecution<'_>,
        ) -> Result<Option<usize>> {
            first_invalid_edge(
                self.graph,
                &self.layout,
                &self.edges,
                &self.scene,
                &self.charset,
                resources,
                execution,
            )
        }

        fn invalid(&self) -> Option<usize> {
            let policy = AsciiResourcePolicy::unbounded();
            self.check(
                &mut ResourceContext::new(policy),
                AsciiExecution::for_test(&policy),
            )
            .unwrap()
        }

        fn route_mut(&mut self, canonical_index: usize) -> &mut PreparedRoute {
            self.scene
                .routes
                .iter_mut()
                .find(|route| route.owner.canonical_edge_index == canonical_index)
                .unwrap()
        }
    }

    fn labeled_chain() -> AsciiGraph {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id);
        }
        for (from, to, label) in [("a", "b", "alpha<br>route"), ("b", "c", "omega<br>route")] {
            graph.add_edge_with_attrs(
                from,
                to,
                GraphEdgeAttrs {
                    label: Some(label.into()),
                    length: 3,
                    ..Default::default()
                },
            );
        }
        graph
    }

    fn separate_horizontal_graph() -> AsciiGraph {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["a", "b", "c", "d"] {
            graph.add_node(id, id);
        }
        for (from, to, label) in [("a", "b", "one"), ("c", "d", "two")] {
            graph.add_edge_with_attrs(
                from,
                to,
                GraphEdgeAttrs {
                    label: Some(label.into()),
                    start_marker: GraphEdgeMarker::Circle,
                    ..Default::default()
                },
            );
        }
        graph
    }

    fn separate_horizontal_layout(graph: &AsciiGraph, options: &AsciiRenderOptions) -> GraphLayout {
        let mut layout = layout_graph(graph, options);
        for node in &mut layout.nodes {
            node.x = if matches!(node.id.as_str(), "a" | "c") {
                4
            } else {
                48
            };
            node.y = if matches!(node.id.as_str(), "a" | "b") {
                2
            } else {
                26
            };
            node.width = 5;
            node.height = 3;
        }
        layout
    }

    #[test]
    fn middle_gaps_nonreciprocal_steps_and_disconnected_active_cells_are_rejected() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "a");
        graph.add_node("b", "b");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                length: 4,
                ..Default::default()
            },
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for corruption in 0..3 {
                let mut fixture = Fixture::new(&graph, &options);
                let plan = &mut fixture.route_mut(0).plan;
                let middle = plan.cells.len() / 2;
                assert!(middle > 2 && middle + 3 < plan.cells.len());
                match corruption {
                    0 => {
                        // Leave a hole in the body and a detached two-cell component.
                        // Every retained cell still has a same-axis neighbor.
                        plan.cells[middle].coord.y += 2;
                        plan.cells[middle + 1].coord.y += 2;
                    }
                    1 => {
                        // Adjacent coordinates alone cannot establish connectivity.
                        plan.cells[middle].directions = DIR_RIGHT;
                    }
                    _ => {
                        // The terminals remain connected, but extra active cells do not.
                        let template = plan.cells[middle];
                        for x in [1000, 1001] {
                            plan.cells.push(PlannedRouteCell {
                                coord: CanvasCoord { x, y: 1000 },
                                ..template
                            });
                        }
                    }
                }
                assert_eq!(fixture.invalid(), Some(0), "corruption {corruption}");
            }
        }
    }

    #[test]
    fn vertical_contacts_use_attachment_directions_and_relocated_tails_remain_valid() {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
                let mut graph = AsciiGraph::new(direction);
                graph.add_node("a", "a");
                graph.add_node("b", "b");
                graph.add_edge_with_attrs(
                    "a",
                    "b",
                    GraphEdgeAttrs {
                        start_marker: GraphEdgeMarker::Circle,
                        length: 4,
                        ..Default::default()
                    },
                );
                let mut fixture = Fixture::new(&graph, &options);
                let charset = fixture.charset;
                let plan = &mut fixture.route_mut(0).plan;
                let attachment = plan.attachments.unwrap().start;
                let contact = plan
                    .active_cells()
                    .find(|(_, cell)| cell.coord == attachment.contact)
                    .unwrap()
                    .1;
                assert_eq!(
                    connection_directions(contact, plan.attachments.unwrap()),
                    direction_mask(attachment.outward)
                );
                if !charset.unicode && direction == GraphDirection::TopDown {
                    assert_eq!(contact.ch, '-');
                    assert_eq!(attachment.outward, StepDirection::Down);
                }
                let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                let candidates = plan
                    .marker_candidates(MarkerEndpoint::Start, "flowchart", &mut resources)
                    .unwrap()
                    .unwrap();
                assert!(candidates.len() > 1);
                plan.materialize_marker_at(
                    MarkerEndpoint::Start,
                    candidates[1],
                    &charset,
                    "flowchart",
                )
                .unwrap();
                assert!(!candidates[1].terminal_tail().is_empty());
                assert_eq!(fixture.invalid(), None);
            }
        }
    }

    #[test]
    fn overlapping_label_text_and_gaps_report_the_first_canonical_edge_in_either_route_order() {
        let graph = separate_horizontal_graph();
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for gap_only in [false, true] {
                let layout = separate_horizontal_layout(&graph, &options);
                let mut fixture = Fixture::with_layout(&graph, &options, layout);
                for (index, y) in [
                    if gap_only { 18 } else { 14 },
                    if gap_only { 10 } else { 14 },
                ]
                .into_iter()
                .enumerate()
                {
                    let label = &mut fixture.route_mut(index).plan.labels[0];
                    assert_eq!(label.descriptor.width(), 3);
                    label.placement = RoutedLabelPlacement::new(24, y, 3);
                }
                assert_eq!(fixture.invalid(), Some(0));
                fixture.scene.routes.reverse();
                assert_eq!(fixture.invalid(), Some(0));
            }
        }
    }

    #[test]
    fn labels_cannot_cover_other_routes_or_their_own_materialized_markers() {
        let mut graph = separate_horizontal_graph();
        graph.edges[1].label = None;
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for own_marker in [false, true] {
                let layout = separate_horizontal_layout(&graph, &options);
                let mut fixture = Fixture::with_layout(&graph, &options, layout);
                let route = fixture.route_mut(0);
                let placement = if own_marker {
                    route
                        .plan
                        .terminal_candidate(MarkerEndpoint::Start, "flowchart")
                        .unwrap()
                        .coord
                } else {
                    CanvasCoord { x: 24, y: 27 }
                };
                let label = &mut route.plan.labels[0];
                label.placement = RoutedLabelPlacement::new(placement.x, placement.y, 3);
                assert_eq!(fixture.invalid(), Some(0), "own_marker={own_marker}");
            }
        }
    }

    #[test]
    fn actual_shape_routes_validate_and_emit_complete_labels_and_marker_kinds() {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
                for (shape, marker, stroke) in [
                    (
                        GraphNodeShape::Rect,
                        GraphEdgeMarker::Open,
                        GraphEdgeStroke::Normal,
                    ),
                    (
                        GraphNodeShape::Diamond,
                        GraphEdgeMarker::Point,
                        GraphEdgeStroke::Dotted,
                    ),
                    (
                        GraphNodeShape::LeanRight,
                        GraphEdgeMarker::Circle,
                        GraphEdgeStroke::Thick,
                    ),
                    (
                        GraphNodeShape::LeanLeft,
                        GraphEdgeMarker::Cross,
                        GraphEdgeStroke::Normal,
                    ),
                ] {
                    let mut graph = AsciiGraph::new(direction);
                    graph.add_node_with_shape_and_style("a", "Source", shape, Default::default());
                    graph.add_node_with_shape_and_style("b", "Target", shape, Default::default());
                    graph.add_edge_with_attrs(
                        "a",
                        "b",
                        GraphEdgeAttrs {
                            label: Some("complete label".into()),
                            start_marker: marker,
                            end_marker: marker,
                            stroke,
                            length: 3,
                            ..Default::default()
                        },
                    );
                    let fixture = Fixture::new(&graph, &options);
                    let extent = fixture.scene.canvas_extent();
                    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                    let scene = fixture.scene.materialize(&resources).unwrap();
                    let mut canvas = crate::canvas::Canvas::with_width_profile(
                        extent.0,
                        extent.1,
                        options.terminal_width_profile,
                    );
                    let mut cells = RouteCells::default();
                    {
                        let mut drawing = RouteDrawing::new(&mut canvas, &mut cells);
                        scene
                            .paint_routes_with_execution(
                                &mut drawing,
                                AsciiExecution::for_test(&resources.policy()),
                            )
                            .unwrap();
                    }
                    scene
                        .draw_labels(&mut canvas, RouteLabelTransform::Identity)
                        .unwrap();
                    assert!(canvas.finish().contains("complete label"));
                    assert_eq!(
                        scene.routes[0]
                            .plan
                            .active_cells()
                            .filter(|(_, cell)| cell.kind == PlannedRouteCellKind::EdgeArrow)
                            .count(),
                        if marker == GraphEdgeMarker::Open {
                            0
                        } else {
                            2
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn actual_self_loops_keep_both_contacts_and_the_complete_label() {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            graph.add_node("a", "Node");
            graph.add_edge_with_attrs(
                "a",
                "a",
                GraphEdgeAttrs {
                    label: Some("retry".into()),
                    start_marker: GraphEdgeMarker::Circle,
                    end_marker: GraphEdgeMarker::Cross,
                    ..Default::default()
                },
            );
            let fixture = Fixture::new(&graph, &options);
            let attachments = fixture.scene.routes[0].plan.attachments.unwrap();
            assert_ne!(attachments.start.contact, attachments.end.contact);
            assert_eq!(fixture.scene.routes[0].plan.labels.len(), 1);
        }
    }

    #[test]
    fn coverage_requires_each_visible_canonical_edge_exactly_once() {
        let mut graph = labeled_chain();
        graph.add_edge_with_attrs(
            "a",
            "c",
            GraphEdgeAttrs {
                stroke: GraphEdgeStroke::Invisible,
                ..Default::default()
            },
        );
        let options = AsciiRenderOptions::ascii();
        let mut missing = Fixture::new(&graph, &options);
        let removed = missing.scene.routes.pop().unwrap();
        assert_eq!(missing.invalid(), Some(removed.owner.canonical_edge_index));

        let mut duplicate = Fixture::new(&graph, &options);
        let route = &duplicate.scene.routes[0];
        duplicate.scene.routes.push(PreparedRoute {
            plan: route.plan.clone(),
            owner: route.owner.clone(),
        });
        assert_eq!(duplicate.invalid(), Some(0));

        let mut invisible = Fixture::new(&graph, &options);
        let invisible_index = invisible
            .edges
            .iter()
            .position(|edge| !edge.stroke.is_visible())
            .unwrap();
        invisible.route_mut(2).owner.canonical_edge_index = invisible_index;
        assert_eq!(invisible.invalid(), Some(invisible_index));

        let mut out_of_range = Fixture::new(&graph, &options);
        out_of_range.scene.routes[0].owner.canonical_edge_index = usize::MAX;
        assert_eq!(out_of_range.invalid(), Some(0));
    }

    #[test]
    fn canonical_order_wins_over_route_order_and_later_missing_edges() {
        let graph = labeled_chain();
        let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        fixture.scene.routes.reverse();
        fixture.scene.routes.remove(0);
        fixture.route_mut(0).owner.from = "wrong-source".into();
        assert_eq!(fixture.invalid(), Some(0));
    }

    #[test]
    fn owner_endpoints_stroke_and_retained_cell_stroke_match_semantics() {
        let graph = labeled_chain();
        for corruption in 0..4 {
            let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
            let route = fixture.route_mut(0);
            match corruption {
                0 => route.owner.from = "c".into(),
                1 => route.owner.to = "c".into(),
                2 => route.owner.stroke = GraphEdgeStroke::Dotted,
                _ => route.plan.cells[0].stroke = GraphEdgeStroke::Thick,
            }
            assert_eq!(fixture.invalid(), Some(0), "corruption {corruption}");
        }
    }

    #[test]
    fn equal_dimensions_do_not_make_catalog_descriptors_interchangeable() {
        let graph = labeled_chain();
        let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        let other = fixture.route_mut(1).plan.labels[0].descriptor;
        let first = &mut fixture.route_mut(0).plan.labels[0];
        assert_eq!(first.descriptor.width(), other.width());
        assert_eq!(first.descriptor.line_count(), other.line_count());
        first.descriptor = other;
        assert_eq!(fixture.invalid(), Some(0));
    }

    #[test]
    fn labels_require_exact_multiplicity_dimensions_placement_width_and_owned_host() {
        let graph = labeled_chain();
        for corruption in 0..5 {
            let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
            let profile = fixture.charset.width_profile;
            let labels = &mut fixture.route_mut(0).plan.labels;
            match corruption {
                0 => labels.clear(),
                1 => labels.push(labels[0].clone()),
                2 => {
                    labels[0].descriptor =
                        RoutedLabelDescriptor::for_test(0, "alpha", profile).unwrap()
                }
                3 => {
                    labels[0].placement = RoutedLabelPlacement::new(
                        labels[0].placement.x(),
                        labels[0].placement.y(),
                        labels[0].placement.width() + 1,
                    )
                }
                _ => {
                    labels[0].anchor = LabelAnchor::Segment {
                        start: CanvasCoord { x: 1000, y: 1000 },
                        end: CanvasCoord { x: 1001, y: 1000 },
                        route_segment: None,
                    }
                }
            }
            assert_eq!(fixture.invalid(), Some(0), "corruption {corruption}");
        }
    }

    #[test]
    fn markers_require_materialization_requested_kind_glyph_and_actual_contact_direction() {
        for marker in [
            GraphEdgeMarker::Point,
            GraphEdgeMarker::Circle,
            GraphEdgeMarker::Cross,
        ] {
            let mut graph = labeled_chain();
            graph.edges[0].end_marker = marker;
            for corruption in 0..4 {
                let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::unicode());
                let charset = fixture.charset;
                let route = fixture.route_mut(0);
                let terminal = route
                    .plan
                    .terminal_candidate(MarkerEndpoint::End, "flowchart")
                    .unwrap();
                match corruption {
                    0 => {
                        route.plan.cells[terminal.cell.index()].kind =
                            PlannedRouteCellKind::RouteCell
                    }
                    1 => route.plan.cells[terminal.cell.index()].ch = '?',
                    2 => {
                        route.plan = route
                            .plan
                            .clone()
                            .with_marker_requests(
                                GraphEdgeMarker::Open,
                                GraphEdgeMarker::Open,
                                "flowchart",
                            )
                            .unwrap()
                    }
                    _ => {
                        let mut flipped = terminal;
                        flipped.point_direction = flipped.point_direction.opposite();
                        route
                            .plan
                            .materialize_marker_at(
                                MarkerEndpoint::End,
                                flipped,
                                &charset,
                                "flowchart",
                            )
                            .unwrap();
                    }
                }
                assert_eq!(
                    fixture.invalid(),
                    Some(0),
                    "marker {marker:?}, corruption {corruption}"
                );
            }
        }
    }

    #[test]
    fn extra_markers_and_forged_shape_contacts_are_rejected() {
        let graph = labeled_chain();
        for corruption in 0..3 {
            let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::unicode());
            let route = fixture.route_mut(0);
            if corruption == 0 {
                let terminal = route
                    .plan
                    .terminal_candidate(MarkerEndpoint::End, "flowchart")
                    .unwrap();
                let start = route
                    .plan
                    .terminal_candidate(MarkerEndpoint::Start, "flowchart")
                    .unwrap();
                let cell = route
                    .plan
                    .cells
                    .iter_mut()
                    .enumerate()
                    .find(|(index, _)| {
                        *index != terminal.cell.index() && *index != start.cell.index()
                    })
                    .unwrap()
                    .1;
                cell.kind = PlannedRouteCellKind::EdgeArrow;
                cell.ch = '>';
            } else {
                let attachment = &mut route.plan.attachments.as_mut().unwrap().end;
                if corruption == 1 {
                    attachment.contact.y += 1;
                } else {
                    attachment.connector = None;
                }
            }
            assert_eq!(fixture.invalid(), Some(0));
        }
    }

    fn container_graph(endpoint: bool, internal: bool) -> AsciiGraph {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id);
        }
        graph.add_group_with_style(
            "g",
            "Group",
            None,
            vec!["a".into(), "b".into()],
            Default::default(),
        );
        graph.add_edge_with_attrs(
            if endpoint { "g" } else { "a" },
            if internal { "b" } else { "c" },
            GraphEdgeAttrs {
                label: Some("condition".into()),
                length: 3,
                ..Default::default()
            },
        );
        graph
    }

    #[test]
    fn actual_container_contacts_and_internal_and_leaving_routes_validate() {
        for endpoint in [false, true] {
            for internal in [false, true] {
                let graph = container_graph(endpoint, internal);
                let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::unicode());
                if endpoint {
                    let group = &fixture.layout.groups[0];
                    let corner = CanvasCoord {
                        x: group.x,
                        y: group.y,
                    };
                    fixture
                        .route_mut(0)
                        .plan
                        .attachments
                        .as_mut()
                        .unwrap()
                        .start
                        .contact = corner;
                    assert_eq!(
                        fixture.invalid(),
                        Some(0),
                        "container corners cannot become contacts"
                    );
                }
            }
        }
    }

    #[test]
    fn internal_routes_cannot_escape_and_boundary_ownership_is_exact() {
        let graph = container_graph(false, true);
        for corruption in 0..3 {
            let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
            match corruption {
                0 => {
                    let group = &fixture.layout.groups[0];
                    let coord = CanvasCoord {
                        x: group.right() + 10,
                        y: group.bottom() + 10,
                    };
                    let plan = &mut fixture.route_mut(0).plan;
                    plan.cells.push(PlannedRouteCell {
                        coord,
                        ..plan.cells[0]
                    });
                }
                1 => fixture.route_mut(0).owner.boundary_group_indices.push(0),
                _ => {
                    fixture.route_mut(0).plan.labels[0].placement =
                        RoutedLabelPlacement::new(1000, 1000, 9)
                }
            }
            assert_eq!(fixture.invalid(), Some(0), "corruption {corruption}");
        }
        let graph = container_graph(false, false);
        let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        assert!(
            !fixture.scene.routes[0]
                .owner
                .boundary_group_indices
                .is_empty()
        );
        fixture.route_mut(0).owner.boundary_group_indices.clear();
        assert_eq!(fixture.invalid(), Some(0));
    }

    fn channel_layout(
        graph: &AsciiGraph,
        options: &AsciiRenderOptions,
        merging: bool,
    ) -> GraphLayout {
        let mut layout = layout_graph(graph, options);
        for node in &mut layout.nodes {
            (node.x, node.y, node.width, node.height) = match node.id.as_str() {
                "a" => (10, 20, 9, 5),
                "b" => (60, 8, 9, 5),
                "c" => (60, 32, 9, 5),
                _ => (60, 56, 9, 5),
            };
            if merging {
                node.x = 100 - node.x - node.width;
            }
            if graph.direction == GraphDirection::TopDown {
                std::mem::swap(&mut node.x, &mut node.y);
                std::mem::swap(&mut node.width, &mut node.height);
            }
        }
        layout
    }

    #[test]
    fn retained_branch_and_merge_channels_validate_and_reject_geometry_drift() {
        for merging in [false, true] {
            for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
                let mut graph = AsciiGraph::new(direction);
                for id in ["a", "b", "c"] {
                    graph.add_node(id, id);
                }
                for id in ["b", "c"] {
                    graph.add_edge_with_attrs(
                        if merging { id } else { "a" },
                        if merging { "a" } else { id },
                        GraphEdgeAttrs {
                            label: Some(id.into()),
                            end_marker: GraphEdgeMarker::Circle,
                            ..Default::default()
                        },
                    );
                }
                let options = AsciiRenderOptions::unicode();
                for corruption in 0..4 {
                    let layout = channel_layout(&graph, &options, merging);
                    let mut fixture = Fixture::with_layout(&graph, &options, layout);
                    assert!(
                        fixture
                            .scene
                            .routes
                            .iter()
                            .all(|route| route.owner.allocated_branch.is_some())
                    );
                    let branch = fixture.route_mut(1).owner.allocated_branch.unwrap();
                    match corruption {
                        0 => {
                            fixture
                                .route_mut(1)
                                .owner
                                .allocated_branch
                                .as_mut()
                                .unwrap()
                                .points[1]
                                .x += 1
                        }
                        1 => {
                            let cell = fixture
                                .route_mut(1)
                                .plan
                                .cells
                                .iter_mut()
                                .find(|cell| cell.coord == branch.points[1])
                                .unwrap();
                            cell.coord.x += 1;
                        }
                        2 => {
                            fixture
                                .route_mut(1)
                                .owner
                                .allocated_branch
                                .as_mut()
                                .unwrap()
                                .shared_endpoint = if merging {
                                MarkerEndpoint::Start
                            } else {
                                MarkerEndpoint::End
                            };
                        }
                        _ => {
                            let plan = &mut fixture.route_mut(1).plan;
                            plan.cells.push(PlannedRouteCell {
                                coord: CanvasCoord { x: 1000, y: 1000 },
                                ..plan.cells[0]
                            });
                        }
                    }
                    assert_eq!(
                        fixture.invalid(),
                        Some(1),
                        "merging={merging}, direction={direction:?}, corruption={corruption}"
                    );
                }
            }
        }
    }

    #[test]
    fn retained_channels_allow_the_canonical_leader_to_fall_back_to_a_terminal_plan() {
        for merging in [false, true] {
            for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
                for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                    let mut graph = AsciiGraph::new(direction);
                    for id in ["a", "b", "c", "d"] {
                        graph.add_node(id, id);
                    }
                    for id in ["b", "c", "d"] {
                        graph.add_edge_with_attrs(
                            if merging { id } else { "a" },
                            if merging { "a" } else { id },
                            GraphEdgeAttrs {
                                label: Some(id.into()),
                                end_marker: GraphEdgeMarker::Circle,
                                ..Default::default()
                            },
                        );
                    }
                    let layout = channel_layout(&graph, &options, merging);
                    let mut fixture = Fixture::with_layout(&graph, &options, layout);
                    assert!(
                        fixture
                            .scene
                            .routes
                            .iter()
                            .all(|route| route.owner.allocated_branch.is_some())
                    );
                    fixture.route_mut(0).owner.allocated_branch = None;
                    assert_eq!(fixture.invalid(), None);
                    fixture.scene.routes.reverse();
                    assert_eq!(fixture.invalid(), None);

                    // The only remaining channel member still owns its complete route.
                    let previous = fixture.route_mut(1).owner.allocated_branch.take();
                    assert_eq!(fixture.invalid(), None);
                    fixture.route_mut(1).owner.allocated_branch = previous;

                    // Losing the canonical leader must not disable peer geometry checks.
                    let branch = fixture
                        .route_mut(2)
                        .owner
                        .allocated_branch
                        .as_mut()
                        .unwrap();
                    branch.points[if merging { 2 } else { 1 }].x += 1;
                    assert_eq!(fixture.invalid(), Some(2));
                }
            }
        }
    }

    #[test]
    fn exact_validation_budget_succeeds_and_one_less_preserves_the_resource_error() {
        let mut cases = vec![(labeled_chain(), None)];
        for merging in [false, true] {
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            for id in ["a", "b", "c"] {
                graph.add_node(id, id);
            }
            for id in ["b", "c"] {
                graph.add_edge_with_attrs(
                    if merging { id } else { "a" },
                    if merging { "a" } else { id },
                    GraphEdgeAttrs {
                        label: Some(id.into()),
                        start_marker: GraphEdgeMarker::Circle,
                        end_marker: GraphEdgeMarker::Circle,
                        ..Default::default()
                    },
                );
            }
            cases.push((graph, Some(merging)));
        }
        let mut loops = AsciiGraph::new(GraphDirection::LeftRight);
        loops.add_node("a", "Node");
        loops.add_edge_with_attrs(
            "a",
            "a",
            GraphEdgeAttrs {
                label: Some("retry".into()),
                start_marker: GraphEdgeMarker::Circle,
                end_marker: GraphEdgeMarker::Cross,
                ..Default::default()
            },
        );
        cases.push((loops, None));
        for (graph, merging) in &cases {
            let options = AsciiRenderOptions::ascii();
            let fixture = if let Some(merging) = merging {
                Fixture::with_layout(graph, &options, channel_layout(graph, &options, *merging))
            } else {
                Fixture::new(graph, &options)
            };
            let policy = AsciiResourcePolicy::unbounded();
            let mut resources = ResourceContext::new(policy);
            assert_eq!(
                fixture
                    .check(&mut resources, AsciiExecution::for_test(&policy))
                    .unwrap(),
                None
            );
            let work = resources.layout_work_used();
            for limit in [work, work - 1] {
                let policy = policy
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap();
                let mut resources = ResourceContext::new(policy);
                let result = fixture.check(&mut resources, AsciiExecution::for_test(&policy));
                if limit == work {
                    assert_eq!(result.unwrap(), None);
                } else {
                    assert!(
                        matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.max == limit)
                    );
                }
            }
        }
    }

    #[test]
    fn cancellation_is_checked_before_storage_and_during_uncontrolled_resource_scans() {
        let graph = labeled_chain();
        let fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        for checkpoints in [0, 20] {
            let policy = AsciiResourcePolicy::unbounded();
            let mut resources = ResourceContext::new(policy);
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            let result = fixture.check(&mut resources, AsciiExecution::new(&control, &policy));
            assert!(
                matches!(result, Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested)
            );
            assert_eq!(resources.layout_work_used() == 0, checkpoints == 0);
        }
    }

    #[test]
    fn coordinate_overflow_remains_a_grid_resource_error() {
        let graph = labeled_chain();
        let mut fixture = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        fixture.layout.nodes[0].x = usize::MAX;
        let policy = AsciiResourcePolicy::unbounded();
        let result = fixture.check(
            &mut ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxGridCells && details.cause == AsciiResourceLimitCause::ArithmeticOverflow)
        );
    }

    #[test]
    fn empty_and_invisible_scenes_have_no_visible_route_obligations() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        assert_eq!(
            Fixture::new(&graph, &AsciiRenderOptions::ascii()).invalid(),
            None
        );
        graph.add_node("a", "a");
        graph.add_node("b", "b");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                stroke: GraphEdgeStroke::Invisible,
                label: Some("hidden".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            Fixture::new(&graph, &AsciiRenderOptions::ascii()).invalid(),
            None
        );
    }

    #[test]
    fn orphan_routes_without_any_canonical_edge_use_the_existing_contract_error() {
        let graph = labeled_chain();
        let mut source = Fixture::new(&graph, &AsciiRenderOptions::ascii());
        let empty_graph = AsciiGraph::new(GraphDirection::LeftRight);
        let mut fixture = Fixture::new(&empty_graph, &AsciiRenderOptions::ascii());
        fixture
            .scene
            .routes
            .push(source.scene.routes.pop().unwrap());
        let policy = AsciiResourcePolicy::unbounded();
        assert!(matches!(
            fixture.check(
                &mut ResourceContext::new(policy),
                AsciiExecution::for_test(&policy)
            ),
            Err(AsciiError::UnsupportedFeature {
                feature: "final route without a canonical semantic edge",
                ..
            })
        ));
    }
}
