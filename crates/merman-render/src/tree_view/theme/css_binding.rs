use serde_json::Value;

use crate::config::{config_css_number_or_string, config_f64, config_f64_css_px, config_string};

/// Operation-local values shared by Tree View measurement and SVG emission.
#[derive(Debug)]
pub(crate) struct TreeViewCssBinding {
    pub(crate) font_family: String,
    pub(crate) label_font_size: f64,
    pub(crate) label_font_size_css: String,
    pub(crate) line_thickness: f64,
    pub(crate) label_color: String,
    pub(crate) line_color: String,
    pub(crate) icon_color: String,
    pub(crate) description_color: String,
    pub(crate) highlight_bg: String,
    pub(crate) highlight_stroke: String,
}

impl TreeViewCssBinding {
    pub(super) fn with_font_family(config: &Value, font_family: &str) -> Self {
        let color = |key, fallback: &str| {
            config_string(config, &["themeVariables", "treeView", key])
                .unwrap_or_else(|| fallback.to_string())
        };
        Self {
            font_family: font_family.to_owned(),
            label_font_size: config_f64_css_px(
                config,
                &["themeVariables", "treeView", "labelFontSize"],
            )
            .unwrap_or(16.0)
            .max(1.0),
            label_font_size_css: config_css_number_or_string(
                config,
                &["themeVariables", "treeView", "labelFontSize"],
            )
            .unwrap_or_else(|| "16px".to_string()),
            line_thickness: config_f64(config, &["treeView", "lineThickness"])
                .unwrap_or(super::DEFAULT_LINE_THICKNESS)
                .max(0.0),
            label_color: color("labelColor", "black"),
            line_color: color("lineColor", "black"),
            icon_color: color("iconColor", "#546e7a"),
            description_color: color("descriptionColor", "#6a9955"),
            highlight_bg: color("highlightBg", "rgba(255, 193, 7, 0.15)"),
            highlight_stroke: color("highlightStroke", "#ffc107"),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_config(config: &Value) -> Self {
        Self::with_font_family(config, &crate::config::config_font_family_css(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binding_preserves_configured_tree_view_roles() {
        let binding = TreeViewCssBinding::from_config(&json!({
            "themeVariables": { "treeView": {
                "labelFontSize": "20px", "labelColor": "#FF0000",
                "lineColor": "#00FF00", "iconColor": "#111111",
                "descriptionColor": "#222222", "highlightBg": "rgba(1, 2, 3, 0.4)",
                "highlightStroke": "#333333"
            }}
        }));
        assert_eq!(binding.label_font_size, 20.0);
        assert_eq!(binding.label_font_size_css, "20px");
        assert_eq!(binding.label_color, "#FF0000");
        assert_eq!(binding.line_color, "#00FF00");
        assert_eq!(binding.icon_color, "#111111");
        assert_eq!(binding.description_color, "#222222");
        assert_eq!(binding.highlight_bg, "rgba(1, 2, 3, 0.4)");
        assert_eq!(binding.highlight_stroke, "#333333");
    }

    #[test]
    fn binding_preserves_default_tree_view_roles() {
        let binding = TreeViewCssBinding::from_config(&json!({}));
        assert_eq!(binding.label_font_size, 16.0);
        assert_eq!(binding.label_font_size_css, "16px");
        assert_eq!(binding.label_color, "black");
        assert_eq!(binding.line_color, "black");
        assert_eq!(binding.icon_color, "#546e7a");
        assert_eq!(binding.description_color, "#6a9955");
        assert_eq!(binding.highlight_bg, "rgba(255, 193, 7, 0.15)");
        assert_eq!(binding.highlight_stroke, "#ffc107");
    }

    #[test]
    fn binding_keeps_opaque_css_and_measurement_fallback_separate() {
        let binding = TreeViewCssBinding::from_config(&json!({
            "themeVariables": {"treeView": {
                "labelFontSize": "var(--tree-size)", "labelColor": "var(--tree-color)"
            }}
        }));
        assert_eq!(binding.label_font_size_css, "var(--tree-size)");
        assert_eq!(binding.label_font_size, 16.0);
        assert_eq!(binding.label_color, "var(--tree-color)");
        let binding = TreeViewCssBinding::from_config(&json!({
            "themeVariables": {"treeView": {"labelFontSize": 0}}
        }));
        assert_eq!(binding.label_font_size_css, "0");
        assert_eq!(binding.label_font_size, 1.0);
    }
}
