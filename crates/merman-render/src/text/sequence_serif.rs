use super::{TextStyle, VendoredFontMetricsTextMeasurer};

fn cssom_rejects_font_family(style: &TextStyle) -> bool {
    style
        .font_family
        .as_deref()
        .is_some_and(|family| family.trim_end().ends_with(';'))
}

pub(crate) fn measure_sequence_calculate_text_width_px(
    text: &str,
    style: &TextStyle,
) -> Option<f64> {
    if !cssom_rejects_font_family(style) {
        return None;
    }

    let table =
        crate::generated::sequence_calculate_text_font_metrics_11_16_0::lookup_font_metrics(
            "serif",
        )?;
    if text.trim_end().is_empty() {
        return Some(0.0);
    }

    Some(
        VendoredFontMetricsTextMeasurer::measure_svg_single_run_bbox_width_with_table(
            table,
            text,
            style.font_size,
        )
        .round(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_sequence_style() -> TextStyle {
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
            font_size: 16.0,
            font_weight: None,
        }
    }

    #[test]
    fn chrome_131_default_serif_matches_calculate_text_width_probes() {
        let style = default_sequence_style();
        for (text, expected, tolerance) in [
            (
                "This is a longer message that should be wrapped by Mermaid's default behavior",
                510.0,
                0.0,
            ),
            (
                "This is a longer message that should be wrapped by Mermaid's default",
                450.0,
                0.0,
            ),
            ("behavior ", 56.0, 0.0),
            ("[Establish a", 75.0, 0.0),
            ("connection ", 70.0, 0.0),
            ("Hello John, how are you today?", 204.0, 1.0),
            ("I'm ", 21.0, 1.0),
            ("finishing up an important", 162.0, 1.0),
            ("meeting. ", 56.0, 1.0),
            ("I feel great! I was not ignoring", 195.0, 1.0),
            ("you. ", 28.0, 1.0),
        ] {
            let actual = measure_sequence_calculate_text_width_px(text, &style)
                .expect("default Sequence font should select the serif profile");
            assert!(
                (actual - expected).abs() <= tolerance,
                "unexpected Chrome 131 calculateTextWidth model for {text:?}: actual={actual}, expected={expected}, tolerance={tolerance}"
            );
        }
    }

    #[test]
    fn valid_css_font_family_stays_with_the_host_profile() {
        let mut style = default_sequence_style();
        style.font_family = Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string());
        assert_eq!(
            measure_sequence_calculate_text_width_px("message", &style),
            None
        );
    }
}
