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
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

/// Plans or materializes one wrapped structured-text row through the same wrapping state machine.
///
/// The planning pass retains no authored text. A successful plan is replayed into local rows only
/// after its complete work, document-cell, output-byte, and grapheme surface has been admitted.
pub(crate) struct BudgetedWrappedText<'prefix> {
    policy: AsciiResourcePolicy,
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
    pub(super) policy: AsciiResourcePolicy,
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
            policy: config.policy,
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
                .checked_add(super::encode::encoded_text_len(
                    text,
                    self.color_mode,
                    self.policy,
                )?)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
            let segment_width = match segment.kind {
                NormalizedSegmentKind::LineBreak => 2,
                _ => segment.display_width(self.width_profile),
            };
            width = width
                .checked_add(segment_width)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
        self.word_width = self
            .word_width
            .checked_add(width)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn push_normalized_text(&mut self, value: &str) -> Result<()> {
        let mut word_start = 0usize;
        for (index, ch) in value.char_indices() {
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
        self.word_width = self
            .word_width
            .checked_add(width)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn prepare_atomic_word_fragment(&mut self, width: usize) -> Result<()> {
        let prospective = self
            .word_width
            .checked_add(width)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;

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
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        if prospective > self.available && self.current_width > 0 {
            self.emit_current()?;
        }
        if self.current_width > 0 {
            self.charge_document_cells(1)?;
            self.charge_output_text(" ")?;
            self.push_current_text(" ")?;
            self.current_width = self
                .current_width
                .checked_add(1)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        }

        self.move_word_to_current()?;
        self.current_width = self
            .current_width
            .checked_add(self.word_width)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.word_width = 0;
        Ok(())
    }

    fn emit_word_chunk(&mut self) -> Result<()> {
        if self.word_width == 0 {
            return Ok(());
        }
        let content = self.take_word();
        self.word_width = 0;
        self.emit_row(content)
    }

    fn emit_current(&mut self) -> Result<()> {
        if self.current_width == 0 {
            return Ok(());
        }
        let content = self.take_current();
        self.current_width = 0;
        self.emit_row(content)
    }

    fn emit_row(&mut self, mut content: String) -> Result<()> {
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
        self.metrics.rows = self
            .metrics
            .rows
            .checked_add(1)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
        let prefix_bytes = super::encode::encoded_text_len(prefix, self.color_mode, self.policy)?;
        self.charge_output_bytes(
            separator
                .checked_add(prefix_bytes)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?,
        )?;
        self.note_replay_event(ReplayEventKind::Prefix, prefix.as_bytes())?;
        Ok(())
    }

    fn finish_paragraph(&mut self) -> Result<()> {
        self.finish_word()?;
        self.emit_current()
    }

    pub(super) fn finish(mut self) -> Result<FinishedWrappedPass> {
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
        self.metrics.layout_work =
            self.metrics.layout_work.checked_add(delta).ok_or_else(|| {
                self.policy
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        let replay_work = self.metrics.layout_work.checked_mul(2).ok_or_else(|| {
            self.policy
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        self.policy.check(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            self.base_layout_work
                .checked_add(self.fixed_layout_work)
                .ok_or_else(|| {
                    self.policy
                        .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
                })?
                .checked_add(replay_work)
                .ok_or_else(|| {
                    self.policy
                        .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
                })?,
        )
    }

    fn charge_document_cells(&mut self, delta: usize) -> Result<()> {
        self.metrics.document_cells = self
            .metrics
            .document_cells
            .checked_add(delta)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.policy.check(
            AsciiResourceLimitId::MaxDocumentCells,
            self.base_document_cells
                .checked_add(self.metrics.document_cells)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?,
        )
    }

    fn charge_output_text(&mut self, value: &str) -> Result<()> {
        self.charge_output_bytes(super::encode::encoded_text_len(
            value,
            self.color_mode,
            self.policy,
        )?)
    }

    fn charge_output_bytes(&mut self, delta: usize) -> Result<()> {
        self.metrics.output_bytes = self
            .metrics
            .output_bytes
            .checked_add(delta)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        self.policy.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.base_output_bytes
                .checked_add(self.metrics.output_bytes)
                .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?,
        )
    }

    fn check_grapheme_bytes(&self, bytes: usize) -> Result<()> {
        self.policy
            .check(AsciiResourceLimitId::MaxGraphemeBytes, bytes)
    }

    fn push_word_text(&mut self, value: &str) -> Result<()> {
        self.note_replay_event(ReplayEventKind::Text, value.as_bytes())?;
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
        let value_len = u64::try_from(value.len()).map_err(|_| {
            self.policy
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        let work = 1usize
            .checked_add(std::mem::size_of::<u64>())
            .and_then(|work| work.checked_add(value.len()))
            .ok_or_else(|| {
                self.policy
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        self.charge_layout_work(work)?;

        let mut hash = self.metrics.replay_fingerprint;
        for byte in std::iter::once(kind as u8)
            .chain(value_len.to_le_bytes())
            .chain(value.iter().copied())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        self.metrics.replay_fingerprint = hash;
        Ok(())
    }

    fn move_word_to_current(&mut self) -> Result<()> {
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
    policy: AsciiResourcePolicy,
    base_layout_work: usize,
) -> Result<(usize, usize)> {
    let mut work = 0usize;
    let mut width = 0usize;
    for prefix in [first_prefix, continuation_prefix] {
        let mut prefix_width = 0usize;
        for grapheme in measured_graphemes(prefix, width_profile) {
            work = work
                .checked_add(1)
                .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
            let actual = base_layout_work
                .checked_add(work)
                .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
            policy.check(AsciiResourceLimitId::MaxLayoutWorkUnits, actual)?;
            prefix_width = prefix_width
                .checked_add(grapheme.width())
                .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
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
