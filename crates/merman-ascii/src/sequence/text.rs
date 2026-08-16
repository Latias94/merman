use super::SequenceCheckpointCursor;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{SafeLine, SafeText};
use crate::text::StyledLine;

pub(super) type SequenceLine = StyledLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceContentSpan {
    left: usize,
    right: usize,
}

impl SequenceContentSpan {
    pub(super) fn inclusive(left: usize, right: usize) -> Result<Self> {
        if left > right {
            return Err(invalid_extent_plan());
        }
        Ok(Self { left, right })
    }

    pub(super) const fn left(self) -> usize {
        self.left
    }

    pub(super) const fn right(self) -> usize {
        self.right
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceRowFootprint {
    retained_width: usize,
    content: Option<SequenceContentSpan>,
}

impl SequenceRowFootprint {
    pub(super) const fn lifeline(retained_width: usize) -> Self {
        Self {
            retained_width,
            content: None,
        }
    }

    pub(super) fn with_content(retained_width: usize, left: usize, right: usize) -> Result<Self> {
        if right >= retained_width {
            return Err(invalid_extent_plan());
        }
        Ok(Self {
            retained_width,
            content: Some(SequenceContentSpan::inclusive(left, right)?),
        })
    }

    pub(super) const fn retained_width(self) -> usize {
        self.retained_width
    }

    pub(super) const fn content(self) -> Option<SequenceContentSpan> {
        self.content
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SequenceBatchExtent {
    height: usize,
    materialized_width: usize,
    retained_width: usize,
    document_cells: usize,
    work_units: usize,
}

impl SequenceBatchExtent {
    pub(super) const fn with_materialized_width(materialized_width: usize) -> Self {
        Self {
            materialized_width,
            height: 0,
            retained_width: 0,
            document_cells: 0,
            work_units: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn from_line_lengths(
        materialized_width: usize,
        lengths: impl IntoIterator<Item = usize>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        Self::try_from_line_lengths(materialized_width, lengths.into_iter().map(Ok), resources)
    }

    #[cfg(test)]
    pub(super) fn try_from_line_lengths(
        materialized_width: usize,
        lengths: impl IntoIterator<Item = Result<usize>>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut extent = Self::with_materialized_width(materialized_width);
        for length in lengths {
            extent.try_push_line_length(length?, resources)?;
        }
        Ok(extent)
    }

    pub(super) fn try_from_line_lengths_with_checkpoints(
        materialized_width: usize,
        lengths: impl IntoIterator<Item = Result<usize>>,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        let mut extent = Self::with_materialized_width(materialized_width);
        for length in lengths {
            checkpoints.tick()?;
            extent.try_push_line_length(length?, resources)?;
        }
        Ok(extent)
    }

    pub(super) fn try_push_line_length(
        &mut self,
        length: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.height = resources.checked_grid_add(self.height, 1)?;
        self.retained_width = self.retained_width.max(length);
        self.materialized_width = self.materialized_width.max(length);
        self.document_cells = self
            .document_cells
            .checked_add(length)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.work_units = resources.checked_work_add(self.work_units, length.max(1))?;
        Ok(())
    }

    pub(super) fn uniform(
        height: usize,
        materialized_width: usize,
        retained_width: usize,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let document_cells = retained_width
            .checked_mul(height)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        let work_units = resources.checked_work_mul(retained_width.max(1), height)?;
        Ok(Self {
            height,
            materialized_width: materialized_width.max(retained_width),
            retained_width,
            document_cells,
            work_units,
        })
    }

    pub(super) const fn height(self) -> usize {
        self.height
    }

    pub(super) const fn retained_width(self) -> usize {
        self.retained_width
    }

    #[cfg(test)]
    pub(super) const fn materialized_width(self) -> usize {
        self.materialized_width
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SequenceExtentLedger {
    height: usize,
    retained_width: usize,
    document_cells: usize,
}

impl SequenceExtentLedger {
    pub(super) fn reserve(
        &self,
        batch: SequenceBatchExtent,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceExtentReservation> {
        checkpoints.before_charge()?;
        let height = resources.checked_grid_add(self.height, batch.height)?;
        let materialized_width = self.retained_width.max(batch.materialized_width);
        resources.grid_extent(materialized_width, height)?;

        let document_cells = self
            .document_cells
            .checked_add(batch.document_cells)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, document_cells)?;
        resources.charge_layout_work(batch.work_units)?;

        Ok(SequenceExtentReservation {
            batch,
            next: Self {
                height,
                retained_width: self.retained_width.max(batch.retained_width),
                document_cells,
            },
        })
    }

    #[cfg(test)]
    pub(super) const fn height(self) -> usize {
        self.height
    }

    #[cfg(test)]
    pub(super) const fn document_cells(self) -> usize {
        self.document_cells
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceExtentReservation {
    batch: SequenceBatchExtent,
    next: SequenceExtentLedger,
}

impl SequenceExtentReservation {
    pub(super) fn commit_footprints_with_checkpoints(
        self,
        ledger: &mut SequenceExtentLedger,
        footprints: &[SequenceRowFootprint],
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        validate_batch_footprints_with_checkpoints(self.batch, footprints, resources, checkpoints)?;
        *ledger = self.next;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn commit(
        self,
        ledger: &mut SequenceExtentLedger,
        lines: &[SequenceLine],
        resources: &ResourceContext,
    ) -> Result<()> {
        let actual = SequenceBatchExtent::from_line_lengths(
            self.batch.materialized_width,
            lines.iter().map(SequenceLine::len),
            resources,
        )?;
        validate_batch_extent(self.batch, actual)?;
        *ledger = self.next;
        Ok(())
    }

    pub(super) fn commit_with_checkpoints(
        self,
        ledger: &mut SequenceExtentLedger,
        lines: &[SequenceLine],
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        validate_batch_lines_with_checkpoints(self.batch, lines, resources, checkpoints)?;
        *ledger = self.next;
        Ok(())
    }
}

pub(super) fn validate_batch_lines_with_checkpoints(
    batch: SequenceBatchExtent,
    lines: &[SequenceLine],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let actual = SequenceBatchExtent::try_from_line_lengths_with_checkpoints(
        batch.materialized_width,
        lines.iter().map(|line| Ok(line.len())),
        resources,
        checkpoints,
    )?;
    validate_batch_extent(batch, actual)
}

pub(super) fn validate_batch_footprints_with_checkpoints(
    batch: SequenceBatchExtent,
    footprints: &[SequenceRowFootprint],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let actual = SequenceBatchExtent::try_from_line_lengths_with_checkpoints(
        batch.materialized_width,
        footprints
            .iter()
            .map(|footprint| Ok(footprint.retained_width())),
        resources,
        checkpoints,
    )?;
    validate_batch_extent(batch, actual)
}

fn validate_batch_extent(expected: SequenceBatchExtent, actual: SequenceBatchExtent) -> Result<()> {
    if actual.height != expected.height
        || actual.retained_width != expected.retained_width
        || actual.document_cells != expected.document_cells
        || actual.work_units != expected.work_units
    {
        return Err(invalid_extent_plan());
    }
    Ok(())
}

fn invalid_extent_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "row extent planning",
    }
}

#[cfg(test)]
pub(super) fn blank_line(
    width: usize,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    SequenceLine::try_blank_with_policy(width, width_profile, resources.policy())
}

pub(super) fn blank_line_with_checkpoints(
    width: usize,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let line_base = ResourceContext::new(resources.policy());
    let line_resources = checkpoints
        .execution()
        .resource_context(&line_base, merman_core::OperationPhase::Layout);
    SequenceLine::try_blank_with_resources_and_checkpoint(
        width,
        width_profile,
        &line_resources,
        || checkpoints.checkpoint(),
    )
}

pub(super) fn charge_text_work(
    value: &str,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &mut ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<()> {
    checkpoints.before_charge()?;
    resources.charge_layout_work(1)?;
    let text = SafeText::new(value);
    for logical_line in text.lines() {
        let line = SafeLine::new(logical_line);
        for grapheme in line.graphemes(width_profile) {
            checkpoints.before_charge()?;
            resources.check_grapheme_bytes(grapheme.text().len())?;
            resources.charge_layout_work(1)?;
        }
    }
    Ok(())
}

pub(super) fn padded_line_with_checkpoints(
    mut line: SequenceLine,
    width: usize,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    line.try_pad_to_with_checkpoint(width, || checkpoints.tick())?;
    Ok(line)
}

pub(super) fn write_text_role(
    line: &mut SequenceLine,
    start: usize,
    text: &str,
    role: AsciiColorRole,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    line.try_write_text_role_with_checkpoint(start, text, role, resources, || checkpoints.tick())
}

pub(super) fn trim_right(line: SequenceLine) -> Result<SequenceLine> {
    line.try_trim_right()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::operation::AsciiExecution;
    use crate::options::TerminalWidthProfile;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    #[test]
    fn extent_ledger_accepts_exact_grid_and_document_limits() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 12)
            .unwrap()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 12)
            .unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut ledger = SequenceExtentLedger::default();

        commit_uniform_batch(&mut ledger, 2, 4, &mut resources).unwrap();
        commit_uniform_batch(&mut ledger, 1, 4, &mut resources).unwrap();

        assert_eq!(ledger.height(), 3);
        assert_eq!(ledger.document_cells(), 12);
    }

    #[test]
    fn extent_ledger_rejects_limit_minus_one_with_exact_aggregate_counts() {
        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxDocumentCells,
        ] {
            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxGridCells, 12)
                .unwrap()
                .with_limit(AsciiResourceLimitId::MaxDocumentCells, 12)
                .unwrap()
                .with_limit(limit, 11)
                .unwrap();
            let mut resources = ResourceContext::new(policy);
            let mut ledger = SequenceExtentLedger::default();

            commit_uniform_batch(&mut ledger, 2, 4, &mut resources).unwrap();
            let error = reserve_uniform_batch(&ledger, 1, 4, &mut resources).unwrap_err();

            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit && details.actual == 12 && details.max == 11
            ));
        }
    }

    #[test]
    fn extent_ledger_accepts_exact_work_and_rejects_limit_minus_one() {
        let exact = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 12)
            .unwrap();
        let mut exact_resources = ResourceContext::new(exact);
        let mut exact_ledger = SequenceExtentLedger::default();
        commit_uniform_batch(&mut exact_ledger, 2, 4, &mut exact_resources).unwrap();
        commit_uniform_batch(&mut exact_ledger, 1, 4, &mut exact_resources).unwrap();
        assert_eq!(exact_resources.layout_work_used(), 12);

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 11)
            .unwrap();
        let mut below_resources = ResourceContext::new(below);
        let mut below_ledger = SequenceExtentLedger::default();
        commit_uniform_batch(&mut below_ledger, 2, 4, &mut below_resources).unwrap();
        let error = reserve_uniform_batch(&below_ledger, 1, 4, &mut below_resources).unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 12
                    && details.max == 11
        ));
    }

    #[test]
    fn combined_grid_rejects_before_the_next_batch_is_materialized() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 100)
            .unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut ledger = SequenceExtentLedger::default();
        commit_uniform_batch(&mut ledger, 9, 10, &mut resources).unwrap();

        let materialized = Cell::new(false);
        let result = (|| {
            let reservation = reserve_uniform_batch(&ledger, 9, 10, &mut resources)?;
            materialized.set(true);
            let lines = uniform_lines(9, 10, &resources)?;
            reservation.commit(&mut ledger, &lines, &resources)
        })();

        let error = result.unwrap_err();
        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 180
                    && details.max == 100
        ));
    }

    #[test]
    fn extent_replay_observes_layout_cancellation_before_committing_the_batch() {
        const ROWS: usize = 65;
        let policy = AsciiResourcePolicy::default();
        let resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(7)
            .expect("the pre-existing work debit should fit");
        resources
            .charge_document_cells(3)
            .expect("the pre-existing document debit should fit");
        let mut ledger = SequenceExtentLedger::default();
        let batch = SequenceBatchExtent::uniform(ROWS, 1, 1, &resources)
            .expect("the replay extent should fit");
        let line_resources = ResourceContext::new(policy);
        let lines = uniform_lines(ROWS, 1, &line_resources).expect("the replay rows should fit");

        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let controlled_resources = execution.resource_context(&resources, OperationPhase::Layout);
        let mut replay_checkpoints =
            SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
        let error = controlled_resources
            .transaction(|controlled_resources| {
                let mut controlled_resources = controlled_resources.clone();
                let reservation =
                    ledger.reserve(batch, &mut controlled_resources, &replay_checkpoints)?;
                control.cancel_after_checkpoints(1);
                reservation.commit_with_checkpoints(
                    &mut ledger,
                    &lines,
                    &controlled_resources,
                    &mut replay_checkpoints,
                )
            })
            .expect_err("the second extent pass should stop at its next cadence checkpoint");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(ledger.height(), 0);
        assert_eq!(resources.layout_work_used(), 7);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn blank_line_initialization_cancels_between_paint_chunks() {
        let policy = AsciiResourcePolicy::default();
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let execution = AsciiExecution::new(&control, &policy);
        let checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let error = blank_line_with_checkpoints(
            129,
            TerminalWidthProfile::Unicode,
            &resources,
            &checkpoints,
        )
        .expect_err("the second blank-paint chunk should observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn styled_text_replay_cancels_after_painting_the_first_chunk() {
        const WIDTH: usize = 128;
        let policy = AsciiResourcePolicy::default();
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
        let mut line = blank_line_with_checkpoints(
            WIDTH,
            TerminalWidthProfile::Unicode,
            &resources,
            &checkpoints,
        )
        .expect("the paint target should fit");
        let text = "A".repeat(WIDTH);
        let callback_count = Cell::new(0usize);

        let error = line
            .try_write_text_role_with_checkpoint(0, &text, AsciiColorRole::Text, &resources, || {
                let next = callback_count.get() + 1;
                callback_count.set(next);
                if next == WIDTH + 65 {
                    control.cancel();
                }
                checkpoints.tick()
            })
            .expect_err("the second write-replay chunk should observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(line.get(0), Some('A'));
        assert!(
            (1..WIDTH).any(|index| line.get(index) == Some(' ')),
            "cancellation should stop before the complete line is painted"
        );
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn next_batch_work_charge_is_independent_of_retained_history() {
        let short = next_batch_work_delta(1);
        let long = next_batch_work_delta(1_024);

        assert_eq!(short, 7);
        assert_eq!(long, short);
    }

    fn next_batch_work_delta(history_rows: usize) -> usize {
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut ledger = SequenceExtentLedger::default();
        commit_uniform_batch(&mut ledger, history_rows, 4, &mut resources).unwrap();
        let before = resources.layout_work_used();
        commit_uniform_batch(&mut ledger, 1, 7, &mut resources).unwrap();
        resources.layout_work_used() - before
    }

    fn reserve_uniform_batch(
        ledger: &SequenceExtentLedger,
        height: usize,
        width: usize,
        resources: &mut ResourceContext,
    ) -> Result<SequenceExtentReservation> {
        let batch = SequenceBatchExtent::uniform(height, width, width, resources)?;
        let policy = resources.policy();
        let checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&policy),
            OperationPhase::Layout,
        );
        ledger.reserve(batch, resources, &checkpoints)
    }

    fn commit_uniform_batch(
        ledger: &mut SequenceExtentLedger,
        height: usize,
        width: usize,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        let reservation = reserve_uniform_batch(ledger, height, width, resources)?;
        let lines = uniform_lines(height, width, resources)?;
        reservation.commit(ledger, &lines, resources)
    }

    fn uniform_lines(
        height: usize,
        width: usize,
        resources: &ResourceContext,
    ) -> Result<Vec<SequenceLine>> {
        (0..height)
            .map(|_| blank_line(width, TerminalWidthProfile::Unicode, resources))
            .collect()
    }
}
