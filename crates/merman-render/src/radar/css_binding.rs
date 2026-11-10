use merman_model::presentation::PresentationConfig;

#[cfg(test)]
use crate::config::config_css_number_or_string;
use crate::config::{config_f64, config_string, value_at};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RadarTextVisibility {
    Hidden,
    Visible,
    Unknown,
}

impl RadarTextVisibility {
    fn measured(size: f64) -> Self {
        if size == 0.0 {
            Self::Hidden
        } else if size > 0.0 && (size as f32).is_normal() {
            Self::Visible
        } else {
            Self::Unknown
        }
    }

    pub(super) fn root(css: &str) -> Self {
        if crate::mermaid_style::css_font_size_is_zero(css) {
            return Self::Hidden;
        }
        let Some(size) = crate::mermaid_style::parse_style_declaration(&format!("font-size:{css}"))
            .and_then(|declaration| declaration.analysis().context_free_font_size_px())
        else {
            return Self::Unknown;
        };
        let mut parser = cssparser::Parser::new(crate::mermaid_style::strip_css_important(css));
        if matches!(parser.next(), Ok(cssparser::Token::Dimension { value, .. })
            if *value <= 0.0 || !value.is_normal())
        {
            return Self::Unknown;
        }
        Self::measured(size)
    }
}

/// Final role CSS and its inherited visibility share the prepared root context.
#[derive(Debug)]
pub(crate) struct RadarRoleFontSize {
    css: String,
    inherits_root: bool,
    visibility: RadarTextVisibility,
}

impl RadarRoleFontSize {
    pub(super) fn resolve(css_px: f64, root_size_css: &str) -> Self {
        let inherits_root = css_px < 0.0;
        let css_px = crate::number_format::canonicalize_number(css_px);
        Self::with_css(
            if inherits_root && css_px == 0.0 {
                "inherit".to_owned()
            } else {
                format!("{}px", crate::number_format::canonical_number(css_px))
            },
            inherits_root,
            RadarTextVisibility::measured(css_px),
            root_size_css,
        )
    }

    fn configured(
        config: &serde_json::Value,
        key: &str,
        default: f64,
        root_size_css: &str,
    ) -> Self {
        let configured = value_at(config, &["radar", key])
            .or_else(|| value_at(config, &["themeVariables", "radar", key]));
        let Some(value) = configured else {
            return Self::resolve(default, root_size_css);
        };
        // Template interpolation preserves string whitespace and joins singleton arrays.
        // Multiple items or objects cannot form this one numeric dimension; no JS evaluator
        // or unbounded recursive coercion is needed for the accepted role option.
        let mut scalar = value;
        while let serde_json::Value::Array(items) = scalar {
            let [item] = items.as_slice() else {
                return Self::with_css(
                    "inherit".to_owned(),
                    true,
                    RadarTextVisibility::Hidden,
                    root_size_css,
                );
            };
            scalar = item;
        }
        let interpolated = match scalar {
            serde_json::Value::String(value) => format!("{value}px"),
            serde_json::Value::Number(value) => format!("{value}px"),
            _ => {
                return Self::with_css(
                    "inherit".to_owned(),
                    true,
                    RadarTextVisibility::Hidden,
                    root_size_css,
                );
            }
        };
        if let Some(size) = crate::config::json_f64(scalar).filter(|size| *size < 0.0) {
            return Self::resolve(size, root_size_css);
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
            return Self::with_css(
                "inherit".to_owned(),
                true,
                RadarTextVisibility::Hidden,
                root_size_css,
            );
        }
        if let Some(size) =
            crate::mermaid_style::resolve_explicit_css_absolute_font_size_px(&interpolated)
        {
            return Self::resolve(size, root_size_css);
        }
        // A safe but unrepresentable dimension must retain its actual CSS. Substituting
        // inheritance could change browser-zero text into the positive root size.
        let css = if crate::mermaid_style::is_resource_free_css_value(&interpolated) {
            interpolated
        } else {
            "inherit".to_owned()
        };
        Self::with_css(css, false, RadarTextVisibility::Unknown, root_size_css)
    }

    fn with_css(
        css: String,
        inherits_root: bool,
        own_visibility: RadarTextVisibility,
        root_size_css: &str,
    ) -> Self {
        let visibility = if inherits_root {
            RadarTextVisibility::root(root_size_css)
        } else {
            own_visibility
        };
        Self {
            css,
            inherits_root,
            visibility,
        }
    }

    pub(crate) fn css(&self) -> &str {
        &self.css
    }

    pub(super) fn inherits_root(&self) -> bool {
        self.inherits_root
    }

    pub(super) fn has_visible_ink(&self) -> bool {
        self.visibility == RadarTextVisibility::Visible
    }

    pub(super) fn is_unknown(&self) -> bool {
        self.visibility == RadarTextVisibility::Unknown
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
            axis_label_font_size: RadarRoleFontSize::configured(
                config,
                "axisLabelFontSize",
                12.0,
                root_size_css,
            ),
            graticule_color: scoped_string("graticuleColor", "#DEDEDE"),
            graticule_opacity: scoped_number("graticuleOpacity", 0.3),
            graticule_stroke_width: scoped_number("graticuleStrokeWidth", 1.0),
            legend_font_size: RadarRoleFontSize::configured(
                config,
                "legendFontSize",
                12.0,
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
        assert_eq!(binding.legend_font_size.css(), "12px");
    }
}
