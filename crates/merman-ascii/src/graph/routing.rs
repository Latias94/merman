use super::charset::GraphCharset;
use super::label::GraphLabel;
#[cfg(test)]
use super::layout::CanvasCoord;
use super::layout::{GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{
    AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeMarker, GraphEdgeStroke, GraphNodeShape,
    GraphNodeStyle,
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
use rustc_hash::FxHashMap as HashMap;
use std::cmp::Ordering;
use std::rc::Rc;

mod allocation;
mod capacity;
mod cell;
mod conflict;
mod geometry;
mod label;
mod occupancy;
mod path;
mod plan;
mod repair;
mod terminal;
#[cfg(test)]
mod tests;
mod validate;

pub(super) use cell::RouteCells;
use cell::{RouteCellPaint, set_edge_cell_with_paint, set_route_cell_with_paint};
use label::{EdgeLabel, RoutedLabelCatalog, RoutedLabelCatalogPlan, draw_routed_label};
#[cfg(test)]
use occupancy::allocate_marker_berths;
#[cfg(test)]
use occupancy::allocate_route_label_placements;
#[cfg(test)]
use occupancy::{MarkerCandidateDisposition, OccupiedRect, ProtectedKind};
use occupancy::{
    RouteCandidateScore, SceneOccupancy, allocate_committed_route_labels,
    allocate_committed_route_marker_berths,
};
use path::StepDirection;
use plan::{EdgeRouteRequest, MarkerEndpoint, PlannedRouteCellKind, RoutePlan};
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
    labels: Rc<RoutedLabelCatalogPlan<'a>>,
    quality_requested: bool,
}

pub(super) struct SceneRoutingInput<'graph> {
    canonical: CanonicalEdges,
    labels: Rc<RoutedLabelCatalogPlan<'graph>>,
    topology: Option<GraphGroupTopology<'graph>>,
    independent_incidents: Vec<bool>,
}

impl<'graph> SceneRoutingInput<'graph> {
    pub(super) fn try_new(
        graph: &'graph AsciiGraph,
        edges: &'graph [AsciiGraphEdge],
        charset: &GraphCharset,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let topology = if graph.groups.is_empty() {
            None
        } else {
            Some(GraphGroupTopology::try_new(graph, resources)?)
        };
        let canonical = canonicalize_edges(edges, resources)?;
        let labels = Rc::new(RoutedLabelCatalogPlan::try_new(
            edges,
            &canonical.source_indices,
            charset.width_profile,
            resources,
        )?);
        let independent_incidents = independent_incident_demand(&canonical.values, resources)?;
        Ok(Self {
            canonical,
            labels,
            topology,
            independent_incidents,
        })
    }
}

// Immutable geometry is shared by both passes and rebuilt after every reflow.
struct PreparedSceneRouting<'layout> {
    graph_layout: &'layout GraphLayout,
    branch_allocation: allocation::BranchAllocation,
    protected: Rc<occupancy::ProtectedScene<'layout>>,
    label_regions: Rc<occupancy::LabelRegionCache<'layout>>,
    route_order: Vec<usize>,
    route_scan_width: usize,
}

impl<'layout> PreparedSceneRouting<'layout> {
    fn try_new(
        graph: &AsciiGraph,
        graph_layout: &'layout GraphLayout,
        input: &SceneRoutingInput<'_>,
        charset: &GraphCharset,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let canonical_edges = &input.canonical.values;
        let branch_allocation = allocation::BranchAllocation::try_new(
            graph,
            graph_layout,
            canonical_edges,
            charset,
            resources,
            execution,
        )?;
        capacity::preflight_fixed_port_capacity(
            graph,
            graph_layout,
            canonical_edges,
            &branch_allocation,
            charset,
            resources,
            execution,
        )?;
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
        let protected = SceneOccupancy::try_prepare_protection(graph_layout, resources)?;
        let label_regions =
            occupancy::LabelRegionCache::try_new(graph_layout, canonical_edges.len(), resources)?;
        let route_order = plan_route_order(graph, graph_layout, canonical_edges, resources)?;
        Ok(Self {
            graph_layout,
            branch_allocation,
            protected,
            label_regions,
            route_order,
            route_scan_width,
        })
    }
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
            .checkpoint_loop(merman_model::OperationPhase::Emit, self.iteration)?;
        self.iteration = self.iteration.saturating_add(1);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOwner {
    canonical_edge_index: usize,
    source_edge_index: usize,
    from: String,
    to: String,
    stroke: GraphEdgeStroke,
    boundary_group_indices: Vec<usize>,
    allocated_branch: Option<allocation::AllocatedBranch>,
}

impl RouteOwner {
    fn try_clone(&self, resources: &ResourceContext) -> Result<Self> {
        resources.charge_layout_work(resources.checked_work_add(
            resources.checked_work_add(self.from.len(), self.to.len())?,
            self.boundary_group_indices.len(),
        )?)?;
        let mut from = String::new();
        from.try_reserve_exact(self.from.len())
            .map_err(|_| layout_allocation_failed())?;
        from.push_str(&self.from);
        let mut to = String::new();
        to.try_reserve_exact(self.to.len())
            .map_err(|_| layout_allocation_failed())?;
        to.push_str(&self.to);
        let mut boundary_group_indices = Vec::new();
        boundary_group_indices
            .try_reserve_exact(self.boundary_group_indices.len())
            .map_err(|_| layout_allocation_failed())?;
        boundary_group_indices.extend_from_slice(&self.boundary_group_indices);
        Ok(Self {
            from,
            to,
            boundary_group_indices,
            ..*self
        })
    }

