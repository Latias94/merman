//! Flowchart-aware text metrics and Markdown measurement helpers.

use super::html_wrap::html_line_text_with_break;
use super::line_break::{
    HtmlBreakKind, html_break_spaces_segments,
    html_break_spaces_segments_with_metadata_and_checkpoints,
};
use super::{
    MermaidMarkdownWordType, TextMeasurer, TextMetrics, TextStyle, WrapMode, ceil_to_1_64_px,
    is_html_collapsible_ascii_whitespace, mermaid_markdown_to_lines,
    mermaid_xhtml_label_plain_text, round_to_1_64_px, trim_html_collapsible_ascii_whitespace, wrap,
};
use crate::environment::{
    BuiltinHtmlLineWidth, BuiltinTextMeasurementOperationCarrier, InlineHtmlMeasurementCarrier,
    TextMeasurementOperation,
};

mod xhtml_fonts;

#[cfg(any(feature = "diagram-block", feature = "diagram-class"))]
pub(crate) fn measure_unwrapped_xhtml_label_fragment(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    root_font_size: f64,
) -> TextMetrics {
    measure_xhtml_label_fragment_with_terminal_fonts(
        measurer,
        fragment,
        style,
        None,
        root_font_size,
    )
}

pub(crate) fn measure_xhtml_label_fragment_with_terminal_fonts(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    root_font_size: f64,
) -> TextMetrics {
    if fragment.starts_with("<p>")
        && let Some(plain) = mermaid_xhtml_label_plain_text(fragment)
        && !plain.contains('<')
    {
        return measurer.measure_wrapped(&plain, style, max_width, WrapMode::HtmlLike);
    }
    measure_html_with_document_root(measurer, fragment, style, max_width, root_font_size)
}

/// Preserves structured HTML whitespace and measurement requests with an explicit root size.
pub(crate) fn measure_html_with_document_root(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    root_font_size: f64,
) -> TextMetrics {
    let carrier = measurer
        .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
        .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
        .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque);
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        carrier,
        fragment,
        style,
        max_width,
        WrapMode::HtmlLike,
        Some(xhtml_fonts::TerminalFonts::new(style, root_font_size)),
    )
}

#[test]
fn html_document_root_context_preserves_plain_label_host_requests() {
    use std::sync::Mutex;

    #[derive(Default)]
    struct Requests(Mutex<Vec<(&'static str, String)>>);

    impl TextMeasurer for Requests {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(("measure", text.to_owned()));
            super::DeterministicTextMeasurer::default().measure(text, style)
        }

        fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.0.lock().unwrap().push(("html-width", text.to_owned()));
            super::DeterministicTextMeasurer::default().measure_html_line_width_px(text, style)
        }
    }

    let style = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };
    for html in ["<p>A  B</p>", "<p>A<br/>B</p>", "<p>A&nbsp;B</p>"] {
        let previous = Requests::default();
        let explicit_root = Requests::default();
        let expected = measure_html_with_inline_styles(
            &previous,
            html,
            &style,
            Some(27.0),
            WrapMode::HtmlLike,
        );
        let actual =
            measure_html_with_document_root(&explicit_root, html, &style, Some(27.0), 16.0);
        assert_eq!(
            (actual.width, actual.height, actual.line_count),
            (expected.width, expected.height, expected.line_count),
            "{html}"
        );
        assert_eq!(
            *explicit_root.0.lock().unwrap(),
            *previous.0.lock().unwrap(),
            "{html}"
        );
    }
}

// This input has already passed through the lightweight HTML parser. Do not use the generic text
// normalizer here: its Unicode trim would erase a final NBSP-only line before inline measurement.
fn normalized_preparsed_html_text_lines(text: &str) -> Vec<String> {
    let mut out = text.split('\n').map(str::to_string).collect::<Vec<_>>();

    while out.len() > 1
        && out
            .last()
            .is_some_and(|line| trim_html_collapsible_ascii_whitespace(line).is_empty())
    {
        out.pop();
    }

    if out.is_empty() {
        vec![String::new()]
    } else {
        out
    }
}

pub(crate) fn style_requests_bold_font_weight(style: &TextStyle) -> bool {
    let Some(w) = style.font_weight.as_deref() else {
        return false;
    };
    let w = w.trim();
    if w.is_empty() {
        return false;
    }
    let lower = w.to_ascii_lowercase();
    if lower == "bold" || lower == "bolder" {
        return true;
    }
    lower
        .parse::<f64>()
        .ok()
        .is_some_and(|n| n.is_finite() && n >= 600.0)
}

pub(crate) fn style_requests_italic_font_style(style: &TextStyle) -> bool {
    let Some(value) = style.font_style.as_deref() else {
        return false;
    };
    let value = value.trim().to_ascii_lowercase();
    value == "italic" || value.starts_with("italic ") || value.starts_with("oblique")
}

#[derive(Debug, Clone, Default)]
struct InlineTextRun {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    terminal_style: Option<std::sync::Arc<TextStyle>>,
    replaced_width_px: Option<f64>,
    ancestor_struts: Vec<InlineAncestorStrut>,
}

#[derive(Debug, Clone)]
struct InlineAncestorStrut {
    start: usize,
    end: usize,
    font_size: f64,
}

impl InlineTextRun {
    fn record_ancestor_strut(&mut self, start: usize, font_size: f64) {
        if font_size <= 0.0 {
            return;
        }
        let end = self.text.len();
        if let Some(previous) = self.ancestor_struts.last_mut()
            && previous.end == start
            && previous.font_size == font_size
        {
            previous.end = end;
        } else {
            self.ancestor_struts.push(InlineAncestorStrut {
                start,
                end,
                font_size,
            });
        }
    }

    fn ancestor_strut_height_px(&self) -> f64 {
        // Truncated collapsible tails retain no line-box contribution. Keep ranges separate
        // from font runs so ancestry changes cannot split shaping or observable host requests.
        self.ancestor_struts
            .iter()
            .take_while(|strut| strut.start < self.text.len())
            .fold(0.0_f64, |size, strut| size.max(strut.font_size))
            * 1.5
    }
}

fn push_inline_text_char(
    runs: &mut Vec<InlineTextRun>,
    ch: char,
    bold: bool,
    italic: bool,
    code: bool,
) {
    push_inline_text_char_with_terminal_style(runs, ch, bold, italic, code, None, 0.0);
}

fn push_inline_text_char_with_terminal_style(
    runs: &mut Vec<InlineTextRun>,
    ch: char,
    bold: bool,
    italic: bool,
    code: bool,
    terminal_style: Option<&std::sync::Arc<TextStyle>>,
    ancestor_strut_font_size: f64,
) {
    if let Some(run) = runs.last_mut().filter(|run| {
        run.replaced_width_px.is_none()
            && run.bold == bold
            && run.italic == italic
            && run.code == code
            && xhtml_fonts::same_font(
                run.terminal_style.as_deref(),
                terminal_style.map(|s| s.as_ref()),
            )
    }) {
        let start = run.text.len();
        run.text.push(ch);
        run.record_ancestor_strut(start, ancestor_strut_font_size);
    } else {
        let mut run = InlineTextRun {
            text: ch.to_string(),
            bold,
            italic,
            code,
            terminal_style: terminal_style.cloned(),
            replaced_width_px: None,
            ancestor_struts: Vec::new(),
        };
        run.record_ancestor_strut(0, ancestor_strut_font_size);
        runs.push(run);
    }
}

fn inline_text_style(base: &TextStyle, bold: bool, italic: bool, code: bool) -> TextStyle {
    let mut style = base.clone();
    if bold && !style_requests_bold_font_weight(&style) {
        style.font_weight = Some("700".to_string());
    }
    if italic && !style_requests_italic_font_style(&style) {
        style.font_style = Some("italic".to_string());
    }
    if code {
        style.font_family = Some("monospace".to_string());
    }
    style
}

fn resolved_inline_text_style<'a>(
    run: &'a InlineTextRun,
    base: &TextStyle,
) -> std::borrow::Cow<'a, TextStyle> {
    match &run.terminal_style {
        Some(style) => std::borrow::Cow::Borrowed(style.as_ref()),
        None => std::borrow::Cow::Owned(inline_text_style(base, run.bold, run.italic, run.code)),
    }
}

/// Matches createText's explicit word attributes, not the enclosing text's inheritance.
pub(crate) fn svg_markdown_word_style(
    base: &TextStyle,
    kind: MermaidMarkdownWordType,
) -> std::borrow::Cow<'_, TextStyle> {
    let bold = kind == MermaidMarkdownWordType::Strong;
    let italic = kind == MermaidMarkdownWordType::Em;
    let weight_matches = match base.font_weight.as_deref() {
        None | Some("normal" | "400") => !bold,
        Some("bold" | "700") => bold,
        _ => false,
    };
    let style_matches = match base.font_style.as_deref() {
        None | Some("normal") => !italic,
        Some("italic") => italic,
        _ => false,
    };
    if weight_matches && style_matches {
        return std::borrow::Cow::Borrowed(base);
    }
    let mut style = base.clone();
    style.font_weight = Some(if bold { "700" } else { "normal" }.to_owned());
    style.font_style = Some(if italic { "italic" } else { "normal" }.to_owned());
    std::borrow::Cow::Owned(style)
}

fn measure_inline_run_width_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    text: &str,
    style: &TextStyle,
    wrap_mode: WrapMode,
    svg_advance: bool,
) -> f64 {
    if wrap_mode == WrapMode::HtmlLike {
        measurer.measure_html_line_width_px(text, style)
    } else if svg_advance {
        measurer.measure_svg_text_computed_length_px(text, style)
    } else {
        measurer.measure_wrapped(text, style, None, wrap_mode).width
    }
}

fn measure_inline_runs_width_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    style: &TextStyle,
    wrap_mode: WrapMode,
    svg_advance: bool,
) -> f64 {
    runs.iter()
        .filter(|run| !run.text.is_empty())
        .take_while(|_| !measurer.cancellation_requested())
        .map(|run| {
            if let Some(width) = run.replaced_width_px {
                return width;
            }
            let run_style = if let Some(terminal) = &run.terminal_style {
                std::borrow::Cow::Borrowed(terminal.as_ref())
            } else if svg_advance && wrap_mode != WrapMode::HtmlLike {
                let kind = if run.bold {
                    MermaidMarkdownWordType::Strong
                } else if run.italic {
                    MermaidMarkdownWordType::Em
                } else {
                    MermaidMarkdownWordType::Normal
                };
                svg_markdown_word_style(style, kind)
            } else {
                std::borrow::Cow::Owned(inline_text_style(style, run.bold, run.italic, run.code))
            };
            measure_inline_run_width_px(measurer, &run.text, &run_style, wrap_mode, svg_advance)
        })
        .sum()
}

#[derive(Debug, Clone, Copy)]
struct InlineRunFragment {
    run_index: usize,
    start: usize,
    end: usize,
    ancestor_strut_font_size: f64,
}

#[derive(Debug, Clone, Copy)]
struct InlineBreakSegment {
    fragment_start: usize,
    fragment_end: usize,
    end: HtmlBreakKind,
}

#[derive(Debug, Default)]
struct InlineHtmlPlanningStats {
    #[cfg(test)]
    source_bytes: usize,
    #[cfg(test)]
    run_visits: usize,
    #[cfg(test)]
    break_segments: usize,
    #[cfg(test)]
    fragment_refs: usize,
    #[cfg(test)]
    fragment_capacity_refs: usize,
    #[cfg(test)]
    line_plan_updates: usize,
    #[cfg(test)]
    line_plan_bytes: usize,
    #[cfg(test)]
    merged_scratch_bytes: usize,
    #[cfg(test)]
    fragment_measure_visits: usize,
    #[cfg(test)]
    measurement_payload_copy_bytes: usize,
    #[cfg(test)]
    source_range: Option<(usize, usize)>,
    #[cfg(test)]
    opaque_style_group_index_bytes: usize,
    #[cfg(test)]
    backend_requests: usize,
    #[cfg(test)]
    backend_request_bytes: usize,
    #[cfg(test)]
    builtin_stream_bytes: usize,
    #[cfg(test)]
    builtin_stream_scalars: usize,
    #[cfg(test)]
    builtin_profile_initializations: usize,
    #[cfg(test)]
    builtin_profile_cache_bytes: usize,
    #[cfg(test)]
    builtin_group_state_copies: usize,
    #[cfg(test)]
    builtin_checkpoint_copies: usize,
}

impl InlineHtmlPlanningStats {
    fn record_source(&mut self, bytes: usize, runs: usize) {
        #[cfg(test)]
        {
            self.source_bytes += bytes;
            // One pass computes the exact concatenation capacity; the second fills it.
            self.run_visits += runs.saturating_mul(2);
        }
        #[cfg(not(test))]
        let _ = (bytes, runs);
    }

    fn record_source_range(&mut self, source: &str) {
        #[cfg(test)]
        {
            let start = source.as_ptr() as usize;
            self.source_range = Some((start, start.saturating_add(source.len())));
        }
        #[cfg(not(test))]
        let _ = source;
    }

    fn record_source_slice(&mut self, text: &str) {
        #[cfg(test)]
        {
            if text.is_empty() {
                return;
            }
            let start = text.as_ptr() as usize;
            let end = start.saturating_add(text.len());
            let is_borrowed = self.source_range.is_some_and(|(source_start, source_end)| {
                start >= source_start && end <= source_end
            });
            if !is_borrowed {
                self.measurement_payload_copy_bytes += text.len();
            }
        }
        #[cfg(not(test))]
        let _ = text;
    }

    fn record_indexed_run(&mut self) {
        #[cfg(test)]
        {
            self.run_visits += 1;
        }
    }

    fn record_segment(&mut self) {
        #[cfg(test)]
        {
            self.break_segments += 1;
        }
    }

    fn record_fragment(&mut self) {
        #[cfg(test)]
        {
            self.fragment_refs += 1;
        }
    }

    fn record_fragment_capacity(&mut self, capacity: usize) {
        #[cfg(test)]
        {
            self.fragment_capacity_refs += capacity;
        }
        #[cfg(not(test))]
        let _ = capacity;
    }

    fn record_line_plan(&mut self, bytes: usize) {
        #[cfg(test)]
        {
            self.line_plan_bytes += bytes;
        }
        #[cfg(not(test))]
        let _ = bytes;
    }

    fn record_line_plan_update(&mut self) {
        #[cfg(test)]
        {
            self.line_plan_updates += 1;
        }
    }

    fn record_fragment_measure_visits(&mut self, visits: usize) {
        #[cfg(test)]
        {
            self.fragment_measure_visits += visits;
        }
        #[cfg(not(test))]
        let _ = visits;
    }

    fn record_opaque_style_group_index(&mut self, capacity: usize) {
        #[cfg(test)]
        {
            self.opaque_style_group_index_bytes += capacity * std::mem::size_of::<usize>();
        }
        #[cfg(not(test))]
        let _ = capacity;
    }

    fn record_backend_request(&mut self, bytes: usize) {
        #[cfg(test)]
        {
            self.backend_requests += 1;
            self.backend_request_bytes += bytes;
        }
        #[cfg(not(test))]
        let _ = bytes;
    }

    fn record_builtin_stream(&mut self, text: &str) {
        #[cfg(test)]
        {
            self.builtin_stream_bytes += text.len();
            self.builtin_stream_scalars += text.chars().count();
        }
        #[cfg(not(test))]
        let _ = text;
    }

    fn record_builtin_profile_initialization(&mut self) {
        #[cfg(test)]
        {
            self.builtin_profile_initializations += 1;
        }
    }

    fn record_builtin_profile_cache(&mut self, bytes: usize) {
        #[cfg(test)]
        {
            self.builtin_profile_cache_bytes += bytes;
        }
        #[cfg(not(test))]
        let _ = bytes;
    }

    fn record_builtin_group_state_copy(&mut self) {
        #[cfg(test)]
        {
            self.builtin_group_state_copies += 1;
        }
    }

    fn record_builtin_checkpoint_copy(&mut self) {
        #[cfg(test)]
        {
            self.builtin_checkpoint_copies += 1;
        }
    }

    #[cfg(test)]
    fn logical_temporary_bytes(&self) -> usize {
        self.source_bytes
            + self.break_segments * std::mem::size_of::<&str>()
            + self.fragment_capacity_refs * std::mem::size_of::<InlineRunFragment>()
            + self.break_segments * std::mem::size_of::<InlineBreakSegment>()
            + self.line_plan_bytes
            + self.merged_scratch_bytes
            + self.opaque_style_group_index_bytes
            + self.builtin_profile_cache_bytes
    }
}

#[derive(Debug)]
struct InlineBreakPlan {
    source: String,
    fragments: Vec<InlineRunFragment>,
    segments: Vec<InlineBreakSegment>,
    opaque_style_group_ends: Vec<usize>,
}

