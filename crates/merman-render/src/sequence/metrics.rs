use super::constants::{sequence_text_dimensions_height_px, sequence_text_line_step_px};
use crate::math::{DelimitedMathLine, MathRenderer, parse_delimited_math_line};
use crate::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, round_to_1_64_px, split_html_br_lines,
};
use merman_core::MermaidConfig;

struct SequenceWrapTextMeasurer<'a>(&'a dyn TextMeasurer);

impl TextMeasurer for SequenceWrapTextMeasurer<'_> {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0.measure(text, style)
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.0.measure_mermaid_calculate_text_width_px(text, style)
    }
}

pub(crate) fn wrap_sequence_label_like_mermaid_lines(
    label: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: f64,
) -> Vec<String> {
    let sequence_measurer = SequenceWrapTextMeasurer(measurer);
    crate::text::wrap_label_like_mermaid_lines(label, &sequence_measurer, style, max_width_px)
}

#[derive(Clone, Copy)]
enum SequenceSvgTextHeightMode {
    CalculatedDimensions,
    DrawnBBox,
}

pub(super) fn measure_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    measure_svg_like_with_html_br_mode(
        measurer,
        text,
        style,
        SequenceSvgTextHeightMode::CalculatedDimensions,
    )
}

pub(super) fn measure_drawn_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    measure_svg_like_with_html_br_mode(measurer, text, style, SequenceSvgTextHeightMode::DrawnBBox)
}

