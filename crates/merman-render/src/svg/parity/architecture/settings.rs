use crate::text::TextStyle;

use super::super::{config_f64, config_f64_css_px};

#[derive(Clone)]
pub(super) struct ArchitectureRenderSettings {
    pub(super) css: String,
    pub(super) icon_size_px: f64,
    pub(super) half_icon: f64,
    pub(super) padding_px: f64,
    pub(super) arch_font_size_px: f64,
    pub(super) svg_font_size_px: f64,
    pub(super) use_max_width: bool,
    pub(super) text_style: TextStyle,
    pub(super) compound_text_style: TextStyle,
}

impl ArchitectureRenderSettings {
    pub(super) fn from_config(diagram_id: &str, effective_config: &serde_json::Value) -> Self {
        let css = super::super::css::architecture_css_with_config(diagram_id, effective_config);

        let icon_size_px = config_f64(effective_config, &["architecture", "iconSize"])
            .unwrap_or(80.0)
            .max(1.0);
        let half_icon = icon_size_px / 2.0;
        let padding_px = config_f64(effective_config, &["architecture", "padding"])
            .unwrap_or(40.0)
            .max(0.0);

        // Mermaid Architecture uses `architecture.fontSize` primarily for layout (Cytoscape node
        // label sizing) and group label positioning. The rendered SVG text inherits the global SVG
        // font size (typically `fontSize: 16`) rather than `architecture.fontSize`.
        let arch_font_size_px = config_f64(effective_config, &["architecture", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0);
        let svg_font_size_px = config_f64_css_px(effective_config, &["themeVariables", "fontSize"])
            .or_else(|| config_f64(effective_config, &["fontSize"]))
            .unwrap_or(16.0)
            .max(1.0);
        let use_max_width = effective_config
            .get("architecture")
            .and_then(|v| v.get("useMaxWidth"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let text_style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: svg_font_size_px,
            font_weight: None,
        };
        let compound_text_style = TextStyle {
            font_family: text_style.font_family.clone(),
            font_size: arch_font_size_px,
            font_weight: None,
        };

        Self {
            css,
            icon_size_px,
            half_icon,
            padding_px,
            arch_font_size_px,
            svg_font_size_px,
            use_max_width,
            text_style,
            compound_text_style,
        }
    }
}