fn index_inline_run_breaks(
    runs: &[InlineTextRun],
    stats: &mut InlineHtmlPlanningStats,
) -> InlineBreakPlan {
    let source_bytes = runs.iter().map(|run| run.text.len()).sum();
    stats.record_source(source_bytes, runs.len());
    if source_bytes == 0 {
        stats.record_segment();
        let source = String::new();
        stats.record_source_range(&source);
        return InlineBreakPlan {
            source,
            fragments: Vec::new(),
            segments: vec![InlineBreakSegment {
                fragment_start: 0,
                fragment_end: 0,
                end: HtmlBreakKind::Mandatory,
            }],
            opaque_style_group_ends: Vec::new(),
        };
    }

    // Unicode line breaking needs the logical text as one string. Build it once, then advance a
    // single run cursor across the resulting byte offsets. A run is never restarted for each
    // segment, and segment text is never copied into owned run objects.
    let mut text = String::with_capacity(source_bytes);
    for run in runs {
        text.push_str(&run.text);
    }
    stats.record_source_range(&text);
    let break_segments =
        match html_break_spaces_segments_with_metadata_and_checkpoints(&text, &mut || {
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(segments) => segments,
            Err(error) => match error {},
        };

    let fragment_capacity = runs
        .len()
        .saturating_add(break_segments.len().saturating_sub(1));
    stats.record_fragment_capacity(fragment_capacity);
    let mut fragments = Vec::with_capacity(fragment_capacity);
    let mut segments = Vec::with_capacity(break_segments.len());
    let mut segment_start = 0usize;
    let mut run_index = 0usize;
    let mut run_start = 0usize;
    let mut strut_index = 0usize;

    for break_segment in break_segments {
        let segment_end = segment_start + break_segment.text.len();
        let fragment_start = fragments.len();
        while run_index < runs.len() && run_start < segment_end {
            let run = &runs[run_index];
            let run_end = run_start + run.text.len();
            stats.record_indexed_run();
            if run_end <= segment_start {
                run_start = run_end;
                run_index += 1;
                strut_index = 0;
                continue;
            }

            let overlap_start = segment_start.max(run_start);
            let overlap_end = segment_end.min(run_end);
            if overlap_start < overlap_end {
                let local_start = overlap_start - run_start;
                let local_end = overlap_end - run_start;
                let mut ancestor_strut_font_size = 0.0_f64;
                // Both cursors advance with the source. A range spanning several fragments is
                // visited once per overlap, never rescanned from the beginning of its font run.
                while let Some(strut) = run.ancestor_struts.get(strut_index) {
                    if strut.start >= local_end {
                        break;
                    }
                    if strut.end > local_start {
                        ancestor_strut_font_size = ancestor_strut_font_size.max(strut.font_size);
                    }
                    if strut.end > local_end {
                        break;
                    }
                    strut_index += 1;
                }
                fragments.push(InlineRunFragment {
                    run_index,
                    start: overlap_start,
                    end: overlap_end,
                    ancestor_strut_font_size,
                });
                stats.record_fragment();
            }

            if run_end <= segment_end {
                run_start = run_end;
                run_index += 1;
                strut_index = 0;
            } else {
                break;
            }
        }
        segments.push(InlineBreakSegment {
            fragment_start,
            fragment_end: fragments.len(),
            end: break_segment.end,
        });
        stats.record_segment();
        segment_start = segment_end;
    }

    InlineBreakPlan {
        source: text,
        fragments,
        segments,
        opaque_style_group_ends: Vec::new(),
    }
}

fn index_opaque_inline_style_groups(
    runs: &[InlineTextRun],
    breaks: &mut InlineBreakPlan,
    stats: &mut InlineHtmlPlanningStats,
) {
    breaks.opaque_style_group_ends = vec![0; breaks.fragments.len()];
    stats.record_opaque_style_group_index(breaks.opaque_style_group_ends.capacity());
    let mut group_end = breaks.fragments.len();
    for index in (0..breaks.fragments.len()).rev() {
        if index + 1 == breaks.fragments.len()
            || !same_inline_style(
                &runs[breaks.fragments[index].run_index],
                &runs[breaks.fragments[index + 1].run_index],
            )
        {
            group_end = index + 1;
        }
        breaks.opaque_style_group_ends[index] = group_end;
        stats.record_fragment_measure_visits(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InlineStyleKey {
    bold: bool,
    italic: bool,
    code: bool,
}

impl InlineStyleKey {
    const fn from_run(run: &InlineTextRun) -> Self {
        Self {
            bold: run.bold,
            italic: run.italic,
            code: run.code,
        }
    }

    fn resolve(self, base: &TextStyle) -> TextStyle {
        inline_text_style(base, self.bold, self.italic, self.code)
    }

    const fn index(self) -> usize {
        (self.bold as usize) | ((self.italic as usize) << 1) | ((self.code as usize) << 2)
    }
}

fn same_inline_style(left: &InlineTextRun, right: &InlineTextRun) -> bool {
    left.replaced_width_px.is_none()
        && right.replaced_width_px.is_none()
        && match (&left.terminal_style, &right.terminal_style) {
            (Some(left), Some(right)) => xhtml_fonts::same_font(Some(left), Some(right)),
            (None, None) => InlineStyleKey::from_run(left) == InlineStyleKey::from_run(right),
            _ => false,
        }
}

fn measure_inline_fragments_width_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    breaks: &InlineBreakPlan,
    fragment_start: usize,
    fragment_end: usize,
    style: &TextStyle,
    stats: &mut InlineHtmlPlanningStats,
) -> f64 {
    measure_inline_fragments_with_break_width_px(
        measurer,
        runs,
        breaks,
        fragment_start,
        fragment_end,
        HtmlBreakKind::Ordinary,
        style,
        stats,
    )
}

#[allow(clippy::too_many_arguments)]
fn measure_inline_fragments_with_break_width_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    breaks: &InlineBreakPlan,
    fragment_start: usize,
    fragment_end: usize,
    selected_break: HtmlBreakKind,
    style: &TextStyle,
    stats: &mut InlineHtmlPlanningStats,
) -> f64 {
    // Keep every request observable. `TextMeasurer` may route to a stateful or fallible host, so
    // this shared planner cannot reuse widths unless the complete operation owns a private
    // built-in measurement carrier.
    let mut width = 0.0;
    let mut index = fragment_start;
    while index < fragment_end {
        stats.record_fragment_measure_visits(1);
        let first = breaks.fragments[index];
        let first_run = &runs[first.run_index];
        let end = breaks.opaque_style_group_ends[index].min(fragment_end);
        if let Some(advance) = first_run.replaced_width_px {
            width += advance;
            index = end;
            continue;
        }
        let last = breaks.fragments[end - 1];
        let text = &breaks.source[first.start..last.end];
        // The inserted glyph belongs to the run containing U+00AD. Measuring it together with
        // that complete style group preserves shaping; a separately measured hyphen is not an
        // additive substitute for the host's width of the resulting string.
        let text = html_line_text_with_break(
            text,
            end == fragment_end && selected_break == HtmlBreakKind::SoftHyphen,
        );
        stats.record_source_slice(&text);
        let run_style = resolved_inline_text_style(first_run, style);
        stats.record_backend_request(text.len());
        width += measurer.measure_html_line_width_px(&text, &run_style);
        index = end;
    }
    width
}

#[derive(Debug, Clone)]
struct BuiltinInlineGroupWidth {
    run_index: usize,
    width: BuiltinHtmlLineWidth,
}

struct BuiltinInlineWidthProfiles {
    carrier: InlineHtmlMeasurementCarrier,
    initial: [Option<BuiltinHtmlLineWidth>; 8],
    terminal: Vec<Option<BuiltinHtmlLineWidth>>,
}

impl BuiltinInlineWidthProfiles {
    fn new(carrier: InlineHtmlMeasurementCarrier, runs: &[InlineTextRun]) -> Self {
        debug_assert!(carrier.is_builtin());
        Self {
            carrier,
            initial: std::array::from_fn(|_| None),
            terminal: if runs.iter().any(|run| run.terminal_style.is_some()) {
                vec![None; runs.len()]
            } else {
                Vec::new()
            },
        }
    }

    fn begin(
        &mut self,
        run_index: usize,
        runs: &[InlineTextRun],
        base_style: &TextStyle,
        stats: &mut InlineHtmlPlanningStats,
    ) -> BuiltinHtmlLineWidth {
        let run = &runs[run_index];
        // The immutable run index is a complete, operation-local key for a resolved font.
        // Primitive profiles retain their fixed-size cache; terminal profiles add O(runs) space.
        let initial = if run.terminal_style.is_some() {
            &mut self.terminal[run_index]
        } else {
            &mut self.initial[InlineStyleKey::from_run(run).index()]
        };
        if initial.is_none() {
            let run_style = run.terminal_style.as_deref().map_or_else(
                || std::borrow::Cow::Owned(InlineStyleKey::from_run(run).resolve(base_style)),
                std::borrow::Cow::Borrowed,
            );
            *initial = Some(
                self.carrier
                    .begin_inline_html_width(&run_style)
                    .expect("built-in inline route was validated before planning"),
            );
            stats.record_builtin_profile_initialization();
        }
        stats.record_builtin_group_state_copy();
        initial
            .as_ref()
            .expect("inline profile was initialized")
            .clone()
    }
}

impl BuiltinInlineGroupWidth {
    fn new(
        profiles: &mut BuiltinInlineWidthProfiles,
        run_index: usize,
        runs: &[InlineTextRun],
        base_style: &TextStyle,
        stats: &mut InlineHtmlPlanningStats,
    ) -> Self {
        Self {
            run_index,
            width: profiles.begin(run_index, runs, base_style, stats),
        }
    }

    fn push_text(&mut self, text: &str, stats: &mut InlineHtmlPlanningStats) {
        stats.record_builtin_stream(text);
        self.width.push_text(text);
    }

    fn width_px(&self) -> f64 {
        self.width.width_px()
    }
}

#[derive(Debug, Clone, Default)]
struct BuiltinInlineLineWidth {
    width_before_last: f64,
    last_group: Option<BuiltinInlineGroupWidth>,
    height: f64,
    has_content: bool,
}

impl BuiltinInlineLineWidth {
    fn is_empty(&self) -> bool {
        !self.has_content
    }

    fn height_px(&self, base: &TextStyle) -> f64 {
        self.height.max(base.font_size.max(0.0) * 1.5)
    }

    fn width_px(&self) -> f64 {
        self.last_group
            .as_ref()
            .map_or(self.width_before_last, |last| {
                self.width_before_last + last.width_px()
            })
    }

    fn push_fragments(
        &mut self,
        profiles: &mut BuiltinInlineWidthProfiles,
        runs: &[InlineTextRun],
        source: &str,
        fragments: &[InlineRunFragment],
        style: &TextStyle,
        stats: &mut InlineHtmlPlanningStats,
    ) {
        for fragment in fragments {
            stats.record_fragment_measure_visits(1);
            let run = &runs[fragment.run_index];
            self.has_content = true;
            self.height = self
                .height
                .max(
                    run.terminal_style
                        .as_deref()
                        .unwrap_or(style)
                        .font_size
                        .max(0.0)
                        * 1.5,
                )
                .max(fragment.ancestor_strut_font_size * 1.5);
            if let Some(advance) = run.replaced_width_px {
                self.width_before_last = self.width_px() + advance;
                self.last_group = None;
                continue;
            }
            if !self
                .last_group
                .as_ref()
                .is_some_and(|group| same_inline_style(&runs[group.run_index], run))
            {
                if let Some(last) = self.last_group.take() {
                    self.width_before_last += last.width_px();
                }
                self.last_group = Some(BuiltinInlineGroupWidth::new(
                    profiles,
                    fragment.run_index,
                    runs,
                    style,
                    stats,
                ));
            }
            self.last_group
                .as_mut()
                .expect("inline group was initialized")
                .push_text(&source[fragment.start..fragment.end], stats);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct InlineLinePlan {
    fragment_start: usize,
    fragment_end: usize,
}

impl InlineLinePlan {
    fn from_segment(segment: InlineBreakSegment) -> Self {
        Self {
            fragment_start: segment.fragment_start,
            fragment_end: segment.fragment_end,
        }
    }

    fn is_empty(self) -> bool {
        self.fragment_start == self.fragment_end
    }

    fn append(&mut self, segment: InlineBreakSegment, stats: &mut InlineHtmlPlanningStats) -> Self {
        let checkpoint = *self;
        if self.is_empty() {
            *self = Self::from_segment(segment);
        } else {
            debug_assert_eq!(self.fragment_end, segment.fragment_start);
            self.fragment_end = segment.fragment_end;
        }
        stats.record_line_plan_update();
        checkpoint
    }
}

#[derive(Debug, Clone, Copy)]
struct InlineHtmlLineLayout {
    natural_width: f64,
    wrapped_width: f64,
    min_content_width: f64,
    line_count: usize,
    height: f64,
    last_line_height: f64,
}

fn inline_html_fragment_height<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    breaks: &InlineBreakPlan,
    line: InlineLinePlan,
    selected_break: HtmlBreakKind,
    base: &TextStyle,
) -> f64 {
    let mut height = base.font_size.max(0.0) * 1.5;
    let mut index = line.fragment_start;
    while index < line.fragment_end && !measurer.cancellation_requested() {
        let first = breaks.fragments[index];
        let run = &runs[first.run_index];
        let end = breaks.opaque_style_group_ends[index].min(line.fragment_end);
        height = breaks.fragments[index..end]
            .iter()
            .fold(height, |height, fragment| {
                height.max(fragment.ancestor_strut_font_size * 1.5)
            });
        if let Some(style) = run.terminal_style.as_deref() {
            let group_height = if run.replaced_width_px.is_some() {
                style.font_size.max(0.0) * 1.5
            } else {
                let last = breaks.fragments[end - 1];
                let text = html_line_text_with_break(
                    &breaks.source[first.start..last.end],
                    end == line.fragment_end && selected_break == HtmlBreakKind::SoftHyphen,
                );
                measurer.measure_html_line_height_px(&text, style)
            };
            height = height.max(group_height);
        }
        index = end;
    }
    height
}

#[cfg(test)]
fn measure_inline_html_line_layout<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
) -> InlineHtmlLineLayout {
    let mut stats = InlineHtmlPlanningStats::default();
    measure_inline_html_line_layout_with_carrier_and_stats(
        measurer,
        InlineHtmlMeasurementCarrier::opaque(),
        runs,
        style,
        max_width,
        &mut stats,
    )
}

#[cfg(test)]
fn measure_inline_html_line_layout_with_stats<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
    stats: &mut InlineHtmlPlanningStats,
) -> InlineHtmlLineLayout {
    measure_inline_html_line_layout_with_carrier_and_stats(
        measurer,
        InlineHtmlMeasurementCarrier::opaque(),
        runs,
        style,
        max_width,
        stats,
    )
}

fn measure_inline_html_line_layout_with_carrier<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
) -> InlineHtmlLineLayout {
    let mut stats = InlineHtmlPlanningStats::default();
    measure_inline_html_line_layout_with_carrier_and_stats(
        measurer, carrier, runs, style, max_width, &mut stats,
    )
}

fn measure_inline_html_line_layout_with_carrier_and_stats<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
    stats: &mut InlineHtmlPlanningStats,
) -> InlineHtmlLineLayout {
    let natural_width = runs
        .iter()
        .filter(|run| !run.text.is_empty())
        .map(|run| {
            if let Some(width) = run.replaced_width_px {
                return width;
            }
            let run_style = resolved_inline_text_style(run, style);
            stats.record_backend_request(run.text.len());
            measure_inline_run_width_px(measurer, &run.text, &run_style, WrapMode::HtmlLike, false)
        })
        .sum();
    if carrier.is_builtin() {
        let active_max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
        if active_max_width.is_none_or(|max_width| natural_width <= max_width) {
            // Min-content cannot affect the public result while the natural line already fits (or
            // wrapping is disabled). Returning the natural width as a conservative private value
            // avoids indexing every break segment on the dominant built-in fast path; it is at
            // most the active maximum and therefore cannot widen another overflowing line.
            let height = runs
                .iter()
                .filter(|run| !run.text.is_empty() || run.replaced_width_px.is_some())
                .fold(style.font_size.max(0.0) * 1.5, |height, run| {
                    height
                        .max(
                            run.terminal_style
                                .as_deref()
                                .unwrap_or(style)
                                .font_size
                                .max(0.0)
                                * 1.5,
                        )
                        .max(run.ancestor_strut_height_px())
                });
            return InlineHtmlLineLayout {
                natural_width,
                wrapped_width: natural_width,
                min_content_width: natural_width,
                line_count: 1,
                height,
                last_line_height: height,
            };
        }

        let breaks = index_inline_run_breaks(runs, stats);
        return measure_builtin_inline_html_line_layout(
            carrier,
            runs,
            style,
            max_width,
            natural_width,
            &breaks,
            stats,
        );
    }

    let mut breaks = index_inline_run_breaks(runs, stats);
    index_opaque_inline_style_groups(runs, &mut breaks, stats);
    measure_opaque_inline_html_line_layout(
        measurer,
        runs,
        style,
        max_width,
        natural_width,
        &breaks,
        stats,
    )
}

fn measure_opaque_inline_html_line_layout<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
    natural_width: f64,
    breaks: &InlineBreakPlan,
    stats: &mut InlineHtmlPlanningStats,
) -> InlineHtmlLineLayout {
    if breaks
        .segments
        .iter()
        .any(|segment| segment.end == HtmlBreakKind::SoftHyphen)
    {
        return measure_inline_soft_hyphen_line_layout(
            breaks,
            max_width,
            natural_width,
            stats,
            |line, selected_break, stats| {
                measure_inline_fragments_with_break_width_px(
                    measurer,
                    runs,
                    breaks,
                    line.fragment_start,
                    line.fragment_end,
                    selected_break,
                    style,
                    stats,
                )
            },
            |line, selected_break| {
                inline_html_fragment_height(measurer, runs, breaks, line, selected_break, style)
            },
        );
    }
    let min_content_width = breaks
        .segments
        .iter()
        .map(|segment| {
            measure_inline_fragments_width_px(
                measurer,
                runs,
                breaks,
                segment.fragment_start,
                segment.fragment_end,
                style,
                stats,
            )
        })
        .fold(0.0_f64, f64::max);

    let natural_height = || {
        inline_html_fragment_height(
            measurer,
            runs,
            breaks,
            InlineLinePlan {
                fragment_start: 0,
                fragment_end: breaks.fragments.len(),
            },
            HtmlBreakKind::Ordinary,
            style,
        )
    };

    let active_max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
    if active_max_width.is_none_or(|max_width| natural_width <= max_width) {
        let height = natural_height();
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
            height,
            last_line_height: height,
        };
    }
    let max_width = active_max_width.expect("overflowing line has a finite wrapping width");

    // A candidate line is always one contiguous range in the indexed fragment table. Appending
    // and rolling back therefore update two offsets instead of copying a growing fragment vector.
    let mut current = InlineLinePlan::default();
    stats.record_line_plan(std::mem::size_of::<InlineLinePlan>());
    let mut wrapped_width = 0.0_f64;
    let mut line_count = 0usize;
    let mut height = 0.0;
    let mut last_line_height = style.font_size.max(0.0) * 1.5;
    for segment in breaks.segments.iter().copied() {
        let checkpoint = current.append(segment, stats);
        let candidate_width = measure_inline_fragments_width_px(
            measurer,
            runs,
            breaks,
            current.fragment_start,
            current.fragment_end,
            style,
            stats,
        );
        if checkpoint.is_empty() || candidate_width <= max_width {
            continue;
        }

        current = checkpoint;
        wrapped_width = wrapped_width.max(measure_inline_fragments_width_px(
            measurer,
            runs,
            breaks,
            current.fragment_start,
            current.fragment_end,
            style,
            stats,
        ));
        line_count += 1;
        last_line_height = inline_html_fragment_height(
            measurer,
            runs,
            breaks,
            current,
            HtmlBreakKind::Ordinary,
            style,
        );
        height += last_line_height;
        current = InlineLinePlan::from_segment(segment);
        stats.record_line_plan_update();
    }

    if !current.is_empty() {
        wrapped_width = wrapped_width.max(measure_inline_fragments_width_px(
            measurer,
            runs,
            breaks,
            current.fragment_start,
            current.fragment_end,
            style,
            stats,
        ));
        line_count += 1;
        last_line_height = inline_html_fragment_height(
            measurer,
            runs,
            breaks,
            current,
            HtmlBreakKind::Ordinary,
            style,
        );
        height += last_line_height;
    }

    InlineHtmlLineLayout {
        natural_width,
        wrapped_width,
        min_content_width,
        line_count: line_count.max(1),
        height: height.max(style.font_size.max(0.0) * 1.5),
        last_line_height,
    }
}

fn measure_builtin_inline_html_line_layout(
    carrier: InlineHtmlMeasurementCarrier,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
    natural_width: f64,
    breaks: &InlineBreakPlan,
    stats: &mut InlineHtmlPlanningStats,
) -> InlineHtmlLineLayout {
    let natural_height = runs
        .iter()
        .filter(|run| !run.text.is_empty() || run.replaced_width_px.is_some())
        .fold(style.font_size.max(0.0) * 1.5, |height, run| {
            height
                .max(
                    run.terminal_style
                        .as_deref()
                        .unwrap_or(style)
                        .font_size
                        .max(0.0)
                        * 1.5,
                )
                .max(run.ancestor_strut_height_px())
        });
    let mut profiles = BuiltinInlineWidthProfiles::new(carrier, runs);
    stats.record_builtin_profile_cache(
        std::mem::size_of::<BuiltinInlineWidthProfiles>()
            + profiles.terminal.capacity() * std::mem::size_of::<Option<BuiltinHtmlLineWidth>>(),
    );
    if breaks
        .segments
        .iter()
        .any(|segment| segment.end == HtmlBreakKind::SoftHyphen)
    {
        let mut measured_line = InlineLinePlan::default();
        let mut current_width = BuiltinInlineLineWidth::default();
        return measure_inline_soft_hyphen_line_layout(
            breaks,
            max_width,
            natural_width,
            stats,
            |line, selected_break, stats| {
                if measured_line.fragment_start != line.fragment_start
                    || measured_line.fragment_end > line.fragment_end
                {
                    current_width = BuiltinInlineLineWidth::default();
                    measured_line = InlineLinePlan {
                        fragment_start: line.fragment_start,
                        fragment_end: line.fragment_start,
                    };
                }
                current_width.push_fragments(
                    &mut profiles,
                    runs,
                    &breaks.source,
                    &breaks.fragments[measured_line.fragment_end..line.fragment_end],
                    style,
                    stats,
                );
                measured_line = line;
                if selected_break == HtmlBreakKind::SoftHyphen {
                    let mut with_hyphen = current_width.clone();
                    stats.record_builtin_checkpoint_copy();
                    if let Some(group) = with_hyphen.last_group.as_mut() {
                        group.push_text("-", stats);
                    }
                    with_hyphen.width_px()
                } else {
                    current_width.width_px()
                }
            },
            |line, _| {
                breaks.fragments[line.fragment_start..line.fragment_end]
                    .iter()
                    .fold(style.font_size.max(0.0) * 1.5, |height, fragment| {
                        height
                            .max(
                                runs[fragment.run_index]
                                    .terminal_style
                                    .as_deref()
                                    .unwrap_or(style)
                                    .font_size
                                    .max(0.0)
                                    * 1.5,
                            )
                            .max(fragment.ancestor_strut_font_size * 1.5)
                    })
            },
        );
    }
    let mut min_content_width = 0.0_f64;
    for segment in &breaks.segments {
        let mut segment_width = BuiltinInlineLineWidth::default();
        segment_width.push_fragments(
            &mut profiles,
            runs,
            &breaks.source,
            &breaks.fragments[segment.fragment_start..segment.fragment_end],
            style,
            stats,
        );
        min_content_width = min_content_width.max(segment_width.width_px());
    }

    let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
            height: natural_height,
            last_line_height: natural_height,
        };
    };
    if natural_width <= max_width {
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
            height: natural_height,
            last_line_height: natural_height,
        };
    }

    let mut current = BuiltinInlineLineWidth::default();
    stats.record_line_plan(std::mem::size_of::<BuiltinInlineLineWidth>());
    let mut wrapped_width = 0.0_f64;
    let mut line_count = 0usize;
    let mut height = 0.0;
    let mut last_line_height = style.font_size.max(0.0) * 1.5;
    for segment in &breaks.segments {
        let checkpoint = current.clone();
        stats.record_builtin_checkpoint_copy();
        current.push_fragments(
            &mut profiles,
            runs,
            &breaks.source,
            &breaks.fragments[segment.fragment_start..segment.fragment_end],
            style,
            stats,
        );
        stats.record_line_plan_update();
        if checkpoint.is_empty() || current.width_px() <= max_width {
            continue;
        }

        current = checkpoint;
        wrapped_width = wrapped_width.max(current.width_px());
        line_count += 1;
        last_line_height = current.height_px(style);
        height += last_line_height;
        current = BuiltinInlineLineWidth::default();
        current.push_fragments(
            &mut profiles,
            runs,
            &breaks.source,
            &breaks.fragments[segment.fragment_start..segment.fragment_end],
            style,
            stats,
        );
        stats.record_line_plan_update();
    }

    if !current.is_empty() {
        wrapped_width = wrapped_width.max(current.width_px());
        line_count += 1;
        last_line_height = current.height_px(style);
        height += last_line_height;
    }

    InlineHtmlLineLayout {
        natural_width,
        wrapped_width,
        min_content_width,
        line_count: line_count.max(1),
        height: height.max(style.font_size.max(0.0) * 1.5),
        last_line_height,
    }
}

