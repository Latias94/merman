use super::encode::encode_budgeted_lines_with_expected;
use super::normalization::{
    NormalizedSegment, NormalizedSegmentKind, try_append_normalized_segment, visible_escape,
    visible_escape_len, visit_normalized_segments,
};
use super::width::{MeasuredGrapheme, grapheme_display_width, measured_graphemes};
use crate::Result;
use crate::color::AsciiColorMode;
use crate::error::AsciiError;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

/// Normalizes and encodes a family-owned line document.
///
/// StructuredText families intentionally do not invent a two-dimensional grid, but they still
/// share the same authored-text and HTML safety boundary as grid-backed renderers.
#[cfg(test)]
pub(crate) fn encode_text_lines(
    lines: Vec<String>,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    for line in lines {
        document.push_line(line)?;
    }
    document.finish(options)
}

pub(crate) struct BudgetedTextDocument {
    lines: Vec<String>,
    resources: ResourceContext,
    width_profile: TerminalWidthProfile,
    color_mode: AsciiColorMode,
    // Debit bytes only when new semantic text enters retained storage. Buffer-to-buffer moves keep
    // ownership of their original debit; line separators and prefixes are admitted separately.
    encoded_bytes_used: usize,
    #[cfg(test)]
    retain_probe: Option<std::rc::Rc<std::cell::Cell<usize>>>,
}

/// Streams one structured-text row from borrowed fragments.
///
/// Every fragment crosses the terminal-safety and resource boundary before it is appended. Callers
/// should use fragments at structural ASCII boundaries (prefixes, separators, authored fields).
pub(crate) struct BudgetedTextLine<'document> {
    document: &'document mut BudgetedTextDocument,
    current: String,
}

/// Streams and wraps structured text while retaining at most one word and one output row.
///
/// Content cells are charged before entering either bounded buffer. Prefixes and synthesized spaces
/// are likewise charged before they are inserted into the retained row.
pub(crate) struct BudgetedWrappedText<'document, 'prefix> {
    document: &'document mut BudgetedTextDocument,
    first_prefix: &'prefix str,
    continuation_prefix: &'prefix str,
    available: usize,
    word: String,
    word_width: usize,
    long_word: bool,
    current: String,
    current_width: usize,
    emitted: bool,
    row_prefix_admitted: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct NormalizedTextRange {
    source_start: usize,
    source_end: usize,
    start: usize,
    end: usize,
}

impl BudgetedTextDocument {
    pub(crate) fn new(options: &AsciiRenderOptions) -> Self {
        Self::from_resources(ResourceContext::new(options.resources), options)
    }

    pub(crate) fn from_resources(resources: ResourceContext, options: &AsciiRenderOptions) -> Self {
        Self {
            lines: Vec::new(),
            resources,
            width_profile: options.terminal_width_profile,
            color_mode: options.color_mode,
            encoded_bytes_used: 0,
            #[cfg(test)]
            retain_probe: None,
        }
    }

    pub(crate) fn resources_mut(&mut self) -> &mut ResourceContext {
        &mut self.resources
    }

    #[cfg(test)]
    pub(crate) fn set_retain_probe(&mut self, probe: std::rc::Rc<std::cell::Cell<usize>>) {
        self.retain_probe = Some(probe);
    }

    #[cfg(test)]
    pub(crate) fn push_optional_line(&mut self, value: Option<&str>) -> Result<()> {
        self.push_optional_prefixed_line("", value)
    }

    #[cfg(test)]
    pub(crate) fn push_optional_prefixed_line(
        &mut self,
        prefix: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        let Some(range) = self.trimmed_normalized_range(value)? else {
            return Ok(());
        };

        self.push_line_with(|line| {
            line.push_str(prefix)?;
            line.push_normalized_range(value, range)
        })
    }

    pub(crate) fn push_line(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.push_line_with(|writer| writer.push_str(line.as_ref()))
    }

