use crate::canvas::finish_styled_line_iter_with_resources;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
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
pub(crate) use self::layered::*;
pub(crate) use self::model::{
    RelationGraphBox, RelationGraphBoxStyle, RelationGraphLabel, RelationGraphLine,
};
pub(crate) use self::self_loop::{
    RelationSelfLoopMetrics, RelationSelfLoopPlan, RelationSelfLoopRows,
};
pub(crate) use self::stack::{RelationParallelPlan, RelationStackPlan, centered_row_blocks_extent};
pub(crate) use self::summary::*;

pub(crate) trait RelationComponentAdapter<R> {
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
    ) -> Result<RelationRegionPlan<'plan>>;

    fn build_summary_row(
        &self,
        relation: &R,
        reason: LayeredRelationSummaryReason,
        resources: &ResourceContext,
    ) -> Result<RelationGraphSummaryRow>;

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError;
}

#[cfg(test)]
pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn render_stacked_boxes_with_options(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    let lines = stacked_box_lines(boxes, options.terminal_width_profile, resources)?;
    render_lines_with_options(&lines, options, resources)
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

fn build_layered_edges<R, A>(
    relations: &[R],
    adapter: &A,
    resources: &mut ResourceContext,
) -> Result<Vec<LayeredRelationEdge>>
where
    A: RelationComponentAdapter<R>,
{
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    edges.extend(
        relations
            .iter()
            .map(|relation| adapter.build_edges(relation)),
    );
    Ok(edges)
}

pub(crate) fn render_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    match render_relation_component_lines(boxes, relations, options, resources, adapter)? {
        Some(lines) => render_lines_with_options(&lines, options, resources),
        None => Ok(String::new()),
    }
}

pub(crate) fn render_relation_component_lines<'plan, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<R> + 'plan,
{
    let edges = build_layered_edges(relations, adapter, resources)?;
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
        let has_relations = !component.edge_indices().is_empty();
        let region =
            plan_relation_component_region(component, relations, options, resources, adapter)?;
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
    Ok(Some(plan.materialize(options, resources)?))
}

fn plan_relation_component_region<'plan, R, A>(
    component: RelationGraphComponent<'plan>,
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R> + 'plan,
{
    let (component_boxes, edge_indices) = component.into_parts();
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
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

    let has_self = selected
        .iter()
        .any(|relation| adapter.is_self_relation(*relation));
    let has_non_self = selected
        .iter()
        .any(|relation| !adapter.is_self_relation(*relation));
    if has_self && has_non_self {
        return plan_relation_summary_region(
            component_boxes,
            selected,
            LayeredRelationSummaryReason::RouteCollision,
            options,
            resources,
            adapter,
        );
    }

    if has_self {
        let first_edge = adapter.build_edges(selected[0]);
        let same_endpoint = selected.iter().all(|relation| {
            let edge = adapter.build_edges(*relation);
            edge.source_id() == first_edge.source_id() && edge.target_id() == first_edge.target_id()
        });
        if same_endpoint {
            let relation_box = find_box_ref(&component_boxes, first_edge.source_id())
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            return plan_relation_self_loop_region(relation_box, selected, adapter, resources);
        }
    }

    if selected.len() > 1 && same_directed_endpoints(&selected, adapter) {
        return adapter.plan_parallel_region(component_boxes, selected, options, resources);
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
        ),
    }
}

fn same_directed_endpoints<R, A>(relations: &[&R], adapter: &A) -> bool
where
    A: RelationComponentAdapter<R>,
{
    let Some(first) = relations.first() else {
        return false;
    };
    let first = adapter.build_edges(first);
    relations.iter().skip(1).all(|relation| {
        let edge = adapter.build_edges(*relation);
        edge.source_id() == first.source_id() && edge.target_id() == first.target_id()
    })
}

fn plan_relation_self_loop_region<'plan, R, A>(
    relation_box: &'plan RelationGraphBox,
    relations: Vec<&'plan R>,
    adapter: &'plan A,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R> + 'plan,
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

fn plan_relation_summary_region<'plan, R, A>(
    boxes: Vec<&'plan RelationGraphBox>,
    relations: Vec<&'plan R>,
    reason: LayeredRelationSummaryReason,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
    adapter: &A,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R>,
{
    let mut rows = Vec::new();
    rows.try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        rows.push(adapter.build_summary_row(relation, reason, resources)?);
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
    horizontal_gap: usize,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    let mut resources = ResourceContext::new(options.resources);
    let lines = render_layered_relation_component_lines(
        boxes,
        relations,
        options,
        horizontal_gap,
        &mut resources,
        adapter,
    )?;
    render_lines_with_options(&lines, options, &mut resources)
}

#[cfg(test)]
pub(crate) fn render_layered_relation_component_lines<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
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
        Err(reason) => render_relation_summary_component_lines(
            boxes,
            relations,
            options,
            reason,
            resources,
            |relation, resources| adapter.build_summary_row(relation, reason, resources),
        ),
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

/// Render a lossless relation summary for a component whose spatial plan is
/// not safe to materialize. This is shared by family-owned parallel planners so
/// they can reject invalid endpoint ports without duplicating section assembly.
#[cfg(test)]
pub(crate) fn render_relation_summary_component_lines<R>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    reason: LayeredRelationSummaryReason,
    resources: &mut ResourceContext,
    mut build_row: impl FnMut(&R, &ResourceContext) -> Result<RelationGraphSummaryRow>,
) -> Result<Vec<RelationGraphLine>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        rows.push(build_row(relation, resources)?);
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

#[cfg(test)]
fn render_layered_relation_component_result<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<Vec<RelationGraphLine>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
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