fn measure_inline_soft_hyphen_line_layout(
    breaks: &InlineBreakPlan,
    max_width: Option<f64>,
    natural_width: f64,
    stats: &mut InlineHtmlPlanningStats,
    mut measure: impl FnMut(InlineLinePlan, HtmlBreakKind, &mut InlineHtmlPlanningStats) -> f64,
    mut line_height: impl FnMut(InlineLinePlan, HtmlBreakKind) -> f64,
) -> InlineHtmlLineLayout {
    // Min-content chooses every available discretionary break. Each selected U+00AD therefore
    // contributes its conditional glyph even though the natural unbroken line hides it.
    let min_content_width = breaks
        .segments
        .iter()
        .map(|segment| measure(InlineLinePlan::from_segment(*segment), segment.end, stats))
        .fold(0.0_f64, f64::max);
    let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
        let height = line_height(
            InlineLinePlan {
                fragment_start: 0,
                fragment_end: breaks.fragments.len(),
            },
            HtmlBreakKind::Ordinary,
        );
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
            height,
            last_line_height: height,
        };
    };
    // Table-cell sizing cannot shrink below min-content. Applying the same lower bound when
    // selecting lines avoids an impossible break that would not fit its own conditional glyph.
    let max_width = max_width.max(min_content_width);
    if natural_width <= max_width {
        let height = line_height(
            InlineLinePlan {
                fragment_start: 0,
                fragment_end: breaks.fragments.len(),
            },
            HtmlBreakKind::Ordinary,
        );
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
            height,
            last_line_height: height,
        };
    }

    let mut current = InlineLinePlan::default();
    let mut last_fitting_break: Option<(usize, InlineLinePlan, HtmlBreakKind)> = None;
    let mut wrapped_width = 0.0_f64;
    let mut line_count = 0usize;
    let mut height = 0.0;
    let mut last_line_height = 0.0;
    let mut index = 0usize;
    stats.record_line_plan(std::mem::size_of::<InlineLinePlan>());
    while index < breaks.segments.len() {
        let segment = breaks.segments[index];
        current.append(segment, stats);
        let candidate_width = measure(current, HtmlBreakKind::Ordinary, stats);
        if candidate_width > max_width
            && let Some((break_index, line, end)) = last_fitting_break.take()
        {
            wrapped_width = wrapped_width.max(measure(line, end, stats));
            line_count += 1;
            last_line_height = line_height(line, end);
            height += last_line_height;
            current = InlineLinePlan::default();
            index = break_index + 1;
            continue;
        }
        if index + 1 < breaks.segments.len() && measure(current, segment.end, stats) <= max_width {
            last_fitting_break = Some((index, current, segment.end));
        }
        index += 1;
    }
    if !current.is_empty() {
        wrapped_width = wrapped_width.max(measure(current, HtmlBreakKind::Ordinary, stats));
        line_count += 1;
        last_line_height = line_height(current, HtmlBreakKind::Ordinary);
        height += last_line_height;
    }
    InlineHtmlLineLayout {
        natural_width,
        wrapped_width,
        min_content_width,
        line_count: line_count.max(1),
        height,
        last_line_height,
    }
}

fn measure_inline_html_layout<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    runs_by_line: &[Vec<InlineTextRun>],
    style: &TextStyle,
    max_width: Option<f64>,
    line_sizes: Option<&[f64]>,
) -> InlineHtmlLineLayout {
    let mut layout = InlineHtmlLineLayout {
        natural_width: 0.0,
        wrapped_width: 0.0,
        min_content_width: 0.0,
        line_count: 0,
        height: 0.0,
        last_line_height: 0.0,
    };
    for (index, runs) in runs_by_line.iter().enumerate() {
        let mut line =
            measure_inline_html_line_layout_with_carrier(measurer, carrier, runs, style, max_width);
        if let Some(size) = line_sizes.and_then(|sizes| sizes.get(index)) {
            // BR contributes to the physical line it terminates, including a line with text.
            let floor = size.max(0.0) * 1.5;
            line.height += (floor - line.last_line_height).max(0.0);
            line.last_line_height = line.last_line_height.max(floor);
        }
        layout.natural_width = layout.natural_width.max(line.natural_width);
        layout.wrapped_width = layout.wrapped_width.max(line.wrapped_width);
        layout.min_content_width = layout.min_content_width.max(line.min_content_width);
        layout.line_count += line.line_count;
        layout.height += line.height;
        layout.last_line_height = line.last_line_height;
    }
    layout.line_count = layout.line_count.max(1);
    layout
}

fn explicit_inline_html_line_boxes(
    runs_by_line: &[Vec<InlineTextRun>],
    image_on_line: &[bool],
) -> usize {
    // Consecutive `<br>` elements create empty inline line boxes. Keep those boxes up to the last
    // visible text/image line; a final image-only line is left to the browser-dependent replaced
    // element bounds instead of assigning it a guessed intrinsic height.
    runs_by_line
        .iter()
        .zip(image_on_line)
        .rposition(|(runs, has_image)| runs.iter().any(|run| !run.text.is_empty()) || *has_image)
        .map(|last_content_line| {
            runs_by_line[..=last_content_line]
                .iter()
                .zip(&image_on_line[..=last_content_line])
                .filter(|(runs, has_image)| {
                    runs.iter().any(|run| !run.text.is_empty()) || !**has_image
                })
                .count()
        })
        // With no visible runs, N explicit `<br>` elements produce N line boxes. The parser keeps
        // one initial row plus one row per break, while block-end rows are suppressed below.
        .unwrap_or_else(|| runs_by_line.len().saturating_sub(1))
}

struct InlineHtmlLineBoxes<'a> {
    explicit_count: usize,
    font_sizes: Option<&'a [f64]>,
}

