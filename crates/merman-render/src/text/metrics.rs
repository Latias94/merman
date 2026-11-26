//! Flowchart-aware text metrics and Markdown measurement helpers.

use super::html_wrap::html_line_text_with_break;
use super::line_break::{
    HtmlBreakKind, html_break_spaces_segments,
    html_break_spaces_segments_with_metadata_and_checkpoints,
};
#[cfg(feature = "markdown-labels")]
use super::mermaid_markdown_to_lines;
use super::{
    MermaidMarkdownWordType, TextMeasurer, TextMetrics, TextStyle, WrapMode, ceil_to_1_64_px,
    is_html_collapsible_ascii_whitespace, mermaid_xhtml_label_plain_text, round_to_1_64_px,
    trim_html_collapsible_ascii_whitespace, wrap,
};
use crate::environment::{
    BuiltinHtmlLineWidth, BuiltinTextMeasurementOperationCarrier, InlineHtmlMeasurementCarrier,
    TextMeasurementOperation,
};

mod xhtml_fonts;

#[derive(Clone, Copy)]
enum InlineHtmlWrap {
    Fit(Option<f64>),
    BreakSpaces(f64),
}

impl InlineHtmlWrap {
    fn max_width(self) -> Option<f64> {
        match self {
            Self::Fit(width) => width,
            Self::BreakSpaces(width) => Some(width),
        }
    }
}

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
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        inline_html_measurement_carrier(measurer),
        fragment,
        style,
        InlineHtmlWrap::Fit(max_width),
        WrapMode::HtmlLike,
        Some(xhtml_fonts::TerminalFonts::new(style, root_font_size)),
    )
}

pub(crate) fn measure_html_with_authored_width(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    wrap_width: Option<f64>,
) -> TextMetrics {
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        inline_html_measurement_carrier(measurer),
        fragment,
        style,
        wrap_width.map_or(InlineHtmlWrap::Fit(None), InlineHtmlWrap::BreakSpaces),
        WrapMode::HtmlLike,
        Some(xhtml_fonts::TerminalFonts::new(style, style.font_size)),
    )
}

fn inline_html_measurement_carrier(measurer: &dyn TextMeasurer) -> InlineHtmlMeasurementCarrier {
    measurer
        .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
        .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
        .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque)
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
    wrapping: InlineHtmlWrap,
    line_boxes: InlineHtmlLineBoxes<'_>,
) -> TextMetrics {
    let max_width = wrapping.max_width();
    let measure = |runs| {
        measure_inline_html_layout(
            measurer,
            carrier,
            runs,
            style,
            max_width,
            line_boxes.font_sizes,
        )
    };
    let active_max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
    let (layout, break_spaces_active) = if matches!(wrapping, InlineHtmlWrap::BreakSpaces(_)) {
        (
            measure(break_spaces_runs_by_line.unwrap_or(runs_by_line)),
            true,
        )
    } else {
        let collapsed = measure(runs_by_line);
        let break_spaces_active = active_max_width.is_some_and(|max_width| {
            collapsed.natural_width >= max_width && collapsed.min_content_width <= max_width
        });
        let layout = if break_spaces_active && let Some(runs) = break_spaces_runs_by_line {
            measure(runs)
        } else {
            collapsed
        };
        (layout, break_spaces_active)
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

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_label_xhtml_for_typography_admission(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
    control: &merman_model::OperationControl,
) -> Result<String, merman_model::OperationCancelled> {
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
    let sanitize_config = crate::graph_label::flowchart_label_sanitize_config(config);
    match merman_markup::sanitize::sanitize_xhtml_with_sink(&html, sanitize_config.as_ref(), &sink)
    {
        Ok(html) => Ok(html),
        Err(merman_markup::sanitize::SanitizeFailure::Output(
            crate::html::HtmlOutputError::Operation(
                crate::resources::OperationWorkError::Cancelled(error),
            ),
        )) => Err(error),
        Err(_) => Ok(String::new()),
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

    let html = flowchart_label_xhtml_for_typography_admission(label, label_type, config, control)?;
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
    inline_html_measurement_carrier(measurer)
        .measure_html_with_inline_styles(measurer, html, style, max_width, wrap_mode)
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
        InlineHtmlWrap::Fit(max_width),
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
    wrapping: InlineHtmlWrap,
    wrap_mode: WrapMode,
    mut terminal_fonts: Option<xhtml_fonts::TerminalFonts>,
) -> TextMetrics {
    let max_width = wrapping.max_width();
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
                    wrapping,
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
            wrapping,
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
            wrapping,
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

#[cfg(feature = "markdown-labels")]
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

#[cfg(feature = "markdown-labels")]
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

#[cfg(feature = "markdown-labels")]
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

/// Measures a Mermaid Markdown label with the supplied text measurer and wrap mode.
///
/// Available with the `markdown-labels` feature.
#[cfg(feature = "markdown-labels")]
pub fn measure_markdown_with_inline_styles(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_markdown_with_inline_styles_impl(measurer, markdown, style, max_width, wrap_mode, false)
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn measure_wrapped_markdown_with_inline_styles(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_markdown_with_inline_styles_impl(measurer, markdown, style, max_width, wrap_mode, true)
}

#[cfg(test)]
mod inline_planning_tests;