    pub(crate) fn push_line_with(
        &mut self,
        render: impl FnOnce(&mut BudgetedTextLine<'_>) -> Result<()>,
    ) -> Result<()> {
        self.resources.charge_layout_work(1)?;
        self.admit_output_line_prefix("")?;
        let mut writer = BudgetedTextLine {
            document: self,
            current: String::new(),
        };
        render(&mut writer)?;
        writer.finish()
    }

    #[cfg(test)]
    pub(crate) fn push_wrapped_prefixed_line(
        &mut self,
        first_prefix: &str,
        continuation_prefix: &str,
        text: &str,
        max_width: usize,
    ) -> Result<()> {
        self.push_wrapped_prefixed_line_with(
            first_prefix,
            continuation_prefix,
            max_width,
            |writer| writer.push_str(text),
        )
    }

    pub(crate) fn push_wrapped_prefixed_line_with(
        &mut self,
        first_prefix: &str,
        continuation_prefix: &str,
        max_width: usize,
        render: impl FnOnce(&mut BudgetedWrappedText<'_, '_>) -> Result<()>,
    ) -> Result<()> {
        let prefix_width = self
            .checked_display_width(first_prefix)?
            .max(self.checked_display_width(continuation_prefix)?);
        let available = if prefix_width < max_width {
            max_width - prefix_width
        } else {
            max_width.max(1)
        };
        self.resources.charge_layout_work(1)?;
        self.admit_output_line_prefix(first_prefix)?;
        let mut writer = BudgetedWrappedText {
            document: self,
            first_prefix,
            continuation_prefix,
            available,
            word: String::new(),
            word_width: 0,
            long_word: false,
            current: String::new(),
            current_width: 0,
            emitted: false,
            row_prefix_admitted: true,
        };
        render(&mut writer)?;
        writer.finish()
    }

    pub(crate) fn finish(self, options: &AsciiRenderOptions) -> Result<String> {
        debug_assert_eq!(self.width_profile, options.terminal_width_profile);
        debug_assert_eq!(self.color_mode, options.color_mode);
        debug_assert_eq!(self.resources.policy(), options.resources);
        let policy = self.resources.policy();
        encode_budgeted_lines_with_expected(
            self.lines,
            options.color_mode,
            policy,
            self.encoded_bytes_used,
            || {},
        )
    }

    #[cfg(test)]
    pub(crate) fn finish_with_probe(
        self,
        options: &AsciiRenderOptions,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<String> {
        debug_assert_eq!(self.width_profile, options.terminal_width_profile);
        debug_assert_eq!(self.color_mode, options.color_mode);
        debug_assert_eq!(self.resources.policy(), options.resources);
        let policy = self.resources.policy();
        encode_budgeted_lines_with_expected(
            self.lines,
            options.color_mode,
            policy,
            self.encoded_bytes_used,
            || materialized.set(true),
        )
    }

    pub(crate) fn preflight_text_work(&mut self, value: &str) -> Result<()> {
        self.resources.charge_layout_work(1)?;
        visit_normalized_segments(value, |segment| self.budget_layout_segment(segment))
    }

    fn checked_display_width(&self, value: &str) -> Result<usize> {
        let mut width = 0usize;
        for grapheme in measured_graphemes(value, self.width_profile) {
            width = width.checked_add(grapheme.width()).ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        }
        Ok(width)
    }

    fn budget_layout_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<()> {
        segment.check_grapheme_budget(&self.resources)?;
        self.resources.charge_layout_work(segment.layout_work())
    }

    fn budget_document_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<()> {
        segment.check_grapheme_budget(&self.resources)?;
        self.resources
            .charge_document_cells(segment.display_width(self.width_profile))
    }

    #[cfg(test)]
    fn trimmed_normalized_range(&mut self, value: &str) -> Result<Option<NormalizedTextRange>> {
        // Determine trim bounds over the normalized byte stream without retaining that stream. The
        // second pass emits only the kept range through `BudgetedTextLine`, so arbitrarily large
        // leading/trailing whitespace never becomes a temporary `String`.
        let mut offset = 0usize;
        let mut start = None;
        let mut end = 0usize;
        let mut source_start = 0usize;
        let mut source_end = 0usize;
        let mut source_normalized_base = 0usize;
        let mut kept_normalized_base = 0usize;
        let mut current_source = None;
        self.resources.charge_layout_work(1)?;
        visit_normalized_segments(value, |segment| {
            self.budget_layout_segment(segment)?;
            let mut buffer = [0u8; 10];
            let text = segment.text(&mut buffer);
            if current_source != Some(segment.source_start) {
                current_source = Some(segment.source_start);
                source_normalized_base = offset;
            }
            let segment_end = offset.checked_add(text.len()).ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
            for (relative, ch) in text.char_indices() {
                if !ch.is_whitespace() {
                    let absolute = offset.checked_add(relative).ok_or_else(|| {
                        self.resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxDocumentCells)
                    })?;
                    if start.is_none() {
                        source_start = segment.source_start;
                        kept_normalized_base = source_normalized_base;
                        start =
                            Some(absolute.checked_sub(kept_normalized_base).ok_or_else(|| {
                                self.resources
                                    .policy()
                                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
                            })?);
                    }
                    source_end = segment.source_end;
                    end = absolute
                        .checked_add(ch.len_utf8())
                        .and_then(|end| end.checked_sub(kept_normalized_base))
                        .ok_or_else(|| {
                            self.resources
                                .policy()
                                .overflow(AsciiResourceLimitId::MaxDocumentCells)
                        })?;
                }
            }
            offset = segment_end;
            Ok::<(), AsciiError>(())
        })?;

        Ok(start.map(|start| NormalizedTextRange {
            source_start,
            source_end,
            start,
            end,
        }))
    }

    fn finish_prebudgeted_line(&mut self, line: String) -> Result<()> {
        self.lines
            .try_reserve(1)
            .map_err(|_| document_allocation_error())?;
        self.lines.push(line);
        Ok(())
    }

    fn push_prebudgeted_normalized_fragments(&mut self, fragments: &[&str]) -> Result<()> {
        self.resources.charge_layout_work(1)?;
        let mut bytes = 0usize;
        for fragment in fragments {
            bytes = bytes
                .checked_add(fragment.len())
                .ok_or_else(document_allocation_error)?;
            for grapheme in fragment.graphemes(true) {
                self.resources.check_grapheme_bytes(grapheme.len())?;
                self.resources
                    .charge_document_cells(grapheme_display_width(grapheme, self.width_profile))?;
            }
        }

        self.lines
            .try_reserve(1)
            .map_err(|_| document_allocation_error())?;
        let mut line = String::new();
        line.try_reserve_exact(bytes)
            .map_err(|_| document_allocation_error())?;
        self.note_retain_materialization();
        for fragment in fragments {
            line.push_str(fragment);
        }
        self.lines.push(line);
        Ok(())
    }

    fn admit_output_line_prefix(&mut self, prefix: &str) -> Result<()> {
        let separator = usize::from(!self.lines.is_empty());
        let prefix_bytes =
            super::encode::encoded_text_len(prefix, self.color_mode, self.resources.policy())?;
        let additional = separator.checked_add(prefix_bytes).ok_or_else(|| {
            self.resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxOutputBytes)
        })?;
        self.admit_output_bytes(additional)
    }

