use crate::config::{config_css_number_or_string, config_f64};
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
    pub(super) fn edge_color_source(config: &serde_json::Value) -> (&'static str, Option<&str>) {
        if let Some(color) = config
            .pointer("/themeVariables/archEdgeColor")
            .and_then(serde_json::Value::as_str)
        {
            ("themeVariables.archEdgeColor", Some(color))
        } else {
            (
                "themeVariables.lineColor",
                config
                    .pointer("/themeVariables/lineColor")
                    .and_then(serde_json::Value::as_str),
            )
        }
    }

    pub(super) fn arrow_color_source(config: &serde_json::Value) -> (&'static str, Option<&str>) {
        if let Some(color) = config
            .pointer("/themeVariables/archEdgeArrowColor")
            .and_then(serde_json::Value::as_str)
        {
            ("themeVariables.archEdgeArrowColor", Some(color))
        } else {
            Self::edge_color_source(config)
        }
    }

    pub(super) fn group_border_color_source(
        config: &serde_json::Value,
    ) -> (&'static str, Option<&str>) {
        if let Some(color) = config
            .pointer("/themeVariables/archGroupBorderColor")
            .and_then(serde_json::Value::as_str)
        {
            ("themeVariables.archGroupBorderColor", Some(color))
        } else {
            (
                "themeVariables.primaryBorderColor",
                config
                    .pointer("/themeVariables/primaryBorderColor")
                    .and_then(serde_json::Value::as_str),
            )
        }
    }

    pub(super) fn resolve(
        config: &serde_json::Value,
        font_family: &str,
        font_size_css: &str,
    ) -> Self {
        let edge_color = Self::edge_color_source(config)
            .1
            .unwrap_or("#333333")
            .to_owned();
        Self {
            common: PreparedCommonCss::with_resolved_typography(
                config,
                font_family.to_owned(),
                font_size_css.to_owned(),
            ),
            arrow_color: Self::arrow_color_source(config)
                .1
                .unwrap_or("#333333")
                .to_owned(),
            edge_color,
            edge_width: config_css_number_or_string(config, &["themeVariables", "archEdgeWidth"])
                .unwrap_or_else(|| "3".to_owned()),
            group_border_color: Self::group_border_color_source(config)
                .1
                .unwrap_or("hsl(240, 60%, 86.2745098039%)")
                .to_owned(),
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