fn finish_inline_html_layout<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    runs_by_line: &[Vec<InlineTextRun>],
    break_spaces_runs_by_line: Option<&[Vec<InlineTextRun>]>,
    style: &TextStyle,
    max_width: Option<f64>,
    line_boxes: InlineHtmlLineBoxes<'_>,
) -> TextMetrics {
    let collapsed = measure_inline_html_layout(
        measurer,
        carrier,
        runs_by_line,
        style,
        max_width,
        line_boxes.font_sizes,
    );
    let active_max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
    let break_spaces_active = active_max_width.is_some_and(|max_width| {
        collapsed.natural_width >= max_width && collapsed.min_content_width <= max_width
    });
    let layout = if break_spaces_active {
        break_spaces_runs_by_line
            .map(|runs| {
                measure_inline_html_layout(
                    measurer,
                    carrier,
                    runs,
                    style,
                    max_width,
                    line_boxes.font_sizes,
                )
            })
            .unwrap_or(collapsed)
    } else {
        collapsed
    };
    let width = if break_spaces_active {
        active_max_width
            .unwrap_or(layout.natural_width)
            .max(layout.min_content_width)
    } else if let Some(max_width) = active_max_width {
        if layout.natural_width > max_width {
            layout
                .wrapped_width
                .max(layout.min_content_width)
                .max(max_width)
        } else {
            layout.natural_width.min(max_width)
        }
    } else {
        layout.natural_width
    };
    TextMetrics {
        width: round_to_1_64_px(width),
        height: if line_boxes.font_sizes.is_some() {
            layout.height
                + line_boxes.explicit_count.saturating_sub(layout.line_count) as f64
                    * style.font_size.max(0.0)
                    * 1.5
        } else {
            layout.line_count.max(line_boxes.explicit_count) as f64 * style.font_size.max(0.0) * 1.5
        },
        line_count: layout.line_count.max(line_boxes.explicit_count),
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod inline_planning_tests {
    use super::*;
    use crate::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use crate::text::DeterministicTextMeasurer;
    use std::cell::{Cell, RefCell};
    use std::sync::{Arc, Mutex};

    fn inline_html_carrier<M: TextMeasurer + ?Sized>(measurer: &M) -> InlineHtmlMeasurementCarrier {
        measurer
            .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
            .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
            .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque)
    }

    type RecordedMeasurementCall = (String, Option<String>, Option<String>, Option<String>);

    #[test]
    fn zero_font_runs_keep_builtin_wrapping_equal_to_visible_siblings() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle::default();
        let visible = "Alpha beta gamma delta epsilon zeta";
        let hidden = "invisible ".repeat(16);
        for width in [48.0, 96.0, 1000.0] {
            let measure = |html: &str| {
                measure_xhtml_label_fragment_with_terminal_fonts(
                    &measurer,
                    html,
                    &style,
                    Some(width),
                    16.0,
                )
            };
            // Keep both controls on the mixed-font path; the legacy uniform-font path uses
            // a distinct width quantization policy that is unrelated to zero glyph advance.
            let visible = format!("<span style='font-size:18px'>{visible}</span>");
            let expected = measure(&format!("<p>{visible}</p>"));
            let actual = measure(&format!(
                "<p><span style='font-size:0px'>{hidden}</span>{visible}</p>"
            ));
            assert_eq!(actual.width, expected.width, "width={width}");
            assert_eq!(actual.height, expected.height, "width={width}");
            assert_eq!(actual.line_count, expected.line_count, "width={width}");
        }
        let zero = measure_xhtml_label_fragment_with_terminal_fonts(
            &measurer,
            "<p><span style='font-size:0px'>invisible</span></p>",
            &style,
            Some(1.0),
            16.0,
        );
        assert_eq!(zero.width, 0.0);
        assert_eq!(zero.height, 24.0);
        assert_eq!(zero.line_count, 1);
    }

    #[test]
    fn inline_ancestor_struts_follow_text_ranges_through_wrapping_and_breaks() {
        struct FontSizedMeasurer;
        impl TextMeasurer for FontSizedMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert!(!text.contains('\u{fffc}'), "icons are fixed-advance boxes");
                TextMetrics {
                    width: text.chars().count() as f64 * style.font_size,
                    height: style.font_size * 1.5,
                    line_count: 1,
                }
            }
        }
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
        let opaque = FontSizedMeasurer;
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        for measurer in [&builtin as &dyn TextMeasurer, &opaque] {
            for (html, max_width, height, lines) in [
                (
                    "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span>Visible</p>",
                    None,
                    96.0,
                    1,
                ),
                (
                    "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span></p>",
                    None,
                    96.0,
                    1,
                ),
                (
                    "<p><span style='font-size:64px'><span style='font-size:10px'>AA</span></span><span style='font-size:10px'> BBBB</span></p>",
                    Some(25.0),
                    111.0,
                    2,
                ),
                (
                    "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden<br/>More</span></span><br/>Visible</p>",
                    Some(1000.0),
                    207.0,
                    3,
                ),
                (
                    "<p><span style='font-size:64px'><span style='font-size:0px'><br/><br/></span></span></p>",
                    None,
                    192.0,
                    2,
                ),
                (
                    "<p><span style='font-size:64px'><span style='font-size:0px'><i class='fa fa-car'></i></span></span>Visible</p>",
                    Some(1000.0),
                    96.0,
                    1,
                ),
            ] {
                let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                    measurer, html, &style, max_width, 16.0,
                );
                assert_eq!(measured.height, height, "{html}: {measured:?}");
                assert_eq!(measured.line_count, lines, "{html}: {measured:?}");
            }
            for html in [
                "<p><span style='font-size:128px'></span>Visible</p>",
                "<p><img style='font-size:128px'/>Visible</p>",
                "<p><span style='font-size:128px'><img/></span>Visible</p>",
                "<p><span style='font-size:128px'><span style='font-size:0px'> </span></span>Visible</p>",
                "<p>Visible<span style='font-size:128px'><span style='font-size:10px'> </span></span></p>",
            ] {
                for max_width in [None, Some(1000.0)] {
                    let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                        measurer, html, &style, max_width, 16.0,
                    );
                    assert_eq!(measured.height, 15.0, "{html}: {measured:?}");
                    assert_eq!(measured.line_count, 1, "{html}: {measured:?}");
                }
            }
            let zero = TextStyle {
                font_size: 0.0,
                ..style.clone()
            };
            for max_width in [None, Some(1000.0)] {
                let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                    measurer,
                    "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span><br/><span style='font-size:10px'>Visible</span></p>",
                    &zero,
                    max_width,
                    16.0,
                );
                assert_eq!(measured.height, 111.0);
                assert_eq!(measured.line_count, 2);
            }
        }
    }

    #[test]
    fn inline_ancestor_struts_do_not_split_opaque_font_requests() {
        #[derive(Default)]
        struct Probe(RefCell<Vec<(String, f64)>>);
        impl TextMeasurer for Probe {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                self.0.borrow_mut().push((text.to_owned(), style.font_size));
                TextMetrics {
                    width: text.chars().count() as f64 * style.font_size,
                    height: style.font_size * 1.5,
                    line_count: 1,
                }
            }
        }
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        for max_width in [None, Some(25.0), Some(1000.0)] {
            let control = Probe::default();
            let expected = measure_xhtml_label_fragment_with_terminal_fonts(
                &control,
                "<p><span style='font-size:10px'>AA BBBB</span></p>",
                &style,
                max_width,
                16.0,
            );
            let probe = Probe::default();
            let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                &probe,
                "<p><span style='font-size:64px'><span style='font-size:10px'>AA</span></span><span style='font-size:10px'> BBBB</span></p>",
                &style,
                max_width,
                16.0,
            );
            assert_eq!(*probe.0.borrow(), *control.0.borrow());
            assert_eq!(measured.width, expected.width);
            assert_eq!(measured.line_count, expected.line_count);
            assert_eq!(measured.height - expected.height, 81.0);
        }
    }

    #[test]
    fn inline_ancestor_ranges_keep_soft_hyphen_and_streaming_font_groups() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let font = Arc::new(style.clone());
        for text in ["AA BBBB", "AA\u{00ad}BBBB"] {
            let mut run = InlineTextRun {
                text: text.to_owned(),
                terminal_style: Some(Arc::clone(&font)),
                ..Default::default()
            };
            run.ancestor_struts.push(InlineAncestorStrut {
                start: 0,
                end: 2,
                font_size: 64.0,
            });
            let control = InlineTextRun {
                ancestor_struts: Vec::new(),
                ..run.clone()
            };
            for width in [15.0, 25.0, 1000.0] {
                let mut reference_stats = InlineHtmlPlanningStats::default();
                let reference = measure_inline_html_line_layout_with_carrier_and_stats(
                    &measurer,
                    inline_html_carrier(&measurer),
                    std::slice::from_ref(&control),
                    &style,
                    Some(width),
                    &mut reference_stats,
                );
                let mut stats = InlineHtmlPlanningStats::default();
                let measured = measure_inline_html_line_layout_with_carrier_and_stats(
                    &measurer,
                    inline_html_carrier(&measurer),
                    std::slice::from_ref(&run),
                    &style,
                    Some(width),
                    &mut stats,
                );
                assert_eq!(measured.line_count, reference.line_count);
                assert_eq!(measured.wrapped_width, reference.wrapped_width);
                assert_eq!(measured.height - reference.height, 81.0);
                assert_eq!(
                    stats.builtin_stream_bytes,
                    reference_stats.builtin_stream_bytes
                );
                assert_eq!(stats.backend_requests, reference_stats.backend_requests);
                assert_eq!(
                    stats.backend_request_bytes,
                    reference_stats.backend_request_bytes
                );
                assert_eq!(
                    stats.builtin_checkpoint_copies,
                    reference_stats.builtin_checkpoint_copies
                );
                assert_eq!(stats.measurement_payload_copy_bytes, 0);
            }
        }
    }

    #[derive(Default)]
    struct RecordingMeasurer {
        calls: RefCell<Vec<RecordedMeasurementCall>>,
    }

    impl TextMeasurer for RecordingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
                style.font_family.clone(),
            ));
            TextMetrics {
                width: text.len() as f64,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    #[test]
    fn html_emphasis_resolves_each_parent_before_measuring_terminal_fonts() {
        // WHATWG's default strong/b weight is bolder; CSS Fonts computes it against the parent.
        // These expectations also match Chromium computed styles, including nested emphasis.
        for (parent_weight, strong, nested) in [
            (100, 400, 700),
            (300, 400, 700),
            (400, 700, 900),
            (500, 700, 900),
            (600, 900, 900),
            (700, 900, 900),
            (900, 900, 900),
        ] {
            let base = TextStyle {
                font_size: 16.0,
                font_weight: Some(parent_weight.to_string()),
                font_style: Some("oblique".to_owned()),
                ..TextStyle::default()
            };
            for (html, weight, font_style) in [
                ("<strong>A</strong>", strong, "oblique"),
                ("<b><strong>A</strong></b>", nested, "oblique"),
                ("<em>A</em>", parent_weight, "italic"),
                ("<i>A</i>", parent_weight, "italic"),
                (
                    "<strong style='font-weight:inherit'>A</strong>",
                    parent_weight,
                    "oblique",
                ),
                ("<b style='font-weight:normal'>A</b>", 400, "oblique"),
                ("<b style='font-weight:initial'>A</b>", 400, "oblique"),
                (
                    "<em style='font-style:inherit'>A</em>",
                    parent_weight,
                    "oblique",
                ),
                (
                    "<em style='font-style:initial'>A</em>",
                    parent_weight,
                    "normal",
                ),
                (
                    "<b style='font-weight:500!important;font-weight:900'>A</b>",
                    500,
                    "oblique",
                ),
            ] {
                for max_width in [None, Some(1000.0)] {
                    for prepared in [false, true] {
                        let measurer = RecordingMeasurer::default();
                        if prepared {
                            measure_xhtml_label_fragment_with_terminal_fonts(
                                &measurer, html, &base, max_width, 16.0,
                            );
                        } else {
                            measure_html_with_inline_styles(
                                &measurer,
                                html,
                                &base,
                                max_width,
                                WrapMode::HtmlLike,
                            );
                        }
                        let calls = measurer.calls.borrow();
                        let (_, actual_weight, actual_style, _) =
                            calls.last().expect("terminal font measurement");
                        assert_eq!(
                            actual_weight.as_deref(),
                            Some(weight.to_string().as_str()),
                            "{parent_weight}/{html}/{max_width:?}/prepared={prepared}: {calls:?}"
                        );
                        assert_eq!(actual_style.as_deref(), Some(font_style), "{html}");
                    }
                }
            }
        }
    }

    #[test]
    fn html_phrasing_defaults_and_author_overrides_use_the_same_font_stack() {
        let base = TextStyle {
            font_family: Some("serif".to_owned()),
            font_style: Some("oblique".to_owned()),
            ..TextStyle::default()
        };
        for (tag, property, expected, override_value) in [
            ("cite", "font-style", "italic", "normal"),
            ("dfn", "font-style", "italic", "normal"),
            ("var", "font-style", "italic", "normal"),
            ("code", "font-family", "monospace", "serif"),
            ("kbd", "font-family", "monospace", "serif"),
            ("samp", "font-family", "monospace", "serif"),
            ("tt", "font-family", "monospace", "serif"),
        ] {
            for override_default in [false, true] {
                let attr = if override_default {
                    format!(" style='{property}:{override_value}!important'")
                } else {
                    String::new()
                };
                let measurer = RecordingMeasurer::default();
                measure_html_with_inline_styles(
                    &measurer,
                    &format!("<{tag}{attr}>A</{tag}>"),
                    &base,
                    None,
                    WrapMode::HtmlLike,
                );
                let calls = measurer.calls.borrow();
                let (_, _, actual_style, actual_family) = calls.last().unwrap();
                let actual = if property == "font-style" {
                    actual_style
                } else {
                    actual_family
                };
                assert_eq!(
                    actual.as_deref(),
                    Some(if override_default {
                        override_value
                    } else {
                        expected
                    }),
                    "{tag}/{attr}"
                );
            }
        }
    }

    #[test]
    fn html_terminal_fonts_restore_the_parent_after_nested_and_void_elements() {
        let measurer = RecordingMeasurer::default();
        let base = TextStyle {
            font_size: 16.0,
            font_weight: Some("400".to_owned()),
            ..TextStyle::default()
        };
        measure_html_with_inline_styles(
            &measurer,
            "<strong>A<strong>B</strong>C</strong>D<img style='font-weight:100'>E",
            &base,
            None,
            WrapMode::HtmlLike,
        );
        let calls = measurer.calls.borrow();
        for (text, expected) in [
            ("A", "700"),
            ("B", "900"),
            ("C", "700"),
            ("D", "400"),
            ("E", "400"),
        ] {
            assert!(
                calls
                    .iter()
                    .any(|(actual, weight, ..)| actual == text
                        && weight.as_deref() == Some(expected)),
                "{text} must use {expected}: {calls:?}"
            );
        }
    }

    #[test]
    fn terminal_font_heights_measure_selected_line_groups_without_repeating_whole_runs() {
        #[derive(Default)]
        struct HeightProbe(RefCell<Vec<String>>);
        impl TextMeasurer for HeightProbe {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64,
                    height: 24.0,
                    line_count: 1,
                }
            }

            fn measure_html_line_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
                self.0.borrow_mut().push(text.to_string());
                24.0
            }
        }
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        for words in [3, 256, 4096] {
            let text = vec!["x"; words].join(" ");
            let probe = HeightProbe::default();
            let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
                &probe,
                &format!("<p><span style='font-size:16px'>{text}</span></p>"),
                &style,
                Some(1_000_000.0),
                16.0,
            );
            assert_eq!(metrics.height, 24.0);
            assert_eq!(metrics.line_count, 1);
            let calls = probe.0.borrow();
            assert_eq!(calls.as_slice(), [text.as_str()]);
            assert_eq!(calls.iter().map(String::len).sum::<usize>(), text.len());
        }
        for (text, budget, expected) in [
            ("x x x x", 4.0, &["x x ", "x x"][..]),
            ("ab\u{00ad}cd", 3.0, &["ab-", "cd"][..]),
        ] {
            let probe = HeightProbe::default();
            let runs = [InlineTextRun {
                text: text.to_string(),
                terminal_style: Some(Arc::new(style.clone())),
                ..Default::default()
            }];
            let layout = measure_inline_html_line_layout(&probe, &runs, &style, Some(budget));
            assert_eq!(layout.line_count, 2);
            assert_eq!(layout.height, 48.0);
            assert_eq!(probe.0.borrow().as_slice(), expected);
        }
    }

    #[test]
    fn prepared_xhtml_terminal_fonts_keep_inline_sizes_empty_rows_and_wrapped_heights() {
        struct FontSizedMeasurer;
        impl TextMeasurer for FontSizedMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert!(
                    !text.contains('\u{fffc}'),
                    "inline boxes are not glyph probes"
                );
                TextMetrics {
                    width: text.chars().count() as f64 * style.font_size,
                    height: style.font_size * 1.5,
                    line_count: 1,
                }
            }
        }
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        for (html, expected_width, expected_height, expected_lines) in [
            ("<p><big>AA</big></p>", 24.0, 18.0, 1),
            ("<p><small>A</small></p>", 10.0 / 1.2, 15.0, 1),
            ("<p><big>A</big><br/>B</p>", 12.0, 33.0, 2),
            ("<p><big>A<br/><br/>B</big></p>", 12.0, 54.0, 3),
            ("<p>A<big><br/></big>B</p>", 10.0, 33.0, 2),
            ("<p><big><br/><br/></big></p>", 0.0, 36.0, 2),
            ("<p>A<br/><br/><img/></p>", 10.0, 30.0, 2),
            (
                "<p style='font-size:10px'>A<br/><br/><img/></p>",
                10.0,
                30.0,
                2,
            ),
            (
                "<p><big><i class='fa fa-car'></i>A</big></p>",
                27.0,
                18.0,
                1,
            ),
            (
                "<p><big style='font-size:inherit'>A</big></p>",
                10.0,
                15.0,
                1,
            ),
            (
                "<p><span style='font-size:2rem'>A</span></p>",
                32.0,
                48.0,
                1,
            ),
            (
                "<p>A<span style='font-size:100px'> </span></p>",
                10.0,
                15.0,
                1,
            ),
        ] {
            for max_width in [None, Some(1000.0)] {
                let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                    &FontSizedMeasurer,
                    html,
                    &style,
                    max_width,
                    16.0,
                );
                assert!(
                    (measured.width - expected_width).abs() < 1.0 / 64.0,
                    "{html}: {measured:?}"
                );
                assert!(
                    (measured.height - expected_height).abs() < 1e-9,
                    "{html}: {measured:?}"
                );
                assert_eq!(measured.line_count, expected_lines, "{html}");
            }
        }
        let wrapped = measure_xhtml_label_fragment_with_terminal_fonts(
            &FontSizedMeasurer,
            "<p><big>A A</big></p>",
            &style,
            Some(15.0),
            16.0,
        );
        assert_eq!(wrapped.line_count, 2);
        assert_eq!(wrapped.height, 36.0);

        let wrapped_icon = measure_xhtml_label_fragment_with_terminal_fonts(
            &FontSizedMeasurer,
            "<p><big><i class='fa fa-car'></i>A</big></p>",
            &style,
            Some(15.0),
            16.0,
        );
        assert_eq!(wrapped_icon.width, 15.0);
        assert_eq!(wrapped_icon.line_count, 2);
        assert_eq!(wrapped_icon.height, 36.0);
        let plain_break = measure_xhtml_label_fragment_with_terminal_fonts(
            &FontSizedMeasurer,
            "<p>A A A<br/>B</p>",
            &style,
            Some(15.0),
            16.0,
        );
        let enlarged_break = measure_xhtml_label_fragment_with_terminal_fonts(
            &FontSizedMeasurer,
            "<p>A A A<big><br/></big>B</p>",
            &style,
            Some(15.0),
            16.0,
        );
        assert_eq!(plain_break.line_count, enlarged_break.line_count);
        assert_eq!(enlarged_break.height - plain_break.height, 3.0);
    }

    #[test]
    fn terminal_font_line_boxes_keep_the_hosts_fractional_height() {
        struct FractionalLineHeight;
        impl TextMeasurer for FractionalLineHeight {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.len() as f64 * style.font_size,
                    height: 62.203125,
                    line_count: 1,
                }
            }
        }
        let style = TextStyle {
            font_size: 24.0,
            ..TextStyle::default()
        };
        for max_width in [None, Some(10000.0)] {
            let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
                &FractionalLineHeight,
                "<p><big><big><big>Account</big></big></big></p>",
                &style,
                max_width,
                16.0,
            );
            assert_eq!(metrics.height, 62.203125);
        }
    }

    struct ScriptedStatefulMeasurer {
        widths: Vec<f64>,
        next: Cell<usize>,
        calls: RefCell<Vec<String>>,
    }

    struct RecordingHostMeasurer {
        calls: Arc<Mutex<Vec<(TextMeasurementOperation, String)>>>,
    }

    impl HostTextMeasurer for RecordingHostMeasurer {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.calls
                .lock()
                .expect("host call log lock")
                .push((request.operation, request.text.to_string()));
            if request.operation == TextMeasurementOperation::CanvasMeasureTextWidth {
                Ok(Some(HostTextMeasurement::Length(request.text.len() as f64)))
            } else {
                Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                    width: request.text.len() as f64,
                    height: request.style.font_size,
                    line_count: 1,
                })))
            }
        }
    }

    impl ScriptedStatefulMeasurer {
        fn new(widths: Vec<f64>) -> Self {
            Self {
                widths,
                next: Cell::new(0),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl TextMeasurer for ScriptedStatefulMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            let index = self.next.get();
            self.next.set(index + 1);
            self.calls.borrow_mut().push(text.to_string());
            TextMetrics {
                width: self.widths[index],
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    fn run(text: impl Into<String>, variant: usize) -> InlineTextRun {
        InlineTextRun {
            text: text.into(),
            bold: variant == 1,
            italic: variant == 2,
            code: variant == 3,
            terminal_style: None,
            replaced_width_px: None,
            ancestor_struts: Vec::new(),
        }
    }

    fn fixed_byte_runs(
        source_bytes: usize,
        run_count: usize,
        break_count: usize,
    ) -> Vec<InlineTextRun> {
        assert!(source_bytes > 0);
        assert!((1..=source_bytes).contains(&run_count));
        assert!(break_count < source_bytes);

        let mut text = vec![b'a'; source_bytes];
        for index in 0..break_count {
            let offset = (index + 1) * source_bytes / (break_count + 1);
            text[offset] = b' ';
        }

        let mut runs = Vec::with_capacity(run_count);
        let mut start = 0usize;
        for index in 0..run_count {
            let end = (index + 1) * source_bytes / run_count;
            runs.push(run(
                String::from_utf8(text[start..end].to_vec()).expect("ASCII workload"),
                index % 4,
            ));
            start = end;
        }
        runs
    }

    fn fragment_text(source: &str, fragments: &[InlineRunFragment]) -> String {
        let mut text = String::new();
        for fragment in fragments {
            text.push_str(&source[fragment.start..fragment.end]);
        }
        text
    }

    #[test]
    fn indexed_break_walk_preserves_unicode_and_cross_run_fragments() {
        let runs = vec![
            run("A\u{301} ", 0),
            run("👩‍💻 ", 1),
            run("مرحبا ", 2),
            run("世界", 3),
        ];
        let expected_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let expected_segments = html_break_spaces_segments(&expected_text);
        let mut stats = InlineHtmlPlanningStats::default();
        let plan = index_inline_run_breaks(&runs, &mut stats);
        let actual_segments = plan
            .segments
            .iter()
            .map(|segment| {
                fragment_text(
                    &plan.source,
                    &plan.fragments[segment.fragment_start..segment.fragment_end],
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual_segments,
            expected_segments
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(stats.source_bytes, expected_text.len());
        assert_eq!(stats.break_segments, actual_segments.len());
        assert!(stats.fragment_refs < runs.len() + actual_segments.len());
        assert_eq!(stats.run_visits, runs.len() * 2 + stats.fragment_refs);
    }

    #[derive(Default)]
    struct SoftHyphenMeasurer {
        calls: RefCell<Vec<RecordedMeasurementCall>>,
        kern_hyphen: bool,
    }

    impl TextMeasurer for SoftHyphenMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
                style.font_family.clone(),
            ));
            let advance = if style_requests_bold_font_weight(style) {
                20.0
            } else {
                10.0
            };
            let kerning = if self.kern_hyphen && text.len() > 1 && text.ends_with('-') {
                5.0
            } else {
                0.0
            };
            TextMetrics {
                width: text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64 * advance
                    - kerning,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    #[test]
    fn svg_markdown_resets_parent_weight_without_changing_html_inheritance() {
        let measurer = SoftHyphenMeasurer::default();
        let style = TextStyle {
            font_weight: Some("700".to_owned()),
            font_style: Some("italic".to_owned()),
            ..TextStyle::default()
        };
        let svg = measure_markdown_with_inline_styles(
            &measurer,
            "plain **bold**",
            &style,
            None,
            WrapMode::SvgLike,
        );
        assert_eq!(svg.width, 150.0);
        let calls = measurer.calls.borrow();
        assert!(
            calls.iter().any(|(text, weight, italic, _)| {
                text == "plain"
                    && weight.as_deref() == Some("normal")
                    && italic.as_deref() == Some("normal")
            }),
            "{calls:?}"
        );
        drop(calls);
        let html = measure_markdown_with_inline_styles(
            &measurer,
            "plain **bold**",
            &style,
            None,
            WrapMode::HtmlLike,
        );
        assert_eq!(html.width, 200.0);
    }

    #[test]
    fn html_markdown_measurement_keeps_code_span_literals() {
        let metrics = measure_markdown_with_inline_styles(
            &SoftHyphenMeasurer::default(),
            "inline: `**not bold**`",
            &TextStyle::default(),
            None,
            WrapMode::HtmlLike,
        );
        assert_eq!(metrics.width, 220.0);
    }

    #[test]
    fn soft_hyphen_inline_min_content_includes_the_selected_glyph() {
        let measurer = SoftHyphenMeasurer::default();
        let style = TextStyle::default();
        let runs = vec![run("alpha\u{00ad}beta", 0)];
        let layout = measure_inline_html_line_layout(&measurer, &runs, &style, Some(50.0));
        assert_eq!(layout.natural_width, 90.0);
        assert_eq!(layout.min_content_width, 60.0);
        assert_eq!(layout.wrapped_width, 60.0);
        assert_eq!(layout.line_count, 2);

        let unbroken = measure_inline_html_line_layout(&measurer, &runs, &style, None);
        assert_eq!(unbroken.wrapped_width, 90.0);
        assert_eq!(unbroken.line_count, 1);
        let terminal = measure_inline_html_line_layout(
            &measurer,
            &[run("alpha\u{00ad}", 0)],
            &style,
            Some(50.0),
        );
        assert_eq!(terminal.natural_width, 50.0);
        assert_eq!(terminal.min_content_width, 50.0);
        assert_eq!(terminal.line_count, 1);
    }

    #[test]
    fn soft_hyphen_inline_glyph_inherits_its_run_and_parent_style() {
        let measurer = SoftHyphenMeasurer::default();
        let mut style = TextStyle {
            font_family: Some("inherited-family".to_string()),
            ..TextStyle::default()
        };
        let runs = vec![run("al", 0), run("pha\u{00ad}", 1), run("beta", 2)];
        let layout = measure_inline_html_line_layout(&measurer, &runs, &style, Some(100.0));
        assert_eq!(layout.natural_width, 120.0);
        assert_eq!(layout.min_content_width, 100.0);
        assert_eq!(layout.wrapped_width, 100.0);
        assert_eq!(layout.line_count, 2);
        assert!(measurer.calls.borrow().contains(&(
            "pha-".to_string(),
            Some("700".to_string()),
            None,
            Some("inherited-family".to_string()),
        )));

        style.font_weight = Some("700".to_string());
        let layout = measure_inline_html_line_layout(
            &measurer,
            &[run("alpha\u{00ad}beta", 0)],
            &style,
            Some(120.0),
        );
        assert_eq!(layout.min_content_width, 120.0);
        assert_eq!(layout.wrapped_width, 120.0);
        assert_eq!(layout.line_count, 2);
    }

    #[test]
    fn soft_hyphen_inline_rewinds_to_an_earlier_fitting_break() {
        let measurer = SoftHyphenMeasurer::default();
        let layout = measure_inline_html_line_layout(
            &measurer,
            &[run("x alpha\u{00ad}beta", 0)],
            &TextStyle::default(),
            Some(70.0),
        );
        assert_eq!(layout.natural_width, 110.0);
        assert_eq!(layout.min_content_width, 60.0);
        assert_eq!(layout.wrapped_width, 60.0);
        assert_eq!(layout.line_count, 3);
    }

    #[test]
    fn soft_hyphen_inline_callback_shapes_the_complete_visible_style_group() {
        let measurer = SoftHyphenMeasurer {
            kern_hyphen: true,
            ..Default::default()
        };
        let layout = measure_inline_html_line_layout(
            &measurer,
            &[run("al", 0), run("pha\u{00ad}beta", 0)],
            &TextStyle::default(),
            Some(55.0),
        );
        assert_eq!(layout.min_content_width, 55.0);
        assert_eq!(layout.wrapped_width, 55.0);
        assert_eq!(layout.line_count, 2);
        let calls = measurer.calls.borrow();
        assert!(calls.iter().any(|call| call.0 == "alpha-"));
        assert!(calls.iter().all(|call| call.0 != "-"));
    }

    #[test]
    fn soft_hyphen_inline_complete_word_fit_does_not_select_a_wider_hyphen() {
        struct WideHyphenMeasurer;
        impl TextMeasurer for WideHyphenMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
            }

            fn measure_wrapped(
                &self,
                text: &str,
                style: &TextStyle,
                _max_width: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                TextMetrics {
                    width: text
                        .chars()
                        .map(|ch| match ch {
                            '\u{00ad}' => 0.0,
                            '-' => 30.0,
                            _ => 10.0,
                        })
                        .sum(),
                    height: style.font_size,
                    line_count: 1,
                }
            }
        }
        let layout = measure_inline_html_line_layout(
            &WideHyphenMeasurer,
            &[run("alpha\u{00ad}b", 0)],
            &TextStyle::default(),
            Some(60.0),
        );
        assert_eq!(layout.natural_width, 60.0);
        assert_eq!(layout.wrapped_width, 60.0);
        assert_eq!(layout.line_count, 1);
    }

    #[test]
    fn soft_hyphen_inline_html_entity_preserves_conditional_styled_measurement() {
        let measurer = SoftHyphenMeasurer::default();
        let layout = measure_html_with_inline_styles(
            &measurer,
            "<strong>alpha&shy;</strong>beta",
            &TextStyle::default(),
            Some(120.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(layout.width, 120.0);
        assert_eq!(layout.line_count, 2);
        assert!(
            measurer
                .calls
                .borrow()
                .iter()
                .any(|call| { call.0 == "alpha-" && call.1.as_deref() == Some("700") })
        );
    }

    #[test]
    fn soft_hyphen_inline_stream_preserves_combining_boundaries_in_one_chunk() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let carrier = inline_html_carrier(&measurer);
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let runs = [run("", 1)];
        let mut profiles = BuiltinInlineWidthProfiles::new(carrier, &runs);
        let mut stats = InlineHtmlPlanningStats::default();
        let mut group = BuiltinInlineGroupWidth::new(&mut profiles, 0, &runs, &style, &mut stats);
        let text = format!("a{}", "\u{00ad}\u{0301}".repeat(8192));
        group.push_text(&text, &mut stats);
        assert_eq!(group.width.pushed_chunks, 1);
        assert_eq!(group.width_px(), 4.3);

        let mut stats = InlineHtmlPlanningStats::default();
        let runs = vec![run(format!("W {text}"), 1)];
        let layout = measure_inline_html_line_layout_with_carrier_and_stats(
            &measurer,
            carrier,
            &runs,
            &style,
            Some(15.0),
            &mut stats,
        );
        assert_eq!(layout.line_count, 2);
        assert!((layout.natural_width - 16.1).abs() < 1.0e-12);
        assert_eq!(stats.source_bytes, runs[0].text.len());
        assert!(
            stats.builtin_stream_bytes <= stats.source_bytes * 3,
            "stats={stats:?}"
        );
    }

    #[test]
    fn soft_hyphen_inline_builtin_and_opaque_routes_choose_the_same_lines() {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
        let deterministic = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        for runs in [
            vec![run("alpha\u{00ad}beta", 0)],
            vec![run("x ", 2), run("alpha\u{00ad}", 1), run("beta", 0)],
            vec![run("ab\u{00ad}cd\u{00ad}ef\u{00ad}gh", 0)],
        ] {
            for max_width in [20.0, 32.0, 48.0] {
                let expected =
                    measure_inline_html_line_layout(&deterministic, &runs, &style, Some(max_width));
                let actual = measure_inline_html_line_layout_with_carrier(
                    &builtin,
                    inline_html_carrier(&builtin),
                    &runs,
                    &style,
                    Some(max_width),
                );
                if expected.natural_width > max_width {
                    assert_layout_eq(actual, expected);
                } else {
                    assert_eq!(actual.line_count, expected.line_count);
                    assert_eq!(actual.wrapped_width, expected.wrapped_width);
                }
            }
        }
    }

    #[test]
    fn opaque_measurer_observes_the_legacy_request_order() {
        let measurer = RecordingMeasurer::default();
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![run("aa ", 0), run("BB", 1), run(" cc", 0)];
        let mut stats = InlineHtmlPlanningStats::default();

        let layout = measure_inline_html_line_layout_with_stats(
            &measurer,
            &runs,
            &style,
            Some(4.0),
            &mut stats,
        );

        let calls = measurer.calls.into_inner();
        let regular = |text: &str| (text.to_string(), None, None, Some("sans-serif".to_string()));
        let bold = |text: &str| {
            (
                text.to_string(),
                Some("700".to_string()),
                None,
                Some("sans-serif".to_string()),
            )
        };
        assert_eq!(
            calls,
            vec![
                regular("aa "),
                bold("BB"),
                regular(" cc"),
                regular("aa "),
                bold("BB"),
                regular(" "),
                regular("cc"),
                regular("aa "),
                regular("aa "),
                bold("BB"),
                regular(" "),
                regular("aa "),
                bold("BB"),
                regular(" cc"),
                bold("BB"),
                regular(" "),
                regular("cc"),
            ]
        );
        assert_eq!(layout.line_count, 3);
        assert_eq!(layout.natural_width, 8.0);
        assert_eq!(layout.min_content_width, 3.0);
        assert_eq!(layout.wrapped_width, 3.0);
        assert_eq!(stats.backend_requests, calls.len());
        assert_eq!(
            stats.backend_request_bytes,
            calls.iter().map(|(text, ..)| text.len()).sum::<usize>()
        );
    }

    #[test]
    fn opaque_same_style_fragments_preserve_payload_order_without_recopying() {
        let measurer = RecordingMeasurer::default();
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![run("aa ", 0), run("bb ", 0), run("cc", 0)];
        let mut stats = InlineHtmlPlanningStats::default();

        let layout = measure_inline_html_line_layout_with_stats(
            &measurer,
            &runs,
            &style,
            Some(4.0),
            &mut stats,
        );

        assert_eq!(
            measurer
                .calls
                .into_inner()
                .into_iter()
                .map(|(text, ..)| text)
                .collect::<Vec<_>>(),
            vec![
                "aa ", "bb ", "cc", "aa ", "bb ", "cc", "aa ", "aa bb ", "aa ", "bb cc", "bb ",
                "cc",
            ]
        );
        assert_eq!(layout.line_count, 3);
        assert_eq!(stats.measurement_payload_copy_bytes, 0);
    }

    #[test]
    fn opaque_public_html_operation_preserves_the_full_request_order() {
        let measurer = RecordingMeasurer::default();
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };

        let metrics = measure_html_with_inline_styles(
            &measurer,
            "aa <strong>BB</strong> cc",
            &style,
            Some(4.0),
            WrapMode::HtmlLike,
        );

        let calls = measurer.calls.into_inner();
        let regular = |text: &str| (text.to_string(), None, None, Some("sans-serif".to_string()));
        let bold = |text: &str| {
            (
                text.to_string(),
                Some("700".to_string()),
                None,
                Some("sans-serif".to_string()),
            )
        };
        let layout_pass = vec![
            regular("aa "),
            bold("BB"),
            regular(" cc"),
            regular("aa "),
            bold("BB"),
            regular(" "),
            regular("cc"),
            regular("aa "),
            regular("aa "),
            bold("BB"),
            regular(" "),
            regular("aa "),
            bold("BB"),
            regular(" cc"),
            bold("BB"),
            regular(" "),
            regular("cc"),
        ];
        let mut expected = vec![
            regular("aa BB cc"),
            regular("aa "),
            bold("BB"),
            regular(" cc"),
        ];
        // Mermaid remeasures after switching from nowrap to break-spaces at the width boundary.
        expected.extend(layout_pass.iter().cloned());
        expected.extend(layout_pass);
        assert_eq!(calls, expected);
        assert_eq!(metrics.width, 4.0);
        assert_eq!(metrics.line_count, 3);
    }

    #[test]
    fn opaque_public_html_operation_preserves_failure_position() {
        struct PanickingMeasurer {
            panic_at: usize,
            calls: RefCell<Vec<String>>,
        }

        impl TextMeasurer for PanickingMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
            }

            fn measure_wrapped(
                &self,
                text: &str,
                style: &TextStyle,
                _max_width: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                let call_index = self.calls.borrow().len() + 1;
                self.calls.borrow_mut().push(text.to_string());
                assert_ne!(call_index, self.panic_at, "scripted measurement failure");
                TextMetrics {
                    width: text.len() as f64,
                    height: style.font_size,
                    line_count: 1,
                }
            }
        }

        let measurer = PanickingMeasurer {
            panic_at: 5,
            calls: RefCell::new(Vec::new()),
        };
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            measure_html_with_inline_styles(
                &measurer,
                "aa <strong>BB</strong> cc",
                &style,
                Some(4.0),
                WrapMode::HtmlLike,
            )
        }));

        assert!(result.is_err());
        assert_eq!(
            measurer.calls.into_inner(),
            vec!["aa BB cc", "aa ", "BB", " cc", "aa "]
        );
    }

    #[test]
    fn opaque_stateful_measurer_keeps_fit_and_overflow_call_semantics() {
        let measurer = ScriptedStatefulMeasurer::new(vec![
            10.0, 10.0, 10.0, // natural width
            3.0, 2.0, 1.0, 2.0, // min-content segments
            3.0, // first candidate
            3.0, 2.0, 1.0, // second candidate fits
            20.0, 2.0, 4.0, // third candidate overflows
            3.0, 2.0, 1.0, // committed checkpoint
            2.0, // final segment
        ]);
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![run("aa ", 0), run("BB", 1), run(" cc", 0)];
        let mut stats = InlineHtmlPlanningStats::default();

        let layout = measure_inline_html_line_layout_with_stats(
            &measurer,
            &runs,
            &style,
            Some(25.0),
            &mut stats,
        );

        assert_eq!(
            measurer.calls.into_inner(),
            vec![
                "aa ", "BB", " cc", "aa ", "BB", " ", "cc", "aa ", "aa ", "BB", " ", "aa ", "BB",
                " cc", "aa ", "BB", " ", "cc",
            ]
        );
        assert_eq!(layout.natural_width, 30.0);
        assert_eq!(layout.min_content_width, 3.0);
        assert_eq!(layout.wrapped_width, 6.0);
        assert_eq!(layout.line_count, 2);
        assert_eq!(stats.backend_requests, 18);
    }

    #[test]
    fn host_route_keeps_the_full_opaque_html_operation_trace() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.inline-recording-host")
                .expect("valid measurement profile id"),
            "v1",
        )
        .expect("valid measurement profile identity");
        let policy = TextMeasurementPolicy::host_display(
            identity,
            Arc::new(RecordingHostMeasurer {
                calls: Arc::clone(&calls),
            }),
            [TextMeasurementPhase::Wrap],
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin host render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let metrics = measure_html_with_inline_styles(
            &measurer,
            "aa <strong>BB</strong> cc",
            &style,
            Some(4.0),
            WrapMode::HtmlLike,
        );

        let calls = calls.lock().expect("host call log lock").clone();
        assert_eq!(calls[0].0, TextMeasurementOperation::Wrapped);
        assert!(calls[1..].iter().all(|(operation, _)| {
            *operation == TextMeasurementOperation::CanvasMeasureTextWidth
        }));
        assert_eq!(
            calls
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>(),
            {
                let layout_pass = [
                    "aa ", "BB", " cc", "aa ", "BB", " ", "cc", "aa ", "aa ", "BB", " ", "aa ",
                    "BB", " cc", "BB", " ", "cc",
                ];
                let mut expected = vec!["aa BB cc", "aa ", "BB", " cc"];
                expected.extend(layout_pass);
                expected.extend(layout_pass);
                expected
            }
        );
        assert_eq!(metrics.width, 4.0);
        assert_eq!(metrics.line_count, 3);
    }

    #[cfg(feature = "diagram-er")]
    #[test]
    fn font_aware_xhtml_preserves_unstyled_opaque_requests() {
        let legacy = RecordingMeasurer::default();
        let terminal = RecordingMeasurer::default();
        let style = TextStyle::default();
        let html = "<p>aa <strong>BB</strong> cc</p>";
        let expected =
            measure_html_with_inline_styles(&legacy, html, &style, Some(4.0), WrapMode::HtmlLike);
        let actual = measure_xhtml_label_fragment_with_terminal_fonts(
            &terminal,
            html,
            &style,
            Some(4.0),
            16.0,
        );
        assert_eq!(terminal.calls.into_inner(), legacy.calls.into_inner());
        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.height, expected.height);
        assert_eq!(actual.line_count, expected.line_count);
    }

    #[cfg(feature = "diagram-er")]
    #[test]
    fn wrapped_terminal_fonts_keep_builtin_and_opaque_line_selection() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
        let opaque = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 14.0,
            ..TextStyle::default()
        };
        for html in [
            "<p><span style='font-size:64px'>A B C D E</span></p>",
            "<p><span style='font-size:64px'>A</span> B C D E F G H I J K L M N O P</p>",
            concat!(
                "<p><span style='font-size:64px;font-family:monospace;font-weight:700'>",
                "AB&shy;CD EF</span> <span style='font-size:28px;font-style:italic'>GH IJ</span></p>",
            ),
        ] {
            for width in [40.0, 100.0, 1000.0] {
                let expected = measure_xhtml_label_fragment_with_terminal_fonts(
                    &opaque,
                    html,
                    &style,
                    Some(width),
                    16.0,
                );
                let actual = measure_xhtml_label_fragment_with_terminal_fonts(
                    &builtin,
                    html,
                    &style,
                    Some(width),
                    16.0,
                );
                assert_eq!(actual.width, expected.width, "{html} at {width}");
                assert_eq!(actual.height, expected.height, "{html} at {width}");
                assert_eq!(actual.line_count, expected.line_count, "{html} at {width}");
            }
        }
        let mixed = measure_xhtml_label_fragment_with_terminal_fonts(
            &builtin,
            "<p><span style='font-size:64px'>A</span> B C D E F G H I J K L M N O P</p>",
            &style,
            Some(100.0),
            16.0,
        );
        assert!(mixed.line_count > 1);
        assert_eq!(mixed.height, 96.0 + (mixed.line_count - 1) as f64 * 21.0);
    }

    #[cfg(feature = "diagram-er")]
    #[test]
    fn wrapped_terminal_fonts_preserve_natural_width_activation_and_min_content() {
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..DeterministicTextMeasurer::default()
        };
        let style = TextStyle {
            font_size: 14.0,
            ..TextStyle::default()
        };
        let html = "<p><span style='font-size:64px'>A   B</span></p>";
        let fits = measure_xhtml_label_fragment_with_terminal_fonts(
            &measurer,
            html,
            &style,
            Some(193.0),
            16.0,
        );
        assert_eq!((fits.width, fits.height, fits.line_count), (192.0, 96.0, 1));
        let activated = measure_xhtml_label_fragment_with_terminal_fonts(
            &measurer,
            html,
            &style,
            Some(192.0),
            16.0,
        );
        assert_eq!(
            (activated.width, activated.height, activated.line_count),
            (192.0, 192.0, 2),
            "equality activates break-spaces and retains the authored spaces",
        );
        let min_content = measure_xhtml_label_fragment_with_terminal_fonts(
            &measurer,
            "<p><span style='font-size:64px'>ABCDE</span></p>",
            &style,
            Some(100.0),
            16.0,
        );
        assert_eq!(
            (
                min_content.width,
                min_content.height,
                min_content.line_count
            ),
            (320.0, 96.0, 1),
            "an unbreakable styled word expands min-content instead of clipping its viewport",
        );
    }

    #[cfg(feature = "diagram-er")]
    #[test]
    fn wrapped_terminal_fonts_route_inherited_fonts_and_selected_heights_to_the_host() {
        type FontCall = (TextMeasurementOperation, String, TextStyle, Option<f64>);
        struct FontHost(Arc<Mutex<Vec<FontCall>>>);

        impl HostTextMeasurer for FontHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                self.0.lock().unwrap().push((
                    request.operation,
                    request.text.to_owned(),
                    request.style.clone(),
                    request.max_width,
                ));
                let width = request.text.chars().count() as f64 * request.style.font_size;
                Ok(Some(match request.operation {
                    TextMeasurementOperation::CanvasMeasureTextWidth => {
                        HostTextMeasurement::Length(width)
                    }
                    TextMeasurementOperation::HtmlLineHeight => {
                        HostTextMeasurement::Length(request.style.font_size * 2.0)
                    }
                    _ => HostTextMeasurement::Metrics(TextMetrics {
                        width,
                        height: request.style.font_size * 2.0,
                        line_count: 1,
                    }),
                }))
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.wrapped-terminal-font-host").unwrap(),
            "v1",
        )
        .unwrap();
        let policy = TextMeasurementPolicy::host_display(
            identity,
            Arc::new(FontHost(Arc::clone(&calls))),
            [TextMeasurementPhase::Wrap],
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle {
            font_size: 14.0,
            ..TextStyle::default()
        };
        let html = concat!(
            "<p><span style='font-size:64px;font-family:monospace;font-weight:700;font-style:italic'>",
            "<strong style='font-size:inherit;font-family:inherit;font-weight:inherit;font-style:inherit'>",
            "A</strong></span><br/>B<br/>C</p>",
        );
        let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
            &measurer,
            html,
            &style,
            Some(100.0),
            16.0,
        );
        assert_eq!(
            (metrics.width, metrics.height, metrics.line_count),
            (64.0, 184.0, 3)
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].0, TextMeasurementOperation::Wrapped);
        assert_eq!(calls[0].1, "A\nB\nC");
        assert_eq!(calls[0].2.font_size, 14.0);
        assert_eq!(calls[0].3, Some(100.0));
        for operation in [
            TextMeasurementOperation::CanvasMeasureTextWidth,
            TextMeasurementOperation::HtmlLineHeight,
        ] {
            let a_calls = calls
                .iter()
                .filter(|call| call.0 == operation && call.1 == "A")
                .collect::<Vec<_>>();
            assert!(!a_calls.is_empty());
            for call in a_calls {
                assert_eq!(call.2.font_size, 64.0);
                assert_eq!(call.2.font_family.as_deref(), Some("monospace"));
                assert_eq!(call.2.font_weight.as_deref(), Some("700"));
                assert_eq!(call.2.font_style.as_deref(), Some("italic"));
            }
        }
        for text in ["B", "C"] {
            let line_calls = calls
                .iter()
                .filter(|call| call.0 == TextMeasurementOperation::HtmlLineHeight && call.1 == text)
                .collect::<Vec<_>>();
            assert!(!line_calls.is_empty());
            assert!(line_calls.iter().all(|call| call.2.font_size == 14.0));
        }
    }

    fn assert_layout_eq(left: InlineHtmlLineLayout, right: InlineHtmlLineLayout) {
        assert!((left.natural_width - right.natural_width).abs() <= 1.0e-9);
        assert!((left.wrapped_width - right.wrapped_width).abs() <= 1.0e-9);
        assert!((left.min_content_width - right.min_content_width).abs() <= 1.0e-9);
        assert_eq!(left.line_count, right.line_count);
    }

    fn assert_layout_bits_eq(
        left: InlineHtmlLineLayout,
        right: InlineHtmlLineLayout,
        max_width: f64,
    ) {
        assert_eq!(
            left.natural_width.to_bits(),
            right.natural_width.to_bits(),
            "natural width mismatch at max_width={max_width}: left={left:?}, right={right:?}"
        );
        assert_eq!(
            left.wrapped_width.to_bits(),
            right.wrapped_width.to_bits(),
            "wrapped width mismatch at max_width={max_width}: left={left:?}, right={right:?}"
        );
        if right.natural_width > max_width {
            assert_eq!(
                left.min_content_width.to_bits(),
                right.min_content_width.to_bits()
            );
        }
        assert_eq!(
            left.line_count, right.line_count,
            "line-count mismatch at max_width={max_width}: left={left:?}, right={right:?}"
        );
    }

    fn next_down(value: f64) -> f64 {
        debug_assert!(value.is_finite() && value > 0.0);
        f64::from_bits(value.to_bits() - 1)
    }

    fn next_up(value: f64) -> f64 {
        debug_assert!(value.is_finite() && value >= 0.0);
        f64::from_bits(value.to_bits() + 1)
    }

    #[test]
    fn qualified_builtin_routes_preserve_ulp_wrap_boundaries_and_backend_sum_order() {
        fn assert_backend(
            backend: &dyn TextMeasurer,
            routed: &dyn TextMeasurer,
            carrier: InlineHtmlMeasurementCarrier,
            style: &TextStyle,
        ) {
            let runs = vec![run("AV ", 0), run("To ", 1), run("fi", 0)];
            let bold_style = inline_text_style(style, true, false, false);
            let boundary = backend
                .measure_wrapped("AV ", style, None, WrapMode::HtmlLike)
                .width
                + backend
                    .measure_wrapped("To ", &bold_style, None, WrapMode::HtmlLike)
                    .width;

            for max_width in [next_down(boundary), boundary, next_up(boundary)] {
                let expected =
                    measure_inline_html_line_layout(backend, &runs, style, Some(max_width));
                let actual = measure_inline_html_line_layout_with_carrier(
                    routed,
                    carrier,
                    &runs,
                    style,
                    Some(max_width),
                );
                assert_layout_bits_eq(actual, expected, max_width);
            }

            let same_style_runs = vec![run("A", 0), run("V ", 0), run("office ", 0), run("fi", 0)];
            for max_width in [32.0, 64.0, 96.0] {
                let expected = measure_inline_html_line_layout(
                    backend,
                    &same_style_runs,
                    style,
                    Some(max_width),
                );
                let actual = measure_inline_html_line_layout_with_carrier(
                    routed,
                    carrier,
                    &same_style_runs,
                    style,
                    Some(max_width),
                );
                assert_layout_bits_eq(actual, expected, max_width);
            }

            let escaped_break_runs = vec![run("wide<br   />tail end", 0)];
            for max_width in [24.0, 64.0, 128.0] {
                let expected = measure_inline_html_line_layout(
                    backend,
                    &escaped_break_runs,
                    style,
                    Some(max_width),
                );
                let actual = measure_inline_html_line_layout_with_carrier(
                    routed,
                    carrier,
                    &escaped_break_runs,
                    style,
                    Some(max_width),
                );
                assert_layout_bits_eq(actual, expected, max_width);

                let html = "wide&lt;br   /&gt;tail end";
                let expected = measure_html_with_inline_styles(
                    backend,
                    html,
                    style,
                    Some(max_width),
                    WrapMode::HtmlLike,
                );
                let actual = measure_html_with_inline_styles(
                    routed,
                    html,
                    style,
                    Some(max_width),
                    WrapMode::HtmlLike,
                );
                assert_eq!(actual.width.to_bits(), expected.width.to_bits());
                assert_eq!(actual.height.to_bits(), expected.height.to_bits());
                assert_eq!(actual.line_count, expected.line_count);
            }
        }

        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let builtin_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let builtin = builtin_session.text_measurer(TextMeasurementPhase::Wrap);
        let builtin_carrier = inline_html_carrier(&builtin);
        assert_backend(
            &DeterministicTextMeasurer::default(),
            &builtin,
            builtin_carrier,
            &style,
        );
        let mut unknown_font_style = style.clone();
        unknown_font_style.font_family = Some("fixture-private-font".to_string());
        assert_backend(
            &DeterministicTextMeasurer::default(),
            &builtin,
            builtin_carrier,
            &unknown_font_style,
        );

        let deterministic_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .expect("deterministic text session");
        let deterministic = deterministic_session.text_measurer(TextMeasurementPhase::Wrap);
        assert_backend(
            &DeterministicTextMeasurer::default(),
            &deterministic,
            inline_html_carrier(&deterministic),
            &style,
        );
    }

    #[test]
    fn qualified_builtin_stream_reports_only_actual_backend_requests() {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let html = "alpha <strong>BETA </strong>omega";

        let _ = measure_html_with_inline_styles(
            &measurer,
            html,
            &style,
            Some(48.0),
            WrapMode::HtmlLike,
        );

        let report = session.text_measurement_report();
        assert_eq!(report.entries().len(), 1);
        let summary = &report.entries()[0];
        assert_eq!(
            summary.provenance().operation,
            TextMeasurementOperation::CanvasMeasureTextWidth
        );
        assert!(
            (3..=6).contains(&summary.count()),
            "each of the three visible runs should require at most one fit and one overflow probe"
        );
    }

    #[test]
    fn qualified_builtin_fit_path_skips_break_indexing_and_stream_replay() {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![run("alpha beta ", 0), run("gamma delta", 1)];
        let natural_width =
            measure_inline_runs_width_px(&measurer, &runs, &style, WrapMode::HtmlLike, false);

        for max_width in [None, Some(natural_width), Some(natural_width + 1.0)] {
            let mut stats = InlineHtmlPlanningStats::default();
            let layout = measure_inline_html_line_layout_with_carrier_and_stats(
                &measurer,
                inline_html_carrier(&measurer),
                &runs,
                &style,
                max_width,
                &mut stats,
            );

            assert_eq!(layout.natural_width.to_bits(), natural_width.to_bits());
            assert_eq!(layout.wrapped_width.to_bits(), natural_width.to_bits());
            assert_eq!(layout.line_count, 1);
            assert_eq!(stats.backend_requests, runs.len());
            assert_eq!(stats.break_segments, 0);
            assert_eq!(stats.fragment_refs, 0);
            assert_eq!(stats.builtin_stream_bytes, 0);
            assert_eq!(stats.builtin_checkpoint_copies, 0);
        }
    }

    #[test]
    fn qualified_builtin_routes_match_opaque_layout_for_unicode_and_rollback() {
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![
            run("A\u{301} alpha ", 0),
            run("👩‍💻 beta ", 1),
            run("مرحبا gamma ", 2),
            run("世界 delta", 3),
        ];
        let html =
            "A\u{301} alpha <strong>👩‍💻 beta </strong><em>مرحبا gamma </em><code>世界 delta</code>";

        let builtin_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let builtin = builtin_session.text_measurer(TextMeasurementPhase::Wrap);
        let builtin_expected = measure_inline_html_line_layout(
            &DeterministicTextMeasurer::default(),
            &runs,
            &style,
            Some(96.0),
        );
        let builtin_actual = measure_inline_html_line_layout_with_carrier(
            &builtin,
            inline_html_carrier(&builtin),
            &runs,
            &style,
            Some(96.0),
        );
        assert_layout_eq(builtin_actual, builtin_expected);
        let builtin_actual =
            measure_html_with_inline_styles(&builtin, html, &style, Some(96.0), WrapMode::HtmlLike);
        let builtin_expected = measure_html_with_inline_styles(
            &DeterministicTextMeasurer::default(),
            html,
            &style,
            Some(96.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(builtin_actual.width, builtin_expected.width);
        assert_eq!(builtin_actual.height, builtin_expected.height);
        assert_eq!(builtin_actual.line_count, builtin_expected.line_count);

        let mut unknown_font_style = style.clone();
        unknown_font_style.font_family = Some("fixture-private-font".to_string());
        let fallback_expected = measure_inline_html_line_layout(
            &DeterministicTextMeasurer::default(),
            &runs,
            &unknown_font_style,
            Some(96.0),
        );
        let fallback_actual = measure_inline_html_line_layout_with_carrier(
            &builtin,
            inline_html_carrier(&builtin),
            &runs,
            &unknown_font_style,
            Some(96.0),
        );
        assert_layout_eq(fallback_actual, fallback_expected);
        let fallback_actual = measure_html_with_inline_styles(
            &builtin,
            html,
            &unknown_font_style,
            Some(96.0),
            WrapMode::HtmlLike,
        );
        let fallback_expected = measure_html_with_inline_styles(
            &DeterministicTextMeasurer::default(),
            html,
            &unknown_font_style,
            Some(96.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(fallback_actual.width, fallback_expected.width);
        assert_eq!(fallback_actual.height, fallback_expected.height);
        assert_eq!(fallback_actual.line_count, fallback_expected.line_count);

        let deterministic_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .expect("deterministic text session");
        let deterministic = deterministic_session.text_measurer(TextMeasurementPhase::Wrap);
        let deterministic_expected = measure_inline_html_line_layout(
            &DeterministicTextMeasurer::default(),
            &runs,
            &style,
            Some(96.0),
        );
        let deterministic_actual = measure_inline_html_line_layout_with_carrier(
            &deterministic,
            inline_html_carrier(&deterministic),
            &runs,
            &style,
            Some(96.0),
        );
        assert_layout_eq(deterministic_actual, deterministic_expected);
        let deterministic_actual = measure_html_with_inline_styles(
            &deterministic,
            html,
            &style,
            Some(96.0),
            WrapMode::HtmlLike,
        );
        let deterministic_expected = measure_html_with_inline_styles(
            &DeterministicTextMeasurer::default(),
            html,
            &style,
            Some(96.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(deterministic_actual.width, deterministic_expected.width);
        assert_eq!(deterministic_actual.height, deterministic_expected.height);
        assert_eq!(
            deterministic_actual.line_count,
            deterministic_expected.line_count
        );
    }

    #[test]
    fn rich_html_path_handles_many_styles_breaks_entities_and_unicode() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let mut html = String::new();
        for index in 0..1024 {
            let (open, close) = match index % 3 {
                0 => ("<strong>", "</strong>"),
                1 => ("<em>", "</em>"),
                _ => ("<code>", "</code>"),
            };
            html.push_str(open);
            html.push_str("a ");
            html.push_str(close);
        }
        html.push_str("&amp;\r\n<br>👩‍💻 مرحبا 世界");

        let metrics = measure_html_with_inline_styles(
            &measurer,
            &html,
            &style,
            Some(96.0),
            WrapMode::HtmlLike,
        );

        assert!(metrics.width.is_finite() && metrics.width > 0.0);
        assert!(metrics.height.is_finite() && metrics.height > 0.0);
        assert!(metrics.line_count > 2);
    }

    #[test]
    fn inline_planner_work_is_additive_for_fixed_byte_r_by_k_matrix() {
        const SOURCE_BYTES: usize = 4096;
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };

        for run_count in [1, 32, 256] {
            for break_count in [0, 31, 255] {
                let runs = fixed_byte_runs(SOURCE_BYTES, run_count, break_count);
                let measurer = RecordingMeasurer::default();
                let mut stats = InlineHtmlPlanningStats::default();
                let _ = measure_inline_html_line_layout_with_stats(
                    &measurer,
                    &runs,
                    &style,
                    Some(32.0),
                    &mut stats,
                );

                let segment_count = break_count + 1;
                let fragment_bound = run_count + break_count;
                assert_eq!(stats.source_bytes, SOURCE_BYTES);
                assert_eq!(stats.break_segments, segment_count);
                assert!(stats.fragment_refs <= fragment_bound);
                assert_eq!(stats.fragment_capacity_refs, fragment_bound);
                assert_eq!(stats.run_visits, run_count * 2 + stats.fragment_refs);
                assert!(stats.line_plan_updates <= segment_count * 2);
                assert_eq!(stats.line_plan_bytes, std::mem::size_of::<InlineLinePlan>());

                let temporary_byte_bound = SOURCE_BYTES
                    + segment_count * std::mem::size_of::<&str>()
                    + fragment_bound * std::mem::size_of::<InlineRunFragment>()
                    + segment_count * std::mem::size_of::<InlineBreakSegment>()
                    + fragment_bound * std::mem::size_of::<usize>()
                    + std::mem::size_of::<InlineLinePlan>();
                assert!(stats.logical_temporary_bytes() <= temporary_byte_bound);
                assert_eq!(stats.backend_requests, measurer.calls.borrow().len());
                assert_eq!(
                    stats.backend_request_bytes,
                    measurer
                        .calls
                        .borrow()
                        .iter()
                        .map(|(text, ..)| text.len())
                        .sum::<usize>()
                );
            }
        }
    }

    #[test]
    fn opaque_same_style_active_line_work_is_linear_in_fragments_and_requests() {
        const SOURCE_BYTES: usize = 4096;
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };

        for run_count in [32, 128, 256] {
            for segment_count in [32, 64, 128] {
                let mut runs = fixed_byte_runs(SOURCE_BYTES, run_count, segment_count - 1);
                for run in &mut runs {
                    run.bold = false;
                    run.italic = false;
                    run.code = false;
                }
                let measurer = RecordingMeasurer::default();
                let mut stats = InlineHtmlPlanningStats::default();
                let _ = measure_inline_html_line_layout_with_stats(
                    &measurer,
                    &runs,
                    &style,
                    Some(32.0),
                    &mut stats,
                );

                let fragment_requests = stats.backend_requests - run_count;
                assert_eq!(
                    stats.fragment_measure_visits,
                    stats.fragment_refs + fragment_requests,
                    "r={run_count}, k={segment_count}, stats={stats:?}"
                );
                assert_eq!(stats.measurement_payload_copy_bytes, 0);
            }
        }
    }

    #[test]
    fn opaque_same_style_utf8_fragments_borrow_source_across_rollback() {
        let measurer = RecordingMeasurer::default();
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let runs = vec![run("A\u{301} ", 0), run("👩‍💻 ", 0), run("世界", 0)];
        let mut stats = InlineHtmlPlanningStats::default();

        let layout = measure_inline_html_line_layout_with_stats(
            &measurer,
            &runs,
            &style,
            Some(8.0),
            &mut stats,
        );

        assert!(layout.line_count > 1);
        assert_eq!(stats.measurement_payload_copy_bytes, 0);
    }

    #[test]
    fn builtin_terminal_line_height_excludes_collapsed_tails_and_keeps_visible_boxes() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
        let opaque = DeterministicTextMeasurer::default();
        let base = TextStyle {
            font_size: 16.0,
            ..Default::default()
        };
        for (html, expected_height) in [
            ("A<span style='font-size:100px'> </span>", 24.0),
            ("<span style='font-size:100px'>&nbsp;</span>", 150.0),
            (
                "<span style='font-size:100px'><i class='fa fa-car'></i></span>",
                150.0,
            ),
        ] {
            for max_width in [None, Some(1000.0)] {
                let expected = measure_html_with_inline_styles(
                    &opaque,
                    html,
                    &base,
                    max_width,
                    WrapMode::HtmlLike,
                );
                let actual = measure_html_with_inline_styles(
                    &builtin,
                    html,
                    &base,
                    max_width,
                    WrapMode::HtmlLike,
                );
                assert_eq!(actual.height, expected_height, "{html}/{max_width:?}");
                assert_eq!(actual.width, expected.width, "{html}/{max_width:?}");
                assert_eq!(actual.height, expected.height, "{html}/{max_width:?}");
                assert_eq!(
                    actual.line_count, expected.line_count,
                    "{html}/{max_width:?}"
                );
            }
        }
    }

    #[test]
    fn builtin_terminal_fonts_keep_linear_stream_work_near_the_natural_width() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let base = TextStyle::default();
        for words in [64, 512, 8192] {
            for run_count in [1, 16] {
                let mut runs = (0..run_count)
                    .map(|index| InlineTextRun {
                        text: "x ".repeat(words / run_count),
                        terminal_style: Some(Arc::new(TextStyle {
                            font_size: if index % 2 == 0 { 16.0 } else { 22.0 },
                            font_weight: Some("900".to_owned()),
                            ..base.clone()
                        })),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();
                runs.last_mut().unwrap().text.pop();
                let bytes: usize = runs.iter().map(|run| run.text.len()).sum();
                let natural = measure_inline_runs_width_px(
                    &measurer,
                    &runs,
                    &base,
                    WrapMode::HtmlLike,
                    false,
                );
                let mut stats = InlineHtmlPlanningStats::default();
                let layout = measure_inline_html_line_layout_with_carrier_and_stats(
                    &measurer,
                    inline_html_carrier(&measurer),
                    &runs,
                    &base,
                    Some(natural - 0.01),
                    &mut stats,
                );
                assert_eq!(layout.line_count, 2, "{words}/{run_count}");
                assert_eq!(stats.backend_requests, run_count);
                assert_eq!(stats.backend_request_bytes, bytes);
                assert!(
                    stats.builtin_stream_bytes <= 3 * bytes,
                    "{words}/{run_count}: {stats:?}"
                );
                assert!(stats.builtin_profile_initializations <= run_count);
                assert_eq!(
                    stats.builtin_profile_cache_bytes,
                    std::mem::size_of::<BuiltinInlineWidthProfiles>()
                        + run_count * std::mem::size_of::<Option<BuiltinHtmlLineWidth>>()
                );
            }
        }

        // Retain a small same-font reference to expose the growing-prefix cost of the old route.
        for words in [32, 64, 128] {
            let runs = [InlineTextRun {
                text: vec!["x"; words].join(" "),
                terminal_style: Some(Arc::new(base.clone())),
                ..Default::default()
            }];
            let natural = measurer.measure_html_line_width_px(&runs[0].text, &base);
            let mut stats = InlineHtmlPlanningStats::default();
            measure_inline_html_line_layout_with_stats(
                &measurer,
                &runs,
                &base,
                Some(natural - 0.01),
                &mut stats,
            );
            assert!(
                stats.backend_request_bytes >= words * words,
                "{words}: {stats:?}"
            );
        }
    }

    #[test]
    fn builtin_terminal_font_streaming_matches_the_opaque_width_and_height_reference() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let base = TextStyle::default();
        let large = Arc::new(TextStyle {
            font_size: 24.0,
            font_weight: Some("900".to_owned()),
            ..base.clone()
        });
        let small = Arc::new(TextStyle {
            font_size: 12.0,
            font_style: Some("italic".to_owned()),
            ..base.clone()
        });
        for text in [
            "alpha beta gamma",
            "alpha\u{00ad}beta gamma",
            "A\u{301} 👩‍💻 世界\u{200b}مرحبا",
        ] {
            for with_icon in [false, true] {
                let mut runs = vec![InlineTextRun {
                    text: "alpha ".to_owned(),
                    terminal_style: Some(Arc::clone(&large)),
                    ..Default::default()
                }];
                if with_icon {
                    runs.push(InlineTextRun {
                        text: "\u{fffc}".to_owned(),
                        terminal_style: Some(Arc::clone(&large)),
                        replaced_width_px: Some(30.0),
                        ..Default::default()
                    });
                }
                runs.push(InlineTextRun {
                    text: text.to_owned(),
                    terminal_style: Some(Arc::clone(&small)),
                    ..Default::default()
                });
                for max_width in [None, Some(10.0), Some(48.0), Some(120.0), Some(10000.0)] {
                    let expected =
                        measure_inline_html_line_layout(&measurer, &runs, &base, max_width);
                    let actual = measure_inline_html_line_layout_with_carrier(
                        &measurer,
                        inline_html_carrier(&measurer),
                        &runs,
                        &base,
                        max_width,
                    );
                    assert!((actual.natural_width - expected.natural_width).abs() < 1e-9);
                    assert!(
                        (actual.wrapped_width - expected.wrapped_width).abs() < 1e-9,
                        "{text}/{with_icon}/{max_width:?}: {actual:?} != {expected:?}"
                    );
                    assert_eq!(actual.line_count, expected.line_count);
                    assert_eq!(actual.height, expected.height);
                    assert_eq!(actual.last_line_height, expected.last_line_height);
                }
            }
        }
    }

    #[test]
    fn qualified_builtin_active_line_work_is_linear_for_r_by_k_matrix() {
        const SOURCE_BYTES: usize = 4096;
        let style = TextStyle {
            font_family: Some("sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);

        for run_count in [1, 32, 128] {
            for segment_count in [16, 32, 64, 128] {
                let runs = fixed_byte_runs(SOURCE_BYTES, run_count, segment_count - 1);
                let natural_width = measure_inline_runs_width_px(
                    &measurer,
                    &runs,
                    &style,
                    WrapMode::HtmlLike,
                    false,
                );
                let mut stats = InlineHtmlPlanningStats::default();
                let layout = measure_inline_html_line_layout_with_carrier_and_stats(
                    &measurer,
                    inline_html_carrier(&measurer),
                    &runs,
                    &style,
                    Some(natural_width - 1.0e-6),
                    &mut stats,
                );

                let fragment_bound = run_count + segment_count - 1;
                assert_eq!(stats.source_bytes, SOURCE_BYTES);
                assert_eq!(stats.break_segments, segment_count);
                assert!(stats.fragment_refs <= fragment_bound);
                assert!(stats.fragment_measure_visits >= stats.fragment_refs * 2);
                assert!(stats.fragment_measure_visits <= stats.fragment_refs * 3);
                assert!(stats.line_plan_updates <= segment_count * 2);
                assert_eq!(
                    stats.line_plan_bytes,
                    std::mem::size_of::<BuiltinInlineLineWidth>()
                );
                assert_eq!(stats.measurement_payload_copy_bytes, 0);
                assert_eq!(stats.backend_requests, run_count);
                assert_eq!(stats.backend_request_bytes, SOURCE_BYTES);
                assert!(stats.builtin_stream_bytes >= SOURCE_BYTES * 2);
                assert!(
                    stats.builtin_stream_bytes <= SOURCE_BYTES * 3,
                    "r={run_count}, k={segment_count}, stats={stats:?}"
                );
                assert!(stats.builtin_stream_scalars <= stats.builtin_stream_bytes);
                assert_eq!(stats.builtin_profile_initializations, run_count.min(4));
                assert_eq!(
                    stats.builtin_profile_cache_bytes,
                    std::mem::size_of::<BuiltinInlineWidthProfiles>()
                );
                assert!(stats.builtin_group_state_copies <= stats.fragment_measure_visits);
                assert_eq!(stats.builtin_checkpoint_copies, segment_count);
                let temporary_byte_bound = SOURCE_BYTES
                    + segment_count * std::mem::size_of::<&str>()
                    + fragment_bound * std::mem::size_of::<InlineRunFragment>()
                    + segment_count * std::mem::size_of::<InlineBreakSegment>()
                    + std::mem::size_of::<BuiltinInlineLineWidth>()
                    + std::mem::size_of::<BuiltinInlineWidthProfiles>();
                assert!(stats.logical_temporary_bytes() <= temporary_byte_bound);
                assert_eq!(layout.line_count, 2);
            }
        }
    }
}

/// Finds retained inline HTML typography that the caller's measurement policy cannot represent.
/// Admission uses the same Markdown projection and HTML normalization as SVG emission.
#[doc(hidden)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub fn unsupported_html_label_style(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
    unsupported_properties: &[&str],
    control: &merman_model::OperationControl,
) -> Result<Option<String>, merman_model::OperationCancelled> {
    use cssparser::{Delimiter, Parser};
    use quick_xml::{Reader, events::Event};

    control.checkpoint()?;
    let html = crate::graph_label::flowchart_label_html_source(label, label_type, config);
    // Sanitizer-removed attributes and encoded angle brackets cannot affect rendered typography.
    let work_meter = crate::resources::OperationWorkMeter::new_with_control(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    );
    let parse_work = std::cell::Cell::new(0);
    let sink = crate::html::HtmlSanitizeSink::new(
        usize::MAX,
        &work_meter,
        &parse_work,
        merman_model::OperationPhase::Layout,
    );
    let html =
        match merman_markup::sanitize::sanitize_xhtml_with_sink(&html, config.as_config(), &sink) {
            Ok(html) => html,
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                crate::html::HtmlOutputError::Operation(
                    crate::resources::OperationWorkError::Cancelled(error),
                ),
            )) => return Err(error),
            Err(_) => String::new(),
        };
    let mut tags = Reader::from_str(&html);
    tags.config_mut().check_end_names = false;
    tags.config_mut().allow_unmatched_ends = true;
    loop {
        control.checkpoint()?;
        let opening = match tags.read_event() {
            Ok(Event::Start(opening) | Event::Empty(opening)) => opening,
            Ok(Event::Eof) | Err(_) => break,
            _ => continue,
        };
        let Some(style) = opening
            .html_attributes()
            .with_checks(false)
            .filter_map(Result::ok)
            .find(|attr| attr.key.as_ref().eq_ignore_ascii_case("style"))
        else {
            continue;
        };
        let style = merman_model::text::decode_html_entities_to_unicode(style.value.as_ref());
        let element = opening.name().as_ref().to_ascii_lowercase();
        let mut declarations = Parser::new(&style);
        while !declarations.is_exhausted() {
            control.checkpoint()?;
            let declaration = declarations.parse_until_after(Delimiter::Semicolon, |declaration| {
                let property = declaration.expect_ident_cloned()?.to_ascii_lowercase();
                declaration.expect_colon()?;
                let first = declaration.next().ok().cloned();
                let inherits = matches!(
                    &first,
                    Some(cssparser::Token::Ident(value)) if value.eq_ignore_ascii_case("inherit")
                ) && (declaration.is_exhausted()
                    || (declaration.try_parse(cssparser::parse_important).is_ok()
                        && declaration.is_exhausted()));
                while declaration.next().is_ok() {}
                // Inheritance can override the intrinsic emphasis or code face modeled below.
                let overrides_element = match element.as_str() {
                    "strong" | "b" => matches!(property.as_str(), "font-weight" | "font"),
                    "cite" | "dfn" | "em" | "i" | "var" => {
                        matches!(property.as_str(), "font-style" | "font")
                    }
                    "code" | "kbd" | "samp" | "tt" => {
                        matches!(property.as_str(), "font-family" | "font")
                    }
                    _ => false,
                };
                Ok::<_, cssparser::ParseError<()>>((
                    property,
                    first.is_some(),
                    inherits && !overrides_element,
                ))
            });
            control.checkpoint()?;
            if let Ok((property, true, false)) = declaration
                && (matches!(
                    property.as_str(),
                    "font-size" | "font-family" | "font-weight" | "font-style"
                ) || unsupported_properties
                    .iter()
                    .any(|name| property.eq_ignore_ascii_case(name)))
            {
                return Ok(Some(property));
            }
        }
    }
    Ok(None)
}

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
#[test]
fn unsupported_html_style_checks_cancellation_during_fragment_recovery() {
    let control = merman_model::OperationControl::new();
    control.cancel_after_checkpoints(8);
    let label = "<span>label</span>".repeat(64);
    let error = unsupported_html_label_style(
        &label,
        "text",
        &merman_model::presentation::PresentationConfig::default(),
        &[],
        &control,
    )
    .expect_err("typography admission must cancel while recovering HTML");
    assert_eq!(control.checkpoint(), Err(error));
}

