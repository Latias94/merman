use super::document::{document_allocation_error, try_write_fmt};
use super::framing::QuotedTerminalTextEvent;
use super::normalization::{
    NormalizedSegment, NormalizedSegmentKind, visible_escape, visit_normalized_segments,
};
use super::width::{MeasuredGrapheme, grapheme_display_width, measured_graphemes};
use crate::Result;
use crate::color::AsciiColorMode;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

const WRAPPED_CHECKPOINT_INTERVAL: usize = 64;

/// Plans or materializes one wrapped structured-text row through the same wrapping state machine.
///
/// The planning pass retains no authored text. A successful plan is replayed into local rows only
/// after its complete work, document-cell, output-byte, and grapheme surface has been admitted.
pub(crate) struct BudgetedWrappedText<'prefix> {
    resources: ResourceContext,
    width_profile: TerminalWidthProfile,
    color_mode: AsciiColorMode,
    first_prefix: &'prefix str,
    continuation_prefix: &'prefix str,
    available: usize,
    word_width: usize,
    long_word: bool,
    current_width: usize,
    emitted: bool,
    metrics: WrappedPassMetrics,
    base_layout_work: usize,
    fixed_layout_work: usize,
    base_document_cells: usize,
    base_output_bytes: usize,
    storage: WrappedStorage,
    #[cfg(test)]
    retain_probe: Option<std::rc::Rc<std::cell::Cell<usize>>>,
}

#[derive(Clone)]
pub(super) struct WrappedPassConfig<'prefix> {
    pub(super) resources: ResourceContext,
    pub(super) width_profile: TerminalWidthProfile,
    pub(super) color_mode: AsciiColorMode,
    pub(super) first_prefix: &'prefix str,
    pub(super) continuation_prefix: &'prefix str,
    pub(super) available: usize,
    pub(super) base_layout_work: usize,
    pub(super) fixed_layout_work: usize,
    pub(super) base_document_cells: usize,
    pub(super) base_output_bytes: usize,
    #[cfg(test)]
    pub(super) retain_probe: Option<std::rc::Rc<std::cell::Cell<usize>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WrappedPassMetrics {
    pub(super) layout_work: usize,
    pub(super) document_cells: usize,
    pub(super) output_bytes: usize,
    pub(super) rows: usize,
    replay_fingerprint: u64,
}

impl Default for WrappedPassMetrics {
    fn default() -> Self {
        Self {
            layout_work: 0,
            document_cells: 0,
            output_bytes: 0,
            rows: 0,
            replay_fingerprint: FNV_OFFSET_BASIS,
        }
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy)]
#[repr(u8)]
enum ReplayEventKind {
    Prefix = 1,
    Row = 2,
    Text = 3,
}

enum WrappedStorage {
    Measure,
    Materialize {
        word: String,
        current: String,
        rows: Vec<String>,
        expected_rows: usize,
    },
}

pub(super) struct FinishedWrappedPass {
    pub(super) metrics: WrappedPassMetrics,
    pub(super) rows: Vec<String>,
}

impl FinishedWrappedPass {
    pub(super) fn verify_metrics(&self, expected: WrappedPassMetrics) -> Result<()> {
        if self.metrics != expected {
            return Err(invalid_wrapped_text_plan());
        }
        Ok(())
    }
}

impl<'prefix> BudgetedWrappedText<'prefix> {
    pub(super) fn measure(config: WrappedPassConfig<'prefix>) -> Self {
        Self::new(config, WrappedStorage::Measure)
    }

    pub(super) fn materialize(
        config: WrappedPassConfig<'prefix>,
        expected_rows: usize,
    ) -> Result<Self> {
        config.resources.checkpoint()?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(expected_rows)
            .map_err(|_| document_allocation_error())?;
        Ok(Self::new(
            config,
            WrappedStorage::Materialize {
                word: String::new(),
                current: String::new(),
                rows,
                expected_rows,
            },
        ))
    }