    fn admit_output_text(&mut self, value: &str) -> Result<()> {
        let additional =
            super::encode::encoded_text_len(value, self.color_mode, self.resources.policy())?;
        self.admit_output_bytes(additional)
    }

    fn admit_normalized_line_fragment(&mut self, value: &str) -> Result<()> {
        let mut additional = 0usize;
        visit_normalized_segments(value, |segment| {
            let segment_bytes = match segment.kind {
                NormalizedSegmentKind::LineBreak => 1,
                _ => {
                    let mut buffer = [0u8; 10];
                    super::encode::encoded_text_len(
                        segment.text(&mut buffer),
                        self.color_mode,
                        self.resources.policy(),
                    )?
                }
            };
            additional = additional.checked_add(segment_bytes).ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
            Ok::<(), AsciiError>(())
        })?;
        self.admit_output_bytes(additional)
    }

    fn admit_output_bytes(&mut self, additional: usize) -> Result<()> {
        let actual = self
            .encoded_bytes_used
            .checked_add(additional)
            .ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
        self.resources
            .check(AsciiResourceLimitId::MaxOutputBytes, actual)?;
        self.encoded_bytes_used = actual;
        Ok(())
    }

    #[cfg(test)]
    fn note_retain_materialization(&self) {
        if let Some(probe) = &self.retain_probe {
            probe.set(probe.get() + 1);
        }
    }

