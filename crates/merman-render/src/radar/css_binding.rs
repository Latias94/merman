use merman_model::presentation::PresentationConfig;

#[cfg(test)]
use crate::config::config_css_number_or_string;
use crate::config::{config_f64, config_string, value_at};

/// Raw numeric CSS and the used-size visibility fact share the prepared root context.
#[derive(Debug)]
pub(crate) struct RadarNumericFontSize {
    css_px: f64,
    has_visible_ink: bool,
}

impl RadarNumericFontSize {
    pub(super) fn resolve(css_px: f64, root_size_css: &str) -> Self {
        let css_px = crate::number_format::canonicalize_number(css_px);
        let has_visible_ink = if css_px < 0.0 {
            // An invalid negative declaration inherits the final root; unknown CSS stays visible.
            !crate::mermaid_style::css_font_size_is_zero(root_size_css)
        } else {
            css_px != 0.0
        };
        Self {
            css_px,
            has_visible_ink,
        }
    }

    pub(crate) fn css_px(&self) -> f64 {
        self.css_px
    }

    pub(super) fn has_visible_ink(&self) -> bool {
        self.has_visible_ink
    }
}

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
    pub(crate) axis_label_font_size: RadarNumericFontSize,
    pub(crate) graticule_color: String,
    pub(crate) graticule_opacity: f64,
    pub(crate) graticule_stroke_width: f64,
    pub(crate) legend_font_size: RadarNumericFontSize,
    pub(crate) curve_opacity: f64,
    pub(crate) curve_stroke_width: f64,
}

impl RadarCssBinding {
    pub(super) fn configured_color<'a>(
        config: &'a serde_json::Value,
        key: &str,
    ) -> Option<&'a str> {
        value_at(config, &["themeVariables", key]).and_then(serde_json::Value::as_str)
    }

    pub(super) fn config_owns_color(config: &PresentationConfig, key: &str) -> bool {
        Self::configured_color(config.as_value(), key).is_some()
            && config.config_paint_overrides_typed_default(&format!("themeVariables.{key}"))
    }

    fn configured_axis_color(config: &serde_json::Value) -> Option<(&str, &'static str)> {
        value_at(config, &["radar", "axisColor"])
            .and_then(serde_json::Value::as_str)
            .map(|color| (color, "radar.axisColor"))
            .or_else(|| {
                value_at(config, &["themeVariables", "radar", "axisColor"])
                    .and_then(serde_json::Value::as_str)
                    .map(|color| (color, "themeVariables.radar.axisColor"))
            })
    }

    pub(super) fn config_owns_axis_color(config: &PresentationConfig) -> bool {
        Self::configured_axis_color(config.as_value())
            .is_some_and(|(_, path)| config.config_paint_overrides_typed_default(path))
    }

    #[cfg(test)]
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        let root_size_css = Self::font_size_css_or_default(config_css_number_or_string(
            config,
            &["themeVariables", "fontSize"],
        ));
        Self::resolve_with_font_size(config, &root_size_css)
    }

    pub(super) fn font_size_css_or_default(configured: Option<String>) -> String {
        configured.unwrap_or_else(|| "16px".to_owned())
    }

    pub(super) fn resolve_with_font_size(config: &serde_json::Value, root_size_css: &str) -> Self {
        let color = |key: &str, fallback: &str| {
            Self::configured_color(config, key)
                .unwrap_or(fallback)
                .to_owned()
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
            axis_color: Self::configured_axis_color(config)
                .map_or("#333333", |(color, _)| color)
                .to_owned(),
            axis_stroke_width: scoped_number("axisStrokeWidth", 2.0),
            axis_label_font_size: RadarNumericFontSize::resolve(
                scoped_number("axisLabelFontSize", 12.0),
                root_size_css,
            ),
            graticule_color: scoped_string("graticuleColor", "#DEDEDE"),
            graticule_opacity: scoped_number("graticuleOpacity", 0.3),
            graticule_stroke_width: scoped_number("graticuleStrokeWidth", 1.0),
            legend_font_size: RadarNumericFontSize::resolve(
                scoped_number("legendFontSize", 12.0),
                root_size_css,
            ),
            curve_opacity: scoped_number("curveOpacity", 0.5),
            curve_stroke_width: scoped_number("curveStrokeWidth", 2.0),
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
                "textColor": "currentColor",
                "radar": {"axisColor": "#123456", "axisStrokeWidth": "3.5"}
            }
        }));
        assert_eq!(binding.axis_color, "var(--axis)");
        assert_eq!(binding.text_color, "currentColor");
        assert_eq!(binding.axis_stroke_width, 3.5);
        assert_eq!(binding.legend_font_size.css_px(), 12.0);
    }
}
