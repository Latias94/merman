//! Shared math preparation and whole-document terminal evidence.

pub(crate) use merman_render_core::math::*;

#[cfg(all(test, feature = "math", feature = "diagram-flowchart"))]
mod tests {
    use merman_model::presentation::PresentationConfig;

    use super::{MathRenderer, RatexMathRenderer};
    use crate::flowchart::{FlowchartLabelMetricsRequest, flowchart_label_metrics_for_layout};
    use crate::text::{DeterministicTextMeasurer, TextStyle, WrapMode};

    #[test]
    fn flowchart_uses_ratex_html_label_metrics() {
        let renderer = RatexMathRenderer;
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let label = "$$x^2$$";
        let wrap_mode = WrapMode::HtmlLike;
        let max_width_px = Some(200.0);
        let expected = renderer
            .measure_html_label(label, &config, &style, max_width_px, wrap_mode)
            .expect("ratex should measure pure flowchart math labels");
        let actual = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &DeterministicTextMeasurer::default(),
            raw_label: label,
            label_type: "text",
            style: &style,
            max_width_px,
            wrap_mode,
            config: &config,
            math_renderer: Some(&renderer),
        });
        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.height, expected.height);
    }
}
