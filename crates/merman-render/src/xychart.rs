//! XYChart layout types are exported through [`crate::model`].

#[cfg(test)]
mod tests {
    use merman_render_charts::xychart::prepare_xychart_diagram;

    use crate::text::{TextMeasurer, TextMetrics, TextStyle};

    #[test]
    fn xychart_measures_title_axes_and_legend_with_the_configured_font() {
        use std::sync::Mutex;
        struct RecordingFont(Mutex<Vec<String>>);
        impl TextMeasurer for RecordingFont {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert_eq!(style.font_family.as_deref(), Some("Inter,sans-serif"));
                self.0.lock().unwrap().push(text.to_owned());
                TextMetrics {
                    width: text.encode_utf16().count() as f64 * style.font_size,
                    height: style.font_size,
                    line_count: 1,
                }
            }
        }
        let parsed = merman_core::MermaidEngine::new().parse_diagram_for_render_model_sync(
            "xychart\ntitle Chart\nx-axis Categories [One, Two, Three]\ny-axis Amount 0 --> 40\nline Series [10, 30, 20]",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap().into_presentation();
        let merman_model::presentation::RenderSemanticModel::XyChart(model) = parsed.model() else {
            panic!("expected XYChart render model");
        };
        let measurer = RecordingFont(Mutex::new(Vec::new()));
        let config = crate::test_support::presentation_config(serde_json::json!({
            "fontFamily":"Inter, sans-serif", "xyChart":{"showLegend":true},
        }));
        prepare_xychart_diagram(model, None, &config, None, false, &measurer).unwrap();
        let text = measurer.0.lock().unwrap();
        for expected in ["Chart", "Categories", "Amount", "One", "Series"] {
            assert!(
                text.iter().any(|text| text == expected),
                "missing measurement: {expected}"
            );
        }
    }
}
