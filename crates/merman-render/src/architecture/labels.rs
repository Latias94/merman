use std::collections::VecDeque;

use crate::architecture_metrics::{ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, extend_bounds};
use crate::entities::decode_mermaid_entities_for_render_text;
use crate::model::Bounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SvgWordType {
    Normal,
    Strong,
    Em,
}

#[derive(Debug, Clone)]
pub(crate) struct SvgWord {
    pub(crate) content: String,
    pub(crate) word_type: SvgWordType,
}

pub(crate) type SvgLine = Vec<SvgWord>;

pub(crate) fn svg_line_plain_text(line: &[SvgWord]) -> String {
    let mut out = String::new();
    for (idx, w) in line.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&w.content);
    }
    out
}

fn svg_word_style(
    style: &crate::text::TextStyle,
    word_type: SvgWordType,
) -> crate::text::TextStyle {
    let mut word_style = style.clone();
    let (font_weight, font_style) = match word_type {
        SvgWordType::Normal => ("normal", "normal"),
        SvgWordType::Strong => ("bold", "normal"),
        SvgWordType::Em => ("normal", "italic"),
    };
    word_style.font_weight = Some(font_weight.to_string());
    word_style.font_style = Some(font_style.to_string());
    word_style
}

fn svg_word_rendered_text(word: &SvgWord, word_index: usize) -> String {
    if word_index == 0 {
        word.content.clone()
    } else {
        format!(" {}", word.content)
    }
}

