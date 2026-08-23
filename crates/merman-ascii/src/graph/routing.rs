use super::charset::GraphCharset;
use super::label::GraphLabel;
#[cfg(test)]
use super::layout::CanvasCoord;
use super::layout::{GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{
    AsciiGraph, AsciiGraphEdge, GraphEdgeMarker, GraphEdgeStroke, GraphNodeShape, GraphNodeStyle,
};
use super::surface::GraphSurface;
use super::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::canvas::Canvas as RawCanvas;
#[cfg(test)]
use crate::canvas::CanvasColor;
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::cmp::Ordering;

mod cell;
mod label;
mod occupancy;
mod path;
mod plan;
#[cfg(test)]
mod tests;

pub(super) use cell::RouteCells;
use cell::{RouteCellPaint, set_edge_cell_with_paint, set_route_cell_with_paint};
use label::{EdgeLabel, RoutedLabelCatalog, RoutedLabelCatalogPlan, draw_routed_label};
#[cfg(test)]
use occupancy::{MarkerCandidateDisposition, OccupiedRect, ProtectedKind};
use occupancy::{
    RouteCandidateScore, SceneOccupancy, allocate_marker_berths, allocate_route_label_placements,
};
#[cfg(test)]
use path::StepDirection;
#[cfg(test)]
use plan::plan_edge_route;
use plan::{
    EdgeRouteCandidates, EdgeRouteRequest, MarkerEndpoint, PlannedRouteCellKind, RoutePlan,
    plan_edge_route_candidates_with_topology,
};
#[cfg(test)]
use plan::{LabelAnchor, PlannedRouteSegment};

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

pub(super) struct RouteScenePlan<'a> {
    routes: Vec<PreparedRoute>,
    extent: (usize, usize),
    planned_cell_count: usize,
    labels: RoutedLabelCatalogPlan<'a>,
}

pub(super) struct RouteScene {
    routes: Vec<PreparedRoute>,
    #[cfg(test)]
    extent: (usize, usize),
    planned_cell_count: usize,
    labels: RoutedLabelCatalog,
}

struct CanonicalEdges {
    values: Vec<AsciiGraphEdge>,
    source_indices: Vec<usize>,
}

struct PreparedRoute {
    plan: RoutePlan,
    owner: RouteOwner,
}

struct RoutePaintCursor<'execution> {
    execution: AsciiExecution<'execution>,
    iteration: usize,
}

impl<'execution> RoutePaintCursor<'execution> {
    const fn new(execution: AsciiExecution<'execution>) -> Self {
        Self {
            execution,
            iteration: 0,
        }
    }

    fn checkpoint(&mut self) -> Result<()> {
        self.execution
            .checkpoint_loop(merman_core::OperationPhase::Emit, self.iteration)?;
        self.iteration = self.iteration.saturating_add(1);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOwner {
    canonical_edge_index: usize,
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

    fn paint_body_with_cursor(
        &self,
        drawing: &mut RouteDrawing<'_>,
        cursor: &mut RoutePaintCursor<'_>,
    ) -> Result<()> {
        paint_route_plan_body_with_cursor(drawing, &self.plan, cursor)
    }

    fn paint_markers_with_cursor(
        &self,
        drawing: &mut RouteDrawing<'_>,
        cursor: &mut RoutePaintCursor<'_>,
    ) -> Result<()> {
        paint_route_plan_markers_with_cursor(drawing, &self.plan, cursor)
    }

    #[cfg(test)]
    fn paint_body(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        let policy = crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut cursor = RoutePaintCursor::new(AsciiExecution::for_test(&policy));
        self.paint_body_with_cursor(drawing, &mut cursor)
    }

    #[cfg(test)]
    fn paint_markers(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        let policy = crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut cursor = RoutePaintCursor::new(AsciiExecution::for_test(&policy));
        self.paint_markers_with_cursor(drawing, &mut cursor)
    }
}

impl RouteScenePlan<'_> {
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        self.extent
    }

    pub(super) fn materialize(self, resources: &ResourceContext) -> Result<RouteScene> {
        Ok(RouteScene {
            routes: self.routes,
            #[cfg(test)]
            extent: self.extent,
            planned_cell_count: self.planned_cell_count,
            labels: self.labels.materialize(resources)?,
        })
    }
}

