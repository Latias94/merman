use crate::config::{
    config_bool, config_diagram_look, config_f64_css_px, config_string, config_string_vec,
};
use crate::svg::PreparedCommonCss;
use merman_markup::theme_color::{ColorChannel, ColorError, ThemeColor, rgba};

#[derive(Debug)]
pub(crate) struct ErCssBinding {
    pub(crate) render_settings: super::super::config::ErRenderSettings,
    pub(crate) native: Option<crate::er::ErNativeStyle>,
    pub(crate) common: PreparedCommonCss,
    pub(crate) font_family: String,
    source_fill_owns_odd_rows: bool,
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
    pub(super) fn from_native(
        theme: &crate::theme::FamilyTheme,
        native: crate::er::ErNativeStyle,
        config: &serde_json::Value,
    ) -> Self {
        let roles = &theme.roles;
        let render_settings = super::super::ErConfigView::new(config)
            .render_settings_with_resolved_typography(
                native.body_family.as_deref(),
                native.font_size,
            );
        let font_family = render_settings.font_family.clone();
        let text_color = roles.text_primary.to_string();
        let line_color = roles.stroke_connector.first_color().to_string();
        let node_border = roles.stroke_border.first_color().to_string();
        let cluster_bkg = roles.surface_container.first_color().to_string();
        let label_background = &native.label_background;
        // The shared Neo and marker rules must use the same paints as the ER writer.
        let paints = serde_json::json!({"themeVariables": {
            "textColor": text_color,
            "nodeBorder": node_border,
            "lineColor": line_color,
            "strokeWidth": native.border_width,
        }});
        let mut common = PreparedCommonCss::with_resolved_typography(
            &paints,
            font_family.clone(),
            format!("{}px", render_settings.font_size),
        );
        if native.body_family.is_none() {
            common = common.with_configured_root_font_family(config);
        }
        Self {
            render_settings,
            common,
            font_family,
            source_fill_owns_odd_rows: false,
            redux_border_colors: Vec::new(),
            redux_background_colors: Vec::new(),
            node_text_color: text_color.clone(),
            text_color,
            line_color,
            main_bkg: roles.surface_emphasis.first_color().to_string(),
            cluster_border: node_border.clone(),
            node_border,
            row_odd: Some(roles.surface_node.first_color().to_string()),
            row_even: Some(cluster_bkg.clone()),
            cluster_bkg,
            title_color: roles.text_title.to_string(),
            tertiary_color: label_background.clone(),
            edge_label_background: label_background.clone(),
            label_background: Ok(label_background.clone()),
            neo_label_background: Ok(label_background.clone()),
            stroke_width: format!("{}px", native.connector_width),
            neo_marker_stroke_width: native.connector_width,
            shadow_flood: "#000000",
            gradient: None,
            native: Some(native),
        }
    }

    pub(super) fn title_color_source(config: &serde_json::Value) -> (Option<&'static str>, &str) {
        crate::config::selected_config_string(
            config,
            &["themeVariables.titleColor", "themeVariables.textColor"],
        )
        .map_or((None, "#333"), |(path, color)| (Some(path), color))
    }

    fn node_text_color_source(config: &serde_json::Value) -> Option<&str> {
        config["themeVariables"]["nodeTextColor"]
            .as_str()
            .filter(|color| !color.is_empty())
            .or_else(|| config["themeVariables"]["textColor"].as_str())
    }

    pub(super) fn source_fill_owns_odd_rows(&self) -> bool {
        self.source_fill_owns_odd_rows
    }

    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        let option = |key: &str| config_string(config, &["themeVariables", key]);
        let color = |key: &str, fallback: &str| option(key).unwrap_or_else(|| fallback.to_owned());
        let theme_name = config_string(config, &["theme"]).unwrap_or_else(|| "default".into());
        let redux_color_theme = matches!(theme_name.as_str(), "redux-color" | "redux-dark-color");
        let text_color = color("textColor", "#333");
        let node_text_color = Self::node_text_color_source(config)
            .unwrap_or(&text_color)
            .to_owned();
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
        let font_family = crate::config::config_font_family_css(config);
        let font_size = crate::config::configured_theme_or_root_font_size_px(config)
            .map(|(size, _)| size)
            .or_else(|| config_f64_css_px(config, &["er", "fontSize"]).filter(|size| *size >= 0.0))
            .unwrap_or(16.0);
        Self {
            native: None,
            render_settings: super::super::ErConfigView::new(config)
                .render_settings_with_resolved_typography(None, None),
            common: PreparedCommonCss::with_resolved_typography(
                config,
                crate::config::normalize_css_font_family(&font_family),
                format!("{font_size}px"),
            )
            .with_configured_root_font_family(config),
            font_family,
            source_fill_owns_odd_rows: matches!(
                theme_name.as_str(),
                "redux" | "redux-dark" | "redux-color" | "redux-dark-color"
            ),
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
            title_color: Self::title_color_source(config).1.to_owned(),
            node_text_color,
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
    use serde_json::json;

    fn resolve(config: serde_json::Value) -> ErCssBinding {
        ErCssBinding::resolve(&config)
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
