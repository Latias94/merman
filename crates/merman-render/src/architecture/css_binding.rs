use crate::config::{config_css_number_or_string, config_f64, config_string};
use crate::svg::PreparedCommonCss;

#[derive(Debug)]
pub(crate) struct ArchitectureCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) edge_color: String,
    pub(crate) arrow_color: String,
    pub(crate) edge_width: String,
    pub(crate) group_border_color: String,
    pub(crate) group_border_width: String,
    pub(crate) icon_size_px: f64,
    pub(crate) padding_px: f64,
    pub(crate) arch_font_size_px: f64,
    pub(crate) use_max_width: bool,
}

impl ArchitectureCssBinding {
    pub(super) fn resolve(
        config: &serde_json::Value,
        font_family: &str,
        font_size_css: &str,
    ) -> Self {
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let line_color = color("lineColor", "#333333");
        let primary_border = color("primaryBorderColor", "hsl(240, 60%, 86.2745098039%)");
        let edge_color = color("archEdgeColor", &line_color);
        Self {
            common: PreparedCommonCss::with_resolved_typography(
                config,
                font_family.to_owned(),
                font_size_css.to_owned(),
            ),
            arrow_color: color("archEdgeArrowColor", &edge_color),
            edge_color,
            edge_width: config_css_number_or_string(config, &["themeVariables", "archEdgeWidth"])
                .unwrap_or_else(|| "3".to_owned()),
            group_border_color: color("archGroupBorderColor", &primary_border),
            group_border_width: config_css_number_or_string(
                config,
                &["themeVariables", "archGroupBorderWidth"],
            )
            .unwrap_or_else(|| "2px".to_owned()),
            icon_size_px: config_f64(config, &["architecture", "iconSize"])
                .unwrap_or(80.0)
                .max(1.0),
            padding_px: config_f64(config, &["architecture", "padding"])
                .unwrap_or(40.0)
                .max(0.0),
            arch_font_size_px: config_f64(config, &["architecture", "fontSize"])
                .unwrap_or(16.0)
                .max(1.0),
            use_max_width: config
                .pointer("/architecture/useMaxWidth")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        }
    }
}