/// Measures inline HTML using inherited fonts, element defaults, and supported font longhands.
///
/// This compatibility entry point has no document-root context: `rem` uses `style.font_size`.
/// Family-specific prepared labels can supply a separate root size; callers needing portable
/// geometry should prefer pixel or parent-relative sizes and a matching host font measurer.
pub fn measure_html_with_inline_styles(
    measurer: &dyn TextMeasurer,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    // Keep the public `TextMeasurer` surface request-oriented. A whole-operation override would
    // let an opaque custom measurer bypass the established callback order; only the unnameable
    // routed carrier may select the built-in streaming implementation.
    let carrier = measurer
        .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
        .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
        .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque);
    carrier.measure_html_with_inline_styles(measurer, html, style, max_width, wrap_mode)
}

impl InlineHtmlMeasurementCarrier {
    pub(crate) fn measure_html_with_inline_styles<M: TextMeasurer + ?Sized>(
        self,
        measurer: &M,
        html: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        measure_html_with_inline_styles_with_carrier(
            measurer, self, html, style, max_width, wrap_mode,
        )
    }
}

/// Consumes markup after `<`, leaving literal comparison signs in the text stream.
///
/// This only finds boundaries: the existing label logic still owns tag names, fonts, and icons.
/// An empty tag represents a skipped comment or an unfinished tag discarded at EOF.
fn take_inline_html_tag(input: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let mut lookahead = input.clone();
    let first = lookahead.next()?;
    let name_start = if first == '/' {
        lookahead.next()?
    } else {
        first
    };
    if matches!(first, '!' | '?') || (first == '/' && !name_start.is_ascii_alphabetic()) {
        let is_comment =
            first == '!' && lookahead.next() == Some('-') && lookahead.next() == Some('-');
        if is_comment {
            // Consume `!--`; quotes and tag-like text in comments have no attribute semantics.
            input.next();
            input.next();
            input.next();
            let mut at_start = true;
            let mut hyphens = 0_u8;
            let mut end_bang = false;
            for ch in input.by_ref() {
                if ch == '>' && (at_start || hyphens == 2 || end_bang) {
                    break;
                }
                end_bang = ch == '!' && hyphens == 2;
                hyphens = if ch == '-' { (hyphens + 1).min(2) } else { 0 };
                at_start = at_start && ch == '-' && hyphens == 1;
            }
        } else {
            // Bogus declarations, end tags, and processing instructions ignore attribute quotes.
            for ch in input.by_ref() {
                if ch == '>' {
                    break;
                }
            }
        }
        return Some(String::new());
    }

    if !name_start.is_ascii_alphabetic() {
        return None;
    }

    #[derive(Clone, Copy)]
    enum Boundary {
        TagName,
        BeforeAttribute,
        AttributeName,
        AfterAttribute,
        BeforeValue,
        UnquotedValue,
        QuotedValue(char),
    }
    use Boundary::*;

    let mut tag = String::new();
    if first == '/' {
        tag.push(input.next()?);
    }
    let mut boundary = TagName;
    for ch in input.by_ref() {
        if ch == '>' && !matches!(boundary, QuotedValue(_)) {
            return Some(tag);
        }
        tag.push(ch);
        boundary = match boundary {
            QuotedValue(quote) if ch == quote => BeforeAttribute,
            QuotedValue(_) => boundary,
            TagName | BeforeAttribute if is_html_collapsible_ascii_whitespace(ch) || ch == '/' => {
                BeforeAttribute
            }
            BeforeAttribute => AttributeName,
            AttributeName | AfterAttribute if ch == '=' => BeforeValue,
            AttributeName if is_html_collapsible_ascii_whitespace(ch) => AfterAttribute,
            AttributeName | AfterAttribute if ch == '/' => BeforeAttribute,
            AfterAttribute if !is_html_collapsible_ascii_whitespace(ch) => AttributeName,
            BeforeValue if is_html_collapsible_ascii_whitespace(ch) => BeforeValue,
            BeforeValue if matches!(ch, '"' | '\'') => QuotedValue(ch),
            BeforeValue => UnquotedValue,
            UnquotedValue if is_html_collapsible_ascii_whitespace(ch) => BeforeAttribute,
            _ => boundary,
        };
    }
    Some(String::new())
}

