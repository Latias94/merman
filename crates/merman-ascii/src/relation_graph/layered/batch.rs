use super::super::{
    LayeredRelationPaintPlan, RelationComponentAdapter, RelationGraphBox, layout_allocation_failed,
};
use super::route::{
    LayeredRelationRouteGeometry, LayeredRelationRoutePlan, LayeredRelationRouteStyle,
    materialize_layered_relation_route_plan,
};
use super::scene::{
    LayeredRelationScene, LayeredRelationScenePlan, LayeredRelationSummaryReason,
    plan_layered_relation_scene,
};
use crate::options::AsciiRenderOptions;
use crate::resource::{LogicalExtent, ResourceContext};
use crate::{AsciiError, Result};

#[derive(Debug)]
pub(in crate::relation_graph) enum LayeredRouteBatchError {
    Resource(AsciiError),
    Semantic(LayeredRelationSummaryReason),
}

impl From<AsciiError> for LayeredRouteBatchError {
    fn from(error: AsciiError) -> Self {
        Self::Resource(error)
    }
}

struct PlannedLayeredRoute {
    edge_index: usize,
    style: LayeredRelationRouteStyle,
    geometry: LayeredRelationRouteGeometry,
}

pub(in crate::relation_graph) fn plan_layered_relation_component_ref_result<'boxes, 'text, R, A>(
    boxes: &[&'boxes RelationGraphBox],
    relations: &[&R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let mut has_self_relation = false;
    let mut has_non_self_relation = false;
    for relation in relations {
        resources.checkpoint()?;
        if adapter.is_self_relation(*relation) {
            has_self_relation = true;
        } else {
            has_non_self_relation = true;
        }
    }
    if has_self_relation && has_non_self_relation {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        resources.checkpoint()?;
        edges.push(adapter.build_edges(*relation));
    }
    let scene = match plan_layered_relation_scene(
        boxes,
        edges,
        horizontal_gap,
        options.terminal_width_profile,
        resources,
    )
    .map_err(|error| error.into_ascii_error(|semantic| adapter.layered_error(semantic)))?
    {
        LayeredRelationScenePlan::Routed(scene) => scene,
        LayeredRelationScenePlan::Summary(reason) => {
            return Ok(Err(reason));
        }
    };

    let (route_plans, extent) =
        match plan_layered_route_batch(&scene, relations, resources, adapter) {
            Ok(plan) => plan,
            Err(LayeredRouteBatchError::Resource(error)) => return Err(error),
            Err(LayeredRouteBatchError::Semantic(reason)) => return Ok(Err(reason)),
        };

    Ok(Ok(LayeredRelationPaintPlan {
        scene,
        routes: route_plans,
        extent,
    }))
}

#[cfg(test)]
pub(in crate::relation_graph) fn plan_layered_relation_component_result<'boxes, 'text, R, A>(
    boxes: &'boxes [RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let relation_refs = relations.iter().collect::<Vec<_>>();
    plan_layered_relation_component_ref_result(
        &box_refs,
        &relation_refs,
        options,
        horizontal_gap,
        resources,
        adapter,
    )
}

pub(in crate::relation_graph) fn plan_layered_route_batch<'text, R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    resources: &ResourceContext,
    adapter: &A,
) -> std::result::Result<(Vec<LayeredRelationRoutePlan>, LogicalExtent), LayeredRouteBatchError>
where
    A: RelationComponentAdapter<'text, R>,
{
    let outcome: Result<
        std::result::Result<
            (Vec<LayeredRelationRoutePlan>, LogicalExtent),
            LayeredRelationSummaryReason,
        >,
    > = resources.transaction(|resources| {
        match resources.transaction_preserving_layout_work(|resources| {
            plan_layered_route_batch_in_transaction(scene, relations, resources, adapter)
        }) {
            Ok(plan) => Ok(Ok(plan)),
            Err(LayeredRouteBatchError::Semantic(reason)) => Ok(Err(reason)),
            Err(LayeredRouteBatchError::Resource(error)) => Err(error),
        }
    });

    match outcome {
        Ok(Ok(plan)) => Ok(plan),
        Ok(Err(reason)) => Err(LayeredRouteBatchError::Semantic(reason)),
        Err(error) => Err(LayeredRouteBatchError::Resource(error)),
    }
}

fn plan_layered_route_batch_in_transaction<'text, R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    resources: &ResourceContext,
    adapter: &A,
) -> std::result::Result<(Vec<LayeredRelationRoutePlan>, LogicalExtent), LayeredRouteBatchError>
where
    A: RelationComponentAdapter<'text, R>,
{
    resources.charge_layout_work(scene.draw_order().len().max(1))?;
    let mut planned = Vec::new();
    planned
        .try_reserve_exact(scene.draw_order().len())
        .map_err(|_| layout_allocation_failed())?;
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
        resources.checkpoint()?;
        let Some(relation) = relations.get(edge_index).copied() else {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        };
        let style = adapter.layered_route_style(relation)?;
        let Some(geometry) =
            scene.plan_edge_geometry(edge_index, lane_offset, style.profile(), resources)?
        else {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        };

        if !scene.edge_ports_fit(edge_index, &geometry) {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        }

        planned.push(PlannedLayeredRoute {
            edge_index,
            style,
            geometry,
        });
    }

    validate_layered_route_geometries(scene, &planned, resources)?;

    let mut route_plans = Vec::new();
    route_plans
        .try_reserve_exact(planned.len())
        .map_err(|_| layout_allocation_failed())?;
    for route in planned {
        resources.checkpoint()?;
        let Some(relation) = relations.get(route.edge_index).copied() else {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        };
        let overlays = adapter.layered_relation_overlays(relation, &route.geometry, resources)?;
        route_plans.push(materialize_layered_relation_route_plan(
            route.geometry,
            route.style,
            resources,
            overlays,
        )?);
    }
    validate_layered_route_batch(scene, &route_plans, resources)?;
    let extent = resources.grid_extent(scene.width(), scene.height())?;
    Ok((route_plans, extent))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::relation_graph) struct PairwiseValidationWork {
    pub(in crate::relation_graph) segment_count: usize,
    pub(in crate::relation_graph) overlay_count: usize,
    pub(in crate::relation_graph) pair_work: usize,
}

