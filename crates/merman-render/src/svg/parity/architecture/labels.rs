use std::collections::VecDeque;

#[cfg(merman_internal_theme_acceptance)]
use sha2::{Digest as _, Sha256};

use crate::architecture_metrics::ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX;
use crate::model::Bounds;

use super::super::{
    SvgOutput, decode_mermaid_entities_for_render_text, escape_attr_into, escape_xml_into, fmt,
};
use super::geometry::extend_bounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SvgWordType {
    Normal,
    Strong,
    Em,
}

#[derive(Debug, Clone)]
pub(super) struct SvgWord {
    content: String,
    word_type: SvgWordType,
}

pub(super) type SvgLine = Vec<SvgWord>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArchitectureTextWriterFacts {
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) fragment_digest: [u8; 32],
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) run_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ArchitectureTextWriterEmission {
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) bounds: Option<Bounds>,
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) facts: ArchitectureTextWriterFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchitectureTextAnchor {
    Start,
    #[cfg(merman_internal_theme_acceptance)]
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchitectureTextBaseline {
    Start,
    #[cfg(merman_internal_theme_acceptance)]
    Middle,
}

pub(super) fn svg_line_plain_text(line: &[SvgWord]) -> String {
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

fn svg_line_computed_length_px(
    line: &[SvgWord],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> f64 {
    line.iter()
        .enumerate()
        .map(|(index, word)| {
            let text = svg_word_rendered_text(word, index);
            let word_style = svg_word_style(style, word.word_type);
            measurer.measure_svg_text_computed_length_px(&text, &word_style)
        })
        .sum()
}

pub(super) fn svg_service_title_bounds(
    lines: &[SvgLine],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> Option<Bounds> {
    let mut bounds = None;
    for (row_index, line) in lines.iter().enumerate() {
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
            let row_y = row_index as f64 * style.font_size.max(1.0) * 1.1;
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

pub(super) fn svg_line_formatted_bbox_width_px(
    line: &[SvgWord],
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> f64 {
    let (left, right) = svg_line_formatted_bbox_x_px(line, measurer, style);
    (left + right).max(0.0)
}

pub(super) fn architecture_svg_text_terminal_bounds(
    lines: &[SvgLine],
    anchor_x: f64,
    anchor_y: f64,
    anchor: ArchitectureTextAnchor,
    baseline: ArchitectureTextBaseline,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) -> Bounds {
    #[cfg(merman_internal_theme_acceptance)]
    if matches!(anchor, ArchitectureTextAnchor::Middle)
        && matches!(baseline, ArchitectureTextBaseline::Middle)
    {
        let label = svg_service_title_bounds(lines, measurer, style).unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        });
        return Bounds {
            min_x: anchor_x + label.min_x,
            min_y: anchor_y + label.min_y,
            max_x: anchor_x + label.max_x,
            max_y: anchor_y + label.max_y,
        };
    }
    #[cfg(merman_internal_theme_acceptance)]
    let mut left = 0.0_f64;
    #[cfg(merman_internal_theme_acceptance)]
    let mut right = 0.0_f64;
    let mut width = 0.0_f64;
    for line in lines {
        let (line_left, line_right) = svg_line_formatted_bbox_x_px(line, measurer, style);
        #[cfg(merman_internal_theme_acceptance)]
        {
            left = left.max(line_left.max(0.0));
            right = right.max(line_right.max(0.0));
        }
        width = width.max((line_left + line_right).max(0.0));
    }

    let first_line = lines
        .first()
        .map(|line| svg_line_plain_text(line))
        .unwrap_or_default();
    let first_line_height = measurer
        .measure_svg_tspan_text_bbox_height_px(&first_line, style)
        .max(0.0);
    let extra_lines = lines.len().max(1).saturating_sub(1) as f64;
    let height = first_line_height + style.font_size.max(1.0) * extra_lines * 1.1;
    let y_offset = match baseline {
        ArchitectureTextBaseline::Start => {
            measurer.measure_svg_create_text_bbox_y_offset_px(&first_line, style)
        }
        #[cfg(merman_internal_theme_acceptance)]
        ArchitectureTextBaseline::Middle => {
            measurer.measure_svg_create_text_middle_bbox_y_offset_px(&first_line, style)
        }
    };
    let (min_x, max_x) = match anchor {
        ArchitectureTextAnchor::Start => (anchor_x, anchor_x + width),
        #[cfg(merman_internal_theme_acceptance)]
        ArchitectureTextAnchor::Middle => (anchor_x - left, anchor_x + right),
    };
    Bounds {
        min_x,
        min_y: anchor_y + y_offset,
        max_x,
        max_y: anchor_y + y_offset + height,
    }
}

pub(super) fn wrap_svg_words_to_lines(
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

pub(super) fn write_svg_text_lines(
    out: &mut impl SvgOutput,
    lines: &[SvgLine],
    inline_style: Option<&str>,
) -> ArchitectureTextWriterFacts {
    #[cfg(merman_internal_theme_acceptance)]
    let start = out.len();
    out.push_str(r#"<text y="-10.1""#);
    out.push_str(r#" style=""#);
    if let Some(style) = inline_style {
        escape_attr_into(out, style);
    }
    out.push_str(r#"">"#);
    if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
        out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        #[cfg(merman_internal_theme_acceptance)]
        return architecture_text_writer_facts(out, start, 1);
        #[cfg(not(merman_internal_theme_acceptance))]
        return ArchitectureTextWriterFacts {};
    }
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em">"#);
        } else if idx == 1 {
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan row" x="0" y="1em" dy="1.1em">"#
            );
        } else {
            let y_em = 1.0 + (idx as f64 - 1.0) * 1.1;
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan row" x="0" y="{:.1}em" dy="1.1em">"#,
                y_em
            );
        }
        for (word_idx, word) in line.iter().enumerate() {
            let (font_style, font_weight) = match word.word_type {
                SvgWordType::Normal => ("normal", "normal"),
                SvgWordType::Strong => ("normal", "bold"),
                SvgWordType::Em => ("italic", "normal"),
            };
            let _ = write!(
                out,
                r#"<tspan font-style="{font_style}" class="text-inner-tspan" font-weight="{font_weight}">"#,
            );
            if word_idx == 0 {
                escape_xml_into(out, word.content.as_str());
            } else {
                out.push(' ');
                escape_xml_into(out, word.content.as_str());
            }
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
    #[cfg(merman_internal_theme_acceptance)]
    {
        architecture_text_writer_facts(
            out,
            start,
            lines
                .len()
                .saturating_add(lines.iter().map(Vec::len).sum::<usize>()),
        )
    }
    #[cfg(not(merman_internal_theme_acceptance))]
    {
        ArchitectureTextWriterFacts {}
    }
}

#[cfg(merman_internal_theme_acceptance)]
fn architecture_text_writer_facts(
    out: &impl SvgOutput,
    start: usize,
    run_count: usize,
) -> ArchitectureTextWriterFacts {
    let fragment = out.as_str().get(start..).unwrap_or_default();
    ArchitectureTextWriterFacts {
        fragment_digest: Sha256::digest(fragment.as_bytes()).into(),
        run_count,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(super) fn write_architecture_service_title(
    out: &mut impl SvgOutput,
    title: &str,
    #[cfg(merman_internal_theme_acceptance)] service_x: f64,
    #[cfg(merman_internal_theme_acceptance)] service_y: f64,
    icon_size_px: f64,
    title_width_px: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    inline_style: Option<&str>,
    #[cfg(merman_internal_theme_acceptance)] observe_bounds: bool,
) -> ArchitectureTextWriterEmission {
    let lines = wrap_svg_words_to_lines(title, title_width_px, measurer, style);
    #[cfg(merman_internal_theme_acceptance)]
    let terminal_bounds = observe_bounds.then(|| {
        architecture_svg_text_terminal_bounds(
            &lines,
            service_x + icon_size_px / 2.0,
            service_y + icon_size_px,
            ArchitectureTextAnchor::Middle,
            ArchitectureTextBaseline::Middle,
            measurer,
            style,
        )
    });
    let _ = write!(
        out,
        r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate({x}, {y})"><g><rect class="background" style="stroke: none"/>"#,
        x = fmt(icon_size_px / 2.0),
        y = fmt(icon_size_px)
    );
    #[cfg(merman_internal_theme_acceptance)]
    let facts = write_svg_text_lines(out, &lines, inline_style);
    #[cfg(not(merman_internal_theme_acceptance))]
    let _ = write_svg_text_lines(out, &lines, inline_style);
    out.push_str("</g></g>");
    ArchitectureTextWriterEmission {
        #[cfg(merman_internal_theme_acceptance)]
        bounds: terminal_bounds,
        #[cfg(merman_internal_theme_acceptance)]
        facts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::cell::RefCell;

    type MeasurementCall = (String, Option<String>, Option<String>);

    #[derive(Default)]
    struct ExactPrimitiveProbe {
        computed_calls: RefCell<Vec<MeasurementCall>>,
        bbox_calls: RefCell<Vec<MeasurementCall>>,
    }

    impl TextMeasurer for ExactPrimitiveProbe {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("Architecture createText must not use generic measurement")
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.computed_calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
            ));
            let scale = if style.font_weight.as_deref() == Some("bold") {
                20.0
            } else if style.font_style.as_deref() == Some("italic") {
                15.0
            } else {
                10.0
            };
            text.chars().count() as f64 * scale
        }

        fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            self.bbox_calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
            ));
            (100.0, 111.0)
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            panic!("Architecture formatted rows must not use the single-tspan operation")
        }
    }

    fn text_style() -> crate::text::TextStyle {
        crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }

    #[test]
    fn styled_svg_wrap_uses_computed_length_with_inner_run_styles() {
        let measurer = ExactPrimitiveProbe::default();
        let lines = wrap_svg_words_to_lines("a **b** *c*", 80.0, &measurer, &text_style());

        assert_eq!(lines.len(), 1);
        let calls = measurer.computed_calls.borrow();
        assert!(calls.iter().any(|call| {
            call == &(
                "a".to_string(),
                Some("normal".to_string()),
                Some("normal".to_string()),
            )
        }));
        assert!(calls.iter().any(|call| {
            call == &(
                " b".to_string(),
                Some("bold".to_string()),
                Some("normal".to_string()),
            )
        }));
        assert!(calls.iter().any(|call| {
            call == &(
                " c".to_string(),
                Some("normal".to_string()),
                Some("italic".to_string()),
            )
        }));
    }

    #[test]
    fn architecture_plain_service_title_matches_create_text_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = text_style();
        let mut out = String::new();

        let emission = write_architecture_service_title(
            &mut out,
            "s1",
            #[cfg(merman_internal_theme_acceptance)]
            0.0,
            #[cfg(merman_internal_theme_acceptance)]
            0.0,
            80.0,
            120.0,
            &measurer,
            &style,
            None,
            #[cfg(merman_internal_theme_acceptance)]
            true,
        );

        #[cfg(merman_internal_theme_acceptance)]
        {
            let bounds = emission.bounds.expect("requested service region");
            assert!(bounds.max_x > bounds.min_x && bounds.max_y > bounds.min_y);
        }
        #[cfg(not(merman_internal_theme_acceptance))]
        let _ = emission;
        assert_eq!(
            out,
            r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate(40, 80)"><g><rect class="background" style="stroke: none"/><text y="-10.1" style=""><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">s1</tspan></tspan></text></g></g>"#
        );
    }

    #[test]
    #[cfg(merman_internal_theme_acceptance)]
    fn optional_service_region_preserves_the_emitted_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let mut observed = String::new();
        let mut unobserved = String::new();
        let observed_emission = write_architecture_service_title(
            &mut observed,
            "Service **title**",
            0.0,
            0.0,
            80.0,
            120.0,
            &measurer,
            &text_style(),
            None,
            true,
        );
        let unobserved_emission = write_architecture_service_title(
            &mut unobserved,
            "Service **title**",
            0.0,
            0.0,
            80.0,
            120.0,
            &measurer,
            &text_style(),
            None,
            false,
        );
        assert!(observed_emission.bounds.is_some());
        assert!(unobserved_emission.bounds.is_none());
        assert_eq!(observed, unobserved);
    }

    #[test]
    fn architecture_plain_words_match_single_line_create_text_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = text_style();
        let mut out = String::new();

        write_svg_text_lines(
            &mut out,
            &wrap_svg_words_to_lines("Service Farm", 120.0, &measurer, &style),
            None,
        );

        assert_eq!(
            out,
            r#"<text y="-10.1" style=""><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">Service</tspan><tspan font-style="normal" class="text-inner-tspan" font-weight="normal"> Farm</tspan></tspan></text>"#
        );
    }

    #[test]
    fn emitted_line_bbox_uses_formatted_operation_and_homogeneous_run_style() {
        let measurer = ExactPrimitiveProbe::default();
        let style = text_style();
        let lines = wrap_svg_words_to_lines("**Bold words**", 1_000.0, &measurer, &style);
        let line = lines.first().expect("one bold line");

        assert_eq!(svg_line_computed_length_px(line, &measurer, &style), 200.0);
        assert_eq!(
            svg_line_formatted_bbox_width_px(line, &measurer, &style),
            211.0
        );
        assert_eq!(
            measurer.bbox_calls.borrow().as_slice(),
            &[(
                "Bold words".to_string(),
                Some("bold".to_string()),
                Some("normal".to_string()),
            )]
        );
    }
}
