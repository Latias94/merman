use serde_json::Value;

use crate::config::{config_css_number_or_string, config_f64, config_string};

/// Operation-local values shared by Tree View measurement and SVG emission.
#[derive(Debug)]
pub(crate) struct TreeViewCssBinding {
    pub(crate) font_family: String,
    pub(crate) label_font_size: f64,
    pub(crate) label_font_size_css: String,
    pub(super) font_size_configured: bool,
    pub(crate) root_font_size_css: Option<String>,
    pub(crate) label_font_size_verified: bool,
    pub(crate) line_thickness: f64,
    pub(crate) label_color: String,
    pub(crate) line_color: String,
    pub(crate) icon_color: String,
    pub(crate) description_color: String,
    pub(crate) highlight_bg: String,
    pub(crate) highlight_stroke: String,
}

impl TreeViewCssBinding {
    pub(super) fn configured_color(config: &Value, key: &str) -> Option<String> {
        config_string(config, &["themeVariables", "treeView", key])
    }

    pub(super) fn configured_font_size_css(config: &Value) -> Option<String> {
        config_css_number_or_string(config, &["themeVariables", "treeView", "labelFontSize"])
    }

    pub(super) fn with_font_family(config: &Value, font_family: &str) -> Self {
        let color = |key, fallback: &str| {
            Self::configured_color(config, key).unwrap_or_else(|| fallback.to_string())
        };
        let label_font_size_css = Self::configured_font_size_css(config);
        let (label_font_size, root_font_size_css, font_size_configured) =
            match label_font_size_css.as_deref() {
                Some(size) => resolve_label_font_size(config, size),
                None => (Some(16.0), None, false),
            };
        Self {
            font_family: font_family.to_owned(),
            label_font_size: label_font_size.unwrap_or(16.0),
            label_font_size_verified: label_font_size.is_some(),
            root_font_size_css,
            font_size_configured,
            label_font_size_css: label_font_size_css.unwrap_or_else(|| "16px".to_string()),
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

fn resolve_label_font_size(config: &Value, size: &str) -> (Option<f64>, Option<String>, bool) {
    use crate::mermaid_style::{
        CssFontSizeContext, CssFontSizeOwnership, css_font_size_is_zero,
        css_font_size_value_override, resolve_mermaid_absolute_font_size_px,
        resolve_mermaid_font_size_px,
    };

    let ownership = css_font_size_value_override(size);
    if ownership.is_some() {
        if let Some(size) = resolve_mermaid_absolute_font_size_px(size) {
            return (Some(size), None, true);
        }
        if css_font_size_is_zero(size) {
            return (Some(0.0), None, true);
        }
    }

    let root_css = config_css_number_or_string(config, &["themeVariables", "fontSize"])
        .or_else(|| config_css_number_or_string(config, &["fontSize"]))
        .unwrap_or_else(|| "16px".to_string());
    let root_px = css_font_size_value_override(&root_css)
        .and_then(|_| resolve_mermaid_absolute_font_size_px(&root_css));
    // Pin the SVG root to the very context used for em, percentages and inherited sizes.
    // rem follows the containing document's root instead, so it remains unverified.
    let root_css = root_px.map_or(root_css, |size| format!("{size}px"));
    let resolved = match ownership {
        None => root_px, // Invalid font-size declarations inherit the actual root size.
        Some(winner) if winner.ownership() != CssFontSizeOwnership::Unverified => {
            root_px.and_then(|root_px| {
                resolve_mermaid_font_size_px(size, CssFontSizeContext::uniform(root_px))
            })
        }
        Some(_) => None,
    };
    (resolved, Some(root_css), ownership.is_some())
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
        assert!(binding.label_font_size_verified);
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
        assert!(!binding.label_font_size_verified);
        assert_eq!(binding.label_color, "var(--tree-color)");
        let binding = TreeViewCssBinding::from_config(&json!({
            "themeVariables": {"treeView": {"labelFontSize": 0}}
        }));
        assert_eq!(binding.label_font_size_css, "0");
        assert_eq!(binding.label_font_size, 0.0);
    }
}
