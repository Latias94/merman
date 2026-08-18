mod geometry;
mod paint;

pub(super) use super::tree::SequenceParticipantSpan;
pub(super) use geometry::SequenceControlBoundaryState;

use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::model::SequenceControlKind;
use super::text::{SequenceDocumentExtent, SequenceLine, SequenceRowFootprint};
use super::{SequenceCheckpointCursor, try_plan_sequence_label};
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::LabelBreakPolicy;
use crate::safe_text::NormalizedLabelPlan;
use geometry::SequenceFrameBounds;
use paint::materialize_control_frames;

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
pub(super) struct SequenceControlFrameNode {
    pub(super) frame_index: usize,
    pub(super) children: Vec<usize>,
    pub(super) depth: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SequenceControlFrameForest {
    pub(super) nodes: Vec<SequenceControlFrameNode>,
    pub(super) roots: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct SequenceControlFrameTree<'diagram> {
    pub(super) forest: SequenceControlFrameForest,
    pub(super) frames: Vec<SequenceControlFrame<'diagram>>,
}

#[derive(Debug)]
pub(super) struct PreparedSequenceControlFrames<'diagram> {
    forest: SequenceControlFrameForest,
    frames: Vec<SequenceControlFrame<'diagram>>,
    frame_plans: Vec<SequenceControlFramePlan<'diagram>>,
    output_admission: SequenceControlOutputAdmission,
}

#[derive(Debug)]
struct SequenceControlFramePlan<'a> {
    body_rows: usize,
    bounds: SequenceFrameBounds,
    row_count: usize,
    total_width: usize,
    title: SequenceControlTitlePlan<'a>,
    separator_titles: Vec<SequenceControlTitlePlan<'a>>,
}

struct SequenceFrameBodyPlanContext<'a, 'diagram> {
    forest: &'a SequenceControlFrameForest,
    frames: &'a [SequenceControlFrame<'diagram>],
    footprints: &'a [SequenceRowFootprint],
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
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        checkpoints.before_charge()?;
        resources.charge_layout_work(keyword.len().max(1))?;
        let label_plan = if label.is_empty() {
            None
        } else {
            let plan = try_plan_sequence_label(
                label,
                width_profile,
                false,
                None,
                LabelBreakPolicy::VisibleLine,
                resources,
                checkpoints,
            )?
            .ok_or_else(invalid_control_frame)?;
            checkpoints.before_charge()?;
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
        checkpoints.before_charge()?;
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

    fn materialization_work_units(self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_work_add(
            self.capacity.max(1),
            self.label_plan
                .map_or(0, NormalizedLabelPlan::materialization_work_units),
        )
    }

    fn materialize_after_admission(
        self,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<String> {
        checkpoints.checkpoint()?;
        let materialized = self.materialize_impl(|| checkpoints.checkpoint());
        checkpoints.checkpoint()?;
        materialized
    }

    fn materialize_impl(self, mut checkpoint: impl FnMut() -> Result<()>) -> Result<String> {
        let label = match self.label_plan {
            Some(plan) => {
                let (mut lines, _) = plan
                    .materialize_after_admission_with_checkpoint(self.label, &mut checkpoint)?
                    .into_parts();
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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SequenceControlOutputAdmission {
    height: usize,
    max_width: usize,
    document_cells: usize,
    work_units: usize,
}

pub(super) fn prepare_sequence_control_frames<'diagram>(
    tree: SequenceControlFrameTree<'diagram>,
    footprints: &[SequenceRowFootprint],
    layout: &SequenceLayout,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Option<PreparedSequenceControlFrames<'diagram>>> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        prepare_sequence_control_frames_transactional(
            tree,
            footprints,
            layout,
            resources,
            checkpoints,
        )
    })
}

fn prepare_sequence_control_frames_transactional<'diagram>(
    tree: SequenceControlFrameTree<'diagram>,
    footprints: &[SequenceRowFootprint],
    layout: &SequenceLayout,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Option<PreparedSequenceControlFrames<'diagram>>> {
    if tree.frames.is_empty() || footprints.is_empty() {
        return Ok(None);
    }

    let mut input_width = 0usize;
    for footprint in footprints {
        checkpoints.tick()?;
        input_width = input_width.max(footprint.retained_width());
    }
    resources.grid_extent(input_width, footprints.len())?;
    checkpoints.before_charge()?;
    charge_work_product(resources, tree.frames.len(), 2)?;
    resources.grid_extent(tree.frames.len(), 1)?;
    if tree.forest.nodes.is_empty() {
        return Ok(None);
    }
    let frame_plans = plan_control_frames(
        &tree.forest,
        &tree.frames,
        footprints,
        layout,
        input_width,
        resources,
        checkpoints,
    )?;
    let output_admission = admit_control_output(
        footprints,
        &tree.forest,
        &tree.frames,
        &frame_plans,
        resources,
        checkpoints,
    )?;
    Ok(Some(PreparedSequenceControlFrames {
        forest: tree.forest,
        frames: tree.frames,
        frame_plans,
        output_admission,
    }))
}