fn measure_html_with_inline_styles_with_carrier<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        carrier,
        html,
        style,
        max_width,
        wrap_mode,
        (wrap_mode == WrapMode::HtmlLike)
            .then(|| xhtml_fonts::TerminalFonts::new(style, style.font_size)),
    )
}

#[allow(clippy::too_many_arguments)]
fn measure_html_with_inline_styles_and_terminal_fonts<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
    mut terminal_fonts: Option<xhtml_fonts::TerminalFonts>,
) -> TextMetrics {
    fn html_tag_class_attr(tag: &str) -> Option<String> {
        let lower = tag.to_ascii_lowercase();
        let idx = lower.find("class=")?;
        let rest = tag[idx + 6..].trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }

        let mut it = rest.chars();
        let _ = it.next();
        let mut value = String::new();
        for ch in it {
            if ch == quote {
                break;
            }
            value.push(ch);
        }

        Some(value)
    }

    fn fontawesome_icon_width_px(tag: &str, font_size: f64) -> Option<f64> {
        let class_attr = html_tag_class_attr(tag)?;
        let mut prefix: Option<&str> = None;
        let mut icon: Option<&str> = None;

        for token in class_attr.split_ascii_whitespace() {
            if matches!(token, "fa" | "fab" | "fak" | "fal" | "far" | "fas") {
                prefix = Some(token);
                continue;
            }
            if let Some(name) = token.strip_prefix("fa-") {
                icon = Some(name);
            }
        }

        let prefix = prefix?;
        let icon = icon?;
        let advance_em = match (prefix, icon) {
            ("fa" | "fab" | "fak" | "fal" | "far" | "fas", _) => 1.25,
            _ => return None,
        };

        Some(round_to_1_64_px(font_size.max(0.0) * advance_em))
    }

    // Mermaid supports inline FontAwesome icons via `<i class="fa fa-..."></i>` inside HTML
    // labels. Upstream layout is computed with FontAwesome CSS available, while exported SVGs
    // keep only the empty `<i>` element. Model the layout-time glyph advance explicitly.
    let mut plain = String::new();
    let mut icon_width_px_by_line: Vec<f64> = vec![0.0];
    let mut icon_on_line: Vec<bool> = vec![false];
    let mut image_on_line: Vec<bool> = vec![false];
    let mut inline_runs_by_line: Vec<Vec<InlineTextRun>> = vec![Vec::new()];
    let mut break_spaces_runs_by_line: Option<Vec<Vec<InlineTextRun>>> = None;
    let mut strong_depth: usize = 0;
    let mut em_depth: usize = 0;
    let mut code_depth: usize = 0;
    let mut fa_icon_depth: usize = 0;
    let mut inline_replaced_boundary = false;

    let html = html.replace("\r\n", "\n");
    let mut it = html.chars().peekable();
    let mut entity_reference = String::new();
    while let Some(ch) = it.next() {
        if ch == '<'
            && let Some(tag) = take_inline_html_tag(&mut it)
        {
            let tag = trim_html_collapsible_ascii_whitespace(&tag);
            let tag_lower = tag.to_ascii_lowercase();
            let tag_trim = trim_html_collapsible_ascii_whitespace(&tag_lower);
            if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                continue;
            }
            let is_closing = tag_trim.starts_with('/');
            let name = tag_trim
                .trim_start_matches('/')
                .trim_end_matches('/')
                .split(is_html_collapsible_ascii_whitespace)
                .find(|part| !part.is_empty())
                .unwrap_or("");

            if let Some(fonts) = terminal_fonts.as_mut() {
                fonts.observe_tag(name, tag, is_closing);
            }

            let fontawesome_icon_width = if name == "i" && !is_closing {
                fontawesome_icon_width_px(
                    tag,
                    terminal_fonts
                        .as_ref()
                        .map_or(style.font_size, |fonts| fonts.current().font_size),
                )
            } else {
                None
            };

            match name {
                "strong" | "b" => {
                    if is_closing {
                        strong_depth = strong_depth.saturating_sub(1);
                    } else {
                        strong_depth += 1;
                    }
                }
                "em" | "i" => {
                    if is_closing {
                        if name == "i" && fa_icon_depth > 0 {
                            fa_icon_depth = fa_icon_depth.saturating_sub(1);
                        } else {
                            em_depth = em_depth.saturating_sub(1);
                        }
                    } else if let Some(icon_w) = fontawesome_icon_width {
                        let line_idx = icon_width_px_by_line.len().saturating_sub(1);
                        icon_width_px_by_line[line_idx] += icon_w;
                        if let Some(slot) = icon_on_line.get_mut(line_idx) {
                            *slot = true;
                        }
                        let mut icon =
                            terminal_fonts
                                .as_ref()
                                .map_or_else(InlineTextRun::default, |fonts| {
                                    InlineTextRun {
                                        // U+FFFC indexes an inline replaced box for Unicode line breaking.
                                        // Its fixed advance is never measured as a text glyph.
                                        text: "\u{fffc}".to_owned(),
                                        terminal_style: Some(std::sync::Arc::clone(
                                            fonts.current(),
                                        )),
                                        replaced_width_px: Some(icon_w),
                                        ..InlineTextRun::default()
                                    }
                                });
                        if let Some(fonts) = terminal_fonts.as_ref() {
                            icon.record_ancestor_strut(0, fonts.current_ancestor_strut_font_size());
                        }
                        if let Some(runs) = inline_runs_by_line.get_mut(line_idx) {
                            runs.push(icon.clone());
                        }
                        if let Some(runs) = break_spaces_runs_by_line
                            .as_mut()
                            .and_then(|lines| lines.last_mut())
                        {
                            runs.push(icon);
                        }
                        inline_replaced_boundary = true;
                        fa_icon_depth += 1;
                    } else {
                        em_depth += 1;
                    }
                }
                "code" => {
                    if is_closing {
                        code_depth = code_depth.saturating_sub(1);
                    } else {
                        code_depth += 1;
                    }
                }
                "img" if !is_closing => {
                    if terminal_fonts.is_some()
                        && let Some(runs) = inline_runs_by_line.last_mut()
                    {
                        runs.push(InlineTextRun::default());
                    }
                    if let Some(slot) = image_on_line.last_mut() {
                        *slot = true;
                    }
                    inline_replaced_boundary = true;
                }
                "br" => {
                    if let Some(fonts) = terminal_fonts.as_mut() {
                        fonts.new_line();
                    }
                    plain.push('\n');
                    icon_width_px_by_line.push(0.0);
                    icon_on_line.push(false);
                    image_on_line.push(false);
                    inline_runs_by_line.push(Vec::new());
                    if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                        lines.push(Vec::new());
                    }
                    inline_replaced_boundary = false;
                }
                "p" | "div" | "li" | "tr" | "ul" | "ol" if is_closing => {
                    // A block end starts the next line only when the current line has content.
                    // Reusing the `<br>` branch here would append a phantom row after
                    // `<p><br></p>` and make blank-only labels impossible to count exactly.
                    if !plain.ends_with('\n') && !plain.is_empty() {
                        if let Some(fonts) = terminal_fonts.as_mut() {
                            fonts.new_line();
                        }
                        plain.push('\n');
                        icon_width_px_by_line.push(0.0);
                        icon_on_line.push(false);
                        image_on_line.push(false);
                        inline_runs_by_line.push(Vec::new());
                        if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                            lines.push(Vec::new());
                        }
                    }
                    inline_replaced_boundary = false;
                }
                _ => {}
            }
            continue;
        }

        let push_char = |decoded: char,
                         plain: &mut String,
                         icon_on_line: &mut Vec<bool>,
                         inline_runs_by_line: &mut Vec<Vec<InlineTextRun>>,
                         break_spaces_runs_by_line: &mut Option<Vec<Vec<InlineTextRun>>>,
                         inline_replaced_boundary: &mut bool| {
            if wrap_mode == WrapMode::HtmlLike
                && is_html_collapsible_ascii_whitespace(decoded)
                && break_spaces_runs_by_line.is_none()
            {
                // The browser decodes entities before it decides whether fixed-width
                // `break-spaces` layout is needed. Lazily fork the exact-text projection at the
                // first decoded whitespace so literal and entity-produced whitespace share the
                // same semantics without duplicating every ordinary label up front.
                *break_spaces_runs_by_line = Some(inline_runs_by_line.clone());
            }
            if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                if decoded == '\n' || decoded == '\r' {
                    lines.push(Vec::new());
                } else if let Some(runs) = lines.last_mut() {
                    push_inline_text_char_with_terminal_style(
                        runs,
                        decoded,
                        strong_depth > 0,
                        em_depth > 0,
                        code_depth > 0,
                        terminal_fonts.as_ref().map(|fonts| fonts.current()),
                        terminal_fonts
                            .as_ref()
                            .map_or(0.0, |fonts| fonts.current_ancestor_strut_font_size()),
                    );
                }
            }
            let decoded = if is_html_collapsible_ascii_whitespace(decoded) {
                ' '
            } else {
                decoded
            };
            if decoded == ' ' {
                let line_has_visual = icon_on_line.last().copied().unwrap_or(false)
                    || image_on_line.last().copied().unwrap_or(false);
                if (plain.ends_with(' ') && !*inline_replaced_boundary)
                    || (plain.ends_with('\n') && !line_has_visual)
                    || (plain.is_empty() && !line_has_visual)
                {
                    return;
                }
            }
            plain.push(decoded);
            if let Some(runs) = inline_runs_by_line.last_mut() {
                push_inline_text_char_with_terminal_style(
                    runs,
                    decoded,
                    strong_depth > 0,
                    em_depth > 0,
                    code_depth > 0,
                    terminal_fonts.as_ref().map(|fonts| fonts.current()),
                    terminal_fonts
                        .as_ref()
                        .map_or(0.0, |fonts| fonts.current_ancestor_strut_font_size()),
                );
            }
            *inline_replaced_boundary = false;
        };

        if ch == '&' {
            entity_reference.clear();
            entity_reference.push('&');
            let mut saw_semicolon = false;
            while let Some(&c) = it.peek() {
                if c == ';' {
                    it.next();
                    saw_semicolon = true;
                    break;
                }
                if c == '<'
                    || c == '&'
                    || is_html_collapsible_ascii_whitespace(c)
                    || entity_reference.len() > 65
                {
                    break;
                }
                entity_reference.push(c);
                it.next();
            }
            if saw_semicolon {
                entity_reference.push(';');
            }
            let decoded = merman_model::text::decode_html_entities_to_unicode(&entity_reference);
            for decoded in decoded.chars() {
                push_char(
                    decoded,
                    &mut plain,
                    &mut icon_on_line,
                    &mut inline_runs_by_line,
                    &mut break_spaces_runs_by_line,
                    &mut inline_replaced_boundary,
                );
            }
            continue;
        }

        push_char(
            ch,
            &mut plain,
            &mut icon_on_line,
            &mut inline_runs_by_line,
            &mut break_spaces_runs_by_line,
            &mut inline_replaced_boundary,
        );
    }

    if let Some(lines) = break_spaces_runs_by_line.as_mut() {
        while lines.len() > 1
            && lines
                .last()
                .is_some_and(|runs| runs.iter().all(|run| run.text.is_empty()))
        {
            lines.pop();
        }
    }

    // Keep whitespace adjacent to inline icons: in HTML it becomes significant when it separates
    // text from an inline-block `<i>` (for both `<i> text` and `text <i>`). Otherwise discard only
    // collapsible ASCII boundary whitespace. U+00A0 remains visible at either boundary.
    let plain = if icon_on_line.iter().any(|v| *v) {
        plain.trim_end_matches('\n').to_string()
    } else {
        plain
            .trim_end_matches(is_html_collapsible_ascii_whitespace)
            .to_string()
    };
    let has_inline_icons = icon_on_line.iter().any(|has_icon| *has_icon);

    if let Some(fonts) = terminal_fonts {
        // The eight-style planner represents ordinary emphasis and code without retaining an
        // arbitrary font stack. Keep that fast path only when every terminal font agrees with it;
        // nested bolder, light/heavy parents, and oblique-to-italic overrides need resolved runs.
        let requires_terminal_styles = fonts.has_font_declarations()
            || inline_runs_by_line.iter().flatten().any(|run| {
                !run.text.is_empty()
                    && run.replaced_width_px.is_none()
                    && !xhtml_fonts::same_font(
                        run.terminal_style.as_deref(),
                        Some(&inline_text_style(style, run.bold, run.italic, run.code)),
                    )
            });
        if requires_terminal_styles {
            debug_assert!(wrap_mode == WrapMode::HtmlLike);
            let explicit_line_boxes =
                explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);
            let lines = normalized_preparsed_html_text_lines(&plain);
            inline_runs_by_line.resize_with(lines.len().max(explicit_line_boxes), Vec::new);
            // Keep collapsed tails out of styled widths; empty runs mark replaced elements.
            for runs in &mut inline_runs_by_line {
                for run in runs.iter_mut().rev() {
                    if run.text.is_empty() {
                        break;
                    }
                    let end = run
                        .text
                        .trim_end_matches(is_html_collapsible_ascii_whitespace)
                        .len();
                    run.text.truncate(end);
                    if end > 0 {
                        break;
                    }
                }
            }
            if !carrier.is_builtin() {
                measurer.measure_wrapped(
                    trim_html_collapsible_ascii_whitespace(&plain),
                    style,
                    max_width,
                    wrap_mode,
                );
            }
            if max_width.is_some() {
                return finish_inline_html_layout(
                    measurer,
                    carrier,
                    &inline_runs_by_line,
                    break_spaces_runs_by_line.as_deref(),
                    style,
                    max_width,
                    InlineHtmlLineBoxes {
                        explicit_count: explicit_line_boxes,
                        font_sizes: Some(fonts.line_sizes()),
                    },
                );
            }
            return fonts.measure(measurer, &inline_runs_by_line, style, explicit_line_boxes);
        }
        // Representable markup retains the existing callback and primitive-font fast path.
        for run in inline_runs_by_line.iter_mut().flatten() {
            run.terminal_style = None;
            if run.replaced_width_px.take().is_some() {
                run.text.clear();
            }
        }
        if let Some(lines) = break_spaces_runs_by_line.as_mut() {
            for run in lines.iter_mut().flatten() {
                run.terminal_style = None;
                if run.replaced_width_px.take().is_some() {
                    run.text.clear();
                }
            }
        }
    }

    if wrap_mode == WrapMode::HtmlLike && !has_inline_icons && carrier.is_builtin() {
        let explicit_line_boxes =
            explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);
        let mut lines = normalized_preparsed_html_text_lines(&plain);
        if lines.is_empty() {
            lines.push(String::new());
        }
        inline_runs_by_line.resize_with(lines.len(), Vec::new);

        // The pinned Mermaid `createText.ts:addHtmlSpan` first measures decoded HTML with
        // `display: table-cell`, `white-space: nowrap`, and `max-width`. When the browser reports
        // exactly that maximum width, it switches to table layout with `white-space: break-spaces`
        // and a fixed width before measuring again. The built-in planner preserves that
        // natural-width activation, min-content expansion, and greedy line count directly;
        // exact browser shaping and `getBoundingClientRect()` floats remain a bounded residual.
        // Consequently the discarded plain-label and per-run premeasurement passes are not part
        // of this operation and need not call the built-in backend.
        return finish_inline_html_layout(
            measurer,
            carrier,
            &inline_runs_by_line,
            break_spaces_runs_by_line.as_deref(),
            style,
            max_width,
            InlineHtmlLineBoxes {
                explicit_count: explicit_line_boxes,
                font_sizes: None,
            },
        );
    }

    // Keep this request before all styled probes for arbitrary and host measurers. Their callbacks
    // are observable, including stateful return values and the exact position of a failure.
    let base_text = trim_html_collapsible_ascii_whitespace(&plain);
    let base = measurer.measure_wrapped(base_text, style, max_width, wrap_mode);
    let explicit_line_boxes = explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);

    let mut lines = normalized_preparsed_html_text_lines(&plain);
    if lines.is_empty() {
        lines.push(String::new());
    }
    icon_width_px_by_line.resize(lines.len(), 0.0);
    icon_on_line.resize(lines.len(), false);
    inline_runs_by_line.resize_with(lines.len(), Vec::new);
    let styled_text_width_px_by_line = inline_runs_by_line
        .iter()
        .map(|runs| measure_inline_runs_width_px(measurer, runs, style, wrap_mode, false))
        .collect::<Vec<_>>();
    let inline_width_px_by_line = styled_text_width_px_by_line
        .iter()
        .zip(&icon_width_px_by_line)
        .map(|(text_width, icon_width)| text_width + icon_width)
        .collect::<Vec<_>>();

    if wrap_mode == WrapMode::HtmlLike && !has_inline_icons {
        return finish_inline_html_layout(
            measurer,
            carrier,
            &inline_runs_by_line,
            break_spaces_runs_by_line.as_deref(),
            style,
            max_width,
            InlineHtmlLineBoxes {
                explicit_count: explicit_line_boxes,
                font_sizes: None,
            },
        );
    }

    let icon_start_wrap = if wrap_mode == WrapMode::HtmlLike {
        max_width
            .filter(|w| w.is_finite() && *w > 0.0)
            .and_then(|w| {
                let mut extra_lines = 0usize;
                let mut wrapped_width: f64 = 0.0;
                let mut has_width_override = false;

                for (idx, line) in lines.iter().enumerate() {
                    if !icon_on_line[idx] || !line.starts_with(is_html_collapsible_ascii_whitespace)
                    {
                        continue;
                    }
                    let text = trim_html_collapsible_ascii_whitespace(line);
                    if text.is_empty() {
                        continue;
                    }

                    let segments = html_break_spaces_segments(text);
                    let text_width = styled_text_width_px_by_line[idx];
                    let first_segment = segments.first().copied().unwrap_or(text);
                    let first_segment_width = measurer
                        .measure_wrapped(first_segment, style, None, wrap_mode)
                        .width;
                    if first_segment_width + icon_width_px_by_line[idx] > w {
                        extra_lines += 1;
                        has_width_override = true;
                        for segment in segments {
                            if segment.is_empty() {
                                continue;
                            }
                            wrapped_width = wrapped_width.max(
                                measurer
                                    .measure_wrapped(segment, style, None, wrap_mode)
                                    .width,
                            );
                        }
                    } else if text_width <= w && inline_width_px_by_line[idx] > w {
                        extra_lines += 1;
                        has_width_override = true;
                        wrapped_width = wrapped_width.max(w);
                    } else if inline_width_px_by_line[idx] > w {
                        has_width_override = true;
                        let mut segment_width: f64 = 0.0;
                        for segment in segments {
                            if segment.is_empty() {
                                continue;
                            }
                            segment_width = segment_width.max(
                                measurer
                                    .measure_wrapped(segment, style, None, wrap_mode)
                                    .width,
                            );
                        }
                        wrapped_width = wrapped_width.max(segment_width.max(w));
                    }
                }

                (has_width_override || extra_lines > 0).then_some((wrapped_width, extra_lines))
            })
    } else {
        None
    };

    let inline_style_extra_wrap_lines = if wrap_mode == WrapMode::HtmlLike {
        max_width
            .filter(|w| w.is_finite() && *w > 0.0)
            .map(|w| {
                lines
                    .iter()
                    .enumerate()
                    .filter(|(idx, line)| {
                        let text = trim_html_collapsible_ascii_whitespace(line);
                        if text.is_empty()
                            || icon_on_line.get(*idx).copied().unwrap_or(false)
                            || !text.chars().any(is_html_collapsible_ascii_whitespace)
                        {
                            return false;
                        }
                        let raw_width =
                            measurer.measure_wrapped(text, style, None, wrap_mode).width;
                        raw_width <= w
                            && styled_text_width_px_by_line
                                .get(*idx)
                                .copied()
                                .unwrap_or(raw_width)
                                > w
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    let max_line_width = inline_width_px_by_line
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);

    // Mermaid's upstream baselines land on a 1/64px lattice. For SVG-label measurement, the
    // underlying `getBBox()` numbers can hit exact `.5/64` ties; use ties-to-even rounding to
    // match the lattice choices observed in upstream class SVG fixtures.
    let mut width = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
            wrap::round_to_1_64_px_ties_to_even(max_line_width)
        }
        WrapMode::HtmlLike => round_to_1_64_px(max_line_width),
    };
    if wrap_mode == WrapMode::HtmlLike
        && let Some(w) = max_width.filter(|w| w.is_finite() && *w > 0.0)
    {
        let raw_w = max_line_width;
        let needs_wrap = raw_w > w;
        if needs_wrap {
            // When wrapping is active, the DOM-driven width behavior is governed by the
            // wrapped layout, not the unwrapped per-line extents. Reuse the wrapped baseline
            // width (without bold deltas) so we don't over-inflate `foreignObject width="..."`
            // from unwrapped lines.
            //
            // The underlying measurer is still responsible for modeling any min-content
            // expansion beyond `max-width`.
            width = icon_start_wrap
                .map(|(icon_width, _)| icon_width)
                .unwrap_or(base.width)
                .max(w);
        } else {
            width = width.min(w);
        }
    }

    let icon_only_extra_lines = if trim_html_collapsible_ascii_whitespace(&plain).is_empty() {
        0
    } else {
        lines
            .iter()
            .enumerate()
            .filter(|(idx, line)| {
                trim_html_collapsible_ascii_whitespace(line).is_empty()
                    && icon_on_line.get(*idx).copied().unwrap_or(false)
                    && icon_width_px_by_line.get(*idx).copied().unwrap_or(0.0) > 0.0
            })
            .count()
    };

    if icon_only_extra_lines > 0 {
        // DOM measurement keeps an inline icon-only line as a normal 1.5em line box and rounds the
        // resulting max line width upward on the 1/64px lattice.
        width = ceil_to_1_64_px(width);
    }

    let (mut height, mut line_count) = if let Some((_, extra_lines)) = icon_start_wrap {
        (
            base.height + extra_lines as f64 * style.font_size.max(0.0) * 1.5,
            base.line_count + extra_lines,
        )
    } else {
        (base.height, base.line_count)
    };
    if icon_only_extra_lines > 0 {
        height += icon_only_extra_lines as f64 * style.font_size.max(0.0) * 1.5;
        line_count += icon_only_extra_lines;
    }
    if inline_style_extra_wrap_lines > 0 {
        height += inline_style_extra_wrap_lines as f64 * style.font_size.max(0.0) * 1.5;
        line_count += inline_style_extra_wrap_lines;
    }

    TextMetrics {
        width,
        height,
        line_count,
    }
}

fn markdown_word_line_plain_text_and_width_px(
    measurer: &dyn TextMeasurer,
    words: &[(String, MermaidMarkdownWordType)],
    style: &TextStyle,
    wrap_mode: WrapMode,
    entity_mode: super::SvgTextEntityMode,
) -> (String, f64) {
    if measurer.cancellation_requested() {
        return (String::new(), 0.0);
    }
    let mut plain = String::new();
    let mut runs = Vec::new();

    for (word_idx, (word, ty)) in words.iter().enumerate() {
        if measurer.cancellation_requested() {
            return (String::new(), 0.0);
        }
        let visible_word = match wrap_mode {
            WrapMode::HtmlLike => merman_model::text::decode_html_entities_to_unicode(word),
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => entity_mode.visible_word(word),
        };
        let bold = *ty == MermaidMarkdownWordType::Strong;
        let italic = *ty == MermaidMarkdownWordType::Em;

        if word_idx > 0 {
            plain.push(' ');
            // Each SVG word tspan owns its leading space and its font attributes.
            let svg = wrap_mode != WrapMode::HtmlLike;
            push_inline_text_char(&mut runs, ' ', svg && bold, svg && italic, false);
        }
        for ch in visible_word.chars() {
            plain.push(ch);
            push_inline_text_char(&mut runs, ch, bold, italic, false);
        }
    }

    let width = measure_inline_runs_width_px(measurer, &runs, style, wrap_mode, true);
    (plain, width)
}

fn measure_markdown_word_line_width_px(
    measurer: &dyn TextMeasurer,
    words: &[(String, MermaidMarkdownWordType)],
    style: &TextStyle,
    wrap_mode: WrapMode,
    entity_mode: super::SvgTextEntityMode,
) -> f64 {
    markdown_word_line_plain_text_and_width_px(measurer, words, style, wrap_mode, entity_mode).1
}

fn split_markdown_word_to_width_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    word: &str,
    ty: MermaidMarkdownWordType,
    max_width_px: f64,
    wrap_mode: WrapMode,
    entity_mode: super::SvgTextEntityMode,
) -> (String, String) {
    if measurer.cancellation_requested() {
        return (String::new(), String::new());
    }
    if max_width_px <= 0.0 {
        return (word.to_string(), String::new());
    }
    use unicode_segmentation::UnicodeSegmentation;

    let Some(first) = word.graphemes(true).next() else {
        return (String::new(), String::new());
    };

    // Mermaid's splitTextToChars uses Intl.Segmenter. An over-wide first grapheme must
    // still make progress without separating its combining marks or emoji joiners.
    let mut split_at = first.len();
    for (index, grapheme) in word.grapheme_indices(true) {
        if measurer.cancellation_requested() {
            return (String::new(), String::new());
        }
        let end = index + grapheme.len();
        let head = word[..end].to_string();
        let width = measure_markdown_word_line_width_px(
            measurer,
            &[(head, ty)],
            style,
            wrap_mode,
            entity_mode,
        );
        if measurer.cancellation_requested() {
            return (String::new(), String::new());
        }
        if width.is_finite() && width <= max_width_px {
            split_at = end;
        } else {
            break;
        }
    }

    if measurer.cancellation_requested() {
        return (String::new(), String::new());
    }
    let (head, tail) = word.split_at(split_at);
    (head.to_string(), tail.to_string())
}

