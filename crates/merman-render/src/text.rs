use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    SvgLike,
    HtmlLike,
}

impl Default for WrapMode {
    fn default() -> Self {
        Self::SvgLike
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 16.0,
            font_weight: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
}

pub fn flowchart_html_line_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * 1.5).max(1.0)
}

pub fn flowchart_apply_mermaid_string_whitespace_height_parity(
    metrics: &mut TextMetrics,
    raw_label: &str,
    style: &TextStyle,
) {
    if metrics.width <= 0.0 && metrics.height <= 0.0 {
        return;
    }

    // Mermaid FlowDB preserves leading/trailing whitespace when the label comes from a quoted
    // string (e.g. `[" test "]`). In Mermaid@11.12.2, HTML label measurement ends up allocating an
    // extra empty line for each side when such whitespace is present, even though the rendered
    // HTML collapses it.
    //
    // This behavior is observable in upstream SVG fixtures (e.g.
    // `upstream_flow_vertice_chaining_with_extras_spec` where `" test "` yields a 3-line label box).
    let bytes = raw_label.as_bytes();
    if bytes.is_empty() {
        return;
    }
    let leading_ws = matches!(bytes.first(), Some(b' ' | b'\t'));
    let trailing_ws = matches!(bytes.last(), Some(b' ' | b'\t'));
    let extra = leading_ws as usize + trailing_ws as usize;
    if extra == 0 {
        return;
    }

    let line_h = flowchart_html_line_height_px(style.font_size);
    metrics.height += extra as f64 * line_h;
    metrics.line_count = metrics.line_count.saturating_add(extra);
}

pub fn flowchart_apply_mermaid_styled_node_height_parity(
    metrics: &mut TextMetrics,
    style: &TextStyle,
) {
    if metrics.width <= 0.0 && metrics.height <= 0.0 {
        return;
    }

    // Mermaid@11.12.2 HTML label measurement for styled flowchart nodes (nodes with inline style or
    // classDef-applied style) often results in a 3-line label box, even when the label is a single
    // line. This is observable in upstream SVG fixtures (e.g.
    // `upstream_flow_style_inline_class_variants_spec` where `test` inside `:::exClass` becomes a
    // 72px-tall label box, yielding a 102px node height with padding).
    //
    // Model this as "at least 3 lines" in headless metrics so layout and foreignObject sizing match.
    let min_lines = 3usize;
    if metrics.line_count >= min_lines {
        return;
    }

    let line_h = flowchart_html_line_height_px(style.font_size);
    let extra = min_lines - metrics.line_count;
    metrics.height += extra as f64 * line_h;
    metrics.line_count = min_lines;
}

pub fn ceil_to_1_64_px(v: f64) -> f64 {
    if !(v.is_finite() && v >= 0.0) {
        return 0.0;
    }
    (v * 64.0).ceil() / 64.0
}

fn normalize_font_key(s: &str) -> String {
    s.chars()
        .filter_map(|ch| {
            if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                None
            } else {
                Some(ch.to_ascii_lowercase())
            }
        })
        .collect()
}

fn is_flowchart_default_font(style: &TextStyle) -> bool {
    let Some(f) = style.font_family.as_deref() else {
        return false;
    };
    normalize_font_key(f) == "trebuchetms,verdana,arial,sans-serif"
}

fn flowchart_default_bold_delta_em(ch: char) -> f64 {
    // Derived from browser `canvas.measureText()` using `font: bold 16px trebuchet ms, verdana, arial, sans-serif`.
    // Values are `bold_em(ch) - regular_em(ch)`.
    match ch {
        'a' => 0.00732421875,
        'c' => 0.0166015625,
        'd' => 0.0234375,
        'e' => 0.029296875,
        'h' => 0.04638671875,
        'i' => 0.01318359375,
        'o' => 0.029296875,
        'p' => 0.025390625,
        'T' => 0.03125,
        'w' => 0.03955078125,
        _ => 0.0,
    }
}

