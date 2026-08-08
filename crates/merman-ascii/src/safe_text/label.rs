use super::normalization::{NormalizedSegment, NormalizedSegmentKind, visit_normalized_segments};
use super::width::grapheme_display_width;
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy, ResourceContext,
};
use crate::text::{html_break_end, wrap_display_lines_with_profile};
use unicode_segmentation::UnicodeSegmentation;

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
/// grapheme, work, document-cell, and conservative output-byte bounds. Only an admitted label is
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
    let selection = match normalized_label_selection(raw, trim, resources)? {
        Some(selection) => selection,
        None => return Ok(None),
    };
    let metrics = preflight_label(raw, selection, width_profile, resources)?;

    resources.charge_document_cells(metrics.document_cells)?;
    resources.check(
        AsciiResourceLimitId::MaxOutputBytes,
        metrics.normalized_bytes,
    )?;
    if wrap_width.is_some() {
        let wrap_work = resources.checked_work_add(metrics.document_cells, metrics.line_count)?;
        resources.charge_layout_work(wrap_work.max(1))?;
    }

    before_materialize();
    let mut lines = materialize_label(raw, selection, metrics, resources.policy())?;
    if let Some(max_width) = wrap_width {
        lines = wrap_materialized_lines(lines, max_width, width_profile)?;
    }
    if lines.is_empty() {
        lines.push(String::new());
    }

    let width = if wrap_width.is_none() {
        metrics.max_width
    } else {
        lines
            .iter()
            .map(|line| checked_line_width(line, width_profile, resources))
            .try_fold(0usize, |maximum, width| {
                width.map(|width| maximum.max(width))
            })?
    };
    Ok(Some(NormalizedLabelLines { lines, width }))
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
struct LabelMetrics {
    normalized_bytes: usize,
    document_cells: usize,
    line_count: usize,
    max_width: usize,
}

fn normalized_label_selection(
    raw: &str,
    trim: bool,
    resources: &ResourceContext,
) -> Result<Option<LabelSelection>> {
    resources.charge_layout_work(raw.len().max(1))?;
    if !trim {
        return Ok(Some(LabelSelection::All));
    }

    let mut offset = 0usize;
    let mut start = None;
    let mut end = 0usize;
    visit_label_tokens(raw, |token| {
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
    resources: &ResourceContext,
) -> Result<LabelMetrics> {
    resources.charge_layout_work(raw.len().max(1))?;
    let mut normalized_bytes = 0usize;
    let mut document_cells = 0usize;
    let mut line_count = 1usize;
    let mut line_width = 0usize;
    let mut max_width = 0usize;
    let policy = resources.policy();

    visit_selected_label_output(raw, selection, policy, |source_segment, output| {
        if let Some(source_segment) = source_segment {
            source_segment.check_grapheme_budget(resources)?;
            resources.charge_layout_work(source_segment.layout_work())?;
        }

        match output {
            LabelOutputSegment::LineBreak => {
                resources.charge_layout_work(1)?;
                normalized_bytes = checked_add(
                    resources,
                    AsciiResourceLimitId::MaxOutputBytes,
                    normalized_bytes,
                    1,
                )?;
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
                normalized_bytes = checked_add(
                    resources,
                    AsciiResourceLimitId::MaxOutputBytes,
                    normalized_bytes,
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
    })?;
    max_width = max_width.max(line_width);

    Ok(LabelMetrics {
        normalized_bytes,
        document_cells,
        line_count,
        max_width,
    })
}

fn materialize_label(
    raw: &str,
    selection: LabelSelection,
    metrics: LabelMetrics,
    policy: AsciiResourcePolicy,
) -> Result<Vec<String>> {
    let mut normalized = String::new();
    normalized
        .try_reserve_exact(metrics.normalized_bytes)
        .map_err(|_| document_allocation_error())?;
    visit_selected_label_output(raw, selection, policy, |_source_segment, output| {
        match output {
            LabelOutputSegment::LineBreak => normalized.push('\n'),
            LabelOutputSegment::Segment(segment) => {
                let mut buffer = [0u8; 10];
                normalized.push_str(segment.text(&mut buffer));
            }
        }
        Ok::<(), AsciiError>(())
    })?;
    debug_assert_eq!(normalized.len(), metrics.normalized_bytes);

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(metrics.line_count)
        .map_err(|_| document_allocation_error())?;
    for line in normalized.split('\n') {
        let mut retained = String::new();
        retained
            .try_reserve_exact(line.len())
            .map_err(|_| document_allocation_error())?;
        retained.push_str(line);
        lines.push(retained);
    }
    debug_assert_eq!(lines.len(), metrics.line_count);
    Ok(lines)
}

fn wrap_materialized_lines(
    lines: Vec<String>,
    max_width: usize,
    width_profile: TerminalWidthProfile,
) -> Result<Vec<String>> {
    let mut wrapped = Vec::new();
    wrapped
        .try_reserve(lines.len())
        .map_err(|_| document_allocation_error())?;
    for line in lines {
        if line.is_empty() {
            wrapped.push(line);
            continue;
        }
        let chunks = wrap_display_lines_with_profile(&line, max_width, width_profile);
        wrapped
            .try_reserve(chunks.len())
            .map_err(|_| document_allocation_error())?;
        wrapped.extend(chunks);
    }
    Ok(wrapped)
}

fn checked_line_width(
    line: &str,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    line.graphemes(true).try_fold(0usize, |width, grapheme| {
        checked_add(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            width,
            grapheme_display_width(grapheme, profile),
        )
    })
}

fn visit_selected_label_output(
    raw: &str,
    selection: LabelSelection,
    policy: AsciiResourcePolicy,
    mut visit: impl FnMut(Option<NormalizedSegment<'_>>, LabelOutputSegment<'_>) -> Result<()>,
) -> Result<()> {
    let mut offset = 0usize;
    visit_label_tokens(raw, |token| {
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
                    visit_normalized_segments(selected, |segment| {
                        let output = if matches!(segment.kind, NormalizedSegmentKind::LineBreak) {
                            LabelOutputSegment::LineBreak
                        } else {
                            LabelOutputSegment::Segment(segment)
                        };
                        visit(Some(source_segment), output)
                    })
                }
            }
        })
    })
}

fn visit_label_tokens<E>(
    raw: &str,
    mut visit: impl FnMut(LabelToken<'_>) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
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

fn document_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
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
