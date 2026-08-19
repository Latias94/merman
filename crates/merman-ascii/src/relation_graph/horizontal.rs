use super::{
    DirectionTransform, LayeredRelationEdge, LayeredRelationError, LayeredRelationSummaryReason,
    PhysicalPortSide, RelationBoxStripPlan, RelationCheckpointCursor, RelationComponentAdapter,
    RelationDirection, RelationExtent, RelationGraphBox, RelationGraphLabel, RelationGraphLine,
    RelationLineChars, RelationPoint, RelationRegionPlan, RelationRenderPlan,
    RelationResourceCheckpointCursor, RelationSelfLoopPlan, RelationSummaryPaintPlan,
    build_layered_edges, grid_overflow, layout_allocation_failed, put_relation_char,
    relation_components, work_overflow,
};
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{LogicalExtent, ResourceContext};
use crate::safe_text::DeferredTextRegistry;
use merman_core::OperationPhase;

mod collision;

use collision::{CompatibleSharedEndpoints, HorizontalEdgeGeometry, VerticalOwnershipSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HorizontalRelationEndpoint {
    marker: Option<HorizontalRelationMarker>,
    label: Option<RelationGraphLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HorizontalRelationMarker {
    text: String,
    width: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
}

impl HorizontalRelationMarker {
    pub(crate) fn new(
        text: impl Into<String>,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let text = text.into();
        let width = crate::text::display_width_with_profile(&text, width_profile);
        Self {
            text,
            width,
            role,
            width_profile,
        }
    }

    fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) -> Result<()> {
        debug_assert_eq!(
            self.width,
            crate::text::display_width_with_profile(&self.text, self.width_profile)
        );
        canvas.write_text_role(x, y, &self.text, self.role)
    }
}

impl HorizontalRelationEndpoint {
    pub(crate) fn new(
        marker: Option<HorizontalRelationMarker>,
        label: Option<RelationGraphLabel>,
    ) -> Self {
        Self { marker, label }
    }

    fn marker_width(&self) -> usize {
        self.marker.as_ref().map(|marker| marker.width).unwrap_or(0)
    }

    fn label_width(&self) -> usize {
        self.label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0)
    }

    fn label_line_count(&self) -> usize {
        self.label
            .as_ref()
            .map(RelationGraphLabel::line_count)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HorizontalRelationStyle {
    source: HorizontalRelationEndpoint,
    target: HorizontalRelationEndpoint,
    label: Option<RelationGraphLabel>,
    horizontal: char,
    vertical: char,
    line_chars: RelationLineChars,
}

impl HorizontalRelationStyle {
    pub(crate) fn new(
        source: HorizontalRelationEndpoint,
        target: HorizontalRelationEndpoint,
        label: Option<RelationGraphLabel>,
        horizontal: char,
        vertical: char,
        line_chars: RelationLineChars,
    ) -> Self {
        Self {
            source,
            target,
            label,
            horizontal,
            vertical,
            line_chars,
        }
    }

    fn label_width(&self) -> usize {
        self.label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0)
    }

    fn label_line_count(&self) -> usize {
        self.label
            .as_ref()
            .map(RelationGraphLabel::line_count)
            .unwrap_or(0)
    }

    fn label_height(&self) -> usize {
        self.source
            .label_line_count()
            .max(self.label_line_count())
            .max(self.target.label_line_count())
    }

    fn required_inner_width(&self, resources: &ResourceContext) -> Result<usize> {
        let marker_width = resources.checked_grid_add(
            resources.checked_grid_add(self.source.marker_width(), self.target.marker_width())?,
            1,
        )?;
        let label_count = usize::from(self.source.label.is_some())
            + usize::from(self.label.is_some())
            + usize::from(self.target.label.is_some());
        let label_width = resources.checked_grid_add(
            resources.checked_grid_add(self.source.label_width(), self.target.label_width())?,
            self.label_width(),
        )?;
        let label_spacing = resources.checked_grid_mul(label_count.saturating_sub(1), 2)?;
        Ok(marker_width
            .max(resources.checked_grid_add(label_width, label_spacing)?)
            .max(1))
    }
}

struct HorizontalNode<'a> {
    original: &'a RelationGraphBox,
    visual: RelationGraphBox,
    x: usize,
}

impl HorizontalNode<'_> {
    fn port_y(&self, box_top: usize, resources: &ResourceContext) -> Result<usize> {
        resources.checked_grid_add(box_top, self.original.height() / 2)
    }
}

struct HorizontalEdgePlan {
    source_index: usize,
    target_index: usize,
    style: HorizontalRelationStyle,
    label_top: usize,
    lane_y: usize,
}

struct HorizontalComponentPlanContext<'plan, 'scratch, R, A> {
    edges: &'scratch [LayeredRelationEdge],
    relations: &'plan [R],
    direction: DirectionTransform,
    options: &'scratch AsciiRenderOptions,
    adapter: &'plan A,
}

pub(crate) struct HorizontalRelationPaintPlan<'a> {
    nodes: Vec<HorizontalNode<'a>>,
    edges: Vec<HorizontalEdgePlan>,
    box_top: usize,
    extent: LogicalExtent,
}