pub fn measure_markdown_with_flowchart_bold_deltas(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    let mut plain = String::new();
    let mut deltas_px_by_line: Vec<f64> = vec![0.0];
    let mut strong_depth: usize = 0;

    let parser = pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    );

    for ev in parser {
        match ev {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => {
                strong_depth += 1;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {
                strong_depth = strong_depth.saturating_sub(1);
            }
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                plain.push_str(&t);
                if strong_depth > 0 && is_flowchart_default_font(style) {
                    let line_idx = deltas_px_by_line.len().saturating_sub(1);
                    for ch in t.chars() {
                        deltas_px_by_line[line_idx] +=
                            flowchart_default_bold_delta_em(ch) * style.font_size.max(1.0);
                    }
                }
            }
            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                plain.push('\n');
                deltas_px_by_line.push(0.0);
            }
            _ => {}
        }
    }

    let plain = plain.trim().to_string();
    let base = measurer.measure_wrapped(&plain, style, max_width, wrap_mode);

    let mut lines = DeterministicTextMeasurer::normalized_text_lines(&plain);
    if lines.is_empty() {
        lines.push(String::new());
    }
    deltas_px_by_line.resize(lines.len(), 0.0);

    let mut max_line_width: f64 = 0.0;
    for (idx, line) in lines.iter().enumerate() {
        let w = measurer.measure_wrapped(line, style, None, wrap_mode).width;
        max_line_width = max_line_width.max(w + deltas_px_by_line[idx]);
    }

    let mut width = ceil_to_1_64_px(max_line_width);
    if wrap_mode == WrapMode::HtmlLike {
        if let Some(w) = max_width.filter(|w| w.is_finite() && *w > 0.0) {
            if max_line_width > w {
                width = w;
            } else {
                width = width.min(w);
            }
        }
    }

    TextMetrics {
        width,
        height: base.height,
        line_count: base.line_count,
    }
}

pub trait TextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics;

    /// Measures the horizontal extents of an SVG `<text>` element relative to its anchor `x`.
    ///
    /// Mermaid's flowchart-v2 viewport sizing uses `getBBox()` on the rendered SVG. For `<text>`
    /// elements this bbox can be slightly asymmetric around the anchor due to glyph overhangs.
    ///
    /// Default implementation assumes a symmetric bbox: `left = right = width/2`.
    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        let m = self.measure(text, style);
        let half = (m.width.max(0.0)) / 2.0;
        (half, half)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let _ = max_width;
        let _ = wrap_mode;
        self.measure(text, style)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeterministicTextMeasurer {
    pub char_width_factor: f64,
    pub line_height_factor: f64,
}

