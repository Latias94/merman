use serde_json::Value;

use crate::config::{config_css_number_or_string, config_f64, config_string, config_string_vec};
use crate::svg::PreparedCommonCss;

use super::config::{RequirementConfigView, RequirementLayoutSettings};

/// Operation-local final Requirement CSS and the typography used by layout.
#[derive(Debug)]
pub(crate) struct RequirementCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) layout: RequirementLayoutSettings,
    pub(crate) border_colors: Vec<String>,
    pub(crate) background_colors: Vec<String>,
    pub(crate) theme_color_limit: usize,
    pub(crate) default_fill: String,
    pub(crate) default_stroke: String,
    pub(crate) node_text_color: String,
    pub(crate) relation_color: String,
    pub(crate) typed_relation_rule: Option<usize>,
    pub(crate) line_color: String,
    pub(crate) requirement_background: String,
    pub(crate) requirement_border_color: String,
    pub(crate) requirement_border_size: String,
    pub(crate) requirement_text_color: String,
    pub(crate) relation_label_background: String,
    pub(crate) relation_label_color: String,
    pub(crate) edge_label_background: String,
    pub(crate) requirement_edge_label_background: String,
    pub(crate) source_path_style_active: bool,
    pub(crate) node_border: String,
    pub(crate) relationship_line_stroke_width: String,
    pub(crate) marker_stroke_width: String,
    pub(crate) shadow_flood_color: &'static str,
}

impl RequirementCssBinding {
    pub(super) fn resolve(
        config: &Value,
        font_family: Option<&str>,
        font_size_css: Option<&str>,
        font_size_px: Option<f64>,
        typed_relation_color: Option<(usize, &str)>,
    ) -> Self {
        let common = match font_size_css {
            Some(size) => PreparedCommonCss::with_resolved_typography(
                config,
                font_family
                    .map(str::to_owned)
                    .unwrap_or_else(|| crate::config::config_font_family_css(config)),
                size.to_owned(),
            ),
            None => PreparedCommonCss::new(config, font_family),
        };
        let option = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let border_colors = config_string_vec(config, &["themeVariables", "borderColorArray"]);
        let background_colors = config_string_vec(config, &["themeVariables", "bkgColorArray"]);
        let edge_label_background = option("edgeLabelBackground", "rgba(232,232,232, 0.8)");
        let authored_edge_background = config_string(
            config,
            &["themeVariables", "requirementEdgeLabelBackground"],
        );
        let source_path_style_active = !border_colors.is_empty()
            || authored_edge_background
                .as_ref()
                .is_some_and(|value| !value.is_empty());
        let node_text_color = config_string(config, &["themeVariables", "nodeTextColor"])
            .filter(|color| !color.is_empty())
            .unwrap_or_else(|| common.text_color().to_owned());
        let neo = crate::config::config_diagram_look(config).is_neo();
        Self {
            common,
            layout: RequirementConfigView::new(config)
                .layout_settings_with_resolved_typography(font_family, font_size_px),
            border_colors,
            background_colors,
            theme_color_limit: config_f64(config, &["themeVariables", "THEME_COLOR_LIMIT"])
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 64.0).ceil() as usize)
                .unwrap_or(12),
            default_fill: color("mainBkg", "#ECECFF"),
            default_stroke: color("nodeBorder", "#9370DB"),
            node_text_color,
            relation_color: typed_relation_color
                .map(|(_, color)| color.to_owned())
                .unwrap_or_else(|| option("relationColor", "#333333")),
            typed_relation_rule: typed_relation_color.map(|(rule, _)| rule),
            line_color: option("lineColor", "#333333"),
            requirement_background: option("requirementBackground", "#ECECFF"),
            requirement_border_color: option(
                "requirementBorderColor",
                "hsl(240, 60%, 86.2745098039%)",
            ),
            requirement_border_size: option("requirementBorderSize", "1"),
            requirement_text_color: option("requirementTextColor", "#131300"),
            relation_label_background: option("relationLabelBackground", "rgba(232,232,232, 0.8)"),
            relation_label_color: option("relationLabelColor", "black"),
            edge_label_background: edge_label_background.clone(),
            requirement_edge_label_background: authored_edge_background
                .unwrap_or(edge_label_background),
            source_path_style_active,
            node_border: option("nodeBorder", "#9370DB"),
            relationship_line_stroke_width: if neo {
                option("strokeWidth", "1")
            } else {
                "1px".to_owned()
            },
            marker_stroke_width: config
                .pointer("/themeVariables/strokeWidth")
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
                .unwrap_or_else(|| "undefined".to_owned()),
            shadow_flood_color: config
                .get("theme")
                .and_then(Value::as_str)
                .filter(|theme| theme.contains("dark"))
                .map(|_| "#FFFFFF")
                .unwrap_or("#000000"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_color_retains_literal_case_sensitive_theme_name_semantics() {
        for (config, expected) in [
            (serde_json::json!({}), "#000000"),
            (serde_json::json!({"theme": "dark"}), "#FFFFFF"),
            (serde_json::json!({"theme": "redux-dark-color"}), "#FFFFFF"),
            (
                serde_json::json!({"theme": "Dark", "darkMode": true}),
                "#000000",
            ),
            (
                serde_json::json!({"theme": 7, "themeVariables": {"darkMode": true}}),
                "#000000",
            ),
        ] {
            let binding = RequirementCssBinding::resolve(&config, None, None, None, None);
            assert_eq!(binding.shadow_flood_color, expected);
        }
    }
}