    #[cfg(not(test))]
    fn note_retain_materialization(&self) {}
}

impl BudgetedTextLine<'_> {
    pub(crate) fn push_str(&mut self, value: &str) -> Result<()> {
        self.document.admit_normalized_line_fragment(value)?;
        visit_normalized_segments(value, |segment| {
            self.document.budget_layout_segment(segment)?;
            match segment.kind {
                NormalizedSegmentKind::LineBreak => {
                    self.document
                        .finish_prebudgeted_line(std::mem::take(&mut self.current))?;
                    self.document.resources.charge_layout_work(1)
                }
                _ => {
                    self.document.budget_document_segment(segment)?;
                    self.document.note_retain_materialization();
                    try_append_normalized_segment(
                        &mut self.current,
                        segment,
                        document_allocation_error,
                    )
                }
            }
        })
    }

    pub(crate) fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        try_write_fmt(arguments, |value| self.push_str(value))
    }

    pub(crate) fn push_quoted_text(&mut self, value: &str) -> Result<()> {
        let resources = self.document.resources.clone();
        super::framing::visit_quoted_terminal_text(value, &resources, |fragment| {
            self.push_str(fragment)
        })
    }

    #[cfg(test)]
    fn push_normalized_range(&mut self, value: &str, range: NormalizedTextRange) -> Result<()> {
        let value = &value[range.source_start..range.source_end];
        let mut offset = 0usize;
        visit_normalized_segments(value, |segment| {
            let mut buffer = [0u8; 10];
            let text = segment.text(&mut buffer);
            let segment_end = offset.checked_add(text.len()).ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
            let kept_start = range.start.max(offset);
            let kept_end = range.end.min(segment_end);
            if kept_start < kept_end {
                self.push_str(&text[kept_start - offset..kept_end - offset])?;
            }
            offset = segment_end;
            Ok(())
        })
    }

    fn finish(self) -> Result<()> {
        self.document.finish_prebudgeted_line(self.current)
    }
}

impl BudgetedWrappedText<'_, '_> {
    pub(crate) fn push_str(&mut self, value: &str) -> Result<()> {
        visit_normalized_segments(value, |segment| {
            self.document.budget_layout_segment(segment)?;
            match segment.kind {
                NormalizedSegmentKind::LineBreak => self.finish_paragraph(),
                NormalizedSegmentKind::Grapheme(grapheme) => self.push_normalized_text(grapheme),
                NormalizedSegmentKind::VisibleEscape(ch) => {
                    let mut buffer = [0u8; 10];
                    let escape = visible_escape(ch, &mut buffer);
                    self.document
                        .resources
                        .charge_document_cells(escape.len())?;
                    self.push_atomic_word_fragment_with_width(escape, escape.len())
                }
            }
        })
    }