pub(in crate::relation_graph) fn measure_pairwise_validation_work(
    counts: impl IntoIterator<Item = (usize, usize)>,
    resources: &ResourceContext,
    include_planar_route_pairs: bool,
) -> Result<PairwiseValidationWork> {
    let mut segment_count = 0usize;
    let mut overlay_count = 0usize;
    let mut pair_work = 0usize;
    for (segments, overlays) in counts {
        pair_work = resources.checked_work_add(
            pair_work,
            resources.checked_work_mul(overlays, overlay_count)?,
        )?;
        if include_planar_route_pairs {
            let route_pairs = resources.checked_work_mul(segments, segment_count)?;
            let overlay_to_prior_routes = resources.checked_work_mul(overlays, segment_count)?;
            let route_to_prior_overlays = resources.checked_work_mul(segments, overlay_count)?;
            pair_work = resources.checked_work_add(
                pair_work,
                resources.checked_work_add(
                    route_pairs,
                    resources.checked_work_add(overlay_to_prior_routes, route_to_prior_overlays)?,
                )?,
            )?;
        }
        segment_count = resources.checked_work_add(segment_count, segments)?;
        overlay_count = resources.checked_work_add(overlay_count, overlays)?;
    }
    Ok(PairwiseValidationWork {
        segment_count,
        overlay_count,
        pair_work,
    })
}

fn validate_layered_route_geometries(
    scene: &LayeredRelationScene<'_>,
    routes: &[PlannedLayeredRoute],
    resources: &ResourceContext,
) -> std::result::Result<(), LayeredRouteBatchError> {
    resources.charge_layout_work(routes.len().max(1))?;
    let measured = measure_pairwise_validation_work(
        routes
            .iter()
            .map(|route| (route.geometry.segment_count(), 0)),
        resources,
        scene.is_planar_k2_2(),
    )?;
    let box_work =
        resources.checked_work_mul(measured.segment_count, scene.placed_box_count().max(1))?;
    resources.charge_layout_work(resources.checked_work_add(box_work, measured.pair_work)?)?;
    if scene.is_planar_k2_2() && routes.len() != 4 {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    if routes
        .iter()
        .any(|route| !route.geometry.fits(scene.width(), scene.height()))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    if scene.is_planar_k2_2() {
        for (index, route) in routes.iter().enumerate() {
            resources.checkpoint()?;
            for other in &routes[index + 1..] {
                resources.checkpoint()?;
                if route.geometry.overlaps(&other.geometry) {
                    return Err(LayeredRouteBatchError::Semantic(
                        LayeredRelationSummaryReason::RouteCollision,
                    ));
                }
            }
        }
    }
    for route in routes {
        resources.checkpoint()?;
        if scene.route_geometry_overlaps_box(&route.geometry) {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        }
    }
    Ok(())
}

fn validate_layered_route_batch(
    scene: &LayeredRelationScene<'_>,
    route_plans: &[LayeredRelationRoutePlan],
    resources: &ResourceContext,
) -> std::result::Result<(), LayeredRouteBatchError> {
    resources.charge_layout_work(route_plans.len().max(1))?;
    let measured = measure_pairwise_validation_work(
        route_plans
            .iter()
            .map(|route| (route.segment_count(), route.overlay_count())),
        resources,
        scene.is_planar_k2_2(),
    )?;
    let box_work = resources.checked_work_mul(
        resources.checked_work_add(measured.segment_count, measured.overlay_count)?,
        scene.placed_box_count().max(1),
    )?;
    resources.charge_layout_work(resources.checked_work_add(box_work, measured.pair_work)?)?;
    if scene.is_planar_k2_2() && route_plans.len() != 4 {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    if route_plans
        .iter()
        .any(|route_plan| !route_plan.route_fits(scene.width(), scene.height()))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    if route_plans
        .iter()
        .any(|route_plan| !route_plan.overlays_fit(scene.width(), scene.height()))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::OverlayCollision,
        ));
    }
    for (index, route_plan) in route_plans.iter().enumerate() {
        resources.checkpoint()?;
        for other in &route_plans[index + 1..] {
            resources.checkpoint()?;
            if route_plan.overlays_overlap(other) {
                return Err(LayeredRouteBatchError::Semantic(
                    LayeredRelationSummaryReason::OverlayCollision,
                ));
            }
        }
    }
    if scene.is_planar_k2_2() {
        for (index, route_plan) in route_plans.iter().enumerate() {
            resources.checkpoint()?;
            for other in &route_plans[index + 1..] {
                resources.checkpoint()?;
                if route_plan.route_overlaps(other) {
                    return Err(LayeredRouteBatchError::Semantic(
                        LayeredRelationSummaryReason::RouteCollision,
                    ));
                }
                if route_plan.overlays_overlap_route(other)
                    || other.overlays_overlap_route(route_plan)
                {
                    return Err(LayeredRouteBatchError::Semantic(
                        LayeredRelationSummaryReason::OverlayCollision,
                    ));
                }
            }
        }
    }
    for route_plan in route_plans {
        resources.checkpoint()?;
        if scene.route_overlaps_box(route_plan) {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::RouteCollision,
            ));
        }
        if scene.overlays_overlap_box(route_plan) {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::OverlayCollision,
            ));
        }
    }
    Ok(())
}
