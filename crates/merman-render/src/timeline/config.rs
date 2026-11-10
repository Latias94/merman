use crate::config::{config_bool, config_f64, config_f64_css_px, config_string};
use crate::text::TextStyle;
use serde_json::Value;

const DEFAULT_LEFT_MARGIN: f64 = 150.0;
const DEFAULT_VIEWBOX_PADDING: f64 = 50.0;
const DEFAULT_USE_MAX_WIDTH: bool = false;
const DEFAULT_FONT_SIZE: f64 = 16.0;
pub(crate) const DEFAULT_THEME_COLOR_LIMIT: usize = 12;
pub(crate) const MAX_THEME_COLOR_LIMIT: usize = 64;

#[derive(Debug)]
pub(super) struct TimelineFontSize {
    pub(super) css: String,
    pub(super) used_px: Option<f64>,
}

impl Default for TimelineFontSize {
    fn default() -> Self {
        Self {
            css: format!("{DEFAULT_FONT_SIZE}px"),
            used_px: Some(DEFAULT_FONT_SIZE),
        }
    }
}

/// Resolve Mermaid's bounded Timeline color-scale count through the shared value-coercion helper so CSS,
/// layout, and the typed terminal plan share JavaScript-compatible coercion and ceilings.
pub(crate) fn timeline_theme_color_limit(effective_config: &Value) -> usize {
    let raw = effective_config
        .get("themeVariables")
        .and_then(|variables| variables.get("THEME_COLOR_LIMIT"));
    raw.map_or(DEFAULT_THEME_COLOR_LIMIT, |value| {
        // Render paths receive core-materialized configuration, where an over-limit value has
        // already failed admission. Keep direct adapter tests deterministic if they bypass that
        // boundary instead of inventing a second unbounded policy.
        merman_model::presentation::theme_color_iterations(Some(value))
            .unwrap_or(DEFAULT_THEME_COLOR_LIMIT)
    })
}

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

    /// Neo's gradient stylesheet replaces both Classic and Redux section paint owners.
    pub(crate) fn uses_neo_gradient(&self) -> bool {
        crate::config::config_diagram_look(self.effective_config).is_neo()
            && config_bool(self.effective_config, &["themeVariables", "useGradient"])
                .unwrap_or(false)
            && self.effective_config.get("theme").and_then(Value::as_str) != Some("neutral")
    }

    pub(crate) fn layout_settings(&self) -> TimelineLayoutSettings {
        let text_style = self.text_style();
        let node_text_style =
            super::TimelineCssBinding::resolve(self.effective_config).node_text_style(&text_style);
        TimelineLayoutSettings {
            text_style: text_style.clone(),
            node_text_style,
            layout_font_size: config_f64_css_px(self.effective_config, &["fontSize"])
                .unwrap_or(text_style.font_size)
                .max(0.0),
            theme_color_limit: timeline_theme_color_limit(self.effective_config),
            is_neo: crate::config::config_diagram_look(self.effective_config).is_neo(),
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

    pub(crate) fn layout_settings_with_resolved_typography(
        &self,
        resolved_font_family_css: Option<&str>,
        resolved_font_size_px: Option<f64>,
    ) -> TimelineLayoutSettings {
        let mut settings = self.layout_settings();
        if let Some(font_family_css) = resolved_font_family_css {
            settings.text_style.font_family = Some(font_family_css.to_owned());
            settings.node_text_style.font_family = Some(font_family_css.to_owned());
        }
        if let Some(font_size_px) = resolved_font_size_px {
            settings.text_style.font_size = font_size_px.max(0.0);
            settings.node_text_style.font_size = font_size_px.max(0.0);
        }
        settings
    }

    pub(super) fn configured_font_size(&self) -> Option<TimelineFontSize> {
        let value = self.effective_config.pointer("/themeVariables/fontSize")?;
        if let Some(size) = value.as_f64() {
            return (size.is_finite() && size >= 0.0).then(|| TimelineFontSize {
                css: format!("{size}px"),
                used_px: Some(size),
            });
        }
        let raw = value.as_str()?.trim();
        let declaration = format!("font-size:{raw}");
        let parsed = crate::mermaid_style::parse_style_declaration(&declaration);
        // This value enters an ordinary stylesheet declaration, not CSSOM setProperty.
        // Parse its optional terminator before applying the single-value ownership boundary.
        let css = parsed.as_ref().map_or(raw, |parsed| parsed.source_value());
        crate::mermaid_style::css_font_size_value_override(css)?;
        // Relative root sizes need the receiving ancestor. Literal zero is independent of
        // that ancestor; expressions and unrepresentable sizes retain unknown visibility.
        let used_px = parsed.as_ref().and_then(|parsed| {
            if parsed.font_size_is_literal_zero() {
                Some(0.0)
            } else {
                parsed
                    .analysis()
                    .context_free_font_size_px()
                    .filter(|size| *size > 0.0 && (*size as f32).is_normal())
            }
        });
        Some(TimelineFontSize {
            css: css.to_owned(),
            used_px,
        })
    }

    pub(super) fn render_font_size_px(&self) -> f64 {
        // Root fontSize controls layout offsets, not CSS inheritance. This estimate is for
        // layout only; the typography plan retains the actual CSS and used-size facts.
        self.configured_font_size()
            .and_then(|size| size.used_px)
            .unwrap_or(DEFAULT_FONT_SIZE)
    }

    fn text_style(&self) -> TextStyle {
        TextStyle {
            font_family: self.font_family(),
            font_size: self.render_font_size_px(),
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
pub(crate) struct TimelineLayoutSettings {
    // getVirtualNodeHeight() has no section class; drawNode() attaches it before wrap()/getBBox().
    pub(crate) text_style: TextStyle,
    pub(crate) node_text_style: TextStyle,
    pub(crate) layout_font_size: f64,
    pub(crate) theme_color_limit: usize,
    pub(crate) is_neo: bool,
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
                    settings.node_text_style.font_weight.as_deref(),
                    theme.contains("redux").then_some("900"),
                    "{theme}/{look}"
                );
            }
        }
    }

    #[test]
    fn timeline_section_weight_requires_an_emitted_section_rule() {
        let cfg = json!({
            "theme": "redux",
            "themeVariables": {"THEME_COLOR_LIMIT": 0, "fontWeight": 900}
        });
        let settings = TimelineConfigView::new(&cfg).layout_settings();
        assert_eq!(settings.node_text_style.font_weight, None);
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
                settings.node_text_style.font_weight.as_deref(),
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
            settings.node_text_style.font_weight.as_deref(),
            Some("var(--section-weight)")
        );
    }

    #[test]
    fn timeline_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = TimelineConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.left_margin, DEFAULT_LEFT_MARGIN);
        assert_eq!(settings.theme_color_limit, DEFAULT_THEME_COLOR_LIMIT);
        assert!(!settings.is_neo);
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
        assert_eq!(settings.theme_color_limit, DEFAULT_THEME_COLOR_LIMIT);
        assert!(!settings.is_neo);
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

    #[test]
    fn timeline_theme_color_limit_preserves_the_css_adapter_domain() {
        assert_eq!(timeline_theme_color_limit(&json!({})), 12);
        assert_eq!(
            timeline_theme_color_limit(&json!({
                "themeVariables": { "THEME_COLOR_LIMIT": 2.9 }
            })),
            3
        );
        assert_eq!(
            timeline_theme_color_limit(&json!({
                "themeVariables": { "THEME_COLOR_LIMIT": 0 }
            })),
            0
        );
        assert_eq!(
            timeline_theme_color_limit(&json!({
                "themeVariables": { "THEME_COLOR_LIMIT": 100 }
            })),
            DEFAULT_THEME_COLOR_LIMIT
        );
    }
}
