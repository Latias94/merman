use crate::config::{
    config_bool, config_f64, config_f64_css_px, config_string, config_theme_or_root_font_size_px,
};
use crate::text::TextStyle;
use serde_json::Value;

const DEFAULT_LEFT_MARGIN: f64 = 150.0;
const DEFAULT_VIEWBOX_PADDING: f64 = 50.0;
const DEFAULT_USE_MAX_WIDTH: bool = false;
const DEFAULT_FONT_SIZE: f64 = 16.0;

pub(crate) struct TimelineConfigView<'a> {
    effective_config: &'a Value,
    timeline_config: &'a Value,
}

impl<'a> TimelineConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            timeline_config: effective_config.get("timeline").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(&self) -> TimelineLayoutSettings {
        let text_style = self.text_style();
        let theme =
            crate::svg::render_theme::PresentationTheme::new(self.effective_config).timeline();
        let mut node_text_style = text_style.clone();
        if theme.is_redux_theme && !theme.sections.is_empty() {
            node_text_style = crate::text::apply_css_font_declarations(
                &text_style,
                &format!("font-weight:{}", theme.font_weight),
            );
            if node_text_style.font_weight.is_none()
                && !crate::text::css_font_value_inherits(&theme.font_weight)
            {
                node_text_style.font_weight = Some(theme.font_weight);
            }
        }
        TimelineLayoutSettings {
            text_style: text_style.clone(),
            node_typography: TimelineNodeTypography {
                inherited: text_style.clone(),
                section: node_text_style,
                section_rule_count: theme.sections.len(),
            },
            layout_font_size: config_f64_css_px(self.effective_config, &["fontSize"])
                .unwrap_or(text_style.font_size)
                .max(1.0),
            left_margin: config_f64(self.timeline_config, &["leftMargin"])
                .unwrap_or(DEFAULT_LEFT_MARGIN)
                .max(0.0),
            disable_multicolor: config_bool(self.timeline_config, &["disableMulticolor"])
                .unwrap_or(false),
            viewbox_padding: config_f64(self.timeline_config, &["padding"])
                .unwrap_or(DEFAULT_VIEWBOX_PADDING)
                .max(0.0),
            use_max_width: config_bool(self.timeline_config, &["useMaxWidth"])
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
        }
    }

    fn text_style(&self) -> TextStyle {
        TextStyle {
            font_family: self.font_family(),
            font_size: config_theme_or_root_font_size_px(self.effective_config, DEFAULT_FONT_SIZE)
                .max(1.0),
            font_weight: None,
            font_style: None,
        }
    }

    fn font_family(&self) -> Option<String> {
        config_string(self.effective_config, &["themeVariables", "fontFamily"])
            .or_else(|| config_string(self.effective_config, &["fontFamily"]))
            .map(|s| s.trim().trim_end_matches(';').trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineNodeTypography {
    inherited: TextStyle,
    section: TextStyle,
    section_rule_count: usize,
}

impl TimelineNodeTypography {
    pub(crate) fn for_section(&self, full_section: i64) -> &TextStyle {
        self.for_index(super::section_index(full_section))
    }

    pub(crate) fn for_class(&self, class: &str) -> &TextStyle {
        class
            .strip_prefix("section-")
            .and_then(|index| index.parse().ok())
            .map_or(&self.inherited, |index| self.for_index(index))
    }

    fn for_index(&self, index: i64) -> &TextStyle {
        // Mermaid 12.1 assigns classes on a fixed ring; configured CSS may cover fewer slots.
        let has_rule = index
            .checked_add(1)
            .and_then(|slot| usize::try_from(slot).ok())
            .is_some_and(|slot| slot < self.section_rule_count);
        if has_rule {
            &self.section
        } else {
            &self.inherited
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineLayoutSettings {
    // getVirtualNodeHeight() has no section class; drawNode() attaches it before wrap()/getBBox().
    pub(crate) text_style: TextStyle,
    pub(crate) node_typography: TimelineNodeTypography,
    pub(crate) layout_font_size: f64,
    pub(crate) left_margin: f64,
    pub(crate) disable_multicolor: bool,
    pub(crate) viewbox_padding: f64,
    pub(crate) use_max_width: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn timeline_section_weight_keeps_virtual_probes_normal() {
        for look in ["classic", "neo"] {
            for theme in [
                "default",
                "redux",
                "redux-color",
                "redux-dark",
                "redux-dark-color",
            ] {
                let cfg = json!({
                    "theme": theme, "look": look,
                    "themeVariables": {"fontWeight": 900}
                });
                let settings = TimelineConfigView::new(&cfg).layout_settings();
                assert_eq!(settings.text_style.font_weight, None);
                assert_eq!(
                    settings
                        .node_typography
                        .for_section(0)
                        .font_weight
                        .as_deref(),
                    theme.contains("redux").then_some("900"),
                    "{theme}/{look}"
                );
            }
        }
    }

    #[test]
    fn timeline_section_weight_priority_is_not_part_of_the_measured_face() {
        for (raw, expected) in [
            ("900 !important", Some("900")),
            ("bold!important", Some("700")),
            ("inherit !important", None),
            ("unset!important", None),
        ] {
            let config = json!({"theme": "redux", "themeVariables": {"fontWeight": raw}});
            let settings = TimelineConfigView::new(&config).layout_settings();
            assert_eq!(settings.text_style.font_weight, None);
            assert_eq!(
                settings
                    .node_typography
                    .for_section(0)
                    .font_weight
                    .as_deref(),
                expected,
                "{raw}"
            );
        }
    }

    #[test]
    fn timeline_dynamic_section_weight_remains_an_unresolved_css_input() {
        let cfg = json!({
            "theme": "redux",
            "themeVariables": {"fontWeight": "var(--section-weight)"}
        });
        let settings = TimelineConfigView::new(&cfg).layout_settings();
        assert_eq!(settings.text_style.font_weight, None);
        assert_eq!(
            settings
                .node_typography
                .for_section(0)
                .font_weight
                .as_deref(),
            Some("var(--section-weight)")
        );
    }

    #[test]
    fn timeline_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = TimelineConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.left_margin, DEFAULT_LEFT_MARGIN);
        assert_eq!(settings.viewbox_padding, DEFAULT_VIEWBOX_PADDING);
        assert!(!settings.disable_multicolor);
        assert!(!settings.use_max_width);
        assert_eq!(settings.text_style.font_family, None);
        assert_eq!(settings.text_style.font_size, DEFAULT_FONT_SIZE);
        assert_eq!(settings.layout_font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn timeline_layout_settings_project_configured_values() {
        let cfg = json!({
            "fontFamily": "Inter, sans-serif",
            "themeVariables": {
                "fontSize": "20px"
            },
            "fontSize": "18px",
            "timeline": {
                "leftMargin": "180",
                "disableMulticolor": true,
                "padding": "12",
                "useMaxWidth": true
            }
        });
        let settings = TimelineConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.left_margin, 180.0);
        assert_eq!(settings.viewbox_padding, 12.0);
        assert!(settings.disable_multicolor);
        assert!(settings.use_max_width);
        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some("Inter, sans-serif")
        );
        assert_eq!(settings.text_style.font_size, 20.0);
        assert_eq!(settings.layout_font_size, 18.0);
    }
}
