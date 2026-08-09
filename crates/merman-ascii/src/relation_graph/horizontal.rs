use super::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationSummaryReason,
    RelationComponentAdapter, RelationGraphBox, RelationGraphLabel, RelationGraphLine,
    RelationLineChars, build_layered_edges, grid_overflow, join_component_line_groups,
    layout_allocation_failed, put_relation_char, relation_components, render_relation_self_loops,
    try_share_relation_box_lines, work_overflow,
};
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{LogicalExtent, ResourceContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationGraphHorizontalDirection {
    LeftRight,
    RightLeft,
}

impl RelationGraphHorizontalDirection {
    fn is_reversed(self) -> bool {
        matches!(self, Self::RightLeft)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationPortSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HorizontalRelationEndpoint {
    marker: Option<RelationGraphLine>,
    label: Option<RelationGraphLabel>,
}

impl HorizontalRelationEndpoint {
    pub(crate) fn new(
        marker: Option<RelationGraphLine>,
        label: Option<RelationGraphLabel>,
    ) -> Self {
        Self { marker, label }
    }

    fn marker_width(&self) -> usize {
        self.marker
            .as_ref()
            .map(RelationGraphLine::width)
            .unwrap_or(0)
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

pub(crate) fn render_horizontal_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    direction: RelationGraphHorizontalDirection,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    if boxes.is_empty() {
        return Ok(Vec::new());
    }
    if relations.is_empty() {
        let mut refs = Vec::new();
        refs.try_reserve_exact(boxes.len())
            .map_err(|_| layout_allocation_failed())?;
        refs.extend(boxes);
        return horizontal_box_strip_lines(
            &refs,
            direction,
            adapter.layered_horizontal_gap(),
            options.terminal_width_profile,
            resources,
        );
    }

    let edges = build_layered_edges(relations, adapter, resources)?;
    let components = relation_components(boxes, &edges, resources)
        .map_err(|error| error.into_ascii_error(|semantic| adapter.layered_error(semantic)))?;
    let mut rendered_groups = Vec::new();
    rendered_groups
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut standalone = Vec::new();
    standalone
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;

    for component in &components {
        if component.edge_indices().is_empty() {
            standalone.extend(component.boxes().iter().copied());
            continue;
        }
        rendered_groups.push(render_horizontal_component(
            component.boxes(),
            component.edge_indices(),
            &edges,
            relations,
            direction,
            options,
            resources,
            adapter,
        )?);
    }

    if !standalone.is_empty() {
        rendered_groups.push(horizontal_box_strip_lines(
            &standalone,
            direction,
            adapter.layered_horizontal_gap(),
            options.terminal_width_profile,
            resources,
        )?);
    }

    join_component_line_groups(rendered_groups, options.terminal_width_profile, resources)
}

pub(crate) fn render_horizontal_box_strip_lines(
    boxes: &[RelationGraphBox],
    direction: RelationGraphHorizontalDirection,
    gap: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
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
    let height = boxes
        .iter()
        .map(RelationGraphBox::height)
        .max()
        .unwrap_or(0);
    let box_width = boxes.iter().try_fold(0usize, |width, relation_box| {
        resources.checked_grid_add(width, relation_box.width())
    })?;
    let gap_width = resources.checked_grid_mul(boxes.len().saturating_sub(1), gap)?;
    let width = resources.checked_grid_add(box_width, gap_width)?;
    resources.grid_extent(width, height)
}

fn horizontal_box_strip_ref_extent(
    boxes: &[&RelationGraphBox],
    gap: usize,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = boxes
        .iter()
        .map(|relation_box| relation_box.height())
        .max()
        .unwrap_or(0);
    let box_width = boxes.iter().try_fold(0usize, |width, relation_box| {
        resources.checked_grid_add(width, relation_box.width())
    })?;
    let gap_width = resources.checked_grid_mul(boxes.len().saturating_sub(1), gap)?;
    let width = resources.checked_grid_add(box_width, gap_width)?;
    resources.grid_extent(width, height)
}

fn render_horizontal_component<R, A>(
    boxes: &[&RelationGraphBox],
    edge_indices: &[usize],
    edges: &[LayeredRelationEdge],
    relations: &[R],
    direction: RelationGraphHorizontalDirection,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    let order = stable_horizontal_order(boxes, edge_indices, edges, direction, resources, adapter)?;
    let self_relation_count = edge_indices.iter().try_fold(0usize, |count, edge_index| {
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        if adapter.is_self_relation(relation) {
            count.checked_add(1).ok_or_else(|| work_overflow(resources))
        } else {
            Ok(count)
        }
    })?;
    if self_relation_count > 0 && self_relation_count < edge_indices.len() {
        return render_horizontal_relation_summary(
            boxes,
            &order,
            edge_indices,
            relations,
            options,
            resources,
            adapter,
        );
    }
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for box_index in order {
        let original = boxes[box_index];
        let mut self_relations = Vec::new();
        self_relations
            .try_reserve_exact(edge_indices.len())
            .map_err(|_| layout_allocation_failed())?;
        for edge_index in edge_indices {
            let relation = relations
                .get(*edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            let edge = edges
                .get(*edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            if adapter.is_self_relation(relation) && edge.source_id() == original.id() {
                self_relations.push(relation);
            }
        }

        let visual = if self_relations.is_empty() {
            original.shared_projection()
        } else {
            let lines = render_relation_self_loops(
                original,
                self_relations.iter().copied(),
                self_relations.len(),
                adapter,
                resources,
            )?;
            RelationGraphBox::from_rendered_lines(
                original.id().to_string(),
                lines,
                options.terminal_width_profile,
                resources,
            )?
        };
        nodes.push(HorizontalNode {
            original,
            visual,
            x: 0,
        });
    }

    let mut edge_plans = Vec::new();
    edge_plans
        .try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        if adapter.is_self_relation(relation) {
            continue;
        }
        let edge = edges
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let source_index = node_index(&nodes, edge.source_id())
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let target_index = node_index(&nodes, edge.target_id())
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        let (source_side, target_side) = if source_index < target_index {
            (RelationPortSide::Right, RelationPortSide::Left)
        } else {
            (RelationPortSide::Left, RelationPortSide::Right)
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
        if nodes.len() == 1 {
            return try_share_relation_box_lines(&nodes[0].visual);
        }
        let mut refs = Vec::new();
        refs.try_reserve_exact(nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        refs.extend(nodes.iter().map(|node| &node.visual));
        return horizontal_box_strip_lines(
            &refs,
            RelationGraphHorizontalDirection::LeftRight,
            adapter.layered_horizontal_gap(),
            options.terminal_width_profile,
            resources,
        );
    }

    let mut gaps = Vec::new();
    gaps.try_reserve_exact(nodes.len().saturating_sub(1))
        .map_err(|_| layout_allocation_failed())?;
    gaps.resize(
        nodes.len().saturating_sub(1),
        adapter.layered_horizontal_gap(),
    );
    for edge_plan in &edge_plans {
        let left = edge_plan.source_index.min(edge_plan.target_index);
        let right = edge_plan.source_index.max(edge_plan.target_index);
        let available = horizontal_span_between(&nodes, &gaps, left, right, resources)?;
        let required =
            resources.checked_grid_add(edge_plan.style.required_inner_width(resources)?, 2)?;
        if available < required {
            gaps[left] = resources.checked_grid_add(gaps[left], required - available)?;
        }
    }

    let mut width = 0;
    for (index, node) in nodes.iter_mut().enumerate() {
        node.x = width;
        width = resources.checked_grid_add(width, node.visual.width())?;
        if let Some(gap) = gaps.get(index) {
            width = resources.checked_grid_add(width, *gap)?;
        }
    }

    let mut label_cursor = 0;
    for edge_plan in &mut edge_plans {
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
        edge_plan.lane_y =
            resources.checked_grid_add(lane_top, resources.checked_grid_mul(index, 2)?)?;
    }
    let box_top =
        resources.checked_grid_add(resources.checked_grid_add(label_cursor, lane_height)?, 1)?;
    let box_height = nodes
        .iter()
        .map(|node| node.visual.height())
        .max()
        .unwrap_or(0);
    let height = resources.checked_grid_add(box_top, box_height)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;

    let mut canvas =
        Canvas::try_with_resources(width, height, options.terminal_width_profile, resources)?;
    for edge_plan in &edge_plans {
        draw_horizontal_edge(&mut canvas, &nodes, edge_plan, box_top, resources)?;
    }
    for node in &nodes {
        node.visual
            .draw_at(&mut canvas, node.x, box_top, resources)?;
    }

    let styled_lines = canvas.into_styled_lines_trimmed()?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(styled_lines.len())
        .map_err(|_| layout_allocation_failed())?;
    rendered.extend(styled_lines.into_iter().map(RelationGraphLine::from_styled));
    Ok(rendered)
}

#[allow(clippy::too_many_arguments)]
fn render_horizontal_relation_summary<R, A>(
    boxes: &[&RelationGraphBox],
    order: &[usize],
    edge_indices: &[usize],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    let mut ordered_boxes = Vec::new();
    ordered_boxes
        .try_reserve_exact(order.len())
        .map_err(|_| layout_allocation_failed())?;
    for box_index in order {
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
        let relation = relations
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        rows.push(adapter.build_summary_row(relation, reason)?);
    }
    let gap = adapter.layered_horizontal_gap();
    let base_extent = horizontal_box_strip_ref_extent(&ordered_boxes, gap, resources)?;
    super::render_relation_document_with_summary(
        base_extent,
        &rows,
        Some(reason),
        options,
        resources,
        |resources| {
            horizontal_box_strip_lines(
                &ordered_boxes,
                RelationGraphHorizontalDirection::LeftRight,
                gap,
                options.terminal_width_profile,
                resources,
            )
        },
    )
}

fn stable_horizontal_order<R, A>(
    boxes: &[&RelationGraphBox],
    edge_indices: &[usize],
    edges: &[LayeredRelationEdge],
    direction: RelationGraphHorizontalDirection,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<usize>>
where
    A: RelationComponentAdapter<R>,
{
    resources.charge_layout_work_product(boxes.len().max(1), edge_indices.len().max(1))?;
    let mut indegree = Vec::new();
    indegree
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    indegree.resize(boxes.len(), 0usize);
    for edge_index in edge_indices {
        let edge = edges
            .get(*edge_index)
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        if edge.source_id() == edge.target_id() {
            continue;
        }
        let target = box_index(boxes, edge.target_id())
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
        let next = indegree
            .iter()
            .enumerate()
            .find_map(|(index, degree)| (!emitted[index] && *degree == 0).then_some(index))
            .or_else(|| emitted.iter().position(|was_emitted| !was_emitted))
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        emitted[next] = true;
        order.push(next);
        let source_id = boxes[next].id();
        for edge_index in edge_indices {
            let edge = edges
                .get(*edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            if edge.source_id() != source_id || edge.source_id() == edge.target_id() {
                continue;
            }
            let target = box_index(boxes, edge.target_id())
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
) -> Result<usize> {
    if left >= right {
        return Err(grid_overflow(resources));
    }
    let mut span = *gaps.get(left).ok_or_else(|| grid_overflow(resources))?;
    for index in left + 1..right {
        span = resources.checked_grid_add(span, nodes[index].visual.width())?;
        span = resources.checked_grid_add(
            span,
            *gaps.get(index).ok_or_else(|| grid_overflow(resources))?,
        )?;
    }
    Ok(span)
}

fn draw_horizontal_edge(
    canvas: &mut Canvas,
    nodes: &[HorizontalNode<'_>],
    edge: &HorizontalEdgePlan,
    box_top: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    let (left_index, left_endpoint, right_index, right_endpoint) = edge.physical_endpoints();
    let left = &nodes[left_index];
    let right = &nodes[right_index];
    // A self-loop expands only the visual projection. Ordinary edges must still
    // attach to the original node face, not to the loop's outer gutter.
    let left_stem_x = resources.checked_grid_add(left.x, left.original.width())?;
    let right_stem_x = right
        .x
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    let left_port_y = left.port_y(box_top, resources)?;
    let right_port_y = right.port_y(box_top, resources)?;
    let vertical_work = edge
        .lane_y
        .abs_diff(left_port_y)
        .checked_add(edge.lane_y.abs_diff(right_port_y))
        .ok_or_else(|| work_overflow(resources))?;
    let horizontal_work = right_stem_x.abs_diff(left_stem_x);
    resources.charge_layout_work(
        vertical_work
            .checked_add(horizontal_work)
            .and_then(|work| work.checked_add(2))
            .ok_or_else(|| work_overflow(resources))?,
    )?;

    draw_vertical_span(
        canvas,
        left_stem_x,
        edge.lane_y,
        left_port_y,
        edge.style.vertical,
        edge.style.line_chars,
    )?;
    draw_vertical_span(
        canvas,
        right_stem_x,
        edge.lane_y,
        right_port_y,
        edge.style.vertical,
        edge.style.line_chars,
    )?;

    let content_start = resources.checked_grid_add(left_stem_x, 1)?;
    let content_end = right_stem_x
        .checked_sub(1)
        .ok_or_else(|| grid_overflow(resources))?;
    let left_marker_end =
        resources.checked_grid_add(content_start, left_endpoint.marker_width())?;
    let right_marker_start = resources
        .checked_grid_add(content_end, 1)?
        .checked_sub(right_endpoint.marker_width())
        .ok_or_else(|| grid_overflow(resources))?;
    if left_marker_end >= right_marker_start {
        return Err(grid_overflow(resources));
    }
    for x in left_marker_end..right_marker_start {
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
        resources.checked_grid_add(content_end, 1)?,
        resources,
    )
}

fn draw_horizontal_labels(
    canvas: &mut Canvas,
    edge: &HorizontalEdgePlan,
    left_endpoint: &HorizontalRelationEndpoint,
    right_endpoint: &HorizontalRelationEndpoint,
    content_start: usize,
    content_end: usize,
    resources: &ResourceContext,
) -> Result<()> {
    let left_width = left_endpoint.label_width();
    let relation_width = edge.style.label_width();
    let right_width = right_endpoint.label_width();
    let right_start = content_end
        .checked_sub(right_width)
        .ok_or_else(|| grid_overflow(resources))?;
    let ideal_relation_start = content_start
        .checked_add(content_end)
        .and_then(|sum| sum.checked_sub(relation_width))
        .map(|remaining| remaining / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let minimum_relation_start = resources.checked_grid_add(
        content_start,
        resources.checked_grid_add(left_width, usize::from(left_width > 0) * 2)?,
    )?;
    let maximum_relation_start = right_start
        .checked_sub(resources.checked_grid_add(relation_width, usize::from(right_width > 0) * 2)?)
        .unwrap_or(minimum_relation_start);
    let relation_start = ideal_relation_start
        .max(minimum_relation_start)
        .min(maximum_relation_start.max(minimum_relation_start));

    if let Some(label) = left_endpoint.label.as_ref() {
        draw_label_at(canvas, content_start, edge.label_top, label, resources)?;
    }
    if let Some(label) = edge.style.label.as_ref() {
        draw_label_at(canvas, relation_start, edge.label_top, label, resources)?;
    }
    if let Some(label) = right_endpoint.label.as_ref() {
        draw_label_at(canvas, right_start, edge.label_top, label, resources)?;
    }
    Ok(())
}

fn draw_label_at(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &RelationGraphLabel,
    resources: &ResourceContext,
) -> Result<()> {
    for (offset, line) in label.lines().iter().enumerate() {
        let row = resources.checked_grid_add(y, offset)?;
        canvas.write_text_role(x, row, line, AsciiColorRole::EdgeLabel)?;
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
) -> Result<()> {
    for y in start_y.min(end_y)..=start_y.max(end_y) {
        put_relation_char(canvas, x, y, ch, chars)?;
    }
    Ok(())
}

fn horizontal_box_strip_lines(
    boxes: &[&RelationGraphBox],
    direction: RelationGraphHorizontalDirection,
    gap: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let extent = horizontal_box_strip_ref_extent(boxes, gap, resources)?;
    let height = extent.height();
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for y in 0..height {
        let mut parts = Vec::new();
        let part_capacity = resources
            .checked_work_mul(boxes.len(), 2)?
            .saturating_sub(1);
        parts
            .try_reserve_exact(part_capacity)
            .map_err(|_| layout_allocation_failed())?;
        for ordered_index in 0..boxes.len() {
            let box_index = if direction.is_reversed() {
                boxes.len() - ordered_index - 1
            } else {
                ordered_index
            };
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

fn box_index(boxes: &[&RelationGraphBox], id: &str) -> Option<usize> {
    boxes
        .iter()
        .position(|relation_box| relation_box.id() == id)
}

fn node_index(nodes: &[HorizontalNode<'_>], id: &str) -> Option<usize> {
    nodes.iter().position(|node| node.original.id() == id)
}