pub(crate) fn wrap_markdown_word_lines(
    measurer: &dyn TextMeasurer,
    parsed: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
    break_long_words: bool,
    entity_mode: super::SvgTextEntityMode,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    // The next fallible family boundary replays cancellation; do not retain partial rows.
    if measurer.cancellation_requested() {
        return Vec::new();
    }
    let Some(max_width_px) = max_width_px.filter(|w| w.is_finite() && *w > 0.0) else {
        return parsed.to_vec();
    };

    let mut out: Vec<Vec<(String, MermaidMarkdownWordType)>> = Vec::new();
    for line in parsed {
        if measurer.cancellation_requested() {
            return Vec::new();
        }
        if line.is_empty() {
            out.push(Vec::new());
            continue;
        }

        // Only split tails need owned storage; unvisited source words remain borrowed.
        let mut tokens = line
            .iter()
            .map(|(word, ty)| (std::borrow::Cow::Borrowed(word.as_str()), *ty));
        let mut pending = None;
        let mut cur: Vec<(String, MermaidMarkdownWordType)> = Vec::new();

        while let Some((word, ty)) = pending.take().or_else(|| tokens.next()) {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            let mut candidate = cur.clone();
            candidate.push((word.to_string(), ty));
            let candidate_width = measure_markdown_word_line_width_px(
                measurer,
                &candidate,
                style,
                wrap_mode,
                entity_mode,
            );
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            if candidate_width <= max_width_px {
                cur = candidate;
                continue;
            }

            if !cur.is_empty() {
                out.push(cur);
                cur = Vec::new();
                pending = Some((word, ty));
                continue;
            }

            let single_word_width = measure_markdown_word_line_width_px(
                measurer,
                &[(word.to_string(), ty)],
                style,
                wrap_mode,
                entity_mode,
            );
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            if single_word_width <= max_width_px || !break_long_words {
                out.push(vec![(word.into_owned(), ty)]);
                continue;
            }

            let (head, tail) = split_markdown_word_to_width_px(
                measurer,
                style,
                word.as_ref(),
                ty,
                max_width_px,
                wrap_mode,
                entity_mode,
            );
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            out.push(vec![(head, ty)]);
            if !tail.is_empty() {
                pending = Some((std::borrow::Cow::Owned(tail), ty));
            }
        }

        if !cur.is_empty() {
            out.push(cur);
        }
    }

    if out.is_empty() {
        vec![Vec::new()]
    } else {
        out
    }
}

