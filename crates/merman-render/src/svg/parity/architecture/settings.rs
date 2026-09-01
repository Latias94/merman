use crate::text::TextStyle;

use super::super::{SvgDiagramId, config_f64};

#[derive(Clone)]
pub(super) struct ArchitectureRenderSettings {
    pub(super) css: String,
    pub(super) font_family_css: String,
    pub(super) typography_css_emission: crate::architecture::ArchitectureTypographyCssEmission,
    pub(super) icon_size_px: f64,
    pub(super) half_icon: f64,
    pub(super) padding_px: f64,
    pub(super) arch_font_size_px: f64,
    pub(super) use_max_width: bool,
    pub(super) text_style: TextStyle,
    pub(super) compound_text_style: TextStyle,
}

impl ArchitectureRenderSettings {
    pub(super) fn from_config(
        diagram_id: SvgDiagramId<'_>,
        effective_config: &serde_json::Value,
    ) -> Self {
        Self::from_config_for_id(diagram_id, effective_config)
    }

    pub(super) fn from_config_with_typography(
        diagram_id: SvgDiagramId<'_>,
        effective_config: &serde_json::Value,
        typography: &crate::architecture::ArchitectureTypographyThemePlan,
    ) -> Self {
        Self::from_config_parts(
            diagram_id,
            effective_config,
            Some(typography.font_family_css()),
            Some(typography.font_size_px()),
        )
    }

    fn from_config_for_id(
        diagram_id: impl std::fmt::Display + Copy,
        effective_config: &serde_json::Value,
    ) -> Self {
        Self::from_config_parts(diagram_id, effective_config, None, None)
    }

    fn from_config_parts(
        diagram_id: impl std::fmt::Display + Copy,
        effective_config: &serde_json::Value,
        resolved_font_family: Option<&str>,
        resolved_font_size_px: Option<f64>,
    ) -> Self {
        let css_parts = super::super::css::architecture_css_parts_with_typography(
            diagram_id,
            effective_config,
            resolved_font_family,
            resolved_font_size_px,
        );

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
        let use_max_width = effective_config
            .get("architecture")
            .and_then(|v| v.get("useMaxWidth"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let text_style = TextStyle {
            font_family: Some(css_parts.font_family.clone()),
            font_size: css_parts.font_size,
            font_weight: None,
            font_style: None,
        };
        let compound_text_style =
            crate::architecture::architecture_cytoscape_text_style(arch_font_size_px);

        Self {
            css: css_parts.css,
            font_family_css: css_parts.font_family,
            typography_css_emission: css_parts.typography_emission,
            icon_size_px,
            half_icon,
            padding_px,
            arch_font_size_px,
            use_max_width,
            text_style,
            compound_text_style,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_render_settings_keep_cytoscape_measurement_owner() {
        let cfg = serde_json::json!({
            "fontFamily": "Courier, monospace",
            "themeVariables": {
                "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
            },
            "architecture": {
                "fontSize": 18
            }
        });

        let settings = ArchitectureRenderSettings::from_config_for_id("arch", &cfg);

        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
        assert_eq!(
            settings.compound_text_style.font_family.as_deref(),
            Some("Helvetica Neue,Helvetica,sans-serif")
        );
        assert!(settings.css.contains(
            r#"#arch{font-family:"IBM Plex Sans",Arial,sans-serif;font-size:16px;fill:#333;}"#
        ));
    }
}
