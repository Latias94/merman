#[cfg(test)]
use crate::canvas::{
    finish_styled_line_iter_with_deferred_probe, finish_styled_line_iter_with_resources,
};
use crate::canvas::{
    finish_styled_line_iter_with_deferred_resources,
    finish_styled_line_iter_with_deferred_resources_with_execution,
};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::DeferredTextRegistry;
use crate::text::{StyledLine, display_width_with_profile};
use crate::{AsciiError, Result};
#[cfg(test)]
use std::rc::Rc;
mod document;
mod horizontal;
mod layered;
mod model;
mod self_loop;
mod stack;
mod summary;

#[cfg(test)]
use self::document::relation_lines_extent;
pub(crate) use self::document::{
    LayeredRelationPaintPlan, RelationBoxStripPlan, RelationDocumentPlan, RelationRegionPlan,
    RelationRenderPlan, RelationSummaryPaintPlan,
};
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
    plan_layered_route_batch_with_probes, spanning_lane_offset_around_intermediate_boxes,
    write_centered_relation_label,
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
pub(crate) use self::stack::{RelationParallelPlan, RelationStackPlan, centered_row_blocks_extent};
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

#[cfg(test)]
pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn render_stacked_boxes_with_deferred_options(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
) -> Result<String> {
    let lines = stacked_box_lines(boxes, options.terminal_width_profile, resources)?;
    render_lines_with_deferred_options(&lines, options, resources, deferred)
}

pub(crate) fn render_stacked_boxes_with_deferred_options_with_execution(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut layout_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Layout);
    let lines = stacked_box_lines_ordered_impl(
        boxes,
        options.terminal_width_profile,
        false,
        &mut layout_resources,
        Some(execution),
    )?;
    render_lines_with_deferred_options_with_execution(
        &lines, options, resources, deferred, execution,
    )
}