impl DeterministicTextMeasurer {
    fn replace_br_variants(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            if text[i..].starts_with("<br") {
                let next = text[i + 3..].chars().next();
                let is_valid = match next {
                    None => true,
                    Some(ch) => ch.is_whitespace() || ch == '/' || ch == '>',
                };
                if is_valid {
                    if let Some(rel_end) = text[i..].find('>') {
                        i += rel_end + 1;
                        out.push('\n');
                        continue;
                    }
                }
            }

            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    pub fn normalized_text_lines(text: &str) -> Vec<String> {
        let t = Self::replace_br_variants(text);
        let mut out = t.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();

        // Mermaid often produces labels with a trailing newline (e.g. YAML `|` block scalars from
        // FlowDB). The rendered label does not keep an extra blank line at the end, so we trim
        // trailing empty lines to keep height parity.
        while out.len() > 1 && out.last().is_some_and(|s| s.trim().is_empty()) {
            out.pop();
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
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

    fn wrap_line(line: &str, max_chars: usize, break_long_words: bool) -> Vec<String> {
        if max_chars == 0 {
            return vec![line.to_string()];
        }

        let mut tokens = std::collections::VecDeque::from(Self::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            if candidate.chars().count() <= max_chars {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            // `tok` itself does not fit on an empty line.
            if tok == " " {
                continue;
            }
            if !break_long_words {
                out.push(tok);
            } else {
                // Split it by characters (Mermaid SVG text mode behavior).
                let tok_chars = tok.chars().collect::<Vec<_>>();
                let head: String = tok_chars.iter().take(max_chars.max(1)).collect();
                let tail: String = tok_chars.iter().skip(max_chars.max(1)).collect();
                out.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VendoredFontMetricsTextMeasurer {
    fallback: DeterministicTextMeasurer,
}

impl VendoredFontMetricsTextMeasurer {
    fn normalize_font_key(s: &str) -> String {
        s.chars()
            .filter_map(|ch| {
                // Mermaid config strings occasionally embed the trailing CSS `;` in `fontFamily`.
                // We treat it as syntactic noise so lookups work with both `...sans-serif` and
                // `...sans-serif;`.
                if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                    None
                } else {
                    Some(ch.to_ascii_lowercase())
                }
            })
            .collect()
    }

    fn lookup_table(
        &self,
        style: &TextStyle,
    ) -> Option<&'static crate::generated::font_metrics_flowchart_11_12_2::FontMetricsTables> {
        let key = style
            .font_family
            .as_deref()
            .map(Self::normalize_font_key)
            .unwrap_or_default();
        if key.is_empty() {
            return None;
        }
        crate::generated::font_metrics_flowchart_11_12_2::lookup_font_metrics(&key)
    }

    fn lookup_char_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return entries[mid].1,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        default_em
    }

    fn lookup_kern_em(kern_pairs: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = kern_pairs.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = kern_pairs[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_overhang_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return entries[mid].1,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        default_em
    }

    fn line_width_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        text: &str,
        font_size: f64,
    ) -> f64 {
        let mut em = 0.0;
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            em += Self::lookup_char_em(entries, default_em, ch);
            if let Some(p) = prev {
                em += Self::lookup_kern_em(kern_pairs, p, ch);
            }
            prev = Some(ch);
        }
        em * font_size
    }

    fn ceil_to_1_64_px(v: f64) -> f64 {
        if !(v.is_finite() && v >= 0.0) {
            return 0.0;
        }
        (v * 64.0).ceil() / 64.0
    }

    fn split_token_to_width_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        tok: &str,
        max_width_px: f64,
        font_size: f64,
    ) -> (String, String) {
        if max_width_px <= 0.0 {
            return (tok.to_string(), String::new());
        }
        let max_em = max_width_px / font_size.max(1.0);
        let mut em = 0.0;
        let mut prev: Option<char> = None;
        let chars = tok.chars().collect::<Vec<_>>();
        let mut split_at = 0usize;
        for (idx, ch) in chars.iter().enumerate() {
            em += Self::lookup_char_em(entries, default_em, *ch);
            if let Some(p) = prev {
                em += Self::lookup_kern_em(kern_pairs, p, *ch);
            }
            prev = Some(*ch);
            if em > max_em && idx > 0 {
                break;
            }
            split_at = idx + 1;
            if em >= max_em {
                break;
            }
        }
        if split_at == 0 {
            split_at = 1.min(chars.len());
        }
        let head = chars.iter().take(split_at).collect::<String>();
        let tail = chars.iter().skip(split_at).collect::<String>();
        (head, tail)
    }

    fn wrap_line_to_width_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        line: &str,
        max_width_px: f64,
        font_size: f64,
        break_long_words: bool,
    ) -> Vec<String> {
        let mut tokens =
            std::collections::VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_trimmed = candidate.trim_end();
            if Self::line_width_px(
                entries,
                default_em,
                kern_pairs,
                candidate_trimmed,
                font_size,
            ) <= max_width_px
            {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
            }

            if tok == " " {
                continue;
            }

            if Self::line_width_px(entries, default_em, kern_pairs, tok.as_str(), font_size)
                <= max_width_px
            {
                cur = tok;
                continue;
            }

            if !break_long_words {
                out.push(tok);
                continue;
            }

            let (head, tail) = Self::split_token_to_width_px(
                entries,
                default_em,
                kern_pairs,
                &tok,
                max_width_px,
                font_size,
            );
            out.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    fn wrap_text_lines_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        text: &str,
        style: &TextStyle,
        max_width_px: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Vec<String> {
        let font_size = style.font_size.max(1.0);
        let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);
        let break_long_words = wrap_mode == WrapMode::SvgLike;

        let mut lines = Vec::new();
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            if let Some(w) = max_width_px {
                lines.extend(Self::wrap_line_to_width_px(
                    entries,
                    default_em,
                    kern_pairs,
                    &line,
                    w,
                    font_size,
                    break_long_words,
                ));
            } else {
                lines.push(line);
            }
        }

        if lines.is_empty() {
            vec!["".to_string()]
        } else {
            lines
        }
    }
}

impl TextMeasurer for VendoredFontMetricsTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::SvgLike)
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        // NOTE: Mermaid's `<text>.getBBox()` can be asymmetric due to glyph overhangs, but a
        // headless approximation derived from upstream `viewBox` deltas proved unstable across
        // fixtures. Keep vendored font metrics deterministic by assuming a symmetric bbox.
        //
        // If we later add a reliable browser-backed measurement backend, we can re-enable an
        // asymmetric model behind a feature flag without changing call sites.
        let advance = self.measure(text, style).width.max(0.0);
        let half = advance / 2.0;
        (half, half)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_wrapped(text, style, max_width, wrap_mode);
        };

        let font_size = style.font_size.max(1.0);
        let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);
        let line_height_factor = match wrap_mode {
            WrapMode::SvgLike => 1.1,
            WrapMode::HtmlLike => 1.5,
        };

        let scale = match wrap_mode {
            WrapMode::HtmlLike => 1.0,
            WrapMode::SvgLike => table.svg_scale.max(0.1),
        };
        let max_width_unscaled = max_width.map(|w| w / scale);

        // Mermaid HTML labels behave differently depending on whether the content "needs" wrapping:
        // - if the unwrapped line width exceeds the configured wrapping width, Mermaid constrains
        //   the element to `width=max_width` and lets HTML wrapping determine line breaks
        //   (`white-space: break-spaces` / `width: 200px` patterns in upstream SVGs).
        // - otherwise, Mermaid uses an auto-sized container and measures the natural width.
        //
        // In headless mode we model this by computing the unwrapped width first, then forcing the
        // measured width to `max_width` when it would overflow.
        let raw_width_unscaled = if wrap_mode == WrapMode::HtmlLike {
            let mut raw_w: f64 = 0.0;
            for line in DeterministicTextMeasurer::normalized_text_lines(text) {
                raw_w = raw_w.max(Self::line_width_px(
                    table.entries,
                    table.default_em.max(0.1),
                    table.kern_pairs,
                    &line,
                    font_size,
                ));
            }
            Some(raw_w)
        } else {
            None
        };

        let lines = Self::wrap_text_lines_px(
            table.entries,
            table.default_em.max(0.1),
            table.kern_pairs,
            text,
            style,
            max_width_unscaled,
            wrap_mode,
        );

        let mut width: f64 = 0.0;
        for line in &lines {
            width = width.max(Self::line_width_px(
                table.entries,
                table.default_em.max(0.1),
                table.kern_pairs,
                line,
                font_size,
            ));
        }

        // Mermaid HTML labels use `max-width` and can visually overflow for long words, but their
        // layout width is effectively clamped to the max width.
        if wrap_mode == WrapMode::HtmlLike {
            if let Some(w) = max_width {
                let needs_wrap = raw_width_unscaled.is_some_and(|rw| rw > w);
                if needs_wrap {
                    width = w;
                } else {
                    width = width.min(w);
                }
            }
            // Empirically, Mermaid's HTML label widths (via `getBoundingClientRect()`) behave like
            // `ceil(width * 64) / 64` over the underlying text advances.
            width = Self::ceil_to_1_64_px(width);
            if let Some(w) = max_width {
                width = width.min(w);
            }
        } else {
            width *= scale;
        }

        let height = lines.len() as f64 * font_size * line_height_factor;
        TextMetrics {
            width,
            height,
            line_count: lines.len(),
        }
    }
}

