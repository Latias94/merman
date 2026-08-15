use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::DeferredTextRegistry;
use crate::text::StyledLine;
use crate::{AsciiError, Result};
#[cfg(test)]
use std::rc::Rc;
mod document;
mod encode;
mod horizontal;
mod layered;
mod model;
mod self_loop;
mod stack;
mod summary;

#[cfg(test)]
pub(crate) use self::document::RelationDocumentPlan;
#[cfg(test)]
use self::document::relation_lines_extent;
pub(crate) use self::document::{
    LayeredRelationPaintPlan, RelationBoxStripPlan, RelationRegionPlan, RelationRenderPlan,
    RelationSummaryPaintPlan, render_relation_document_with_summary,
};
pub(crate) use self::encode::{
    render_lines_with_deferred_options, render_lines_with_deferred_options_with_execution,
};
#[cfg(test)]
pub(crate) use self::encode::{render_lines_with_deferred_probe, render_lines_with_options};
pub(crate) use self::horizontal::*;
// Keep the inferred `source_port`/`target_port` return type reachable to
// sibling family modules even though callers do not name it directly.
#[allow(unused_imports)]
pub(crate) use self::layered::LayeredRelationPhysicalPort;
pub(crate) use self::layered::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationPhysicalSide,
    LayeredRelationRouteGeometry, LayeredRelationRouteProfile, LayeredRelationRouteStyle,
    LayeredRelationSummaryReason, RelationLineChars, RelationOverlay,
    centered_label_lines_with_role, centered_text_line_with_role, label_lines_with_role,
    marker_line_with_role, put_relation_char,
};
#[cfg(test)]
use self::layered::{
    LayeredRelationRoutePlan, LayeredRelationRouteRequest, LayeredRelationScene,
    LayeredRelationScenePlan, LayeredRouteBatchError, PairwiseValidationWork,
    PlacedRelationGraphBox, measure_pairwise_validation_work, parallel_relation_lane_offsets,
    plan_layered_relation_boxes, plan_layered_relation_component_result,
    plan_layered_relation_route, plan_layered_relation_scene, plan_layered_route_batch,
    spanning_lane_offset_around_intermediate_boxes, write_centered_relation_label,
};
use self::layered::{
    RelationGraphComponent, plan_layered_relation_component_ref_result, relation_components,
};
pub(crate) use self::model::{
    RelationGraphBox, RelationGraphBoxStyle, RelationGraphLabel, RelationGraphLine,
};
pub(crate) use self::self_loop::{
    RelationSelfLoopMetrics, RelationSelfLoopPlan, RelationSelfLoopRows,
};
pub(crate) use self::stack::{
    RelationParallelPlan, RelationStackPlan, centered_row_blocks_extent,
    render_stacked_boxes_with_deferred_options,
    render_stacked_boxes_with_deferred_options_with_execution, stacked_box_extent,
    stacked_box_lines, stacked_box_lines_ordered,
};
#[cfg(test)]
pub(crate) use self::stack::{render_stacked_boxes, render_stacked_boxes_with_section};
pub(crate) use self::summary::*;

pub(crate) trait RelationComponentAdapter<'text, R> {
    fn build_edges(&self, relation: &R) -> LayeredRelationEdge;

    fn is_self_relation(&self, relation: &R) -> bool;

    /// Describe a self-loop without constructing any styled terminal rows.
    ///
    /// Families own marker/cardinality semantics; the shared renderer only
    /// consumes the resulting geometry metrics for resource admission.
    fn self_loop_metrics(
        &self,
        relation: &R,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopMetrics>;

    fn self_loop_rows(
        &self,
        relation: &R,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopRows>;

    fn horizontal_relation_style(
        &self,
        relation: &R,
        source_side: RelationPortSide,
        target_side: RelationPortSide,
        resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle>;

    fn layered_horizontal_gap(&self) -> usize;

    fn layered_route_style(&self, relation: &R) -> Result<LayeredRelationRouteStyle>;

    fn layered_relation_overlays(
        &self,
        relation: &R,
        geometry: &LayeredRelationRouteGeometry,
        resources: &ResourceContext,
    ) -> Result<Vec<RelationOverlay>>;

    fn plan_vertical_region<'plan>(
        &self,
        boxes: &[&'plan RelationGraphBox],
        relation: &'plan R,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>>;

    fn plan_parallel_region<'plan>(
        &self,
        boxes: Vec<&'plan RelationGraphBox>,
        relations: Vec<&'plan R>,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        deferred: &mut DeferredTextRegistry<'text>,
    ) -> Result<RelationRegionPlan<'plan>>;

    fn build_summary_row(
        &self,
        relation: &R,
        reason: LayeredRelationSummaryReason,
        resources: &ResourceContext,
        deferred: &mut DeferredTextRegistry<'text>,
    ) -> Result<RelationGraphSummaryRow>;

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError;
}

fn build_layered_edges<'text, R, A>(
    relations: &[R],
    adapter: &A,
    resources: &mut ResourceContext,
) -> Result<Vec<LayeredRelationEdge>>
where
    A: RelationComponentAdapter<'text, R>,
{
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        resources.checkpoint()?;
        edges.push(adapter.build_edges(relation));
    }
    Ok(edges)
}

