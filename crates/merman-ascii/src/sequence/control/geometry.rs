use super::super::SequenceCheckpointCursor;
use super::super::chars::SequenceChars;
use super::super::layout::SequenceLayout;
use super::super::lifeline::build_lifeline_line;
use super::super::text::{SequenceLine, SequenceRowFootprint};
use super::super::tree::SequenceParticipantSpan;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

const PARTICIPANT_FRAME_MARGIN: usize = 2;
const CONTENT_FRAME_MARGIN: usize = 1;
const NESTED_FRAME_MARGIN: usize = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::sequence) struct SequenceControlBoundaryState {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
}

impl SequenceControlBoundaryState {
    pub(in crate::sequence) fn try_capture(
        active_counts: &[usize],
        visible_actors: &[bool],
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        let width = active_counts.len().max(visible_actors.len());
        resources.grid_extent(width, 1)?;
        checkpoints.before_charge()?;
        resources.charge_layout_work(
            active_counts
                .len()
                .checked_add(visible_actors.len())
                .ok_or_else(|| resources.work_overflow())?,
        )?;
        Ok(Self {
            active_counts: try_clone_slice(active_counts)?,
            visible_actors: try_clone_slice(visible_actors)?,
        })
    }

    pub(super) fn render_lifeline(
        &self,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceLine> {
        build_lifeline_line(
            layout,
            chars,
            &self.active_counts,
            &self.visible_actors,
            resources,
            checkpoints,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceFrameBounds {
    left: usize,
    right: usize,
}

impl SequenceFrameBounds {
    pub(super) fn from_participants(
        span: SequenceParticipantSpan,
        layout: &SequenceLayout,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let first = layout
            .participant_centers
            .get(span.first)
            .copied()
            .ok_or_else(invalid_control_geometry)?;
        let last = layout
            .participant_centers
            .get(span.last)
            .copied()
            .ok_or_else(invalid_control_geometry)?;
        Ok(Self {
            left: first.saturating_sub(PARTICIPANT_FRAME_MARGIN),
            right: resources.checked_grid_add(last, PARTICIPANT_FRAME_MARGIN)?,
        })
    }

    pub(super) fn full_width(width: usize) -> Result<Self> {
        let right = width.checked_sub(1).ok_or_else(invalid_control_geometry)?;
        Ok(Self { left: 0, right })
    }

    pub(super) const fn left(self) -> usize {
        self.left
    }

    pub(super) const fn right(self) -> usize {
        self.right
    }

    pub(super) fn width(self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_grid_add(self.right.abs_diff(self.left), 1)
    }

    pub(super) fn right_exclusive(self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_grid_add(self.right, 1)
    }

    #[cfg(test)]
    pub(super) fn include_line_content(
        &mut self,
        line: &SequenceLine,
        participant_span: SequenceParticipantSpan,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(line.len().max(1))?;
        for index in 0..line.len() {
            let Some(ch) = line.get(index) else {
                continue;
            };
            if ch == ' ' || is_unrelated_lifeline(index, ch, participant_span, layout, chars) {
                continue;
            }
            self.left = self.left.min(index.saturating_sub(CONTENT_FRAME_MARGIN));
            self.right = self
                .right
                .max(resources.checked_grid_add(index, CONTENT_FRAME_MARGIN)?);
        }
        Ok(())
    }

    pub(super) fn include_footprint_content(
        &mut self,
        footprint: SequenceRowFootprint,
        resources: &ResourceContext,
    ) -> Result<()> {
        let Some(content) = footprint.content() else {
            return Ok(());
        };
        self.left = self
            .left
            .min(content.left().saturating_sub(CONTENT_FRAME_MARGIN));
        self.right = self
            .right
            .max(resources.checked_grid_add(content.right(), CONTENT_FRAME_MARGIN)?);
        Ok(())
    }

    pub(super) fn include_child(&mut self, child: Self, resources: &ResourceContext) -> Result<()> {
        self.left = self
            .left
            .min(child.left.saturating_sub(NESTED_FRAME_MARGIN));
        self.right = self
            .right
            .max(resources.checked_grid_add(child.right, NESTED_FRAME_MARGIN)?);
        Ok(())
    }

    pub(super) fn ensure_width(
        &mut self,
        required_width: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        let width = self.width(resources)?;
        if width < required_width {
            self.right = resources.checked_grid_add(self.right, required_width - width)?;
        }
        Ok(())
    }

    pub(super) fn shift_right(&mut self, offset: usize, resources: &ResourceContext) -> Result<()> {
        self.left = resources.checked_grid_add(self.left, offset)?;
        self.right = resources.checked_grid_add(self.right, offset)?;
        Ok(())
    }
}

#[cfg(test)]
fn is_unrelated_lifeline(
    index: usize,
    ch: char,
    participant_span: SequenceParticipantSpan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> bool {
    if ch != chars.vertical && ch != chars.active_vertical {
        return false;
    }
    layout
        .participant_centers
        .binary_search(&index)
        .is_ok_and(|actor| !participant_span.contains(actor))
}

fn try_clone_slice<T: Copy>(source: &[T]) -> Result<Vec<T>> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| allocation_failed())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn invalid_control_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "control frame geometry",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::OperationPhase;

    #[test]
    fn boundary_snapshot_accepts_exact_work_and_rejects_max_minus_one() {
        let mut exact = resources_with_work_limit(4);
        let exact_policy = exact.policy();
        let exact_checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&exact_policy),
            OperationPhase::Layout,
        );
        let snapshot = SequenceControlBoundaryState::try_capture(
            &[0, 1],
            &[true, false],
            &mut exact,
            &exact_checkpoints,
        )
        .expect("the exact boundary snapshot work should be admitted");
        assert_eq!(snapshot.active_counts, [0, 1]);
        assert_eq!(snapshot.visible_actors, [true, false]);

        let mut below = resources_with_work_limit(3);
        let below_policy = below.policy();
        let below_checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&below_policy),
            OperationPhase::Layout,
        );
        let error = SequenceControlBoundaryState::try_capture(
            &[0, 1],
            &[true, false],
            &mut below,
            &below_checkpoints,
        )
        .expect_err("the boundary snapshot should reject max minus one");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 4
                    && details.max == 3
        ));
    }

    #[test]
    fn local_frame_scan_ignores_unrelated_lifelines_at_the_exact_work_boundary() {
        let layout = test_layout();
        let chars = SequenceChars::for_options(&AsciiRenderOptions::ascii());
        let line = SequenceLine::plain_text_with_profile("|   |", TerminalWidthProfile::Unicode);
        let span = SequenceParticipantSpan::single(1);

        let mut exact = resources_with_work_limit(5);
        let mut bounds = SequenceFrameBounds::from_participants(span, &layout, &exact)
            .expect("participant bounds should fit");
        bounds
            .include_line_content(&line, span, &layout, &chars, &mut exact)
            .expect("the exact five-cell scan should be admitted");
        assert_eq!(bounds.left(), 2);
        assert_eq!(bounds.right(), 6);

        let mut below = resources_with_work_limit(4);
        let mut bounds = SequenceFrameBounds::from_participants(span, &layout, &below)
            .expect("participant bounds should fit");
        let error = bounds
            .include_line_content(&line, span, &layout, &chars, &mut below)
            .expect_err("the five-cell scan should reject max minus one");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 5
                    && details.max == 4
        ));
    }

    fn resources_with_work_limit(limit: usize) -> ResourceContext {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
            .expect("the layout-work override should be valid");
        ResourceContext::new(policy)
    }

    fn test_layout() -> SequenceLayout {
        SequenceLayout {
            participant_widths: vec![3, 3],
            participant_centers: vec![0, 4],
            total_width: 5,
            message_spacing: 1,
            self_message_width: 4,
            width_profile: TerminalWidthProfile::Unicode,
        }
    }
}