#[cfg(test)]
fn plan_layered_relation_component_result<'boxes, R, A>(
    boxes: &'boxes [RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
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

#[derive(Debug)]
enum LayeredRouteBatchError {
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

fn plan_layered_relation_component_ref_result<'boxes, R, A>(
    boxes: &[&'boxes RelationGraphBox],
    relations: &[&R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    let has_self_relation = relations
        .iter()
        .any(|relation| adapter.is_self_relation(*relation));
    if has_self_relation
        && relations
            .iter()
            .any(|relation| !adapter.is_self_relation(*relation))
    {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    edges.extend(
        relations
            .iter()
            .map(|relation| adapter.build_edges(*relation)),
    );
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

fn plan_layered_route_batch<R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    resources: &ResourceContext,
    adapter: &A,
) -> std::result::Result<
    (
        Vec<LayeredRelationRoutePlan>,
        crate::resource::LogicalExtent,
    ),
    LayeredRouteBatchError,
>
where
    A: RelationComponentAdapter<R>,
{
    plan_layered_route_batch_with_probes(scene, relations, resources, adapter, || {}, || {})
}

fn plan_layered_route_batch_with_probes<R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    resources: &ResourceContext,
    adapter: &A,
    before_geometry_collision_scan: impl FnOnce(),
    before_materialized_collision_scan: impl FnOnce(),
) -> std::result::Result<
    (
        Vec<LayeredRelationRoutePlan>,
        crate::resource::LogicalExtent,
    ),
    LayeredRouteBatchError,
>
where
    A: RelationComponentAdapter<R>,
{
    let outcome: Result<
        std::result::Result<
            (
                Vec<LayeredRelationRoutePlan>,
                crate::resource::LogicalExtent,
            ),
            LayeredRelationSummaryReason,
        >,
    > = resources.transaction(|resources| {
        match resources.transaction_preserving_layout_work(|resources| {
            plan_layered_route_batch_in_transaction(
                scene,
                relations,
                resources,
                adapter,
                before_geometry_collision_scan,
                before_materialized_collision_scan,
            )
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

fn plan_layered_route_batch_in_transaction<R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    resources: &ResourceContext,
    adapter: &A,
    before_geometry_collision_scan: impl FnOnce(),
    before_materialized_collision_scan: impl FnOnce(),
) -> std::result::Result<
    (
        Vec<LayeredRelationRoutePlan>,
        crate::resource::LogicalExtent,
    ),
    LayeredRouteBatchError,
>
where
    A: RelationComponentAdapter<R>,
{
    resources.charge_layout_work(scene.draw_order().len().max(1))?;
    let mut planned = Vec::new();
    planned
        .try_reserve_exact(scene.draw_order().len())
        .map_err(|_| layout_allocation_failed())?;
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
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

    validate_layered_route_geometries_with_probe(
        scene,
        &planned,
        resources,
        before_geometry_collision_scan,
    )?;

    let mut route_plans = Vec::new();
    route_plans
        .try_reserve_exact(planned.len())
        .map_err(|_| layout_allocation_failed())?;
    for route in planned {
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
    validate_layered_route_batch_with_probe(
        scene,
        &route_plans,
        resources,
        before_materialized_collision_scan,
    )?;
    let extent = resources.grid_extent(scene.width(), scene.height())?;
    Ok((route_plans, extent))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PairwiseValidationWork {
    segment_count: usize,
    overlay_count: usize,
    pair_work: usize,
}

fn measure_pairwise_validation_work(
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

fn validate_layered_route_geometries_with_probe(
    scene: &LayeredRelationScene<'_>,
    routes: &[PlannedLayeredRoute],
    resources: &ResourceContext,
    before_collision_scan: impl FnOnce(),
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
    before_collision_scan();
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
            if routes[index + 1..]
                .iter()
                .any(|other| route.geometry.overlaps(&other.geometry))
            {
                return Err(LayeredRouteBatchError::Semantic(
                    LayeredRelationSummaryReason::RouteCollision,
                ));
            }
        }
    }
    if routes
        .iter()
        .any(|route| scene.route_geometry_overlaps_box(&route.geometry))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    Ok(())
}

fn validate_layered_route_batch_with_probe(
    scene: &LayeredRelationScene<'_>,
    route_plans: &[LayeredRelationRoutePlan],
    resources: &ResourceContext,
    before_collision_scan: impl FnOnce(),
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
    before_collision_scan();
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
        if route_plans[index + 1..]
            .iter()
            .any(|other| route_plan.overlays_overlap(other))
        {
            return Err(LayeredRouteBatchError::Semantic(
                LayeredRelationSummaryReason::OverlayCollision,
            ));
        }
    }
    if scene.is_planar_k2_2() {
        for (index, route_plan) in route_plans.iter().enumerate() {
            if route_plans[index + 1..]
                .iter()
                .any(|other| route_plan.route_overlaps(other))
            {
                return Err(LayeredRouteBatchError::Semantic(
                    LayeredRelationSummaryReason::RouteCollision,
                ));
            }
            if route_plans[index + 1..].iter().any(|other| {
                route_plan.overlays_overlap_route(other) || other.overlays_overlap_route(route_plan)
            }) {
                return Err(LayeredRouteBatchError::Semantic(
                    LayeredRelationSummaryReason::OverlayCollision,
                ));
            }
        }
    }
    if route_plans
        .iter()
        .any(|route_plan| scene.route_overlaps_box(route_plan))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::RouteCollision,
        ));
    }
    if route_plans
        .iter()
        .any(|route_plan| scene.overlays_overlap_box(route_plan))
    {
        return Err(LayeredRouteBatchError::Semantic(
            LayeredRelationSummaryReason::OverlayCollision,
        ));
    }
    Ok(())
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