impl TextMeasurer for DeterministicTextMeasurer {
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
        let uses_heuristic_widths = self.char_width_factor == 0.0;
        let char_width_factor = if uses_heuristic_widths {
            match wrap_mode {
                WrapMode::SvgLike => 0.6,
                WrapMode::HtmlLike => 0.5,
            }
        } else {
            self.char_width_factor
        };
        let default_line_height_factor = match wrap_mode {
            WrapMode::SvgLike => 1.1,
            WrapMode::HtmlLike => 1.5,
        };
        let line_height_factor = if self.line_height_factor == 0.0 {
            default_line_height_factor
        } else {
            self.line_height_factor
        };

        let font_size = style.font_size.max(1.0);
        let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);
        let break_long_words = wrap_mode == WrapMode::SvgLike;

        let raw_lines = Self::normalized_text_lines(text);
        let mut raw_width: f64 = 0.0;
        for line in &raw_lines {
            let w = if uses_heuristic_widths {
                estimate_line_width_px(line, font_size)
            } else {
                line.chars().count() as f64 * font_size * char_width_factor
            };
            raw_width = raw_width.max(w);
        }
        let needs_wrap =
            wrap_mode == WrapMode::HtmlLike && max_width.is_some_and(|w| raw_width > w);

        let mut lines = Vec::new();
        for line in raw_lines {
            if let Some(w) = max_width {
                let char_px = font_size * char_width_factor;
                let max_chars = ((w / char_px).floor() as isize).max(1) as usize;
                lines.extend(Self::wrap_line(&line, max_chars, break_long_words));
            } else {
                lines.push(line);
            }
        }

        let mut width: f64 = 0.0;
        for line in &lines {
            let w = if uses_heuristic_widths {
                estimate_line_width_px(line, font_size)
            } else {
                line.chars().count() as f64 * font_size * char_width_factor
            };
            width = width.max(w);
        }
        // Mermaid HTML labels use `max-width` and can visually overflow for long words, but their
        // layout width is effectively clamped to the max width. Mirror this to avoid explosive
        // headless widths when `htmlLabels=true`.
        if wrap_mode == WrapMode::HtmlLike {
            if let Some(w) = max_width {
                if needs_wrap {
                    width = w;
                } else {
                    width = width.min(w);
                }
            }
        }
        let height = lines.len() as f64 * font_size * line_height_factor;
        TextMetrics {
            width,
            height,
            line_count: lines.len(),
        }
    }
}