    fn shared_channel_group(&self) -> Option<usize> {
        self.allocated_branch.map(|branch| branch.group)
    }

    fn shared_marker_group(
        &self,
        endpoint: MarkerEndpoint,
        coord: super::layout::CanvasCoord,
    ) -> Option<usize> {
        let branch = self.allocated_branch?;
        let berth = match endpoint {
            MarkerEndpoint::Start => branch.points[0],
            MarkerEndpoint::End => branch.points[3],
        };
        (branch.shared_endpoint == endpoint && berth == coord).then_some(branch.group)
    }

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
            stroke: GraphEdgeStroke::Normal,
            boundary_group_indices: Vec::new(),
            allocated_branch: None,
        }
    }

    #[cfg(test)]
    fn set_source_group_for_test(&mut self, group: Option<usize>) {
        let start = CanvasCoord { x: 0, y: 0 };
        let end = CanvasCoord { x: 10, y: 0 };
        self.allocated_branch = group.map(|group| allocation::AllocatedBranch {
            group,
            shared_endpoint: MarkerEndpoint::Start,
            attachments: plan::NodeAttachments {
                start: plan::NodeAttachment {
                    contact: start,
                    outward: StepDirection::Right,
                    connector: None,
                },
                end: plan::NodeAttachment {
                    contact: end,
                    outward: StepDirection::Left,
                    connector: None,
                },
            },
            points: [start, end, end, end],
            label_host: None,
        });
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
            merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut cursor = RoutePaintCursor::new(AsciiExecution::for_test(&policy));
        self.paint_body_with_cursor(drawing, &mut cursor)
    }

    #[cfg(test)]
    fn paint_markers(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        let policy = crate::resource::AsciiResourcePolicy::for_profile(
            merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
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
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            route.paint_body_with_cursor(drawing, &mut cursor)?;
        }
        for route in &self.routes {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
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
                    mirrored_horizontally: false,
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
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            for label in &route.plan.labels {
                execution.checkpoint(merman_model::OperationPhase::Emit)?;
                let text = self.labels.get(label.descriptor)?;
                let label = transform.apply(EdgeLabel {
                    text,
                    placement: label.placement,
                    color: label.paint.color,
                    mirrored_horizontally: false,
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
                mirrored_horizontally: !label.mirrored_horizontally,
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
                    mirrored_horizontally: label.mirrored_horizontally,
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) fn prepare_route_scene_with_resources<'a>(
    graph: &'a AsciiGraph,
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
        &crate::options::AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy(),
        resources,
        AsciiExecution::for_test(&policy),
    )
}

#[cfg(test)]
pub(super) fn prepare_route_scene_with_execution<'a>(
    graph: &'a AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &'a [AsciiGraphEdge],
    charset: &GraphCharset,
    policy: &crate::options::GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RouteScenePlan<'a>> {
    let input = SceneRoutingInput::try_new(graph, edges, charset, resources)?;
    route_scene_with_input(
        graph,
        graph_layout,
        &input,
        charset,
        policy,
        resources,
        execution,
    )
}

pub(super) fn route_scene_with_input<'a>(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    input: &SceneRoutingInput<'a>,
    charset: &GraphCharset,
    policy: &crate::options::GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RouteScenePlan<'a>> {
    let local_budget = crate::operation::GraphPlanningBudget::default();
    let execution = execution.with_graph_planning(&local_budget);
    let feasibility = &execution
        .graph_planning()
        .expect("scene planning budget")
        .feasibility;
    let preparation_start = resources.layout_work_used();
    let initial_allowance = feasibility.remaining().1;
    let prepared = (|| {
        execution.rebind_resource_context(resources, merman_model::OperationPhase::Layout);
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        PreparedSceneRouting::try_new(graph, graph_layout, input, charset, resources, execution)
    })();
    feasibility.account_scene_work(preparation_start, initial_allowance, resources);
    let prepared = prepared?;
    let incumbent = route_scene_kernel(
        graph, &prepared, input, charset, policy, false, resources, execution,
    )?;
    let quality = &execution
        .graph_planning()
        .expect("scene planning budget")
        .quality;
    if !incumbent.quality_requested || quality.remaining().0 == 0 || quality.remaining().1 == 0 {
        return Ok(incumbent);
    }
    match route_scene_kernel(
        graph, &prepared, input, charset, policy, true, resources, execution,
    ) {
        Ok(improved) => Ok(improved),
        // Optional quality cannot discard a complete scene when a caller's private
        // preparation slice ends. Public resource and cancellation errors still propagate.
        Err(AsciiError::PlanningWorkAllowanceExhausted) => Ok(incumbent),
        Err(error) if error.graph_planning_fallback_reason().is_some() => Ok(incumbent),
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn route_scene_kernel<'a>(
    graph: &AsciiGraph,
    prepared: &PreparedSceneRouting<'_>,
    input: &SceneRoutingInput<'a>,
    charset: &GraphCharset,
    policy: &crate::options::GraphLayoutPolicy,
    quality: bool,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RouteScenePlan<'a>> {
    let budget = execution.graph_planning().expect("scene planning budget");
    let allowance = if quality {
        &budget.quality
    } else {
        &budget.feasibility
    };
    let start_work = resources.layout_work_used();
    let initial_allowance = allowance.remaining().1;
    let result = (|| {
        execution.rebind_resource_context(resources, merman_model::OperationPhase::Layout);
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        let topology = &input.topology;
        let label_plans = &input.labels;
        let CanonicalEdges {
            values: canonical_edges,
            source_indices,
        } = &input.canonical;
        let graph_layout = prepared.graph_layout;
        let branch_allocation = &prepared.branch_allocation;
        let mut routes: Vec<PreparedRoute> = Vec::new();
        routes
            .try_reserve(canonical_edges.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut occupancy = SceneOccupancy::try_new_with_protection(
            Rc::clone(&prepared.label_regions),
            canonical_edges.len(),
            policy.edge_label_lane_radius,
            Rc::clone(&prepared.protected),
            resources,
            execution,
        )?;

        let mut repair_budget = repair::SceneRepairBudget::for_execution(execution);
        let mut quality_requested = false;

        let mut pending_self_loop_start = None::<usize>;
        for &edge_index in &prepared.route_order {
            let edge = &canonical_edges[edge_index];
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            if quality {
                allowance.account_scene_work(start_work, initial_allowance, resources);
            }
            if quality && allowance.remaining().1 == 0 {
                return Err(AsciiError::RoutingSearchExhausted {
                    diagram_type: graph.diagram_type(),
                    edge_index: source_indices[edge_index],
                });
            }
            if let Some(start) = pending_self_loop_start
                && (edge.from != routes[start].owner.from || edge.from != edge.to)
            {
                flush_deferred_loop_labels(
                    &mut routes,
                    start,
                    &mut occupancy,
                    resources,
                    graph.diagram_type(),
                )?;
                pending_self_loop_start = None;
            }
            if edge.stroke == super::model::GraphEdgeStroke::Invisible {
                continue;
            }
            resources.charge_layout_work(prepared.route_scan_width)?;
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
            let request = EdgeRouteRequest {
                graph,
                graph_layout,
                edges: canonical_edges,
                from: &from,
                to: &to,
                edge_index,
                edge,
                charset,
            };
            let forward = forward_route_direction(request, resources)?;
            let candidates = if edge.from == edge.to {
                terminal::plan_self_loop_candidates(
                    request,
                    label_plans.descriptor(edge_index),
                    resources,
                    execution,
                )?
            } else {
                Vec::new()
            };

            let boundary_group_indices =
                route_boundary_group_indices(graph, topology.as_ref(), edge, resources)?;
            let mut owner = RouteOwner {
                canonical_edge_index: edge_index,
                source_edge_index: source_indices[edge_index],
                from: edge.from.clone(),
                to: edge.to.clone(),
                stroke: edge.stroke,
                boundary_group_indices,
                allocated_branch: None,
            };
            let mut selected = None::<(RouteCandidateScore, usize, RoutePlan)>;
            for (candidate_index, plan) in candidates.into_iter().enumerate() {
                execution.checkpoint(merman_model::OperationPhase::Layout)?;
                let Some((score, plan)) = admit_route_candidate(
                    request,
                    path::RouteObstacles {
                        scene: &occupancy,
                        routes: &routes,
                        owner: &owner,
                    },
                    plan,
                    resources,
                )?
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
            let mut honors_allocation = false;
            if let Some(branch) = branch_allocation.get(edge_index) {
                owner.allocated_branch = Some(*branch);
                let candidate = match plan_allocated_channel(
                    request,
                    branch,
                    label_plans.descriptor(edge_index),
                    resources,
                    execution,
                )? {
                    Some(plan) => admit_route_candidate(
                        request,
                        path::RouteObstacles {
                            scene: &occupancy,
                            routes: &routes,
                            owner: &owner,
                        },
                        plan,
                        resources,
                    )?,
                    None => None,
                };
                if let Some((score, plan)) = candidate {
                    // A jointly allocated trunk is a scene decision. Choosing a shorter
                    // private detour per edge would dismantle that decision one branch at a time.
                    selected = Some((score, usize::MAX, plan));
                    honors_allocation = true;
                } else {
                    owner.allocated_branch = None;
                }
            }
            if !honors_allocation
                && !(edge.from == edge.to && selected.is_some())
                && let Some((score, plan)) = terminal::plan_direct_route(
                    request,
                    path::RouteObstacles {
                        scene: &occupancy,
                        routes: &routes,
                        owner: &owner,
                    },
                    label_plans.descriptor(edge_index),
                    resources,
                    execution,
                )?
                && selected
                    .as_ref()
                    .is_none_or(|(incumbent, _, incumbent_plan)| {
                        route_candidate_is_better(
                            forward,
                            (score, plan.attachments),
                            (*incumbent, incumbent_plan.attachments),
                        )
                    })
            {
                selected = Some((score, usize::MAX, plan));
            }
            let container_endpoint = matches!(
                topology
                    .as_ref()
                    .and_then(|topology| topology.endpoint_index(&edge.from)),
                Some(GraphEndpointIndex::Group(_))
            );
            if quality {
                allowance.account_scene_work(start_work, initial_allowance, resources);
            }
            let mut defer_loop_labels = edge.from == edge.to && !container_endpoint;
            let mut needs_terminal_search = selected.is_none()
                || (quality || input.independent_incidents[edge_index])
                    && !honors_allocation
                    && selected.as_ref().is_some_and(|(score, _, plan)| {
                        route_needs_search(forward, *score, plan, defer_loop_labels)
                    });
            if needs_terminal_search && let Some(start) = pending_self_loop_start.take() {
                flush_deferred_loop_labels(
                    &mut routes,
                    start,
                    &mut occupancy,
                    resources,
                    graph.diagram_type(),
                )?;
                if let Some((_, index, plan)) = selected.take() {
                    selected = admit_route_candidate(
                        request,
                        path::RouteObstacles {
                            scene: &occupancy,
                            routes: &routes,
                            owner: &owner,
                        },
                        plan,
                        resources,
                    )?
                    .map(|(score, plan)| (score, index, plan));
                }
            }
            if needs_terminal_search && !quality {
                occupancy.begin_blocker_collection(resources)?;
                terminal::plan_direct_route(
                    request,
                    path::RouteObstacles {
                        scene: &occupancy,
                        routes: &routes,
                        owner: &owner,
                    },
                    label_plans.descriptor(edge_index),
                    resources,
                    execution,
                )?;
                let witnesses = occupancy
                    .take_blocker_collection(resources)?
                    .unwrap_or_default();
                if selected.is_none()
                    && !witnesses.blockers.is_empty()
                    && edge.from != edge.to
                    && let Some((score, plan)) = try_undisplaced_route(
                        request,
                        path::RouteObstacles {
                            scene: &occupancy,
                            routes: &routes,
                            owner: &owner,
                        },
                        label_plans.descriptor(edge_index),
                        &budget.allocation,
                        resources,
                        execution,
                    )?
                {
                    needs_terminal_search = input.independent_incidents[edge_index]
                        && route_needs_search(forward, score, &plan, false);
                    selected = Some((score, usize::MAX, plan));
                    defer_loop_labels = false;
                }
                if selected.is_none()
                    && !witnesses.blockers.is_empty()
                    && edge.from != edge.to
                    && repair_blocked_scene_for_input(
                        request,
                        input,
                        &owner,
                        &witnesses,
                        &mut routes,
                        &mut occupancy,
                        &mut repair_budget,
                        resources,
                        execution,
                    )?
                {
                    quality_requested = true;
                    continue;
                }
                if needs_terminal_search {
                    occupancy.begin_blocker_collection(resources)?;
                }
            }
            if needs_terminal_search
                && let Some((score, plan)) = terminal::plan_terminal_route_with_budget(
                    request,
                    path::RouteObstacles {
                        scene: &occupancy,
                        routes: &routes,
                        owner: &owner,
                    },
                    label_plans.descriptor(edge_index),
                    selected
                        .as_ref()
                        .map(|(score, _, plan)| (*score, plan.attachments)),
                    terminal::SearchBudget::Limited(if quality && selected.is_some() {
                        &execution
                            .graph_planning()
                            .expect("scene search budget")
                            .quality_improvement
                    } else if quality {
                        &execution
                            .graph_planning()
                            .expect("scene search budget")
                            .quality
                    } else if selected.is_some() {
                        &execution
                            .graph_planning()
                            .expect("scene search budget")
                            .allocation
                    } else {
                        &execution
                            .graph_planning()
                            .expect("scene search budget")
                            .feasibility
                    }),
                    resources,
                    execution,
                )?
            {
                selected = Some((score, usize::MAX, plan));
                defer_loop_labels = false;
            }
            if needs_terminal_search && !quality {
                let witnesses = occupancy
                    .take_blocker_collection(resources)?
                    .unwrap_or_default();
                if selected.is_none()
                    && edge.from != edge.to
                    && repair_blocked_scene_for_input(
                        request,
                        input,
                        &owner,
                        &witnesses,
                        &mut routes,
                        &mut occupancy,
                        &mut repair_budget,
                        resources,
                        execution,
                    )?
                {
                    quality_requested = true;
                    continue;
                }
            }
            let plan = match selected {
                Some((score, _, plan)) => {
                    if !honors_allocation
                        && route_needs_search(forward, score, &plan, defer_loop_labels)
                    {
                        quality_requested = true;
                    }
                    plan
                }
                None => {
                    return Err(AsciiError::RoutingSearchExhausted {
                        diagram_type: graph.diagram_type(),
                        edge_index: source_indices[edge_index],
                    });
                }
            };
            let mut plan = plan.try_with_stroke(edge.stroke, charset, graph.diagram_type())?;
            occupancy.preserve_group_crossings(&mut plan, &owner, resources)?;
            let start = plan.terminal_candidate(MarkerEndpoint::Start, graph.diagram_type())?;
            let end = plan.terminal_candidate(MarkerEndpoint::End, graph.diagram_type())?;
            let prepared = PreparedRoute { plan, owner };
            let route_index = routes.len();
            occupancy.commit_route(route_index, &prepared, start, end, resources)?;
            routes.push(prepared);
            // Admission and materialization share the current occupancy. Subsequent route
            // candidates must preserve the distinct marker berths already committed here.
            allocate_committed_route_marker_berths(
                &mut routes,
                route_index,
                &mut occupancy,
                charset,
                resources,
                graph.diagram_type(),
            )?;
            // Predetermined node loops can reserve labels after their nested lanes are known.
            // Searched loops and container loops protect each label before the next search.
            if defer_loop_labels {
                pending_self_loop_start.get_or_insert(route_index);
            } else {
                allocate_committed_route_labels(
                    &mut routes,
                    route_index,
                    &mut occupancy,
                    resources,
                    graph.diagram_type(),
                )?;
            }
        }

        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        if let Some(start) = pending_self_loop_start {
            flush_deferred_loop_labels(
                &mut routes,
                start,
                &mut occupancy,
                resources,
                graph.diagram_type(),
            )?;
        }
        if let Some(index) = validate::first_invalid_routes(
            graph,
            graph_layout,
            canonical_edges,
            &routes,
            label_plans,
            charset,
            resources,
            execution,
        )? {
            return Err(AsciiError::RoutingSearchExhausted {
                diagram_type: graph.diagram_type(),
                edge_index: source_indices[index],
            });
        }
        let mut width = 0;
        let mut height = 0;
        let mut planned_cell_count = 0usize;
        for route in &routes {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
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

        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        let scene = RouteScenePlan {
            routes,
            extent: (width, height),
            planned_cell_count,
            labels: Rc::clone(label_plans),
            quality_requested,
        };
        Ok(scene)
    })();
    if quality {
        allowance.account_scene_work(start_work, initial_allowance, resources);
    }
    result
}

fn try_undisplaced_route(
    request: EdgeRouteRequest<'_>,
    obstacles: path::RouteObstacles<'_>,
    label: Option<label::RoutedLabelDescriptor>,
    allowance: &crate::operation::SearchAllowance,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    // A blocked direct path need not displace another edge. Split the remaining
    // slice between an unchanged-scene trial and repair, without adding credit.
    let work = allowance
        .remaining()
        .1
        .min(resources.remaining_planning_work().unwrap_or(usize::MAX))
        / 2;
    if work == 0 {
        return Ok(None);
    }
    let mut trial = resources.clone().with_planning_work_allowance(work);
    match terminal::plan_terminal_route_with_budget(
        request,
        obstacles,
        label,
        None,
        terminal::SearchBudget::Limited(allowance),
        &mut trial,
        execution,
    ) {
        Err(AsciiError::PlanningWorkAllowanceExhausted) => Ok(None),
        result => result,
    }
}

#[allow(clippy::too_many_arguments)]
fn repair_blocked_scene_for_input(
    request: EdgeRouteRequest<'_>,
    input: &SceneRoutingInput<'_>,
    owner: &RouteOwner,
    witnesses: &conflict::BlockerWitnesses,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    budget: &mut repair::SceneRepairBudget<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    repair::repair_blocked_scene(
        request,
        input.topology.as_ref(),
        &input.labels,
        owner,
        witnesses,
        routes,
        occupancy,
        budget,
        resources,
        execution,
    )
}

fn flush_deferred_loop_labels(
    routes: &mut [PreparedRoute],
    start: usize,
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    for route_index in start..routes.len() {
        allocate_committed_route_labels(routes, route_index, occupancy, resources, diagram_type)?;
    }
    Ok(())
}

fn plan_allocated_channel(
    request: EdgeRouteRequest<'_>,
    branch: &allocation::AllocatedBranch,
    descriptor: Option<label::RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<RoutePlan>> {
    let Some(path) = terminal::direct_path(
        branch.points,
        branch.attachments.start.outward,
        branch.attachments.end.outward.opposite(),
        resources,
        execution,
    )?
    else {
        return Ok(None);
    };
    let label = if let Some(descriptor) = descriptor {
        let Some([start, end]) = branch.label_host else {
            return Ok(None);
        };
        let Some(placement) = label::routed_label_placement_for_descriptor(start, end, descriptor)
        else {
            return Ok(None);
        };
        let mut label = plan::PlannedRouteLabel::new(descriptor, placement);
        label.anchor = plan::LabelAnchor::Segment {
            start,
            end,
            route_segment: None,
        };
        Some(label)
    } else {
        None
    };
    plan::orthogonal::plan_terminal_path(
        &path,
        branch.attachments,
        request.edge,
        label,
        request.charset,
        resources,
    )
}

fn independent_incident_demand(
    edges: &[AsciiGraphEdge],
    resources: &ResourceContext,
) -> Result<Vec<bool>> {
    let mut pairs = HashMap::<(&str, &str), usize>::default();
    let mut loops = rustc_hash::FxHashSet::default();
    pairs
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    loops
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    fn key(edge: &AsciiGraphEdge) -> (&str, &str) {
        if edge.from <= edge.to {
            (&edge.from, &edge.to)
        } else {
            (&edge.to, &edge.from)
        }
    }
    for edge in edges.iter().filter(|edge| edge.stroke.is_visible()) {
        resources.checkpoint()?;
        resources
            .charge_layout_work(resources.checked_work_add(edge.from.len(), edge.to.len())?)?;
        let count = pairs.entry(key(edge)).or_default();
        *count = resources.checked_work_add(*count, 1)?;
        if edge.from == edge.to {
            loops.insert(edge.from.as_str());
        }
    }
    let mut demand = Vec::new();
    demand
        .try_reserve_exact(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        resources.checkpoint()?;
        resources
            .charge_layout_work(resources.checked_work_add(edge.from.len(), edge.to.len())?)?;
        demand.push(
            pairs.get(&key(edge)).is_some_and(|count| *count > 1)
                || loops.contains(edge.from.as_str())
                || loops.contains(edge.to.as_str()),
        );
    }
    Ok(demand)
}

/// Read the common scope from the resolved frames, without rerunning group placement.
/// Unresolved implicit directions retain the existing router rather than guessing an axis.
fn compound_route_direction(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    resources: &ResourceContext,
) -> Result<Option<GraphDirection>> {
    // Keep flat routing, including its exact small-grid work boundary, unchanged.
    if graph.diagram_type() != "flowchart" || layout.groups.is_empty() || from.id == to.id {
        return Ok(None);
    }
    let mut common: Option<&GroupLayout> = None;
    for group in &layout.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if group.kind == super::model::GraphGroupKind::Container
            && geometry::group_contains_node(group, from)
            && geometry::group_contains_node(group, to)
            && common.is_none_or(|outer| {
                outer.x <= group.x
                    && group.right() <= outer.right()
                    && outer.y <= group.y
                    && group.bottom() <= outer.bottom()
            })
        {
            common = Some(group);
        }
    }
    let Some(common) = common else {
        return Ok(Some(graph.direction.canonical()));
    };
    for group in &graph.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if group.id == common.id {
            return Ok(group
                .direction
                .map(|direction| direction.before_root_output_transform(graph.direction)));
        }
    }
    Ok(None)
}

fn forward_route_direction(
    request: EdgeRouteRequest<'_>,
    resources: &ResourceContext,
) -> Result<Option<StepDirection>> {
    let Some(direction) = compound_route_direction(
        request.graph,
        request.graph_layout,
        request.from,
        request.to,
        resources,
    )?
    else {
        return Ok(None);
    };
    // A preference is only meaningful when the resolved contours leave a forward gap.
    // Feedback, overlapping ranks and ancestor/member endpoints keep all their contacts.
    Ok(match direction {
        GraphDirection::LeftRight if request.from.right() < request.to.x => {
            Some(StepDirection::Right)
        }
        GraphDirection::RightLeft if request.to.right() < request.from.x => {
            Some(StepDirection::Left)
        }
        GraphDirection::TopDown if request.from.bottom() < request.to.y => {
            Some(StepDirection::Down)
        }
        GraphDirection::BottomTop if request.to.bottom() < request.from.y => {
            Some(StepDirection::Up)
        }
        _ => None,
    })
}

fn contacts_follow_direction(
    forward: Option<StepDirection>,
    attachments: Option<plan::NodeAttachments>,
) -> bool {
    forward.is_none_or(|forward| {
        attachments.is_some_and(|attachments| {
            attachments.start.outward == forward && attachments.end.outward == forward.opposite()
        })
    })
}

/// Endpoint reuse precedes contact preference; legal crossings keep their geometry cost.
/// With no known scope direction, retain the original score ordering exactly.
fn route_candidate_is_better(
    forward: Option<StepDirection>,
    candidate: (RouteCandidateScore, Option<plan::NodeAttachments>),
    incumbent: (RouteCandidateScore, Option<plan::NodeAttachments>),
) -> bool {
    if forward.is_none() {
        return candidate.0 < incumbent.0;
    }
    let key = |(score, attachments): (RouteCandidateScore, Option<plan::NodeAttachments>)| {
        (
            score.port_reuse(),
            !contacts_follow_direction(forward, attachments),
            score,
        )
    };
    key(candidate) < key(incumbent)
}

fn route_needs_search(
    forward: Option<StepDirection>,
    score: RouteCandidateScore,
    plan: &RoutePlan,
    defer_loop_labels: bool,
) -> bool {
    score.has_congestion()
        || (!defer_loop_labels && score.grows_scene())
        || !contacts_follow_direction(forward, plan.attachments)
}

fn plan_route_order(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    resources: &ResourceContext,
) -> Result<Vec<usize>> {
    resources.charge_layout_work(resources.checked_work_add(layout.nodes.len(), edges.len())?)?;
    let mut visible_count = 0;
    for (index, edge) in edges.iter().enumerate() {
        if index.is_multiple_of(64) {
            resources.checkpoint()?;
        }
        visible_count += usize::from(edge.stroke.is_visible());
    }
    let mut order = Vec::new();
    order
        .try_reserve(visible_count)
        .map_err(|_| layout_allocation_failed())?;
    if visible_count == 0 {
        return Ok(order);
    }
    let mut levels = HashMap::default();
    levels
        .try_reserve(layout.nodes.len())
        .map_err(|_| layout_allocation_failed())?;
    let horizontal = graph.direction.canonical() == super::model::GraphDirection::LeftRight;
    for node in &layout.nodes {
        resources.checkpoint()?;
        levels.insert(
            node.id.as_str(),
            (if horizontal { node.grid.x } else { node.grid.y }, node),
        );
    }
    resources.charge_layout_work(edges.len())?;
    for (index, edge) in edges.iter().enumerate() {
        resources.checkpoint()?;
        if edge.stroke.is_visible() {
            order.push(index);
        }
    }
    let mut scoped_ranks = HashMap::default();
    if graph.diagram_type() == "flowchart" && !layout.groups.is_empty() {
        resources.charge_layout_work(order.len())?;
        scoped_ranks
            .try_reserve(order.len())
            .map_err(|_| layout_allocation_failed())?;
        for &index in &order {
            let edge = &edges[index];
            let Some(((_, from), (_, to))) = levels
                .get(edge.from.as_str())
                .zip(levels.get(edge.to.as_str()))
            else {
                continue;
            };
            let Some(direction) = compound_route_direction(graph, layout, from, to, resources)?
            else {
                continue;
            };
            let ranks = match direction {
                GraphDirection::LeftRight => (from.center_x(), to.center_x()),
                GraphDirection::RightLeft => (to.center_x(), from.center_x()),
                GraphDirection::TopDown => (from.center_y(), to.center_y()),
                GraphDirection::BottomTop => (to.center_y(), from.center_y()),
            };
            scoped_ranks.insert(index, ranks);
        }
    }
    super::layout::charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by_key(|&index| {
        let edge = &edges[index];
        let ranks = scoped_ranks.get(&index).copied().or_else(|| {
            levels
                .get(edge.from.as_str())
                .zip(levels.get(edge.to.as_str()))
                .map(|((from, _), (to, _))| (*from, *to))
        });
        let feedback = edge.from != edge.to && ranks.is_some_and(|(from, to)| from >= to);
        let span = ranks.map_or(usize::MAX, |(from, to)| from.abs_diff(to));
        (feedback, span, index)
    });
    Ok(order)
}

fn admit_route_candidate(
    request: EdgeRouteRequest<'_>,
    obstacles: path::RouteObstacles<'_>,
    plan: RoutePlan,
    resources: &mut ResourceContext,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    if plan.labels.is_empty()
        && let Some(raw) = request.edge.label.as_deref()
        && super::label::try_measure_edge_label(
            raw,
            request.charset.width_profile,
            resources,
            request.edge.label_break_policy,
        )?
        .is_some()
    {
        return Ok(None);
    }
    let Some(plan) = plan.resolve_endpoint_attachments(
        plan::EndpointGeometry::for_route(
            request.from,
            request.to,
            request.graph_layout,
            resources,
        )?,
        plan::EndpointGeometry::for_route(
            request.to,
            request.from,
            request.graph_layout,
            resources,
        )?,
        request.charset,
        resources,
    )?
    else {
        return Ok(None);
    };
    if !(geometry::CompoundRouteGeometry {
        layout: request.graph_layout,
        from: request.from,
        to: request.to,
        attachments: plan.attachments,
    })
    .endpoints_allow_plan(&plan, resources)?
    {
        return Ok(None);
    }
    let plan = plan
        .with_marker_requests(
            request.edge.start_marker,
            request.edge.end_marker,
            request.graph.diagram_type(),
        )?
        .with_style(request.edge.style);
    obstacles.scene.admit_route(
        obstacles.routes,
        plan,
        obstacles.owner,
        resources,
        request.graph.diagram_type(),
    )
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
    for (group_index, group) in graph.groups.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let crosses_boundary =
            from_groups.contains(&group_index) != to_groups.contains(&group_index);
        // Region separators are open lines, not closed containers. A transition can
        // cross intermediate separators even when neither endpoint belongs to that region.
        // CompoundRouteGeometry still reserves unrelated container interiors.
        if group.kind == super::model::GraphGroupKind::Divider
            || crosses_boundary
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
        merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
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
        merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
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
