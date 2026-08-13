use super::normalization::{NormalizedSegment, NormalizedSegmentKind, visit_normalized_segments};
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy, ResourceContext,
};
use crate::text::html_break_end;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedLabelLines {
    lines: Vec<String>,
    width: usize,
}

impl NormalizedLabelLines {
    pub(crate) fn into_parts(self) -> (Vec<String>, usize) {
        (self.lines, self.width)
    }
}

/// Builds terminal-safe label rows without retaining the normalized expansion before admission.
///
/// The source is scanned without allocation to establish terminal normalization, label-break,
/// grapheme, work, document-cell, and retained row-byte bounds. Only an admitted label is
/// materialized. `trim` matches relation labels, whose terminal-normalized authored text is
/// trimmed before Mermaid `<br>`/`\\n` label breaks are interpreted.
pub(crate) fn try_build_normalized_label_lines(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelLines>> {
    try_build_normalized_label_lines_impl(raw, width_profile, trim, wrap_width, resources, || {})
}

fn try_build_normalized_label_lines_impl(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<Option<NormalizedLabelLines>> {
    resources.transaction(|resources| {
        try_build_normalized_label_lines_transactional(
            raw,
            width_profile,
            trim,
            wrap_width,
            resources,
            before_materialize,
        )
    })
}

fn try_build_normalized_label_lines_transactional(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<Option<NormalizedLabelLines>> {
    let Some(plan) =
        try_plan_normalized_label_lines(raw, width_profile, trim, wrap_width, resources)?
    else {
        return Ok(None);
    };
    let metrics = plan.metrics();
    resources.grid_extent(metrics.max_width.max(1), metrics.line_count)?;
    resources.check_usage(0, metrics.document_cells)?;
    resources.check(
        AsciiResourceLimitId::MaxOutputBytes,
        metrics.materialized_bytes,
    )?;
    resources.charge_usage(0, metrics.document_cells)?;
    plan.materialize_with(raw, resources, before_materialize)
        .map(Some)
}

#[derive(Debug, Clone, Copy)]
enum LabelSelection {
    All,
    Range { start: usize, end: usize },
}

#[derive(Debug, Clone, Copy)]
enum LabelToken<'a> {
    Segment(NormalizedSegment<'a>),
    AuthoredBreak(&'a str),
}

#[derive(Debug, Clone, Copy)]
enum LabelOutputSegment<'a> {
    Segment(NormalizedSegment<'a>),
    LineBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizedLabelMetrics {
    pub(crate) materialized_bytes: usize,
    pub(crate) document_cells: usize,
    pub(crate) line_count: usize,
    pub(crate) max_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizedLabelRowMetrics {
    pub(crate) width: usize,
    pub(crate) retained_width: usize,
    materialized_bytes: usize,
}

impl NormalizedLabelMetrics {
    const EMPTY: Self = Self {
        materialized_bytes: 0,
        document_cells: 0,
        line_count: 0,
        max_width: 0,
    };

    fn try_include_row(
        &mut self,
        row: NormalizedLabelRowMetrics,
        policy: AsciiResourcePolicy,
    ) -> Result<()> {
        self.line_count = checked_add_with_policy(
            policy,
            AsciiResourceLimitId::MaxDocumentCells,
            self.line_count,
            1,
        )?;
        self.max_width = self.max_width.max(row.width);
        self.document_cells = checked_add_with_policy(
            policy,
            AsciiResourceLimitId::MaxDocumentCells,
            self.document_cells,
            row.width,
        )?;
        self.materialized_bytes = checked_add_with_policy(
            policy,
            AsciiResourceLimitId::MaxOutputBytes,
            self.materialized_bytes,
            row.materialized_bytes,
        )?;
        Ok(())
    }
}

struct MaterializedLabelRows {
    lines: Vec<String>,
    metrics: NormalizedLabelMetrics,
    expected_line_count: usize,
}

impl MaterializedLabelRows {
    fn try_new(expected_line_count: usize) -> Result<Self> {
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(expected_line_count)
            .map_err(|_| document_allocation_error())?;
        Ok(Self {
            lines,
            metrics: NormalizedLabelMetrics::EMPTY,
            expected_line_count,
        })
    }

    fn try_push_row(
        &mut self,
        row: String,
        width: usize,
        policy: AsciiResourcePolicy,
    ) -> Result<()> {
        if self.lines.len() >= self.expected_line_count {
            return Err(invalid_label_extent_plan());
        }
        self.metrics.try_include_row(
            NormalizedLabelRowMetrics {
                width,
                retained_width: width,
                materialized_bytes: row.len(),
            },
            policy,
        )?;
        self.lines.push(row);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelBreakPolicy {
    MermaidLabelBreaks,
    StructuralParagraphs,
    VisibleLine,
}

impl LabelBreakPolicy {
    const fn preserve_empty_paragraphs(self) -> bool {
        matches!(self, Self::MermaidLabelBreaks)
    }

    const fn ensure_nonempty_wrapped_output(self) -> bool {
        matches!(self, Self::MermaidLabelBreaks)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalizedLabelPlan {
    selection: LabelSelection,
    source_metrics: NormalizedLabelMetrics,
    output_metrics: NormalizedLabelMetrics,
    width_profile: TerminalWidthProfile,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    replay_work_units: usize,
    policy: AsciiResourcePolicy,
}

impl NormalizedLabelPlan {
    pub(crate) const fn metrics(self) -> NormalizedLabelMetrics {
        self.output_metrics
    }

    pub(crate) const fn materialization_work_units(self) -> usize {
        self.replay_work_units
    }

    pub(crate) fn check_materialization_limits(self, resources: &ResourceContext) -> Result<()> {
        resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.output_metrics.materialized_bytes,
        )
    }

    #[cfg(test)]
    pub(crate) fn try_visit_line_widths(
        self,
        raw: &str,
        resources: &ResourceContext,
        mut visit: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        self.try_visit_row_metrics(raw, resources, |row| visit(row.width))
    }

    pub(crate) fn try_visit_row_metrics(
        self,
        raw: &str,
        resources: &ResourceContext,
        visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            self.try_visit_row_metrics_transactional(raw, resources, visit)
        })
    }

    fn try_visit_row_metrics_transactional(
        self,
        raw: &str,
        resources: &ResourceContext,
        visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
    ) -> Result<()> {
        resources.charge_layout_work(self.replay_work_units)?;
        visit_label_row_metrics(
            raw,
            self.selection,
            self.width_profile,
            self.wrap_width,
            self.break_policy,
            self.policy,
            visit,
        )
    }

    pub(crate) fn materialize(
        self,
        raw: &str,
        resources: &ResourceContext,
    ) -> Result<NormalizedLabelLines> {
        self.materialize_with(raw, resources, || {})
    }

    pub(crate) fn materialize_after_admission(self, raw: &str) -> Result<NormalizedLabelLines> {
        self.materialize_impl(raw, || {})
    }

    fn materialize_with(
        self,
        raw: &str,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<NormalizedLabelLines> {
        resources.transaction(|resources| {
            self.materialize_with_transactional(raw, resources, before_materialize)
        })
    }

    fn materialize_with_transactional(
        self,
        raw: &str,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<NormalizedLabelLines> {
        resources.charge_layout_work(self.replay_work_units)?;
        self.materialize_impl(raw, before_materialize)
    }

    fn materialize_impl(
        self,
        raw: &str,
        before_materialize: impl FnOnce(),
    ) -> Result<NormalizedLabelLines> {
        before_materialize();
        let mut materialized = match self.wrap_width {
            Some(max_width) => materialize_wrapped_label(
                raw,
                self.selection,
                self.width_profile,
                max_width,
                self.break_policy,
                self.policy,
                self.output_metrics.line_count,
            )?,
            None => materialize_label(
                raw,
                self.selection,
                self.source_metrics,
                self.width_profile,
                self.break_policy,
                self.policy,
            )?,
        };
        if materialized.lines.is_empty()
            && (self.wrap_width.is_none() || self.break_policy.ensure_nonempty_wrapped_output())
        {
            materialized.try_push_row(String::new(), 0, self.policy)?;
        }
        if materialized.metrics != self.output_metrics {
            return Err(invalid_label_extent_plan());
        }

        Ok(NormalizedLabelLines {
            lines: materialized.lines,
            width: self.output_metrics.max_width,
        })
    }

    #[cfg(test)]
    pub(crate) fn materialize_with_probe(
        self,
        raw: &str,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<NormalizedLabelLines> {
        self.materialize_with(raw, resources, || materialized.set(true))
    }
}

pub(crate) fn try_plan_normalized_label_lines(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelPlan>> {
    try_plan_normalized_label_lines_with_policy(
        raw,
        width_profile,
        trim,
        wrap_width,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
    )
}

pub(crate) fn try_plan_normalized_label_lines_with_policy(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelPlan>> {
    resources.transaction(|resources| {
        try_plan_normalized_label_lines_with_policy_transactional(
            raw,
            width_profile,
            trim,
            wrap_width,
            break_policy,
            resources,
        )
    })
}

fn try_plan_normalized_label_lines_with_policy_transactional(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelPlan>> {
    let selection = match normalized_label_selection(raw, trim, break_policy, resources)? {
        Some(selection) => selection,
        None => return Ok(None),
    };
    let source_metrics = preflight_label(raw, selection, width_profile, break_policy, resources)?;
    let replay_work_units = resources.checked_work_add(
        raw.len().max(1),
        resources.checked_work_add(source_metrics.document_cells, source_metrics.line_count)?,
    )?;

    let output_metrics = if let Some(max_width) = wrap_width {
        resources.charge_layout_work(replay_work_units)?;
        measure_label_output_metrics(
            raw,
            selection,
            width_profile,
            Some(max_width),
            break_policy,
            resources.policy(),
        )?
    } else {
        source_metrics
    };

    Ok(Some(NormalizedLabelPlan {
        selection,
        source_metrics,
        output_metrics,
        width_profile,
        wrap_width,
        break_policy,
        replay_work_units,
        policy: resources.policy(),
    }))
}

/// Measures a terminal label without materializing its normalized rows.
///
/// Callers that own a grid extent can use this descriptor to size the grid first, then call
/// `try_build_normalized_label_lines` only after that extent has been admitted.
pub(crate) fn try_measure_normalized_label_lines(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelMetrics>> {
    resources.transaction(|resources| {
        try_measure_normalized_label_lines_transactional(raw, width_profile, trim, resources)
    })
}

fn try_measure_normalized_label_lines_transactional(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelMetrics>> {
    let selection = match normalized_label_selection(
        raw,
        trim,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
    )? {
        Some(selection) => selection,
        None => return Ok(None),
    };
    Ok(Some(preflight_label(
        raw,
        selection,
        width_profile,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
    )?))
}

fn normalized_label_selection(
    raw: &str,
    trim: bool,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
) -> Result<Option<LabelSelection>> {
    if !trim {
        return Ok(Some(LabelSelection::All));
    }
    resources.charge_layout_work(raw.len().max(1))?;

    let mut offset = 0usize;
    let mut start = None;
    let mut end = 0usize;
    visit_label_tokens(raw, break_policy, |token| {
        with_token_trim_text(token, |text| {
            for (relative, ch) in text.char_indices() {
                if ch.is_whitespace() {
                    continue;
                }
                let absolute = checked_add(
                    resources,
                    AsciiResourceLimitId::MaxOutputBytes,
                    offset,
                    relative,
                )?;
                start.get_or_insert(absolute);
                end = checked_add(
                    resources,
                    AsciiResourceLimitId::MaxOutputBytes,
                    absolute,
                    ch.len_utf8(),
                )?;
            }
            offset = checked_add(
                resources,
                AsciiResourceLimitId::MaxOutputBytes,
                offset,
                text.len(),
            )?;
            Ok::<(), AsciiError>(())
        })
    })?;

    Ok(start.map(|start| LabelSelection::Range { start, end }))
}

fn preflight_label(
    raw: &str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
) -> Result<NormalizedLabelMetrics> {
    resources.charge_layout_work(raw.len().max(1))?;
    let mut materialized_bytes = 0usize;
    let mut document_cells = 0usize;
    let mut line_count = 1usize;
    let mut line_width = 0usize;
    let mut max_width = 0usize;
    let policy = resources.policy();

    visit_selected_label_output(
        raw,
        selection,
        break_policy,
        policy,
        |source_segment, output| {
            if let Some(source_segment) = source_segment {
                source_segment.check_grapheme_budget(resources)?;
                resources.charge_layout_work(source_segment.layout_work())?;
            }

            match output {
                LabelOutputSegment::LineBreak => {
                    resources.charge_layout_work(1)?;
                    line_count = checked_add(
                        resources,
                        AsciiResourceLimitId::MaxDocumentCells,
                        line_count,
                        1,
                    )?;
                    max_width = max_width.max(line_width);
                    line_width = 0;
                }
                LabelOutputSegment::Segment(segment) => {
                    segment.check_grapheme_budget(resources)?;
                    resources.charge_layout_work(segment.layout_work())?;
                    let mut buffer = [0u8; 10];
                    let text = segment.text(&mut buffer);
                    materialized_bytes = checked_add(
                        resources,
                        AsciiResourceLimitId::MaxOutputBytes,
                        materialized_bytes,
                        text.len(),
                    )?;
                    let width = segment.display_width(width_profile);
                    document_cells = checked_add(
                        resources,
                        AsciiResourceLimitId::MaxDocumentCells,
                        document_cells,
                        width,
                    )?;
                    line_width = checked_add(
                        resources,
                        AsciiResourceLimitId::MaxDocumentCells,
                        line_width,
                        width,
                    )?;
                }
            }
            Ok(())
        },
    )?;
    max_width = max_width.max(line_width);

    Ok(NormalizedLabelMetrics {
        materialized_bytes,
        document_cells,
        line_count,
        max_width,
    })
}

fn materialize_label(
    raw: &str,
    selection: LabelSelection,
    metrics: NormalizedLabelMetrics,
    width_profile: TerminalWidthProfile,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
) -> Result<MaterializedLabelRows> {
    let mut materialized = MaterializedLabelRows::try_new(metrics.line_count)?;
    let mut current = String::new();
    let mut current_width = 0usize;
    visit_selected_label_output(
        raw,
        selection,
        break_policy,
        policy,
        |_source_segment, output| {
            match output {
                LabelOutputSegment::LineBreak => {
                    materialized.try_push_row(
                        std::mem::take(&mut current),
                        current_width,
                        policy,
                    )?;
                    current_width = 0;
                }
                LabelOutputSegment::Segment(segment) => {
                    let mut buffer = [0u8; 10];
                    let text = segment.text(&mut buffer);
                    current
                        .try_reserve(text.len())
                        .map_err(|_| document_allocation_error())?;
                    current.push_str(text);
                    current_width = checked_add_with_policy(
                        policy,
                        AsciiResourceLimitId::MaxDocumentCells,
                        current_width,
                        segment.display_width(width_profile),
                    )?;
                }
            }
            Ok::<(), AsciiError>(())
        },
    )?;
    materialized.try_push_row(current, current_width, policy)?;
    Ok(materialized)
}

fn measure_label_output_metrics(
    raw: &str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
) -> Result<NormalizedLabelMetrics> {
    let mut metrics = NormalizedLabelMetrics::EMPTY;
    visit_label_row_metrics(
        raw,
        selection,
        width_profile,
        wrap_width,
        break_policy,
        policy,
        |row| metrics.try_include_row(row, policy),
    )?;
    Ok(metrics)
}

fn visit_label_row_metrics(
    raw: &str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    mut visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
) -> Result<()> {
    let Some(max_width) = wrap_width else {
        let mut line_width = 0usize;
        let mut retained_width = 0usize;
        let mut materialized_bytes = 0usize;
        visit_selected_label_output(raw, selection, break_policy, policy, |_source, output| {
            match output {
                LabelOutputSegment::LineBreak => {
                    visit(NormalizedLabelRowMetrics {
                        width: line_width,
                        retained_width,
                        materialized_bytes,
                    })?;
                    line_width = 0;
                    retained_width = 0;
                    materialized_bytes = 0;
                }
                LabelOutputSegment::Segment(segment) => {
                    let mut buffer = [0u8; 10];
                    let text = segment.text(&mut buffer);
                    materialized_bytes = checked_add_with_policy(
                        policy,
                        AsciiResourceLimitId::MaxOutputBytes,
                        materialized_bytes,
                        text.len(),
                    )?;
                    line_width = checked_add_with_policy(
                        policy,
                        AsciiResourceLimitId::MaxDocumentCells,
                        line_width,
                        segment.display_width(width_profile),
                    )?;
                    if text != " " {
                        retained_width = line_width;
                    }
                }
            }
            Ok(())
        })?;
        return visit(NormalizedLabelRowMetrics {
            width: line_width,
            retained_width,
            materialized_bytes,
        });
    };

    process_wrapped_label(
        raw,
        selection,
        width_profile,
        max_width,
        break_policy,
        policy,
        WrappedWidthSink {
            visit,
            policy,
            word_bytes: 0,
            line_bytes: 0,
        },
    )
}

trait WrappedLabelSink {
    type Output;

    fn push_word_unit(&mut self, text: &str) -> Result<()>;
    fn emit_word_chunk(&mut self, width: usize) -> Result<()>;
    fn append_word_to_line(&mut self, separator: bool) -> Result<()>;
    fn emit_line(&mut self, width: usize) -> Result<()>;
    fn emit_empty_line(&mut self) -> Result<()>;
    fn finish(self) -> Result<Self::Output>;
}

struct WrappedLabelProcessor<S> {
    max_width: usize,
    width_profile: TerminalWidthProfile,
    policy: AsciiResourcePolicy,
    sink: S,
    preserve_empty_paragraphs: bool,
    ensure_nonempty_output: bool,
    paragraph_has_text: bool,
    in_word: bool,
    word_is_long: bool,
    word_width: usize,
    chunk_width: usize,
    line_width: usize,
    emitted_lines: usize,
}

impl<S> WrappedLabelProcessor<S>
where
    S: WrappedLabelSink,
{
    fn new(
        max_width: usize,
        width_profile: TerminalWidthProfile,
        policy: AsciiResourcePolicy,
        break_policy: LabelBreakPolicy,
        sink: S,
    ) -> Self {
        Self {
            max_width: max_width.max(1),
            width_profile,
            policy,
            sink,
            preserve_empty_paragraphs: break_policy.preserve_empty_paragraphs(),
            ensure_nonempty_output: break_policy.ensure_nonempty_wrapped_output(),
            paragraph_has_text: false,
            in_word: false,
            word_is_long: false,
            word_width: 0,
            chunk_width: 0,
            line_width: 0,
            emitted_lines: 0,
        }
    }

    fn push_segment(&mut self, segment: NormalizedSegment<'_>) -> Result<()> {
        let mut buffer = [0u8; 10];
        let text = segment.text(&mut buffer);
        self.paragraph_has_text |= !text.is_empty();
        if matches!(segment.kind, NormalizedSegmentKind::Grapheme(grapheme) if grapheme.chars().all(char::is_whitespace))
        {
            return self.finish_word();
        }
        self.in_word = true;
        self.push_word_unit(text, segment.display_width(self.width_profile))
    }

    fn push_word_unit(&mut self, text: &str, width: usize) -> Result<()> {
        let next_chunk_width = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxDocumentCells,
            self.chunk_width,
            width,
        )?;
        if next_chunk_width > self.max_width && self.chunk_width > 0 {
            if !self.word_is_long {
                self.flush_line()?;
                self.word_is_long = true;
            }
            self.emit_word_chunk()?;
        }
        self.sink.push_word_unit(text)?;
        self.chunk_width = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxDocumentCells,
            self.chunk_width,
            width,
        )?;
        self.word_width = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxDocumentCells,
            self.word_width,
            width,
        )?;
        Ok(())
    }

    fn finish_word(&mut self) -> Result<()> {
        if !self.in_word {
            return Ok(());
        }
        if self.word_is_long || self.word_width > self.max_width {
            if !self.word_is_long {
                self.flush_line()?;
            }
            if self.chunk_width > 0 {
                self.emit_word_chunk()?;
            }
            self.line_width = 0;
        } else {
            let separator = self.line_width > 0;
            let needed = checked_add_with_policy(
                self.policy,
                AsciiResourceLimitId::MaxDocumentCells,
                checked_add_with_policy(
                    self.policy,
                    AsciiResourceLimitId::MaxDocumentCells,
                    self.line_width,
                    usize::from(separator),
                )?,
                self.word_width,
            )?;
            if needed > self.max_width && self.line_width > 0 {
                self.flush_line()?;
            }
            let separator = self.line_width > 0;
            self.sink.append_word_to_line(separator)?;
            self.line_width = checked_add_with_policy(
                self.policy,
                AsciiResourceLimitId::MaxDocumentCells,
                checked_add_with_policy(
                    self.policy,
                    AsciiResourceLimitId::MaxDocumentCells,
                    self.line_width,
                    usize::from(separator),
                )?,
                self.word_width,
            )?;
        }

        self.in_word = false;
        self.word_is_long = false;
        self.word_width = 0;
        self.chunk_width = 0;
        Ok(())
    }

    fn finish_paragraph(&mut self) -> Result<()> {
        self.finish_word()?;
        self.flush_line()?;
        if !self.paragraph_has_text && self.preserve_empty_paragraphs {
            self.emit_empty_line()?;
        }
        self.paragraph_has_text = false;
        Ok(())
    }

    fn finish(mut self) -> Result<S::Output> {
        self.finish_paragraph()?;
        if self.emitted_lines == 0 && self.ensure_nonempty_output {
            self.emit_empty_line()?;
        }
        self.sink.finish()
    }

    fn flush_line(&mut self) -> Result<()> {
        if self.line_width > 0 {
            let width = self.line_width;
            self.line_width = 0;
            self.record_emitted_line()?;
            self.sink.emit_line(width)?;
        }
        Ok(())
    }

    fn emit_word_chunk(&mut self) -> Result<()> {
        let width = self.chunk_width;
        self.chunk_width = 0;
        self.record_emitted_line()?;
        self.sink.emit_word_chunk(width)
    }

    fn emit_empty_line(&mut self) -> Result<()> {
        self.record_emitted_line()?;
        self.sink.emit_empty_line()
    }

    fn record_emitted_line(&mut self) -> Result<()> {
        self.emitted_lines = self
            .emitted_lines
            .checked_add(1)
            .ok_or_else(|| self.policy.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        Ok(())
    }
}

struct WrappedWidthSink<F> {
    visit: F,
    policy: AsciiResourcePolicy,
    word_bytes: usize,
    line_bytes: usize,
}

impl<F> WrappedLabelSink for WrappedWidthSink<F>
where
    F: FnMut(NormalizedLabelRowMetrics) -> Result<()>,
{
    type Output = ();

    fn push_word_unit(&mut self, text: &str) -> Result<()> {
        self.word_bytes = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxOutputBytes,
            self.word_bytes,
            text.len(),
        )?;
        Ok(())
    }

    fn emit_word_chunk(&mut self, width: usize) -> Result<()> {
        let materialized_bytes = std::mem::take(&mut self.word_bytes);
        (self.visit)(NormalizedLabelRowMetrics {
            width,
            retained_width: width,
            materialized_bytes,
        })
    }

    fn append_word_to_line(&mut self, separator: bool) -> Result<()> {
        let extra = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxOutputBytes,
            self.word_bytes,
            usize::from(separator),
        )?;
        self.line_bytes = checked_add_with_policy(
            self.policy,
            AsciiResourceLimitId::MaxOutputBytes,
            self.line_bytes,
            extra,
        )?;
        self.word_bytes = 0;
        Ok(())
    }

    fn emit_line(&mut self, width: usize) -> Result<()> {
        let materialized_bytes = std::mem::take(&mut self.line_bytes);
        (self.visit)(NormalizedLabelRowMetrics {
            width,
            retained_width: width,
            materialized_bytes,
        })
    }

    fn emit_empty_line(&mut self) -> Result<()> {
        (self.visit)(NormalizedLabelRowMetrics {
            width: 0,
            retained_width: 0,
            materialized_bytes: 0,
        })
    }

    fn finish(self) -> Result<Self::Output> {
        Ok(())
    }
}

struct MaterializedWrappedLabelSink {
    materialized: MaterializedLabelRows,
    current: String,
    word: String,
    policy: AsciiResourcePolicy,
}

impl MaterializedWrappedLabelSink {
    fn try_new(expected_lines: usize, policy: AsciiResourcePolicy) -> Result<Self> {
        Ok(Self {
            materialized: MaterializedLabelRows::try_new(expected_lines)?,
            current: String::new(),
            word: String::new(),
            policy,
        })
    }

    fn push_row(&mut self, row: String, width: usize) -> Result<()> {
        self.materialized.try_push_row(row, width, self.policy)
    }
}

impl WrappedLabelSink for MaterializedWrappedLabelSink {
    type Output = MaterializedLabelRows;

    fn push_word_unit(&mut self, text: &str) -> Result<()> {
        self.word
            .try_reserve(text.len())
            .map_err(|_| document_allocation_error())?;
        self.word.push_str(text);
        Ok(())
    }

    fn emit_word_chunk(&mut self, width: usize) -> Result<()> {
        let row = std::mem::take(&mut self.word);
        self.push_row(row, width)
    }

    fn append_word_to_line(&mut self, separator: bool) -> Result<()> {
        let extra = self
            .word
            .len()
            .checked_add(usize::from(separator))
            .ok_or_else(document_allocation_error)?;
        self.current
            .try_reserve(extra)
            .map_err(|_| document_allocation_error())?;
        if separator {
            self.current.push(' ');
        }
        self.current.push_str(&self.word);
        self.word.clear();
        Ok(())
    }

    fn emit_line(&mut self, width: usize) -> Result<()> {
        let row = std::mem::take(&mut self.current);
        self.push_row(row, width)
    }

    fn emit_empty_line(&mut self) -> Result<()> {
        self.push_row(String::new(), 0)
    }

    fn finish(self) -> Result<Self::Output> {
        debug_assert!(self.current.is_empty());
        debug_assert!(self.word.is_empty());
        Ok(self.materialized)
    }
}

fn process_wrapped_label<S>(
    raw: &str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    max_width: usize,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    sink: S,
) -> Result<S::Output>
where
    S: WrappedLabelSink,
{
    let mut wrapped =
        WrappedLabelProcessor::new(max_width, width_profile, policy, break_policy, sink);
    visit_selected_label_output(raw, selection, break_policy, policy, |_source, output| {
        match output {
            LabelOutputSegment::LineBreak => wrapped.finish_paragraph()?,
            LabelOutputSegment::Segment(segment) => {
                wrapped.push_segment(segment)?;
            }
        }
        Ok(())
    })?;
    wrapped.finish()
}

fn materialize_wrapped_label(
    raw: &str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    max_width: usize,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    expected_lines: usize,
) -> Result<MaterializedLabelRows> {
    process_wrapped_label(
        raw,
        selection,
        width_profile,
        max_width,
        break_policy,
        policy,
        MaterializedWrappedLabelSink::try_new(expected_lines, policy)?,
    )
}

fn visit_selected_label_output(
    raw: &str,
    selection: LabelSelection,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    mut visit: impl FnMut(Option<NormalizedSegment<'_>>, LabelOutputSegment<'_>) -> Result<()>,
) -> Result<()> {
    let mut offset = 0usize;
    visit_label_tokens(raw, break_policy, |token| {
        with_token_trim_text(token, |trim_text| {
            let token_start = offset;
            let token_end = offset
                .checked_add(trim_text.len())
                .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
            offset = token_end;
            let selected = match selection {
                LabelSelection::All => 0..trim_text.len(),
                LabelSelection::Range { start, end } => {
                    let kept_start = start.max(token_start);
                    let kept_end = end.min(token_end);
                    if kept_start >= kept_end {
                        return Ok(());
                    }
                    kept_start - token_start..kept_end - token_start
                }
            };

            match token {
                LabelToken::AuthoredBreak(_) => {
                    debug_assert_eq!(selected, (0..trim_text.len()));
                    visit(None, LabelOutputSegment::LineBreak)
                }
                LabelToken::Segment(source_segment) => {
                    let selected = &trim_text[selected];
                    let mut emit = |segment: NormalizedSegment<'_>| {
                        let output = match (segment.kind, break_policy) {
                            (NormalizedSegmentKind::LineBreak, LabelBreakPolicy::VisibleLine) => {
                                LabelOutputSegment::Segment(NormalizedSegment {
                                    kind: NormalizedSegmentKind::VisibleEscape('\n'),
                                    ..segment
                                })
                            }
                            (NormalizedSegmentKind::LineBreak, _) => LabelOutputSegment::LineBreak,
                            _ => LabelOutputSegment::Segment(segment),
                        };
                        visit(Some(source_segment), output)
                    };
                    if selected.len() == trim_text.len() {
                        emit(source_segment)
                    } else {
                        visit_normalized_segments(selected, emit)
                    }
                }
            }
        })
    })
}

fn visit_label_tokens<E>(
    raw: &str,
    break_policy: LabelBreakPolicy,
    mut visit: impl FnMut(LabelToken<'_>) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    if !matches!(break_policy, LabelBreakPolicy::MermaidLabelBreaks) {
        return visit_normalized_segments(raw, |segment| visit(LabelToken::Segment(segment)));
    }

    let mut chunk_start = 0usize;
    let mut index = 0usize;
    while index < raw.len() {
        let label_break_end = html_break_end(raw, index).or_else(|| {
            raw[index..]
                .starts_with("\\n")
                .then_some(index.saturating_add(2))
        });
        if let Some(end) = label_break_end {
            visit_normalized_segments(&raw[chunk_start..index], |segment| {
                visit(LabelToken::Segment(segment))
            })?;
            visit(LabelToken::AuthoredBreak(&raw[index..end]))?;
            index = end;
            chunk_start = end;
            continue;
        }

        let Some(ch) = raw[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
    }
    visit_normalized_segments(&raw[chunk_start..], |segment| {
        visit(LabelToken::Segment(segment))
    })
}

fn with_token_trim_text<T, E>(
    token: LabelToken<'_>,
    visit: impl FnOnce(&str) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    match token {
        LabelToken::Segment(segment) => {
            let mut buffer = [0u8; 10];
            visit(segment.text(&mut buffer))
        }
        LabelToken::AuthoredBreak(source) => visit(source),
    }
}

fn checked_add(
    resources: &ResourceContext,
    id: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(id))
}

fn checked_add_with_policy(
    policy: AsciiResourcePolicy,
    id: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right).ok_or_else(|| policy.overflow(id))
}

fn document_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
    }
}

fn invalid_label_extent_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "terminal_text",
        feature: "label extent planning",
    }
}

#[cfg(test)]
pub(crate) fn try_build_normalized_label_lines_with_probe(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &mut ResourceContext,
    materialized: &std::cell::Cell<bool>,
) -> Result<Option<NormalizedLabelLines>> {
    try_build_normalized_label_lines_impl(raw, width_profile, trim, wrap_width, resources, || {
        materialized.set(true)
    })
}
