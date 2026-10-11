//! Deterministic text measurement and wrapping fallback.

use super::heuristic::append_text_width_em;
use super::html_wrap::{HtmlWhiteSpace, NormalizedHtmlText, plan_html_lines};
use super::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, trim_end_html_collapsible_ascii_whitespace,
    trim_html_collapsible_ascii_whitespace,
};
use std::convert::Infallible;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

type TextWidthPxFn = Arc<dyn Fn(&str, &TextStyle) -> f64 + Send + Sync>;

#[derive(Clone)]
struct WidthCallbackTextMeasurer {
    width_px: TextWidthPxFn,
    line_height_factor: f64,
}

#[derive(Clone, Copy, Default)]
struct LineWidthAccumulator {
    heuristic_em: f64,
    char_count: usize,
}

#[derive(Clone, Copy)]
enum LineWidthSource<'a> {
    Heuristic,
    UniformAdvanceEm(f64),
    Callback(&'a TextWidthPxFn),
}

struct LongWordSplitResult<'a> {
    tail: Option<(&'a str, LineWidthAccumulator)>,
    #[cfg(test)]
    stats: LongWordSplitStats,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct LongWordSplitStats {
    grapheme_visits: usize,
    partitioned_bytes: usize,
}

#[derive(Clone, Copy)]
struct LineWidthModel<'a> {
    font_size: f64,
    source: LineWidthSource<'a>,
    style: &'a TextStyle,
    html_text: bool,
}

