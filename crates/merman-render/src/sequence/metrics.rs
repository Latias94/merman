use super::constants::{sequence_text_dimensions_height_px, sequence_text_line_step_px};
use crate::math::MathRenderer;
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode, split_html_br_lines};
use merman_core::MermaidConfig;

pub(super) fn measure_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    let lines = split_html_br_lines(text);
    let default_line_height = (style.font_size.max(1.0) * 1.1).max(1.0);
    let calculated_line_height = sequence_text_dimensions_height_px(style.font_size);
    let normalize_line_height = |height: f64| {
        let h = height.max(0.0);
        if style.font_size < 16.0 {
            h.min(calculated_line_height)
        } else {
            h
        }
    };
    if lines.len() <= 1 {
        // Mermaid's `calculateTextDimensions` draws one `<text>/<tspan>` run per line, rounds
        // that bbox width, and keeps height from the same single-run bbox path.
        let width_metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun);
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun);
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            default_line_height
        };
        return (
            width_metrics.width.round().max(0.0),
            normalize_line_height(h),
        );
    }
    let mut max_w: f64 = 0.0;
    let mut line_h: f64 = 0.0;
    for line in &lines {
        let width_metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLikeSingleRun);
        max_w = max_w.max(width_metrics.width.round().max(0.0));
        let metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLikeSingleRun);
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            default_line_height
        };
        line_h = line_h.max(normalize_line_height(h));
    }
    (
        max_w,
        (line_h * lines.len() as f64).max(default_line_height),
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
    measure_sequence_math_label(text, style, config, math_renderer, mode)
        .unwrap_or_else(|| measure_svg_like_with_html_br(measurer, text, style))
}

#[cfg(test)]
mod tests {
    #[test]
    fn sequence_default_message_widths_match_mermaid_default_font_family() {
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
        let style = crate::text::TextStyle {
            // Mermaid's default global font family includes the trailing semicolon, and Sequence
            // copies that value into messageFontFamily before calculateTextDimensions runs.
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
            font_size: 16.0,
            font_weight: None,
        };

        let (hello_bob_w, _) =
            super::measure_svg_like_with_html_br(&measurer, "Hello Bob, how are you?", &style);
        assert_eq!(hello_bob_w, 160.0);

        let (hello_alice_w, _) = super::measure_svg_like_with_html_br(
            &measurer,
            "Hello Alice, I'm fine and you?",
            &style,
        );
        assert_eq!(hello_alice_w, 193.0);

        let (hello_alice_carol_w, _) = super::measure_svg_like_with_html_br(
            &measurer,
            "Hello Alice, please meet Carol?",
            &style,
        );
        assert_eq!(hello_alice_carol_w, 203.0);

        let (hello_john_w, _) =
            super::measure_svg_like_with_html_br(&measurer, "Hello John, how are you?", &style);
        assert_eq!(hello_john_w, 164.0);

        let (bidirectional_dotted_w, _) =
            super::measure_svg_like_with_html_br(&measurer, "bidirectional_dotted", &style);
        assert_eq!(bidirectional_dotted_w, 130.0);
    }
}
