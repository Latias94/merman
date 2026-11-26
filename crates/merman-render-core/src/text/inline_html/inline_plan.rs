//! Numeric inline HTML planning over already collected runs.

use crate::text::html_wrap::html_line_text_with_break;
use crate::text::line_break::{
    HtmlBreakKind, html_break_spaces_segments_with_metadata_and_checkpoints,
};
use crate::text::measurement::{BuiltinHtmlLineWidth, InlineHtmlMeasurementCarrier};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode, round_to_1_64_px};

pub(super) fn same_font(left: Option<&TextStyle>, right: Option<&TextStyle>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            std::ptr::eq(left, right)
                || (left.font_size == right.font_size
                    && left.font_family == right.font_family
                    && left.font_weight == right.font_weight
                    && left.font_style == right.font_style)
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
pub(super) enum InlineHtmlWrap {
    Fit(Option<f64>),
    BreakSpaces(f64),
}

impl InlineHtmlWrap {
    pub(super) fn max_width(self) -> Option<f64> {
        match self {
            Self::Fit(width) => width,
            Self::BreakSpaces(width) => Some(width),
        }
    }
}

#[doc(hidden)]
pub fn style_requests_bold_font_weight(style: &TextStyle) -> bool {
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
pub(super) struct InlineTextRun {
    pub(super) text: String,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) code: bool,
    pub(super) terminal_style: Option<std::sync::Arc<TextStyle>>,
    pub(super) replaced_width_px: Option<f64>,
    pub(super) ancestor_struts: Vec<InlineAncestorStrut>,
}

#[derive(Debug, Clone)]
pub(super) struct InlineAncestorStrut {
    start: usize,
    end: usize,
    font_size: f64,
}

impl InlineTextRun {
    pub(super) fn record_ancestor_strut(&mut self, start: usize, font_size: f64) {
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

    pub(super) fn ancestor_strut_height_px(&self) -> f64 {
        // Truncated collapsible tails retain no line-box contribution. Keep ranges separate
        // from font runs so ancestry changes cannot split shaping or observable host requests.
        self.ancestor_struts
            .iter()
            .take_while(|strut| strut.start < self.text.len())
            .fold(0.0_f64, |size, strut| size.max(strut.font_size))
            * 1.5
    }
}

pub(super) fn inline_text_style(
    base: &TextStyle,
    bold: bool,
    italic: bool,
    code: bool,
) -> TextStyle {
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

pub(super) fn measure_inline_run_width_px<M: TextMeasurer + ?Sized>(
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
            (Some(left), Some(right)) => same_font(Some(left), Some(right)),
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

pub(super) fn explicit_inline_html_line_boxes(
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

pub(super) struct InlineHtmlLineBoxes<'a> {
    pub(super) explicit_count: usize,
    pub(super) font_sizes: Option<&'a [f64]>,
}

pub(super) fn finish_inline_html_layout<M: TextMeasurer + ?Sized>(
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

#[cfg(test)]
#[path = "inline_planning_tests.rs"]
mod inline_planning_tests;
