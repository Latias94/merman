//! Fixed Mindmap visual inputs prepared before layout and consumed by SVG emission.

use merman_core::MermaidConfig;

use super::theme::{
    MindmapNodeFillOwnership, MindmapNodeFillSource, MindmapNodePalettePlan,
    MindmapWriterThemeTokens,
};

#[derive(Debug)]
pub(crate) struct MindmapSectionCss {
    pub(crate) fill: String,
    pub(crate) inverse: String,
    pub(crate) label: String,
}

#[derive(Debug)]
pub(crate) struct MindmapCssBinding {
    pub(crate) common: crate::svg::PreparedCommonCss,
    pub(crate) sections: Vec<MindmapSectionCss>,
    pub(crate) neutral_label: String,
    pub(crate) theme: Box<str>,
    pub(crate) look: Box<str>,
    pub(crate) root_fill: String,
    pub(crate) root_label: String,
    pub(crate) tokens: MindmapWriterThemeTokens,
    pub(crate) stroke_width: String,
    pub(crate) drop_shadow: String,
    pub(crate) use_gradient: bool,
    pub(crate) gradient: Option<(String, String)>,
    pub(crate) shadow_flood: &'static str,
    pub(crate) cluster_background: Box<str>,
    pub(crate) cluster_border: Box<str>,
    pub(crate) node_fill_ownership: MindmapNodeFillOwnership,
    pub(crate) neo_fill_source: MindmapNodeFillSource,
}

impl MindmapCssBinding {
    pub(super) fn new(config: &MermaidConfig, plan: &MindmapNodePalettePlan) -> Self {
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
        let tokens = plan.writer_theme_tokens(
            &token("mainBkg", &root_fill),
            &super::theme::mindmap_node_border_css(config),
        );
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
        let sections = (0..super::theme::mindmap_theme_color_limit(config))
            .map(|slot| MindmapSectionCss {
                fill: super::theme::mindmap_color_scale_css(config, slot),
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
        let theme_id = merman_core::MermaidThemeId::parse(theme).unwrap_or_default();
        Self {
            common: crate::svg::PreparedCommonCss::new(raw, Some(plan.font_family_css())),
            sections,
            neutral_label: token("cScaleLabel1", "black"),
            theme: theme.into(),
            look: crate::config::mermaid_config_diagram_look(config)
                .as_str()
                .into(),
            root_fill,
            root_label,
            tokens,
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
            node_fill_ownership: MindmapNodeFillOwnership::from_config(config),
            neo_fill_source: if use_gradient
                || matches!(
                    theme_id,
                    merman_core::MermaidThemeId::Redux
                        | merman_core::MermaidThemeId::ReduxDark
                        | merman_core::MermaidThemeId::Neutral
                ) {
                MindmapNodeFillSource::MainBackground
            } else {
                MindmapNodeFillSource::ColorScale
            },
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