#[cfg(test)]
pub(crate) fn render_stacked_boxes_with_section(
    boxes: &[RelationGraphBox],
    section_title: RelationGraphLine,
    section_lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    let additional_lines = resources.checked_grid_add(
        usize::from(!boxes.is_empty() && !section_lines.is_empty()),
        resources.checked_grid_add(usize::from(!section_lines.is_empty()), section_lines.len())?,
    )?;
    let base_height = stacked_boxes_height(boxes, resources)?;
    let height = resources.checked_grid_add(base_height, additional_lines)?;
    let width = boxes
        .iter()
        .map(RelationGraphBox::width)
        .chain(std::iter::once(section_title.width()))
        .chain(section_lines.iter().map(RelationGraphLine::width))
        .max()
        .unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain(
                "",
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }

    if !section_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(RelationGraphLine::try_plain(
                "",
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.push(section_title);
        lines.extend(section_lines.iter().map(RelationGraphLine::shared));
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    render_lines_with_options(&lines, options, resources)
}

pub(crate) fn stacked_box_lines(
    boxes: &[RelationGraphBox],
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    stacked_box_lines_ordered(boxes, width_profile, false, resources)
}

pub(crate) fn stacked_box_lines_ordered(
    boxes: &[RelationGraphBox],
    width_profile: TerminalWidthProfile,
    reverse: bool,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    stacked_box_lines_ordered_impl(boxes, width_profile, reverse, resources, None)
}

fn stacked_box_lines_ordered_impl(
    boxes: &[RelationGraphBox],
    width_profile: TerminalWidthProfile,
    reverse: bool,
    resources: &mut ResourceContext,
    execution: Option<AsciiExecution<'_>>,
) -> Result<Vec<RelationGraphLine>> {
    let extent = stacked_box_extent(boxes, resources)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    let ordered = (0..boxes.len()).map(|index| {
        let ordered_index = if reverse {
            boxes.len() - index - 1
        } else {
            index
        };
        &boxes[ordered_index]
    });
    for (index, relation_box) in ordered.enumerate() {
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

pub(crate) fn stacked_box_extent(
    boxes: &[RelationGraphBox],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = stacked_boxes_height(boxes, resources)?;
    let width = boxes.iter().map(RelationGraphBox::width).max().unwrap_or(0);
    resources.grid_extent(width, height)
}

fn stacked_box_ref_lines(
    boxes: &[&RelationGraphBox],
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let extent = stacked_box_ref_extent(boxes, resources)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

fn stacked_box_ref_extent(
    boxes: &[&RelationGraphBox],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = boxes
        .iter()
        .try_fold(boxes.len().saturating_sub(1), |height, relation_box| {
            resources.checked_grid_add(height, relation_box.height())
        })?;
    let width = boxes
        .iter()
        .map(|relation_box| relation_box.width())
        .max()
        .unwrap_or(0);
    resources.grid_extent(width, height)
}

fn stacked_boxes_height(boxes: &[RelationGraphBox], resources: &ResourceContext) -> Result<usize> {
    boxes
        .iter()
        .try_fold(boxes.len().saturating_sub(1), |height, relation_box| {
            resources.checked_grid_add(height, relation_box.height())
        })
}

fn build_layered_edges<'text, R, A>(
    relations: &[R],
    adapter: &A,
    resources: &mut ResourceContext,
    execution: Option<AsciiExecution<'_>>,
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
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
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
    match render_relation_component_lines(boxes, relations, options, resources, adapter, deferred)?
    {
        Some(lines) => render_lines_with_deferred_options(&lines, options, resources, deferred),
        None => Ok(String::new()),
    }
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
    match render_relation_component_lines_with_execution(
        boxes, relations, options, resources, adapter, deferred, execution,
    )? {
        Some(lines) => render_lines_with_deferred_options_with_execution(
            &lines, options, resources, deferred, execution,
        ),
        None => Ok(String::new()),
    }
}

pub(crate) fn render_relation_component_lines<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    render_relation_component_lines_impl(
        boxes, relations, options, resources, adapter, deferred, None,
    )
}

fn render_relation_component_lines_impl<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: Option<AsciiExecution<'_>>,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let edges = build_layered_edges(relations, adapter, resources, execution)?;
    for _ in boxes {
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
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
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
        let has_relations = !component.edge_indices().is_empty();
        let region = plan_relation_component_region(
            component, relations, options, resources, adapter, deferred, execution,
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
    let plan = RelationRenderPlan::try_new(regions, resources)?;
    checkpoint(execution, merman_core::OperationPhase::Emit)?;
    let lines = match execution {
        Some(execution) => {
            let mut emit_resources =
                execution.resource_context(resources, merman_core::OperationPhase::Emit);
            plan.materialize(options, &mut emit_resources)?
        }
        None => plan.materialize(options, resources)?,
    };
    for _ in &lines {
        checkpoint(execution, merman_core::OperationPhase::Emit)?;
    }
    Ok(Some(lines))
}

pub(crate) fn render_relation_component_lines_with_execution<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    let mut resources = execution.resource_context(resources, merman_core::OperationPhase::Layout);
    render_relation_component_lines_impl(
        boxes,
        relations,
        options,
        &mut resources,
        adapter,
        deferred,
        Some(execution),
    )
}

fn plan_relation_component_region<'plan, 'text, R, A>(
    component: RelationGraphComponent<'plan>,
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: Option<AsciiExecution<'_>>,
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
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
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
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
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
            checkpoint(execution, merman_core::OperationPhase::Layout)?;
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

    if selected.len() > 1 && same_directed_endpoints(&selected, adapter, execution)? {
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
        execution,
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
    execution: Option<AsciiExecution<'_>>,
) -> Result<bool>
where
    A: RelationComponentAdapter<'text, R>,
{
    let Some(first) = relations.first() else {
        return Ok(false);
    };
    let first = adapter.build_edges(first);
    for relation in relations.iter().skip(1) {
        checkpoint(execution, merman_core::OperationPhase::Layout)?;
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

/// Admit a base relation block and an optional lossless summary as one logical
/// document before either block allocates its terminal rows.
pub(crate) fn render_relation_document_with_summary(
    base_extent: LogicalExtent,
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    build_base: impl FnOnce(&mut ResourceContext) -> Result<Vec<RelationGraphLine>>,
) -> Result<Vec<RelationGraphLine>> {
    let summary_extent = if rows.is_empty() {
        None
    } else {
        Some(relation_summary_extent(rows, reason, options, resources)?)
    };
    let plan = RelationDocumentPlan::new(
        base_extent,
        summary_extent,
        display_width_with_profile("relations:", options.terminal_width_profile),
        resources,
    )?;
    if rows.is_empty() {
        plan.materialize(resources, build_base)
    } else {
        plan.materialize_with_section(options, resources, build_base, |resources| {
            relation_summary_lines_for_rows(rows, reason, options, resources)
        })
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

fn checkpoint(
    execution: Option<AsciiExecution<'_>>,
    phase: merman_core::OperationPhase,
) -> Result<()> {
    execution.map_or(Ok(()), |execution| execution.checkpoint(phase))
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

pub(crate) fn vertical_center(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    extra_half_widths: &[usize],
) -> usize {
    extra_half_widths
        .iter()
        .copied()
        .fold((top.width / 2).max(bottom.width / 2), usize::max)
}

fn vertical_stack_extent(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_extent: LogicalExtent,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = resources.checked_grid_add(
        resources.checked_grid_add(top.height(), relation_extent.height())?,
        bottom.height(),
    )?;
    let top_left = center
        .checked_sub(top.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let bottom_left = center
        .checked_sub(bottom.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let top_width = resources.checked_grid_add(top_left, top.width())?;
    let bottom_width = resources.checked_grid_add(bottom_left, bottom.width())?;
    resources.grid_extent(
        relation_extent.width().max(top_width).max(bottom_width),
        height,
    )
}

fn assemble_vertical_stack_lines(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
    extent: LogicalExtent,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(try_align_box_lines(top, center, resources)?);
    lines.extend(relation_lines);
    lines.extend(try_align_box_lines(bottom, center, resources)?);
    debug_assert_eq!(lines.len(), extent.height());
    debug_assert_eq!(
        lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0),
        extent.width()
    );
    Ok(lines)
}

#[cfg(test)]
fn render_box(relation_box: &RelationGraphBox) -> String {
    let mut rendered = relation_box
        .lines
        .iter()
        .map(RelationGraphLine::text)
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

#[cfg(test)]
pub(crate) fn render_lines_with_options(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );

    finish_styled_line_iter_with_resources(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
    )
}

pub(crate) fn render_lines_with_deferred_options(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );

    finish_styled_line_iter_with_deferred_resources(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
        deferred,
    )
}

pub(crate) fn render_lines_with_deferred_options_with_execution(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );

    let mut resources = execution.resource_context(resources, merman_core::OperationPhase::Emit);
    finish_styled_line_iter_with_deferred_resources_with_execution(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        &mut resources,
        deferred,
        execution,
    )
}

#[cfg(test)]
pub(crate) fn render_lines_with_deferred_probe(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );

    finish_styled_line_iter_with_deferred_probe(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
        deferred,
        before_materialize,
    )
}

fn try_align_box_lines(
    relation_box: &RelationGraphBox,
    center: usize,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let left_padding = center
        .checked_sub(relation_box.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(relation_box.height())
        .map_err(|_| layout_allocation_failed())?;
    for line in relation_box.lines() {
        lines.push(try_padded_line(line, left_padding, 0, resources)?);
    }
    Ok(lines)
}

fn try_padded_line(
    line: &RelationGraphLine,
    left: usize,
    right: usize,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let mut padded = StyledLine::try_blank_with_resources(left, line.width_profile(), resources)?;
    padded.try_push_line(&line.line)?;
    padded.try_push_spaces(right)?;
    Ok(RelationGraphLine::from_styled(padded))
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
