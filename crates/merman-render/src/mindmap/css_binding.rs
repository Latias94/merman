//! Fixed Mindmap visual inputs prepared before layout and consumed by SVG emission.

use merman_model::presentation::PresentationConfig;

use super::theme::{mindmap_color_scale_css, mindmap_theme_color_limit};

#[derive(Debug)]
pub(crate) struct MindmapSectionCss {
    pub(crate) fill: String,
    pub(crate) inverse: String,
    pub(crate) label: String,
}

#[derive(Debug)]
pub(crate) struct MindmapCssBinding {
    pub(crate) common: crate::svg::PreparedCommonCss,
    pub(crate) label_style: crate::text::TextStyle,
    pub(crate) sections: Vec<MindmapSectionCss>,
    pub(crate) neutral_label: String,
    pub(crate) label_background: Option<String>,
    pub(crate) label_text: Option<String>,
    pub(crate) theme: Box<str>,
    pub(crate) look: Box<str>,
    pub(crate) root_fill: String,
    pub(crate) root_label: String,
    pub(crate) node_border: String,
    pub(crate) main_background: String,
    pub(crate) stroke_width: String,
    pub(crate) drop_shadow: String,
    pub(crate) use_gradient: bool,
    pub(crate) gradient: Option<(String, String)>,
    pub(crate) shadow_flood: &'static str,
    pub(crate) cluster_background: Box<str>,
    pub(crate) cluster_border: Box<str>,
}

impl MindmapCssBinding {
    pub(crate) fn new(config: &PresentationConfig) -> Self {
        let raw = config.as_value();
        let token = |key: &str, fallback: &str| {
            config
                .get_str(&format!("themeVariables.{key}"))
                .unwrap_or(fallback)
                .to_owned()
        };
        let theme = config.get_str("theme").unwrap_or_default();
        let use_gradient = raw
            .pointer("/themeVariables/useGradient")
            .and_then(crate::config::json_bool)
            .unwrap_or(false);
        let gradient = use_gradient
            .then(|| {
                Some((
                    config.get_str("themeVariables.gradientStart")?.to_owned(),
                    config.get_str("themeVariables.gradientStop")?.to_owned(),
                ))
            })
            .flatten();
        let root_fill = token("git0", "hsl(240, 100%, 46.2745098039%)");
        let root_label = token("gitBranchLabel0", "#ffffff");
        let main_background = token("mainBkg", &root_fill);
        let node_border = crate::config::selected_config_string(
            raw,
            &[
                "themeVariables.nodeBorder",
                "themeVariables.gitBranchLabel0",
            ],
        )
        .map_or("#ffffff", |(_, color)| color)
        .to_owned();
        const INVERSE: [&str; 12] = [
            "hsl(60, 100%, 86.2745098039%)",
            "hsl(240, 100%, 83.5294117647%)",
            "hsl(260, 100%, 86.2745098039%)",
            "hsl(90, 100%, 86.2745098039%)",
            "hsl(120, 100%, 86.2745098039%)",
            "hsl(150, 100%, 86.2745098039%)",
            "hsl(180, 100%, 86.2745098039%)",
            "hsl(210, 100%, 86.2745098039%)",
            "hsl(270, 100%, 86.2745098039%)",
            "hsl(330, 100%, 86.2745098039%)",
            "hsl(0, 100%, 86.2745098039%)",
            "hsl(30, 100%, 86.2745098039%)",
        ];
        let sections = (0..mindmap_theme_color_limit(config))
            .map(|slot| MindmapSectionCss {
                fill: mindmap_color_scale_css(config, slot),
                inverse: token(&format!("cScaleInv{slot}"), INVERSE[slot % INVERSE.len()]),
                label: token(
                    &format!("cScaleLabel{slot}"),
                    if slot == 0 || slot == 3 {
                        "#ffffff"
                    } else {
                        "black"
                    },
                ),
            })
            .collect();
        let font_family = crate::config::config_font_family_css(config);
        let common = crate::svg::PreparedCommonCss::new(raw, Some(&font_family));
        let label_style = super::mindmap_text_style_with_font_family(raw, font_family);
        Self {
            common,
            label_style,
            sections,
            neutral_label: token("cScaleLabel1", "black"),
            label_background: crate::config::config_string(
                raw,
                &["themeVariables", "mindmap", "labelBackgroundColor"],
            ),
            label_text: crate::config::config_string(
                raw,
                &["themeVariables", "mindmap", "labelTextColor"],
            ),
            theme: theme.into(),
            look: crate::config::mermaid_config_diagram_look(config)
                .as_str()
                .into(),
            root_fill,
            root_label,
            node_border,
            main_background,
            stroke_width: crate::config::config_css_number_or_string(
                raw,
                &["themeVariables", "strokeWidth"],
            )
            .unwrap_or_else(|| "2".into()),
            drop_shadow: crate::config::config_css_number_or_string(
                raw,
                &["themeVariables", "dropShadow"],
            )
            .unwrap_or_else(|| "none".into()),
            use_gradient,
            gradient,
            shadow_flood: if theme.contains("dark") {
                "#FFFFFF"
            } else {
                "#000000"
            },
            cluster_background: token("clusterBkg", "#ffffde").into(),
            cluster_border: token("clusterBorder", "#aaaa33").into(),
        }
    }

    pub(crate) fn model_look<'a>(&'a self, model_look: &'a str) -> &'a str {
        let model_look = model_look.trim();
        if model_look.is_empty() || model_look == "default" {
            &self.look
        } else {
            model_look
        }
    }
}
