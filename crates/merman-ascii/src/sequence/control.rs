mod geometry;

pub(super) use geometry::{SequenceControlBoundaryState, SequenceParticipantSpan};

use super::layout::SequenceLayout;
use super::model::SequenceControlKind;
use super::render::SequenceChars;
use super::text::{SequenceLine, padded_line};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::NormalizedLabelPlan;
use crate::safe_text::{LabelBreakPolicy, try_plan_normalized_label_lines_with_policy};
use geometry::SequenceFrameBounds;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrame<'a> {
    pub(super) kind: SequenceControlKind,
    pub(super) label: &'a str,
    pub(super) background: Option<AsciiRgb>,
    pub(super) participant_span: Option<SequenceParticipantSpan>,
    pub(super) start_boundary: SequenceControlBoundaryState,
    pub(super) start_row: usize,
    pub(super) separators: Vec<SequenceControlFrameSeparator<'a>>,
    pub(super) end_boundary: Option<SequenceControlBoundaryState>,
    pub(super) end_row: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrameSeparator<'a> {
    pub(super) label: &'a str,
    pub(super) boundary: SequenceControlBoundaryState,
    pub(super) row: usize,
}

impl SequenceControlFrame<'_> {
    pub(super) fn current_section_start_row(&self) -> usize {
        self.separators
            .last()
            .map(|separator| separator.row)
            .unwrap_or(self.start_row)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlFrameNode {
    frame_index: usize,
    children: Vec<usize>,
    depth: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlFrameForest {
    nodes: Vec<SequenceControlFrameNode>,
    roots: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceControlBodyRow {
    Content,
    Separator(usize),
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlBody {
    rows: Vec<SequenceControlBodyRow>,
    content: Vec<SequenceLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceControlFramePlan {
    body_rows: usize,
    bounds: SequenceFrameBounds,
    row_count: usize,
    total_width: usize,
}

struct SequenceFrameBodyPlanContext<'a, 'diagram> {
    forest: &'a SequenceControlFrameForest,
    frames: &'a [SequenceControlFrame<'diagram>],
    lines: &'a [SequenceLine],
    layout: &'a SequenceLayout,
    chars: &'a SequenceChars,
}

#[derive(Debug, Clone, Copy)]
struct SequenceControlTitlePlan<'a> {
    keyword: &'static str,
    label: &'a str,
    label_plan: Option<NormalizedLabelPlan>,
    width: usize,
    capacity: usize,
}

impl<'a> SequenceControlTitlePlan<'a> {
    fn try_new(
        keyword: &'static str,
        label: &'a str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.charge_layout_work(keyword.len().max(1))?;
        let label_plan = if label.is_empty() {
            None
        } else {
            let plan = try_plan_normalized_label_lines_with_policy(
                label,
                width_profile,
                false,
                None,
                LabelBreakPolicy::VisibleLine,
                resources,
            )?
            .ok_or_else(invalid_control_frame)?;
            plan.check_materialization_limits(resources)?;
            Some(plan)
        };
        let label_metrics = label_plan.map(NormalizedLabelPlan::metrics);
        let separator_bytes = if label.is_empty() { 2 } else { 3 };
        let capacity = keyword
            .len()
            .checked_add(label_metrics.map_or(0, |metrics| metrics.materialized_bytes))
            .and_then(|length| length.checked_add(separator_bytes))
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, capacity)?;

        let mut width = resources.checked_grid_add(keyword.len(), 2)?;
        if let Some(label_metrics) = label_metrics {
            width = resources.checked_grid_add(
                width,
                resources.checked_grid_add(label_metrics.max_width, 1)?,
            )?;
        }
        Ok(Self {
            keyword,
            label,
            label_plan,
            width,
            capacity,
        })
    }

    const fn width(self) -> usize {
        self.width
    }

    fn materialize(self, resources: &ResourceContext) -> Result<String> {
        self.materialize_with(resources, || {})
    }

    fn materialize_with(
        self,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<String> {
        resources.charge_layout_work(self.capacity.max(1))?;
        before_materialize();
        let label = match self.label_plan {
            Some(plan) => {
                let (mut lines, _) = plan.materialize(self.label, resources)?.into_parts();
                if lines.len() != 1 {
                    return Err(invalid_control_frame());
                }
                Some(lines.pop().ok_or_else(invalid_control_frame)?)
            }
            None => None,
        };
        let mut title = String::new();
        title
            .try_reserve_exact(self.capacity)
            .map_err(|_| allocation_failed())?;
        title.push(' ');
        title.push_str(self.keyword);
        if let Some(label) = label.as_deref() {
            title.push(' ');
            title.push_str(label);
        }
        title.push(' ');
        if title.len() != self.capacity {
            return Err(invalid_control_frame());
        }
        Ok(title)
    }

    #[cfg(test)]
    fn materialize_with_probe(
        self,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<String> {
        self.materialize_with(resources, || materialized.set(true))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SequenceControlOutputAdmission {
    height: usize,
    max_width: usize,
    document_cells: usize,
    work_units: usize,
}

pub(super) fn render_sequence_control_frames(
    lines: Vec<SequenceLine>,
    frames: &[SequenceControlFrame<'_>],
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    if frames.is_empty() || lines.is_empty() {
        return Ok(lines);
    }

    let line_count = lines.len();
    let width_profile = lines[0].width_profile();
    let input_width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    resources.grid_extent(input_width, line_count)?;
    charge_work_product(resources, frames.len(), 2)?;
    resources.grid_extent(frames.len(), 1)?;
    let forest = control_frame_tree(frames, line_count, resources)?;
    if forest.nodes.is_empty() {
        return Ok(lines);
    }
    let frame_plans = plan_control_frames(
        &forest,
        frames,
        &lines,
        layout,
        chars,
        width_profile,
        resources,
    )?;
    let output_admission = admit_control_output(&lines, &forest, frames, &frame_plans, resources)?;

    let mut remaining_lines = Vec::new();
    remaining_lines
        .try_reserve_exact(line_count)
        .map_err(|_| allocation_failed())?;
    remaining_lines.extend(lines.into_iter().map(Some));

    let mut rendered_nodes = Vec::new();
    rendered_nodes
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    rendered_nodes.resize_with(forest.nodes.len(), || None);

    let mut traversal = Vec::new();
    traversal
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    for root in forest.roots.iter().rev() {
        let root_node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        resources.check_nesting_depth(root_node.depth)?;
        traversal.push((*root, false));
    }

    while let Some((node_index, expanded)) = traversal.pop() {
        if expanded {
            let rendered = render_frame_node_iterative(
                node_index,
                &forest,
                frames,
                &frame_plans,
                &mut remaining_lines,
                layout,
                chars,
                width_profile,
                &mut rendered_nodes,
                resources,
            )?;
            let slot = rendered_nodes
                .get_mut(node_index)
                .ok_or_else(invalid_control_frame)?;
            if slot.replace(rendered).is_some() {
                return Err(invalid_control_frame());
            }
            continue;
        }

        let node = forest
            .nodes
            .get(node_index)
            .ok_or_else(invalid_control_frame)?;
        let additional = node
            .children
            .len()
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        traversal
            .try_reserve(additional)
            .map_err(|_| allocation_failed())?;
        traversal.push((node_index, true));
        for child in node.children.iter().rev() {
            let child_node = forest.nodes.get(*child).ok_or_else(invalid_control_frame)?;
            resources.check_nesting_depth(child_node.depth)?;
            traversal.push((*child, false));
        }
    }

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(output_admission.height)
        .map_err(|_| allocation_failed())?;
    let mut row = 0;

    for root in &forest.roots {
        let node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let Some(node_end) = valid_frame_end_row(frame, line_count) else {
            continue;
        };

        if row < frame.start_row {
            extend_taken_lines(&mut rendered, &mut remaining_lines, row..frame.start_row)?;
        }
        let frame_lines = rendered_nodes
            .get_mut(*root)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        extend_owned_lines(&mut rendered, frame_lines)?;
        row = resources.checked_grid_add(node_end, 1)?;
    }

    if row < line_count {
        extend_taken_lines(&mut rendered, &mut remaining_lines, row..line_count)?;
    }
    if remaining_lines.iter().any(Option::is_some) || rendered_nodes.iter().any(Option::is_some) {
        return Err(invalid_control_frame());
    }
    output_admission.validate(&rendered, resources)?;
    Ok(rendered)
}

#[allow(clippy::too_many_arguments)]
fn render_frame_node_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan],
    lines: &mut [Option<SequenceLine>],
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    resources.check_nesting_depth(node.depth)?;
    let frame = frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let plan = frame_plans
        .get(node_index)
        .copied()
        .ok_or_else(invalid_control_frame)?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(plan.row_count)
        .map_err(|_| allocation_failed())?;
    let body = render_frame_body_iterative(
        node_index,
        forest,
        frames,
        lines,
        rendered_nodes,
        plan.body_rows,
        resources,
    )?;
    let mut content = body.content.into_iter();
    rendered.push(render_top_border(
        frame,
        plan,
        layout,
        chars,
        width_profile,
        resources,
    )?);

    for row in body.rows {
        match row {
            SequenceControlBodyRow::Content => {
                let line = content.next().ok_or_else(invalid_control_frame)?;
                rendered.push(render_content_row(
                    line,
                    plan,
                    chars,
                    frame.background,
                    resources,
                )?);
            }
            SequenceControlBodyRow::Separator(separator_index) => {
                let separator = frame
                    .separators
                    .get(separator_index)
                    .ok_or_else(invalid_control_frame)?;
                rendered.push(render_separator_border(
                    frame,
                    separator,
                    plan,
                    layout,
                    chars,
                    width_profile,
                    resources,
                )?);
            }
        }
    }
    if content.next().is_some() {
        return Err(invalid_control_frame());
    }

    rendered.push(render_bottom_border(
        frame,
        plan,
        layout,
        chars,
        frame.background,
        resources,
    )?);
    Ok(rendered)
}

fn plan_control_frames(
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    lines: &[SequenceLine],
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceControlFramePlan>> {
    let input_width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    let body_context = SequenceFrameBodyPlanContext {
        forest,
        frames,
        lines,
        layout,
        chars,
    };
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    pending.resize(forest.nodes.len(), None);

    for node_index in (0..forest.nodes.len()).rev() {
        let node = forest
            .nodes
            .get(node_index)
            .ok_or_else(invalid_control_frame)?;
        resources.check_nesting_depth(node.depth)?;
        resources.charge_layout_work(1)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let (participant_span, initial_bounds) = if layout.participant_centers.is_empty() {
            (None, SequenceFrameBounds::full_width(input_width.max(1))?)
        } else {
            let span = match frame.participant_span {
                Some(span) => span,
                None => SequenceParticipantSpan::all(layout)?,
            };
            (
                Some(span),
                SequenceFrameBounds::from_participants(span, layout, resources)?,
            )
        };
        let (body_rows, mut bounds) = planned_frame_body_extent(
            node_index,
            &body_context,
            &pending,
            participant_span,
            initial_bounds,
            resources,
        )?;
        bounds.ensure_width(
            minimum_frame_width(frame, width_profile, resources)?,
            resources,
        )?;
        let inset_levels = node
            .depth
            .checked_sub(1)
            .ok_or_else(invalid_control_frame)?;
        bounds.shift_right(resources.checked_grid_mul(inset_levels, 2)?, resources)?;
        let row_count = resources.checked_grid_add(body_rows, 2)?;
        let total_width = input_width.max(bounds.right_exclusive(resources)?);
        resources.grid_extent(total_width, row_count)?;
        charge_work_product(resources, total_width, row_count)?;
        let slot = pending
            .get_mut(node_index)
            .ok_or_else(invalid_control_frame)?;
        *slot = Some(SequenceControlFramePlan {
            body_rows,
            bounds,
            row_count,
            total_width,
        });
    }

    let mut plans = Vec::new();
    plans
        .try_reserve_exact(pending.len())
        .map_err(|_| allocation_failed())?;
    for plan in pending {
        plans.push(plan.ok_or_else(invalid_control_frame)?);
    }
    Ok(plans)
}

fn planned_frame_body_extent(
    node_index: usize,
    context: &SequenceFrameBodyPlanContext<'_, '_>,
    frame_plans: &[Option<SequenceControlFramePlan>],
    participant_span: Option<SequenceParticipantSpan>,
    mut bounds: SequenceFrameBounds,
    resources: &mut ResourceContext,
) -> Result<(usize, SequenceFrameBounds)> {
    let node = context
        .forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = context
        .frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let end_row = frame.end_row.ok_or_else(invalid_control_frame)?;
    let mut planned_rows = 0;
    let mut row = frame.start_row;
    let mut child_index = 0;
    let mut separator_index = 0;

    while row <= end_row {
        resources.charge_layout_work(1)?;
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            planned_rows = resources.checked_grid_add(planned_rows, 1)?;
            separator_index = resources.checked_grid_add(separator_index, 1)?;
        }

        if let Some(child_node_index) = node.children.get(child_index).copied() {
            let child = context
                .forest
                .nodes
                .get(child_node_index)
                .ok_or_else(invalid_control_frame)?;
            let child_frame = context
                .frames
                .get(child.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if child_frame.start_row == row {
                let child_plan = frame_plans
                    .get(child_node_index)
                    .and_then(Option::as_ref)
                    .ok_or_else(invalid_control_frame)?;
                planned_rows = resources.checked_grid_add(planned_rows, child_plan.row_count)?;
                bounds.include_child(child_plan.bounds, resources)?;
                row = resources
                    .checked_grid_add(child_frame.end_row.ok_or_else(invalid_control_frame)?, 1)?;
                child_index = resources.checked_grid_add(child_index, 1)?;
                continue;
            }
        }

        let line = context.lines.get(row).ok_or_else(invalid_control_frame)?;
        planned_rows = resources.checked_grid_add(planned_rows, 1)?;
        if let Some(participant_span) = participant_span {
            bounds.include_line_content(
                line,
                participant_span,
                context.layout,
                context.chars,
                resources,
            )?;
        }
        row = resources.checked_grid_add(row, 1)?;
    }

    Ok((planned_rows, bounds))
}

fn admit_control_output(
    lines: &[SequenceLine],
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan],
    resources: &mut ResourceContext,
) -> Result<SequenceControlOutputAdmission> {
    let mut admission = SequenceControlOutputAdmission::default();
    let mut row = 0;

    for root in &forest.roots {
        let node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let end_row = valid_frame_end_row(frame, lines.len()).ok_or_else(invalid_control_frame)?;
        if frame.start_row < row {
            return Err(invalid_control_frame());
        }
        for line in lines
            .get(row..frame.start_row)
            .ok_or_else(invalid_control_frame)?
        {
            admission.add_line(line.len(), resources)?;
        }
        let plan = frame_plans
            .get(*root)
            .copied()
            .ok_or_else(invalid_control_frame)?;
        admission.add_uniform(plan.total_width, plan.row_count, resources)?;
        row = resources.checked_grid_add(end_row, 1)?;
    }

    for line in lines.get(row..).ok_or_else(invalid_control_frame)? {
        admission.add_line(line.len(), resources)?;
    }
    admission.admit(resources)?;
    Ok(admission)
}

impl SequenceControlOutputAdmission {
    fn add_line(&mut self, width: usize, resources: &ResourceContext) -> Result<()> {
        self.height = resources.checked_grid_add(self.height, 1)?;
        self.max_width = self.max_width.max(width);
        self.document_cells = self.document_cells.checked_add(width).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        self.work_units = resources.checked_work_add(self.work_units, width.max(1))?;
        Ok(())
    }

    fn add_uniform(
        &mut self,
        width: usize,
        height: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.height = resources.checked_grid_add(self.height, height)?;
        self.max_width = self.max_width.max(width);
        let cells = width.checked_mul(height).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        self.document_cells = self.document_cells.checked_add(cells).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        let work = resources.checked_work_mul(width.max(1), height)?;
        self.work_units = resources.checked_work_add(self.work_units, work)?;
        Ok(())
    }

    fn admit(self, resources: &mut ResourceContext) -> Result<()> {
        resources.grid_extent(self.max_width, self.height)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, self.document_cells)?;
        resources.charge_layout_work(self.work_units)
    }

    fn validate(self, lines: &[SequenceLine], resources: &ResourceContext) -> Result<()> {
        let mut actual = Self::default();
        for line in lines {
            actual.add_line(line.len(), resources)?;
        }
        if actual.height != self.height
            || actual.max_width != self.max_width
            || actual.document_cells != self.document_cells
            || actual.work_units != self.work_units
        {
            return Err(invalid_control_frame());
        }
        Ok(())
    }
}

fn render_frame_body_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    lines: &mut [Option<SequenceLine>],
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    planned_rows: usize,
    resources: &mut ResourceContext,
) -> Result<SequenceControlBody> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let end_row = frame.end_row.ok_or_else(invalid_control_frame)?;
    let mut body_rows = Vec::new();
    body_rows
        .try_reserve_exact(planned_rows)
        .map_err(|_| allocation_failed())?;
    let mut content = Vec::new();
    content
        .try_reserve_exact(planned_rows)
        .map_err(|_| allocation_failed())?;
    let mut row = frame.start_row;
    let mut child_index = 0;
    let mut separator_index = 0;

    while row <= end_row {
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            body_rows.try_reserve(1).map_err(|_| allocation_failed())?;
            body_rows.push(SequenceControlBodyRow::Separator(separator_index));
            separator_index = resources.checked_grid_add(separator_index, 1)?;
        }

        if let Some(child_node_index) = node.children.get(child_index).copied() {
            let child = forest
                .nodes
                .get(child_node_index)
                .ok_or_else(invalid_control_frame)?;
            let child_frame = frames
                .get(child.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if child_frame.start_row == row {
                resources.check_nesting_depth(child.depth)?;
                let child_lines = rendered_nodes
                    .get_mut(child_node_index)
                    .and_then(Option::take)
                    .ok_or_else(invalid_control_frame)?;
                body_rows
                    .try_reserve(child_lines.len())
                    .map_err(|_| allocation_failed())?;
                content
                    .try_reserve(child_lines.len())
                    .map_err(|_| allocation_failed())?;
                body_rows.extend(std::iter::repeat_n(
                    SequenceControlBodyRow::Content,
                    child_lines.len(),
                ));
                content.extend(child_lines);
                row = resources
                    .checked_grid_add(child_frame.end_row.ok_or_else(invalid_control_frame)?, 1)?;
                child_index = resources.checked_grid_add(child_index, 1)?;
                continue;
            }
        }

        let line = lines
            .get_mut(row)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        body_rows.try_reserve(1).map_err(|_| allocation_failed())?;
        content.try_reserve(1).map_err(|_| allocation_failed())?;
        body_rows.push(SequenceControlBodyRow::Content);
        content.push(line);
        row = resources.checked_grid_add(row, 1)?;
    }

    if body_rows.len() != planned_rows {
        return Err(invalid_control_frame());
    }
    Ok(SequenceControlBody {
        rows: body_rows,
        content,
    })
}

fn control_frame_tree(
    frames: &[SequenceControlFrame<'_>],
    line_count: usize,
    resources: &mut ResourceContext,
) -> Result<SequenceControlFrameForest> {
    let mut forest = SequenceControlFrameForest {
        nodes: Vec::<SequenceControlFrameNode>::new(),
        roots: Vec::new(),
    };
    let mut active = Vec::<usize>::new();

    for (frame_index, frame) in frames.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if valid_frame_end_row(frame, line_count).is_none() {
            continue;
        }

        while let Some(node_index) = active.last().copied() {
            let node = forest
                .nodes
                .get(node_index)
                .ok_or_else(invalid_control_frame)?;
            let active_frame = frames
                .get(node.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if active_frame
                .end_row
                .is_some_and(|end_row| end_row < frame.start_row)
            {
                active.pop();
            } else {
                break;
            }
        }

        let depth = active
            .len()
            .checked_add(1)
            .ok_or_else(|| nesting_overflow(resources))?;
        resources.check_nesting_depth(depth)?;
        forest
            .nodes
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        active.try_reserve(1).map_err(|_| allocation_failed())?;
        let parent_index = active.last().copied();
        if let Some(parent_index) = parent_index {
            forest
                .nodes
                .get_mut(parent_index)
                .ok_or_else(invalid_control_frame)?
                .children
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        } else {
            forest
                .roots
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        }

        let node_index = forest.nodes.len();
        forest.nodes.push(SequenceControlFrameNode {
            frame_index,
            children: Vec::new(),
            depth,
        });

        if let Some(parent_index) = parent_index {
            let parent = forest
                .nodes
                .get_mut(parent_index)
                .ok_or_else(invalid_control_frame)?;
            parent.children.push(node_index);
        } else {
            forest.roots.push(node_index);
        }
        active.push(node_index);
    }

    Ok(forest)
}

fn valid_frame_end_row(frame: &SequenceControlFrame<'_>, line_count: usize) -> Option<usize> {
    let end_row = frame.end_row?;
    (frame.start_row < line_count && end_row < line_count && frame.start_row <= end_row)
        .then_some(end_row)
}

fn minimum_frame_width(
    frame: &SequenceControlFrame<'_>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let title_width = frame_title_plan(frame, width_profile, resources)?.width();
    let mut separator_width = 0;
    for separator in &frame.separators {
        separator_width = separator_width
            .max(separator_title_plan(frame, separator, width_profile, resources)?.width());
    }

    Ok(resources
        .checked_grid_add(title_width, 2)?
        .max(3)
        .max(resources.checked_grid_add(separator_width, 2)?))
}

fn render_top_border(
    frame: &SequenceControlFrame<'_>,
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title = frame_title_plan(frame, width_profile, resources)?.materialize(resources)?;
    let base = frame
        .start_boundary
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        resources,
    )
}

fn render_bottom_border(
    frame: &SequenceControlFrame<'_>,
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let base = frame
        .end_boundary
        .as_ref()
        .ok_or_else(invalid_control_frame)?
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        plan,
        None,
        background,
        resources,
    )
}

fn render_separator_border(
    frame: &SequenceControlFrame<'_>,
    separator: &SequenceControlFrameSeparator<'_>,
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title =
        separator_title_plan(frame, separator, width_profile, resources)?.materialize(resources)?;
    let base = separator
        .boundary
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.tee_right,
        chars.tee_left,
        chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        resources,
    )
}