pub(crate) fn svg_line_computed_length_px(
    line: &[SvgWord],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> f64 {
    if style.font_size == 0.0 {
        return 0.0;
    }
    line.iter()
        .enumerate()
        .map(|(index, word)| {
            let text = svg_word_rendered_text(word, index);
            let word_style = svg_word_style(style, word.word_type);
            measurer.measure_svg_text_computed_length_px(&text, &word_style)
        })
        .sum()
}

pub(crate) fn svg_service_title_bounds(
    lines: &[SvgLine],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> Option<Bounds> {
    if !svg_lines_have_visible_text(lines, style.font_size) {
        return None;
    }
    let mut bounds = None;
    for (row_index, line) in lines.iter().enumerate() {
        if !svg_lines_have_visible_text(std::slice::from_ref(line), style.font_size) {
            continue;
        }
        let mut runs: Vec<(SvgWordType, String)> = Vec::new();
        for (word_index, word) in line.iter().enumerate() {
            if let Some((word_type, text)) = runs.last_mut()
                && *word_type == word.word_type
            {
                text.push(' ');
                text.push_str(&word.content);
            } else {
                runs.push((word.word_type, svg_word_rendered_text(word, word_index)));
            }
        }

        let mixed_faces = runs.len() > 1;
        let mut advance = 0.0;
        let mut row_bounds = None;
        for (word_type, text) in runs {
            let run_style = svg_word_style(style, word_type);
            // Scalar probes retain each face. Advances position runs; ink widths do not.
            let run_advance = if mixed_faces {
                measurer.measure_svg_text_computed_length_px(&text, &run_style)
            } else {
                0.0
            };
            let center = advance + run_advance / 2.0;
            let (left, right) = measurer.measure_svg_text_bbox_x(&text, &run_style);
            let height = measurer
                .measure_svg_tspan_text_bbox_height_px(&text, &run_style)
                .max(0.0);
            let top = measurer.measure_svg_create_text_middle_bbox_y_offset_px(&text, &run_style);
            extend_bounds(
                &mut row_bounds,
                Bounds {
                    min_x: center - left,
                    min_y: top,
                    max_x: center + right,
                    max_y: top + height,
                },
            );
            advance += run_advance;
        }
        if let Some(mut row) = row_bounds {
            // Each emitted outer tspan restarts at x=0 and advances by 1.1em per row.
            let row_y = row_index as f64 * style.font_size * 1.1;
            row.min_x -= advance / 2.0;
            row.max_x -= advance / 2.0;
            row.min_y += row_y;
            row.max_y += row_y;
            extend_bounds(&mut bounds, row);
        }
    }
    bounds
}

fn svg_line_formatted_bbox_x_px(
    line: &[SvgWord],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> (f64, f64) {
    if line.is_empty() {
        return (0.0, 0.0);
    }
    let text = svg_line_plain_text(line);
    let mut measured_style = style.clone();
    // The current operation transports one style. Homogeneous rows can therefore preserve the
    // exact inner-run style; mixed rows still use the correct outer-tspan primitive with the base
    // style until measurement requests can carry structured runs.
    if let Some(word_type) = line.first().map(|word| word.word_type)
        && line.iter().all(|word| word.word_type == word_type)
    {
        measured_style = svg_word_style(style, word_type);
    }
    measurer.measure_svg_text_bbox_x(&text, &measured_style)
}

pub(crate) fn svg_line_formatted_bbox_width_px(
    line: &[SvgWord],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> f64 {
    let (left, right) = svg_line_formatted_bbox_x_px(line, measurer, style);
    (left + right).max(0.0)
}

pub(crate) fn architecture_svg_text_terminal_bounds(
    lines: &[SvgLine],
    anchor_x: f64,
    anchor_y: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> Bounds {
    if !svg_lines_have_visible_text(lines, style.font_size) {
        return Bounds {
            min_x: anchor_x,
            min_y: anchor_y,
            max_x: anchor_x,
            max_y: anchor_y,
        };
    }
    let mut width = 0.0_f64;
    for line in lines {
        width = width.max(svg_line_formatted_bbox_width_px(line, measurer, style));
    }

    let first_line = lines
        .first()
        .map(|line| svg_line_plain_text(line))
        .unwrap_or_default();
    let first_line_height = measurer
        .measure_svg_tspan_text_bbox_height_px(&first_line, style)
        .max(0.0);
    let extra_lines = lines.len().max(1).saturating_sub(1) as f64;
    let height = first_line_height + style.font_size * extra_lines * 1.1;
    let y_offset = measurer.measure_svg_create_text_bbox_y_offset_px(&first_line, style);
    Bounds {
        min_x: anchor_x,
        min_y: anchor_y + y_offset,
        max_x: anchor_x + width,
        max_y: anchor_y + y_offset + height,
    }
}

pub(crate) fn wrap_svg_words_to_lines(
    text: &str,
    max_width_px: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> Vec<SvgLine> {
    // Mirrors Mermaid `createText(..., { useHtmlLabels: false, width })` behavior for SVG text
    // labels:
    // - tokenization matches `markdownToLines(...)`:
    //   - Markdown parsed (strong/em) into per-word style tags
    //   - inline HTML is kept as an atomic "word" (even if it contains spaces)
    //   - plain text splits on ASCII space and drops empties
    // - long tokens are split by character when they do not fit (via `splitWordToFitWidth`)
    // - lines are greedily constructed and then split further as needed (`splitLineToFitWidth`)
    //
    // References (Mermaid@11.16.0):
    // - `packages/mermaid/src/rendering-util/createText.ts`
    // - `packages/mermaid/src/rendering-util/splitText.ts`
    // - `packages/mermaid/src/rendering-util/handle-markdown-text.ts`
    let max_width_px = if max_width_px.is_finite() && max_width_px > 0.0 {
        max_width_px
    } else {
        ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX
    };

    fn check_fit(
        measurer: &dyn crate::text::TextMeasurer,
        style: &crate::text::TextStyle,
        max_width_px: f64,
        line: &[SvgWord],
    ) -> bool {
        if line.is_empty() {
            return true;
        }
        svg_line_computed_length_px(line, measurer, style) <= max_width_px
    }

    fn split_word_to_fit_width(
        measurer: &dyn crate::text::TextMeasurer,
        style: &crate::text::TextStyle,
        max_width_px: f64,
        word: SvgWord,
    ) -> (SvgWord, SvgWord) {
        if word.content.is_empty() {
            return (
                SvgWord {
                    content: String::new(),
                    word_type: word.word_type,
                },
                SvgWord {
                    content: String::new(),
                    word_type: word.word_type,
                },
            );
        }

        let mut used = String::new();
        let mut remaining: VecDeque<char> = word.content.chars().collect::<VecDeque<_>>();

        while let Some(ch) = remaining.pop_front() {
            let mut candidate = used.clone();
            candidate.push(ch);
            let candidate_word = SvgWord {
                content: candidate.clone(),
                word_type: word.word_type,
            };
            if check_fit(
                measurer,
                style,
                max_width_px,
                std::slice::from_ref(&candidate_word),
            ) {
                used = candidate;
                continue;
            }

            if used.is_empty() {
                // If the first character does not fit, split it anyway (Mermaid behavior).
                used.push(ch);
            } else {
                remaining.push_front(ch);
            }
            break;
        }

        let rest: String = remaining.into_iter().collect();
        (
            SvgWord {
                content: used,
                word_type: word.word_type,
            },
            SvgWord {
                content: rest,
                word_type: word.word_type,
            },
        )
    }

    fn split_line_to_fit_width(
        measurer: &dyn crate::text::TextMeasurer,
        style: &crate::text::TextStyle,
        max_width_px: f64,
        line: SvgLine,
    ) -> Vec<SvgLine> {
        let mut words: VecDeque<SvgWord> = line.into_iter().collect::<VecDeque<_>>();
        let mut lines: Vec<SvgLine> = Vec::new();
        let mut new_line: SvgLine = Vec::new();

        while let Some(next_word) = words.pop_front() {
            let mut line_with_next = new_line.clone();
            line_with_next.push(next_word.clone());

            if check_fit(measurer, style, max_width_px, &line_with_next) {
                new_line = line_with_next;
                continue;
            }

            if !new_line.is_empty() {
                lines.push(new_line);
                new_line = Vec::new();
                words.push_front(next_word);
                continue;
            }

            if !next_word.content.is_empty() {
                let (head, rest) =
                    split_word_to_fit_width(measurer, style, max_width_px, next_word);
                lines.push(vec![head]);
                if !rest.content.is_empty() {
                    words.push_front(rest);
                }
            }
        }

        if !new_line.is_empty() {
            lines.push(new_line);
        }

        lines
    }

    let parsed_lines = parse_svg_word_lines(text);

    let mut out: Vec<SvgLine> = Vec::new();
    for line in parsed_lines {
        if line.is_empty() {
            out.push(Vec::new());
            continue;
        }
        if check_fit(measurer, style, max_width_px, &line) {
            out.push(line);
        } else {
            out.extend(split_line_to_fit_width(measurer, style, max_width_px, line));
        }
    }

    if out.is_empty() {
        vec![Vec::new()]
    } else {
        out
    }
}

fn parse_svg_word_lines(text: &str) -> Vec<SvgLine> {
    fn preprocess_svg_markdown(text: &str) -> String {
        // Mermaid preprocesses markdown before lexing:
        // - replace `<br/>` with `\n`
        // - collapse multiple newlines
        // - dedent leading indentation
        //
        // We reuse our `<br>` normalization and trailing-empty trimming for determinism.
        let joined = crate::text::DeterministicTextMeasurer::normalized_text_lines(text).join("\n");

        // Collapse multiple newlines to one (equivalent to `/\n{2,}/g -> "\n"`).
        let mut collapsed = String::with_capacity(joined.len());
        let mut prev_nl = false;
        for ch in joined.chars() {
            if ch == '\n' {
                if prev_nl {
                    continue;
                }
                prev_nl = true;
                collapsed.push('\n');
            } else {
                prev_nl = false;
                collapsed.push(ch);
            }
        }

        let lines = collapsed
            .split('\n')
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').count())
            .min()
            .unwrap_or(0);
        if min_indent == 0 {
            return lines.join("\n");
        }
        lines
            .into_iter()
            .map(|l| l.chars().skip(min_indent).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    let preprocessed = preprocess_svg_markdown(decoded.as_ref());

    let mut parsed_lines: Vec<SvgLine> = vec![Vec::new()];
    let mut current_line: usize = 0;
    let mut strong_depth: usize = 0;
    let mut em_depth: usize = 0;

    let parser = pulldown_cmark::Parser::new_ext(
        preprocessed.as_str(),
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    );

    for ev in parser {
        match ev {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => {
                strong_depth += 1;
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Emphasis) => {
                em_depth += 1;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {
                strong_depth = strong_depth.saturating_sub(1);
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Emphasis) => {
                em_depth = em_depth.saturating_sub(1);
            }
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                let word_type = if strong_depth > 0 {
                    SvgWordType::Strong
                } else if em_depth > 0 {
                    SvgWordType::Em
                } else {
                    SvgWordType::Normal
                };

                let parts = t.split('\n').collect::<Vec<_>>();
                for (idx, part) in parts.iter().enumerate() {
                    if idx != 0 {
                        current_line += 1;
                        parsed_lines.push(Vec::new());
                    }
                    for word in part.split(' ') {
                        let word = word.replace("&#39;", "'");
                        if !word.is_empty() {
                            parsed_lines[current_line].push(SvgWord {
                                content: word,
                                word_type,
                            });
                        }
                    }
                }
            }
            pulldown_cmark::Event::Html(t) => {
                // Mermaid `markdownToLines` keeps HTML as an atomic word (no whitespace split).
                parsed_lines[current_line].push(SvgWord {
                    content: t.to_string(),
                    word_type: SvgWordType::Normal,
                });
            }
            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                current_line += 1;
                parsed_lines.push(Vec::new());
            }
            _ => {}
        }
    }

    parsed_lines
}

fn svg_lines_have_visible_words(lines: &[SvgLine]) -> bool {
    lines
        .iter()
        .flatten()
        .any(|word| !crate::text::trim_html_collapsible_ascii_whitespace(&word.content).is_empty())
}

pub(crate) fn svg_lines_have_visible_text(lines: &[SvgLine], font_size_px: f64) -> bool {
    font_size_px > 0.0 && svg_lines_have_visible_words(lines)
}
