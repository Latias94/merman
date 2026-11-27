//! Shared measurement of Mermaid Markdown label rows.

use super::inline_html::{
    MermaidMarkdownWordType, SvgTextEntityMode, markdown_word_line_plain_text_and_width_px,
    svg_markdown_word_style,
};
use super::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, round_to_1_64_px,
    trim_html_collapsible_ascii_whitespace,
};
#[cfg(feature = "markdown-labels")]
use super::{ceil_to_1_64_px, markdown::mermaid_markdown_to_lines};

fn measure_markdown_word_line_width_px(
    measurer: &dyn TextMeasurer,
    words: &[(String, MermaidMarkdownWordType)],
    style: &TextStyle,
    wrap_mode: WrapMode,
    entity_mode: SvgTextEntityMode,
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
    entity_mode: SvgTextEntityMode,
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

pub fn wrap_markdown_word_lines(
    measurer: &dyn TextMeasurer,
    parsed: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
    break_long_words: bool,
    entity_mode: SvgTextEntityMode,
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
pub fn mermaid_markdown_to_wrapped_word_lines(
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
        SvgTextEntityMode::CreateTextSource,
    )
}

/// Measures the SVG word rows emitted by `createText`, without HTML-only image sizing.
pub fn measure_svg_markdown_word_lines(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    entity_mode: SvgTextEntityMode,
) -> TextMetrics {
    measure_svg_markdown_word_lines_with_origin(measurer, lines, style, entity_mode).0
}

/// Retains the origin found during the same word measurements, without replaying host requests.
pub fn measure_svg_markdown_word_lines_with_origin(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, MermaidMarkdownWordType)>],
    style: &TextStyle,
    entity_mode: SvgTextEntityMode,
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
            SvgTextEntityMode::CreateTextSource,
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
            SvgTextEntityMode::CreateTextSource,
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
            SvgTextEntityMode::CreateTextSource,
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
                    SvgTextEntityMode::CreateTextSource,
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
pub fn measure_wrapped_markdown_with_inline_styles(
    measurer: &dyn TextMeasurer,
    markdown: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_markdown_with_inline_styles_impl(measurer, markdown, style, max_width, wrap_mode, true)
}
