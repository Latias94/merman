use crate::config::{config_bool, config_css_number_or_string, config_string, config_string_vec};

#[derive(Debug)]
pub(crate) struct TimelineSectionCssBinding {
    pub(crate) c_scale: String,
    pub(crate) c_scale_label: String,
    pub(crate) c_scale_inv: String,
    source_scale_present: bool,
    source_inverse_present: bool,
}

/// Operation-owned visual inputs, including browser CSS and absent gradient stops.
#[derive(Debug)]
pub(crate) struct TimelineCssBinding {
    pub(crate) is_redux_theme: bool,
    pub(crate) is_dark_theme: bool,
    pub(crate) is_color_theme: bool,
    pub(crate) is_neo: bool,
    pub(crate) use_neo_gradient: bool,
    pub(crate) stroke_width: String,
    pub(crate) font_weight: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) disabled_fill: String,
    pub(crate) disabled_text_fill: String,
    pub(crate) root_fill: String,
    pub(crate) root_label: String,
    pub(crate) border_colors: Vec<String>,
    pub(crate) sections: Vec<TimelineSectionCssBinding>,
    pub(crate) gradient_stops: [Option<String>; 2],
    source_border_slots: Vec<bool>,
    pub(crate) is_dark_name: bool,
}

impl TimelineCssBinding {
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        let theme = config_string(config, &["theme"]).unwrap_or_else(|| "default".into());
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.into())
        };
        let css = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.into())
        };
        let section_count = super::timeline_theme_color_limit(config);
        let sections = (0..section_count)
            .map(|index| {
                let [fill, label, inverse] =
                    crate::svg::render_theme::timeline_default_section_colors(index);
                let source_fill =
                    config_string(config, &["themeVariables", &format!("cScale{index}")]);
                let source_inverse =
                    config_string(config, &["themeVariables", &format!("cScaleInv{index}")]);
                TimelineSectionCssBinding {
                    source_scale_present: source_fill.is_some(),
                    source_inverse_present: source_inverse.is_some(),
                    c_scale: source_fill.unwrap_or_else(|| fill.into()),
                    c_scale_label: color(&format!("cScaleLabel{index}"), label),
                    c_scale_inv: source_inverse.unwrap_or_else(|| inverse.into()),
                }
            })
            .collect();
        Self {
            is_redux_theme: theme.contains("redux"),
            is_color_theme: theme.contains("color"),
            is_dark_name: theme.contains("dark"),
            is_dark_theme: config_bool(config, &["darkMode"])
                .or_else(|| config_bool(config, &["themeVariables", "darkMode"]))
                .unwrap_or_else(|| theme.contains("dark")),
            is_neo: crate::config::config_diagram_look(config).is_neo(),
            use_neo_gradient: super::TimelineConfigView::new(config).uses_neo_gradient(),
            stroke_width: css("strokeWidth", "1"),
            font_weight: css("fontWeight", "normal"),
            main_bkg: color("mainBkg", "#ECECFF"),
            node_border: color("nodeBorder", "#9370DB"),
            disabled_fill: color("tertiaryColor", "lightgray"),
            disabled_text_fill: color("clusterBorder", "#efefef"),
            root_fill: color("git0", "hsl(240, 100%, 46.2745098039%)"),
            root_label: color("gitBranchLabel0", "#ffffff"),
            border_colors: config_string_vec(config, &["themeVariables", "borderColorArray"]),
            sections,
            gradient_stops: ["gradientStart", "gradientStop"]
                .map(|key| config_string(config, &["themeVariables", key])),
            source_border_slots: crate::config::value_at(
                config,
                &["themeVariables", "borderColorArray"],
            )
            .and_then(serde_json::Value::as_array)
            .map(|values| values.iter().map(serde_json::Value::is_string).collect())
            .unwrap_or_default(),
        }
    }

    pub(crate) fn source_scale(&self, slot: usize) -> Option<&str> {
        let section = self.sections.get(slot)?;
        section
            .source_scale_present
            .then_some(section.c_scale.as_str())
    }

    pub(crate) fn source_inverse(&self, slot: usize) -> Option<&str> {
        let section = self.sections.get(slot)?;
        section
            .source_inverse_present
            .then_some(section.c_scale_inv.as_str())
    }

    pub(crate) fn has_source_border_slot(&self, slot: usize) -> bool {
        self.source_border_slots.get(slot).copied().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_preserves_existing_explicit_role_contract() {
        let timeline = TimelineCssBinding::resolve(&serde_json::json!({
            "theme": "redux-color",
            "themeVariables": {
                "THEME_COLOR_LIMIT": 2, "strokeWidth": 5, "fontWeight": 600,
                "mainBkg": "#111827", "nodeBorder": "#38bdf8",
                "tertiaryColor": "#334155", "clusterBorder": "#f97316",
                "git0": "#22c55e", "gitBranchLabel0": "#020617",
                "borderColorArray": ["#ef4444", "#f59e0b"],
                "cScale0": "#ef4444", "cScaleLabel0": "#e879f9", "cScaleInv0": "#334155",
                "cScale1": "#172554", "cScaleLabel1": "#f8fafc", "cScaleInv1": "#475569"
            }
        }));
        assert!(timeline.is_redux_theme);
        assert!(!timeline.is_dark_theme);
        assert!(timeline.is_color_theme);
        assert_eq!(timeline.stroke_width, "5");
        assert_eq!(timeline.font_weight, "600");
        assert_eq!(timeline.main_bkg, "#111827");
        assert_eq!(timeline.node_border, "#38bdf8");
        assert_eq!(timeline.disabled_fill, "#334155");
        assert_eq!(timeline.disabled_text_fill, "#f97316");
        assert_eq!(timeline.root_fill, "#22c55e");
        assert_eq!(timeline.root_label, "#020617");
        assert_eq!(timeline.border_colors, ["#ef4444", "#f59e0b"]);
        assert_eq!(timeline.sections.len(), 2);
        assert_eq!(timeline.sections[0].c_scale, "#ef4444");
        assert_eq!(timeline.sections[0].c_scale_label, "#e879f9");
        assert_eq!(timeline.sections[0].c_scale_inv, "#334155");
        assert_eq!(timeline.sections[1].c_scale, "#172554");
        assert_eq!(timeline.sections[1].c_scale_label, "#f8fafc");
        assert_eq!(timeline.sections[1].c_scale_inv, "#475569");
    }

    #[test]
    fn binding_preserves_existing_default_role_contract() {
        let timeline = TimelineCssBinding::resolve(&serde_json::json!({}));
        assert!(!timeline.is_redux_theme);
        assert!(!timeline.is_dark_theme);
        assert!(!timeline.is_color_theme);
        assert_eq!(timeline.stroke_width, "1");
        assert_eq!(timeline.font_weight, "normal");
        assert_eq!(timeline.main_bkg, "#ECECFF");
        assert_eq!(timeline.node_border, "#9370DB");
        assert_eq!(timeline.disabled_fill, "lightgray");
        assert_eq!(timeline.disabled_text_fill, "#efefef");
        assert_eq!(timeline.root_fill, "hsl(240, 100%, 46.2745098039%)");
        assert_eq!(timeline.root_label, "#ffffff");
        assert!(timeline.border_colors.is_empty());
        assert_eq!(timeline.sections.len(), 12);
        assert_eq!(
            timeline.sections[0].c_scale,
            "hsl(240, 100%, 76.2745098039%)"
        );
        assert_eq!(timeline.sections[0].c_scale_label, "#ffffff");
        assert_eq!(
            timeline.sections[0].c_scale_inv,
            "hsl(60, 100%, 86.2745098039%)"
        );
        assert_eq!(timeline.sections[1].c_scale_label, "black");
    }

    #[test]
    fn binding_preserves_raw_css_and_independent_absent_gradient_stop() {
        let binding = TimelineCssBinding::resolve(&serde_json::json!({
            "theme": "redux-dark-color", "darkMode": false, "look": "neo",
            "themeVariables": {
                "THEME_COLOR_LIMIT": 2, "cScale0": "var(--section)",
                "strokeWidth": "calc(1px + 2px)", "fontWeight": 500,
                "useGradient": true, "gradientStart": "currentColor", "gradientStop": null
            }
        }));
        assert_eq!(binding.sections.len(), 2);
        assert_eq!(binding.sections[0].c_scale, "var(--section)");
        assert_eq!(binding.stroke_width, "calc(1px + 2px)");
        assert_eq!(binding.font_weight, "500");
        assert!(binding.is_redux_theme && binding.is_color_theme && binding.use_neo_gradient);
        assert!(!binding.is_dark_theme);
        assert_eq!(binding.gradient_stops, [Some("currentColor".into()), None]);
    }

    #[test]
    fn source_presence_retains_original_border_array_slots() {
        let binding = TimelineCssBinding::resolve(&serde_json::json!({
            "themeVariables": {"borderColorArray": [null, "var(--border)"], "cScale0": null}
        }));
        assert!(!binding.has_source_border_slot(0));
        assert!(binding.has_source_border_slot(1));
        assert_eq!(binding.border_colors, ["var(--border)"]);
        assert_eq!(binding.source_scale(0), None);
        assert_eq!(binding.source_inverse(0), None);
        assert!(!binding.sections[0].c_scale.is_empty());
    }
}
