use crate::config::{config_string, json_value_is_truthy};
use crate::svg::PreparedCommonCss;
use merman_markup::theme_color::{darken, lighten};
use merman_model::presentation::PresentationConfig;
use serde_json::Value;

use super::KanbanTaskTheme;

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
    pub(super) fn resolve(
        config: &Value,
        tasks: &KanbanTaskTheme,
        resolved_text_color: Option<&str>,
        dark_mode: bool,
    ) -> crate::Result<Self> {
        let common = match tasks.resolved_typography_for_css() {
            Some((family, size)) => PreparedCommonCss::with_resolved_typography(
                config,
                family.to_owned(),
                size.to_owned(),
            ),
            None => PreparedCommonCss::new(config, Some(tasks.font_family_css()))
                .with_configured_root_font_family(config),
        };
        Self::from_common(config, common, resolved_text_color, dark_mode)
    }

    fn from_common(
        config: &Value,
        mut common: PreparedCommonCss,
        resolved_text_color: Option<&str>,
        dark_mode: bool,
    ) -> crate::Result<Self> {
        if let Some(color) = resolved_text_color {
            common = common.with_text_color(color);
        }
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_string())
        };
        let sections = (0..12)
            .map(|index| {
                let [scale, label, inverse] =
                    crate::svg::render_theme::kanban_section_defaults(index);
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
            .collect::<crate::Result<Vec<_>>>()?;
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
    pub(crate) fn for_test(
        config: &Value,
        typography: Option<(&str, &str)>,
    ) -> crate::Result<Self> {
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
            None,
            dark_mode(&crate::test_support::presentation_config(config.clone())),
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
}