// The arguments map one-to-one to the terminal border primitive: geometry, optional label,
// background, and terminal glyphs are intentionally kept explicit at call sites.
#[allow(clippy::too_many_arguments)]
fn render_border_row(
    base: SequenceLine,
    left: char,
    right: char,
    horizontal: char,
    plan: SequenceControlFramePlan,
    label: Option<&str>,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    resources.grid_extent(plan.total_width, 1)?;
    let mut row = padded_line(base, plan.total_width)?;
    let left_index = plan.bounds.left();
    let right_index = plan.bounds.right();
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background(&mut row, left_index..frame_end, background);
    for x in left_index..frame_end {
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(left_index, left, AsciiColorRole::SequenceFrame)?;
    row.try_set_role(right_index, right, AsciiColorRole::SequenceFrame)?;
    if let Some(label) = label {
        row.try_write_text_role(
            resources.checked_grid_add(left_index, 1)?,
            label,
            AsciiColorRole::Text,
        )?;
    }
    Ok(row)
}

fn render_content_row(
    row: SequenceLine,
    plan: SequenceControlFramePlan,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    resources.grid_extent(plan.total_width, 1)?;
    let mut row = padded_line(row, plan.total_width)?;
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background_if_unset(&mut row, plan.bounds.left()..frame_end, background);
    paint_frame_vertical_if_unset(&mut row, plan.bounds.left(), chars.vertical)?;
    paint_frame_vertical_if_unset(&mut row, plan.bounds.right(), chars.vertical)?;
    Ok(row)
}

fn paint_frame_vertical_if_unset(
    row: &mut SequenceLine,
    index: usize,
    vertical: char,
) -> Result<()> {
    if row.get(index) == Some(' ') {
        row.try_set_role(index, vertical, AsciiColorRole::SequenceFrame)?;
    }
    Ok(())
}

fn paint_row_background(
    row: &mut SequenceLine,
    range: impl Iterator<Item = usize>,
    background: Option<AsciiRgb>,
) {
    let Some(background) = background else {
        return;
    };
    for x in range {
        row.set_background_color(x, background);
    }
}

fn paint_row_background_if_unset(
    row: &mut SequenceLine,
    range: impl Iterator<Item = usize>,
    background: Option<AsciiRgb>,
) {
    let Some(background) = background else {
        return;
    };
    for x in range {
        row.set_background_color_if_unset(x, background);
    }
}

fn frame_title_plan<'a>(
    frame: &SequenceControlFrame<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceControlTitlePlan<'a>> {
    SequenceControlTitlePlan::try_new(frame.kind.keyword(), frame.label, width_profile, resources)
}

