use crate::config::{
    config_bool, config_diagram_look, config_f64_css_px, config_string, config_string_vec,
};
use crate::family::InheritedFontStackPlan;
use crate::svg::PreparedCommonCss;
use merman_core::theme_color::{ColorChannel, ColorError, ThemeColor, rgba};

#[derive(Debug)]
pub(crate) struct ErCssBinding {
    pub(crate) render_settings: super::super::config::ErRenderSettings,
    pub(crate) common: PreparedCommonCss,
    pub(crate) font_family: String,
    pub(crate) font_size_css: String,
    pub(crate) theme_name: String,
    pub(crate) redux_border_colors: Vec<String>,
    pub(crate) redux_background_colors: Vec<String>,
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) cluster_bkg: String,
    pub(crate) cluster_border: String,
    pub(crate) title_color: String,
    pub(crate) node_text_color: String,
    pub(crate) tertiary_color: String,
    pub(crate) edge_label_background: String,
    pub(crate) label_background: Result<String, ColorError>,
    pub(crate) neo_label_background: Result<String, ColorError>,
    pub(crate) stroke_width: String,
    pub(crate) neo_marker_stroke_width: f64,
    pub(crate) row_odd: Option<String>,
    pub(crate) row_even: Option<String>,
    pub(crate) shadow_flood: &'static str,
    pub(crate) gradient: Option<(String, String)>,
}

impl ErCssBinding {
    pub(crate) fn resolve(
        config: &serde_json::Value,
        fonts: &InheritedFontStackPlan,
        size: &super::ErBaseFontSizePlan,
    ) -> Self {
        let option = |key: &str| config_string(config, &["themeVariables", key]);
        let color = |key: &str, fallback: &str| option(key).unwrap_or_else(|| fallback.to_owned());
        let theme_name = config_string(config, &["theme"]).unwrap_or_else(|| "default".into());
        let redux_color_theme = matches!(theme_name.as_str(), "redux-color" | "redux-dark-color");
        let text_color = color("textColor", "#333");
        let main_bkg = color("mainBkg", "#ECECFF");
        let node_border = color("nodeBorder", "#9370DB");
        let tertiary_color = color("tertiaryColor", "hsl(80, 100%, 96.2745098039%)");
        let faded = fade(&tertiary_color);
        let er_background = redux_color_theme
            .then(|| option("erEdgeLabelBackground"))
            .flatten();
        let label_background = er_background.clone().map_or_else(|| faded.clone(), Ok);
        let gradient = config_bool(config, &["themeVariables", "useGradient"])
            .unwrap_or(false)
            .then(|| {
                let start = option("gradientStart")
                    .or_else(|| option("primaryBorderColor"))
                    .unwrap_or_else(|| "#9370DB".into());
                let stop = option("gradientStop")
                    .or_else(|| option("secondaryBorderColor"))
                    .unwrap_or_else(|| start.clone());
                (start, stop)
            });
        let font_family = fonts.font_family_css().to_owned();
        let font_size_css = size.font_size_css().to_owned();
        Self {
            render_settings: super::super::ErConfigView::new(config)
                .render_settings_with_resolved_typography(
                    fonts
                        .typed_font_stack_active()
                        .then(|| fonts.font_family_css()),
                    size.layout_override_px(),
                ),
            common: PreparedCommonCss::bind(
                config,
                &font_family,
                &font_size_css,
                fonts.typed_font_stack_active(),
            ),
            font_family,
            font_size_css,
            redux_border_colors: if redux_color_theme {
                config_string_vec(config, &["themeVariables", "borderColorArray"])
            } else {
                Vec::new()
            },
            redux_background_colors: if redux_color_theme {
                config_string_vec(config, &["themeVariables", "bkgColorArray"])
            } else {
                Vec::new()
            },
            text_color: text_color.clone(),
            line_color: color("lineColor", "#333333"),
            cluster_bkg: color("clusterBkg", &main_bkg),
            cluster_border: color("clusterBorder", &node_border),
            title_color: color("titleColor", &text_color),
            node_text_color: color("nodeTextColor", &text_color),
            main_bkg,
            node_border,
            tertiary_color,
            edge_label_background: er_background
                .unwrap_or_else(|| color("edgeLabelBackground", "rgba(232,232,232, 0.8)")),
            label_background,
            neo_label_background: faded,
            stroke_width: if config_diagram_look(config).is_neo() {
                crate::config::config_css_number_or_string(
                    config,
                    &["themeVariables", "strokeWidth"],
                )
                .unwrap_or_else(|| "1px".into())
            } else {
                "1px".into()
            },
            neo_marker_stroke_width: config_f64_css_px(config, &["themeVariables", "strokeWidth"])
                .filter(|value| value.is_finite())
                .unwrap_or(1.0)
                .max(0.0),
            row_odd: option("rowOdd"),
            row_even: option("rowEven"),
            shadow_flood: if theme_name.contains("dark") {
                "#FFFFFF"
            } else {
                "#000000"
            },
            theme_name,
            gradient,
        }
    }
}

fn fade(value: &str) -> Result<String, ColorError> {
    let color = ThemeColor::parse(value.trim())?;
    rgba(
        color.channel(ColorChannel::Red),
        color.channel(ColorChannel::Green),
        color.channel(ColorChannel::Blue),
        0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::MermaidConfig;
    use serde_json::json;

    fn resolve(config: serde_json::Value) -> ErCssBinding {
        let config = MermaidConfig::from_value(config);
        ErCssBinding::resolve(
            config.as_value(),
            &InheritedFontStackPlan::resolve_property_local(None, &config),
            &super::super::ErBaseFontSizePlan::resolve(None, &config),
        )
    }

    #[test]
    fn redux_roles_and_raw_css_remain_distinct_from_native_measurements() {
        let binding = resolve(
            json!({"theme": "redux-dark-color", "look": "neo", "themeVariables": {
                "rowOdd": "var(--row)", "nodeBorder": "currentColor", "strokeWidth": "2px",
                "borderColorArray": ["#123456"], "bkgColorArray": ["#abcdef"],
                "erEdgeLabelBackground": "var(--label)", "tertiaryColor": "not-a-color"
            }}),
        );
        assert_eq!(binding.node_border, "currentColor");
        assert_eq!(binding.row_odd.as_deref(), Some("var(--row)"));
        assert_eq!(binding.label_background.as_deref(), Ok("var(--label)"));
        assert!(binding.neo_label_background.is_err());
        assert_eq!(binding.redux_border_colors, ["#123456"]);
        assert_eq!(binding.redux_background_colors, ["#abcdef"]);
        assert_eq!(binding.shadow_flood, "#FFFFFF");
        assert_eq!(binding.neo_marker_stroke_width, 2.0);
    }

    #[test]
    fn defaults_and_gradient_fallbacks_preserve_mermaid_roles() {
        let defaults = resolve(json!({}));
        assert_eq!(defaults.main_bkg, "#ECECFF");
        assert_eq!(defaults.node_border, "#9370DB");
        assert_eq!(defaults.text_color, "#333");
        assert_eq!(defaults.stroke_width, "1px");
        assert_eq!(defaults.shadow_flood, "#000000");
        assert!(defaults.label_background.is_ok());
        assert!(defaults.gradient.is_none());
        let gradient = resolve(json!({"themeVariables": {
            "useGradient": true, "primaryBorderColor": "var(--start)"
        }}));
        assert_eq!(
            gradient.gradient,
            Some(("var(--start)".into(), "var(--start)".into()))
        );
    }
}