    fn new(config: WrappedPassConfig<'prefix>, storage: WrappedStorage) -> Self {
        Self {
            resources: config.resources,
            width_profile: config.width_profile,
            color_mode: config.color_mode,
            first_prefix: config.first_prefix,
            continuation_prefix: config.continuation_prefix,
            available: config.available,
            word_width: 0,
            long_word: false,
            current_width: 0,
            emitted: false,
            metrics: WrappedPassMetrics::default(),
            base_layout_work: config.base_layout_work,
            fixed_layout_work: config.fixed_layout_work,
            base_document_cells: config.base_document_cells,
            base_output_bytes: config.base_output_bytes,
            storage,
            #[cfg(test)]
            retain_probe: config.retain_probe,
        }
    }

    pub(super) fn admit_first_prefix(&mut self, separator: usize) -> Result<()> {
        self.admit_prefix_content(self.first_prefix, separator)
    }

    pub(crate) fn push_str(&mut self, value: &str) -> Result<()> {
        visit_normalized_segments(value, |segment| {
            self.budget_layout_segment(segment)?;
            match segment.kind {
                NormalizedSegmentKind::LineBreak => self.finish_paragraph(),
                NormalizedSegmentKind::Grapheme(grapheme) => self.push_normalized_text(grapheme),
                NormalizedSegmentKind::VisibleEscape(ch) => {
                    let mut buffer = [0u8; 10];
                    let escape = visible_escape(ch, &mut buffer);
                    self.push_atomic_word_fragment_with_width(escape, escape.len())
                }
            }
        })
    }