fn separator_title_plan<'a>(
    frame: &SequenceControlFrame<'_>,
    separator: &SequenceControlFrameSeparator<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceControlTitlePlan<'a>> {
    SequenceControlTitlePlan::try_new(
        frame
            .kind
            .separator_keyword()
            .unwrap_or_else(|| frame.kind.keyword()),
        separator.label,
        width_profile,
        resources,
    )
}

fn extend_taken_lines(
    target: &mut Vec<SequenceLine>,
    source: &mut [Option<SequenceLine>],
    range: std::ops::Range<usize>,
) -> Result<()> {
    let source = source.get_mut(range).ok_or_else(invalid_control_frame)?;
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    for line in source {
        target.push(line.take().ok_or_else(invalid_control_frame)?);
    }
    Ok(())
}

fn extend_owned_lines(target: &mut Vec<SequenceLine>, source: Vec<SequenceLine>) -> Result<()> {
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    target.extend(source);
    Ok(())
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn nesting_overflow(resources: &ResourceContext) -> AsciiError {
    resources.nesting_overflow()
}

fn invalid_control_frame() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "control block ordering",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::resource::AsciiResourcePolicy;
    use crate::sequence::text::blank_line;
    #[cfg(not(target_arch = "wasm32"))]
    use merman_core::resources::ResourceProfile;

    #[test]
    fn nested_frames_fail_at_the_configured_depth_before_rendering() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, 1)
            .expect("the nesting override should be valid");
        let mut resources = ResourceContext::new(policy);
        let line = blank_line(1, TerminalWidthProfile::Unicode, &resources)
            .expect("the seed row should fit");
        let frames = vec![
            test_frame(SequenceControlKind::Loop, "", 0, 0),
            test_frame(SequenceControlKind::Opt, "", 0, 0),
        ];

        let error = render_sequence_control_frames(
            vec![line],
            &frames,
            &test_layout(),
            &ascii_chars(),
            &mut resources,
        )
        .expect_err("the second frame should exceed the nesting policy");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == 2
                    && details.max == 1
        ));
    }

    #[test]
    fn control_output_admits_aggregate_extent_before_frame_materialization() {
        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxDocumentCells,
        ] {
            let rendered = render_disjoint_frames_with_limit(limit, 48)
                .expect("the exact aggregate output extent should be admitted");
            assert_eq!(rendered.len(), 6);

            let error = render_disjoint_frames_with_limit(limit, 47)
                .expect_err("the aggregate output extent should exceed the limit");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit && details.actual == 48 && details.max == 47
            ));
        }
    }

    #[test]
    fn control_title_materialization_follows_aggregate_grid_admission() {
        let exact_materialized = Cell::new(false);
        admit_and_materialize_control_title(84, &exact_materialized)
            .expect("the exact 14x6 aggregate control extent should be admitted");
        assert!(exact_materialized.get());

        let below_materialized = Cell::new(false);
        let error = admit_and_materialize_control_title(83, &below_materialized)
            .expect_err("the aggregate control extent should exceed the limit by one cell");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 84
                    && details.max == 83
        ));
        assert!(!below_materialized.get());
    }

    fn admit_and_materialize_control_title(
        maximum: usize,
        materialized: &Cell<bool>,
    ) -> Result<()> {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
            .expect("the aggregate control grid limit override should be valid");
        let mut resources = ResourceContext::new(policy);
        let lines = vec![
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
        ];
        let frames = vec![
            test_frame(SequenceControlKind::Loop, "batch", 0, 0),
            test_frame(SequenceControlKind::Loop, "batch", 1, 1),
        ];
        let forest = control_frame_tree(&frames, lines.len(), &mut resources)?;
        let frame_plans = plan_control_frames(
            &forest,
            &frames,
            &lines,
            &test_layout(),
            &ascii_chars(),
            TerminalWidthProfile::Unicode,
            &mut resources,
        )?;
        let admission =
            admit_control_output(&lines, &forest, &frames, &frame_plans, &mut resources)?;
        assert_eq!(admission.max_width, 14);
        assert_eq!(admission.height, 6);
        frame_title_plan(&frames[0], TerminalWidthProfile::Unicode, &resources)?
            .materialize_with_probe(&resources, materialized)?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn deeply_nested_frames_render_on_a_small_stack() {
        const DEPTH: usize = 96;

        let rendered_len = std::thread::Builder::new()
            .name("sequence-control-small-stack".to_string())
            .stack_size(64 * 1024)
            .spawn(|| {
                let policy =
                    AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
                let mut resources = ResourceContext::new(policy);
                let line = blank_line(1, TerminalWidthProfile::Unicode, &resources)
                    .expect("the seed row should fit");
                let frames = vec![test_frame(SequenceControlKind::Loop, "", 0, 0); DEPTH];

                render_sequence_control_frames(
                    vec![line],
                    &frames,
                    &test_layout(),
                    &ascii_chars(),
                    &mut resources,
                )
                .expect("iterative rendering should not depend on the thread stack")
                .len()
            })
            .expect("the small-stack thread should start")
            .join()
            .expect("the small-stack thread should finish");

        assert_eq!(rendered_len, DEPTH * 2 + 1);
    }

    fn render_disjoint_frames_with_limit(
        limit: AsciiResourceLimitId,
        maximum: usize,
    ) -> Result<Vec<SequenceLine>> {
        let policy = AsciiResourcePolicy::default()
            .with_limit(limit, maximum)
            .expect("the aggregate output limit override should be valid");
        let mut resources = ResourceContext::new(policy);
        let lines = vec![
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
        ];
        let frames = vec![
            test_frame(SequenceControlKind::Loop, "", 0, 0),
            test_frame(SequenceControlKind::Loop, "", 1, 1),
        ];

        render_sequence_control_frames(
            lines,
            &frames,
            &test_layout(),
            &ascii_chars(),
            &mut resources,
        )
    }

    fn test_frame(
        kind: SequenceControlKind,
        label: &'static str,
        start_row: usize,
        end_row: usize,
    ) -> SequenceControlFrame<'static> {
        SequenceControlFrame {
            kind,
            label,
            background: None,
            participant_span: None,
            start_boundary: SequenceControlBoundaryState::default(),
            start_row,
            separators: Vec::new(),
            end_boundary: Some(SequenceControlBoundaryState::default()),
            end_row: Some(end_row),
        }
    }

    fn test_layout() -> SequenceLayout {
        SequenceLayout {
            participant_widths: Vec::new(),
            participant_centers: Vec::new(),
            total_width: 3,
            message_spacing: 1,
            self_message_width: 4,
            width_profile: TerminalWidthProfile::Unicode,
        }
    }

    fn ascii_chars() -> SequenceChars {
        SequenceChars {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            active_vertical: '#',
            destroyed_mark: 'x',
            tee_down: '+',
            tee_up: '+',
            tee_right: '+',
            tee_left: '+',
            filled_arrow_right: '>',
            filled_arrow_left: '<',
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
            unicode_markers: false,
        }
    }
}
