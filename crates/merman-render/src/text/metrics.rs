//! Flowchart-aware text metrics and Markdown measurement helpers.

use super::line_break::html_break_spaces_segments;
use super::{
    DeterministicTextMeasurer, MermaidMarkdownWordType, TextMeasurer, TextMetrics, TextStyle,
    WrapMode, ceil_to_1_64_px, mermaid_markdown_to_lines, mermaid_xhtml_label_plain_text,
    round_to_1_64_px, wrap,
};

pub(crate) fn measure_xhtml_label_fragment(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    if let Some(plain_text) = mermaid_xhtml_label_plain_text(fragment) {
        measurer.measure_wrapped(&plain_text, style, max_width, wrap_mode)
    } else {
        measure_html_with_inline_styles(measurer, fragment, style, max_width, wrap_mode)
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
    lower.parse::<i32>().ok().is_some_and(|n| n >= 600)
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
}

fn push_inline_text_char(
    runs: &mut Vec<InlineTextRun>,
    ch: char,
    bold: bool,
    italic: bool,
    code: bool,
) {
    if let Some(run) = runs
        .last_mut()
        .filter(|run| run.bold == bold && run.italic == italic && run.code == code)
    {
        run.text.push(ch);
    } else {
        runs.push(InlineTextRun {
            text: ch.to_string(),
            bold,
            italic,
            code,
        });
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

fn measure_inline_run_width_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap_mode: WrapMode,
    svg_advance: bool,
) -> f64 {
    if svg_advance && wrap_mode != WrapMode::HtmlLike {
        measurer.measure_svg_text_computed_length_px(text, style)
    } else {
        measurer.measure_wrapped(text, style, None, wrap_mode).width
    }
}

fn measure_inline_runs_width_px(
    measurer: &dyn TextMeasurer,
    runs: &[InlineTextRun],
    style: &TextStyle,
    wrap_mode: WrapMode,
    svg_advance: bool,
) -> f64 {
    runs.iter()
        .filter(|run| !run.text.is_empty())
        .map(|run| {
            let run_style = inline_text_style(style, run.bold, run.italic, run.code);
            measure_inline_run_width_px(measurer, &run.text, &run_style, wrap_mode, svg_advance)
        })
        .sum()
}

fn append_inline_runs(target: &mut Vec<InlineTextRun>, source: &[InlineTextRun]) {
    for run in source.iter().filter(|run| !run.text.is_empty()) {
        if let Some(last) = target.last_mut().filter(|last| {
            last.bold == run.bold && last.italic == run.italic && last.code == run.code
        }) {
            last.text.push_str(&run.text);
        } else {
            target.push(run.clone());
        }
    }
}

fn split_inline_runs_at_html_breaks(runs: &[InlineTextRun]) -> Vec<Vec<InlineTextRun>> {
    let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
    if text.is_empty() {
        return vec![Vec::new()];
    }

    let mut segments = Vec::new();
    let mut segment_start = 0usize;
    for segment in html_break_spaces_segments(&text) {
        let segment_end = segment_start + segment.len();
        let mut segment_runs = Vec::new();
        let mut run_start = 0usize;

        for run in runs {
            let run_end = run_start + run.text.len();
            let overlap_start = segment_start.max(run_start);
            let overlap_end = segment_end.min(run_end);
            if overlap_start < overlap_end {
                let fragment = &run.text[overlap_start - run_start..overlap_end - run_start];
                append_inline_runs(
                    &mut segment_runs,
                    &[InlineTextRun {
                        text: fragment.to_string(),
                        bold: run.bold,
                        italic: run.italic,
                        code: run.code,
                    }],
                );
            }
            run_start = run_end;
        }

        segments.push(segment_runs);
        segment_start = segment_end;
    }
    segments
}

#[derive(Debug, Clone, Copy)]
struct InlineHtmlLineLayout {
    natural_width: f64,
    wrapped_width: f64,
    min_content_width: f64,
    line_count: usize,
}

fn measure_inline_html_line_layout(
    measurer: &dyn TextMeasurer,
    runs: &[InlineTextRun],
    style: &TextStyle,
    max_width: Option<f64>,
) -> InlineHtmlLineLayout {
    let natural_width =
        measure_inline_runs_width_px(measurer, runs, style, WrapMode::HtmlLike, false);
    let segments = split_inline_runs_at_html_breaks(runs);
    let min_content_width = segments
        .iter()
        .map(|segment| {
            measure_inline_runs_width_px(measurer, segment, style, WrapMode::HtmlLike, false)
        })
        .fold(0.0_f64, f64::max);

    let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
        };
    };
    if natural_width <= max_width {
        return InlineHtmlLineLayout {
            natural_width,
            wrapped_width: natural_width,
            min_content_width,
            line_count: 1,
        };
    }

    let mut current = Vec::new();
    let mut wrapped_width = 0.0_f64;
    let mut line_count = 0usize;
    for segment in segments {
        let mut candidate = current.clone();
        append_inline_runs(&mut candidate, &segment);
        let candidate_width =
            measure_inline_runs_width_px(measurer, &candidate, style, WrapMode::HtmlLike, false);
        if current.is_empty() || candidate_width <= max_width {
            current = candidate;
            continue;
        }

        wrapped_width = wrapped_width.max(measure_inline_runs_width_px(
            measurer,
            &current,
            style,
            WrapMode::HtmlLike,
            false,
        ));
        line_count += 1;
        current = segment;
    }

    if !current.is_empty() {
        wrapped_width = wrapped_width.max(measure_inline_runs_width_px(
            measurer,
            &current,
            style,
            WrapMode::HtmlLike,
            false,
        ));
        line_count += 1;
    }

    InlineHtmlLineLayout {
        natural_width,
        wrapped_width,
        min_content_width,
        line_count: line_count.max(1),
    }
}