impl LineWidthModel<'_> {
    fn width_px(self, text: &str) -> f64 {
        if let LineWidthSource::Callback(width_px) = self.source {
            return width_px(text, self.style);
        }

        let mut width = LineWidthAccumulator::default();
        self.append_builtin(&mut width, text);
        self.finish_builtin(width)
    }

    fn candidate_width_px(
        self,
        width: LineWidthAccumulator,
        candidate: &str,
        appended: &str,
    ) -> (LineWidthAccumulator, f64) {
        if let LineWidthSource::Callback(width_px) = self.source {
            return (width, width_px(candidate, self.style));
        }

        let mut width = width;
        self.append_builtin(&mut width, appended);
        (width, self.finish_builtin(width))
    }

    fn append_builtin(self, width: &mut LineWidthAccumulator, text: &str) {
        match self.source {
            LineWidthSource::Heuristic => append_text_width_em(&mut width.heuristic_em, text),
            LineWidthSource::UniformAdvanceEm(_) => {
                width.char_count = width.char_count.saturating_add(
                    text.chars()
                        .filter(|ch| !self.html_text || *ch != '\u{00ad}')
                        .count(),
                );
            }
            LineWidthSource::Callback(_) => unreachable!("callback widths use complete strings"),
        }
    }

    fn finish_builtin(self, width: LineWidthAccumulator) -> f64 {
        match self.source {
            LineWidthSource::Heuristic => width.heuristic_em * self.font_size,
            LineWidthSource::UniformAdvanceEm(advance_em) => {
                width.char_count as f64 * self.font_size * advance_em
            }
            LineWidthSource::Callback(_) => unreachable!("callback widths use complete strings"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeterministicTextMeasurer {
    pub char_width_factor: f64,
    pub line_height_factor: f64,
}

impl DeterministicTextMeasurer {
    /// Uses caller-supplied widths with Merman's wrapping behavior.
    ///
    /// Uses `width_px` to measure complete candidate strings while Merman retains its wrapping and
    /// line-height behavior. Vertical metrics and baseline offsets remain deterministic
    /// approximations; the callback does not provide glyph bounds or font ascent/descent.
    /// HTML candidates retain unselected U+00AD controls for shaping. The callback must measure
    /// their zero glyph advance without deleting the input controls before shaping.
    ///
    /// The callback must return a finite, non-negative width in CSS pixels. Use
    /// [`crate::environment::HostTextMeasurer`] when measurement can fail or vary by operation.
    pub fn with_width_callback(
        self,
        width_px: impl Fn(&str, &TextStyle) -> f64 + Send + Sync + 'static,
    ) -> Arc<dyn TextMeasurer + Send + Sync> {
        Arc::new(WidthCallbackTextMeasurer {
            width_px: Arc::new(width_px),
            line_height_factor: self.line_height_factor,
        })
    }

    fn collapse_svg_text_whitespace(text: &str) -> String {
        let mut collapsed = String::with_capacity(text.len());
        let mut pending_space = false;

        for ch in text.chars() {
            if matches!(ch, ' ' | '\t' | '\r' | '\n') {
                pending_space = !collapsed.is_empty();
            } else {
                if pending_space {
                    collapsed.push(' ');
                    pending_space = false;
                }
                collapsed.push(ch);
            }
        }

        collapsed
    }

    fn replace_br_variants(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            let Some(rest) = text.get(i..) else {
                break;
            };

            // Mirror Mermaid's `lineBreakRegex = /<br\\s*\\/?>/gi` behavior:
            // - allow ASCII whitespace between `br` and the optional `/` or `>`
            // - do NOT accept extra characters (e.g. `<br \\t/>` should *not* count as a break)
            if rest.starts_with('<') {
                let bytes = text.as_bytes();
                if i + 3 < bytes.len()
                    && matches!(bytes[i + 1], b'b' | b'B')
                    && matches!(bytes[i + 2], b'r' | b'R')
                {
                    let mut j = i + 3;
                    while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'/' {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'>' {
                        out.push('\n');
                        i = j + 1;
                        continue;
                    }
                }
            }

            let Some(ch) = rest.chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    fn normalized_text_lines_with(
        text: &str,
        trailing_line_is_empty: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let t = Self::replace_br_variants(text);
        let mut out = t.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();

        // Mermaid often produces labels with a trailing newline (e.g. YAML `|` block scalars from
        // FlowDB). The rendered label does not keep an extra blank line at the end, so we trim
        // trailing empty lines to keep height parity.
        while out.len() > 1 && out.last().is_some_and(|s| trailing_line_is_empty(s)) {
            out.pop();
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    pub fn normalized_text_lines(text: &str) -> Vec<String> {
        Self::normalized_text_lines_with(text, |line| {
            trim_html_collapsible_ascii_whitespace(line).is_empty()
        })
    }

    pub(crate) fn normalized_text_lines_for_wrap_mode(
        text: &str,
        _wrap_mode: WrapMode,
    ) -> Vec<String> {
        Self::normalized_text_lines(text)
    }

    pub(crate) fn split_line_to_words(text: &str) -> Vec<String> {
        // Mirrors Mermaid's `splitLineToWords` fallback behavior when `Intl.Segmenter` is absent:
        // split by spaces, then re-add the spaces as separate tokens (preserving multiple spaces).
        let parts = text.split(' ').collect::<Vec<_>>();
        let mut out: Vec<String> = Vec::new();
        for part in parts {
            if !part.is_empty() {
                out.push(part.to_string());
            }
            out.push(" ".to_string());
        }
        while out.last().is_some_and(|s| s == " ") {
            out.pop();
        }
        out
    }

    fn wrapped_line_has_visible_content(text: &str) -> bool {
        !trim_html_collapsible_ascii_whitespace(text).is_empty()
    }

    fn split_long_token_to_lines<'a>(
        token: &'a str,
        max_width_px: f64,
        width_model: LineWidthModel<'_>,
        out: &mut Vec<String>,
    ) -> LongWordSplitResult<'a> {
        #[cfg(test)]
        let mut stats = LongWordSplitStats::default();
        let mut line_start = 0usize;
        let mut width = LineWidthAccumulator::default();
        let mut width_px = 0.0;
        let mut has_positive_advance = false;

        for (index, grapheme) in token.grapheme_indices(true) {
            #[cfg(test)]
            {
                stats.grapheme_visits = stats.grapheme_visits.saturating_add(1);
            }
            let candidate_end = index + grapheme.len();
            let (candidate, candidate_width_px) =
                width_model.candidate_width_px(width, &token[line_start..candidate_end], grapheme);
            if has_positive_advance
                && candidate_width_px > width_px
                && candidate_width_px > max_width_px
            {
                let completed = &token[line_start..index];
                #[cfg(test)]
                {
                    stats.partitioned_bytes =
                        stats.partitioned_bytes.saturating_add(completed.len());
                }
                out.push(completed.to_string());
                line_start = index;
                width = LineWidthAccumulator::default();
                match width_model.source {
                    LineWidthSource::Callback(width_callback) => {
                        width_px = width_callback(grapheme, width_model.style);
                    }
                    LineWidthSource::Heuristic | LineWidthSource::UniformAdvanceEm(_) => {
                        width_model.append_builtin(&mut width, grapheme);
                        width_px = width_model.finish_builtin(width);
                    }
                }
                has_positive_advance = true;
                continue;
            }

            width = candidate;
            width_px = candidate_width_px;
            has_positive_advance |= candidate_width_px > 0.0;
        }

        let tail = &token[line_start..];
        if tail.is_empty() {
            return LongWordSplitResult {
                tail: None,
                #[cfg(test)]
                stats,
            };
        }
        let tail = if width_model.width_px(tail) > max_width_px {
            // A visible grapheme wider than the whole line still makes progress. Leading and
            // trailing zero-width graphemes stay attached instead of becoming orphan lines.
            #[cfg(test)]
            {
                stats.partitioned_bytes = stats.partitioned_bytes.saturating_add(tail.len());
            }
            out.push(tail.to_string());
            None
        } else {
            #[cfg(test)]
            {
                stats.partitioned_bytes = stats.partitioned_bytes.saturating_add(tail.len());
            }
            Some((tail, width))
        };

        LongWordSplitResult {
            tail,
            #[cfg(test)]
            stats,
        }
    }

    fn wrap_svg_line(
        line: &str,
        max_width_px: f64,
        width_model: LineWidthModel<'_>,
    ) -> Vec<String> {
        if !max_width_px.is_finite() || max_width_px <= 0.0 {
            return vec![line.to_string()];
        }

        let tokens = Self::split_line_to_words(line);
        let mut tokens = std::collections::VecDeque::from(tokens);
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut cur_width = LineWidthAccumulator::default();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let previous_len = cur.len();
            cur.push_str(&tok);
            let (candidate_width, candidate_width_px) =
                width_model.candidate_width_px(cur_width, &cur, &tok);
            if candidate_width_px <= max_width_px {
                cur_width = candidate_width;
                continue;
            }
            cur.truncate(previous_len);

            if Self::wrapped_line_has_visible_content(&cur) {
                let line = trim_end_html_collapsible_ascii_whitespace(&cur).to_string();
                cur.clear();
                out.push(line);
                cur_width = LineWidthAccumulator::default();
                tokens.push_front(tok);
                continue;
            }

            // `tok` itself does not fit on an empty line.
            if tok == " " {
                continue;
            }
            // Walk the token once, keeping its final fitting tail for a following token.
            if let Some((tail, tail_width)) =
                Self::split_long_token_to_lines(&tok, max_width_px, width_model, &mut out).tail
            {
                cur.clear();
                cur.push_str(tail);
                cur_width = tail_width;
            }
        }

        if Self::wrapped_line_has_visible_content(&cur) {
            out.push(trim_end_html_collapsible_ascii_whitespace(&cur).to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }
}

impl TextMeasurer for DeterministicTextMeasurer {
    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        let source = if self.char_width_factor == 0.0 {
            LineWidthSource::Heuristic
        } else {
            LineWidthSource::UniformAdvanceEm(self.char_width_factor)
        };
        let model = LineWidthModel {
            font_size: style.font_size.max(1.0),
            source,
            style,
            html_text: true,
        };
        model.width_px(text).max(0.0)
    }

    #[allow(private_interfaces)]
    fn begin_html_line_width(
        &self,
        style: &TextStyle,
    ) -> Option<crate::environment::BuiltinHtmlLineWidth> {
        Some(crate::environment::BuiltinHtmlLineWidth::deterministic(
            style,
            self.char_width_factor,
        ))
    }

    #[allow(private_interfaces)]
    fn begin_svg_text_computed_length(
        &self,
        style: &TextStyle,
    ) -> Option<crate::environment::BuiltinSvgComputedLength> {
        (self.char_width_factor == 0.0)
            .then(|| crate::environment::BuiltinSvgComputedLength::deterministic(style))
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::SvgLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.measure_wrapped_impl(text, style, max_width, wrap_mode)
            .0
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        self.measure_wrapped_impl(text, style, max_width, wrap_mode)
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        let collapsed = Self::collapse_svg_text_whitespace(text);
        TextMetrics {
            width: self.measure_svg_simple_text_bbox_width_for_wrap_px(&collapsed, style),
            height: self.measure_svg_simple_text_bbox_height_px(&collapsed, style),
            line_count: 1,
        }
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        let t = trim_end_html_collapsible_ascii_whitespace(text);
        if t.is_empty() {
            return 0.0;
        }
        (style.font_size.max(1.0) * 1.1).max(0.0)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        if trim_end_html_collapsible_ascii_whitespace(text).is_empty() {
            0.0
        } else {
            super::svg_wrapped_first_line_bbox_height_px(style)
        }
    }
}

impl TextMeasurer for WidthCallbackTextMeasurer {
    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        (self.width_px)(text, style)
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::SvgLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        DeterministicTextMeasurer::measure_wrapped_impl_with_width(
            text,
            style,
            max_width,
            wrap_mode,
            LineWidthSource::Callback(&self.width_px),
            self.line_height_factor,
        )
        .0
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        DeterministicTextMeasurer::measure_wrapped_impl_with_width(
            text,
            style,
            max_width,
            wrap_mode,
            LineWidthSource::Callback(&self.width_px),
            self.line_height_factor,
        )
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        let collapsed = DeterministicTextMeasurer::collapse_svg_text_whitespace(text);
        TextMetrics {
            width: self.measure_svg_simple_text_bbox_width_for_wrap_px(&collapsed, style),
            height: self.measure_svg_simple_text_bbox_height_px(&collapsed, style),
            line_count: 1,
        }
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        let text = trim_end_html_collapsible_ascii_whitespace(text);
        if text.is_empty() {
            return 0.0;
        }
        (style.font_size.max(1.0) * 1.1).max(0.0)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        if trim_end_html_collapsible_ascii_whitespace(text).is_empty() {
            0.0
        } else {
            super::svg_wrapped_first_line_bbox_height_px(style)
        }
    }
}

impl DeterministicTextMeasurer {
    fn measure_wrapped_impl(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        let width_source = if self.char_width_factor == 0.0 {
            LineWidthSource::Heuristic
        } else {
            LineWidthSource::UniformAdvanceEm(self.char_width_factor)
        };
        Self::measure_wrapped_impl_with_width(
            text,
            style,
            max_width,
            wrap_mode,
            width_source,
            self.line_height_factor,
        )
    }

    fn measure_wrapped_impl_with_width(
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
        width_source: LineWidthSource<'_>,
        configured_line_height_factor: f64,
    ) -> (TextMetrics, Option<f64>) {
        let default_line_height_factor = match wrap_mode {
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
            WrapMode::HtmlLike => 1.5,
        };
        let line_height_factor = if configured_line_height_factor == 0.0 {
            default_line_height_factor
        } else {
            configured_line_height_factor
        };

        let font_size = style.font_size.max(1.0);
        let width_model = LineWidthModel {
            font_size,
            source: width_source,
            style,
            html_text: wrap_mode == WrapMode::HtmlLike,
        };
        let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);
        let raw_lines = Self::normalized_text_lines(text);
        let raw_width = (wrap_mode == WrapMode::HtmlLike || max_width.is_none()).then(|| {
            raw_lines
                .iter()
                .fold(0.0_f64, |width, line| width.max(width_model.width_px(line)))
        });
        let html_break_spaces_active = wrap_mode == WrapMode::HtmlLike
            && max_width
                .is_some_and(|max_width| raw_width.is_some_and(|raw_width| raw_width > max_width));
        let (mut width, line_count) = if wrap_mode == WrapMode::HtmlLike
            && let Some(max_width) = max_width
        {
            let mut width = 0.0_f64;
            let mut line_count = 0;
            for line in raw_lines {
                let line = if html_break_spaces_active {
                    line.as_str()
                } else {
                    // Mermaid enables break-spaces only after the natural HTML width exceeds
                    // its container. Before that switch, keep the previous wrapper's edge-space
                    // trimming while retaining interior spaces and non-breaking separators.
                    trim_html_collapsible_ascii_whitespace(&line)
                };
                let normalized = NormalizedHtmlText::single_mode(line, HtmlWhiteSpace::BreakSpaces);
                let builtin = match width_source {
                    LineWidthSource::Heuristic => Some(
                        crate::environment::BuiltinHtmlLineWidth::deterministic(style, 0.0),
                    ),
                    LineWidthSource::UniformAdvanceEm(factor) => Some(
                        crate::environment::BuiltinHtmlLineWidth::deterministic(style, factor),
                    ),
                    LineWidthSource::Callback(_) => None,
                };
                let mut measure = |text: &str| Ok::<f64, Infallible>(width_model.width_px(text));
                let mut checkpoint = || Ok::<(), Infallible>(());
                let planned = match plan_html_lines(
                    &normalized,
                    Some(max_width),
                    builtin,
                    &mut measure,
                    &mut checkpoint,
                ) {
                    Ok(planned) => planned,
                    Err(never) => match never {},
                };
                for line in planned {
                    width = width.max(line.width);
                    line_count += 1;
                }
            }
            (width, line_count)
        } else {
            let mut lines = Vec::new();
            for line in raw_lines {
                if let Some(max_width) = max_width {
                    lines.extend(Self::wrap_svg_line(&line, max_width, width_model));
                } else {
                    lines.push(line);
                }
            }
            let width = if max_width.is_none() {
                raw_width.expect("unwrapped measurement computes raw width")
            } else {
                lines
                    .iter()
                    .fold(0.0_f64, |width, line| width.max(width_model.width_px(line)))
            };
            (width, lines.len())
        };
        if html_break_spaces_active && let Some(max_width) = max_width {
            // Mermaid switches overflowing HTML labels to a fixed-width table. Breakable text
            // therefore occupies the configured width, while an unbreakable segment may still
            // expand it through its min-content width.
            width = width.max(max_width);
        }
        let height = line_count as f64 * font_size * line_height_factor;
        let metrics = TextMetrics {
            width,
            height,
            line_count,
        };
        let raw_width_px = if wrap_mode == WrapMode::HtmlLike {
            Some(raw_width.expect("HTML measurement computes raw width"))
        } else {
            None
        };
        (metrics, raw_width_px)
    }
}

#[cfg(test)]
mod tests {
    use super::{DeterministicTextMeasurer, LineWidthModel, LineWidthSource};
    use crate::text::{TextMeasurer, TextStyle, WrapMode};
    use std::sync::{Arc, Mutex};
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn wrapping_uses_estimated_width_instead_of_character_count() {
        let measurer = DeterministicTextMeasurer::default();
        let metrics = measurer.measure_wrapped(
            "iiii WWWW",
            &TextStyle {
                font_size: 10.0,
                ..TextStyle::default()
            },
            Some(20.0),
            WrapMode::SvgLike,
        );

        assert_eq!(metrics.line_count, 3);
        assert!((metrics.width - 17.0).abs() < f64::EPSILON, "{metrics:?}");
    }

    #[test]
    fn long_word_splitting_keeps_zero_width_scalars_with_visible_text() {
        let mut completed = Vec::new();
        let result = DeterministicTextMeasurer::split_long_token_to_lines(
            "W\u{0301}W",
            8.0,
            LineWidthModel {
                font_size: 10.0,
                source: LineWidthSource::Heuristic,
                style: &TextStyle::default(),
                html_text: false,
            },
            &mut completed,
        );
        let tail = result.tail;
        assert_eq!(completed, ["W\u{0301}", "W"]);
        assert!(tail.is_none(), "both visible graphemes exceed the line");

        let metrics = DeterministicTextMeasurer::default().measure_wrapped(
            "\u{0301}WW",
            &TextStyle {
                font_size: 10.0,
                ..TextStyle::default()
            },
            Some(8.5),
            WrapMode::SvgLike,
        );

        assert_eq!(metrics.line_count, 2);
        assert!((metrics.width - 8.5).abs() < f64::EPSILON, "{metrics:?}");
    }

    #[test]
    fn default_width_uses_sequence_aware_unicode_display_width() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };

        for sequence in ["👩‍🔬", "👨‍👩‍👧‍👦", "👍🏽", "🇨🇳", "1️⃣"]
        {
            let metrics = measurer.measure(sequence, &style);
            assert_eq!(metrics.width, 16.0, "sequence={sequence:?}");
        }
    }

    #[test]
    fn wrapping_splits_long_emoji_runs_only_at_grapheme_boundaries() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let width_model = LineWidthModel {
            font_size: 16.0,
            source: LineWidthSource::Heuristic,
            style: &style,
            html_text: false,
        };

        for (text, first, second) in [
            ("👩‍🔬👨‍🔬", "👩‍🔬", "👨‍🔬"),
            ("👍🏽👍🏻", "👍🏽", "👍🏻"),
            ("🇨🇳🇺🇸", "🇨🇳", "🇺🇸"),
            ("1️⃣2️⃣", "1️⃣", "2️⃣"),
        ] {
            let mut completed = Vec::new();
            let result = DeterministicTextMeasurer::split_long_token_to_lines(
                text,
                16.0,
                width_model,
                &mut completed,
            );
            let (tail, _) = result
                .tail
                .expect("the final emoji grapheme fits on the next line");
            assert_eq!(completed, [first], "text={text:?}");
            assert_eq!(tail, second, "text={text:?}");

            let metrics = measurer.measure_wrapped(text, &style, Some(16.0), WrapMode::SvgLike);
            assert_eq!(metrics.line_count, 2, "text={text:?}: {metrics:?}");
            assert_eq!(metrics.width, 16.0, "text={text:?}: {metrics:?}");
        }
    }

    #[test]
    fn long_word_split_work_is_linear_in_graphemes_and_output_bytes() {
        let width_model = LineWidthModel {
            font_size: 10.0,
            source: LineWidthSource::Heuristic,
            style: &TextStyle::default(),
            html_text: false,
        };

        for grapheme_count in [128, 256, 512] {
            let text = "W".repeat(grapheme_count);
            let mut lines = Vec::new();
            let result = DeterministicTextMeasurer::split_long_token_to_lines(
                &text,
                8.5,
                width_model,
                &mut lines,
            );
            let (tail, _) = result.tail.expect("the final grapheme fits exactly");
            lines.push(tail.to_string());

            assert_eq!(lines.len(), grapheme_count);
            assert!(lines.iter().all(|line| line == "W"));
            assert_eq!(lines.concat(), text);
            assert_eq!(result.stats.grapheme_visits, grapheme_count);
            assert_eq!(result.stats.partitioned_bytes, text.len());
        }
    }

    #[test]
    fn long_word_split_work_accounts_for_bytes_inside_one_grapheme() {
        let width_model = LineWidthModel {
            font_size: 10.0,
            source: LineWidthSource::Heuristic,
            style: &TextStyle::default(),
            html_text: false,
        };

        for combining_scalars in [128, 256, 512] {
            let text = format!("W{}", "\u{0301}".repeat(combining_scalars));
            let mut lines = Vec::new();
            let result = DeterministicTextMeasurer::split_long_token_to_lines(
                &text,
                8.0,
                width_model,
                &mut lines,
            );

            assert!(
                result.tail.is_none(),
                "the over-wide grapheme is emitted directly"
            );
            assert_eq!(lines, [text.as_str()]);
            assert_eq!(result.stats.grapheme_visits, 1);
            assert_eq!(result.stats.partitioned_bytes, text.len());
        }
    }

    #[test]
    fn long_word_final_tail_can_share_its_line_with_following_tokens() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let width_model = LineWidthModel {
            font_size: 10.0,
            source: LineWidthSource::Heuristic,
            style: &TextStyle::default(),
            html_text: false,
        };
        let lines = DeterministicTextMeasurer::wrap_svg_line("WWW i", 15.0, width_model);

        assert_eq!(lines, ["W", "W", "W i"]);
        let metrics = measurer.measure_wrapped("WWW i", &style, Some(15.0), WrapMode::SvgLike);
        assert_eq!(metrics.line_count, lines.len());
        assert_eq!(metrics.width.to_bits(), 14.6_f64.to_bits());
    }

    #[test]
    fn long_word_final_tail_discards_collapsible_invisible_prefix() {
        for invisible_prefix in ["\t ", "\r "] {
            for width_model in [
                LineWidthModel {
                    font_size: 10.0,
                    source: LineWidthSource::Heuristic,
                    style: &TextStyle::default(),
                    html_text: false,
                },
                LineWidthModel {
                    font_size: 10.0,
                    source: LineWidthSource::UniformAdvanceEm(0.6),
                    style: &TextStyle::default(),
                    html_text: false,
                },
            ] {
                let text = format!("{invisible_prefix}WWW");
                let lines = DeterministicTextMeasurer::wrap_svg_line(&text, 10.0, width_model);

                assert_eq!(lines, ["W", "W", "W"], "text={text:?}");
                assert!(
                    lines.iter().all(|line| width_model.width_px(line) <= 10.0),
                    "text={text:?}, lines={lines:?}"
                );
            }
        }
    }

    #[test]
    fn html_long_words_preserve_their_min_content_width() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };

        let metrics = measurer.measure_wrapped(
            "Supercalifragilisticexpialidocious",
            &style,
            Some(20.0),
            WrapMode::HtmlLike,
        );

        assert_eq!(metrics.line_count, 1);
        assert!(metrics.width > 20.0, "{metrics:?}");
    }

    #[test]
    fn html_wrapping_uses_unicode_soft_breaks_and_fixed_container_width() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };

        let cjk = measurer.measure_wrapped("负责人审批", &style, Some(25.0), WrapMode::HtmlLike);
        assert_eq!(cjk.line_count, 3);
        assert_eq!(cjk.width.to_bits(), 25.0_f64.to_bits());

        let breakable =
            measurer.measure_wrapped("alpha beta", &style, Some(40.0), WrapMode::HtmlLike);
        assert_eq!(breakable.line_count, 2);
        assert_eq!(breakable.width.to_bits(), 40.0_f64.to_bits());

        let hyphenated =
            measurer.measure_wrapped("half-rounded", &style, Some(21.5), WrapMode::HtmlLike);
        assert_eq!(hyphenated.line_count, 2);
        assert!(
            hyphenated.width > 21.5,
            "unbreakable segment keeps min-content width: {hyphenated:?}"
        );

        let preserved_spaces =
            measurer.measure_wrapped("a  b", &style, Some(7.0), WrapMode::HtmlLike);
        assert_eq!(preserved_spaces.line_count, 3);
        assert!(preserved_spaces.width > 7.0, "{preserved_spaces:?}");
    }

    #[test]
    fn html_soft_hyphen_is_hidden_until_selected_and_contributes_its_visible_width() {
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..DeterministicTextMeasurer::default()
        };
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let text = "alpha\u{00ad}beta";

        let natural = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
        assert_eq!(natural.width, 90.0);
        assert_eq!(natural.line_count, 1);
        let fitted = measurer.measure_wrapped(text, &style, Some(90.0), WrapMode::HtmlLike);
        assert_eq!(fitted.width, 90.0);
        assert_eq!(fitted.line_count, 1);

        let wrapped = measurer.measure_wrapped(text, &style, Some(60.0), WrapMode::HtmlLike);
        assert_eq!(wrapped.line_count, 2);
        assert_eq!(wrapped.width, 60.0);
        let (wrapped, raw_width) =
            measurer.measure_wrapped_with_raw_width(text, &style, Some(50.0), WrapMode::HtmlLike);
        assert_eq!(wrapped.line_count, 2);
        assert_eq!(
            wrapped.width, 60.0,
            "the selected hyphen contributes min-content width"
        );
        assert_eq!(raw_width, Some(90.0));
    }

    #[test]
    fn html_inactive_break_spaces_trims_only_collapsible_edges_and_keeps_raw_width() {
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..DeterministicTextMeasurer::default()
        };
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let text = " A  B\u{00a0} ";
        let natural = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
        assert_eq!(natural.width, 70.0);
        let (fitted, raw) =
            measurer.measure_wrapped_with_raw_width(text, &style, Some(70.0), WrapMode::HtmlLike);
        assert_eq!(fitted.width, 50.0);
        assert_eq!(fitted.line_count, 1);
        assert_eq!(raw, Some(70.0));

        let (active, raw) =
            measurer.measure_wrapped_with_raw_width(text, &style, Some(60.0), WrapMode::HtmlLike);
        assert_eq!(active.line_count, 2);
        assert_eq!(active.width, 60.0);
        assert_eq!(raw, Some(70.0));
    }

    #[test]
    fn html_soft_hyphen_callback_preserves_shaping_controls_in_complete_candidates() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&calls);
        let measurer = DeterministicTextMeasurer::default().with_width_callback(move |text, _| {
            recorded.lock().unwrap().push(text.to_string());
            match text {
                "f\u{00ad}fi" => 18.0,
                "f-" => 16.0,
                "fi" => 11.0,
                _ => text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64 * 10.0,
            }
        });
        let style = TextStyle::default();
        let text = "f\u{00ad}fi";

        let natural = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
        assert_eq!(natural.width, 18.0);
        assert_eq!(&*calls.lock().unwrap(), &["f\u{00ad}fi"]);
        calls.lock().unwrap().clear();

        let wrapped = measurer.measure_wrapped(text, &style, Some(16.0), WrapMode::HtmlLike);
        assert_eq!(wrapped.line_count, 2);
        assert_eq!(wrapped.width, 16.0);
        let calls = calls.lock().unwrap();
        assert!(calls.iter().any(|candidate| candidate == "f\u{00ad}fi"));
        assert!(calls.iter().any(|candidate| candidate == "f-"));
        assert!(calls.iter().any(|candidate| candidate == "fi"));
    }

    #[test]
    fn decoded_html_line_width_keeps_literal_markup_and_matches_streamed_candidates() {
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        for char_width_factor in [0.0, 1.0] {
            let measurer = DeterministicTextMeasurer {
                char_width_factor,
                ..DeterministicTextMeasurer::default()
            };
            let text = "left<br>👩‍🔬A\u{301}right";
            let expected = measurer.measure_html_line_width_px(text, &style);
            let mut streamed = measurer
                .begin_html_line_width(&style)
                .expect("built-in line width");
            for part in ["left<", "br>", "👩", "\u{200d}", "🔬A", "\u{301}right"] {
                streamed.push_text(part);
            }
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            assert!(
                expected
                    > measurer
                        .measure_wrapped(text, &style, None, WrapMode::HtmlLike)
                        .width
            );
            assert_eq!(
                measurer
                    .measure_html_line_width_px("a\u{00ad}b", &style)
                    .to_bits(),
                measurer.measure_html_line_width_px("ab", &style).to_bits()
            );
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&calls);
        let callback = DeterministicTextMeasurer::default().with_width_callback(move |text, _| {
            recorded.lock().unwrap().push(text.to_string());
            23.5
        });
        assert_eq!(callback.measure_html_line_width_px("a<br>b", &style), 23.5);
        assert_eq!(&*calls.lock().unwrap(), &["a<br>b"]);
    }

    #[test]
    fn repeated_measurement_is_bitwise_deterministic() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let first = measurer.measure_wrapped(
            "Latin / 中文 / 🙂 / e\u{0301}",
            &style,
            Some(80.0),
            WrapMode::SvgLike,
        );
        let second = measurer.measure_wrapped(
            "Latin / 中文 / 🙂 / e\u{0301}",
            &style,
            Some(80.0),
            WrapMode::SvgLike,
        );

        assert_eq!(first.width.to_bits(), second.width.to_bits());
        assert_eq!(first.height.to_bits(), second.height.to_bits());
        assert_eq!(first.line_count, second.line_count);
    }

    #[test]
    fn exact_natural_width_does_not_trigger_a_spurious_wrap() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let text = "FtQHasK pGRJ";

        for mode in [WrapMode::SvgLike, WrapMode::HtmlLike] {
            let natural = measurer.measure_wrapped(text, &style, None, mode);
            let fitted = measurer.measure_wrapped(text, &style, Some(natural.width), mode);
            assert_eq!(fitted.line_count, 1, "{mode:?}: {natural:?} -> {fitted:?}");
            assert_eq!(fitted.width.to_bits(), natural.width.to_bits());
        }
    }

    #[test]
    fn non_additive_callback_fits_at_exact_natural_width() {
        let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, _| {
            let count = text.chars().count();
            count as f64 * 10.0 - count.saturating_sub(1) as f64
        });
        let style = TextStyle::default();
        let text = "A V";

        for mode in [WrapMode::SvgLike, WrapMode::HtmlLike] {
            let natural = measurer.measure_wrapped(text, &style, None, mode);
            assert_eq!(natural.width, 28.0);
            let fitted = measurer.measure_wrapped(text, &style, Some(natural.width), mode);
            assert_eq!(fitted.line_count, 1, "{mode:?}: {natural:?} -> {fitted:?}");
            assert_eq!(fitted.width.to_bits(), natural.width.to_bits());
        }
    }

    #[test]
    fn callback_preserves_html_fixed_width_and_long_word_overflow() {
        let measurer = DeterministicTextMeasurer::default()
            .with_width_callback(|text, _| text.graphemes(true).count() as f64 * 10.0);
        let style = TextStyle::default();

        let breakable =
            measurer.measure_wrapped("alpha beta", &style, Some(60.0), WrapMode::HtmlLike);
        assert_eq!(breakable.line_count, 2);
        assert_eq!(breakable.width, 60.0);

        let long_word =
            measurer.measure_wrapped("abcdefgh", &style, Some(30.0), WrapMode::HtmlLike);
        assert_eq!(long_word.line_count, 1);
        assert_eq!(long_word.width, 80.0);
    }

    #[test]
    fn callback_svg_wrapping_splits_only_at_grapheme_boundaries() {
        let measurer = DeterministicTextMeasurer::default()
            .with_width_callback(|text, _| text.graphemes(true).count() as f64 * 10.0);

        let metrics =
            measurer.measure_wrapped("👩‍🔬👨‍🔬", &TextStyle::default(), Some(10.0), WrapMode::SvgLike);

        assert_eq!(metrics.line_count, 2);
        assert_eq!(metrics.width, 10.0);
    }

    #[test]
    fn callback_receives_complete_text_and_returns_pixel_width() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&calls);
        let measurer =
            DeterministicTextMeasurer::default().with_width_callback(move |text, style| {
                recorded
                    .lock()
                    .unwrap()
                    .push((text.to_string(), style.font_size));
                37.0
            });
        let style = TextStyle {
            font_size: 99.0,
            ..TextStyle::default()
        };

        assert_eq!(measurer.measure("ffi", &style).width, 37.0);
        assert_eq!(&*calls.lock().unwrap(), &[("ffi".to_string(), 99.0)]);
    }
}
