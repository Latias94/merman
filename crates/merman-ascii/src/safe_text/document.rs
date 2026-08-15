use super::encode::encode_budgeted_lines_with_expected;
#[cfg(test)]
use super::encode::encode_budgeted_lines_with_expected_and_probe;
use super::normalization::{
    NormalizedSegment, NormalizedSegmentKind, try_append_normalized_segment, visible_escape,
    visible_escape_len, visit_normalized_segments,
};
use super::width::grapheme_display_width;
use super::wrapped::{BudgetedWrappedText, WrappedPassConfig, measure_wrapped_prefix_widths};
use crate::Result;
use crate::color::AsciiColorMode;
use crate::error::AsciiError;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use std::fmt;

/// Normalizes and encodes a family-owned line document.
///
/// StructuredText families intentionally do not invent a two-dimensional grid, but they still
/// share the same authored-text and HTML safety boundary as grid-backed renderers.
#[cfg(test)]
pub(crate) fn encode_text_lines(
    lines: Vec<String>,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options, resources);
    for line in lines {
        document.push_line(line)?;
    }
    document.finish()
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

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct NormalizedTextRange {
    source_start: usize,
    source_end: usize,
    start: usize,
    end: usize,
}

impl BudgetedTextDocument {
    #[cfg(test)]
    pub(crate) fn new(options: &AsciiRenderOptions, resources: AsciiResourcePolicy) -> Self {
        Self::from_resources(ResourceContext::new(resources), options)
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
        render: impl Fn(&mut BudgetedWrappedText<'_>) -> Result<()>,
    ) -> Result<()> {
        // The producer is replayed after admission. Production callers must therefore remain
        // deterministic and avoid externally visible side effects.
        let (prefix_width, prefix_width_work) = measure_wrapped_prefix_widths(
            first_prefix,
            continuation_prefix,
            self.width_profile,
            &self.resources,
        )?;
        let available = if prefix_width < max_width {
            max_width - prefix_width
        } else {
            max_width.max(1)
        };
        let first_separator = usize::from(!self.lines.is_empty());
        let base_layout_work = self.resources.layout_work_used();
        let base_document_cells = self.resources.document_cells_used();
        let base_output_bytes = self.encoded_bytes_used;
        let pass_config = WrappedPassConfig {
            resources: self.resources.clone(),
            width_profile: self.width_profile,
            color_mode: self.color_mode,
            first_prefix,
            continuation_prefix,
            available,
            base_layout_work,
            fixed_layout_work: prefix_width_work,
            base_document_cells,
            base_output_bytes,
            #[cfg(test)]
            retain_probe: self.retain_probe.clone(),
        };

        let mut planner = BudgetedWrappedText::measure(pass_config.clone());
        planner.charge_layout_work(1)?;
        planner.admit_first_prefix(first_separator)?;
        render(&mut planner)?;
        let plan = planner.finish()?.metrics;
        let total_layout_work = prefix_width_work
            .checked_add(plan.layout_work.checked_mul(2).ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?)
            .ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;

        self.resources
            .check_usage(total_layout_work, plan.document_cells)?;
        let total_output_bytes = base_output_bytes
            .checked_add(plan.output_bytes)
            .ok_or_else(|| {
                self.resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
        self.resources
            .check(AsciiResourceLimitId::MaxOutputBytes, total_output_bytes)?;

        let mut materializer = BudgetedWrappedText::materialize(pass_config, plan.rows)?;
        materializer.charge_layout_work(1)?;
        materializer.admit_first_prefix(first_separator)?;
        render(&mut materializer)?;
        let materialized = materializer.finish()?;
        materialized.verify_metrics(plan)?;

        self.lines
            .try_reserve(materialized.rows.len())
            .map_err(|_| document_allocation_error())?;
        self.resources
            .charge_usage(total_layout_work, plan.document_cells)?;
        self.encoded_bytes_used = total_output_bytes;
        self.lines.extend(materialized.rows);
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<String> {
        encode_budgeted_lines_with_expected(
            self.lines,
            self.color_mode,
            &self.resources,
            self.encoded_bytes_used,
        )
    }

    #[cfg(test)]
    pub(crate) fn finish_with_probe(self, materialized: &std::cell::Cell<bool>) -> Result<String> {
        encode_budgeted_lines_with_expected_and_probe(
            self.lines,
            self.color_mode,
            &self.resources,
            self.encoded_bytes_used,
            || materialized.set(true),
        )
    }

    pub(crate) fn preflight_text_work(&mut self, value: &str) -> Result<()> {
        self.resources.charge_layout_work(1)?;
        visit_normalized_segments(value, |segment| self.budget_layout_segment(segment))
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

/// Charges the normalized authored-text scan without materializing its expansion.
pub(crate) fn charge_text_layout(resources: &ResourceContext, value: &str) -> Result<()> {
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

pub(super) fn try_write_fmt(
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

pub(super) fn document_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
    }
}