fn measure_inline_html_layout(
    measurer: &dyn TextMeasurer,
    runs_by_line: &[Vec<InlineTextRun>],
    style: &TextStyle,
    max_width: Option<f64>,
) -> InlineHtmlLineLayout {
    let mut layout = InlineHtmlLineLayout {
        natural_width: 0.0,
        wrapped_width: 0.0,
        min_content_width: 0.0,
        line_count: 0,
    };
    for runs in runs_by_line {
        let line = measure_inline_html_line_layout(measurer, runs, style, max_width);
        layout.natural_width = layout.natural_width.max(line.natural_width);
        layout.wrapped_width = layout.wrapped_width.max(line.wrapped_width);
        layout.min_content_width = layout.min_content_width.max(line.min_content_width);
        layout.line_count += line.line_count;
    }
    layout.line_count = layout.line_count.max(1);
    layout
}

pub fn measure_html_with_inline_styles(
    measurer: &dyn TextMeasurer,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
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

        Some(round_to_1_64_px(font_size.max(1.0) * advance_em))
    }

    // Mermaid supports inline FontAwesome icons via `<i class="fa fa-..."></i>` inside HTML
    // labels. Upstream layout is computed with FontAwesome CSS available, while exported SVGs
    // keep only the empty `<i>` element. Model the layout-time glyph advance explicitly.
    fn decode_html_entity(entity: &str) -> Option<char> {
        match entity {
            "nbsp" => Some(' '),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "#39" => Some('\''),
            _ => {
                if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        }
    }

    let mut plain = String::new();
    let mut icon_width_px_by_line: Vec<f64> = vec![0.0];
    let mut icon_on_line: Vec<bool> = vec![false];
    let mut image_on_line: Vec<bool> = vec![false];
    let mut inline_runs_by_line: Vec<Vec<InlineTextRun>> = vec![Vec::new()];
    let mut strong_depth: usize = 0;
    let mut em_depth: usize = 0;
    let mut code_depth: usize = 0;
    let mut fa_icon_depth: usize = 0;

    let html = html.replace("\r\n", "\n");
    let mut it = html.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '<' {
            let mut tag = String::new();
            for c in it.by_ref() {
                if c == '>' {
                    break;
                }
                tag.push(c);
            }
            let tag = tag.trim();
            let tag_lower = tag.to_ascii_lowercase();
            let tag_trim = tag_lower.trim();
            if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                continue;
            }
            let is_closing = tag_trim.starts_with('/');
            let name = tag_trim
                .trim_start_matches('/')
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");

            let fontawesome_icon_width = if name == "i" && !is_closing {
                fontawesome_icon_width_px(tag, style.font_size)
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
                        if let Some(runs) = inline_runs_by_line.get_mut(line_idx) {
                            runs.push(InlineTextRun::default());
                        }
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
                    if let Some(slot) = image_on_line.last_mut() {
                        *slot = true;
                    }
                }
                "br" => {
                    plain.push('\n');
                    icon_width_px_by_line.push(0.0);
                    icon_on_line.push(false);
                    image_on_line.push(false);
                    inline_runs_by_line.push(Vec::new());
                }
                "p" | "div" | "li" | "tr" | "ul" | "ol" if is_closing => {
                    plain.push('\n');
                    icon_width_px_by_line.push(0.0);
                    icon_on_line.push(false);
                    image_on_line.push(false);
                    inline_runs_by_line.push(Vec::new());
                }
                _ => {}
            }
            continue;
        }

        let mut push_char =
            |decoded: char,
             plain: &mut String,
             icon_width_px_by_line: &mut Vec<f64>,
             icon_on_line: &mut Vec<bool>,
             inline_runs_by_line: &mut Vec<Vec<InlineTextRun>>| {
                plain.push(decoded);
                if decoded == '\n' {
                    icon_width_px_by_line.push(0.0);
                    icon_on_line.push(false);
                    image_on_line.push(false);
                    inline_runs_by_line.push(Vec::new());
                    return;
                }
                if let Some(runs) = inline_runs_by_line.last_mut() {
                    push_inline_text_char(
                        runs,
                        decoded,
                        strong_depth > 0,
                        em_depth > 0,
                        code_depth > 0,
                    );
                }
            };

        if ch == '&' {
            let mut entity = String::new();
            let mut saw_semicolon = false;
            while let Some(&c) = it.peek() {
                if c == ';' {
                    it.next();
                    saw_semicolon = true;
                    break;
                }
                if c == '<' || c == '&' || c.is_whitespace() || entity.len() > 32 {
                    break;
                }
                entity.push(c);
                it.next();
            }
            if saw_semicolon {
                if let Some(decoded) = decode_html_entity(entity.as_str()) {
                    push_char(
                        decoded,
                        &mut plain,
                        &mut icon_width_px_by_line,
                        &mut icon_on_line,
                        &mut inline_runs_by_line,
                    );
                } else {
                    for literal in format!("&{entity};").chars() {
                        push_char(
                            literal,
                            &mut plain,
                            &mut icon_width_px_by_line,
                            &mut icon_on_line,
                            &mut inline_runs_by_line,
                        );
                    }
                }
            } else {
                for literal in format!("&{entity}").chars() {
                    push_char(
                        literal,
                        &mut plain,
                        &mut icon_width_px_by_line,
                        &mut icon_on_line,
                        &mut inline_runs_by_line,
                    );
                }
            }
            continue;
        }

        push_char(
            ch,
            &mut plain,
            &mut icon_width_px_by_line,
            &mut icon_on_line,
            &mut inline_runs_by_line,
        );
    }

    // Keep whitespace adjacent to inline icons: in HTML it becomes significant when it separates
    // text from an inline-block `<i>` (for both `<i> text` and `text <i>`). Only drop the newline
    // that our lightweight parser adds for closing block tags.
    let plain = if icon_on_line.iter().any(|v| *v) {
        plain.trim_end_matches('\n').to_string()
    } else {
        plain.trim_end().to_string()
    };
    let base = measurer.measure_wrapped(plain.trim(), style, max_width, wrap_mode);

    // Consecutive `<br>` elements create empty inline line boxes. Keep those boxes up to the last
    // visible text/image line; a final image-only line is left to the browser-dependent replaced
    // element bounds instead of assigning it a guessed intrinsic height.
    let explicit_line_boxes = inline_runs_by_line
        .iter()
        .zip(&image_on_line)
        .rposition(|(runs, has_image)| !runs.is_empty() || *has_image)
        .map(|last_content_line| {
            inline_runs_by_line[..=last_content_line]
                .iter()
                .zip(&image_on_line[..=last_content_line])
                .filter(|(runs, has_image)| !runs.is_empty() || !**has_image)
                .count()
        })
        .unwrap_or(0);

    let mut lines = DeterministicTextMeasurer::normalized_text_lines(&plain);
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

    if wrap_mode == WrapMode::HtmlLike && !icon_on_line.iter().any(|has_icon| *has_icon) {
        let layout = measure_inline_html_layout(measurer, &inline_runs_by_line, style, max_width);
        let width = if let Some(max_width) = max_width.filter(|w| w.is_finite() && *w > 0.0) {
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
        return TextMetrics {
            width: round_to_1_64_px(width),
            height: layout.line_count.max(explicit_line_boxes) as f64
                * style.font_size.max(1.0)
                * 1.5,
            line_count: layout.line_count.max(explicit_line_boxes),
        };
    }

    let icon_start_wrap = if wrap_mode == WrapMode::HtmlLike {
        max_width
            .filter(|w| w.is_finite() && *w > 0.0)
            .and_then(|w| {
                let mut extra_lines = 0usize;
                let mut wrapped_width: f64 = 0.0;
                let mut has_width_override = false;

                for (idx, line) in lines.iter().enumerate() {
                    if !icon_on_line[idx] || !line.starts_with(char::is_whitespace) {
                        continue;
                    }
                    let text = line.trim();
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
                        let text = line.trim();
                        if text.is_empty()
                            || icon_on_line.get(*idx).copied().unwrap_or(false)
                            || !text.chars().any(|ch| ch.is_whitespace())
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

    let icon_only_extra_lines = if plain.trim().is_empty() {
        0
    } else {
        lines
            .iter()
            .enumerate()
            .filter(|(idx, line)| {
                line.trim().is_empty()
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
            base.height + extra_lines as f64 * style.font_size.max(1.0) * 1.5,
            base.line_count + extra_lines,
        )
    } else {
        (base.height, base.line_count)
    };
    if icon_only_extra_lines > 0 {
        height += icon_only_extra_lines as f64 * style.font_size.max(1.0) * 1.5;
        line_count += icon_only_extra_lines;
    }
    if inline_style_extra_wrap_lines > 0 {
        height += inline_style_extra_wrap_lines as f64 * style.font_size.max(1.0) * 1.5;
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
) -> (String, f64) {
    let mut plain = String::new();
    let mut runs = Vec::new();

    for (word_idx, (word, ty)) in words.iter().enumerate() {
        let visible_word = merman_core::entities::decode_html_entities_to_unicode(word);
        let bold = *ty == MermaidMarkdownWordType::Strong;
        let italic = *ty == MermaidMarkdownWordType::Em;

        if word_idx > 0 {
            plain.push(' ');
            push_inline_text_char(&mut runs, ' ', false, false, false);
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
) -> f64 {
    markdown_word_line_plain_text_and_width_px(measurer, words, style, wrap_mode).1
}

fn split_markdown_word_to_width_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    word: &str,
    ty: MermaidMarkdownWordType,
    max_width_px: f64,
    wrap_mode: WrapMode,
) -> (String, String) {
    if max_width_px <= 0.0 {
        return (word.to_string(), String::new());
    }
    let chars = word.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return (String::new(), String::new());
    }

    let mut split_at = 1usize;
    for idx in 1..=chars.len() {
        let head = chars[..idx].iter().collect::<String>();
        let width =
            measure_markdown_word_line_width_px(measurer, &[(head.clone(), ty)], style, wrap_mode);
        if width.is_finite() && width <= max_width_px {
            split_at = idx;
        } else {
            break;
        }
    }

    let head = chars[..split_at].iter().collect::<String>();
    let tail = chars[split_at..].iter().collect::<String>();
    (head, tail)
}

fn wrap_markdown_word_lines(
    measurer: &dyn TextMeasurer,
    parsed: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
    break_long_words: bool,
) -> Vec<Vec<(String, MermaidMarkdownWordType)>> {
    let Some(max_width_px) = max_width_px.filter(|w| w.is_finite() && *w > 0.0) else {
        return parsed.to_vec();
    };

    let mut out: Vec<Vec<(String, MermaidMarkdownWordType)>> = Vec::new();
    for line in parsed {
        if line.is_empty() {
            out.push(Vec::new());
            continue;
        }

        let mut tokens = std::collections::VecDeque::from(line.clone());
        let mut cur: Vec<(String, MermaidMarkdownWordType)> = Vec::new();

        while let Some((word, ty)) = tokens.pop_front() {
            let mut candidate = cur.clone();
            candidate.push((word.clone(), ty));
            if measure_markdown_word_line_width_px(measurer, &candidate, style, wrap_mode)
                <= max_width_px
            {
                cur = candidate;
                continue;
            }

            if !cur.is_empty() {
                out.push(cur);
                cur = Vec::new();
                tokens.push_front((word, ty));
                continue;
            }

            let single_word_width = measure_markdown_word_line_width_px(
                measurer,
                &[(word.clone(), ty)],
                style,
                wrap_mode,
            );
            if single_word_width <= max_width_px || !break_long_words {
                out.push(vec![(word, ty)]);
                continue;
            }

            let (head, tail) = split_markdown_word_to_width_px(
                measurer,
                style,
                &word,
                ty,
                max_width_px,
                wrap_mode,
            );
            out.push(vec![(head, ty)]);
            if !tail.is_empty() {
                tokens.push_front((tail, ty));
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
    wrap_markdown_word_lines(measurer, &parsed, style, max_width_px, wrap_mode, true)
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
    if markdown.contains("![") {
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
            let has_any_text = !total_text.trim().is_empty();

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
            let line_height = style.font_size.max(1.0) * 1.5;

            let mut width: f64 = 0.0;
            let mut height: f64 = 0.0;
            let mut line_count: usize = 0;

            for p in paragraphs {
                let p_text = p.text.trim().to_string();
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

    let raw_parsed = mermaid_markdown_to_lines(markdown, true);
    let html_paragraph_gap_lines = if wrap_mode == WrapMode::HtmlLike {
        html_markdown_paragraph_gap_lines(markdown)
    } else {
        0
    };
    let parsed = if manually_wrap_words {
        wrap_markdown_word_lines(measurer, &raw_parsed, style, max_width, wrap_mode, true)
    } else {
        raw_parsed.clone()
    };

    let mut plain_lines: Vec<String> = Vec::with_capacity(parsed.len().max(1));
    let mut styled_width_px_by_line: Vec<f64> = Vec::with_capacity(parsed.len().max(1));
    for words in &parsed {
        let (plain, width) =
            markdown_word_line_plain_text_and_width_px(measurer, words, style, wrap_mode);
        plain_lines.push(plain);
        styled_width_px_by_line.push(width);
    }

    let plain = plain_lines.join("\n");
    let plain = plain.trim().to_string();
    let base = if manually_wrap_words {
        measurer.measure_wrapped(&plain, style, None, wrap_mode)
    } else {
        measurer.measure_wrapped(&plain, style, max_width, wrap_mode)
    };

    let max_line_width = styled_width_px_by_line
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);

    // Mermaid's upstream baselines land on a power-of-two lattice:
    // - DOM-measured HTML labels tend to snap to 1/64px.
    // - SVG-label markdown `getBBox()` tends to snap to 1/64px in our upstream baselines.
    //
    // Quantize accordingly so strict-XML layout remains stable.
    let mut width = match wrap_mode {
        WrapMode::HtmlLike => round_to_1_64_px(max_line_width),
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => round_to_1_64_px(max_line_width),
    };
    if wrap_mode == WrapMode::HtmlLike
        && let Some(w) = max_width.filter(|w| w.is_finite() && *w > 0.0)
    {
        let raw_w = raw_parsed
            .iter()
            .map(|words| {
                markdown_word_line_plain_text_and_width_px(measurer, words, style, wrap_mode).1
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
        height: base.height + html_paragraph_gap_lines as f64 * style.font_size.max(1.0) * 1.5,
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