impl HorizontalRelationPaintPlan<'_> {
    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn paint(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let mut checkpoints = RelationResourceCheckpointCursor::new();
        resources.checkpoint()?;
        let mut canvas = Canvas::try_with_controlled_resources(
            self.extent.width(),
            self.extent.height(),
            options.terminal_width_profile,
            resources,
        )?;
        for edge in &self.edges {
            checkpoints.tick(resources)?;
            draw_horizontal_edge(
                &mut canvas,
                &self.nodes,
                edge,
                self.box_top,
                resources,
                &mut checkpoints,
            )?;
        }
        for node in &self.nodes {
            checkpoints.tick(resources)?;
            for (row_index, line) in node.visual.lines().iter().enumerate() {
                checkpoints.tick(resources)?;
                let y = resources.checked_grid_add(self.box_top, row_index)?;
                line.draw_at(&mut canvas, node.x, y)?;
            }
        }

        let styled_lines = canvas.into_styled_lines_preserving_extent()?;
        let mut rendered = Vec::new();
        rendered
            .try_reserve_exact(styled_lines.len())
            .map_err(|_| layout_allocation_failed())?;
        for line in styled_lines {
            checkpoints.tick(resources)?;
            rendered.push(RelationGraphLine::from_styled(line));
        }
        Ok(rendered)
    }
}