pub(crate) fn render_relation_components_with_deferred<'text, R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<String>
where
    A: RelationComponentAdapter<'text, R>,
{
    let lines = materialize_relation_component_lines(
        boxes, relations, options, resources, adapter, deferred,
    )?;
    render_lines_with_deferred_options(&lines, options, resources, deferred)
}

pub(crate) fn render_relation_components_with_deferred_with_execution<'text, R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<String>
where
    A: RelationComponentAdapter<'text, R>,
{
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let lines = materialize_relation_component_lines_with_execution(
        boxes, relations, options, resources, adapter, deferred, execution,
    )?;
    render_lines_with_deferred_options_with_execution(
        &lines, options, resources, deferred, execution,
    )
}

pub(crate) fn render_relation_component_lines<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    materialize_relation_component_lines(boxes, relations, options, resources, adapter, deferred)
}

fn plan_relation_components<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<RelationRenderPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let edges = build_layered_edges(relations, adapter, resources)?;
    for _ in boxes {
        resources.checkpoint()?;
    }
    let layered_error = |error| adapter.layered_error(error);
    let components = relation_components(boxes, &edges, resources)
        .map_err(|error| error.into_ascii_error(layered_error))?;
    resources.charge_layout_work(components.len().max(1))?;
    let mut relation_regions = Vec::new();
    relation_regions
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut standalone_regions = Vec::new();
    standalone_regions
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    for component in components {
        resources.checkpoint()?;
        let has_relations = !component.edge_indices().is_empty();
        let region = plan_relation_component_region(
            component, relations, options, resources, adapter, deferred,
        )?;
        if has_relations {
            relation_regions.push(region);
        } else {
            standalone_regions.push(region);
        }
    }

    let mut regions = Vec::new();
    regions
        .try_reserve_exact(
            relation_regions
                .len()
                .checked_add(standalone_regions.len())
                .ok_or_else(|| work_overflow(resources))?,
        )
        .map_err(|_| layout_allocation_failed())?;
    if relation_regions.len() > 1 && relation_regions.iter().all(|region| !region.is_summary()) {
        regions.push(RelationRegionPlan::horizontal_strip(
            relation_regions,
            adapter.layered_horizontal_gap(),
            resources,
        )?);
    } else {
        regions.extend(relation_regions);
    }
    regions.extend(standalone_regions);
    RelationRenderPlan::try_new(regions, resources)
}

fn materialize_relation_component_lines<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let plan = plan_relation_components(boxes, relations, options, resources, adapter, deferred)?;
    plan.materialize(options, resources)
}

fn materialize_relation_component_lines_with_execution<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let mut layout_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Layout);
    let plan = plan_relation_components(
        boxes,
        relations,
        options,
        &mut layout_resources,
        adapter,
        deferred,
    )?;
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    emit_resources.checkpoint()?;
    let lines = plan.materialize(options, &mut emit_resources)?;
    for _ in &lines {
        emit_resources.checkpoint()?;
    }
    Ok(lines)
}

pub(crate) fn render_relation_component_lines_with_execution<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    materialize_relation_component_lines_with_execution(
        boxes, relations, options, resources, adapter, deferred, execution,
    )
}

fn plan_relation_component_region<'plan, 'text, R, A>(
    component: RelationGraphComponent<'plan>,
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let (component_boxes, edge_indices) = component.into_parts();
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
        resources.checkpoint()?;
        selected.push(
            relations
                .get(edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?,
        );
    }
    if selected.is_empty() {
        return Ok(RelationRegionPlan::BoxStrip(RelationBoxStripPlan::stacked(
            component_boxes,
            resources,
        )?));
    }

    let mut has_self = false;
    let mut has_non_self = false;
    for relation in &selected {
        resources.checkpoint()?;
        if adapter.is_self_relation(*relation) {
            has_self = true;
        } else {
            has_non_self = true;
        }
    }
    if has_self && has_non_self {
        return plan_relation_summary_region(
            component_boxes,
            selected,
            LayeredRelationSummaryReason::RouteCollision,
            options,
            resources,
            adapter,
            deferred,
        );
    }

    if has_self {
        let first_edge = adapter.build_edges(selected[0]);
        let mut same_endpoint = true;
        for relation in &selected {
            resources.checkpoint()?;
            let edge = adapter.build_edges(*relation);
            if edge.source_id() != first_edge.source_id()
                || edge.target_id() != first_edge.target_id()
            {
                same_endpoint = false;
                break;
            }
        }
        if same_endpoint {
            let relation_box = find_box_ref(&component_boxes, first_edge.source_id())
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            return plan_relation_self_loop_region(relation_box, selected, adapter, resources);
        }
    }

    if selected.len() > 1 && same_directed_endpoints(&selected, adapter, resources)? {
        return adapter.plan_parallel_region(
            component_boxes,
            selected,
            options,
            resources,
            deferred,
        );
    }
    if let [relation] = selected.as_slice() {
        return adapter.plan_vertical_region(&component_boxes, relation, resources);
    }

    match plan_layered_relation_component_ref_result(
        &component_boxes,
        &selected,
        options,
        adapter.layered_horizontal_gap(),
        resources,
        adapter,
    )? {
        Ok(plan) => Ok(RelationRegionPlan::Layered(plan)),
        Err(reason) => plan_relation_summary_region(
            component_boxes,
            selected,
            reason,
            options,
            resources,
            adapter,
            deferred,
        ),
    }
}

