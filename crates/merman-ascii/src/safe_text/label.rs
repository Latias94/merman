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
#[cfg(test)]
pub(crate) fn try_build_normalized_label_lines(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    resources: &ResourceContext,
) -> Result<Option<NormalizedLabelLines>> {
    resources.transaction(|resources| {
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
        plan.materialize(raw, resources).map(Some)
    })
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

#[derive(Debug, Clone, Copy)]
struct LabelReplay<'a> {
    raw: &'a str,
    selection: LabelSelection,
    width_profile: TerminalWidthProfile,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeferredLabelPiece<'a> {
    fragment: DeferredLabelFragment<'a>,
    display_width: u8,
    plain_bytes: usize,
    html_bytes: usize,
    replay_work_units: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredLabelFragment<'a> {
    Borrowed(&'a str),
    Scalar(char),
    VisibleEscape(char),
}

impl DeferredLabelFragment<'_> {
    fn try_visit(self, visit: &mut dyn FnMut(&str) -> Result<()>) -> Result<()> {
        match self {
            Self::Borrowed(value) => visit(value),
            Self::Scalar(value) => {
                let mut buffer = [0u8; 4];
                visit(value.encode_utf8(&mut buffer))
            }
            Self::VisibleEscape(value) => {
                let mut buffer = [0u8; 10];
                visit(super::normalization::visible_escape(value, &mut buffer))
            }
        }
    }
}

impl DeferredLabelPiece<'_> {
    pub(super) const fn display_width(self) -> usize {
        self.display_width as usize
    }

    pub(super) const fn replay_work_units(self) -> usize {
        self.replay_work_units
    }

    pub(super) const fn encoded_bytes(self, html: bool) -> usize {
        if html {
            self.html_bytes
        } else {
            self.plain_bytes
        }
    }

    pub(super) fn try_visit(self, visit: &mut dyn FnMut(&str) -> Result<()>) -> Result<()> {
        self.fragment.try_visit(visit)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DeferredLabelRow<'a> {
    pieces: Vec<DeferredLabelPiece<'a>>,
    width: usize,
}

impl<'a> DeferredLabelRow<'a> {
    pub(super) fn pieces(&self) -> &[DeferredLabelPiece<'a>] {
        &self.pieces
    }

    pub(super) const fn width(&self) -> usize {
        self.width
    }
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
    terminal_projection_lossy: bool,
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