    pub(crate) fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        try_write_fmt(arguments, |value| self.push_str(value))
    }

    pub(crate) fn push_quoted_text(&mut self, value: &str) -> Result<()> {
        let resources = self.document.resources.clone();
        super::framing::visit_quoted_terminal_text(value, &resources, |fragment| {
            self.push_exact_normalized_text(fragment)
        })
    }

    fn push_exact_normalized_text(&mut self, value: &str) -> Result<()> {
        let mut width = 0usize;
        let mut bytes = 0usize;
        visit_normalized_segments(value, |segment| {
            self.document.budget_layout_segment(segment)?;
            let mut buffer = [0u8; 10];
            let text = match segment.kind {
                NormalizedSegmentKind::LineBreak => "\\n",
                _ => segment.text(&mut buffer),
            };
            bytes = bytes.checked_add(text.len()).ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
            let segment_width = match segment.kind {
                NormalizedSegmentKind::LineBreak => 2,
                _ => segment.display_width(self.document.width_profile),
            };
            width = width.checked_add(segment_width).ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
            Ok::<(), AsciiError>(())
        })?;

        let total_cells = self
            .document
            .resources
            .document_cells_used()
            .checked_add(width)
            .ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        self.document
            .resources
            .check(AsciiResourceLimitId::MaxDocumentCells, total_cells)?;

        self.prepare_atomic_word_fragment(width)?;
        self.admit_row_before_new_content()?;

        let mut encoded_bytes = 0usize;
        visit_normalized_segments(value, |segment| {
            let mut buffer = [0u8; 10];
            let text = match segment.kind {
                NormalizedSegmentKind::LineBreak => "\\n",
                _ => segment.text(&mut buffer),
            };
            encoded_bytes = encoded_bytes
                .checked_add(super::encode::encoded_text_len(
                    text,
                    self.document.color_mode,
                    self.document.resources.policy(),
                )?)
                .ok_or_else(|| {
                    self.document
                        .resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxOutputBytes)
                })?;
            Ok::<(), AsciiError>(())
        })?;
        self.document.admit_output_bytes(encoded_bytes)?;
        self.document.note_retain_materialization();

        let mut normalized = String::new();
        normalized
            .try_reserve(bytes)
            .map_err(|_| document_allocation_error())?;
        visit_normalized_segments(value, |segment| {
            let mut buffer = [0u8; 10];
            let text = match segment.kind {
                NormalizedSegmentKind::LineBreak => "\\n",
                _ => segment.text(&mut buffer),
            };
            normalized.push_str(text);
            Ok::<(), AsciiError>(())
        })?;

        self.document.resources.charge_document_cells(width)?;
        self.push_prepared_atomic_word_fragment_with_width(&normalized, width)
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
        for grapheme in measured_graphemes(value, self.document.width_profile) {
            self.push_word_grapheme(grapheme)?;
        }
        Ok(())
    }

    fn push_atomic_word_fragment_with_width(&mut self, value: &str, width: usize) -> Result<()> {
        self.prepare_atomic_word_fragment(width)?;
        self.admit_row_before_new_content()?;
        self.document.admit_output_text(value)?;
        self.document.note_retain_materialization();
        self.push_prepared_atomic_word_fragment_with_width(value, width)
    }

    fn prepare_atomic_word_fragment(&mut self, width: usize) -> Result<()> {
        let prospective = self.word_width.checked_add(width).ok_or_else(|| {
            self.document
                .resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        if prospective > self.available && !self.word.is_empty() {
            if !self.long_word {
                if !self.current.is_empty() {
                    self.emit_current()?;
                }
                self.long_word = true;
            }
            self.emit_word_chunk()?;
        }

        if self.word.is_empty() && width > self.available {
            if !self.current.is_empty() {
                self.emit_current()?;
            }
            self.long_word = true;
        }
        Ok(())
    }

    fn push_prepared_atomic_word_fragment_with_width(
        &mut self,
        value: &str,
        width: usize,
    ) -> Result<()> {
        try_push_document_str(&mut self.word, value)?;
        self.word_width = self.word_width.checked_add(width).ok_or_else(|| {
            self.document
                .resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxDocumentCells)
        })?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn push_word_grapheme(&mut self, grapheme: MeasuredGrapheme<'_>) -> Result<()> {
        self.document
            .resources
            .charge_document_cells(grapheme.width())?;
        let prospective = self
            .word_width
            .checked_add(grapheme.width())
            .ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;

        if prospective > self.available && !self.word.is_empty() {
            if !self.long_word {
                if !self.current.is_empty() {
                    self.emit_current()?;
                }
                self.long_word = true;
            }
            self.emit_word_chunk()?;
        }

        self.admit_row_before_new_content()?;
        self.document.admit_output_text(grapheme.text())?;
        self.document.note_retain_materialization();
        try_push_document_str(&mut self.word, grapheme.text())?;
        self.word_width = self
            .word_width
            .checked_add(grapheme.width())
            .ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        if self.long_word && self.word_width >= self.available {
            self.emit_word_chunk()?;
        }
        Ok(())
    }

    fn finish_word(&mut self) -> Result<()> {
        if self.word.is_empty() {
            self.long_word = false;
            return Ok(());
        }
        if self.long_word {
            self.emit_word_chunk()?;
            self.long_word = false;
            return Ok(());
        }

        let separator = usize::from(!self.current.is_empty());
        let prospective = self
            .current_width
            .checked_add(separator)
            .and_then(|width| width.checked_add(self.word_width))
            .ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        if prospective > self.available && !self.current.is_empty() {
            self.emit_current()?;
        }
        if !self.current.is_empty() {
            self.document.resources.charge_document_cells(1)?;
            self.document.admit_output_text(" ")?;
            self.document.note_retain_materialization();
            try_push_document_str(&mut self.current, " ")?;
            self.current_width = self.current_width.checked_add(1).ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        }

        let word = std::mem::take(&mut self.word);
        try_push_document_str(&mut self.current, &word)?;
        self.current_width = self
            .current_width
            .checked_add(self.word_width)
            .ok_or_else(|| {
                self.document
                    .resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxDocumentCells)
            })?;
        self.word_width = 0;
        Ok(())
    }

    fn emit_word_chunk(&mut self) -> Result<()> {
        if self.word.is_empty() {
            return Ok(());
        }
        let content = std::mem::take(&mut self.word);
        self.word_width = 0;
        self.emit_row(content)
    }

    fn emit_current(&mut self) -> Result<()> {
        if self.current.is_empty() {
            return Ok(());
        }
        let content = std::mem::take(&mut self.current);
        self.current_width = 0;
        self.emit_row(content)
    }

    fn emit_row(&mut self, mut content: String) -> Result<()> {
        let prefix = if self.emitted {
            self.continuation_prefix
        } else {
            self.first_prefix
        };
        self.document.resources.charge_layout_work(1)?;
        for grapheme in prefix.graphemes(true) {
            self.document
                .resources
                .check_grapheme_bytes(grapheme.len())?;
            self.document
                .resources
                .charge_document_cells(grapheme_display_width(
                    grapheme,
                    self.document.width_profile,
                ))?;
        }
        if !self.row_prefix_admitted {
            self.document.admit_output_line_prefix(prefix)?;
            self.row_prefix_admitted = true;
        }
        self.document
            .lines
            .try_reserve(1)
            .map_err(|_| document_allocation_error())?;
        content
            .try_reserve(prefix.len())
            .map_err(|_| document_allocation_error())?;
        self.document.note_retain_materialization();
        content.insert_str(0, prefix);
        self.document.lines.push(content);
        self.emitted = true;
        self.row_prefix_admitted = false;
        Ok(())
    }

    fn admit_row_before_new_content(&mut self) -> Result<()> {
        if self.emitted
            && !self.row_prefix_admitted
            && self.word.is_empty()
            && self.current.is_empty()
        {
            self.document
                .admit_output_line_prefix(self.continuation_prefix)?;
            self.row_prefix_admitted = true;
        }
        Ok(())
    }

    fn finish_paragraph(&mut self) -> Result<()> {
        self.finish_word()?;
        self.emit_current()
    }

    fn finish(mut self) -> Result<()> {
        self.finish_paragraph()?;
        if !self.emitted {
            self.document
                .push_prebudgeted_normalized_fragments(&[self.first_prefix])?;
        }
        Ok(())
    }
}

