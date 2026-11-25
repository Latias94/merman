use merman_model::presentation::PresentationConfig;

use crate::config::config_font_family_css;
use crate::svg::PreparedCommonCss;
use crate::text::TextStyle;
use crate::theme::{FamilyTheme, font_family_css};

use super::config::TimelineFontSize;
use super::{TimelineCssBinding, TimelineNativeStyle};

#[derive(Debug)]
enum TimelineAppearance {
    Mermaid(TimelineCssBinding),
    Native(TimelineNativeStyle),
}

#[derive(Debug)]
pub(crate) struct TimelineStyle {
    appearance: TimelineAppearance,
    common_css: PreparedCommonCss,
    layout_settings: super::config::TimelineLayoutSettings,
    used_font_size_px: Option<f64>,
}

impl TimelineStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        match native {
            Some(theme) => Self::resolve_native(config, theme),
            None => Self::resolve_mermaid(config),
        }
    }

    fn resolve_mermaid(config: &PresentationConfig) -> Self {
        let config_view = super::TimelineConfigView::new(config.as_value());
        let font_size = config_view.configured_font_size().unwrap_or_default();
        let font_family = config_font_family_css(config);
        let font_size_px = font_size.used_px.unwrap_or(16.0);
        let css = TimelineCssBinding::resolve(config.as_value());
        let text_style = TextStyle {
            font_family: Some(font_family.clone()),
            font_size: font_size_px,
            font_weight: None,
            font_style: None,
        };
        let node_text_style = css.node_text_style(&text_style);
        let layout_settings = config_view.layout_settings_for(
            text_style,
            node_text_style,
            None,
            config_view.root_font_size_px().unwrap_or(font_size_px),
        );
        Self {
            appearance: TimelineAppearance::Mermaid(css),
            common_css: PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                font_family,
                font_size.css,
            ),
            layout_settings,
            used_font_size_px: font_size.used_px,
        }
    }

    fn resolve_native(config: &PresentationConfig, theme: &FamilyTheme) -> Self {
        let config_view = super::TimelineConfigView::new(config.as_value());
        let typography = &theme.typography;
        let source_family = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| config.explicit_config_owns_path(path))
            .find_map(|path| {
                config
                    .get_str(path)
                    .and_then(crate::config::normalized_config_font_family)
            });
        let body_family = source_family
            .clone()
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
        let title_family = source_family
            .unwrap_or_else(|| font_family_css(typography.stack(typography.title_family)));
        let has_config_size = config.explicit_config_owns_path("themeVariables.fontSize");
        let font_size = if has_config_size {
            config_view.configured_font_size().unwrap_or_default()
        } else {
            TimelineFontSize {
                css: format!("{}px", typography.base_size_px),
                used_px: Some(typography.base_size_px),
            }
        };
        let font_size_px = font_size.used_px.unwrap_or(16.0);
        let layout_font_size = if config.explicit_config_owns_path("fontSize") {
            config_view.root_font_size_px().unwrap_or_else(|| {
                if has_config_size {
                    font_size_px
                } else {
                    config_view.render_font_size_px()
                }
            })
        } else {
            font_size_px
        };
        let is_redux = config
            .get_str("theme")
            .is_some_and(|theme| theme.contains("redux"));
        let configured_weight = (is_redux
            && config.explicit_config_owns_path("themeVariables.fontWeight")
            && super::timeline_theme_color_limit(config.as_value()) != 0)
            .then(|| {
                crate::config::config_css_number_or_string(
                    config,
                    &["themeVariables", "fontWeight"],
                )
                .unwrap_or_else(|| "normal".into())
            });
        let label_weight = typography.label_weight.to_string();
        let node_weight = configured_weight
            .as_deref()
            .unwrap_or(&label_weight)
            .to_owned();
        let mut text_style = TextStyle {
            font_family: Some(body_family.clone()),
            font_size: font_size_px,
            font_weight: None,
            font_style: None,
        };
        // Explicit Redux weight inherits from the unweighted measurement style, as in Mermaid.
        let mut node_text_style =
            TimelineCssBinding::node_style_for_weight(&text_style, configured_weight.as_deref());
        if configured_weight.is_none() {
            node_text_style.font_weight = Some(label_weight.clone());
        }
        text_style.font_weight = Some(label_weight);
        let title_style = TextStyle {
            font_family: Some(title_family),
            font_size: font_size_px * 1.9375,
            font_weight: Some(typography.title_weight.to_string()),
            font_style: None,
        };
        let native = TimelineNativeStyle::new(theme, config, &config_view, is_redux, node_weight);
        let common_css = PreparedCommonCss::with_resolved_typography(
            config.as_value(),
            body_family,
            font_size.css,
        )
        .with_text_color(&native.text);
        Self {
            appearance: TimelineAppearance::Native(native),
            common_css,
            layout_settings: config_view.layout_settings_for(
                text_style,
                node_text_style,
                Some(title_style),
                layout_font_size,
            ),
            used_font_size_px: font_size.used_px,
        }
    }

    pub(crate) fn native(&self) -> Option<&TimelineNativeStyle> {
        match &self.appearance {
            TimelineAppearance::Native(native) => Some(native),
            TimelineAppearance::Mermaid(_) => None,
        }
    }

    pub(crate) fn css_binding(&self) -> Option<&TimelineCssBinding> {
        match &self.appearance {
            TimelineAppearance::Mermaid(css) => Some(css),
            TimelineAppearance::Native(_) => None,
        }
    }

    pub(crate) fn gradient_stops(&self) -> Option<&[Option<String>; 2]> {
        match &self.appearance {
            TimelineAppearance::Native(native) => native.gradient_stops.as_ref(),
            TimelineAppearance::Mermaid(css) => css.use_neo_gradient.then_some(&css.gradient_stops),
        }
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn layout_settings(&self) -> &super::config::TimelineLayoutSettings {
        &self.layout_settings
    }

    pub(crate) const fn used_font_size_px(&self) -> Option<f64> {
        self.used_font_size_px
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_timeline_projects_authored_gradient_and_redux_font_weight() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "redux", "look": "neo", "fontSize": "18px",
            "themeVariables": {
                "fontSize": "24px", "fontWeight": "bold !important",
                "useGradient": true, "gradientStop": "#112233"
            }
        }));
        let theme = crate::theme::ThemeCompiler::new()
            .compile_preset(crate::theme::BuiltinPreset::Studio, None)
            .unwrap();
        let plan = TimelineStyle::resolve(&config, Some(theme.family("timeline")));
        let settings = plan.layout_settings();
        assert!(plan.css_binding().is_none());
        assert!(settings.is_neo);
        assert_eq!(settings.layout_font_size, 18.0);
        assert_eq!(settings.node_text_style.font_size, 24.0);
        assert_eq!(settings.node_text_style.font_weight.as_deref(), Some("700"));
        assert_eq!(plan.native().unwrap().node_weight, "bold !important");
        let [start, stop] = plan.gradient_stops().unwrap();
        assert_eq!(start.as_deref(), None);
        assert_eq!(stop.as_deref(), Some("#112233"));
    }

    #[test]
    fn prepared_layout_preserves_independent_root_font_offset() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "redux", "fontSize": "18px",
            "themeVariables": {"fontSize": "24px", "fontWeight": 900, "textColor": "var(--text)"}
        }));
        let plan = TimelineStyle::resolve(&config, None);
        assert_eq!(plan.layout_settings().text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().node_text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().text_style.font_weight, None);
        assert_eq!(
            plan.layout_settings()
                .node_text_style
                .font_weight
                .as_deref(),
            plan.css_binding().unwrap().section_font_weight()
        );
        assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        assert_eq!(plan.common_css().text_color(), "var(--text)");
        assert_eq!(
            plan.layout_settings().text_style.font_family.as_deref(),
            Some(plan.common_css().font_family())
        );
        assert_eq!(
            plan.layout_settings().node_text_style.font_family,
            plan.layout_settings().text_style.font_family
        );
        for size in [0.0, 0.5] {
            let config = crate::test_support::presentation_config(serde_json::json!({
                "fontSize": "18px", "themeVariables": {"fontSize": format!("{size}px")}
            }));
            let plan = TimelineStyle::resolve(&config, None);
            assert_eq!(plan.layout_settings().text_style.font_size, size);
            assert_eq!(plan.layout_settings().node_text_style.font_size, size);
            assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        }
    }
}
