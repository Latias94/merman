use super::constants::{sequence_text_dimensions_height_px, sequence_text_line_step_px};
use crate::math::{DelimitedMathLine, MathRenderer, parse_delimited_math_line};
use crate::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, measure_mermaid_text_dimensions,
    split_html_br_lines,
};
use merman_core::MermaidConfig;

struct SequenceWrapTextMeasurer<'a>(&'a dyn TextMeasurer);

impl TextMeasurer for SequenceWrapTextMeasurer<'_> {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0.measure(text, style)
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        measure_svg_like_with_html_br(self.0, text, style).0
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

fn sequence_drawn_text_style(style: &TextStyle) -> TextStyle {
    let mut effective = style.clone();
    if let Some(font_family) = effective.font_family.as_mut()
        && font_family.trim_end().ends_with(';')
    {
        // The same rejected inline assignment on final Sequence text falls back to the diagram
        // root, whose stylesheet contains the configured family as a valid declaration value.
        *font_family = font_family
            .trim_end()
            .trim_end_matches(';')
            .trim_end()
            .to_string();
    }
    effective
}

pub(super) fn measure_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    let dimensions = measure_mermaid_text_dimensions(measurer, text, style);
    (dimensions.width as f64, dimensions.height as f64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceDrawnTextNode {
    Direct,
    Tspan,
}

pub(super) fn measure_drawn_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    node: SequenceDrawnTextNode,
) -> (f64, f64) {
    let effective_style = sequence_drawn_text_style(style);
    let lines = split_html_br_lines(text);
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    for line in lines {
        let measured_line = if line.is_empty() { "\u{200b}" } else { line };
        let line_width = match node {
            SequenceDrawnTextNode::Direct => {
                measurer.measure_svg_raw_text_bbox_width_px(measured_line, &effective_style)
            }
            SequenceDrawnTextNode::Tspan => {
                measurer.measure_svg_tspan_text_bbox_width_px(measured_line, &effective_style)
            }
        }
        .max(0.0);
        let line_height = match node {
            SequenceDrawnTextNode::Direct => {
                measurer.measure_svg_simple_text_bbox_height_px(measured_line, &effective_style)
            }
            SequenceDrawnTextNode::Tspan => {
                measurer.measure_svg_tspan_text_bbox_height_px(measured_line, &effective_style)
            }
        }
        .max(0.0);
        width = width.max(line_width);
        height += line_height;
    }

    (width, height)
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
        width,
        height: height.max(1.0),
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
    use crate::math::MathRenderer;
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::cell::RefCell;

    #[derive(Debug)]
    struct PreciseMathRenderer;

    impl MathRenderer for PreciseMathRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            text.contains("$$").then(|| text.to_string())
        }

        fn measure_sequence_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<TextMetrics> {
            (text.starts_with("$$") && text.ends_with("$$")).then_some(TextMetrics {
                width: 10.008,
                height: 20.008,
                line_count: 1,
            })
        }
    }

    struct PreciseTextMeasurer;

    impl TextMeasurer for PreciseTextMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 1.001,
                height: 2.002,
                line_count: 1,
            }
        }
    }

    #[derive(Default)]
    struct OperationProbe {
        calls: RefCell<Vec<(String, String, String)>>,
    }

    impl OperationProbe {
        fn record(&self, operation: &str, text: &str, style: &TextStyle) {
            self.calls.borrow_mut().push((
                operation.to_string(),
                text.to_string(),
                style.font_family.clone().unwrap_or_default(),
            ));
        }
    }

    impl TextMeasurer for OperationProbe {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("Sequence text must use the DOM-shape operation, not generic measure")
        }

        fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.record("simple-width", text, style);
            match style.font_family.as_deref() {
                Some("sans-serif") => 60.0,
                Some("serif") => 70.0,
                _ => 50.0,
            }
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            text: &str,
            style: &TextStyle,
        ) -> TextMetrics {
            self.record("mermaid-dimensions", text, style);
            let width = match style.font_family.as_deref() {
                Some("sans-serif") => 60.0,
                Some(family) if family.trim_end().ends_with(';') => 70.0,
                _ => 50.0,
            };
            let height = match style.font_family.as_deref() {
                Some("sans-serif") => 16.0,
                Some(family) if family.trim_end().ends_with(';') => 17.0,
                _ => 19.0,
            };
            TextMetrics {
                width,
                height,
                line_count: 1,
            }
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.record("raw-width", text, style);
            101.0
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.record("tspan-width", text, style);
            202.0
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.record("tspan-height", text, style);
            23.0
        }

        fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.record("simple-height", text, style);
            match style.font_family.as_deref() {
                Some("sans-serif") => 16.0,
                Some("serif") => 17.0,
                _ => 19.0,
            }
        }
    }

    fn default_sequence_style() -> TextStyle {
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
            font_size: 16.0,
            font_weight: Some("400".to_string()),
            font_style: None,
        }
    }

    #[test]
    fn sequence_mixed_math_metrics_preserve_fragment_precision() {
        let config = merman_core::MermaidConfig::default();
        let style = TextStyle::default();
        let metrics = super::measure_sequence_mixed_math_label(
            &PreciseTextMeasurer,
            "a$$x$$b",
            &style,
            &config,
            &PreciseMathRenderer,
        )
        .unwrap();

        assert!((metrics.width - 12.01).abs() < 1e-12, "{metrics:?}");
        assert!((metrics.height - 20.008).abs() < 1e-12, "{metrics:?}");
    }

    #[cfg(feature = "math")]
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

    #[cfg(feature = "math")]
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
    fn sequence_calculated_dimensions_preserve_cssom_input_for_the_exact_operation() {
        let measurer = OperationProbe::default();
        let style = default_sequence_style();

        let dimensions =
            super::measure_svg_like_with_html_br(&measurer, "alpha<br><br>beta", &style);

        assert_eq!(dimensions, (70.0, 51.0));
        let calls = measurer.calls.borrow();
        assert_eq!(calls.len(), 6);
        assert!(
            calls
                .iter()
                .all(|(operation, _, _)| operation == "mermaid-dimensions")
        );
        assert!(calls.iter().any(|(_, _, family)| family == "sans-serif"));
        assert!(calls.iter().any(|(_, _, family)| family.ends_with(';')));
        assert!(calls.iter().any(|(_, text, _)| text == "\u{200b}"));
    }

    #[test]
    fn sequence_drawn_dimensions_route_direct_and_tspan_dom_shapes_separately() {
        let style = default_sequence_style();
        let direct = OperationProbe::default();
        let direct_dimensions = super::measure_drawn_svg_like_with_html_br(
            &direct,
            "alpha<br><br>beta",
            &style,
            super::SequenceDrawnTextNode::Direct,
        );
        assert_eq!(direct_dimensions, (101.0, 57.0));
        let direct_calls = direct.calls.borrow();
        assert_eq!(
            direct_calls
                .iter()
                .filter(|(operation, _, _)| operation == "raw-width")
                .count(),
            3
        );
        assert!(
            direct_calls
                .iter()
                .all(|(_, _, family)| family == "\"trebuchet ms\", verdana, arial, sans-serif")
        );
        assert!(direct_calls.iter().any(|(_, text, _)| text == "\u{200b}"));

        let tspan = OperationProbe::default();
        let tspan_dimensions = super::measure_drawn_svg_like_with_html_br(
            &tspan,
            "alpha",
            &style,
            super::SequenceDrawnTextNode::Tspan,
        );
        assert_eq!(tspan_dimensions, (202.0, 23.0));
        let tspan_calls = tspan.calls.borrow();
        assert!(
            tspan_calls
                .iter()
                .any(|(operation, _, _)| operation == "tspan-width")
        );
        assert!(
            tspan_calls
                .iter()
                .all(|(operation, _, _)| operation != "raw-width")
        );
        assert!(
            tspan_calls
                .iter()
                .any(|(operation, _, _)| operation == "tspan-height")
        );
        assert!(
            tspan_calls
                .iter()
                .all(|(operation, _, _)| operation != "simple-height")
        );
    }

    #[test]
    fn sequence_multiline_tspan_height_rounds_only_after_raw_line_accumulation() {
        struct SmallFontProbe;

        impl TextMeasurer for SmallFontProbe {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                unreachable!("the Sequence DOM operation is explicit")
            }

            fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
                1.0
            }

            fn measure_svg_tspan_text_bbox_height_px(
                &self,
                _text: &str,
                _style: &TextStyle,
            ) -> f64 {
                11.05078125
            }
        }

        let text = std::iter::repeat_n("g", 10)
            .collect::<Vec<_>>()
            .join("<br>");
        let (_, raw_height) = super::measure_drawn_svg_like_with_html_br(
            &SmallFontProbe,
            &text,
            &default_sequence_style(),
            super::SequenceDrawnTextNode::Tspan,
        );

        assert_eq!(raw_height, 110.5078125);
        assert_eq!(raw_height.round(), 111.0);
        assert_ne!(raw_height.round(), 10.0 * 11.05078125_f64.round());
    }
}
