use serde_json::Value;

use merman_render_core::config::{config_css_number_or_string, config_f64, config_string};

use super::config::DEFAULT_LINE_THICKNESS;
use super::native_theme::TreeViewNativeStyle;

/// Operation-local values shared by Tree View measurement and SVG emission.
#[derive(Debug)]
pub struct TreeViewCssBinding {
    pub(crate) native: Option<TreeViewNativeStyle>,
    pub(crate) font_family: String,
    pub(crate) label_font_size: f64,
    pub(crate) label_font_size_css: String,
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
    fn configured_color(config: &Value, key: &str) -> Option<String> {
        config_string(config, &["themeVariables", "treeView", key])
    }

    fn configured_font_size_css(config: &Value) -> Option<String> {
        config_css_number_or_string(config, &["themeVariables", "treeView", "labelFontSize"])
    }

    pub(crate) fn from_config(config: &Value) -> Self {
        let color = |key, fallback: &str| {
            Self::configured_color(config, key).unwrap_or_else(|| fallback.to_string())
        };
        let label_font_size_css = Self::configured_font_size_css(config);
        let (label_font_size, root_font_size_css) = match label_font_size_css.as_deref() {
            Some(size) => resolve_label_font_size(config, size),
            None => (Some(16.0), None),
        };
        Self {
            native: None,
            font_family: merman_render_core::config::config_font_family_css(config),
            label_font_size: label_font_size.unwrap_or(16.0),
            label_font_size_verified: label_font_size.is_some(),
            root_font_size_css,
            label_font_size_css: label_font_size_css.unwrap_or_else(|| "16px".to_string()),
            line_thickness: config_f64(config, &["treeView", "lineThickness"])
                .unwrap_or(DEFAULT_LINE_THICKNESS)
                .max(0.0),
            label_color: color("labelColor", "black"),
            line_color: color("lineColor", "black"),
            icon_color: color("iconColor", "#546e7a"),
            description_color: color("descriptionColor", "#6a9955"),
            highlight_bg: color("highlightBg", "rgba(255, 193, 7, 0.15)"),
            highlight_stroke: color("highlightStroke", "#ffc107"),
        }
    }

    pub(crate) fn for_native(
        config: &merman_model::presentation::PresentationConfig,
        theme: &merman_theme_contract::v2::FamilyTheme,
    ) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let line_thickness = config_f64(config, &["treeView", "lineThickness"])
            .filter(|_| config.explicit_config_owns_path("treeView.lineThickness"))
            .unwrap_or(theme.form.connector_width)
            .max(0.0);
        Self {
            native: Some(TreeViewNativeStyle {
                node_fill: roles.surface_node.first_color(),
                on_highlight: roles.text_on_emphasis,
                label_weight: typography.label_weight.to_string(),
                directory_weight: typography.title_weight.to_string(),
            }),
            font_family: merman_render_core::mermaid_style::font_family_css(
                typography.stack(typography.code_family),
            ),
            label_font_size: typography.base_size_px,
            label_font_size_css: format!("{}px", typography.base_size_px),
            root_font_size_css: None,
            label_font_size_verified: true,
            line_thickness,
            label_color: roles.text_primary.to_string(),
            line_color: roles.stroke_guide.first_color().to_string(),
            icon_color: roles.text_muted.to_string(),
            description_color: roles.text_muted.to_string(),
            highlight_bg: roles.surface_emphasis.first_color().to_string(),
            highlight_stroke: roles.stroke_border.first_color().to_string(),
        }
    }

    pub(crate) fn includes_guide_outset(&self) -> bool {
        self.native.is_some() && self.line_thickness > 0.0
    }
}

fn resolve_label_font_size(config: &Value, size: &str) -> (Option<f64>, Option<String>) {
    use merman_render_core::mermaid_style::{
        CssFontSizeContext, CssFontSizeOwnership, css_font_size_is_zero,
        css_font_size_value_override, resolve_mermaid_absolute_font_size_px,
        resolve_mermaid_font_size_px,
    };

    let ownership = css_font_size_value_override(size);
    if ownership.is_some() {
        if let Some(size) = resolve_mermaid_absolute_font_size_px(size) {
            return (Some(size), None);
        }
        if css_font_size_is_zero(size) {
            return (Some(0.0), None);
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
    (resolved, Some(root_css))
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