pub fn wrap_text_lines_px(
    text: &str,
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
) -> Vec<String> {
    let font_size = style.font_size.max(1.0);
    let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);
    let break_long_words = wrap_mode == WrapMode::SvgLike;

    fn split_token_to_width_px(tok: &str, max_width_px: f64, font_size: f64) -> (String, String) {
        let max_em = max_width_px / font_size;
        let mut em = 0.0;
        let chars = tok.chars().collect::<Vec<_>>();
        let mut split_at = 0usize;
        for (idx, ch) in chars.iter().enumerate() {
            em += estimate_char_width_em(*ch);
            if em > max_em && idx > 0 {
                break;
            }
            split_at = idx + 1;
            if em >= max_em {
                break;
            }
        }
        if split_at == 0 {
            split_at = 1.min(chars.len());
        }
        let head = chars.iter().take(split_at).collect::<String>();
        let tail = chars.iter().skip(split_at).collect::<String>();
        (head, tail)
    }

    fn wrap_line_to_width_px(
        line: &str,
        max_width_px: f64,
        font_size: f64,
        break_long_words: bool,
    ) -> Vec<String> {
        let mut tokens =
            std::collections::VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_trimmed = candidate.trim_end();
            if estimate_line_width_px(candidate_trimmed, font_size) <= max_width_px {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            if tok == " " {
                continue;
            }

            if !break_long_words {
                out.push(tok);
            } else {
                let (head, tail) = split_token_to_width_px(&tok, max_width_px, font_size);
                out.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for line in DeterministicTextMeasurer::normalized_text_lines(text) {
        if let Some(w) = max_width_px {
            lines.extend(wrap_line_to_width_px(&line, w, font_size, break_long_words));
        } else {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        vec!["".to_string()]
    } else {
        lines
    }
}

fn estimate_line_width_px(line: &str, font_size: f64) -> f64 {
    let mut em = 0.0;
    for ch in line.chars() {
        em += estimate_char_width_em(ch);
    }
    em * font_size
}

fn estimate_char_width_em(ch: char) -> f64 {
    if ch == ' ' {
        return 0.33;
    }
    if ch == '\t' {
        return 0.66;
    }
    if ch == '_' || ch == '-' {
        return 0.33;
    }
    if matches!(ch, '.' | ',' | ':' | ';') {
        return 0.28;
    }
    if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
        return 0.33;
    }
    if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
        return 0.45;
    }
    if ch.is_ascii_digit() {
        return 0.56;
    }
    if ch.is_ascii_uppercase() {
        return match ch {
            'I' => 0.30,
            'W' => 0.85,
            _ => 0.60,
        };
    }
    if ch.is_ascii_lowercase() {
        return match ch {
            'i' | 'l' => 0.28,
            'm' | 'w' => 0.78,
            'k' | 'y' => 0.55,
            _ => 0.43,
        };
    }
    // Punctuation/symbols/unicode: approximate.
    0.60
}
