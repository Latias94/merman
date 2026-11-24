use merman_model::presentation::PresentationConfig;

/// The CSS token and its measurable width share the same source selection.
#[derive(Debug)]
pub(crate) struct ClassRelationWidthBinding {
    css: String,
    paint_width: Option<f32>,
}

impl ClassRelationWidthBinding {
    pub(super) fn from_config(config: &PresentationConfig) -> Self {
        let values = config.as_value();
        let css =
            crate::config::config_css_number_or_string(values, &["themeVariables", "strokeWidth"]);
        let authored =
            css.is_some() && config.explicit_config_owns_path("themeVariables.strokeWidth");
        let paint_width = authored
            .then(|| {
                crate::config::config_f64_css_px(values, &["themeVariables", "strokeWidth"])
                    .filter(|value| value.is_finite() && *value >= 0.0 && *value <= f32::MAX as f64)
                    .map(|value| value as f32)
            })
            .flatten();
        Self {
            css: css.unwrap_or_else(|| "1".to_owned()),
            paint_width,
        }
    }

    pub(super) fn from_native(width: f64) -> Self {
        Self {
            css: width.to_string(),
            paint_width: Some(width as f32),
        }
    }

    pub(crate) fn css(&self) -> &str {
        &self.css
    }

    pub(crate) fn paint_width(&self) -> Option<f32> {
        self.paint_width
    }
}