pub(crate) fn mermaid_markdown_to_wrapped_word_lines(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    let parsed = mermaid_markdown_to_lines(markdown, true);
    wrap_markdown_word_lines(
        measurer,
        &parsed,
        style,
        max_width_px,
        wrap_mode,
        true,
        super::SvgTextEntityMode::CreateTextSource,
    )
}

/// Measures the SVG word rows emitted by `createText`, without HTML-only image sizing.
pub(crate) fn measure_svg_markdown_word_lines(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    entity_mode: super::SvgTextEntityMode,
) -> TextMetrics {
    measure_svg_markdown_word_lines_with_origin(measurer, lines, style, entity_mode).0
}

/// Retains the origin found during the same word measurements, without replaying host requests.
pub(crate) fn measure_svg_markdown_word_lines_with_origin(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    entity_mode: super::SvgTextEntityMode,
) -> (TextMetrics, f64) {
    if lines.iter().all(Vec::is_empty) {
        return (
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            },
            0.0,
        );
    }
    let mut width = 0.0_f64;
    let mut ink: Option<(f64, f64)> = None;
    for (row_index, line) in lines.iter().enumerate() {
        let (_, line_width) = markdown_word_line_plain_text_and_width_px(
            measurer,
            line,
            style,
            WrapMode::SvgLike,
            entity_mode,
        );
        width = width.max(line_width);
        for (word_index, (word, kind)) in line.iter().enumerate() {
            let visible = entity_mode.visible_word(word);
            if trim_html_collapsible_ascii_whitespace(&visible).is_empty() {
                continue;
            }
            let text = if word_index == 0 {
                visible
            } else {
                std::borrow::Cow::Owned(format!(" {visible}"))
            };
            let face = svg_markdown_word_style(style, *kind);
            // Literal word content cannot introduce rows; only retained row indices do that.
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(&text, &face)
                + row_index as f64 * style.font_size * 1.1;
            let height = measurer.measure_svg_tspan_text_bbox_height_px(&text, &face);
            if height > 0.0 {
                let bottom = top + height;
                ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
            }
        }
    }
    (
        TextMetrics {
            width: round_to_1_64_px(width),
            height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
            line_count: lines.len(),
        },
        ink.map_or(0.0, |(top, _)| top),
    )
}

fn html_markdown_paragraph_gap_lines(markdown: &str) -> usize {
    if !markdown.contains("\n\n") && !markdown.contains("\r\n\r\n") {
        return 0;
    }

    let markdown = markdown
        .strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(markdown)
        .replace("\r\n", "\n");
    let parser = pulldown_cmark::Parser::new_ext(
        &markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    );
    let paragraph_count = parser
        .filter(|ev| {
            matches!(
                ev,
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph)
            )
        })
        .count();

    paragraph_count.saturating_sub(1)
}

fn measure_markdown_with_inline_styles_impl(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
    manually_wrap_words: bool,
) -> TextMetrics {
    // Mermaid's flowchart HTML labels support inline Markdown images. These affect layout even
    // when the label has no textual content (e.g. `![](...)`).
    //
    // We keep the existing text-focused Markdown measurement for the common case, and only
    // special-case when we observe at least one image token.
    if wrap_mode == WrapMode::HtmlLike && markdown.contains("![") {
        #[derive(Debug, Default, Clone)]
        struct Paragraph {
            text: String,
            image_urls: Vec<String>,
        }

        fn measure_markdown_images(
            measurer: &dyn TextMeasurer,
            markdown: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            let parser = pulldown_cmark::Parser::new_ext(
                markdown,
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS,
            );

            let mut paragraphs: Vec<Paragraph> = Vec::new();
            let mut current = Paragraph::default();
            let mut in_paragraph = false;

            for ev in parser {
                match ev {
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {
                        if in_paragraph {
                            paragraphs.push(std::mem::take(&mut current));
                        }
                        in_paragraph = true;
                    }
                    pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                        if in_paragraph {
                            paragraphs.push(std::mem::take(&mut current));
                        }
                        in_paragraph = false;
                    }
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image {
                        dest_url, ..
                    }) => {
                        current.image_urls.push(dest_url.to_string());
                    }
                    pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                        current.text.push_str(&t);
                    }
                    pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                        current.text.push('\n');
                    }
                    _ => {}
                }
            }
            if in_paragraph {
                paragraphs.push(current);
            }

            let total_images: usize = paragraphs.iter().map(|p| p.image_urls.len()).sum();
            if total_images == 0 {
                return None;
            }

            let total_text = paragraphs
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let has_any_text = !trim_html_collapsible_ascii_whitespace(&total_text).is_empty();

            // Mermaid renders a single standalone Markdown image without a `<p>` wrapper and
            // applies fixed `80px` sizing. In the upstream fixtures, missing/empty `src` yields
            // `height="0"` while keeping the width.
            if total_images == 1 && !has_any_text {
                let url = paragraphs
                    .iter()
                    .flat_map(|p| p.image_urls.iter())
                    .next()
                    .cloned()
                    .unwrap_or_default();
                let img_w = 80.0;
                let has_src = !url.trim().is_empty();
                let img_h = if has_src { img_w } else { 0.0 };
                return Some(TextMetrics {
                    width: ceil_to_1_64_px(img_w),
                    height: ceil_to_1_64_px(img_h),
                    line_count: if img_h > 0.0 { 1 } else { 0 },
                });
            }

            let max_w = max_width.unwrap_or(200.0).max(1.0);
            let line_height = style.font_size.max(0.0) * 1.5;

            let mut width: f64 = 0.0;
            let mut height: f64 = 0.0;
            let mut line_count: usize = 0;

            for p in paragraphs {
                let p_text = trim_html_collapsible_ascii_whitespace(&p.text).to_string();
                let text_metrics = if p_text.is_empty() {
                    TextMetrics {
                        width: 0.0,
                        height: 0.0,
                        line_count: 0,
                    }
                } else {
                    measurer.measure_wrapped(&p_text, style, Some(max_w), wrap_mode)
                };

                if !p.image_urls.is_empty() {
                    // Markdown images inside paragraphs use `width: 100%` in Mermaid's HTML label
                    // output, so they expand to the available width.
                    width = width.max(max_w);
                    if text_metrics.line_count == 0 {
                        // Image-only paragraphs include an extra line box from the `<p>` element.
                        height += line_height;
                        line_count += 1;
                    }
                    for url in p.image_urls {
                        let has_src = !url.trim().is_empty();
                        let img_h = if has_src { max_w } else { 0.0 };
                        height += img_h;
                        if img_h > 0.0 {
                            line_count += 1;
                        }
                    }
                }

                width = width.max(text_metrics.width);
                height += text_metrics.height;
                line_count += text_metrics.line_count;
            }

            Some(TextMetrics {
                width: ceil_to_1_64_px(width),
                height: ceil_to_1_64_px(height),
                line_count,
            })
        }

        if let Some(m) = measure_markdown_images(measurer, markdown, style, max_width, wrap_mode) {
            return m;
        }
    }

    let raw_parsed = if wrap_mode == WrapMode::HtmlLike {
        super::markdown::html_markdown_measurement_lines(markdown)
    } else {
        mermaid_markdown_to_lines(markdown, true)
    };
    let html_paragraph_gap_lines = if wrap_mode == WrapMode::HtmlLike {
        html_markdown_paragraph_gap_lines(markdown)
    } else {
        0
    };
    let parsed = if manually_wrap_words || wrap_mode != WrapMode::HtmlLike {
        wrap_markdown_word_lines(
            measurer,
            &raw_parsed,
            style,
            max_width,
            wrap_mode,
            true,
            super::SvgTextEntityMode::CreateTextSource,
        )
    } else {
        raw_parsed.clone()
    };

    if wrap_mode != WrapMode::HtmlLike {
        // The SVG writer consumes these retained rows through textContent assignments.
        // A decoded literal must never re-enter the generic label parser for its height.
        return measure_svg_markdown_word_lines(
            measurer,
            &parsed,
            style,
            super::SvgTextEntityMode::CreateTextSource,
        );
    }

    let mut plain_lines: Vec<String> = Vec::with_capacity(parsed.len().max(1));
    let mut styled_width_px_by_line: Vec<f64> = Vec::with_capacity(parsed.len().max(1));
    for words in &parsed {
        let (plain, width) = markdown_word_line_plain_text_and_width_px(
            measurer,
            words,
            style,
            wrap_mode,
            super::SvgTextEntityMode::CreateTextSource,
        );
        plain_lines.push(plain);
        styled_width_px_by_line.push(width);
    }

    let plain = trim_html_collapsible_ascii_whitespace(&plain_lines.join("\n")).to_string();
    let base = if manually_wrap_words {
        measurer.measure_wrapped(&plain, style, None, wrap_mode)
    } else {
        measurer.measure_wrapped(&plain, style, max_width, wrap_mode)
    };

    let max_line_width = styled_width_px_by_line
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);

    // DOM-measured HTML label widths snap to a 1/64px lattice.
    let mut width = round_to_1_64_px(max_line_width);
    if let Some(w) = max_width.filter(|w| w.is_finite() && *w > 0.0) {
        let raw_w = raw_parsed
            .iter()
            .map(|words| {
                markdown_word_line_plain_text_and_width_px(
                    measurer,
                    words,
                    style,
                    wrap_mode,
                    super::SvgTextEntityMode::CreateTextSource,
                )
                .1
            })
            .fold(0.0_f64, f64::max);
        let needs_wrap = raw_w > w;
        if needs_wrap {
            if manually_wrap_words {
                width = width.max(w);
            } else {
                width = base.width.max(w);
            }
        } else {
            width = width.min(w);
        }
    }

    TextMetrics {
        width,
        height: base.height + html_paragraph_gap_lines as f64 * style.font_size.max(0.0) * 1.5,
        line_count: base.line_count + html_paragraph_gap_lines,
    }
}

pub fn measure_markdown_with_inline_styles(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_markdown_with_inline_styles_impl(measurer, markdown, style, max_width, wrap_mode, false)
}

pub(crate) fn measure_wrapped_markdown_with_inline_styles(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_markdown_with_inline_styles_impl(measurer, markdown, style, max_width, wrap_mode, true)
}