/// Charges the normalized authored-text scan without materializing its expansion.
pub(crate) fn charge_text_layout(resources: &mut ResourceContext, value: &str) -> Result<()> {
    resources.charge_layout_work(1)?;
    visit_normalized_segments(value, |segment| {
        segment.check_grapheme_budget(resources)?;
        resources.charge_layout_work(segment.layout_work())
    })
}

/// Streams one authored terminal line through the safety boundary without first materializing its
/// normalized expansion. Returning `false` from `visit` stops at the current display boundary.
enum SafeLineVisitError {
    Stop,
    Render(AsciiError),
}

pub(crate) fn visit_safe_line_graphemes(
    resources: &mut ResourceContext,
    value: &str,
    profile: TerminalWidthProfile,
    mut visit: impl FnMut(&str, usize) -> Result<bool>,
) -> Result<()> {
    resources.charge_layout_work(1)?;
    let result = visit_normalized_segments(value, |segment| {
        if matches!(segment.kind, NormalizedSegmentKind::LineBreak) {
            resources
                .check_grapheme_bytes(segment.source_grapheme_bytes)
                .map_err(SafeLineVisitError::Render)?;
            resources
                .charge_layout_work(visible_escape_len('\n'))
                .map_err(SafeLineVisitError::Render)?;
            return visit_visible_escape_graphemes('\n', &mut visit);
        }

        segment
            .check_grapheme_budget(resources)
            .map_err(SafeLineVisitError::Render)?;
        resources
            .charge_layout_work(segment.layout_work())
            .map_err(SafeLineVisitError::Render)?;
        match segment.kind {
            NormalizedSegmentKind::Grapheme(grapheme) => {
                if visit(grapheme, grapheme_display_width(grapheme, profile))
                    .map_err(SafeLineVisitError::Render)?
                {
                    Ok(())
                } else {
                    Err(SafeLineVisitError::Stop)
                }
            }
            NormalizedSegmentKind::VisibleEscape(ch) => {
                visit_visible_escape_graphemes(ch, &mut visit)
            }
            NormalizedSegmentKind::LineBreak => unreachable!("handled above"),
        }
    });

    match result {
        Ok(()) | Err(SafeLineVisitError::Stop) => Ok(()),
        Err(SafeLineVisitError::Render(error)) => Err(error),
    }
}

