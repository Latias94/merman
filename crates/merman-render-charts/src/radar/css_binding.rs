use merman_render_core::{
    config::{config_f64, config_string, json_f64, value_at},
    mermaid_style::{is_resource_free_css_value, resolve_explicit_css_absolute_font_size_px},
    number_format::{canonical_number, canonicalize_number},
};

#[derive(Debug)]
pub(crate) struct RadarRoleFontSize(String);

impl RadarRoleFontSize {
    pub(super) fn resolve(css_px: f64) -> Self {
        let negative = css_px < 0.0;
        let css_px = canonicalize_number(css_px);
        Self(if negative && css_px == 0.0 {
            "inherit".to_owned()
        } else {
            format!("{}px", canonical_number(css_px))
        })
    }

    fn configured(config: &serde_json::Value, key: &str, default: f64) -> Self {
        let configured = value_at(config, &["radar", key])
            .or_else(|| value_at(config, &["themeVariables", "radar", key]));
        let Some(value) = configured else {
            return Self::resolve(default);
        };
        // Template interpolation preserves string whitespace and joins singleton arrays.
        // Multiple items or objects cannot form this one numeric dimension; no JS evaluator
        // or unbounded recursive coercion is needed for the accepted role option.
        let mut scalar = value;
        while let serde_json::Value::Array(items) = scalar {
            let [item] = items.as_slice() else {
                return Self("inherit".to_owned());
            };
            scalar = item;
        }
        let interpolated = match scalar {
            serde_json::Value::String(value) => format!("{value}px"),
            serde_json::Value::Number(value) => format!("{value}px"),
            _ => {
                return Self("inherit".to_owned());
            }
        };
        if let Some(size) = json_f64(scalar).filter(|size| *size < 0.0) {
            return Self::resolve(size);
        }
        // These roles append one px unit; general declaration ownership is deliberately
        // conservative about malformed multi-token values such as `24 px`.
        let px_dimension = {
            let mut parser = cssparser::Parser::new(&interpolated);
            let px_dimension = matches!(parser.next(), Ok(cssparser::Token::Dimension { unit, .. })
                if unit.eq_ignore_ascii_case("px"));
            px_dimension && parser.is_exhausted()
        };
        if !px_dimension {
            return Self("inherit".to_owned());
        }
        if let Some(size) = resolve_explicit_css_absolute_font_size_px(&interpolated) {
            return Self::resolve(size);
        }
        // A safe but unrepresentable dimension must retain its actual CSS. Substituting
        // inheritance could change browser-zero text into the positive root size.
        let css = if is_resource_free_css_value(&interpolated) {
            interpolated
        } else {
            "inherit".to_owned()
        };
        Self(css)
    }

    pub(crate) fn css(&self) -> &str {
        &self.0
    }
}

/// Radar CSS inputs resolved before SVG rendering.
/// Raw browser colors remain strings; numeric geometry follows the existing coercion.
#[derive(Debug)]
pub(crate) struct RadarCssBinding {
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) error_bkg_color: String,
    pub(crate) error_text_color: String,
    pub(crate) title_color: String,
    pub(crate) axis_color: String,
    pub(crate) native_axis_label_color: Option<String>,
    pub(crate) axis_stroke_width: f64,
    pub(crate) axis_label_font_size: RadarRoleFontSize,
    pub(crate) graticule_color: String,
    pub(crate) graticule_opacity: f64,
    pub(crate) graticule_stroke_width: f64,
    pub(crate) legend_font_size: RadarRoleFontSize,
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

    #[cfg(test)]
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        Self::resolve_styles(config)
    }

    pub(super) fn font_size_css_or_default(configured: Option<String>) -> String {
        configured.unwrap_or_else(|| "16px".to_owned())
    }

    pub(super) fn resolve_styles(config: &serde_json::Value) -> Self {
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
            native_axis_label_color: None,
            axis_stroke_width: scoped_number("axisStrokeWidth", 2.0),
            axis_label_font_size: RadarRoleFontSize::configured(config, "axisLabelFontSize", 12.0),
            graticule_color: scoped_string("graticuleColor", "#DEDEDE"),
            graticule_opacity: scoped_number("graticuleOpacity", 0.3),
            graticule_stroke_width: scoped_number("graticuleStrokeWidth", 1.0),
            legend_font_size: RadarRoleFontSize::configured(config, "legendFontSize", 12.0),
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
        assert_eq!(binding.legend_font_size.css(), "12px");
    }
}