fn same_directed_endpoints<'text, R, A>(
    relations: &[&R],
    adapter: &A,
    resources: &ResourceContext,
) -> Result<bool>
where
    A: RelationComponentAdapter<'text, R>,
{
    let Some(first) = relations.first() else {
        return Ok(false);
    };
    let first = adapter.build_edges(first);
    for relation in relations.iter().skip(1) {
        resources.checkpoint()?;
        let edge = adapter.build_edges(*relation);
        if edge.source_id() != first.source_id() || edge.target_id() != first.target_id() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn plan_relation_self_loop_region<'plan, 'text, R, A>(
    relation_box: &'plan RelationGraphBox,
    relations: Vec<&'plan R>,
    adapter: &'plan A,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let mut metrics = Vec::new();
    metrics
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in &relations {
        metrics.push(adapter.self_loop_metrics(relation, resources)?);
    }
    let plan = RelationSelfLoopPlan::try_new(relation_box, metrics, resources)?;
    Ok(RelationRegionPlan::SelfLoops {
        plan,
        rows: Box::new(move |resources| {
            let mut loops = Vec::new();
            loops
                .try_reserve_exact(relations.len())
                .map_err(|_| layout_allocation_failed())?;
            for relation in relations {
                loops.push(adapter.self_loop_rows(relation, resources)?);
            }
            Ok(loops)
        }),
    })
}

fn plan_relation_summary_region<'plan, 'text, R, A>(
    boxes: Vec<&'plan RelationGraphBox>,
    relations: Vec<&'plan R>,
    reason: LayeredRelationSummaryReason,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let mut rows = Vec::new();
    rows.try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        rows.push(adapter.build_summary_row(relation, reason, resources, deferred)?);
    }
    Ok(RelationRegionPlan::Summary(
        RelationSummaryPaintPlan::stacked(boxes, rows, Some(reason), options, resources)?,
    ))
}

#[cfg(test)]
pub(crate) fn render_layered_relation_component<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    policy: AsciiResourcePolicy,
    horizontal_gap: usize,
    adapter: &A,
) -> Result<String>
where
    for<'text> A: RelationComponentAdapter<'text, R>,
{
    let mut resources = ResourceContext::new(policy);
    let mut deferred = DeferredTextRegistry::new();
    let lines = render_layered_relation_component_lines(
        boxes,
        relations,
        options,
        horizontal_gap,
        &mut resources,
        adapter,
        &mut deferred,
    )?;
    render_lines_with_deferred_options(&lines, options, &mut resources, &deferred)
}

#[cfg(test)]
pub(crate) fn render_layered_relation_component_lines<'text, R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R>,
{
    match render_layered_relation_component_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )? {
        Ok(rendered) => Ok(rendered),
        Err(reason) => {
            let mut rows = Vec::new();
            rows.try_reserve_exact(relations.len())
                .map_err(|_| layout_allocation_failed())?;
            for relation in relations {
                rows.push(adapter.build_summary_row(relation, reason, resources, deferred)?);
            }
            let base_extent = stacked_box_extent(boxes, resources)?;
            render_relation_document_with_summary(
                base_extent,
                &rows,
                Some(reason),
                options,
                resources,
                |resources| stacked_box_lines(boxes, options.terminal_width_profile, resources),
            )
        }
    }
}

#[cfg(test)]
fn render_layered_relation_component_result<'text, R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<Vec<RelationGraphLine>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<'text, R>,
{
    match plan_layered_relation_component_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )? {
        Ok(plan) => Ok(Ok(plan.paint(options, resources)?)),
        Err(reason) => Ok(Err(reason)),
    }
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn try_share_relation_box_lines(relation_box: &RelationGraphBox) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(relation_box.height())
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    Ok(lines)
}

pub(crate) fn find_box<'a>(
    boxes: &'a [RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
    boxes.iter().find(|relation_box| relation_box.id() == id)
}

pub(crate) fn find_box_ref<'a>(
    boxes: &[&'a RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
    boxes
        .iter()
        .copied()
        .find(|relation_box| relation_box.id() == id)
}

pub(crate) fn try_concat_relation_lines(
    parts: Vec<RelationGraphLine>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let mut line = StyledLine::with_resources(width_profile, resources);
    for part in parts {
        line.try_push_line(&part.line)?;
    }
    Ok(RelationGraphLine::from_styled(line))
}

#[cfg(test)]
mod tests;