    pub(crate) fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        try_write_fmt(arguments, |value| self.push_str(value))
    }

    pub(crate) fn push_quoted_text(&mut self, value: &str) -> Result<()> {
        super::framing::visit_quoted_terminal_text_with(value, |event| match event {
            QuotedTerminalTextEvent::SourceGrapheme(grapheme) => {
                self.charge_layout_work(1)?;
                self.check_grapheme_bytes(grapheme.len())
            }
            QuotedTerminalTextEvent::OutputFragment(fragment) => {
                self.push_exact_normalized_text(fragment)
            }
        })
    }

    fn push_exact_normalized_text(&mut self, value: &str) -> Result<()> {
        let mut width = 0usize;
        let mut encoded_bytes = 0usize;
        visit_normalized_segments(value, |segment| {
            self.budget_layout_segment(segment)?;
            let mut buffer = [0u8; 10];
            let text = match segment.kind {
                NormalizedSegmentKind::LineBreak => "\\n",
                _ => segment.text(&mut buffer),
            };
            encoded_bytes = encoded_bytes
                .checked_add(super::encode::encoded_text_len_with_resources(
                    text,
                    self.color_mode,
                    &self.resources,
                )?)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxOutputBytes)
                })?;
            let segment_width = match segment.kind {
                NormalizedSegmentKind::LineBreak => 2,
                _ => segment.display_width(self.width_profile),
            };
            width = width.checked_add(segment_width).ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
            Ok::<(), AsciiError>(())
        })?;

        self.prepare_atomic_word_fragment(width)?;
        self.charge_document_cells(width)?;
        self.charge_output_bytes(encoded_bytes)?;
        visit_normalized_segments(value, |segment| {
            self.charge_layout_work(segment.layout_work())?;
            let mut buffer = [0u8; 10];
            let text = match segment.kind {
                NormalizedSegmentKind::LineBreak => "\\n",
                _ => segment.text(&mut buffer),
            };
            self.push_word_text(text)
        })?;
        self.word_width = self.word_width.checked_add(width).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn push_normalized_text(&mut self, value: &str) -> Result<()> {
        let mut word_start = 0usize;
        for (iteration, (index, ch)) in value.char_indices().enumerate() {
            self.checkpoint_loop(iteration)?;
            if !ch.is_whitespace() {
                continue;
            }
            self.push_word_fragment(&value[word_start..index])?;
            self.finish_word()?;
            word_start = index + ch.len_utf8();
        }
        self.push_word_fragment(&value[word_start..])
    }

    fn push_word_fragment(&mut self, value: &str) -> Result<()> {
        for grapheme in measured_graphemes(value, self.width_profile) {
            self.push_word_grapheme(grapheme)?;
        }
        Ok(())
    }

    fn push_atomic_word_fragment_with_width(&mut self, value: &str, width: usize) -> Result<()> {
        self.prepare_atomic_word_fragment(width)?;
        self.charge_document_cells(width)?;
        self.charge_output_text(value)?;
        self.push_word_text(value)?;
        self.word_width = self.word_width.checked_add(width).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn prepare_atomic_word_fragment(&mut self, width: usize) -> Result<()> {
        let prospective = self.word_width.checked_add(width).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        if prospective > self.available && self.word_width > 0 {
            if !self.long_word {
                if self.current_width > 0 {
                    self.emit_current()?;
                }
                self.long_word = true;
            }
            self.emit_word_chunk()?;
        }

        if self.word_width == 0 && width > self.available {
            if self.current_width > 0 {
                self.emit_current()?;
            }
            self.long_word = true;
        }
        Ok(())
    }

    fn push_word_grapheme(&mut self, grapheme: MeasuredGrapheme<'_>) -> Result<()> {
        self.charge_document_cells(grapheme.width())?;
        let prospective = self
            .word_width
            .checked_add(grapheme.width())
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;

        if prospective > self.available && self.word_width > 0 {
            if !self.long_word {
                if self.current_width > 0 {
                    self.emit_current()?;
                }
                self.long_word = true;
            }
            self.emit_word_chunk()?;
        }

        self.charge_output_text(grapheme.text())?;
        self.push_word_text(grapheme.text())?;
        self.word_width = self
            .word_width
            .checked_add(grapheme.width())
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn finish_word(&mut self) -> Result<()> {
        if self.word_width == 0 {
            self.long_word = false;
            return Ok(());
        }
        if self.long_word {
            self.emit_word_chunk()?;
            self.long_word = false;
            return Ok(());
        }

        let separator = usize::from(self.current_width > 0);
        let prospective = self
            .current_width
            .checked_add(separator)
            .and_then(|width| width.checked_add(self.word_width))
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        if prospective > self.available && self.current_width > 0 {
            self.emit_current()?;
        }
        if self.current_width > 0 {
            self.charge_document_cells(1)?;
            self.charge_output_text(" ")?;
            self.push_current_text(" ")?;
            self.current_width = self.current_width.checked_add(1).ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        }

        self.move_word_to_current()?;
        self.current_width = self
            .current_width
            .checked_add(self.word_width)
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        self.word_width = 0;
        Ok(())
    }

    fn emit_word_chunk(&mut self) -> Result<()> {
        self.resources.checkpoint()?;
        if self.word_width == 0 {
            return Ok(());
        }
        let content = self.take_word();
        self.word_width = 0;
        self.emit_row(content)
    }

    fn emit_current(&mut self) -> Result<()> {
        self.resources.checkpoint()?;
        if self.current_width == 0 {
            return Ok(());
        }
        let content = self.take_current();
        self.current_width = 0;
        self.emit_row(content)
    }

    fn emit_row(&mut self, mut content: String) -> Result<()> {
        self.resources.checkpoint()?;
        let prefix = if self.emitted {
            self.continuation_prefix
        } else {
            self.first_prefix
        };
        if self.emitted {
            self.admit_prefix(prefix, 1)?;
        }
        #[cfg(test)]
        let retain_probe = self.retain_probe.clone();
        if let WrappedStorage::Materialize {
            rows,
            expected_rows,
            ..
        } = &mut self.storage
        {
            if rows.len() >= *expected_rows {
                return Err(invalid_wrapped_text_plan());
            }
            content
                .try_reserve(prefix.len())
                .map_err(|_| document_allocation_error())?;
            #[cfg(test)]
            if let Some(probe) = retain_probe {
                probe.set(probe.get() + 1);
            }
            content.insert_str(0, prefix);
            rows.push(content);
        }
        self.note_replay_event(ReplayEventKind::Row, &[])?;
        self.metrics.rows = self.metrics.rows.checked_add(1).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        self.emitted = true;
        Ok(())
    }

    fn admit_prefix(&mut self, prefix: &str, separator: usize) -> Result<()> {
        self.charge_layout_work(1)?;
        self.admit_prefix_content(prefix, separator)
    }

    fn admit_prefix_content(&mut self, prefix: &str, separator: usize) -> Result<()> {
        for grapheme in prefix.graphemes(true) {
            self.charge_layout_work(1)?;
            self.check_grapheme_bytes(grapheme.len())?;
            self.charge_document_cells(grapheme_display_width(grapheme, self.width_profile))?;
        }
        let prefix_bytes = super::encode::encoded_text_len_with_resources(
            prefix,
            self.color_mode,
            &self.resources,
        )?;
        self.charge_output_bytes(separator.checked_add(prefix_bytes).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxOutputBytes)
        })?)?;
        self.note_replay_event(ReplayEventKind::Prefix, prefix.as_bytes())?;
        Ok(())
    }

    fn finish_paragraph(&mut self) -> Result<()> {
        self.finish_word()?;
        self.emit_current()
    }

    pub(super) fn finish(mut self) -> Result<FinishedWrappedPass> {
        self.resources.checkpoint()?;
        self.finish_paragraph()?;
        if !self.emitted {
            self.emit_row(String::new())?;
        }
        let rows = match self.storage {
            WrappedStorage::Measure => Vec::new(),
            WrappedStorage::Materialize {
                rows,
                expected_rows,
                ..
            } => {
                if rows.len() != expected_rows {
                    return Err(invalid_wrapped_text_plan());
                }
                rows
            }
        };
        Ok(FinishedWrappedPass {
            metrics: self.metrics,
            rows,
        })
    }

    fn budget_layout_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<()> {
        self.check_grapheme_bytes(segment.source_grapheme_bytes)?;
        match segment.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => {
                self.check_grapheme_bytes(grapheme.len())?
            }
            NormalizedSegmentKind::VisibleEscape(_) => self.check_grapheme_bytes(1)?,
            NormalizedSegmentKind::LineBreak => {}
        }
        self.charge_layout_work(segment.layout_work())
    }

    pub(super) fn charge_layout_work(&mut self, delta: usize) -> Result<()> {
        self.resources.checkpoint()?;
        self.metrics.layout_work =
            self.metrics.layout_work.checked_add(delta).ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        let replay_work = self.metrics.layout_work.checked_mul(2).ok_or_else(|| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        self.resources.check(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.base_layout_work
                .checked_add(self.fixed_layout_work)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
                })?
                .checked_add(replay_work)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
                })?,
        )
    }

    fn checkpoint_loop(&self, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(WRAPPED_CHECKPOINT_INTERVAL) {
            self.resources.checkpoint()?;
        }
        Ok(())
    }

    fn charge_document_cells(&mut self, delta: usize) -> Result<()> {
        self.resources.checkpoint()?;
        self.metrics.document_cells =
            self.metrics
                .document_cells
                .checked_add(delta)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxDocumentCells)
                })?;
        self.resources.check(
            AsciiResourceLimitId::MaxDocumentCells,
            self.base_document_cells
                .checked_add(self.metrics.document_cells)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxDocumentCells)
                })?,
        )
    }

    fn charge_output_text(&mut self, value: &str) -> Result<()> {
        self.charge_output_bytes(super::encode::encoded_text_len_with_resources(
            value,
            self.color_mode,
            &self.resources,
        )?)
    }

    fn charge_output_bytes(&mut self, delta: usize) -> Result<()> {
        self.resources.checkpoint()?;
        self.metrics.output_bytes =
            self.metrics
                .output_bytes
                .checked_add(delta)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxOutputBytes)
                })?;
        self.resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.base_output_bytes
                .checked_add(self.metrics.output_bytes)
                .ok_or_else(|| {
                    self.resources
                        .overflow(AsciiResourceLimitId::MaxOutputBytes)
                })?,
        )
    }

    fn check_grapheme_bytes(&self, bytes: usize) -> Result<()> {
        self.resources
            .check(AsciiResourceLimitId::MaxGraphemeBytes, bytes)
    }

    fn push_word_text(&mut self, value: &str) -> Result<()> {
        self.note_replay_event(ReplayEventKind::Text, value.as_bytes())?;
        self.resources.checkpoint()?;
        #[cfg(test)]
        let retain_probe = self.retain_probe.clone();
        if let WrappedStorage::Materialize { word, .. } = &mut self.storage {
            #[cfg(test)]
            if let Some(probe) = retain_probe {
                probe.set(probe.get() + 1);
            }
            try_push_document_str(word, value)?;
        }
        Ok(())
    }

    fn push_current_text(&mut self, value: &str) -> Result<()> {
        self.note_replay_event(ReplayEventKind::Text, value.as_bytes())?;
        self.resources.checkpoint()?;
        #[cfg(test)]
        let retain_probe = self.retain_probe.clone();
        if let WrappedStorage::Materialize { current, .. } = &mut self.storage {
            #[cfg(test)]
            if let Some(probe) = retain_probe {
                probe.set(probe.get() + 1);
            }
            try_push_document_str(current, value)?;
        }
        Ok(())
    }

    fn note_replay_event(&mut self, kind: ReplayEventKind, value: &[u8]) -> Result<()> {
        self.resources.checkpoint()?;
        let value_len = u64::try_from(value.len()).map_err(|_| {
            self.resources
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        let work = 1usize
            .checked_add(std::mem::size_of::<u64>())
            .and_then(|work| work.checked_add(value.len()))
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        self.charge_layout_work(work)?;

        let mut hash = self.metrics.replay_fingerprint;
        for (index, byte) in std::iter::once(kind as u8)
            .chain(value_len.to_le_bytes())
            .chain(value.iter().copied())
            .enumerate()
        {
            self.checkpoint_loop(index)?;
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        self.metrics.replay_fingerprint = hash;
        Ok(())
    }

    fn move_word_to_current(&mut self) -> Result<()> {
        self.resources.checkpoint()?;
        if let WrappedStorage::Materialize { word, current, .. } = &mut self.storage {
            current
                .try_reserve(word.len())
                .map_err(|_| document_allocation_error())?;
            current.push_str(word);
            word.clear();
        }
        Ok(())
    }

    fn take_word(&mut self) -> String {
        match &mut self.storage {
            WrappedStorage::Measure => String::new(),
            WrappedStorage::Materialize { word, .. } => std::mem::take(word),
        }
    }

    fn take_current(&mut self) -> String {
        match &mut self.storage {
            WrappedStorage::Measure => String::new(),
            WrappedStorage::Materialize { current, .. } => std::mem::take(current),
        }
    }
}

pub(super) fn measure_wrapped_prefix_widths(
    first_prefix: &str,
    continuation_prefix: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    resources.checkpoint()?;
    let base_layout_work = resources.layout_work_used();
    let mut work = 0usize;
    let mut width = 0usize;
    for prefix in [first_prefix, continuation_prefix] {
        let mut prefix_width = 0usize;
        for grapheme in measured_graphemes(prefix, width_profile) {
            resources.checkpoint()?;
            work = work
                .checked_add(1)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
            let actual = base_layout_work
                .checked_add(work)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
            resources.check(AsciiResourceLimitId::MaxLayoutWorkUnits, actual)?;
            prefix_width = prefix_width
                .checked_add(grapheme.width())
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        }
        width = width.max(prefix_width);
    }
    Ok((width, work))
}

fn try_push_document_str(output: &mut String, value: &str) -> Result<()> {
    output
        .try_reserve(value.len())
        .map_err(|_| document_allocation_error())?;
    output.push_str(value);
    Ok(())
}

fn invalid_wrapped_text_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "structured_text",
        feature: "wrapped text replay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::{OperationControl, OperationPhase};
    use std::cell::Cell;
    use std::rc::Rc;

    fn pass_config(
        resources: ResourceContext,
        retain_probe: Rc<Cell<usize>>,
    ) -> WrappedPassConfig<'static> {
        let base_layout_work = resources.layout_work_used();
        let base_document_cells = resources.document_cells_used();
        WrappedPassConfig {
            resources,
            width_profile: TerminalWidthProfile::Unicode,
            color_mode: AsciiColorMode::Plain,
            first_prefix: "",
            continuation_prefix: "",
            available: 80,
            base_layout_work,
            fixed_layout_work: 0,
            base_document_cells,
            base_output_bytes: 0,
            retain_probe: Some(retain_probe),
        }
    }

    fn resources_with_prior_usage() -> ResourceContext {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());
        resources
            .charge_usage(2, 3)
            .expect("prior structured-text usage should fit");
        resources
    }

    #[test]
    fn wrapped_prefix_counting_prioritizes_cancellation_without_debiting_ledgers() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid limit");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let controlled = resources.controlled(control, OperationPhase::Emit);

        let error = measure_wrapped_prefix_widths(
            "prefix",
            "continuation",
            TerminalWidthProfile::Unicode,
            &controlled,
        )
        .expect_err("prefix counting should observe cancellation before the next work ceiling");

        assert!(matches!(
            error,
            AsciiError::Cancelled(details) if details.phase == OperationPhase::Emit
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn wrapped_measure_replay_observes_cancellation_without_debiting_ledgers() {
        let resources = resources_with_prior_usage();
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Emit);
        let retain_probe = Rc::new(Cell::new(0));
        let mut planner = BudgetedWrappedText::measure(pass_config(controlled, retain_probe));
        planner
            .charge_layout_work(1)
            .expect("initial virtual work should fit");
        planner
            .admit_first_prefix(0)
            .expect("empty prefix should fit");

        control.cancel_after_checkpoints(8);
        let error = planner
            .push_str(&"measure".repeat(128))
            .expect_err("measure replay should observe scheduled cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(details) if details.phase == OperationPhase::Emit
        ));
        assert_eq!(resources.layout_work_used(), 2);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn wrapped_materialize_replay_cancellation_discards_speculative_debits() {
        let resources = resources_with_prior_usage();
        let planning_control = OperationControl::new();
        let planning_resources = resources.controlled(planning_control, OperationPhase::Emit);
        let retain_probe = Rc::new(Cell::new(0));
        let mut planner =
            BudgetedWrappedText::measure(pass_config(planning_resources, Rc::clone(&retain_probe)));
        planner
            .charge_layout_work(1)
            .expect("initial virtual work should fit");
        planner
            .admit_first_prefix(0)
            .expect("empty prefix should fit");
        planner
            .push_str("retainedmore")
            .expect("measurement should succeed");
        let plan = planner.finish().expect("measurement should finish").metrics;

        let materialize_control = OperationControl::new();
        let materialize_resources =
            resources.controlled(materialize_control.clone(), OperationPhase::Emit);
        let mut materializer = BudgetedWrappedText::materialize(
            pass_config(materialize_resources, Rc::clone(&retain_probe)),
            plan.rows,
        )
        .expect("materializer allocation should succeed");
        materializer
            .charge_layout_work(1)
            .expect("initial materialize work should fit");
        materializer
            .admit_first_prefix(0)
            .expect("empty prefix should fit");
        materializer
            .push_str("retained")
            .expect("the first replay fragment should be retained locally");
        assert!(retain_probe.get() > 0);

        materialize_control.cancel();
        let error = materializer
            .push_str("more")
            .expect_err("materialize replay should observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(details) if details.phase == OperationPhase::Emit
        ));
        assert_eq!(resources.layout_work_used(), 2);
        assert_eq!(resources.document_cells_used(), 3);
    }
}