impl PreparedSequenceControlFrames<'_> {
    pub(super) const fn output_extent(&self) -> SequenceDocumentExtent {
        SequenceDocumentExtent::new(
            self.output_admission.max_width,
            self.output_admission.height,
        )
    }

    pub(super) fn materialization_work_units(
        &self,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<usize> {
        control_materialization_work_units(&self.frame_plans, resources, checkpoints)
    }

    pub(super) fn materialize(
        self,
        lines: Vec<SequenceLine>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Vec<SequenceLine>> {
        materialize_control_frames(
            lines,
            &self.forest,
            &self.frames,
            &self.frame_plans,
            self.output_admission,
            layout,
            chars,
            resources,
            checkpoints,
        )
    }
}

fn control_materialization_work_units(
    frame_plans: &[SequenceControlFramePlan<'_>],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<usize> {
    let mut total = 0usize;
    for plan in frame_plans {
        checkpoints.tick()?;
        total =
            resources.checked_work_add(total, plan.title.materialization_work_units(resources)?)?;
        for title in &plan.separator_titles {
            checkpoints.tick()?;
            total =
                resources.checked_work_add(total, title.materialization_work_units(resources)?)?;
        }
    }
    Ok(total)
}

#[cfg(test)]
fn line_footprint(line: &SequenceLine) -> Result<SequenceRowFootprint> {
    let retained_width = line.len();
    let left = (0..retained_width).find(|index| line.get(*index).is_some_and(|ch| ch != ' '));
    let right = (0..retained_width)
        .rev()
        .find(|index| line.get(*index).is_some_and(|ch| ch != ' '));
    match left.zip(right) {
        Some((left, right)) => SequenceRowFootprint::with_content(retained_width, left, right),
        None => Ok(SequenceRowFootprint::lifeline(retained_width)),
    }
}

fn plan_control_frames<'diagram>(
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'diagram>],
    footprints: &[SequenceRowFootprint],
    layout: &SequenceLayout,
    input_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceControlFramePlan<'diagram>>> {
    let body_context = SequenceFrameBodyPlanContext {
        forest,
        frames,
        footprints,
    };
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    pending.resize_with(forest.nodes.len(), || None);

    for node_index in (0..forest.nodes.len()).rev() {
        checkpoints.tick()?;
        let node = forest
            .nodes
            .get(node_index)
            .ok_or_else(invalid_control_frame)?;
        resources.check_nesting_depth(node.depth)?;
        checkpoints.before_charge()?;
        resources.charge_layout_work(1)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let (participant_span, initial_bounds) = if layout.participant_centers.is_empty() {
            (None, SequenceFrameBounds::full_width(input_width.max(1))?)
        } else {
            let span = match frame.participant_span {
                Some(span) => span,
                None => SequenceParticipantSpan::all(layout.participant_centers.len())?,
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
            checkpoints,
        )?;
        let title = frame_title_plan(frame, layout.width_profile, resources, checkpoints)?;
        let mut separator_titles = Vec::new();
        separator_titles
            .try_reserve_exact(frame.separators.len())
            .map_err(|_| allocation_failed())?;
        for separator in &frame.separators {
            checkpoints.tick()?;
            separator_titles.push(separator_title_plan(
                frame,
                separator,
                layout.width_profile,
                resources,
                checkpoints,
            )?);
        }
        let minimum_width = resources.checked_grid_add(title.width(), 2)?.max(3).max(
            resources.checked_grid_add(
                separator_titles
                    .iter()
                    .map(|title| title.width())
                    .max()
                    .unwrap_or(0),
                2,
            )?,
        );
        bounds.ensure_width(minimum_width, resources)?;
        let inset_levels = node
            .depth
            .checked_sub(1)
            .ok_or_else(invalid_control_frame)?;
        bounds.shift_right(resources.checked_grid_mul(inset_levels, 2)?, resources)?;
        let row_count = resources.checked_grid_add(body_rows, 2)?;
        let total_width = input_width.max(bounds.right_exclusive(resources)?);
        resources.grid_extent(total_width, row_count)?;
        checkpoints.before_charge()?;
        charge_work_product(resources, total_width, row_count)?;
        let slot = pending
            .get_mut(node_index)
            .ok_or_else(invalid_control_frame)?;
        *slot = Some(SequenceControlFramePlan {
            body_rows,
            bounds,
            row_count,
            total_width,
            title,
            separator_titles,
        });
    }

    let mut plans = Vec::new();
    plans
        .try_reserve_exact(pending.len())
        .map_err(|_| allocation_failed())?;
    for plan in pending {
        checkpoints.tick()?;
        plans.push(plan.ok_or_else(invalid_control_frame)?);
    }
    Ok(plans)
}

fn planned_frame_body_extent(
    node_index: usize,
    context: &SequenceFrameBodyPlanContext<'_, '_>,
    frame_plans: &[Option<SequenceControlFramePlan<'_>>],
    participant_span: Option<SequenceParticipantSpan>,
    mut bounds: SequenceFrameBounds,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
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
        checkpoints.before_charge()?;
        resources.charge_layout_work(1)?;
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            checkpoints.tick()?;
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

        let footprint = context
            .footprints
            .get(row)
            .copied()
            .ok_or_else(invalid_control_frame)?;
        planned_rows = resources.checked_grid_add(planned_rows, 1)?;
        if participant_span.is_some() {
            bounds.include_footprint_content(footprint, resources)?;
        }
        row = resources.checked_grid_add(row, 1)?;
    }

    Ok((planned_rows, bounds))
}

fn admit_control_output(
    footprints: &[SequenceRowFootprint],
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan<'_>],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceControlOutputAdmission> {
    let mut admission = SequenceControlOutputAdmission::default();
    let mut row = 0;

    for root in &forest.roots {
        checkpoints.tick()?;
        let node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let end_row =
            valid_frame_end_row(frame, footprints.len()).ok_or_else(invalid_control_frame)?;
        if frame.start_row < row {
            return Err(invalid_control_frame());
        }
        for footprint in footprints
            .get(row..frame.start_row)
            .ok_or_else(invalid_control_frame)?
        {
            checkpoints.tick()?;
            admission.add_line(footprint.retained_width(), resources)?;
        }
        let plan = frame_plans.get(*root).ok_or_else(invalid_control_frame)?;
        admission.add_uniform(plan.total_width, plan.row_count, resources)?;
        row = resources.checked_grid_add(end_row, 1)?;
    }

    for footprint in footprints.get(row..).ok_or_else(invalid_control_frame)? {
        checkpoints.tick()?;
        admission.add_line(footprint.retained_width(), resources)?;
    }
    admission.admit(resources, checkpoints)?;
    Ok(admission)
}

impl SequenceControlOutputAdmission {
    fn add_line(&mut self, width: usize, resources: &ResourceContext) -> Result<()> {
        self.height = resources.checked_grid_add(self.height, 1)?;
        self.max_width = self.max_width.max(width);
        self.document_cells = self
            .document_cells
            .checked_add(width)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
        let cells = width
            .checked_mul(height)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.document_cells = self
            .document_cells
            .checked_add(cells)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        let work = resources.checked_work_mul(width.max(1), height)?;
        self.work_units = resources.checked_work_add(self.work_units, work)?;
        Ok(())
    }

    fn admit(
        self,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        resources.grid_extent(self.max_width, self.height)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, self.document_cells)?;
        checkpoints.before_charge()?;
        resources.charge_layout_work(self.work_units)
    }

    fn validate(
        self,
        lines: &[SequenceLine],
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        let mut actual = Self::default();
        for line in lines {
            checkpoints.tick()?;
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

fn valid_frame_end_row(frame: &SequenceControlFrame<'_>, line_count: usize) -> Option<usize> {
    let end_row = frame.end_row?;
    (frame.start_row < line_count && end_row < line_count && frame.start_row <= end_row)
        .then_some(end_row)
}

fn frame_title_plan<'a>(
    frame: &SequenceControlFrame<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<SequenceControlTitlePlan<'a>> {
    SequenceControlTitlePlan::try_new(
        frame.kind.keyword(),
        frame.label,
        width_profile,
        resources,
        checkpoints,
    )
}

fn separator_title_plan<'a>(
    frame: &SequenceControlFrame<'_>,
    separator: &SequenceControlFrameSeparator<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<SequenceControlTitlePlan<'a>> {
    SequenceControlTitlePlan::try_new(
        frame
            .kind
            .separator_keyword()
            .unwrap_or_else(|| frame.kind.keyword()),
        separator.label,
        width_profile,
        resources,
        checkpoints,
    )
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
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
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::resource::AsciiResourcePolicy;
    use crate::sequence::text::blank_line;
    #[cfg(not(target_arch = "wasm32"))]
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};

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
        let exact = admit_and_materialize_control_title(84)
            .expect("the exact 14x6 aggregate control extent should be admitted");
        assert_eq!(exact, " loop batch ");

        let error = admit_and_materialize_control_title(83)
            .expect_err("the aggregate control extent should exceed the limit by one cell");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 84
                    && details.max == 83
        ));
    }

    fn admit_and_materialize_control_title(maximum: usize) -> Result<String> {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
            .expect("the aggregate control grid limit override should be valid");
        let mut resources = ResourceContext::new(policy);
        let lines = [
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
            blank_line(4, TerminalWidthProfile::Unicode, &resources)?,
        ];
        let frames = vec![
            test_frame(SequenceControlKind::Loop, "batch", 0, 0),
            test_frame(SequenceControlKind::Loop, "batch", 1, 1),
        ];
        let tree = disjoint_tree(frames);
        let footprints = lines
            .iter()
            .map(line_footprint)
            .collect::<Result<Vec<_>>>()?;
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&policy),
            OperationPhase::Layout,
        );
        let transaction = resources.clone();
        let result = transaction.transaction(|_| {
            let frame_plans = plan_control_frames(
                &tree.forest,
                &tree.frames,
                &footprints,
                &test_layout(),
                4,
                &mut resources,
                &mut checkpoints,
            )?;
            let admission = admit_control_output(
                &footprints,
                &tree.forest,
                &tree.frames,
                &frame_plans,
                &mut resources,
                &mut checkpoints,
            )?;
            assert_eq!(admission.max_width, 14);
            assert_eq!(admission.height, 6);
            let title = frame_plans[0].title;
            checkpoints.before_charge()?;
            resources.charge_layout_work(title.materialization_work_units(&resources)?)?;
            title.materialize_after_admission(&checkpoints)
        });
        if result.is_err() {
            assert_eq!(resources.layout_work_used(), 0);
            assert_eq!(resources.document_cells_used(), 0);
        }
        result
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
                let tree = nested_tree(frames);

                render_control_tree(
                    vec![line],
                    tree,
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

    #[test]
    fn control_frame_planning_inner_loop_observes_cancellation() {
        const ROWS: usize = 128;
        let policy = AsciiResourcePolicy::default();
        let mut resources = ResourceContext::new(policy);
        let lines = (0..ROWS)
            .map(|_| blank_line(4, TerminalWidthProfile::Unicode, &resources))
            .collect::<Result<Vec<_>>>()
            .expect("control test rows should fit");
        let footprints = lines
            .iter()
            .map(line_footprint)
            .collect::<Result<Vec<_>>>()
            .expect("control test footprints should be valid");
        let tree = disjoint_tree(vec![test_frame(SequenceControlKind::Loop, "", 0, ROWS - 1)]);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(5);

        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::new(&control, &policy),
            OperationPhase::Layout,
        );
        let error = prepare_sequence_control_frames(
            tree,
            &footprints,
            &test_layout(),
            &mut resources,
            &mut checkpoints,
        )
        .expect_err("control-frame planning should observe scheduled cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }

    #[test]
    fn control_frame_materialization_inner_loop_observes_cancellation() {
        const ROWS: usize = 128;
        let policy = AsciiResourcePolicy::default();
        let mut resources = ResourceContext::new(policy);
        let lines = (0..ROWS)
            .map(|_| blank_line(4, TerminalWidthProfile::Unicode, &resources))
            .collect::<Result<Vec<_>>>()
            .expect("control test rows should fit");
        let footprints = lines
            .iter()
            .map(line_footprint)
            .collect::<Result<Vec<_>>>()
            .expect("control test footprints should be valid");
        let tree = disjoint_tree(vec![test_frame(SequenceControlKind::Loop, "", 0, ROWS - 1)]);
        let mut planning_checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&policy),
            OperationPhase::Layout,
        );
        let prepared = prepare_sequence_control_frames(
            tree,
            &footprints,
            &test_layout(),
            &mut resources,
            &mut planning_checkpoints,
        )
        .expect("control-frame planning should succeed")
        .expect("the test frame should require materialization");
        let materialization_work = prepared
            .materialization_work_units(&resources, &mut planning_checkpoints)
            .expect("control-frame materialization work should be representable");
        planning_checkpoints
            .before_charge()
            .expect("standalone planning checkpoint should succeed");
        resources
            .charge_layout_work(materialization_work)
            .expect("control-frame materialization work should fit");
        let before_materialization = resources.layout_work_used();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::new(&control, &policy),
            OperationPhase::Layout,
        );

        let transaction = resources.clone();
        let error = transaction
            .transaction(|_| {
                prepared.materialize(
                    lines,
                    &test_layout(),
                    &ascii_chars(),
                    &mut resources,
                    &mut checkpoints,
                )
            })
            .expect_err("control-frame materialization should observe scheduled cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), before_materialization);
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

        render_control_tree(
            lines,
            disjoint_tree(frames),
            &test_layout(),
            &ascii_chars(),
            &mut resources,
        )
    }

    fn render_control_tree(
        lines: Vec<SequenceLine>,
        tree: SequenceControlFrameTree<'_>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
    ) -> Result<Vec<SequenceLine>> {
        let footprints = lines
            .iter()
            .map(line_footprint)
            .collect::<Result<Vec<_>>>()?;
        let policy = resources.policy();
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&policy),
            OperationPhase::Layout,
        );
        let Some(prepared) = prepare_sequence_control_frames(
            tree,
            &footprints,
            layout,
            resources,
            &mut checkpoints,
        )?
        else {
            return Ok(lines);
        };
        let materialization_work =
            prepared.materialization_work_units(resources, &mut checkpoints)?;
        checkpoints.before_charge()?;
        resources.charge_layout_work(materialization_work)?;
        prepared.materialize(lines, layout, chars, resources, &mut checkpoints)
    }

    fn disjoint_tree(
        frames: Vec<SequenceControlFrame<'static>>,
    ) -> SequenceControlFrameTree<'static> {
        let nodes = (0..frames.len())
            .map(|frame_index| SequenceControlFrameNode {
                frame_index,
                children: Vec::new(),
                depth: 1,
            })
            .collect();
        SequenceControlFrameTree {
            forest: SequenceControlFrameForest {
                nodes,
                roots: (0..frames.len()).collect(),
            },
            frames,
        }
    }

    fn nested_tree(
        frames: Vec<SequenceControlFrame<'static>>,
    ) -> SequenceControlFrameTree<'static> {
        let nodes = (0..frames.len())
            .map(|frame_index| SequenceControlFrameNode {
                frame_index,
                children: (frame_index + 1 < frames.len())
                    .then_some(frame_index + 1)
                    .into_iter()
                    .collect(),
                depth: frame_index + 1,
            })
            .collect();
        SequenceControlFrameTree {
            forest: SequenceControlFrameForest {
                nodes,
                roots: (!frames.is_empty()).then_some(0).into_iter().collect(),
            },
            frames,
        }
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