impl HorizontalEdgePlan {
    fn physical_endpoints(
        &self,
    ) -> (
        usize,
        &HorizontalRelationEndpoint,
        usize,
        &HorizontalRelationEndpoint,
    ) {
        if self.source_index < self.target_index {
            (
                self.source_index,
                &self.style.source,
                self.target_index,
                &self.style.target,
            )
        } else {
            (
                self.target_index,
                &self.style.target,
                self.source_index,
                &self.style.source,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_horizontal_relation_components_with_execution<'text, R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    direction: DirectionTransform,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let direction = direction.require_horizontal()?;
    let mut layout_checkpoints = RelationCheckpointCursor::new(execution, OperationPhase::Layout);
    let mut layout_resources = execution.resource_context(resources, OperationPhase::Layout);
    layout_checkpoints.checkpoint()?;
    let plan = plan_horizontal_relation_components(
        boxes,
        relations,
        direction,
        options,
        &mut layout_resources,
        adapter,
        deferred,
        &mut layout_checkpoints,
    )?;
    let mut emit_checkpoints = layout_checkpoints.next_phase(OperationPhase::Emit);
    let mut emit_resources = execution.resource_context(resources, OperationPhase::Emit);
    emit_checkpoints.checkpoint()?;
    let lines = plan.materialize(options, &mut emit_resources, &mut emit_checkpoints)?;
    emit_checkpoints.checkpoint()?;
    Ok(lines)
}

#[allow(clippy::too_many_arguments)]
fn plan_horizontal_relation_components<'plan, 'text, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    direction: DirectionTransform,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
    deferred: &mut DeferredTextRegistry<'text>,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<RelationRenderPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R> + 'plan,
{
    if boxes.is_empty() {
        return RelationRenderPlan::try_new(Vec::new(), resources);
    }
    if relations.is_empty() {
        let mut refs = Vec::new();
        refs.try_reserve_exact(boxes.len())
            .map_err(|_| layout_allocation_failed())?;
        refs.extend(boxes);
        let region = RelationRegionPlan::BoxStrip(RelationBoxStripPlan::horizontal(
            refs,
            direction,
            adapter.layered_horizontal_gap(),
            options.terminal_width_profile,
            resources,
        )?);
        checkpoints.before_charge()?;
        return RelationRenderPlan::try_new(vec![region], resources);
    }

    let edges = build_layered_edges(relations, adapter, resources, checkpoints)?;
    for _ in boxes {
        checkpoints.tick()?;
    }
    let components = relation_components(boxes, &edges, resources)
        .map_err(|error| error.into_ascii_error(|semantic| adapter.layered_error(semantic)))?;
    let mut regions = Vec::new();
    regions
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut standalone = Vec::new();
    standalone
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let context = HorizontalComponentPlanContext {
        edges: &edges,
        relations,
        direction,
        options,
        adapter,
    };

    for component in &components {
        checkpoints.tick()?;
        if component.edge_indices().is_empty() {
            standalone.extend(component.boxes().iter().copied());
            continue;
        }
        regions.push(plan_horizontal_component(
            component.boxes(),
            component.edge_indices(),
            &context,
            resources,
            deferred,
            checkpoints,
        )?);
    }

    if !standalone.is_empty() {
        regions.push(RelationRegionPlan::BoxStrip(
            RelationBoxStripPlan::horizontal(
                standalone,
                direction,
                adapter.layered_horizontal_gap(),
                options.terminal_width_profile,
                resources,
            )?,
        ));
    }

    checkpoints.before_charge()?;
    RelationRenderPlan::try_new(regions, resources)
}

pub(crate) fn render_horizontal_box_strip_lines(
    boxes: &[RelationGraphBox],
    direction: DirectionTransform,
    gap: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let direction = direction.require_horizontal()?;
    horizontal_box_strip_extent(boxes, gap, resources)?;
    let mut refs = Vec::new();
    refs.try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    refs.extend(boxes);
    horizontal_box_strip_lines(&refs, direction, gap, width_profile, resources)
}

pub(crate) fn horizontal_box_strip_extent(
    boxes: &[RelationGraphBox],
    gap: usize,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let mut height = 0;
    let mut box_width = 0;
    for relation_box in boxes {
        checkpoints.tick(resources)?;
        height = height.max(relation_box.height());
        box_width = resources.checked_grid_add(box_width, relation_box.width())?;
    }
    let gap_width = resources.checked_grid_mul(boxes.len().saturating_sub(1), gap)?;
    let width = resources.checked_grid_add(box_width, gap_width)?;
    resources.grid_extent(width, height)
}

pub(crate) fn horizontal_box_strip_ref_extent(
    boxes: &[&RelationGraphBox],
    gap: usize,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let mut height = 0;
    let mut box_width = 0;
    for relation_box in boxes {
        checkpoints.tick(resources)?;
        height = height.max(relation_box.height());
        box_width = resources.checked_grid_add(box_width, relation_box.width())?;
    }
    let gap_width = resources.checked_grid_mul(boxes.len().saturating_sub(1), gap)?;
    let width = resources.checked_grid_add(box_width, gap_width)?;
    resources.grid_extent(width, height)
}

fn plan_horizontal_component<'plan, 'text, R, A>(
    boxes: &[&'plan RelationGraphBox],
    edge_indices: &[usize],
    context: &HorizontalComponentPlanContext<'plan, '_, R, A>,
    resources: &mut ResourceContext,
    deferred: &mut DeferredTextRegistry<'text>,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let adapter = context.adapter;
    let relations = context.relations;
    let options = context.options;
    let order = stable_horizontal_order(
        boxes,
        edge_indices,
        context.edges,
        context.direction,
        resources,
        adapter,
        checkpoints,
    )?;
    let mut self_relation_count = 0usize;
    for edge_index in edge_indices {
        checkpoints.tick()?;
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        if adapter.is_self_relation(relation) {
            self_relation_count = self_relation_count
                .checked_add(1)
                .ok_or_else(|| work_overflow(resources))?;
        }
    }
    if self_relation_count > 0 && self_relation_count < edge_indices.len() {
        return Ok(RelationRegionPlan::Summary(
            plan_horizontal_relation_summary(
                boxes,
                &order,
                edge_indices,
                relations,
                options,
                resources,
                adapter,
                deferred,
                checkpoints,
            )?,
        ));
    }
    if self_relation_count == edge_indices.len() {
        let &[box_index] = order.as_slice() else {
            return Err(adapter.layered_error(LayeredRelationError::UnrelatedBoxes));
        };
        let relation_box = boxes
            .get(box_index)
            .copied()
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let mut relation_refs = Vec::new();
        relation_refs
            .try_reserve_exact(edge_indices.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut metrics = Vec::new();
        metrics
            .try_reserve_exact(edge_indices.len())
            .map_err(|_| layout_allocation_failed())?;
        for edge_index in edge_indices {
            checkpoints.tick()?;
            let relation = relations
                .get(*edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            relation_refs.push(relation);
            metrics.push(adapter.self_loop_metrics(relation, resources)?);
        }
        checkpoints.before_charge()?;
        let plan = RelationSelfLoopPlan::try_new(relation_box, metrics, resources)?;
        return Ok(RelationRegionPlan::SelfLoops {
            plan,
            rows: Box::new(move |resources| {
                let mut checkpoints = RelationResourceCheckpointCursor::new();
                let mut loops = Vec::new();
                loops
                    .try_reserve_exact(relation_refs.len())
                    .map_err(|_| layout_allocation_failed())?;
                for relation in relation_refs {
                    checkpoints.tick(resources)?;
                    loops.push(adapter.self_loop_rows(relation, resources)?);
                }
                Ok(loops)
            }),
        });
    }
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for box_index in &order {
        checkpoints.tick()?;
        let original = boxes[*box_index];
        nodes.push(HorizontalNode {
            original,
            visual: original.shared_projection(),
            x: 0,
        });
    }

    let mut edge_plans = Vec::new();
    edge_plans
        .try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
        checkpoints.tick()?;
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let edge = context
            .edges
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let source_index = node_index(&nodes, edge.route_source_id(), checkpoints)?
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let target_index = node_index(&nodes, edge.route_target_id(), checkpoints)?
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let forward_sides = (
            context.direction.map_port(PhysicalPortSide::Bottom),
            context.direction.map_port(PhysicalPortSide::Top),
        );
        let (source_side, target_side) =
            if (source_index < target_index) != context.direction.is_reversed() {
                forward_sides
            } else {
                (forward_sides.1, forward_sides.0)
            };
        let style =
            adapter.horizontal_relation_style(relation, source_side, target_side, resources)?;
        edge_plans.push(HorizontalEdgePlan {
            source_index,
            target_index,
            style,
            label_top: 0,
            lane_y: 0,
        });
    }

    if edge_plans.is_empty() {
        let mut refs = Vec::new();
        refs.try_reserve_exact(nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        refs.extend(nodes.iter().map(|node| node.original));
        return Ok(RelationRegionPlan::BoxStrip(
            RelationBoxStripPlan::horizontal(
                refs,
                RelationDirection::LeftRight.transform(),
                adapter.layered_horizontal_gap(),
                options.terminal_width_profile,
                resources,
            )?,
        ));
    }

    let mut gaps = Vec::new();
    gaps.try_reserve_exact(nodes.len().saturating_sub(1))
        .map_err(|_| layout_allocation_failed())?;
    gaps.resize(
        nodes.len().saturating_sub(1),
        adapter.layered_horizontal_gap(),
    );
    for edge_plan in &edge_plans {
        checkpoints.tick()?;
        let left = edge_plan.source_index.min(edge_plan.target_index);
        let right = edge_plan.source_index.max(edge_plan.target_index);
        let available =
            horizontal_span_between(&nodes, &gaps, left, right, resources, checkpoints)?;
        let required =
            resources.checked_grid_add(edge_plan.style.required_inner_width(resources)?, 2)?;
        if available < required {
            gaps[left] = resources.checked_grid_add(gaps[left], required - available)?;
        }
    }

    let mut width = 0;
    for (index, node) in nodes.iter_mut().enumerate() {
        checkpoints.tick()?;
        node.x = width;
        width = resources.checked_grid_add(width, node.visual.width())?;
        if let Some(gap) = gaps.get(index) {
            width = resources.checked_grid_add(width, *gap)?;
        }
    }

    let mut label_cursor = 0;
    for edge_plan in &mut edge_plans {
        checkpoints.tick()?;
        edge_plan.label_top = label_cursor;
        let label_height = edge_plan.style.label_height();
        label_cursor = resources.checked_grid_add(label_cursor, label_height)?;
        if label_height > 0 {
            label_cursor = resources.checked_grid_add(label_cursor, 1)?;
        }
    }
    let lane_height = resources.checked_grid_mul(edge_plans.len(), 2)?;
    let lane_top = label_cursor;
    for (index, edge_plan) in edge_plans.iter_mut().enumerate() {
        checkpoints.tick()?;
        edge_plan.lane_y =
            resources.checked_grid_add(lane_top, resources.checked_grid_mul(index, 2)?)?;
    }
    let box_top =
        resources.checked_grid_add(resources.checked_grid_add(label_cursor, lane_height)?, 1)?;
    let mut box_height = 0;
    for node in &nodes {
        checkpoints.tick()?;
        box_height = box_height.max(node.visual.height());
    }
    let height = resources.checked_grid_add(box_top, box_height)?;
    if horizontal_edge_owners_overlap(&nodes, &edge_plans, box_top, resources, checkpoints)? {
        return Ok(RelationRegionPlan::Summary(
            plan_horizontal_relation_summary(
                boxes,
                &order,
                edge_indices,
                relations,
                options,
                resources,
                adapter,
                deferred,
                checkpoints,
            )?,
        ));
    }
    let extent = resources.grid_extent(width, height)?;
    Ok(RelationRegionPlan::Horizontal(
        HorizontalRelationPaintPlan {
            nodes,
            edges: edge_plans,
            box_top,
            extent,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn plan_horizontal_relation_summary<'plan, 'text, R, A>(
    boxes: &[&'plan RelationGraphBox],
    order: &[usize],
    edge_indices: &[usize],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
    deferred: &mut DeferredTextRegistry<'text>,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<RelationSummaryPaintPlan<'plan>>
where
    A: RelationComponentAdapter<'text, R>,
{
    let mut ordered_boxes = Vec::new();
    ordered_boxes
        .try_reserve_exact(order.len())
        .map_err(|_| layout_allocation_failed())?;
    for box_index in order {
        checkpoints.tick()?;
        ordered_boxes.push(
            *boxes
                .get(*box_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?,
        );
    }
    let reason = LayeredRelationSummaryReason::RouteCollision;
    let mut rows = Vec::new();
    rows.try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
        checkpoints.tick()?;
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        rows.push(adapter.build_summary_row(relation, reason, resources, deferred)?);
    }
    let gap = adapter.layered_horizontal_gap();
    RelationSummaryPaintPlan::horizontal(
        ordered_boxes,
        RelationDirection::LeftRight.transform(),
        gap,
        rows,
        Some(reason),
        options,
        resources,
    )
}

fn stable_horizontal_order<'text, R, A>(
    boxes: &[&RelationGraphBox],
    edge_indices: &[usize],
    edges: &[LayeredRelationEdge],
    direction: DirectionTransform,
    resources: &mut ResourceContext,
    adapter: &A,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<Vec<usize>>
where
    A: RelationComponentAdapter<'text, R>,
{
    checkpoints.before_charge()?;
    resources.charge_layout_work_product(boxes.len().max(1), edge_indices.len().max(1))?;
    let mut indegree = Vec::new();
    indegree
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    indegree.resize(boxes.len(), 0usize);
    for edge_index in edge_indices {
        checkpoints.tick()?;
        let edge = edges
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        if edge.route_source_id() == edge.route_target_id() {
            continue;
        }
        let target = box_index(boxes, edge.route_target_id(), checkpoints)?
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        indegree[target] = indegree[target]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
    }

    let mut emitted = Vec::new();
    emitted
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    emitted.resize(boxes.len(), false);
    let mut order = Vec::new();
    order
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    while order.len() < boxes.len() {
        let mut next = None;
        for (index, degree) in indegree.iter().enumerate() {
            checkpoints.tick()?;
            if !emitted[index] && *degree == 0 {
                next = Some(index);
                break;
            }
        }
        if next.is_none() {
            for (index, was_emitted) in emitted.iter().enumerate() {
                checkpoints.tick()?;
                if !was_emitted {
                    next = Some(index);
                    break;
                }
            }
        }
        let next =
            next.ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        emitted[next] = true;
        order.push(next);
        let source_id = boxes[next].id();
        for edge_index in edge_indices {
            checkpoints.tick()?;
            let edge = edges
                .get(*edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            if edge.route_source_id() != source_id
                || edge.route_source_id() == edge.route_target_id()
            {
                continue;
            }
            let target = box_index(boxes, edge.route_target_id(), checkpoints)?
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            indegree[target] = indegree[target].saturating_sub(1);
        }
    }
    if direction.is_reversed() {
        order.reverse();
    }
    Ok(order)
}

fn horizontal_span_between(
    nodes: &[HorizontalNode<'_>],
    gaps: &[usize],
    left: usize,
    right: usize,
    resources: &ResourceContext,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<usize> {
    if left >= right {
        return Err(grid_overflow(resources));
    }
    let mut span = *gaps.get(left).ok_or_else(|| grid_overflow(resources))?;
    for (node, gap) in nodes.iter().zip(gaps).take(right).skip(left + 1) {
        checkpoints.tick()?;
        span = resources.checked_grid_add(span, node.visual.width())?;
        span = resources.checked_grid_add(span, *gap)?;
    }
    Ok(span)
}

fn horizontal_edge_geometry(
    nodes: &[HorizontalNode<'_>],
    edge: &HorizontalEdgePlan,
    box_top: usize,
    resources: &ResourceContext,
) -> Result<HorizontalEdgeGeometry> {
    let (left_index, _, right_index, _) = edge.physical_endpoints();
    let left = nodes
        .get(left_index)
        .ok_or_else(|| grid_overflow(resources))?;
    let right = nodes
        .get(right_index)
        .ok_or_else(|| grid_overflow(resources))?;
    let left_x = resources.checked_grid_add(left.x, left.original.width())?;
    let right_x = right
        .x
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    let left_port_y = left.port_y(box_top, resources)?;
    let right_port_y = right.port_y(box_top, resources)?;
    Ok(HorizontalEdgeGeometry {
        lane_y: edge.lane_y,
        left_x,
        right_x,
        left_port_y,
        right_port_y,
        left_stem: VerticalOwnershipSpan {
            node_index: left_index,
            x: left_x,
            top: edge.lane_y.min(left_port_y),
            bottom: edge.lane_y.max(left_port_y),
        },
        right_stem: VerticalOwnershipSpan {
            node_index: right_index,
            x: right_x,
            top: edge.lane_y.min(right_port_y),
            bottom: edge.lane_y.max(right_port_y),
        },
    })
}

fn horizontal_edge_endpoint(
    edge: &HorizontalEdgePlan,
    node_index: usize,
) -> Option<&HorizontalRelationEndpoint> {
    if edge.source_index == node_index {
        Some(&edge.style.source)
    } else if edge.target_index == node_index {
        Some(&edge.style.target)
    } else {
        None
    }
}

fn compatible_shared_endpoints(
    left: &HorizontalEdgePlan,
    right: &HorizontalEdgePlan,
) -> CompatibleSharedEndpoints {
    let mut compatible = CompatibleSharedEndpoints::default();
    for node_index in [left.source_index, left.target_index] {
        let Some(left_endpoint) = horizontal_edge_endpoint(left, node_index) else {
            continue;
        };
        let Some(right_endpoint) = horizontal_edge_endpoint(right, node_index) else {
            continue;
        };
        let same_endpoint_paint = left_endpoint.marker == right_endpoint.marker
            && left.style.horizontal == right.style.horizontal
            && left.style.vertical == right.style.vertical
            && left.style.line_chars == right.style.line_chars;
        if same_endpoint_paint {
            compatible = compatible.add(node_index);
        }
    }
    compatible
}

fn horizontal_edge_owners_overlap(
    nodes: &[HorizontalNode<'_>],
    edges: &[HorizontalEdgePlan],
    box_top: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<bool> {
    checkpoints.before_charge()?;
    resources.charge_layout_work(edges.len())?;
    let mut geometries = Vec::new();
    geometries
        .try_reserve_exact(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        checkpoints.tick()?;
        geometries.push(horizontal_edge_geometry(nodes, edge, box_top, resources)?);
    }

    let pair_count = edges
        .len()
        .checked_mul(edges.len().saturating_sub(1))
        .map(|pairs| pairs / 2)
        .ok_or_else(|| work_overflow(resources))?;
    checkpoints.before_charge()?;
    resources.charge_layout_work(pair_count)?;
    for left_index in 0..edges.len() {
        for right_index in (left_index + 1)..edges.len() {
            checkpoints.tick()?;
            let compatible_shared_endpoints =
                compatible_shared_endpoints(&edges[left_index], &edges[right_index]);
            if geometries[left_index]
                .has_owner_collision_with(geometries[right_index], compatible_shared_endpoints)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn draw_horizontal_edge(
    canvas: &mut Canvas,
    nodes: &[HorizontalNode<'_>],
    edge: &HorizontalEdgePlan,
    box_top: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<()> {
    let (_, left_endpoint, _, right_endpoint) = edge.physical_endpoints();
    let geometry = horizontal_edge_geometry(nodes, edge, box_top, resources)?;
    let left_stem_x = geometry.left_x;
    let right_stem_x = geometry.right_x;
    let vertical_work = geometry
        .left_stem
        .bottom
        .abs_diff(geometry.left_stem.top)
        .checked_add(geometry.right_stem.bottom.abs_diff(geometry.right_stem.top))
        .ok_or_else(|| work_overflow(resources))?;
    let horizontal_work = right_stem_x.abs_diff(left_stem_x);
    resources.checkpoint()?;
    resources.charge_layout_work(
        vertical_work
            .checked_add(horizontal_work)
            .and_then(|work| work.checked_add(2))
            .ok_or_else(|| work_overflow(resources))?,
    )?;

    let mut paint = HorizontalPaintControl {
        resources,
        checkpoints,
    };

    draw_vertical_span(
        canvas,
        left_stem_x,
        edge.lane_y,
        geometry.left_port_y,
        edge.style.vertical,
        edge.style.line_chars,
        &mut paint,
    )?;
    draw_vertical_span(
        canvas,
        right_stem_x,
        edge.lane_y,
        geometry.right_port_y,
        edge.style.vertical,
        edge.style.line_chars,
        &mut paint,
    )?;

    let content_start = paint.resources.checked_grid_add(left_stem_x, 1)?;
    let content_end = right_stem_x
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(paint.resources))?;
    let left_marker_end = paint
        .resources
        .checked_grid_add(content_start, left_endpoint.marker_width())?;
    let right_marker_start = paint
        .resources
        .checked_grid_add(content_end, 1)?
        .checked_sub(right_endpoint.marker_width())
        .ok_or_else(|| grid_overflow(paint.resources))?;
    if left_marker_end >= right_marker_start {
        return Err(grid_overflow(paint.resources));
    }
    for x in left_marker_end..right_marker_start {
        paint.tick()?;
        put_relation_char(
            canvas,
            x,
            edge.lane_y,
            edge.style.horizontal,
            edge.style.line_chars,
        )?;
    }
    if let Some(marker) = left_endpoint.marker.as_ref() {
        marker.draw_at(canvas, content_start, edge.lane_y)?;
    }
    if let Some(marker) = right_endpoint.marker.as_ref() {
        marker.draw_at(canvas, right_marker_start, edge.lane_y)?;
    }

    draw_horizontal_labels(
        canvas,
        edge,
        left_endpoint,
        right_endpoint,
        content_start,
        paint.resources.checked_grid_add(content_end, 1)?,
        &mut paint,
    )
}

struct HorizontalPaintControl<'a> {
    resources: &'a ResourceContext,
    checkpoints: &'a mut RelationResourceCheckpointCursor,
}

impl HorizontalPaintControl<'_> {
    fn tick(&mut self) -> Result<()> {
        self.checkpoints.tick(self.resources)
    }
}

fn draw_horizontal_labels(
    canvas: &mut Canvas,
    edge: &HorizontalEdgePlan,
    left_endpoint: &HorizontalRelationEndpoint,
    right_endpoint: &HorizontalRelationEndpoint,
    content_start: usize,
    content_end: usize,
    paint: &mut HorizontalPaintControl<'_>,
) -> Result<()> {
    let left_width = left_endpoint.label_width();
    let relation_width = edge.style.label_width();
    let right_width = right_endpoint.label_width();
    let right_start = content_end
        .checked_sub(right_width)
        .ok_or_else(|| grid_overflow(paint.resources))?;
    let ideal_relation_start = content_start
        .checked_add(content_end)
        .and_then(|sum| sum.checked_sub(relation_width))
        .map(|remaining| remaining / 2)
        .ok_or_else(|| grid_overflow(paint.resources))?;
    let minimum_relation_start = paint.resources.checked_grid_add(
        content_start,
        paint
            .resources
            .checked_grid_add(left_width, usize::from(left_width > 0) * 2)?,
    )?;
    let maximum_relation_start = right_start
        .checked_sub(
            paint
                .resources
                .checked_grid_add(relation_width, usize::from(right_width > 0) * 2)?,
        )
        .unwrap_or(minimum_relation_start);
    let relation_start = ideal_relation_start
        .max(minimum_relation_start)
        .min(maximum_relation_start.max(minimum_relation_start));

    if let Some(label) = left_endpoint.label.as_ref() {
        draw_label_at(canvas, content_start, edge.label_top, label, paint)?;
    }
    if let Some(label) = edge.style.label.as_ref() {
        draw_label_at(canvas, relation_start, edge.label_top, label, paint)?;
    }
    if let Some(label) = right_endpoint.label.as_ref() {
        draw_label_at(canvas, right_start, edge.label_top, label, paint)?;
    }
    Ok(())
}

fn draw_label_at(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &RelationGraphLabel,
    paint: &mut HorizontalPaintControl<'_>,
) -> Result<()> {
    for (offset, line) in label.lines().iter().enumerate() {
        paint.tick()?;
        let row = paint.resources.checked_grid_add(y, offset)?;
        canvas.write_deferred_text_role(x, row, line, AsciiColorRole::EdgeLabel)?;
    }
    Ok(())
}

fn draw_vertical_span(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
    paint: &mut HorizontalPaintControl<'_>,
) -> Result<()> {
    for y in start_y.min(end_y)..=start_y.max(end_y) {
        paint.tick()?;
        put_relation_char(canvas, x, y, ch, chars)?;
    }
    Ok(())
}

pub(crate) fn horizontal_box_strip_lines(
    boxes: &[&RelationGraphBox],
    direction: DirectionTransform,
    gap: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let direction = direction.require_horizontal()?;
    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let extent = horizontal_box_strip_ref_extent(boxes, gap, resources)?;
    let height = extent.height();
    let canonical_order_extent = RelationExtent::new(usize::from(!boxes.is_empty()), boxes.len());
    let physical_order_extent = direction.map_extent(canonical_order_extent);
    debug_assert_eq!(
        physical_order_extent.height(),
        usize::from(!boxes.is_empty())
    );
    resources.checkpoint()?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for y in 0..height {
        checkpoints.tick(resources)?;
        let mut parts = Vec::new();
        let part_capacity = resources
            .checked_work_mul(boxes.len(), 2)?
            .saturating_sub(1);
        parts
            .try_reserve_exact(part_capacity)
            .map_err(|_| layout_allocation_failed())?;
        for ordered_index in 0..physical_order_extent.width() {
            checkpoints.tick(resources)?;
            let physical_point = direction
                .map_point(RelationPoint::new(0, ordered_index), canonical_order_extent)
                .ok_or_else(|| grid_overflow(resources))?;
            debug_assert_eq!(physical_point.y(), 0);
            let box_index = physical_point.x();
            let relation_box = boxes[box_index];
            if ordered_index > 0 {
                parts.push(RelationGraphLine::try_blank(gap, width_profile, resources)?);
            }
            let top_padding = height.saturating_sub(relation_box.height()) / 2;
            let line = match y.checked_sub(top_padding) {
                Some(row) if row < relation_box.height() => relation_box.lines()[row].clone(),
                _ => RelationGraphLine::try_blank(relation_box.width(), width_profile, resources)?,
            };
            parts.push(line);
        }
        lines.push(super::try_concat_relation_lines(
            parts,
            width_profile,
            resources,
        )?);
    }
    Ok(lines)
}

fn box_index(
    boxes: &[&RelationGraphBox],
    id: &str,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<Option<usize>> {
    for (index, relation_box) in boxes.iter().enumerate() {
        checkpoints.tick()?;
        if relation_box.id() == id {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn node_index(
    nodes: &[HorizontalNode<'_>],
    id: &str,
    checkpoints: &mut RelationCheckpointCursor<'_>,
) -> Result<Option<usize>> {
    for (index, node) in nodes.iter().enumerate() {
        checkpoints.tick()?;
        if node.original.id() == id {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl};

    #[test]
    fn horizontal_owner_scan_has_an_exact_work_boundary() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let nodes = vec![
            HorizontalNode {
                original: &boxes[0],
                visual: boxes[0].shared_projection(),
                x: 0,
            },
            HorizontalNode {
                original: &boxes[1],
                visual: boxes[1].shared_projection(),
                x: 10,
            },
            HorizontalNode {
                original: &boxes[2],
                visual: boxes[2].shared_projection(),
                x: 20,
            },
        ];
        let style = HorizontalRelationStyle::new(
            HorizontalRelationEndpoint::new(None, None),
            HorizontalRelationEndpoint::new(None, None),
            None,
            '-',
            '|',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
        );
        let edges = vec![
            HorizontalEdgePlan {
                source_index: 0,
                target_index: 1,
                style: style.clone(),
                label_top: 0,
                lane_y: 2,
            },
            HorizontalEdgePlan {
                source_index: 0,
                target_index: 2,
                style,
                label_top: 0,
                lane_y: 4,
            },
        ];

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let measured_ledger = ResourceContext::new(unbounded);
        let measured_execution = AsciiExecution::for_test(&unbounded);
        let mut measured_resources =
            measured_execution.resource_context(&measured_ledger, OperationPhase::Layout);
        let mut measured_checkpoints =
            RelationCheckpointCursor::new(measured_execution, OperationPhase::Layout);
        assert!(
            horizontal_edge_owners_overlap(
                &nodes,
                &edges,
                8,
                &mut measured_resources,
                &mut measured_checkpoints,
            )
            .expect("shared-source owner scan should succeed")
        );
        let exact_work = measured_resources.layout_work_used();
        assert_eq!(exact_work, 3);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact owner-scan work limit should be valid");
        let exact_ledger = ResourceContext::new(exact_policy);
        let exact_execution = AsciiExecution::for_test(&exact_policy);
        let mut exact_resources =
            exact_execution.resource_context(&exact_ledger, OperationPhase::Layout);
        let mut exact_checkpoints =
            RelationCheckpointCursor::new(exact_execution, OperationPhase::Layout);
        assert!(
            horizontal_edge_owners_overlap(
                &nodes,
                &edges,
                8,
                &mut exact_resources,
                &mut exact_checkpoints,
            )
            .expect("exact owner-scan work should pass")
        );
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one owner-scan work limit should be valid");
        let below_ledger = ResourceContext::new(below_policy);
        let below_execution = AsciiExecution::for_test(&below_policy);
        let mut below_resources =
            below_execution.resource_context(&below_ledger, OperationPhase::Layout);
        let mut below_checkpoints =
            RelationCheckpointCursor::new(below_execution, OperationPhase::Layout);
        let error = horizontal_edge_owners_overlap(
            &nodes,
            &edges,
            8,
            &mut below_resources,
            &mut below_checkpoints,
        )
        .expect_err("max-minus-one owner-scan work should reject");
        assert!(matches!(
            error,
            crate::error::AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == exact_work
                    && details.max == exact_work - 1
        ));
    }

    #[test]
    fn horizontal_owner_scan_observes_cancellation_before_work_admission() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let nodes = vec![
            HorizontalNode {
                original: &boxes[0],
                visual: boxes[0].shared_projection(),
                x: 0,
            },
            HorizontalNode {
                original: &boxes[1],
                visual: boxes[1].shared_projection(),
                x: 10,
            },
        ];
        let style = HorizontalRelationStyle::new(
            HorizontalRelationEndpoint::new(None, None),
            HorizontalRelationEndpoint::new(None, None),
            None,
            '-',
            '|',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
        );
        let edges = vec![
            HorizontalEdgePlan {
                source_index: 0,
                target_index: 1,
                style: style.clone(),
                label_top: 0,
                lane_y: 2,
            },
            HorizontalEdgePlan {
                source_index: 0,
                target_index: 1,
                style,
                label_top: 0,
                lane_y: 4,
            },
        ];
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("single work-unit policy should be valid");
        let control = OperationControl::new();
        control.cancel();
        let ledger = ResourceContext::new(policy);
        let execution = AsciiExecution::new(&control, &policy);
        let mut resources = execution.resource_context(&ledger, OperationPhase::Layout);
        let mut checkpoints = RelationCheckpointCursor::new(execution, OperationPhase::Layout);

        let error =
            horizontal_edge_owners_overlap(&nodes, &edges, 8, &mut resources, &mut checkpoints)
                .expect_err("cancellation should win over work admission");

        assert!(matches!(
            error,
            crate::error::AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn shared_endpoint_compatibility_ignores_remote_marker_differences() {
        let left = test_edge_plan(0, 1, "shared", "left-remote", 2);
        let mut right = test_edge_plan(0, 2, "shared", "right-remote", 4);

        assert_eq!(
            compatible_shared_endpoints(&left, &right),
            CompatibleSharedEndpoints::single(0)
        );

        right.style.source.marker = test_marker("different-shared-marker");
        assert_eq!(
            compatible_shared_endpoints(&left, &right),
            CompatibleSharedEndpoints::default(),
            "a marker difference at the shared endpoint must remain a conservative collision"
        );
    }

    #[test]
    fn shared_endpoint_compatibility_uses_the_physical_node_across_relation_direction() {
        let incoming = test_edge_plan(0, 1, "incoming-remote", "shared", 2);
        let outgoing = test_edge_plan(1, 2, "shared", "outgoing-remote", 4);

        assert_eq!(
            compatible_shared_endpoints(&incoming, &outgoing),
            CompatibleSharedEndpoints::single(1),
            "the shared physical node must match even when it is target on one edge and source on the other"
        );
    }

    fn test_edge_plan(
        source_index: usize,
        target_index: usize,
        source_marker: &str,
        target_marker: &str,
        lane_y: usize,
    ) -> HorizontalEdgePlan {
        HorizontalEdgePlan {
            source_index,
            target_index,
            style: HorizontalRelationStyle::new(
                HorizontalRelationEndpoint::new(test_marker(source_marker), None),
                HorizontalRelationEndpoint::new(test_marker(target_marker), None),
                None,
                '-',
                '|',
                RelationLineChars::new(['-', '|', '.', ':'], '+'),
            ),
            label_top: 0,
            lane_y,
        }
    }

    fn test_marker(text: &str) -> Option<HorizontalRelationMarker> {
        Some(HorizontalRelationMarker::new(
            text,
            AsciiColorRole::EdgeArrow,
            TerminalWidthProfile::Unicode,
        ))
    }
}