    /// Returns true when terminal normalization replaced authored scalars with visible escapes.
    pub(crate) const fn terminal_projection_is_lossy(self) -> bool {
        self.terminal_projection_lossy
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

    #[cfg(test)]
    pub(crate) fn try_visit_row_metrics(
        self,
        raw: &str,
        resources: &ResourceContext,
        visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
    ) -> Result<()> {
        self.try_visit_row_metrics_with_checkpoint(
            raw,
            resources,
            || checkpoint_resources(resources),
            visit,
        )
    }

    pub(crate) fn try_visit_row_metrics_with_checkpoint(
        self,
        raw: &str,
        resources: &ResourceContext,
        mut checkpoint: impl FnMut() -> Result<()>,
        visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            self.try_visit_row_metrics_transactional(raw, resources, &mut checkpoint, visit)
        })
    }

    fn try_visit_row_metrics_transactional(
        self,
        raw: &str,
        resources: &ResourceContext,
        checkpoint: &mut impl FnMut() -> Result<()>,
        visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
    ) -> Result<()> {
        resources.charge_layout_work(self.replay_work_units)?;
        visit_label_row_metrics(
            LabelReplay {
                raw,
                selection: self.selection,
                width_profile: self.width_profile,
                break_policy: self.break_policy,
                policy: self.policy,
            },
            self.wrap_width,
            checkpoint,
            visit,
        )
    }

    pub(crate) fn materialize(
        self,
        raw: &str,
        resources: &ResourceContext,
    ) -> Result<NormalizedLabelLines> {
        self.materialize_with_checkpoint(raw, resources, || checkpoint_resources(resources))
    }

    pub(crate) fn materialize_with_checkpoint(
        self,
        raw: &str,
        resources: &ResourceContext,
        checkpoint: impl FnMut() -> Result<()>,
    ) -> Result<NormalizedLabelLines> {
        resources.transaction(|resources| {
            resources.charge_layout_work(self.replay_work_units)?;
            self.materialize_impl(raw, checkpoint)
        })
    }

    pub(crate) fn materialize_after_admission(self, raw: &str) -> Result<NormalizedLabelLines> {
        self.materialize_impl(raw, || Ok(()))
    }

    pub(crate) fn materialize_after_admission_with_checkpoint(
        self,
        raw: &str,
        checkpoint: impl FnMut() -> Result<()>,
    ) -> Result<NormalizedLabelLines> {
        self.materialize_impl(raw, checkpoint)
    }

    pub(super) fn try_deferred_rows<'a>(
        self,
        raw: &'a str,
        resources: &ResourceContext,
    ) -> Result<Vec<DeferredLabelRow<'a>>> {
        resources.transaction(|resources| {
            if self.wrap_width.is_some() {
                return Err(invalid_label_extent_plan());
            }
            resources.charge_layout_work(self.replay_work_units)?;

            let mut rows = Vec::new();
            rows.try_reserve_exact(self.output_metrics.line_count)
                .map_err(|_| document_allocation_error())?;
            let mut pieces = Vec::new();
            let mut row_width = 0usize;
            visit_selected_label_output_with_checkpoint(
                raw,
                self.selection,
                self.break_policy,
                self.policy,
                &mut || checkpoint_resources(resources),
                |_source, output| {
                    match output {
                        LabelOutputSegment::LineBreak => {
                            rows.push(DeferredLabelRow {
                                pieces: std::mem::take(&mut pieces),
                                width: row_width,
                            });
                            row_width = 0;
                        }
                        LabelOutputSegment::Segment(segment) => {
                            let width = segment.display_width(self.width_profile);
                            let mut buffer = [0u8; 10];
                            let text = segment.text(&mut buffer);
                            let mut html_bytes = 0usize;
                            super::encode::visit_html_escaped_text(text, |fragment| {
                                html_bytes = checked_add_with_policy(
                                    self.policy,
                                    AsciiResourceLimitId::MaxOutputBytes,
                                    html_bytes,
                                    fragment.len(),
                                )?;
                                Ok(())
                            })?;
                            row_width = checked_add_with_policy(
                                self.policy,
                                AsciiResourceLimitId::MaxDocumentCells,
                                row_width,
                                width,
                            )?;
                            pieces
                                .try_reserve(1)
                                .map_err(|_| document_allocation_error())?;
                            pieces.push(DeferredLabelPiece {
                                fragment: deferred_label_fragment(raw, segment)?,
                                display_width: u8::try_from(width)
                                    .map_err(|_| invalid_label_extent_plan())?,
                                plain_bytes: text.len(),
                                html_bytes,
                                replay_work_units: text.len().max(1),
                            });
                        }
                    }
                    Ok(())
                },
            )?;
            rows.push(DeferredLabelRow {
                pieces,
                width: row_width,
            });
            if rows.len() != self.output_metrics.line_count
                || rows.iter().map(DeferredLabelRow::width).max().unwrap_or(0)
                    != self.output_metrics.max_width
            {
                return Err(invalid_label_extent_plan());
            }
            Ok(rows)
        })
    }

    fn materialize_impl(
        self,
        raw: &str,
        mut checkpoint: impl FnMut() -> Result<()>,
    ) -> Result<NormalizedLabelLines> {
        let mut materialized = match self.wrap_width {
            Some(max_width) => materialize_wrapped_label(
                LabelReplay {
                    raw,
                    selection: self.selection,
                    width_profile: self.width_profile,
                    break_policy: self.break_policy,
                    policy: self.policy,
                },
                max_width,
                self.output_metrics.line_count,
                &mut checkpoint,
            )?,
            None => materialize_label(
                raw,
                self.selection,
                self.source_metrics,
                self.width_profile,
                self.break_policy,
                self.policy,
                &mut checkpoint,
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
}

fn deferred_label_fragment<'a>(
    raw: &'a str,
    segment: NormalizedSegment<'_>,
) -> Result<DeferredLabelFragment<'a>> {
    match segment.kind {
        NormalizedSegmentKind::Grapheme(value) => {
            let raw_start = raw.as_ptr() as usize;
            let raw_end = raw_start
                .checked_add(raw.len())
                .ok_or_else(invalid_label_extent_plan)?;
            let value_start = value.as_ptr() as usize;
            let value_end = value_start
                .checked_add(value.len())
                .ok_or_else(invalid_label_extent_plan)?;
            if raw_start <= value_start && value_end <= raw_end {
                let start = value_start - raw_start;
                let end = value_end - raw_start;
                return raw
                    .get(start..end)
                    .map(DeferredLabelFragment::Borrowed)
                    .ok_or_else(invalid_label_extent_plan);
            }
            let mut chars = value.chars();
            let scalar = chars.next().ok_or_else(invalid_label_extent_plan)?;
            if chars.next().is_some() {
                return Err(invalid_label_extent_plan());
            }
            Ok(DeferredLabelFragment::Scalar(scalar))
        }
        NormalizedSegmentKind::VisibleEscape(value) => {
            Ok(DeferredLabelFragment::VisibleEscape(value))
        }
        NormalizedSegmentKind::LineBreak => Err(invalid_label_extent_plan()),
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
    try_plan_normalized_label_lines_with_policy_and_checkpoint(
        raw,
        width_profile,
        trim,
        wrap_width,
        break_policy,
        resources,
        || checkpoint_resources(resources),
    )
}

pub(crate) fn try_plan_normalized_label_lines_with_policy_and_checkpoint(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<Option<NormalizedLabelPlan>> {
    resources.transaction(|resources| {
        try_plan_normalized_label_lines_with_policy_transactional(
            raw,
            width_profile,
            trim,
            wrap_width,
            break_policy,
            resources,
            &mut checkpoint,
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
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<NormalizedLabelPlan>> {
    checkpoint()?;
    let selection =
        match normalized_label_selection(raw, trim, break_policy, resources, checkpoint)? {
            Some(selection) => selection,
            None => return Ok(None),
        };
    let source_preflight = preflight_label(
        raw,
        selection,
        width_profile,
        break_policy,
        resources,
        checkpoint,
    )?;
    let source_metrics = source_preflight.metrics;
    checkpoint()?;
    let replay_work_units = resources.checked_work_add(
        raw.len().max(1),
        resources.checked_work_add(source_metrics.document_cells, source_metrics.line_count)?,
    )?;

    let output_metrics = if let Some(max_width) = wrap_width {
        resources.charge_layout_work(replay_work_units)?;
        measure_label_output_metrics(
            LabelReplay {
                raw,
                selection,
                width_profile,
                break_policy,
                policy: resources.policy(),
            },
            Some(max_width),
            checkpoint,
        )?
    } else {
        source_metrics
    };

    Ok(Some(NormalizedLabelPlan {
        selection,
        source_metrics,
        output_metrics,
        terminal_projection_lossy: source_preflight.terminal_projection_lossy,
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
    let mut checkpoint = || checkpoint_resources(resources);
    let selection = match normalized_label_selection(
        raw,
        trim,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
        &mut checkpoint,
    )? {
        Some(selection) => selection,
        None => return Ok(None),
    };
    Ok(Some(
        preflight_label(
            raw,
            selection,
            width_profile,
            LabelBreakPolicy::MermaidLabelBreaks,
            resources,
            &mut checkpoint,
        )?
        .metrics,
    ))
}

#[derive(Debug, Clone, Copy)]
struct NormalizedLabelPreflight {
    metrics: NormalizedLabelMetrics,
    terminal_projection_lossy: bool,
}

fn normalized_label_selection(
    raw: &str,
    trim: bool,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<LabelSelection>> {
    if !trim {
        return Ok(Some(LabelSelection::All));
    }
    resources.charge_layout_work(raw.len().max(1))?;

    let mut offset = 0usize;
    let mut start = None;
    let mut end = 0usize;
    let mut scalar_iteration = 0usize;
    visit_label_tokens(raw, break_policy, checkpoint, |token, checkpoint| {
        with_token_trim_text(token, |text| {
            for (relative, ch) in text.char_indices() {
                if scalar_iteration.is_multiple_of(LABEL_TOKEN_CHECKPOINT_INTERVAL) {
                    checkpoint()?;
                }
                scalar_iteration = scalar_iteration.wrapping_add(1);
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
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<NormalizedLabelPreflight> {
    resources.charge_layout_work(raw.len().max(1))?;
    let mut materialized_bytes = 0usize;
    let mut document_cells = 0usize;
    let mut line_count = 1usize;
    let mut line_width = 0usize;
    let mut max_width = 0usize;
    let mut terminal_projection_lossy = false;
    let policy = resources.policy();

    visit_selected_label_output_with_checkpoint(
        raw,
        selection,
        break_policy,
        policy,
        checkpoint,
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
                    terminal_projection_lossy |=
                        matches!(segment.kind, NormalizedSegmentKind::VisibleEscape(_));
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

    Ok(NormalizedLabelPreflight {
        metrics: NormalizedLabelMetrics {
            materialized_bytes,
            document_cells,
            line_count,
            max_width,
        },
        terminal_projection_lossy,
    })
}

fn materialize_label(
    raw: &str,
    selection: LabelSelection,
    metrics: NormalizedLabelMetrics,
    width_profile: TerminalWidthProfile,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<MaterializedLabelRows> {
    let mut materialized = MaterializedLabelRows::try_new(metrics.line_count)?;
    let mut current = String::new();
    let mut current_width = 0usize;
    visit_selected_label_output_with_checkpoint(
        raw,
        selection,
        break_policy,
        policy,
        checkpoint,
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
    replay: LabelReplay<'_>,
    wrap_width: Option<usize>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<NormalizedLabelMetrics> {
    let mut metrics = NormalizedLabelMetrics::EMPTY;
    visit_label_row_metrics(replay, wrap_width, checkpoint, |row| {
        metrics.try_include_row(row, replay.policy)
    })?;
    Ok(metrics)
}

fn visit_label_row_metrics(
    replay: LabelReplay<'_>,
    wrap_width: Option<usize>,
    checkpoint: &mut impl FnMut() -> Result<()>,
    mut visit: impl FnMut(NormalizedLabelRowMetrics) -> Result<()>,
) -> Result<()> {
    let Some(max_width) = wrap_width else {
        let mut line_width = 0usize;
        let mut retained_width = 0usize;
        let mut materialized_bytes = 0usize;
        visit_selected_label_output_with_checkpoint(
            replay.raw,
            replay.selection,
            replay.break_policy,
            replay.policy,
            checkpoint,
            |_source, output| {
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
                            replay.policy,
                            AsciiResourceLimitId::MaxOutputBytes,
                            materialized_bytes,
                            text.len(),
                        )?;
                        line_width = checked_add_with_policy(
                            replay.policy,
                            AsciiResourceLimitId::MaxDocumentCells,
                            line_width,
                            segment.display_width(replay.width_profile),
                        )?;
                        if text != " " {
                            retained_width = line_width;
                        }
                    }
                }
                Ok(())
            },
        )?;
        return visit(NormalizedLabelRowMetrics {
            width: line_width,
            retained_width,
            materialized_bytes,
        });
    };

    process_wrapped_label(
        replay,
        max_width,
        checkpoint,
        WrappedWidthSink {
            visit,
            policy: replay.policy,
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
    replay: LabelReplay<'_>,
    max_width: usize,
    checkpoint: &mut impl FnMut() -> Result<()>,
    sink: S,
) -> Result<S::Output>
where
    S: WrappedLabelSink,
{
    let mut wrapped = WrappedLabelProcessor::new(
        max_width,
        replay.width_profile,
        replay.policy,
        replay.break_policy,
        sink,
    );
    visit_selected_label_output_with_checkpoint(
        replay.raw,
        replay.selection,
        replay.break_policy,
        replay.policy,
        checkpoint,
        |_source, output| {
            match output {
                LabelOutputSegment::LineBreak => wrapped.finish_paragraph()?,
                LabelOutputSegment::Segment(segment) => {
                    wrapped.push_segment(segment)?;
                }
            }
            Ok(())
        },
    )?;
    wrapped.finish()
}

fn materialize_wrapped_label(
    replay: LabelReplay<'_>,
    max_width: usize,
    expected_lines: usize,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<MaterializedLabelRows> {
    process_wrapped_label(
        replay,
        max_width,
        checkpoint,
        MaterializedWrappedLabelSink::try_new(expected_lines, replay.policy)?,
    )
}

fn visit_selected_label_output_with_checkpoint(
    raw: &str,
    selection: LabelSelection,
    break_policy: LabelBreakPolicy,
    policy: AsciiResourcePolicy,
    checkpoint: &mut impl FnMut() -> Result<()>,
    mut visit: impl FnMut(Option<NormalizedSegment<'_>>, LabelOutputSegment<'_>) -> Result<()>,
) -> Result<()> {
    let mut offset = 0usize;
    visit_label_tokens(raw, break_policy, checkpoint, |token, checkpoint| {
        with_token_trim_text(token, |trim_text| {
            checkpoint()?;
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
                        checkpoint()?;
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

const LABEL_TOKEN_CHECKPOINT_INTERVAL: usize = 64;

fn checkpoint_resources(resources: &ResourceContext) -> Result<()> {
    resources.check(
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        resources.layout_work_used(),
    )
}

fn visit_label_tokens<F>(
    raw: &str,
    break_policy: LabelBreakPolicy,
    checkpoint: &mut F,
    mut visit: impl FnMut(LabelToken<'_>, &mut F) -> Result<()>,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    if !matches!(break_policy, LabelBreakPolicy::MermaidLabelBreaks) {
        checkpoint()?;
        return visit_normalized_segments(raw, |segment| {
            visit(LabelToken::Segment(segment), checkpoint)
        });
    }

    let mut chunk_start = 0usize;
    let mut index = 0usize;
    let mut scalar_iteration = 0usize;
    while index < raw.len() {
        if scalar_iteration.is_multiple_of(LABEL_TOKEN_CHECKPOINT_INTERVAL) {
            checkpoint()?;
        }
        scalar_iteration = scalar_iteration.wrapping_add(1);
        let label_break_end =
            html_break_end(raw, index).or_else(|| mermaid_escaped_newline_end(raw, index));
        if let Some(end) = label_break_end {
            visit_normalized_segments(&raw[chunk_start..index], |segment| {
                visit(LabelToken::Segment(segment), checkpoint)
            })?;
            visit(LabelToken::AuthoredBreak(&raw[index..end]), checkpoint)?;
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
        visit(LabelToken::Segment(segment), checkpoint)
    })
}

fn mermaid_escaped_newline_end(raw: &str, index: usize) -> Option<usize> {
    if !raw.get(index..)?.starts_with("\\n") {
        return None;
    }

    let preceding_backslashes = raw[..index]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count();
    (preceding_backslashes % 2 == 0).then_some(index.saturating_add(2))
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
mod cancellation_tests {
    use std::cell::Cell;

    use super::*;
    use crate::operation::AsciiExecution;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    #[test]
    fn admitted_label_materialization_checks_cancellation_inside_the_replay() {
        let raw = "A".repeat(128);
        let policy = AsciiResourcePolicy::default();
        let resources = ResourceContext::new(policy);
        let plan = try_plan_normalized_label_lines_with_policy(
            &raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            LabelBreakPolicy::VisibleLine,
            &resources,
        )
        .expect("the label plan should fit")
        .expect("non-trimmed text should retain a row");

        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let execution = AsciiExecution::new(&control, &policy);
        let checkpoints = Cell::new(0usize);
        let error = plan
            .materialize_after_admission_with_checkpoint(&raw, || {
                checkpoints.set(checkpoints.get() + 1);
                execution.checkpoint(OperationPhase::Layout)
            })
            .expect_err("materialization should stop during its replay pass");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(checkpoints.get(), 2);
    }
}
