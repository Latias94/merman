use super::encode::encode_budgeted_lines_with_expected;
use super::normalization::{
    NormalizedSegment, NormalizedSegmentKind, try_append_normalized_segment, visible_escape,
    visible_escape_len, visit_normalized_segments,
};
use super::width::grapheme_display_width;
use super::wrapped::{BudgetedWrappedText, WrappedPassConfig, measure_wrapped_prefix_widths};
use crate::Result;
use crate::color::AsciiColorMode;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
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
    planned_width: usize,
    planned_height: usize,
}

/// Streams one structured-text row from borrowed fragments.
///
/// Every fragment crosses the terminal-safety and resource boundary before it is appended. Callers
/// should use fragments at structural ASCII boundaries (prefixes, separators, authored fields).
pub(crate) struct BudgetedTextLine<'document> {
    document: &'document mut BudgetedTextDocument,
    current: String,
    current_width: usize,
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
            planned_width: 0,
            planned_height: 0,
        }
    }

    pub(crate) fn resources_mut(&mut self) -> &mut ResourceContext {
        &mut self.resources
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
            current_width: 0,
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
        for row in materialized.rows {
            self.record_materialized_row(&row)?;
            self.lines.push(row);
        }
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

    pub(crate) fn finish_with_execution(self, execution: AsciiExecution<'_>) -> Result<String> {
        execution.admit_primary_extent(
            self.planned_width,
            self.planned_height,
            self.width_profile,
        )?;
        self.finish()
    }

    pub(crate) fn preflight_text_work(&mut self, value: &str) -> Result<()> {
        self.resources.charge_layout_work(1)?;
        visit_normalized_segments(value, |segment| self.budget_layout_segment(segment))
    }

    fn budget_layout_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<()> {
        segment.check_grapheme_budget(&self.resources)?;
        self.resources.charge_layout_work(segment.layout_work())
    }

    fn budget_document_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<usize> {
        segment.check_grapheme_budget(&self.resources)?;
        let width = segment.display_width(self.width_profile);
        self.resources.charge_document_cells(width)?;
        Ok(width)
    }

    fn finish_prebudgeted_line(&mut self, line: String, width: usize) -> Result<()> {
        self.lines
            .try_reserve(1)
            .map_err(|_| document_allocation_error())?;
        self.planned_width = self.planned_width.max(width);
        self.planned_height = self
            .planned_height
            .checked_add(1)
            .ok_or_else(|| self.resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
        self.lines.push(line);
        Ok(())
    }

    fn record_materialized_row(&mut self, row: &str) -> Result<()> {
        let width = crate::text::display_width_with_profile(row, self.width_profile);
        self.planned_width = self.planned_width.max(width);
        self.planned_height = self
            .planned_height
            .checked_add(1)
            .ok_or_else(|| self.resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
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
}

impl BudgetedTextLine<'_> {
    pub(crate) fn push_str(&mut self, value: &str) -> Result<()> {
        self.document.admit_normalized_line_fragment(value)?;
        visit_normalized_segments(value, |segment| {
            self.document.budget_layout_segment(segment)?;
            match segment.kind {
                NormalizedSegmentKind::LineBreak => {
                    self.document.finish_prebudgeted_line(
                        std::mem::take(&mut self.current),
                        self.current_width,
                    )?;
                    self.current_width = 0;
                    self.document.resources.charge_layout_work(1)
                }
                _ => {
                    let width = self.document.budget_document_segment(segment)?;
                    self.current_width = self
                        .document
                        .resources
                        .checked_grid_add(self.current_width, width)?;
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

    fn finish(self) -> Result<()> {
        self.document
            .finish_prebudgeted_line(self.current, self.current_width)
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