impl RouteScene {
    #[cfg(test)]
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        self.extent
    }

    pub(super) fn planned_cell_count(&self) -> usize {
        self.planned_cell_count
    }

    pub(super) fn paint_routes_with_execution(
        &self,
        drawing: &mut RouteDrawing<'_>,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        let mut cursor = RoutePaintCursor::new(execution);
        for route in &self.routes {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            route.paint_body_with_cursor(drawing, &mut cursor)?;
        }
        for route in &self.routes {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            route.paint_markers_with_cursor(drawing, &mut cursor)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn draw_labels(
        &self,
        canvas: &mut RawCanvas,
        transform: RouteLabelTransform,
    ) -> Result<()> {
        for route in &self.routes {
            for label in &route.plan.labels {
                let text = self.labels.get(label.descriptor)?;
                let label = transform.apply(EdgeLabel {
                    text,
                    placement: label.placement,
                    color: label.paint.color,
                });
                draw_routed_label(canvas, &label)?;
            }
        }
        Ok(())
    }

    pub(super) fn draw_labels_with_execution(
        &self,
        canvas: &mut RawCanvas,
        transform: RouteLabelTransform,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        for route in &self.routes {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            for label in &route.plan.labels {
                execution.checkpoint(merman_core::OperationPhase::Emit)?;
                let text = self.labels.get(label.descriptor)?;
                let label = transform.apply(EdgeLabel {
                    text,
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
    fn apply<'a>(self, label: EdgeLabel<'a>) -> EdgeLabel<'a> {
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
                    text: label.text,
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

#[cfg(test)]
pub(super) fn prepare_route_scene_with_resources<'a>(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &'a [AsciiGraphEdge],
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<RouteScenePlan<'a>> {
    let policy = resources.policy();
    prepare_route_scene_with_execution(
        graph,
        graph_layout,
        edges,
        charset,
        resources,
        AsciiExecution::for_test(&policy),
    )
}

pub(super) fn prepare_route_scene_with_execution<'a>(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &'a [AsciiGraphEdge],
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RouteScenePlan<'a>> {
    execution.rebind_resource_context(resources, merman_core::OperationPhase::Layout);
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    let canonical_edges = canonicalize_edges(edges, resources)?;
    let label_plans = RoutedLabelCatalogPlan::try_new(
        edges,
        &canonical_edges.source_indices,
        charset.width_profile,
        resources,
    )?;
    let canonical_edges = canonical_edges.values;
    let mut routes = Vec::new();
    routes
        .try_reserve(canonical_edges.len())
        .map_err(|_| layout_allocation_failed())?;
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

    let mut occupancy = SceneOccupancy::try_new_for_routes(
        graph_layout,
        canonical_edges.len(),
        resources,
        execution,
    )?;

    for (edge_index, edge) in canonical_edges.iter().enumerate() {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
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
            label_plans.descriptor(edge_index),
            resources,
            execution,
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
            from: edge.from.clone(),
            to: edge.to.clone(),
            boundary_group_indices,
        };
        let mut selected = None::<(RouteCandidateScore, usize, RoutePlan)>;
        for (candidate_index, plan) in candidates.into_iter().enumerate() {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
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
        let plan = plan.try_with_stroke(edge.stroke, charset, graph.diagram_type())?;
        let start = plan.terminal_candidate(MarkerEndpoint::Start, graph.diagram_type())?;
        let end = plan.terminal_candidate(MarkerEndpoint::End, graph.diagram_type())?;
        let prepared = PreparedRoute { plan, owner };
        let route_index = routes.len();
        occupancy.commit_route(route_index, &prepared, start, end, resources)?;
        routes.push(prepared);
    }

    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    allocate_marker_berths(
        &mut routes,
        &mut occupancy,
        charset,
        resources,
        graph.diagram_type(),
    )?;
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    allocate_route_label_placements(&mut routes, &mut occupancy, resources, graph.diagram_type())?;

    let mut width = 0;
    let mut height = 0;
    let mut planned_cell_count = 0usize;
    for route in &routes {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
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

    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    Ok(RouteScenePlan {
        routes,
        extent: (width, height),
        planned_cell_count,
        labels: label_plans,
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
) -> Result<CanonicalEdges> {
    let mut order = Vec::new();
    order
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_failed())?;
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
        .map_err(|_| layout_allocation_failed())?;
    for index in &order {
        resources.charge_layout_work(1)?;
        canonical_edges.push(edges[*index].clone());
    }
    Ok(CanonicalEdges {
        values: canonical_edges,
        source_indices: order,
    })
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str())
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
    prepare_route_scene_with_resources(graph, graph_layout, edges, charset, &mut resources)?
        .materialize(&resources)
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
        label: GraphLabel::empty_with_profile(charset.width_profile),
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
    let policy = crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    );
    let mut cursor = RoutePaintCursor::new(AsciiExecution::for_test(&policy));
    paint_route_plan_body_with_cursor(drawing, plan, &mut cursor)?;
    paint_route_plan_markers_with_cursor(drawing, plan, &mut cursor)
}

fn paint_route_plan_body_with_cursor(
    drawing: &mut RouteDrawing<'_>,
    plan: &RoutePlan,
    cursor: &mut RoutePaintCursor<'_>,
) -> Result<()> {
    for (_, cell) in plan.active_cells() {
        cursor.checkpoint()?;
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
                RouteCellPaint {
                    stroke: cell.stroke,
                    directions: cell.directions,
                    unicode: cell.unicode,
                    diagram_type: plan.diagram_type,
                    color: cell.paint.color,
                },
            )?,
            PlannedRouteCellKind::EdgeArrow => {}
        }
    }
    Ok(())
}

fn paint_route_plan_markers_with_cursor(
    drawing: &mut RouteDrawing<'_>,
    plan: &RoutePlan,
    cursor: &mut RoutePaintCursor<'_>,
) -> Result<()> {
    for (_, cell) in plan.active_cells() {
        cursor.checkpoint()?;
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
