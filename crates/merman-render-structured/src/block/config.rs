use merman_render_core::config::{config_f64, config_font_family_or_first_array_css};
use merman_render_core::text::TextStyle;
use serde_json::Value;

const DEFAULT_BLOCK_PADDING: f64 = 8.0;
const DEFAULT_BLOCK_FONT_SIZE: f64 = 16.0;

pub(crate) struct BlockConfigView<'a> {
    effective_config: &'a Value,
    block_config: &'a Value,
}

impl<'a> BlockConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            block_config: effective_config.get("block").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(&self) -> BlockLayoutSettings {
        BlockLayoutSettings {
            padding: self.padding(),
            text_style: TextStyle {
                font_family: Some(config_font_family_or_first_array_css(self.effective_config)),
                font_size: merman_render_core::config::config_theme_or_root_font_size_px_opt(
                    self.effective_config,
                )
                .filter(|size| *size >= 0.0)
                .unwrap_or(DEFAULT_BLOCK_FONT_SIZE),
                font_weight: None,
                font_style: None,
            },
        }
    }

    pub(crate) fn padding(&self) -> f64 {
        self.block_f64("padding").unwrap_or(DEFAULT_BLOCK_PADDING)
    }

    fn shape_settings(&self, native: Option<&super::BlockNativeStyle>) -> BlockShapeSettings {
        let neo_square = self.effective_config.get("look").and_then(Value::as_str) == Some("neo");
        if let Some(style) = native {
            return BlockShapeSettings {
                neo_square,
                round_radius: style.form.radius,
                native_radii: Some((style.form.radius, style.form.container_radius)),
            };
        }
        let radius = self.effective_config.pointer("/themeVariables/radius");
        let round_radius = radius
            .filter(|value| match value {
                Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
                Value::String(value) => !value.is_empty(),
                _ => false,
            })
            .and_then(merman_render_core::config::json_f64)
            .unwrap_or(5.0);
        BlockShapeSettings {
            neo_square,
            round_radius,
            native_radii: None,
        }
    }

    fn block_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.block_config, &[key])
    }
}

pub(crate) struct BlockLayoutSettings {
    pub(crate) padding: f64,
    pub(crate) text_style: TextStyle,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockShapeSettings {
    pub(crate) neo_square: bool,
    pub(crate) round_radius: f64,
    pub(super) native_radii: Option<(f64, f64)>,
}

impl BlockShapeSettings {
    pub(crate) fn resolve(
        effective_config: &Value,
        native: Option<&super::BlockNativeStyle>,
    ) -> Self {
        BlockConfigView::new(effective_config).shape_settings(native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn block_layout_settings_project_padding_and_text_style() {
        let cfg = json!({
            "fontFamily": "Root Sans, Arial",
            "themeVariables": {
                "fontSize": "24px"
            },
            "block": {
                "padding": "12"
            }
        });

        let settings = BlockConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.padding, 12.0);
        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some("Root Sans,Arial")
        );
        assert_eq!(settings.text_style.font_size, 24.0);
    }

    #[test]
    fn block_layout_settings_preserve_zero_font_size_and_padding_semantics() {
        let cfg = json!({
            "themeVariables": {
                "fontSize": 0
            },
            "block": {
                "padding": -2
            }
        });

        let settings = BlockConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.padding, -2.0);
        assert_eq!(settings.text_style.font_size, 0.0);
    }

    #[test]
    fn invalid_negative_font_sizes_use_the_default_without_changing_padding() {
        for value in [json!(-2), json!("-2px")] {
            for config in [
                json!({"themeVariables": {"fontSize": value}, "block": {"padding": -2}}),
                json!({"fontSize": value, "block": {"padding": -2}}),
            ] {
                let settings = BlockConfigView::new(&config).layout_settings();
                assert_eq!(settings.text_style.font_size, DEFAULT_BLOCK_FONT_SIZE);
                assert_eq!(settings.padding, -2.0);
            }
        }
    }

    #[test]
    fn block_layout_settings_honor_root_html_label_override() {
        let cfg = json!({
            "htmlLabels": false,
            "flowchart": {"htmlLabels": true}
        });

        assert!(!merman_render_core::config::config_effective_html_labels(
            &cfg
        ));
    }

    #[test]
    fn block_layout_settings_use_flowchart_html_label_fallback() {
        let cfg = json!({"flowchart": {"htmlLabels": false}});

        assert!(!merman_render_core::config::config_effective_html_labels(
            &cfg
        ));
    }
}
