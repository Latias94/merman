use crate::config::{config_f64, config_string};

/// Mermaid CSS inputs frozen before Radar's typed paint winners are lowered.
/// Raw browser colors remain strings; numeric geometry follows the existing coercion.
#[derive(Debug)]
pub(crate) struct RadarCssBinding {
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) error_bkg_color: String,
    pub(crate) error_text_color: String,
    pub(crate) title_color: String,
    pub(crate) axis_color: String,
    pub(crate) axis_stroke_width: f64,
    pub(crate) axis_label_font_size: f64,
    pub(crate) graticule_color: String,
    pub(crate) graticule_opacity: f64,
    pub(crate) graticule_stroke_width: f64,
    pub(crate) legend_font_size: f64,
    pub(crate) curve_opacity: f64,
    pub(crate) curve_stroke_width: f64,
    pub(crate) series_colors: [String; 12],
}

impl RadarCssBinding {
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_string())
        };
        let scoped_string = |key: &str, fallback: &str| {
            config_string(config, &["radar", key])
                .or_else(|| config_string(config, &["themeVariables", "radar", key]))
                .unwrap_or_else(|| fallback.to_string())
        };
        let scoped_number = |key: &str, fallback: f64| {
            config_f64(config, &["radar", key])
                .or_else(|| config_f64(config, &["themeVariables", "radar", key]))
                .unwrap_or(fallback)
        };
        Self {
            text_color: color("textColor", "#333"),
            line_color: color("lineColor", "#333333"),
            error_bkg_color: color("errorBkgColor", "#552222"),
            error_text_color: color("errorTextColor", "#552222"),
            title_color: color("titleColor", "#333"),
            axis_color: scoped_string("axisColor", "#333333"),
            axis_stroke_width: scoped_number("axisStrokeWidth", 2.0),
            axis_label_font_size: scoped_number("axisLabelFontSize", 12.0),
            graticule_color: scoped_string("graticuleColor", "#DEDEDE"),
            graticule_opacity: scoped_number("graticuleOpacity", 0.3),
            graticule_stroke_width: scoped_number("graticuleStrokeWidth", 1.0),
            legend_font_size: scoped_number("legendFontSize", 12.0),
            curve_opacity: scoped_number("curveOpacity", 0.5),
            curve_stroke_width: scoped_number("curveStrokeWidth", 2.0),
            series_colors: std::array::from_fn(|index| {
                color(
                    &format!("cScale{index}"),
                    crate::svg::render_theme::radar_default_series_color(index),
                )
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_css_and_scoped_numeric_fallback_remain_distinct() {
        let binding = RadarCssBinding::resolve(&serde_json::json!({
            "radar": {"axisColor": "var(--axis)", "axisStrokeWidth": "invalid"},
            "themeVariables": {
                "textColor": "currentColor", "cScale0": "var(--series)",
                "radar": {"axisColor": "#123456", "axisStrokeWidth": "3.5"}
            }
        }));
        assert_eq!(binding.axis_color, "var(--axis)");
        assert_eq!(binding.text_color, "currentColor");
        assert_eq!(binding.series_colors[0], "var(--series)");
        assert_eq!(binding.axis_stroke_width, 3.5);
        assert_eq!(binding.legend_font_size, 12.0);
    }
}
