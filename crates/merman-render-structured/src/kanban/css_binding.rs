use merman_markup::theme_color::{darken, lighten};
use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Result,
    config::{config_string, json_value_is_truthy},
    svg_css::{PreparedCommonCss, default_section_colors},
};
use serde_json::Value;

#[derive(Debug)]
pub(crate) struct KanbanSectionCssBinding {
    pub(crate) section_fill: String,
    pub(crate) c_scale: String,
    pub(crate) c_scale_label: String,
    pub(crate) c_scale_inv: String,
}

/// Final common and family CSS prepared before Kanban writers consume it.
#[derive(Debug)]
pub(crate) struct KanbanCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) background: String,
    pub(crate) node_border: String,
    pub(crate) root_fill: String,
    pub(crate) root_label: String,
    pub(crate) sections: Vec<KanbanSectionCssBinding>,
}

pub(super) fn dark_mode(config: &PresentationConfig) -> bool {
    let variable = config
        .as_value()
        .get("themeVariables")
        .and_then(|variables| variables.get("darkMode"))
        .map(json_value_is_truthy);
    // Mermaid's stylesheet reads themeVariables. Keep an explicitly configured root alias,
    // but do not let its initialized false value shadow the selected or authored theme.
    if config.explicit_config_owns_path("themeVariables.darkMode") {
        return variable.unwrap_or(false);
    }
    if config.explicit_config_owns_path("darkMode") {
        return config.get_bool("darkMode").or(variable).unwrap_or(false);
    }
    variable
        .or_else(|| config.get_bool("darkMode"))
        .unwrap_or(false)
}

impl KanbanCssBinding {
    pub(super) fn mermaid(config: &PresentationConfig, common: PreparedCommonCss) -> Result<Self> {
        Self::from_common(config.as_value(), common, dark_mode(config))
    }

    fn from_common(config: &Value, common: PreparedCommonCss, dark_mode: bool) -> Result<Self> {
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_string())
        };
        let sections = (0..12)
            .map(|index| {
                let [scale, label, inverse] = default_section_colors(index);
                let c_scale = color(&format!("cScale{index}"), scale);
                let section_fill = if dark_mode {
                    darken(&c_scale, 10.0)?
                } else {
                    lighten(&c_scale, 10.0)?
                };
                Ok(KanbanSectionCssBinding {
                    section_fill,
                    c_scale,
                    c_scale_label: color(&format!("cScaleLabel{index}"), label),
                    c_scale_inv: color(&format!("cScaleInv{index}"), inverse),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            common,
            background: color("background", "white"),
            node_border: color("nodeBorder", "#9370DB"),
            root_fill: color("git0", "hsl(240, 100%, 46.2745098039%)"),
            root_label: color("gitBranchLabel0", "#ffffff"),
            sections,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(config: &Value, typography: Option<(&str, &str)>) -> Result<Self> {
        let common = match typography {
            Some((family, size)) => PreparedCommonCss::with_resolved_typography(
                config,
                family.to_owned(),
                size.to_owned(),
            ),
            None => PreparedCommonCss::new(config, None),
        };
        Self::from_common(
            config,
            common,
            dark_mode(&super::presentation_config_for_test(config.clone())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binds_all_sections_even_when_the_model_has_fewer_sections() {
        let invalid = json!({"themeVariables": {"cScale11": "var(--late-section)"}});
        assert!(KanbanCssBinding::for_test(&invalid, None).is_err());
        let config = json!({"darkMode": true, "themeVariables": {
            "cScale0": "#123456", "background": "var(--card)",
            "nodeBorder": "currentColor"
        }});
        let binding = KanbanCssBinding::for_test(&config, None).unwrap();
        assert_eq!(binding.sections.len(), 12);
        assert_eq!(
            binding.sections[0].section_fill,
            darken("#123456", 10.0).unwrap()
        );
        assert_eq!(binding.background, "var(--card)");
        assert_eq!(binding.node_border, "currentColor");
    }

    #[test]
    fn prepared_kanban_binding_resolves_theme_roles() {
        let cfg = json!({
            "themeVariables": {
                "background": "#0f172a",
                "nodeBorder": "#38bdf8",
                "textColor": "#f8fafc",
                "git0": "#22c55e",
                "gitBranchLabel0": "#020617",
                "cScale0": "hsl(160, 80%, 40%)",
                "cScaleLabel0": "#f8fafc",
                "cScaleInv0": "#111827"
            }
        });

        let kanban = KanbanCssBinding::for_test(&cfg, None).unwrap();

        assert_eq!(kanban.common.text_color(), "#f8fafc");
        assert_eq!(kanban.background, "#0f172a");
        assert_eq!(kanban.node_border, "#38bdf8");
        assert_eq!(kanban.root_fill, "#22c55e");
        assert_eq!(kanban.root_label, "#020617");
        assert_eq!(kanban.sections[0].c_scale, "hsl(160, 80%, 40%)");
        assert_eq!(kanban.sections[0].section_fill, "hsl(160, 80%, 50%)");
        assert_eq!(kanban.sections[0].c_scale_label, "#f8fafc");
        assert_eq!(kanban.sections[0].c_scale_inv, "#111827");
    }

    #[test]
    fn prepared_kanban_binding_dark_mode_adjusts_section_fill_down() {
        let cfg = json!({
            "darkMode": true
        });

        let kanban = KanbanCssBinding::for_test(&cfg, None).unwrap();

        assert_eq!(kanban.sections[0].c_scale, "hsl(240, 100%, 76.2745098039%)");
        assert_eq!(
            kanban.sections[0].section_fill,
            "hsl(240, 100%, 66.2745098039%)"
        );
        assert_eq!(kanban.sections[0].c_scale_label, "#ffffff");
        assert_eq!(
            kanban.sections[0].c_scale_inv,
            "hsl(60, 100%, 86.2745098039%)"
        );
    }

    #[test]
    fn prepared_kanban_binding_uses_khroma_for_every_supported_color_syntax() {
        let cases = [
            ("#ff0000", "hsl(0, 100%, 60%)"),
            ("rebeccapurple", "hsl(270, 50%, 50%)"),
            (
                "rgb(18 52 86 / .5)",
                "hsla(210, 65.3846153846%, 30.3921568627%, 0.5)",
            ),
        ];

        for (input, expected) in cases {
            let cfg = json!({ "themeVariables": { "cScale0": input } });
            let kanban = KanbanCssBinding::for_test(&cfg, None).unwrap();
            assert_eq!(kanban.sections[0].section_fill, expected, "{input}");
        }
    }

    #[test]
    fn prepared_kanban_binding_rejects_invalid_derived_colors() {
        let cfg = json!({ "themeVariables": { "cScale0": "var(--runtime-color)" } });
        let error = KanbanCssBinding::for_test(&cfg, None).unwrap_err();
        assert!(matches!(error, merman_render_core::Error::Color(_)));
    }
}