fn visit_visible_escape_graphemes(
    ch: char,
    visit: &mut impl FnMut(&str, usize) -> Result<bool>,
) -> std::result::Result<(), SafeLineVisitError> {
    let mut buffer = [0u8; 10];
    let escape = visible_escape(ch, &mut buffer);
    for byte in escape.as_bytes() {
        let scalar = std::str::from_utf8(std::slice::from_ref(byte))
            .expect("visible escapes contain only ASCII bytes");
        match visit(scalar, 1) {
            Ok(true) => {}
            Ok(false) => return Err(SafeLineVisitError::Stop),
            Err(error) => return Err(SafeLineVisitError::Render(error)),
        }
    }
    Ok(())
}

fn try_push_document_str(output: &mut String, value: &str) -> Result<()> {
    output
        .try_reserve(value.len())
        .map_err(|_| document_allocation_error())?;
    output.push_str(value);
    Ok(())
}

fn try_write_fmt(
    arguments: fmt::Arguments<'_>,
    mut push_str: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    struct Adapter<'a, F> {
        push_str: &'a mut F,
        error: Option<AsciiError>,
    }

    impl<F> fmt::Write for Adapter<'_, F>
    where
        F: FnMut(&str) -> Result<()>,
    {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            match (self.push_str)(value) {
                Ok(()) => Ok(()),
                Err(error) => {
                    self.error = Some(error);
                    Err(fmt::Error)
                }
            }
        }
    }

    let mut adapter = Adapter {
        push_str: &mut push_str,
        error: None,
    };
    if fmt::write(&mut adapter, arguments).is_err() {
        return Err(adapter.error.unwrap_or(AsciiError::InvalidOption {
            field: "structured_text",
            message: "formatting failed",
        }));
    }
    Ok(())
}

fn document_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
    }
}