fn measure_svg_like_with_html_br_mode(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    height_mode: SequenceSvgTextHeightMode,
) -> (f64, f64) {
    let lines = split_html_br_lines(text);
    let calculated_line_height = sequence_text_dimensions_height_px(style.font_size);
    let drawn_line_height = sequence_text_line_step_px(style.font_size).floor().max(1.0);
    let fallback_line_height = match height_mode {
        SequenceSvgTextHeightMode::CalculatedDimensions => calculated_line_height,
        SequenceSvgTextHeightMode::DrawnBBox => drawn_line_height,
    };
    let normalize_line_height = |height: f64| match height_mode {
        // Mermaid's temporary `calculateTextDimensions` text can be shorter than the final drawn
        // `<text>` bbox. Keep smaller host measurements, but cap generic vendored bbox height at
        // the source-backed Chrome curve used by Sequence spacing.
        SequenceSvgTextHeightMode::CalculatedDimensions => {
            height.max(0.0).min(calculated_line_height)
        }
        // `drawNote` sums the final text nodes' `getBBox().height`. Cap generic vendored metrics at
        // the Chrome 131 drawn-text curve while preserving its taller 16px bbox (19px rather than
        // the 17px temporary-dimensions height).
        SequenceSvgTextHeightMode::DrawnBBox => height.max(0.0).min(drawn_line_height),
    };
    if lines.len() <= 1 {
        // Mermaid's `calculateTextDimensions` draws one `<text>/<tspan>` run per line, rounds
        // that bbox width, and keeps height from the same single-run bbox path.
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun);
        let width = match height_mode {
            SequenceSvgTextHeightMode::CalculatedDimensions => {
                measurer.measure_mermaid_calculate_text_width_px(text, style)
            }
            SequenceSvgTextHeightMode::DrawnBBox => metrics.width,
        };
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            fallback_line_height
        };
        return (width.round().max(0.0), normalize_line_height(h));
    }
    let mut max_w: f64 = 0.0;
    let mut line_h: f64 = 0.0;
    for line in &lines {
        let metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLikeSingleRun);
        let width = match height_mode {
            SequenceSvgTextHeightMode::CalculatedDimensions => {
                measurer.measure_mermaid_calculate_text_width_px(line, style)
            }
            SequenceSvgTextHeightMode::DrawnBBox => metrics.width,
        };
        max_w = max_w.max(width.round().max(0.0));
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            fallback_line_height
        };
        line_h = line_h.max(normalize_line_height(h));
    }
    (
        max_w,
        (line_h * lines.len() as f64).max(fallback_line_height),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMathHeightMode {
    Actor,
    Bound,
    Draw,
}

fn sequence_math_chunks(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut search_from = 0usize;
    while let Some(start_rel) = text[search_from..].find("$$") {
        let start = search_from + start_rel;
        let content_start = start + 2;
        let Some(end_rel) = text[content_start..].find("$$") else {
            break;
        };
        let end = content_start + end_rel + 2;
        chunks.push(&text[start..end]);
        search_from = end;
    }
    chunks
}

fn measure_plain_sequence_fragment(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> TextMetrics {
    measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun)
}

fn measure_sequence_mixed_math_line(
    measurer: &dyn TextMeasurer,
    parsed: DelimitedMathLine<'_>,
    style: &TextStyle,
    config: &MermaidConfig,
    math_renderer: &(dyn MathRenderer + Send + Sync),
) -> Option<(f64, f64)> {
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;

    for fragment in parsed.fragments {
        if !fragment.leading_text.is_empty() {
            let metrics = measure_plain_sequence_fragment(measurer, fragment.leading_text, style);
            width += metrics.width.max(0.0);
            height = height.max(metrics.height.max(0.0));
        }

        let math_metrics = math_renderer
            .measure_sequence_html_label(fragment.delimited, config)
            .or_else(|| {
                math_renderer.measure_html_label(
                    fragment.delimited,
                    config,
                    style,
                    Some(10_000.0),
                    WrapMode::HtmlLike,
                )
            })?;
        width += math_metrics.width.max(0.0);
        height = height.max(math_metrics.height.max(0.0));
    }

    if !parsed.trailing_text.is_empty() {
        let metrics = measure_plain_sequence_fragment(measurer, parsed.trailing_text, style);
        width += metrics.width.max(0.0);
        height = height.max(metrics.height.max(0.0));
    }

    Some((width, height.max(1.0)))
}

fn measure_sequence_mixed_math_label(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    config: &MermaidConfig,
    math_renderer: &(dyn MathRenderer + Send + Sync),
) -> Option<TextMetrics> {
    let mut saw_math = false;
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    let mut line_count = 0usize;

    for line in split_html_br_lines(text) {
        line_count += 1;
        let (line_width, line_height) = if let Some(parsed) = parse_delimited_math_line(line) {
            saw_math = true;
            measure_sequence_mixed_math_line(measurer, parsed, style, config, math_renderer)?
        } else {
            let (w, h) = measure_svg_like_with_html_br(measurer, line, style);
            (w.max(0.0), h.max(0.0))
        };
        width = width.max(line_width);
        height += line_height;
    }

    saw_math.then_some(TextMetrics {
        width: round_to_1_64_px(width),
        height: round_to_1_64_px(height.max(1.0)),
        line_count: line_count.max(1),
    })
}

fn sequence_math_height_px(
    text: &str,
    style: &TextStyle,
    config: &MermaidConfig,
    math_renderer: &(dyn MathRenderer + Send + Sync),
    mode: SequenceMathHeightMode,
    full_metrics: &TextMetrics,
) -> f64 {
    match mode {
        SequenceMathHeightMode::Actor => full_metrics.height.round().max(1.0),
        SequenceMathHeightMode::Bound | SequenceMathHeightMode::Draw => {
            let line_step = sequence_text_line_step_px(style.font_size).round().max(1.0);
            let base = if mode == SequenceMathHeightMode::Draw {
                line_step
            } else {
                (line_step - 1.0)
                    .max(sequence_text_dimensions_height_px(style.font_size))
                    .max(1.0)
            };
            let math_h = sequence_math_chunks(text)
                .into_iter()
                .filter_map(|chunk| math_renderer.measure_sequence_html_label(chunk, config))
                .map(|m| m.height.round() + 2.0)
                .fold(base, f64::max);
            math_h.round().max(1.0)
        }
    }
}

pub(crate) fn measure_sequence_math_label(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    config: &MermaidConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    mode: SequenceMathHeightMode,
) -> Option<(f64, f64)> {
    if !text.contains("$$") {
        return None;
    }
    let renderer = math_renderer?;
    let full_metrics = renderer
        .measure_sequence_html_label(text, config)
        .or_else(|| measure_sequence_mixed_math_label(measurer, text, style, config, renderer))
        .or_else(|| {
            renderer.measure_html_label(text, config, style, Some(10_000.0), WrapMode::HtmlLike)
        })?;
    let height = sequence_math_height_px(text, style, config, renderer, mode, &full_metrics);
    Some((full_metrics.width.round().max(1.0), height))
}

pub(super) fn measure_sequence_label_for_layout(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    config: &MermaidConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    mode: SequenceMathHeightMode,
) -> (f64, f64) {
    measure_sequence_math_label(measurer, text, style, config, math_renderer, mode)
        .unwrap_or_else(|| measure_svg_like_with_html_br(measurer, text, style))
}

#[cfg(test)]
mod tests {
    use crate::text::TextMeasurer;

    #[cfg(feature = "ratex-math")]
    #[test]
    fn sequence_math_measurement_handles_multiple_formulas_on_one_line() {
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
        let renderer = crate::math::RatexMathRenderer;
        let config = merman_core::MermaidConfig::default();
        let style = crate::text::TextStyle::default();

        let (width, height) = super::measure_sequence_math_label(
            &measurer,
            "a $$x$$ b $$y$$ c",
            &style,
            &config,
            Some(&renderer),
            super::SequenceMathHeightMode::Actor,
        )
        .expect("each same-line math fragment should contribute to Sequence measurement");

        assert!(width > 0.0, "expected positive measured width");
        assert!(height > 0.0, "expected positive measured height");
    }

    #[cfg(feature = "ratex-math")]
    #[test]
    fn sequence_math_measurement_ignores_unclosed_delimiters_on_plain_lines() {
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
        let renderer = crate::math::RatexMathRenderer;
        let config = merman_core::MermaidConfig::default();
        let style = crate::text::TextStyle::default();

        assert!(
            super::measure_sequence_math_label(
                &measurer,
                "literal $$",
                &style,
                &config,
                Some(&renderer),
                super::SequenceMathHeightMode::Actor,
            )
            .is_none(),
            "an unmatched delimiter alone must use plain Sequence measurement"
        );

        let metrics = super::measure_sequence_math_label(
            &measurer,
            "valid $$x$$<br>literal $$",
            &style,
            &config,
            Some(&renderer),
            super::SequenceMathHeightMode::Actor,
        );

        assert!(
            metrics.is_some(),
            "an unmatched delimiter on a plain line must not discard complete formulas"
        );
    }

    #[test]
    fn sequence_default_message_metrics_use_current_sequence_svg_bbox_facts() {
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
        let style = crate::text::TextStyle {
            // Mermaid's default global font family includes the trailing semicolon, and Sequence
            // copies that value into messageFontFamily before calculateTextDimensions runs.
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
            font_size: 16.0,
            font_weight: None,
        };
        let cases = [
            "Hello Bob, how are you?",
            "Hello Bob, how are - you?",
            "Hello Alice, I'm fine and you?",
            "Hello Alice, please meet Carol?",
            "Feeling fresh like a daisy",
            "Fine, thank you. And you?",
            "Hello Charley, how are you?",
            "Hello John, how are you?",
            "Did you want to go to the game tonight?",
            "How about you John?",
            "bidirectional_dotted",
            "Alice-in-Wonderland",
        ];

        for text in cases {
            let (measured_width, measured_height) =
                super::measure_svg_like_with_html_br(&measurer, text, &style);
            let (_, drawn_height) =
                super::measure_drawn_svg_like_with_html_br(&measurer, text, &style);
            let expected_width =
                TextMeasurer::measure_svg_simple_text_bbox_width_px(&measurer, text, &style)
                    .round()
                    .max(0.0);

            assert_eq!(
                measured_width, expected_width,
                "expected Sequence message width to stay aligned with the current single-run SVG bbox fact for {text:?}"
            );
            assert_eq!(
                measured_height, 17.0,
                "expected Sequence message height to use the Chrome 131 single-run SVG bbox fact for {text:?}"
            );
            assert_eq!(
                drawn_height, 19.0,
                "expected Sequence drawn text to retain its getBBox height for {text:?}"
            );
        }
    }

    #[test]
    fn sequence_small_font_drawn_height_uses_chrome_bbox_curve() {
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
        let style = crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
            font_size: 10.0,
            font_weight: None,
        };

        let (_, calculated_height) =
            super::measure_svg_like_with_html_br(&measurer, "Note", &style);
        let (_, drawn_height) =
            super::measure_drawn_svg_like_with_html_br(&measurer, "Note", &style);

        assert_eq!(calculated_height, 11.0);
        assert_eq!(drawn_height, 11.0);
    }
}
